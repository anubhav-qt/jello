// A song's key mapping (docs/adr/0047-song-key-mappings.md): which notes each
// playing key plays. A key plays one note or any set of notes. Nothing stays
// fixed between songs, since the screen shows which key to press.
//
// A scale's default keeps chords on the left hand and the melody on the right,
// with the most-played under resting fingers. Keys left over repeat the best
// notes in other octaves, and the best chords in other voicings.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::guitar::GuitarMapping;
use crate::harmony::Quality;
use crate::scales::Scale;

// The most notes one key plays.
pub const MOST_NOTES_ON_A_KEY: usize = 12;

// The left hand's chord keys, each role's key first: the home row, then the
// upper row, then the bottom row (docs/design.md#song-key-mappings).
const CHORD_KEYS_BY_ROLE: [&str; 14] = [
    "KeyF", "KeyS", "KeyD", "KeyA", "KeyG", "KeyT", "KeyR", "KeyQ", "KeyW", "KeyE", "KeyV", "KeyZ", "KeyX", "KeyC",
];

// The left hand's other keys. They hold other voicings of the best chords.
const SPARE_CHORD_KEYS: [&str; 9] = [
    "KeyB",
    "ControlLeft",
    "Tab",
    "Backquote",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
];

// The right hand's rows.
const NUMBER_ROW: [&str; 5] = ["Digit6", "Digit7", "Digit8", "Digit9", "Digit0"];
const UPPER_ROW: [&str; 5] = ["KeyY", "KeyU", "KeyI", "KeyO", "KeyP"];
const UPPER_END: [&str; 3] = ["BracketLeft", "BracketRight", "Backslash"];
const HOME_ROW: [&str; 6] = ["KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote"];
const BOTTOM_ROW: [&str; 5] = ["KeyN", "KeyM", "Comma", "Period", "Slash"];

// Every key a mapping can use: all the left hand's and all the right hand's.
pub fn mappable_keys() -> Vec<&'static str> {
    let mut keys = Vec::new();
    keys.extend(CHORD_KEYS_BY_ROLE);
    keys.extend(SPARE_CHORD_KEYS);
    keys.extend(NUMBER_ROW);
    keys.extend(UPPER_ROW);
    keys.extend(UPPER_END);
    keys.extend(HOME_ROW);
    keys.extend(BOTTOM_ROW);
    keys
}

pub fn is_mappable(key: &str) -> bool {
    mappable_keys().contains(&key)
}

// Which notes each key plays, and the keys an optimizer must leave alone. A
// key that isn't in `keys` plays nothing.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct KeyMapping {
    pub keys: BTreeMap<String, Vec<u8>>,
    pub locked: BTreeSet<String>,
    // What the same keys play on a guitar's track (src/guitar.rs).
    pub guitar: GuitarMapping,
}

impl KeyMapping {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    // `scale`'s default mapping.
    pub fn default_for(scale: Scale) -> KeyMapping {
        let mut mapping = KeyMapping::default();
        mapping.place_chords(scale);
        mapping.place_melody(scale);
        mapping.guitar = GuitarMapping::default_for(scale);
        mapping
    }

    fn place_chords(&mut self, scale: Scale) {
        let chords = scale.kind.chords();
        let mut left_over: Vec<&str> = Vec::new();
        for (place, key) in CHORD_KEYS_BY_ROLE.iter().enumerate() {
            match chords.get(place) {
                Some(chord) => {
                    self.keys.insert(String::from(*key), triad_notes(scale, *chord, 0));
                }
                None => left_over.push(key),
            }
        }
        left_over.extend(SPARE_CHORD_KEYS);

        // Inversions of each chord in turn, best first. An inversion moves
        // the lowest note up an octave.
        let mut voicings = Vec::new();
        for inversion in 1..=4 {
            for chord in &chords {
                voicings.push(triad_notes(scale, *chord, inversion));
            }
        }
        for (key, notes) in left_over.into_iter().zip(voicings) {
            self.keys.insert(String::from(key), notes);
        }
    }

