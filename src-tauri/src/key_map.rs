// The laptop keyboard: what each key does, and which notes each key held down
// started (docs/adr/0009-how-notes-become-sound.md).
//
// Keys are named by their place on the keyboard, the way the webview's
// `event.code` names them ("KeyQ", "Digit2", "Slash"), not by the letter
// printed on them. So every keyboard layout plays the same spots.

use std::collections::HashMap;

use serde::Serialize;

// Laptop keys can't feel how hard they are hit, so every note they play uses
// this velocity (1 to 127).
pub const LAPTOP_VELOCITY: u8 = 100;

// The default layout (docs/design.md, "Computer keyboard"): the left hand
// plays chords and the right hand plays the melody, like on an accordion.

// The left hand's chords. Each letter key from A to G plays the major chord
// with its name, so a chord chart like "C G F" is played by typing it. In
// touch typing, all seven are left-hand keys. Each chord's root, the note it
// is named after, is in octave 3, from C3 up to B3.
const CHORD_ROOTS: [(&str, u8); 7] = [
    ("KeyC", 48), // C3
    ("KeyD", 50), // D3
    ("KeyE", 52), // E3
    ("KeyF", 53), // F3
    ("KeyG", 55), // G3
    ("KeyA", 57), // A3
    ("KeyB", 59), // B3
];

// The right hand's melody, one note per key. Each finger plays a column of
// four keys, from the bottom row up to the number row, each a half step
// above the one below it, and the next finger's column carries on from
// there. So an octave is always the same row, three fingers over: M is
// middle C and / is the C above it. The keys play G#3 to D#5. Listed finger
// by finger, bottom row first.
const MELODY_NOTES: [(&str, u8); 20] = [
    // Index reaching in.
    ("KeyN", 56),   // G#3
    ("KeyH", 57),   // A3
    ("KeyY", 58),   // A#3
    ("Digit6", 59), // B3
    // Index.
    ("KeyM", 60),   // C4, middle C
    ("KeyJ", 61),   // C#4
    ("KeyU", 62),   // D4
    ("Digit7", 63), // D#4
    // Middle.
    ("Comma", 64),  // E4
    ("KeyK", 65),   // F4
    ("KeyI", 66),   // F#4
    ("Digit8", 67), // G4
    // Ring.
    ("Period", 68), // G#4
    ("KeyL", 69),   // A4
    ("KeyO", 70),   // A#4
    ("Digit9", 71), // B4
    // Pinky.
    ("Slash", 72),     // C5
    ("Semicolon", 73), // C#5
    ("KeyP", 74),      // D5
    ("Digit0", 75),    // D#5
];

// The default layout's key for moving new notes on to the next wave: ` in the
// top left corner, away from every note.
const WAVE_KEY: &str = "Backquote";

// The major chord on `root`, lowest note first: the root, the note four half
// steps above it and the note seven half steps above it. C major is C, E
// and G.
fn major_chord(root: u8) -> [u8; 3] {
    [root, root + 4, root + 7]
}

// What pressing a key does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyAction {
    // Plays this note until the key is let go.
    PlayNote(u8),
    // Plays these notes together until the key is let go: a chord, lowest
    // note first.
    PlayChord([u8; 3]),
    // Moves new notes on to the next wave.
    SwitchWave,
}

pub struct KeyMap {
    // What each key does. A key that isn't in here does nothing.
    layout: HashMap<String, KeyAction>,
    // The keys held down right now, each with the notes it started.
    held: HashMap<String, Vec<u8>>,
}

// What each key in the layout does, for the UI to draw on the keys.
#[derive(Serialize)]
pub struct LayoutForUi {
    // Each key that plays one note, with its note: "KeyM" plays 60.
    notes: HashMap<String, u8>,
    // Each key that plays a chord, with its notes, lowest first: "KeyC"
    // plays [48, 52, 55].
    chords: HashMap<String, [u8; 3]>,
    // The keys that move new notes on to the next wave.
    wave_keys: Vec<String>,
}

