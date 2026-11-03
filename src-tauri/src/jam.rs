// Never lose an idea (docs/design.md#ai-and-learning-rethought-2026-11-03).
// Jello records every press. After a pause in a jam, the rail asks for the
// moments worth keeping, and Rust measures candidates: phrases played again
// in about the same rhythm, and phrases that end on a long hold. With a
// model, the AI picks the ones worth keeping and names each; without one,
// the card shows the best three by measure. Each has Play, Loop (a new loop
// layer) and Section (a new section).

use std::sync::Mutex;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent;
use crate::compose::Composer;
use crate::engine::Engine;
use crate::layer_notes::{self, WrittenNote};
use crate::phrases::{self, KeyEvent, Phrase, Phrases};
use crate::sections::{self, SectionEdit};
use crate::song_file::note_name;
use crate::tool_server;

// How many candidates are measured at most, for the AI to pick from.
const MOST_CANDIDATES: usize = 8;
// The shortest and longest phrase looked for, in presses.
const FEWEST_PRESSES: usize = 3;
const MOST_PRESSES: usize = 8;
// How far apart two plays of a phrase's gaps may be: a share of the gap,
// or this many ms, whichever is more.
const RHYTHM_SHARE: f64 = 0.3;
const RHYTHM_LEAST_MS: f64 = 80.0;
// A hold this long ends a phrase, and the presses before it count if they
// came within this long.
const LONG_HOLD_MS: u64 = 1200;
const HELD_PHRASE_MS: u64 = 4000;

// A press of the jam: its key, its notes, and when, relative to the jam.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JamPress {
    pub key: String,
    pub notes: Vec<u8>,
    #[serde(rename = "startMs")]
    pub start_ms: u64,
    #[serde(rename = "holdMs")]
    pub hold_ms: u64,
}

// A moment worth keeping: its presses from 0 ms, a name in a few words,
// and how strongly it was measured.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Moment {
    pub name: String,
    pub notes: String,
    pub presses: Vec<JamPress>,
    pub score: f64,
}

// A candidate moment: the presses it covers, by place.
struct Candidate {
    first: usize,
    count: usize,
    name: String,
    score: f64,
}

// The jam's best moments, best first, at most `most`, never two sharing a
// press. `presses` are in the order they went down.
pub fn moments_of(presses: &[JamPress], most: usize) -> Vec<Moment> {
    let mut candidates = repeated_phrases(presses);
    candidates.extend(held_endings(presses));
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));

    let mut taken = vec![false; presses.len()];
    let mut moments = Vec::new();
    for candidate in candidates {
        if moments.len() == most {
            break;
        }
        let places = candidate.first..candidate.first + candidate.count;
        if places.clone().any(|place| taken[place]) {
            continue;
        }
        for place in places.clone() {
            taken[place] = true;
        }
        moments.push(moment_of(&presses[places], candidate.name, candidate.score));
    }
    moments
}

fn moment_of(presses: &[JamPress], name: String, score: f64) -> Moment {
    let first_ms = presses[0].start_ms;
    let mut moved = Vec::new();
    let mut names = Vec::new();
    for press in presses {
        let mut press = press.clone();
        press.start_ms -= first_ms;
        names.push(notes_name(&press.notes));
        moved.push(press);
    }
    Moment {
        name,
        notes: names.join(" "),
        presses: moved,
        score,
    }
}

fn notes_name(notes: &[u8]) -> String {
    let mut names = Vec::new();
    for note in notes {
        names.push(note_name(*note));
    }
    names.join("+")
}

// Phrases of FEWEST_PRESSES to MOST_PRESSES presses played again later,
// with the same notes in about the same rhythm. The score grows with the
// length and the plays.
fn repeated_phrases(presses: &[JamPress]) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for count in FEWEST_PRESSES..=MOST_PRESSES {
        if count * 2 > presses.len() {
            break;
        }
        for first in 0..=presses.len() - count {
            let plays = plays_of(presses, first, count);
            if plays < 2 {
                continue;
            }
            candidates.push(Candidate {
                first,
                count,
                name: format!("Played {plays} times"),
                score: (count * plays) as f64,
            });
        }
    }
    candidates
}