    fn place_melody(&mut self, scale: Scale) {
        let shape = scale.kind.shape();
        let tonic = melody_tonic(scale.tonic);
        let home = shape.home;

        for (place, key) in HOME_ROW.iter().enumerate() {
            // The last key repeats the first note an octave up.
            let step = match home.get(place) {
                Some(step) => *step,
                None => home[0] + 12,
            };
            self.place_note(key, tonic + step);
        }
        for (place, key) in NUMBER_ROW.iter().enumerate() {
            self.place_note(key, tonic + home[place] + 12);
        }
        for (place, key) in BOTTOM_ROW.iter().enumerate() {
            self.place_note(key, tonic + home[place] - 12);
        }
        for (place, key) in UPPER_END.iter().enumerate() {
            self.place_note(key, tonic + home[place + 1] + 12);
        }
        for (place, key) in UPPER_ROW.iter().enumerate() {
            // A scale with fewer notes than keys repeats its best notes two
            // octaves up.
            let note = match shape.upper.get(place) {
                Some(step) => tonic + *step,
                None => tonic + home[1 + place - shape.upper.len()] + 24,
            };
            self.place_note(key, note);
        }
    }

    fn place_note(&mut self, key: &str, note: i16) {
        self.keys.insert(String::from(key), vec![note as u8]);
    }

    // Puts `notes` on `key`, lowest first and each once. No notes clears it.
    // Refuses a key no mapping uses, a note past 127 and too many notes.
    pub fn set_key(&mut self, key: &str, notes: &[u8]) -> Result<(), String> {
        if !is_mappable(key) {
            return Err(format!("{key} can't play notes: it does something else in Jello, or nothing"));
        }
        let mut sorted = notes.to_vec();
        sorted.sort();
        sorted.dedup();
        if sorted.len() > MOST_NOTES_ON_A_KEY {
            return Err(format!("a key plays at most {MOST_NOTES_ON_A_KEY} notes, not {}", sorted.len()));
        }
        for note in &sorted {
            if *note > 127 {
                return Err(format!("{note} is not a note: they go from 0 to 127"));
            }
        }
        if sorted.is_empty() {
            self.keys.remove(key);
        } else {
            self.keys.insert(String::from(key), sorted);
        }
        Ok(())
    }

    // Locks `key`, so an optimizer can't move it, or lets it go.
    pub fn set_locked(&mut self, key: &str, locked: bool) -> Result<(), String> {
        if !is_mappable(key) {
            return Err(format!("{key} can't play notes, so there is nothing to lock"));
        }
        if locked {
            self.locked.insert(String::from(key));
        } else {
            self.locked.remove(key);
        }
        Ok(())
    }

    // The default for `scale`, keeping every locked key as it is.
    pub fn with_default_for(&self, scale: Scale) -> KeyMapping {
        let mut mapping = KeyMapping::default_for(scale);
        for key in &self.locked {
            match self.keys.get(key) {
                Some(notes) => {
                    mapping.keys.insert(key.clone(), notes.clone());
                }
                None => {
                    mapping.keys.remove(key);
                }
            }
        }
        mapping.locked = self.locked.clone();
        mapping
    }

    // The first key that plays exactly `notes`, in the order of the keys'
    // names, or None.
    pub fn key_playing(&self, notes: &[u8]) -> Option<&str> {
        for (key, played) in &self.keys {
            if played == notes {
                return Some(key);
            }
        }
        None
    }
}

// The home note in the middle of the melody keys' reach, from G3 to F#4, so
// C major's is C4 and A minor's is A3 (docs/design.md#song-key-mappings).
fn melody_tonic(pitch: u8) -> i16 {
    let pitch = i16::from(pitch);
    if pitch >= 7 {
        return 48 + pitch;
    }
    60 + pitch
}

