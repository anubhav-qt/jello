// Taste drills (docs/design.md#ai-and-learning-rethought-2026-11-03), with
// targets from the player's own songs and presets:
//   - chord: Jello plays a chord of the song's progression; name it.
//   - interval: Jello plays two notes in a row from the song's loop; name
//     the step between them.
//   - sound: match a preset of the player's on the picked track until it
//     measures close, rendered on a second engine and compared.
// The chord and interval drills are answered either way: by playing it
// back on the keys (Jello checks the notes) or by picking one of four
// answers in the rail. Hearing which EQ band moved and matching a
// reference's balance come later.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands;
use crate::compose::{Composer, HeardPress};
use crate::engine::{Engine, GLIDE_KEY_NOTE, VIBRATO_KEY_NOTE};
use crate::harmony::{Chord, Quality, chord_name};
use crate::key_map::LAPTOP_VELOCITY;
use crate::measures::{Measures, measure};
use crate::presets;
use crate::song_file::SongLoop;
use crate::song_session;
use crate::sound_file::Sound;
use crate::sound_profile::SoundProfile;

pub const KINDS: [&str; 3] = ["chord", "interval", "sound"];
// How many answers the rail offers.
const CHOICES: usize = 4;
// Wrong tries before Jello says the answer.
const MOST_WRONG: u32 = 3;
// Presses that start this close, in ms, are one chord.
const TOGETHER_MS: u64 = 80;
// How long Jello holds a chord, and each note of an interval, in ms.
const CHORD_MS: u64 = 1200;
const NOTE_MS: u64 = 600;
// The sound drill: the chord rendered, how long it is held and rings on,
// and how close is close, from 0 to 100.
const TEST_CHORD: [u8; 3] = [48, 52, 55];
const TEST_HELD_MS: u64 = 1000;
const TEST_RING_MS: u64 = 500;
const CLOSE_ENOUGH: u32 = 85;

const INTERVAL_NAMES: [&str; 13] = [
    "unison",
    "minor 2nd",
    "major 2nd",
    "minor 3rd",
    "major 3rd",
    "perfect 4th",
    "tritone",
    "perfect 5th",
    "minor 6th",
    "major 6th",
    "minor 7th",
    "major 7th",
    "octave",
];

// What a drill asks.
#[derive(Clone, Debug, PartialEq)]
enum Question {
    Chord(Chord),
    // The first note and the second.
    Interval(u8, u8),
    // The preset's name and its sound.
    Sound(String, Box<Sound>),
}

// The drill going on: what it asks, the answers offered, the right one,
// the press it listens from, and how it is going.
pub struct Drill {
    kind: String,
    question: Question,
    choices: Vec<String>,
    answer: String,
    first_press: u64,
    wrong: u32,
    state: String,
    said: String,
    closeness: Option<u32>,
}

#[derive(Default)]
pub struct Drills {
    drill: Option<Drill>,
    // For picking, so each drill differs.
    seed: u64,
}

impl Drills {
    // A whole number below `below`, from the drills' own sequence.
    fn pick(&mut self, below: usize) -> usize {
        if self.seed == 0 {
            let mut bytes = [0u8; 8];
            if getrandom::fill(&mut bytes).is_err() {
                bytes = [7, 1, 3, 5, 9, 2, 4, 6];
            }
            self.seed = u64::from_le_bytes(bytes) | 1;
        }
        // xorshift
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % below.max(1) as u64) as usize
    }
}

// What the rail shows of a drill.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DrillView {
    pub kind: String,
    pub prompt: String,
    pub choices: Vec<String>,
    // "asking", "right", "wrong", "told" (the answer after MOST_WRONG
    // tries), or for the sound drill "far" and "close".
    pub state: String,
    pub said: String,
    pub wrong: u32,
    pub closeness: Option<u32>,
}

fn view_of(drill: &Drill) -> DrillView {
    let prompt = match &drill.question {
        Question::Chord(_) => String::from("Which chord is this? Play it back, or pick it."),
        Question::Interval(_, _) => String::from("Which step is this? Play the two notes, or pick it."),
        Question::Sound(name, _) => format!("Match {name} on the picked track, then Measure."),
    };
    DrillView {
        kind: drill.kind.clone(),
        prompt,
        choices: drill.choices.clone(),
        state: drill.state.clone(),
        said: drill.said.clone(),
        wrong: drill.wrong,
        closeness: drill.closeness,
    }
}