// How many times the phrase of `count` presses at `first` is played, from
// there on, without overlapping itself.
fn plays_of(presses: &[JamPress], first: usize, count: usize) -> usize {
    let phrase = &presses[first..first + count];
    let mut plays = 1;
    let mut next = first + count;
    while next + count <= presses.len() {
        if same_phrase(phrase, &presses[next..next + count]) {
            plays += 1;
            next += count;
        } else {
            next += 1;
        }
    }
    plays
}

fn same_phrase(a: &[JamPress], b: &[JamPress]) -> bool {
    for place in 0..a.len() {
        if a[place].notes != b[place].notes {
            return false;
        }
    }
    for place in 1..a.len() {
        let gap_a = (a[place].start_ms - a[place - 1].start_ms) as f64;
        let gap_b = (b[place].start_ms - b[place - 1].start_ms) as f64;
        let allowed = (gap_a * RHYTHM_SHARE).max(RHYTHM_LEAST_MS);
        if (gap_a - gap_b).abs() > allowed {
            return false;
        }
    }
    true
}

// Phrases that end on a hold of LONG_HOLD_MS or more, with at least two
// presses before it within HELD_PHRASE_MS.
fn held_endings(presses: &[JamPress]) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (last, press) in presses.iter().enumerate() {
        if press.hold_ms < LONG_HOLD_MS {
            continue;
        }
        let mut first = last;
        while first > 0 && last - first + 1 < MOST_PRESSES {
            let before = &presses[first - 1];
            if press.start_ms - before.start_ms > HELD_PHRASE_MS {
                break;
            }
            first -= 1;
        }
        let count = last - first + 1;
        if count < FEWEST_PRESSES {
            continue;
        }
        let held_seconds = press.hold_ms as f64 / 1000.0;
        candidates.push(Candidate {
            first,
            count,
            name: String::from("Held ending"),
            score: count as f64 + held_seconds,
        });
    }
    candidates
}

// The moments of the last jam, so its buttons can name one by place.
#[derive(Default)]
pub struct Jam {
    moments: Vec<Moment>,
}

impl Jam {
    fn moment(&self, place: usize) -> Result<Moment, String> {
        match self.moments.get(place) {
            Some(moment) => Ok(moment.clone()),
            None => Err(format!("there is no moment {}", place + 1)),
        }
    }
}

// What the rail's jam card shows: the moments, and the press to start the
// next jam from.
#[derive(Serialize)]
pub struct JamView {
    pub moments: Vec<Moment>,
    pub next: u64,
}

// The jam's moments from press `since` on. The rail asks after a pause.
#[tauri::command]
pub fn jam_moments(composer: State<'_, Mutex<Composer>>, jam: State<'_, Mutex<Jam>>, since: u64) -> JamView {
    let composer = composer.lock().unwrap();
    let presses = composer.jam_presses_from(since);
    let next = composer.next_press_id();
    drop(composer);
    let moments = moments_of(&presses, MOST_CANDIDATES);
    jam.lock().unwrap().moments = moments.clone();
    JamView { moments, next }
}

// A moment in one line for the AI, like "1. Played 2 times: 4 presses over
// 2.4 s, D4 E4 F#4 G#4".
fn moment_line(place: usize, moment: &Moment) -> String {
    let mut end_ms = 0;
    for press in &moment.presses {
        end_ms = end_ms.max(press.start_ms + press.hold_ms);
    }
    let seconds = end_ms as f64 / 1000.0;
    format!(
        "{}. {}: {} presses over {seconds:.1} s, {}",
        place + 1,
        moment.name,
        moment.presses.len(),
        moment.notes
    )
}

// The AI picks the moments worth keeping from the candidates of the jam
// that ends at press `jam`, and names each. The host answers with a
// "moments_picked" event. Without a running agent nothing is asked and the
// answer is false, so the card keeps Jello's own picks.
#[tauri::command]
pub fn pick_jam_moments(app: AppHandle, jam: State<'_, Mutex<Jam>>, next: u64) -> bool {
    let moments = jam.lock().unwrap().moments.clone();
    if moments.is_empty() {
        return false;
    }
    let mut lines = Vec::new();
    for (place, moment) in moments.iter().enumerate() {
        lines.push(moment_line(place, moment));
    }
    let message = json!({"type": "pick_moments", "jam": next, "moments": lines});
    agent::tell_host_if_running(&app, message)
}

