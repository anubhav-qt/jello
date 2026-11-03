// Practicing your own songs (docs/design.md#ai-and-learning-rethought-2026-11-03).
// Any section becomes a practice piece: the player picks a track and plays
// its presses as recorded, scored per hand (the left and right halves of
// the keyboard), while Jello plays the other tracks in their own sounds.
// Practice climbs a tempo ladder up to the section's tempo; a bar with a
// miss becomes the loop until it is clean twice, then the whole section
// plays again. What each bar needs comes from song_skills.rs.

use std::sync::Mutex;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::arrange;
use crate::compose::Composer;
use crate::engine::{Engine, FASTEST_TEMPO, GLIDE_KEY_NOTE, SLOWEST_TEMPO, VIBRATO_KEY_NOTE};
use crate::key_map::KeyMap;
use crate::practice::{self, BEATS_PER_BAR, Part, Piece, PieceNote, Played, Practice, Section, SongPiece};
use crate::scoring::Hand;
use crate::song_file::{SongLoop, SongSection, note_name};
use crate::song_session::{self, OpenSong};
use crate::song_skills::{self, SongPress};

// The keys a touch typist plays with the left hand. Every other key is the
// right hand's.
const LEFT_KEYS: [&str; 28] = [
    "Backquote",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Tab",
    "KeyQ",
    "KeyW",
    "KeyE",
    "KeyR",
    "KeyT",
    "CapsLock",
    "KeyA",
    "KeyS",
    "KeyD",
    "KeyF",
    "KeyG",
    "IntlBackslash",
    "KeyZ",
    "KeyX",
    "KeyC",
    "KeyV",
    "KeyB",
    "ShiftLeft",
    "ControlLeft",
    "AltLeft",
    "MetaLeft",
];

// The ladder starts at this share of the section's tempo, in percent, and
// climbs to it in this many steps, each after this many clean tries.
const LADDER_START_PERCENT: u32 = 70;
const LADDER_STEPS: u32 = 3;
const CLEAN_TO_CLIMB: u32 = 2;
// Clean tries of a hard bar before the whole section plays again.
const CLEAN_TO_LEAVE_BAR: u32 = 2;
// Notes that start this close, in beats, with the same end, were one press.
const SAME_PRESS: f64 = 0.000_1;

// The hand that plays `key`.
pub fn hand_of_key(key: &str) -> Hand {
    if LEFT_KEYS.contains(&key) {
        return Hand::Left;
    }
    Hand::Right
}

// A note of a layer, in beats from the section's start.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placed {
    pitch: u8,
    start: f64,
    end: f64,
}

// The notes of layer `layer` of track `track` as `section` plays them: the
// layer repeats on its own length from the section's start, and notes still
// sounding at its end let go there.
fn section_notes(section: &SongSection, looped: &SongLoop, track: usize, layer: usize) -> Vec<Placed> {
    let Some(found) = arrange::layer_of(looped, track, layer) else {
        return Vec::new();
    };
    let period = arrange::period_beats(found, looped);
    let length = f64::from(section.bars) * BEATS_PER_BAR;
    let mut placed = Vec::new();
    if period <= 0.0 {
        return placed;
    }
    let mut round_start = 0.0;
    while round_start < length {
        for note in &found.notes {
            if note.note == VIBRATO_KEY_NOTE || note.note == GLIDE_KEY_NOTE {
                continue;
            }
            let start = round_start + note.start;
            if start >= length {
                continue;
            }
            let end = (round_start + note.end).min(length);
            if end <= start {
                continue;
            }
            placed.push(Placed { pitch: note.note, start, end });
        }
        round_start += period;
    }
    placed.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    placed
}