// The pitch classes of `notes`.
fn classes_of(notes: &[u8]) -> BTreeSet<u8> {
    let mut classes = BTreeSet::new();
    for note in notes {
        classes.insert(note % 12);
    }
    classes
}

fn interval_name(first: u8, second: u8) -> String {
    let step = (i32::from(second) - i32::from(first)).unsigned_abs() as usize;
    let name = INTERVAL_NAMES[step.min(12)];
    if second > first {
        return format!("{name} up");
    }
    if second < first {
        return format!("{name} down");
    }
    String::from(name)
}

// The song's chords, each once, in the order first played.
fn distinct_chords(chords: &[Chord]) -> Vec<Chord> {
    let mut distinct = Vec::new();
    for chord in chords {
        let known = distinct.iter().any(|seen: &Chord| seen.pitch() == chord.pitch() && seen.quality == chord.quality);
        if !known {
            distinct.push(*chord);
        }
    }
    distinct
}

// Four answers: `answer` and three others from `others`, in an order the
// drills pick.
fn choices_with(drills: &mut Drills, answer: &str, others: &[String]) -> Vec<String> {
    let mut choices = vec![answer.to_string()];
    for other in others {
        if choices.len() == CHOICES {
            break;
        }
        if !choices.contains(other) {
            choices.push(other.clone());
        }
    }
    let place = drills.pick(choices.len());
    choices.swap(0, place);
    choices
}

// Chords near `chord` to offer as wrong answers: the same root's other
// quality, and the chords a fourth and a fifth away.
fn near_chords(chord: Chord) -> Vec<Chord> {
    let mut other = Quality::Major;
    if chord.quality == Quality::Major {
        other = Quality::Minor;
    }
    vec![
        Chord { root: chord.root, quality: other },
        Chord { root: 48 + (chord.root + 5) % 12, quality: chord.quality },
        Chord { root: 48 + (chord.root + 7) % 12, quality: chord.quality },
        Chord { root: 48 + (chord.root + 2) % 12, quality: other },
    ]
}

// A chord drill on `chords`, the song's progression.
fn chord_drill(drills: &mut Drills, chords: &[Chord], name: impl Fn(Chord) -> String) -> Result<(Question, String, Vec<String>), String> {
    let distinct = distinct_chords(chords);
    if distinct.is_empty() {
        return Err(String::from("the song has no chords yet: play a progression first"));
    }
    let chord = distinct[drills.pick(distinct.len())];
    let answer = name(chord);
    let mut others = Vec::new();
    for other in &distinct {
        others.push(name(*other));
    }
    for other in near_chords(chord) {
        others.push(name(other));
    }
    let choices = choices_with(drills, &answer, &others);
    Ok((Question::Chord(chord), answer, choices))
}

// Two notes in a row, a step of 1 to 12 half steps, from the loop's
// layers on instrument tracks.
fn steps_of(looped: &SongLoop, drums: &[bool]) -> Vec<(u8, u8)> {
    let mut steps = Vec::new();
    for layer in &looped.layers {
        if drums.get(layer.track).copied().unwrap_or(false) {
            continue;
        }
        let mut notes = Vec::new();
        for note in &layer.notes {
            if note.note == VIBRATO_KEY_NOTE || note.note == GLIDE_KEY_NOTE {
                continue;
            }
            notes.push(*note);
        }
        notes.sort_by(|a, b| a.start.total_cmp(&b.start));
        for place in 1..notes.len() {
            let first = notes[place - 1];
            let second = notes[place];
            if (second.start - first.start).abs() < 0.01 {
                continue;
            }
            let step = (i32::from(second.note) - i32::from(first.note)).abs();
            if step >= 1 && step <= 12 {
                steps.push((first.note, second.note));
            }
        }
    }
    steps
}

fn interval_drill(drills: &mut Drills, steps: &[(u8, u8)]) -> Result<(Question, String, Vec<String>), String> {
    if steps.is_empty() {
        return Err(String::from("the loop has no two notes in a row to drill: record a melody first"));
    }
    let (first, second) = steps[drills.pick(steps.len())];
    let answer = interval_name(first, second);
    let step = (i32::from(second) - i32::from(first)).abs();
    let mut others = Vec::new();
    for change in [1, -1, 2, -2, 5] {
        let near = step + change;
        if (1..=12).contains(&near) {
            let mut moved = i32::from(first) + near;
            if second < first {
                moved = i32::from(first) - near;
            }
            others.push(interval_name(first, moved.clamp(0, 127) as u8));
        }
    }
    let choices = choices_with(drills, &answer, &others);
    Ok((Question::Interval(first, second), answer, choices))
}