// `chord` as notes from octave 3, its root from C3 to B3, moved up
// `inversion` times.
fn triad_notes(scale: Scale, chord: (u8, Quality), inversion: usize) -> Vec<u8> {
    let (root_step, quality) = chord;
    let root = 48 + (scale.tonic + root_step) % 12;
    let third = match quality {
        Quality::Major => 4,
        Quality::Minor => 3,
    };
    let mut notes = vec![root, root + third, root + 7];
    for _ in 0..inversion {
        notes.sort();
        notes[0] += 12;
    }
    notes.sort();
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::{all_keys, chord_of_name, chord_of_notes, guide};
    use crate::scales::{SCALE_KINDS, ScaleKind};
    use crate::song_file::note_of_name;

    fn test_scale(kind: ScaleKind, tonic: u8) -> Scale {
        Scale { tonic, kind }
    }

    fn c_major() -> KeyMapping {
        KeyMapping::default_for(test_scale(ScaleKind::Major, 0))
    }

    fn a_minor() -> KeyMapping {
        KeyMapping::default_for(test_scale(ScaleKind::Minor, 9))
    }

    fn assert_notes(mapping: &KeyMapping, keys: &[&str], names: &[&str]) {
        for (key, name) in keys.iter().zip(names) {
            let note = note_of_name(name).unwrap();
            assert_eq!(mapping.keys[*key], vec![note], "{key} should play {name}");
        }
    }

    fn assert_chords(mapping: &KeyMapping, keys: &[&str], names: &[&str]) {
        for (key, name) in keys.iter().zip(names) {
            let wanted = chord_of_name(name).unwrap();
            let chord = chord_of_notes(&mapping.keys[*key]).unwrap();
            assert_eq!(chord.pitch(), wanted.pitch(), "{key} should play {name}");
            assert_eq!(chord.quality, wanted.quality, "{key} should play {name}");
        }
    }

    #[test]
    fn c_major_keeps_the_approved_melody_shape() {
        let mapping = c_major();
        assert_notes(&mapping, &NUMBER_ROW, &["A4", "C5", "D5", "E5", "G5"]);
        assert_notes(&mapping, &UPPER_ROW, &["F#4", "D#4", "F4", "B3", "A#3"]);
        assert_notes(&mapping, &UPPER_END, &["C5", "D5", "E5"]);
        assert_notes(&mapping, &HOME_ROW, &["A3", "C4", "D4", "E4", "G4", "A4"]);
        assert_notes(&mapping, &BOTTOM_ROW, &["A2", "C3", "D3", "E3", "G3"]);
    }

    #[test]
    fn a_minor_keeps_the_approved_melody_shape() {
        let mapping = a_minor();
        assert_notes(&mapping, &NUMBER_ROW, &["G4", "A4", "C5", "D5", "E5"]);
        assert_notes(&mapping, &UPPER_ROW, &["F#4", "D#4", "F4", "B3", "G#3"]);
        assert_notes(&mapping, &UPPER_END, &["A4", "C5", "D5"]);
        assert_notes(&mapping, &HOME_ROW, &["G3", "A3", "C4", "D4", "E4", "G4"]);
        assert_notes(&mapping, &BOTTOM_ROW, &["G2", "A2", "C3", "D3", "E3"]);
    }

    #[test]
    fn c_major_places_its_chords_by_role() {
        let mapping = c_major();
        assert_chords(&mapping, &["KeyA", "KeyS", "KeyD", "KeyF", "KeyG"], &["Am", "G", "F", "C", "Dm"]);
        assert_chords(&mapping, &["KeyQ", "KeyW", "KeyE", "KeyR", "KeyT"], &["Eb", "Ab", "Fm", "Bb", "Em"]);
        assert_chords(&mapping, &["KeyZ", "KeyX", "KeyC", "KeyV"], &["B", "A", "D", "E"]);
    }

    #[test]
    fn the_spare_keys_hold_other_voicings_of_the_best_chords() {
        let mapping = c_major();
        // The best chord, C, in first inversion on the first spare key.
        assert_eq!(mapping.keys["KeyB"], vec![52, 55, 60]);
        // No two left-hand keys play the same voicing.
        let mut seen = Vec::new();
        for key in CHORD_KEYS_BY_ROLE.iter().chain(SPARE_CHORD_KEYS.iter()) {
            let notes = &mapping.keys[*key];
            assert!(!seen.contains(notes), "{key} repeats {notes:?}");
            seen.push(notes.clone());
        }
    }

    #[test]
    fn every_scale_on_every_home_note_fills_all_47_keys_with_its_own_notes() {
        for kind in SCALE_KINDS {
            for tonic in 0..12 {
                let scale = test_scale(kind, tonic);
                let mapping = KeyMapping::default_for(scale);
                assert_eq!(mapping.keys.len(), 47, "{}", scale.name());
                for key in HOME_ROW.iter().chain(&NUMBER_ROW).chain(&UPPER_ROW).chain(&UPPER_END).chain(&BOTTOM_ROW) {
                    let notes = &mapping.keys[*key];
                    assert_eq!(notes.len(), 1, "{} {key}", scale.name());
                    assert!(scale.holds(notes[0]), "{} {key} plays {}", scale.name(), notes[0]);
                }
                for key in CHORD_KEYS_BY_ROLE.iter().chain(&SPARE_CHORD_KEYS) {
                    let notes = &mapping.keys[*key];
                    assert!(chord_of_notes(notes).is_some(), "{} {key} plays {notes:?}", scale.name());
                }
            }
        }
    }

    #[test]
    fn major_and_minor_keys_hold_every_chord_guidance_lets_work() {
        // 14 chords fit a major key and 11 a minor one today.
        for key in all_keys() {
            let scale = Scale::of_key(key);
            let mapping = KeyMapping::default_for(scale);
            let mut fitting = 0;
            for place in 0..CHORD_KEYS_BY_ROLE.len() {
                let notes = &mapping.keys[CHORD_KEYS_BY_ROLE[place]];
                let chord = chord_of_notes(notes).unwrap();
                if guide(key, chord).fit != crate::harmony::Fit::Outside {
                    fitting += 1;
                }
            }
            let expected = match key.mode {
                crate::harmony::Mode::Major => 14,
                crate::harmony::Mode::Minor => 11,
            };
            assert!(fitting >= expected, "{} has {fitting}", scale.name());
        }
    }

    #[test]
    fn set_key_sorts_clears_and_refuses_what_it_should() {
        let mut mapping = KeyMapping::default();
        mapping.set_key("KeyF", &[64, 60, 60]).unwrap();
        assert_eq!(mapping.keys["KeyF"], vec![60, 64]);
        mapping.set_key("KeyF", &[]).unwrap();
        assert!(mapping.is_empty());

        assert!(mapping.set_key("ShiftRight", &[60]).is_err());
        assert!(mapping.set_key("KeyF", &[128]).is_err());
        let too_many: Vec<u8> = (40..60).collect();
        assert!(mapping.set_key("KeyF", &too_many).is_err());
        assert!(mapping.is_empty());
    }

    #[test]
    fn a_new_default_keeps_the_locked_keys() {
        let mut mapping = c_major();
        mapping.set_key("KeyJ", &[61, 65]).unwrap();
        mapping.set_locked("KeyJ", true).unwrap();
        mapping.set_key("KeyK", &[]).unwrap();
        mapping.set_locked("KeyK", true).unwrap();
        assert!(mapping.set_locked("Enter", true).is_err());

        let next = mapping.with_default_for(test_scale(ScaleKind::Minor, 9));
        assert_eq!(next.keys["KeyJ"], vec![61, 65]);
        assert!(!next.keys.contains_key("KeyK"));
        assert_eq!(next.keys["KeyL"], a_minor().keys["KeyL"]);
        assert!(next.locked.contains("KeyJ"));
    }

    #[test]
    fn the_key_playing_some_notes_is_found() {
        let mapping = c_major();
        assert_eq!(mapping.key_playing(&[48, 52, 55]), Some("KeyF"));
        assert_eq!(mapping.key_playing(&[1]), None);
    }
}