// Notes that start and end together were one press: its notes, lowest
// first, when it started and how long it was held.
fn presses_of(notes: &[Placed]) -> Vec<(Vec<u8>, f64, f64)> {
    let mut presses: Vec<(Vec<u8>, f64, f64)> = Vec::new();
    for note in notes {
        let mut joined = false;
        for press in presses.iter_mut() {
            let same_start = (press.1 - note.start).abs() < SAME_PRESS;
            let same_end = (press.1 + press.2 - note.end).abs() < SAME_PRESS;
            if same_start && same_end && !press.0.contains(&note.pitch) {
                press.0.push(note.pitch);
                press.0.sort();
                joined = true;
                break;
            }
        }
        if !joined {
            presses.push((vec![note.pitch], note.start, note.end - note.start));
        }
    }
    presses.sort_by(|a, b| a.1.total_cmp(&b.1));
    presses
}

// The key that plays exactly `pitches` now, if one does.
fn key_playing(key_map: &KeyMap, pitches: &[u8]) -> Option<String> {
    practice::key_for(key_map, pitches)
}

fn notes_label(pitches: &[u8]) -> String {
    let mut names = Vec::new();
    for pitch in pitches {
        names.push(note_name(*pitch));
    }
    names.join(" ")
}

// What a song piece is made from.
pub struct SongSource<'a> {
    pub song: &'a str,
    pub section: &'a SongSection,
    pub looped: &'a SongLoop,
    pub track: usize,
    // Every track's name and whether it is a drums track, by number from 0.
    pub track_names: Vec<String>,
    pub drums: Vec<bool>,
    pub key_map: &'a KeyMap,
}

// The practice piece for `source`: the track's presses split by hand, one
// piece section per bar, and the other tracks' layers as Jello's parts,
// each on its own layer slot. Refused for a drums track, a track that
// plays nothing in the section, or presses no key plays now.
pub fn piece_of_section(source: &SongSource) -> Result<Piece, String> {
    let section = source.section;
    let Some(track_name) = source.track_names.get(source.track) else {
        return Err(format!("there is no track {}", source.track + 1));
    };
    if source.drums[source.track] {
        return Err(String::from("a drums track can't be practiced yet: pick an instrument track"));
    }

    let mut own = Vec::new();
    for used in &section.layers {
        if used.track == source.track {
            own.extend(section_notes(section, source.looped, used.track, used.layer));
        }
    }
    own.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    let mut presses = Vec::new();
    let mut unplayable = 0;
    for (pitches, start, hold) in presses_of(&own) {
        let Some(key) = key_playing(source.key_map, &pitches) else {
            unplayable += 1;
            continue;
        };
        let hand = hand_of_key(&key);
        presses.push(SongPress { pitches, key, hand, start, hold });
    }
    if presses.is_empty() && unplayable > 0 {
        return Err(format!(
            "no key plays {track_name}'s notes in {} now: the song's keys changed since",
            section.name
        ));
    }
    if presses.is_empty() {
        return Err(format!("{track_name} plays nothing in {}", section.name));
    }

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut keys = Vec::new();
    for (index, press) in presses.iter().enumerate() {
        let mut played = Played::Melody;
        if press.pitches.len() > 1 {
            played = Played::Chord;
        }
        let note = PieceNote {
            index,
            label: notes_label(&press.pitches),
            pitches: press.pitches.clone(),
            played,
            start: press.start,
            hold: press.hold,
            octave: 0,
            moved: None,
        };
        keys.push(press.key.clone());
        if press.hand == Hand::Left {
            left.push(note);
        } else {
            right.push(note);
        }
    }

    let mut parts = vec![
        Part { name: String::from("Left hand"), hand: Hand::Left, notes: left, slot: None },
        Part { name: String::from("Right hand"), hand: Hand::Right, notes: right, slot: None },
    ];
    let mut index = presses.len();
    for used in &section.layers {
        if used.track == source.track {
            continue;
        }
        let mut notes = Vec::new();
        for placed in section_notes(section, source.looped, used.track, used.layer) {
            notes.push(PieceNote {
                index,
                label: note_name(placed.pitch),
                pitches: vec![placed.pitch],
                played: Played::Melody,
                start: placed.start,
                hold: placed.end - placed.start,
                octave: 0,
                moved: None,
            });
            index += 1;
        }
        if notes.is_empty() {
            continue;
        }
        let name = source.track_names.get(used.track).cloned().unwrap_or_default();
        parts.push(Part { name, hand: Hand::Jello, notes, slot: Some((used.track, used.layer)) });
    }

    let mut sections = Vec::new();
    let mut bar_skills = Vec::new();
    for bar in 1..=section.bars {
        sections.push(Section { name: bar_name(bar), from_bar: bar, to_bar: bar });
        bar_skills.push(song_skills::skills_of_bar(&presses, bar));
    }
    Ok(Piece {
        name: format!("{} · {track_name}", section.name),
        area: None,
        octave: 0,
        bars: section.bars,
        parts,
        sections,
        song: Some(SongPiece {
            song: source.song.to_string(),
            section: section.name.clone(),
            track: track_name.clone(),
            keys,
            bar_skills,
        }),
    })
}