// Presses that started within TOGETHER_MS of each other, as one.
fn grouped(presses: &[HeardPress]) -> Vec<Vec<u8>> {
    let mut groups: Vec<(u64, Vec<u8>)> = Vec::new();
    for press in presses {
        let joins = match groups.last() {
            Some((start, _)) => press.pressed_ms.saturating_sub(*start) <= TOGETHER_MS,
            None => false,
        };
        if joins {
            let last = groups.len() - 1;
            groups[last].1.extend(press.notes.iter().copied());
        } else {
            groups.push((press.pressed_ms, press.notes.clone()));
        }
    }
    let mut notes = Vec::new();
    for (_, group) in groups {
        notes.push(group);
    }
    notes
}

// Whether `presses` play the drill's chord or interval back: Some(true)
// when they do, Some(false) for a try that doesn't, and None while
// nothing was tried.
fn played_back(question: &Question, presses: &[HeardPress]) -> Option<bool> {
    let groups = grouped(presses);
    match question {
        Question::Chord(chord) => {
            let wanted = classes_of(&chord.notes());
            let mut tried = false;
            for group in &groups {
                if group.len() < 2 {
                    continue;
                }
                if classes_of(group) == wanted {
                    return Some(true);
                }
                tried = true;
            }
            if tried {
                return Some(false);
            }
            None
        }
        Question::Interval(first, second) => {
            let wanted = i32::from(*second) - i32::from(*first);
            if groups.len() < 2 {
                return None;
            }
            for place in 1..groups.len() {
                let before = i32::from(groups[place - 1][0]);
                let after = i32::from(groups[place][0]);
                if after - before == wanted {
                    return Some(true);
                }
            }
            Some(false)
        }
        Question::Sound(_, _) => None,
    }
}

// A wrong try: after MOST_WRONG, Jello says the answer.
fn note_wrong(drill: &mut Drill, said: String) {
    drill.wrong += 1;
    if drill.wrong >= MOST_WRONG {
        drill.state = String::from("told");
        drill.said = format!("It was {}.", drill.answer);
    } else {
        drill.state = String::from("wrong");
        drill.said = said;
    }
}

fn is_over(drill: &Drill) -> bool {
    drill.state == "right" || drill.state == "told" || drill.state == "close"
}

// The test chord rendered in `sound` on a second engine.
fn render_sound(sound: &Sound) -> Result<(Vec<f32>, u32), String> {
    let mut engine = Engine::for_render()?;
    let sample_rate = engine.sample_rate();
    let mut profile = SoundProfile::new();
    profile.take_sound(&mut engine, sound);
    engine.settle_volumes();
    engine.trim_lookahead();
    for note in TEST_CHORD {
        engine.note_on(note, LAPTOP_VELOCITY);
    }
    let held = (TEST_HELD_MS * u64::from(sample_rate) / 1000) as usize;
    let mut samples = engine.render(held);
    for note in TEST_CHORD {
        engine.note_off(note);
    }
    let ring = (TEST_RING_MS * u64::from(sample_rate) / 1000) as usize;
    samples.extend(engine.render(ring));
    Ok((samples, sample_rate))
}

// How close two measures are, from 0 to 100, and the biggest difference
// in words, from `yours` toward `target`.
pub fn closeness(yours: &Measures, target: &Measures) -> (u32, String) {
    let mut misses: Vec<(f32, String)> = Vec::new();
    let your_hz = yours.brightness_hz.unwrap_or(1.0).max(1.0);
    let target_hz = target.brightness_hz.unwrap_or(1.0).max(1.0);
    let octaves = (target_hz / your_hz).log2();
    let mut brighter = "brighter";
    if octaves < 0.0 {
        brighter = "darker";
    }
    misses.push((octaves.abs() * 40.0, format!("make it {brighter}")));
    let bands = [
        (target.lows_percent - yours.lows_percent, "lows"),
        (target.mids_percent - yours.mids_percent, "mids"),
        (target.highs_percent - yours.highs_percent, "highs"),
    ];
    for (change, band) in bands {
        let mut more = "more";
        if change < 0.0 {
            more = "less";
        }
        misses.push((change.abs() * 0.8, format!("{more} {band}")));
    }
    let your_db = yours.loudness_db.unwrap_or(-90.0);
    let target_db = target.loudness_db.unwrap_or(-90.0);
    let mut louder = "louder";
    if target_db < your_db {
        louder = "quieter";
    }
    misses.push(((target_db - your_db).abs() * 1.5, format!("make it {louder}")));

    let mut total = 0.0;
    let mut biggest = (0.0, String::new());
    for (amount, words) in misses {
        total += amount;
        if amount > biggest.0 {
            biggest = (amount, words);
        }
    }
    let close = (100.0 - total).clamp(0.0, 100.0).round() as u32;
    (close, biggest.1)
}