impl KeyMap {
    // The default layout, with no keys held.
    pub fn new() -> KeyMap {
        let mut layout = HashMap::new();
        for (key, root) in CHORD_ROOTS {
            let chord = major_chord(root);
            layout.insert(String::from(key), KeyAction::PlayChord(chord));
        }
        for (key, note) in MELODY_NOTES {
            layout.insert(String::from(key), KeyAction::PlayNote(note));
        }
        layout.insert(String::from(WAVE_KEY), KeyAction::SwitchWave);

        KeyMap {
            layout,
            held: HashMap::new(),
        }
    }

    // What pressing `key` should do, or None for nothing.
    pub fn press(&mut self, key: &str) -> Option<KeyAction> {
        // A held key is already playing its notes. Doing nothing means one key
        // can never start them twice.
        if self.held.contains_key(key) {
            return None;
        }

        // A key that isn't in the layout does nothing.
        let action = match self.layout.get(key) {
            Some(action) => *action,
            None => return None,
        };

        // Remember the notes, so letting go of the key stops these ones. If
        // another held key is already playing one of them, the engine plays
        // it again on the voice it already has, so the note sounds once.
        match action {
            KeyAction::PlayNote(note) => {
                self.held.insert(String::from(key), vec![note]);
            }
            KeyAction::PlayChord(notes) => {
                self.held.insert(String::from(key), notes.to_vec());
            }
            KeyAction::SwitchWave => {}
        }
        Some(action)
    }

    // The notes to stop when `key` is let go: the ones it started, even if the
    // layout has changed since, so no note gets stuck. A note another held key
    // is still playing is left out: the A chord and H both play A3, so A3
    // keeps sounding until both are let go. Empty if the key started no notes.
    pub fn release(&mut self, key: &str) -> Vec<u8> {
        let started = match self.held.remove(key) {
            Some(notes) => notes,
            None => return Vec::new(),
        };

        let mut to_stop = Vec::new();
        for note in started {
            if !self.is_note_held(note) {
                to_stop.push(note);
            }
        }
        to_stop
    }

    // Whether any held key is playing `note`.
    fn is_note_held(&self, note: u8) -> bool {
        for held_notes in self.held.values() {
            if held_notes.contains(&note) {
                return true;
            }
        }
        false
    }

    // Forgets every held key, for when every note is let go at once.
    pub fn release_all(&mut self) {
        self.held.clear();
    }

    // What every key in the layout does right now, for the UI to draw.
    pub fn layout_for_ui(&self) -> LayoutForUi {
        let mut notes = HashMap::new();
        let mut chords = HashMap::new();
        let mut wave_keys = Vec::new();
        for (key, action) in &self.layout {
            match action {
                KeyAction::PlayNote(note) => {
                    notes.insert(key.clone(), *note);
                }
                KeyAction::PlayChord(chord) => {
                    chords.insert(key.clone(), *chord);
                }
                KeyAction::SwitchWave => {
                    wave_keys.push(key.clone());
                }
            }
        }

        // The layout keeps its keys in no order, so sort them to give the
        // same answer every time.
        wave_keys.sort();

        LayoutForUi {
            notes,
            chords,
            wave_keys,
        }
    }
}

// These tests only ask the key map what keys do. They make no sound.
#[cfg(test)]
mod tests {
    use super::*;

    // Presses each finger's keys, bottom row first, one finger after another,
    // and checks they play every note from `lowest` up, each one half step
    // above the last.
    fn assert_plays_in_order(fingers: &[[&str; 4]], lowest: u8) {
        let mut key_map = KeyMap::new();
        let mut note = lowest;
        for finger in fingers {
            for key in finger {
                let action = key_map.press(key);
                assert_eq!(action, Some(KeyAction::PlayNote(note)), "key {key}");
                note += 1;
            }
        }
    }