pub fn bar_name(bar: u32) -> String {
    format!("Bar {bar}")
}

// The settings a song piece starts with: a ladder from LADDER_START_PERCENT
// of `tempo` up to it, playing again until it is done.
pub fn starting_settings(tempo: u32) -> Value {
    let start = (tempo * LADDER_START_PERCENT / 100).clamp(SLOWEST_TEMPO, FASTEST_TEMPO);
    let step = ((tempo.saturating_sub(start)) / LADDER_STEPS).clamp(1, 20);
    json!({
        "tempo": start,
        "ladder": {"step": step, "clean": CLEAN_TO_CLIMB, "top": tempo.clamp(SLOWEST_TEMPO, FASTEST_TEMPO)},
        "loop": true,
        "count_in": 1,
        "hands": "both",
        "mode": "along",
        "coach": false,
    })
}

// After a try of a song piece. On a hard bar, two clean tries in a row
// play the whole section again. On the whole section, the ladder climbs,
// and a try that wasn't clean makes its hardest bar the loop. Returns
// whether practice is done: the ladder at its top, clean enough there.
pub fn after_try(practice: &mut Practice, clean: bool, summary: &Value) -> bool {
    if practice.settings.section.is_some() {
        if clean {
            practice.clean_in_row += 1;
        } else {
            practice.clean_in_row = 0;
        }
        if practice.clean_in_row >= CLEAN_TO_LEAVE_BAR {
            practice.settings.section = None;
            practice.clean_in_row = 0;
        }
        return false;
    }
    let done = practice::climb(practice, clean);
    if clean || done {
        return done;
    }
    if let Some(bar) = hardest_bar(summary) {
        practice.settings.section = Some(bar);
        practice.clean_in_row = 0;
    }
    false
}

// The bar with the most misses in a try, then the furthest off on
// average; None if every bar was clean.
pub fn hardest_bar(summary: &Value) -> Option<String> {
    let mut hardest: Option<(String, f64, f64)> = None;
    let Some(sections) = summary["sections"].as_array() else {
        return None;
    };
    for section in sections {
        let Some(name) = section["name"].as_str() else {
            continue;
        };
        let tally = &section["tally"];
        let missed = tally["missed"].as_f64().unwrap_or(0.0);
        let off = tally["mean_offset_ms"].as_f64().unwrap_or(0.0).abs();
        let notes = tally["notes"].as_f64().unwrap_or(0.0);
        let hits = tally["hits"].as_f64().unwrap_or(0.0);
        if missed == 0.0 && hits >= notes {
            continue;
        }
        let mut harder = true;
        if let Some((_, most_missed, most_off)) = &hardest {
            if missed == *most_missed {
                harder = off > *most_off;
            } else {
                harder = missed > *most_missed;
            }
        }
        if harder {
            hardest = Some((name.to_string(), missed, off));
        }
    }
    let (name, _, _) = hardest?;
    Some(name)
}