// Plays `notes` together for `ms` on the picked track, on a thread.
fn sound_notes(app: &AppHandle, groups: Vec<Vec<u8>>, ms: u64) {
    let app = app.clone();
    thread::spawn(move || {
        for notes in groups {
            {
                let engine = app.state::<Mutex<Engine>>();
                let mut engine = engine.lock().unwrap();
                for note in &notes {
                    engine.note_on(*note, LAPTOP_VELOCITY);
                }
            }
            thread::sleep(Duration::from_millis(ms));
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            for note in &notes {
                engine.note_off(*note);
            }
        }
    });
}

// Starts a drill of `kind`, one of KINDS, and plays its question.
#[tauri::command]
pub fn start_drill(
    app: AppHandle,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    drills: State<'_, Mutex<Drills>>,
    kind: String,
) -> Result<DrillView, String> {
    if !KINDS.contains(&kind.as_str()) {
        return Err(format!("there is no {kind} drill: chord, interval or sound"));
    }
    let mut drills = drills.lock().unwrap();
    let composer_now = composer.lock().unwrap();
    let first_press = composer_now.next_press_id();
    let made = match kind.as_str() {
        "chord" => {
            let mut chords = Vec::new();
            for held in composer_now.held_chords() {
                chords.push(held.chord);
            }
            let key = composer_now.key();
            chord_drill(&mut drills, &chords, |chord| chord_name(chord, key))
        }
        "interval" => {
            let mut drums = Vec::new();
            for track in composer_now.tracks().list() {
                drums.push(track.is_drums());
            }
            let looped = song_session::loop_now(&mut engine.lock().unwrap());
            let steps = match &looped {
                Some(looped) => steps_of(looped, &drums),
                None => Vec::new(),
            };
            interval_drill(&mut drills, &steps)
        }
        _ => {
            let lit = composer_now.lit_preset().map(str::to_string);
            sound_drill(&app, &mut drills, lit.as_deref())
        }
    };
    drop(composer_now);
    let (question, answer, choices) = made?;
    let drill = Drill {
        kind,
        question,
        choices,
        answer,
        first_press,
        wrong: 0,
        state: String::from("asking"),
        said: String::new(),
        closeness: None,
    };
    let view = view_of(&drill);
    drills.drill = Some(drill);
    drop(drills);
    hear(&app)?;
    Ok(view)
}

fn sound_drill(app: &AppHandle, drills: &mut Drills, lit: Option<&str>) -> Result<(Question, String, Vec<String>), String> {
    let folder = commands::sounds_folder(app)?;
    let mut names = Vec::new();
    for preset in presets::list(&folder, None) {
        if preset.readable && Some(preset.name.as_str()) != lit {
            names.push(preset.name);
        }
    }
    if names.is_empty() {
        return Err(String::from("there is no other preset to match: save a sound as a preset first"));
    }
    let name = names[drills.pick(names.len())].clone();
    let sound = presets::read(&folder, &name, &SoundProfile::new())?;
    Ok((Question::Sound(name.clone(), Box::new(sound)), name, Vec::new()))
}

// Plays the drill's question again. For the sound drill, the target plays
// on the picked track for a moment, then its own sound comes back.
#[tauri::command]
pub fn hear_drill(app: AppHandle) -> Result<(), String> {
    hear(&app)
}

fn hear(app: &AppHandle) -> Result<(), String> {
    let question = {
        let drills = app.state::<Mutex<Drills>>();
        let drills = drills.lock().unwrap();
        let Some(drill) = &drills.drill else {
            return Err(String::from("no drill is going on"));
        };
        drill.question.clone()
    };
    app.state::<Mutex<Engine>>().lock().unwrap().start_audio()?;
    match question {
        Question::Chord(chord) => sound_notes(app, vec![chord.notes().to_vec()], CHORD_MS),
        Question::Interval(first, second) => sound_notes(app, vec![vec![first], vec![second]], NOTE_MS),
        Question::Sound(_, sound) => hear_target(app, *sound),
    }
    Ok(())
}