    #[test]
    fn the_right_hand_plays_g_sharp_3_to_d_sharp_5_finger_by_finger() {
        // G#3 is note 56.
        let fingers = [
            ["KeyN", "KeyH", "KeyY", "Digit6"],       // index reaching in
            ["KeyM", "KeyJ", "KeyU", "Digit7"],       // index
            ["Comma", "KeyK", "KeyI", "Digit8"],      // middle
            ["Period", "KeyL", "KeyO", "Digit9"],     // ring
            ["Slash", "Semicolon", "KeyP", "Digit0"], // pinky
        ];
        assert_plays_in_order(&fingers, 56);
    }

    #[test]
    fn each_letter_from_a_to_g_plays_its_major_chord() {
        // Each chord's root is in octave 3: C3 is note 48.
        let chords = [
            ("KeyC", [48, 52, 55]), // C3 E3 G3
            ("KeyD", [50, 54, 57]), // D3 F#3 A3
            ("KeyE", [52, 56, 59]), // E3 G#3 B3
            ("KeyF", [53, 57, 60]), // F3 A3 C4
            ("KeyG", [55, 59, 62]), // G3 B3 D4
            ("KeyA", [57, 61, 64]), // A3 C#4 E4
            ("KeyB", [59, 63, 66]), // B3 D#4 F#4
        ];
        let mut key_map = KeyMap::new();
        for (key, chord) in chords {
            let action = key_map.press(key);
            assert_eq!(action, Some(KeyAction::PlayChord(chord)), "key {key}");
        }
    }