// The moment as a phrase to play on the keys.
fn phrase_of_moment(moment: &Moment) -> Phrase {
    let mut events = Vec::new();
    let mut words = Vec::new();
    let mut length_ms = 0;
    for press in &moment.presses {
        let up_ms = press.start_ms + press.hold_ms;
        events.push(KeyEvent { at_ms: press.start_ms, key: press.key.clone(), down: true });
        events.push(KeyEvent { at_ms: up_ms, key: press.key.clone(), down: false });
        words.push(format!("{} at {} ms", notes_name(&press.notes), press.start_ms));
        length_ms = length_ms.max(up_ms);
    }
    events.sort_by_key(|event| (event.at_ms, event.down));
    Phrase { events, length_ms, presses: words }
}

// Play on a moment: its presses played on the keys again.
#[tauri::command]
pub fn play_jam_moment(app: AppHandle, jam: State<'_, Mutex<Jam>>, place: usize) -> Result<(), String> {
    let moment = jam.lock().unwrap().moment(place)?;
    if app.state::<Phrases>().is_playing() {
        return Err(String::from("something is playing: stop it first"));
    }
    app.state::<Mutex<Engine>>().lock().unwrap().start_audio()?;
    let phrase = phrase_of_moment(&moment);
    std::thread::spawn(move || {
        let result = tool_server::with_studio(&app, |studio| phrases::play(studio, &phrase));
        if let Err(why) = result {
            let _ = app.emit("jam-failed", why);
        }
    });
    Ok(())
}

// The moment's notes in beats at `tempo`, for a loop layer.
fn notes_in_beats(moment: &Moment, tempo: u32) -> Vec<WrittenNote> {
    let ms_per_beat = 60_000.0 / f64::from(tempo);
    let mut notes = Vec::new();
    for press in &moment.presses {
        let start = press.start_ms as f64 / ms_per_beat;
        let end = (press.start_ms + press.hold_ms) as f64 / ms_per_beat;
        for note in &press.notes {
            notes.push(WrittenNote { note: *note, start, end });
        }
    }
    notes
}

// Loop or Section on a moment: a new loop layer on the picked track, and
// for "section" a new section from the loop as it then plays.
fn keep_moment(composer: &mut Composer, engine: &mut Engine, moment: &Moment, how: &str) -> Result<String, String> {
    if how != "loop" && how != "section" {
        return Err(format!("a moment is kept as a loop or a section, not {how}"));
    }
    let notes = notes_in_beats(moment, composer.tempo());
    let written = layer_notes::write_layer(engine, None, None, None, &notes)?;
    composer.follow_tempo(engine);
    if how == "loop" {
        return Ok(written.line);
    }
    sections::edit(composer, engine, SectionEdit::Save)?;
    Ok(format!("{}; saved as a new section", written.line))
}