// The try's line for the skill record: the song, the section, and each bar
// played with what it needs.
pub fn song_line(song: &SongPiece, summary: &Value) -> Value {
    let mut bars = Vec::new();
    for (place, skills) in song.bar_skills.iter().enumerate() {
        let bar = place as u32 + 1;
        let name = bar_name(bar);
        let mut played = false;
        if let Some(sections) = summary["sections"].as_array() {
            for section in sections {
                if section["name"] == json!(name) {
                    played = true;
                }
            }
        }
        if played {
            bars.push(json!({"bar": bar, "skills": skills}));
        }
    }
    json!({"song": song.song, "section": song.section, "track": song.track, "bars": bars})
}

// Practice a section of the open song: its place among the sections, from
// 0, and the track, from 0. Sets the piece and tells the screen.
#[tauri::command]
pub fn practice_section(
    app: AppHandle,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    key_map: State<'_, Mutex<KeyMap>>,
    practice: State<'_, Mutex<Practice>>,
    section: usize,
    track: usize,
) -> Result<Value, String> {
    let song = app.state::<Mutex<OpenSong>>().lock().unwrap().name().to_string();
    let open = OpenParts {
        song: &song,
        composer: &composer,
        engine: &engine,
        key_map: &key_map,
        practice: &practice,
    };
    let view = set_section(&open, section, track)?;
    let _ = app.emit("practice", &view);
    Ok(view)
}

// What setting a section's piece reaches, for the button and the tool.
pub struct OpenParts<'a> {
    pub song: &'a str,
    pub composer: &'a Mutex<Composer>,
    pub engine: &'a Mutex<Engine>,
    pub key_map: &'a Mutex<KeyMap>,
    pub practice: &'a Mutex<Practice>,
}