    #[test]
    fn backquote_switches_the_wave() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("Backquote"), Some(KeyAction::SwitchWave));
    }

    #[test]
    fn keys_outside_the_layout_do_nothing() {
        // The left hand's other keys, the keys past the right pinky, Space
        // and the Shifts.
        let mut key_map = KeyMap::new();
        let keys = [
            "Digit1",
            "Digit5",
            "KeyQ",
            "KeyW",
            "KeyR",
            "KeyT",
            "KeyS",
            "KeyZ",
            "KeyX",
            "KeyV",
            "Tab",
            "CapsLock",
            "Minus",
            "BracketLeft",
            "Quote",
            "Backslash",
            "Enter",
            "Space",
            "ShiftLeft",
            "ShiftRight",
        ];
        for key in keys {
            assert_eq!(key_map.press(key), None, "key {key}");
        }

        // The layout holds the 7 chords, the 20 notes and the wave key, and
        // nothing else.
        assert_eq!(key_map.layout.len(), 28);
    }

    #[test]
    fn pressing_a_held_key_again_does_nothing() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
        assert_eq!(key_map.press("KeyM"), None);

        // Once it's let go, it plays again.
        assert_eq!(key_map.release("KeyM"), vec![60]);
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));

        // The same goes for a chord.
        let c_major = KeyAction::PlayChord([48, 52, 55]);
        assert_eq!(key_map.press("KeyC"), Some(c_major));
        assert_eq!(key_map.press("KeyC"), None);
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
        assert_eq!(key_map.press("KeyC"), Some(c_major));
    }

    #[test]
    fn letting_go_stops_the_notes_the_key_started() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        key_map.press("KeyC");
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
        assert_eq!(key_map.release("KeyM"), vec![60]);
    }

    #[test]
    fn a_note_a_chord_and_a_held_key_share_stops_when_both_are_let_go() {
        // The A chord (A3 C#4 E4) and H both play A3. The second press plays
        // it again.
        let mut key_map = KeyMap::new();
        let a_major = KeyAction::PlayChord([57, 61, 64]);
        assert_eq!(key_map.press("KeyA"), Some(a_major));
        assert_eq!(key_map.press("KeyH"), Some(KeyAction::PlayNote(57)));

        // Letting go of the chord stops its other notes, but A3 keeps
        // sounding for H, still held.
        assert_eq!(key_map.release("KeyA"), vec![61, 64]);
        assert_eq!(key_map.release("KeyH"), vec![57]);
    }

    #[test]
    fn a_note_two_held_chords_share_stops_when_both_are_let_go() {
        // The C chord (C3 E3 G3) and the E chord (E3 G#3 B3) both play E3.
        let mut key_map = KeyMap::new();
        key_map.press("KeyC");
        key_map.press("KeyE");

        // Letting go of E keeps E3 sounding for C, still held. E3 is C's
        // middle note, so every note of a held chord counts, not just its
        // root.
        assert_eq!(key_map.release("KeyE"), vec![56, 59]);
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
    }

    #[test]
    fn letting_go_of_a_key_that_started_no_note_does_nothing() {
        let mut key_map = KeyMap::new();
        let nothing: Vec<u8> = Vec::new();

        // Never pressed.
        assert_eq!(key_map.release("KeyM"), nothing);

        // Pressed, but it plays no note.
        key_map.press("Backquote");
        assert_eq!(key_map.release("Backquote"), nothing);
        key_map.press("Space");
        assert_eq!(key_map.release("Space"), nothing);

        // Let go of already.
        key_map.press("KeyM");
        key_map.release("KeyM");
        assert_eq!(key_map.release("KeyM"), nothing);
    }

    #[test]
    fn letting_go_stops_the_started_notes_even_after_the_layout_changes() {
        // Hold M, then move M to D4 while it's held. Letting go still stops
        // middle C, the note M started, so it can't get stuck.
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        let play_d4 = KeyAction::PlayNote(62);
        key_map.layout.insert(String::from("KeyM"), play_d4);
        assert_eq!(key_map.release("KeyM"), vec![60]);

        // The next press plays the new note.
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(62)));

        // The same goes for a chord: hold C, then move C to the D chord.
        key_map.press("KeyC");
        let play_d_major = KeyAction::PlayChord([50, 54, 57]);
        key_map.layout.insert(String::from("KeyC"), play_d_major);
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
    }

    #[test]
    fn letting_go_of_everything_forgets_every_held_key() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        key_map.press("KeyC");
        key_map.release_all();

        // Nothing is left to let go of, and the keys play again.
        let nothing: Vec<u8> = Vec::new();
        assert_eq!(key_map.release("KeyM"), nothing);
        assert_eq!(key_map.release("KeyC"), nothing);
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
        assert_eq!(key_map.press("KeyC"), Some(KeyAction::PlayChord([48, 52, 55])));
    }

    #[test]
    fn the_ui_is_told_every_note_chord_and_the_wave_key() {
        let key_map = KeyMap::new();
        let layout = key_map.layout_for_ui();

        assert_eq!(layout.notes.len(), 20);
        for (key, note) in MELODY_NOTES {
            assert_eq!(layout.notes.get(key), Some(&note), "key {key}");
        }
        assert_eq!(layout.chords.len(), 7);
        assert_eq!(layout.chords.get("KeyC"), Some(&[48, 52, 55]));
        assert_eq!(layout.chords.get("KeyB"), Some(&[59, 63, 66]));
        assert_eq!(layout.wave_keys, vec![String::from("Backquote")]);
    }

    #[test]
    fn the_ui_is_told_the_layout_as_it_is_now() {
        // Move M to D4, and the UI is told M plays D4.
        let mut key_map = KeyMap::new();
        let play_d4 = KeyAction::PlayNote(62);
        key_map.layout.insert(String::from("KeyM"), play_d4);

        let layout = key_map.layout_for_ui();
        assert_eq!(layout.notes.get("KeyM"), Some(&62));
    }

    #[test]
    fn the_ui_reads_the_layout_by_these_names() {
        // The UI reads `notes`, `chords` and `wave_keys` (KeyLayout in
        // ui/keyboard.ts), so renaming them here would leave keys blank.
        let key_map = KeyMap::new();
        let json = serde_json::to_value(key_map.layout_for_ui()).unwrap();
        assert_eq!(json["notes"]["KeyM"], 60);
        assert_eq!(json["chords"]["KeyC"][0], 48);
        assert_eq!(json["chords"]["KeyC"][2], 55);
        assert_eq!(json["wave_keys"][0], "Backquote");
    }
}