#[tauri::command]
pub fn keep_jam_moment(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    phrases: State<'_, Phrases>,
    jam: State<'_, Mutex<Jam>>,
    place: usize,
    how: String,
) -> Result<String, String> {
    if phrases.is_playing() {
        return Err(String::from("something is playing: stop it first"));
    }
    let moment = jam.lock().unwrap().moment(place)?;
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    keep_moment(&mut composer, &mut engine, &moment, &how)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(note: u8, start_ms: u64, hold_ms: u64) -> JamPress {
        JamPress { key: format!("Key{note}"), notes: vec![note], start_ms, hold_ms }
    }

    // `notes` played from `at`, one every `gap` ms, each held 200 ms.
    fn run(notes: &[u8], at: u64, gap: u64) -> Vec<JamPress> {
        let mut presses = Vec::new();
        for (place, note) in notes.iter().enumerate() {
            presses.push(press(*note, at + place as u64 * gap, 200));
        }
        presses
    }

    #[test]
    fn a_phrase_played_again_in_the_same_rhythm_is_a_moment() {
        let mut presses = run(&[60, 62, 64, 65], 0, 300);
        presses.extend(run(&[70, 71], 2000, 300));
        presses.extend(run(&[60, 62, 64, 65], 4000, 320));
        let moments = moments_of(&presses, 3);
        assert_eq!(moments[0].name, "Played 2 times");
        assert_eq!(moments[0].presses.len(), 4);
        assert_eq!(moments[0].presses[0].start_ms, 0);
        assert_eq!(moments[0].notes, "C4 D4 E4 F4");
    }

    #[test]
    fn the_same_notes_in_another_rhythm_are_not_a_repeat() {
        let mut presses = run(&[60, 62, 64], 0, 300);
        presses.extend(run(&[60, 62, 64], 3000, 900));
        assert!(moments_of(&presses, 3).is_empty());
    }

    #[test]
    fn a_phrase_ending_on_a_long_hold_is_a_moment() {
        let mut presses = run(&[67, 69], 0, 250);
        presses.push(press(72, 500, 1500));
        let moments = moments_of(&presses, 3);
        assert_eq!(moments.len(), 1);
        assert_eq!(moments[0].name, "Held ending");
        assert_eq!(moments[0].presses.len(), 3);
    }

    #[test]
    fn a_long_hold_after_a_long_gap_stands_alone_and_is_no_moment() {
        let mut presses = run(&[67, 69], 0, 250);
        presses.push(press(72, 9000, 1500));
        assert!(moments_of(&presses, 3).is_empty());
    }

    #[test]
    fn at_most_three_moments_never_sharing_a_press() {
        let mut presses = Vec::new();
        let mut at = 0;
        for phrase in [[60, 62, 64], [65, 67, 69], [71, 72, 74], [48, 50, 52]] {
            for _ in 0..2 {
                presses.extend(run(&phrase, at, 300));
                at += 1500;
            }
        }
        let moments = moments_of(&presses, 3);
        assert_eq!(moments.len(), 3);
        let mut firsts = Vec::new();
        for moment in &moments {
            firsts.push(moment.notes.clone());
        }
        firsts.dedup();
        assert_eq!(firsts.len(), 3);
    }

    #[test]
    fn a_moment_sharing_only_some_presses_with_a_better_one_is_left_out() {
        // The repeat takes C4 D4 E4; the held ending would take B3 too.
        let mut presses = vec![press(59, 0, 200)];
        presses.push(press(60, 250, 200));
        presses.push(press(62, 500, 200));
        presses.push(press(64, 750, 1500));
        presses.extend(run(&[60, 62, 64], 5000, 250));
        let moments = moments_of(&presses, 3);
        let mut used = 0;
        for moment in &moments {
            used += moment.presses.len();
        }
        assert!(used <= presses.len() - 1, "{moments:?}");
    }

    #[test]
    fn nothing_played_gives_nothing() {
        assert!(moments_of(&[], 3).is_empty());
        assert!(moments_of(&run(&[60, 62], 0, 300), 3).is_empty());
    }

    #[test]
    fn a_moment_is_told_to_the_ai_in_one_line() {
        let mut presses = run(&[60, 62, 64], 0, 300);
        presses.extend(run(&[60, 62, 64], 1500, 300));
        let moments = moments_of(&presses, 3);
        let line = moment_line(0, &moments[0]);
        assert_eq!(line, "1. Played 2 times: 3 presses over 0.8 s, C4 D4 E4");
    }

    #[test]
    fn a_moment_plays_back_its_keys_in_time() {
        let moment = moment_of(&run(&[60, 62, 64], 1000, 300), String::from("x"), 1.0);
        let phrase = phrase_of_moment(&moment);
        assert_eq!(phrase.events.len(), 6);
        assert_eq!(phrase.events[0].at_ms, 0);
        assert!(phrase.events[0].down);
        assert_eq!(phrase.length_ms, 800);
    }

    #[test]
    fn a_moment_becomes_notes_in_beats() {
        let moment = moment_of(&run(&[60, 62], 0, 500), String::from("x"), 1.0);
        let notes = notes_in_beats(&moment, 120);
        assert_eq!(notes[1].start, 1.0);
        assert!((notes[1].end - 1.4).abs() < 1e-9);
    }

    #[test]
    fn keeping_a_moment_as_a_loop_or_section() {
        let mut composer = Composer::new();
        let mut engine = Engine::new().unwrap();
        let moment = moment_of(&run(&[60, 62, 64], 0, 500), String::from("x"), 1.0);
        assert!(keep_moment(&mut composer, &mut engine, &moment, "song").is_err());
        let line = keep_moment(&mut composer, &mut engine, &moment, "loop").unwrap();
        assert!(!line.is_empty());
        keep_moment(&mut composer, &mut engine, &moment, "section").unwrap();
        assert_eq!(composer.sections().len(), 1);
    }
}