// Makes section `section`'s track `track` (both from 0) the piece
// practiced, and returns the practice view.
pub fn set_section(open: &OpenParts, section: usize, track: usize) -> Result<Value, String> {
    let composer = open.composer.lock().unwrap();
    let Some(found) = composer.sections().get(section) else {
        return Err(format!("there is no section {}", section + 1));
    };
    let found = found.clone();
    let looped = song_session::loop_now(&mut open.engine.lock().unwrap());
    let Some(looped) = looped else {
        return Err(String::from("there is no loop: sections play its layers"));
    };
    let mut track_names = Vec::new();
    let mut drums = Vec::new();
    for (place, each) in composer.tracks().list().iter().enumerate() {
        track_names.push(each.name(place + 1));
        drums.push(each.is_drums());
    }
    drop(composer);
    let key_map = open.key_map.lock().unwrap();
    let source = SongSource {
        song: open.song,
        section: &found,
        looped: &looped,
        track,
        track_names,
        drums,
        key_map: &key_map,
    };
    let piece = piece_of_section(&source)?;
    drop(key_map);
    let mut practice = open.practice.lock().unwrap();
    practice::set_piece(&mut practice, piece, &starting_settings(found.tempo), found.tempo)?;
    Ok(practice.view())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_file::{BeatNote, SectionLayer, SongLayer};

    fn layer(track: usize, bars: u32, notes: &[(u8, f64, f64)]) -> SongLayer {
        let mut beat_notes = Vec::new();
        for (note, start, end) in notes {
            beat_notes.push(BeatNote { note: *note, start: *start, end: *end });
        }
        SongLayer { track, bars, volume: 100, playing: true, notes: beat_notes }
    }

    fn section(bars: u32, layers: &[(usize, usize)]) -> SongSection {
        let mut used = Vec::new();
        for (track, layer) in layers {
            used.push(SectionLayer { track: *track, layer: *layer });
        }
        SongSection {
            name: String::from("Verse"),
            bars,
            tempo: 100,
            key_shift: 0,
            layers: used,
            automation: Vec::new(),
            keys: None,
        }
    }

    fn source<'a>(section: &'a SongSection, looped: &'a SongLoop, key_map: &'a KeyMap, track: usize) -> SongSource<'a> {
        SongSource {
            song: "Song",
            section,
            looped,
            track,
            track_names: vec![String::from("Piano"), String::from("Bass"), String::from("Drums")],
            drums: vec![false, false, true],
            key_map,
        }
    }

    #[test]
    fn the_keyboard_s_halves_are_the_hands() {
        assert_eq!(hand_of_key("KeyC"), Hand::Left);
        assert_eq!(hand_of_key("Digit5"), Hand::Left);
        assert_eq!(hand_of_key("KeyH"), Hand::Right);
        assert_eq!(hand_of_key("Digit6"), Hand::Right);
        assert_eq!(hand_of_key("BracketRight"), Hand::Right);
    }

    #[test]
    fn a_track_s_presses_split_by_hand_and_the_other_tracks_are_jello_s() {
        // C major on the C key (left), and A3 on H (right) twice; a bass
        // layer on track 2.
        let looped = SongLoop {
            bars: 1,
            layers: vec![
                layer(0, 0, &[(48, 0.0, 2.0), (52, 0.0, 2.0), (55, 0.0, 2.0), (57, 1.0, 1.5), (57, 2.0, 2.5)]),
                layer(1, 0, &[(36, 0.0, 4.0)]),
            ],
        };
        let verse = section(2, &[(0, 0), (1, 0)]);
        let key_map = KeyMap::with_test_layout();
        let piece = piece_of_section(&source(&verse, &looped, &key_map, 0)).unwrap();
        assert_eq!(piece.name, "Verse · Piano");
        assert_eq!(piece.bars, 2);
        // The layer repeats in bar 2.
        assert_eq!(piece.parts[0].hand, Hand::Left);
        assert_eq!(piece.parts[0].notes.len(), 2);
        assert_eq!(piece.parts[0].notes[0].pitches, vec![48, 52, 55]);
        assert_eq!(piece.parts[0].notes[0].played, Played::Chord);
        assert_eq!(piece.parts[1].hand, Hand::Right);
        assert_eq!(piece.parts[1].notes.len(), 4);
        assert_eq!(piece.parts[1].notes[0].start, 1.0);
        assert_eq!(piece.parts[1].notes[0].hold, 0.5);
        assert_eq!(piece.parts[2].hand, Hand::Jello);
        assert_eq!(piece.parts[2].name, "Bass");
        assert_eq!(piece.parts[2].slot, Some((1, 0)));
        assert_eq!(piece.parts[2].notes.len(), 2);
        let song = piece.song.unwrap();
        assert_eq!(song.keys[0], "KeyC");
        assert_eq!(song.keys[1], "KeyH");
        assert_eq!(song.bar_skills.len(), 2);
        assert_eq!(piece.sections[1].name, "Bar 2");
    }

    #[test]
    fn a_layer_of_its_own_length_repeats_and_ends_with_the_section() {
        let looped = SongLoop { bars: 1, layers: vec![layer(0, 1, &[(57, 3.0, 5.0)])] };
        let verse = section(2, &[(0, 0)]);
        let notes = section_notes(&verse, &looped, 0, 0);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].end, 5.0);
        assert_eq!(notes[1].start, 7.0);
        assert_eq!(notes[1].end, 8.0);
    }

    #[test]
    fn drums_nothing_to_play_and_keys_that_changed_are_refused() {
        let looped = SongLoop { bars: 1, layers: vec![layer(0, 0, &[(20, 0.0, 1.0)]), layer(2, 0, &[(36, 0.0, 1.0)])] };
        let verse = section(1, &[(0, 0), (2, 0)]);
        let key_map = KeyMap::with_test_layout();
        let drums = piece_of_section(&source(&verse, &looped, &key_map, 2)).unwrap_err();
        assert!(drums.contains("drums"));
        let nothing = piece_of_section(&source(&verse, &looped, &key_map, 1)).unwrap_err();
        assert_eq!(nothing, "Bass plays nothing in Verse");
        let changed = piece_of_section(&source(&verse, &looped, &key_map, 0)).unwrap_err();
        assert!(changed.contains("keys changed"));
        let missing = piece_of_section(&source(&verse, &looped, &key_map, 5)).unwrap_err();
        assert_eq!(missing, "there is no track 6");
    }

    fn summary_with(bars: &[(u32, f64, f64, f64)]) -> Value {
        let mut sections = Vec::new();
        for (bar, notes, missed, off) in bars {
            sections.push(json!({
                "name": bar_name(*bar),
                "tally": {"notes": notes, "hits": notes - missed, "missed": missed, "mean_offset_ms": off},
            }));
        }
        json!({"sections": sections})
    }

    #[test]
    fn the_hardest_bar_has_the_most_misses_then_the_furthest_off() {
        let summary = summary_with(&[(1, 4.0, 0.0, 10.0), (2, 4.0, 1.0, 5.0), (3, 4.0, 1.0, -30.0)]);
        assert_eq!(hardest_bar(&summary), Some(String::from("Bar 3")));
        let clean = summary_with(&[(1, 4.0, 0.0, 10.0)]);
        assert_eq!(hardest_bar(&clean), None);
        // No miss counted, but not every note hit, is still not clean.
        let short = json!({"sections": [{"name": "Bar 4", "tally": {"notes": 4.0, "hits": 3.0, "missed": 0.0}}]});
        assert_eq!(hardest_bar(&short), Some(String::from("Bar 4")));
    }

    #[test]
    fn a_messy_try_between_clean_ones_starts_the_count_again() {
        let mut practice = Practice::default();
        practice.settings.section = Some(String::from("Bar 2"));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert!(!after_try(&mut practice, false, &json!({})));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert_eq!(practice.settings.section, Some(String::from("Bar 2")));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert_eq!(practice.settings.section, None);
    }

    #[test]
    fn a_hard_bar_loops_until_clean_twice_then_the_section_plays_again() {
        let mut practice = Practice::default();
        practice.settings.ladder = Some(practice::Ladder { step: 10, clean: 2, top: 100 });
        practice.tempo_now = 80;
        let missed = summary_with(&[(1, 4.0, 0.0, 0.0), (2, 4.0, 2.0, 0.0)]);
        assert!(!after_try(&mut practice, false, &missed));
        assert_eq!(practice.settings.section, Some(String::from("Bar 2")));
        // A miss in between starts the count of clean tries again.
        assert!(!after_try(&mut practice, true, &json!({})));
        assert!(!after_try(&mut practice, false, &missed));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert_eq!(practice.settings.section, Some(String::from("Bar 2")));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert_eq!(practice.settings.section, None);
        assert_eq!(practice.tempo_now, 80);
        // Then the ladder climbs on clean whole tries, and ends at its top.
        assert!(!after_try(&mut practice, true, &json!({})));
        assert!(!after_try(&mut practice, true, &json!({})));
        assert_eq!(practice.tempo_now, 90);
        practice.tempo_now = 100;
        assert!(!after_try(&mut practice, true, &json!({})));
        assert!(after_try(&mut practice, true, &json!({})));
    }

    #[test]
    fn the_ladder_starts_lower_and_climbs_to_the_section_s_tempo() {
        let settings = starting_settings(120);
        assert_eq!(settings["tempo"], json!(84));
        assert_eq!(settings["ladder"]["top"], json!(120));
        assert_eq!(settings["ladder"]["step"], json!(12));
    }

    #[test]
    fn a_try_s_line_names_the_bars_played_and_what_they_need() {
        let song = SongPiece {
            song: String::from("Song"),
            section: String::from("Verse"),
            track: String::from("Piano"),
            keys: Vec::new(),
            bar_skills: vec![vec![String::from("run of eighths")], Vec::new()],
        };
        let summary = summary_with(&[(1, 4.0, 0.0, 0.0)]);
        let line = song_line(&song, &summary);
        assert_eq!(line["bars"], json!([{"bar": 1, "skills": ["run of eighths"]}]));
        assert_eq!(line["song"], json!("Song"));
    }
}