// The target sound on the picked track for the test chord, then the
// track's own sound again. The song's sound is never changed.
fn hear_target(app: &AppHandle, sound: Sound) {
    let app = app.clone();
    thread::spawn(move || {
        {
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            let mut target = SoundProfile::new();
            target.take_sound(&mut engine, &sound);
            for note in TEST_CHORD {
                engine.note_on(note, LAPTOP_VELOCITY);
            }
        }
        thread::sleep(Duration::from_millis(TEST_HELD_MS));
        {
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            for note in TEST_CHORD {
                engine.note_off(note);
            }
        }
        thread::sleep(Duration::from_millis(TEST_RING_MS));
        let composer = app.state::<Mutex<Composer>>();
        let composer = composer.lock().unwrap();
        let engine = app.state::<Mutex<Engine>>();
        let mut engine = engine.lock().unwrap();
        composer.sound_profile().send_all(&mut engine);
    });
}

// The player picks `choice` in the rail.
#[tauri::command]
pub fn answer_drill(drills: State<'_, Mutex<Drills>>, choice: String) -> Result<DrillView, String> {
    let mut drills = drills.lock().unwrap();
    let Some(drill) = drills.drill.as_mut() else {
        return Err(String::from("no drill is going on"));
    };
    if is_over(drill) {
        return Ok(view_of(drill));
    }
    if !drill.choices.contains(&choice) {
        return Err(format!("{choice} isn't one of the answers"));
    }
    if choice == drill.answer {
        drill.state = String::from("right");
        drill.said = format!("Right: {}.", drill.answer);
    } else {
        note_wrong(drill, format!("Not {choice}. Hear it again?"));
    }
    Ok(view_of(drill))
}

// Checks what the player played since the drill began, or for the sound
// drill measures the picked track against the target.
#[tauri::command]
pub fn check_drill(composer: State<'_, Mutex<Composer>>, drills: State<'_, Mutex<Drills>>) -> Result<DrillView, String> {
    let mut drills = drills.lock().unwrap();
    let Some(drill) = drills.drill.as_mut() else {
        return Err(String::from("no drill is going on"));
    };
    if is_over(drill) {
        return Ok(view_of(drill));
    }
    if let Question::Sound(_, target) = &drill.question {
        let yours = composer.lock().unwrap().sound_profile().sound();
        let (your_samples, rate) = render_sound(&yours)?;
        let (target_samples, _) = render_sound(target)?;
        let (close, hint) = closeness(&measure(&your_samples, rate), &measure(&target_samples, rate));
        drill.closeness = Some(close);
        if close >= CLOSE_ENOUGH {
            drill.state = String::from("close");
            drill.said = format!("{close}% close: that's {}.", drill.answer);
        } else {
            drill.state = String::from("far");
            drill.said = format!("{close}% close: {hint}.");
        }
        return Ok(view_of(drill));
    }
    let presses = composer.lock().unwrap().presses_from(drill.first_press);
    let played = played_back(&drill.question, &presses);
    if played == Some(true) {
        drill.state = String::from("right");
        drill.said = format!("Right: {}.", drill.answer);
    }
    if played == Some(false) {
        let next = composer.lock().unwrap().next_press_id();
        drill.first_press = next;
        note_wrong(drill, String::from("Not quite. Hear it again and play it back."));
    }
    Ok(view_of(drill))
}

// The drill ends.
#[tauri::command]
pub fn end_drill(drills: State<'_, Mutex<Drills>>) {
    drills.lock().unwrap().drill = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_file::{BeatNote, SongLayer};

    fn major(root: u8) -> Chord {
        Chord { root, quality: Quality::Major }
    }

    fn named(chord: Chord) -> String {
        chord_name(chord, None)
    }

    fn press(id: u64, notes: &[u8], pressed_ms: u64) -> HeardPress {
        HeardPress { id, notes: notes.to_vec(), pressed_ms, held_ms: Some(200) }
    }

    #[test]
    fn a_chord_drill_asks_a_chord_of_the_song_with_four_answers() {
        let mut drills = Drills::default();
        let chords = [major(48), major(55), major(48)];
        let (question, answer, choices) = chord_drill(&mut drills, &chords, named).unwrap();
        let Question::Chord(chord) = question else {
            panic!("a chord drill asks a chord");
        };
        assert!(chord == major(48) || chord == major(55));
        assert_eq!(answer, named(chord));
        assert_eq!(choices.len(), 4);
        assert!(choices.contains(&answer));
        let distinct: BTreeSet<&String> = choices.iter().collect();
        assert_eq!(distinct.len(), 4);
        assert!(chord_drill(&mut drills, &[], named).unwrap_err().contains("no chords"));
    }

    #[test]
    fn a_chord_is_played_back_in_any_voicing_and_notes_close_together_count() {
        let question = Question::Chord(major(48));
        assert_eq!(played_back(&question, &[press(1, &[60, 64, 67], 0)]), Some(true));
        let rolled = [press(1, &[48], 0), press(2, &[52], 30), press(3, &[55], 60)];
        assert_eq!(played_back(&question, &rolled), Some(true));
        assert_eq!(played_back(&question, &[press(1, &[48, 51, 55], 0)]), Some(false));
        assert_eq!(played_back(&question, &[press(1, &[48], 0)]), None);
        assert_eq!(played_back(&question, &[]), None);
    }

    #[test]
    fn an_interval_is_its_step_and_its_way() {
        assert_eq!(interval_name(60, 67), "perfect 5th up");
        assert_eq!(interval_name(67, 64), "minor 3rd down");
        assert_eq!(interval_name(60, 72), "octave up");
        let question = Question::Interval(60, 64);
        assert_eq!(played_back(&question, &[press(1, &[62], 0), press(2, &[66], 500)]), Some(true));
        assert_eq!(played_back(&question, &[press(1, &[62], 0), press(2, &[65], 500)]), Some(false));
        assert_eq!(played_back(&question, &[press(1, &[62], 0)]), None);
    }

    #[test]
    fn interval_steps_come_from_the_loop_and_leave_out_drums() {
        let melody = SongLayer {
            track: 0,
            bars: 0,
            volume: 100,
            playing: true,
            notes: vec![
                BeatNote { note: 60, start: 0.0, end: 1.0 },
                BeatNote { note: 67, start: 1.0, end: 1.5 },
                BeatNote { note: 67, start: 1.5, end: 2.0 },
                BeatNote { note: 90, start: 2.0, end: 3.0 },
            ],
        };
        let mut drums = melody.clone();
        drums.track = 1;
        let looped = SongLoop { bars: 1, layers: vec![melody, drums] };
        assert_eq!(steps_of(&looped, &[false, true]), vec![(60, 67)]);
        let mut drills = Drills::default();
        let (_, answer, choices) = interval_drill(&mut drills, &[(60, 67)]).unwrap();
        assert_eq!(answer, "perfect 5th up");
        assert_eq!(choices.len(), 4);
        assert!(interval_drill(&mut drills, &[]).is_err());
    }

    #[test]
    fn three_wrong_tries_tell_the_answer() {
        let mut drill = Drill {
            kind: String::from("chord"),
            question: Question::Chord(major(48)),
            choices: vec![String::from("C"), String::from("G")],
            answer: String::from("C"),
            first_press: 0,
            wrong: 0,
            state: String::from("asking"),
            said: String::new(),
            closeness: None,
        };
        note_wrong(&mut drill, String::from("Not G."));
        assert_eq!(drill.state, "wrong");
        note_wrong(&mut drill, String::from("Not G."));
        note_wrong(&mut drill, String::from("Not G."));
        assert_eq!(drill.state, "told");
        assert_eq!(drill.said, "It was C.");
        assert!(is_over(&drill));
    }

    #[test]
    fn the_same_sound_is_all_the_way_close_and_a_darker_one_says_so() {
        let sound = SoundProfile::new().sound();
        let (samples, rate) = render_sound(&sound).unwrap();
        let measured = measure(&samples, rate);
        let (close, _) = closeness(&measured, &measured);
        assert_eq!(close, 100);

        let mut darker = measure(&samples, rate);
        darker.brightness_hz = Some(measured.brightness_hz.unwrap() / 4.0);
        let (close, hint) = closeness(&darker, &measured);
        assert!(close < CLOSE_ENOUGH);
        assert_eq!(hint, "make it brighter");
    }

    #[test]
    fn the_drills_pick_within_range() {
        let mut drills = Drills::default();
        for _ in 0..100 {
            assert!(drills.pick(3) < 3);
        }
        assert_eq!(drills.pick(1), 0);
    }
}
