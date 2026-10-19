// The laptop keyboard: what each key does, and which note each key held down
// started (docs/adr/0009-how-notes-become-sound.md).
//
// Keys are named by their place on the keyboard, the way the webview's
// `event.code` names them ("KeyQ", "Digit2", "Slash"), not by the letter
// printed on them. So every keyboard layout plays the same spots.

use std::collections::HashMap;

// Laptop keys can't feel how hard they are hit, so every note they play uses
// this velocity (1 to 127).
pub const LAPTOP_VELOCITY: u8 = 100;

// The default layout's notes (docs/design.md, "Computer keyboard"), in piano
// order. The Z row plays C3 to B3 and the Q row plays C4 to B4, with the
// black keys on the row above each one. A piano has no black key between E
// and F, so F and 4 play nothing.
const DEFAULT_NOTES: [(&str, u8); 24] = [
    ("KeyZ", 48),   // C3
    ("KeyS", 49),   // C#3
    ("KeyX", 50),   // D3
    ("KeyD", 51),   // D#3
    ("KeyC", 52),   // E3
    ("KeyV", 53),   // F3
    ("KeyG", 54),   // F#3
    ("KeyB", 55),   // G3
    ("KeyH", 56),   // G#3
    ("KeyN", 57),   // A3
    ("KeyJ", 58),   // A#3
    ("KeyM", 59),   // B3
    ("KeyQ", 60),   // C4, middle C
    ("Digit2", 61), // C#4
    ("KeyW", 62),   // D4
    ("Digit3", 63), // D#4
    ("KeyE", 64),   // E4
    ("KeyR", 65),   // F4
    ("Digit5", 66), // F#4
    ("KeyT", 67),   // G4
    ("Digit6", 68), // G#4
    ("KeyY", 69),   // A4
    ("Digit7", 70), // A#4
    ("KeyU", 71),   // B4
];

// The default layout's key for moving new notes on to the next wave.
const WAVE_KEY: &str = "Slash";

// What pressing a key does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyAction {
    // Plays this note until the key is let go.
    PlayNote(u8),
    // Moves new notes on to the next wave.
    SwitchWave,
}

pub struct KeyMap {
    // What each key does. A key that isn't in here does nothing.
    layout: HashMap<String, KeyAction>,
    // The keys held down right now, each with the note it started.
    held: HashMap<String, u8>,
}

impl KeyMap {
    // The default layout, with no keys held.
    pub fn new() -> KeyMap {
        let mut layout = HashMap::new();
        for (key, note) in DEFAULT_NOTES {
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
        // A held key is already playing its note. Doing nothing means one key
        // can never start two notes.
        if self.held.contains_key(key) {
            return None;
        }

        // A key that isn't in the layout does nothing.
        let action = match self.layout.get(key) {
            Some(action) => *action,
            None => return None,
        };

        // Remember the note, so letting go of the key stops this one.
        if let KeyAction::PlayNote(note) = action {
            self.held.insert(String::from(key), note);
        }
        Some(action)
    }

    // The note to stop when `key` is let go: the one it started, even if the
    // layout has changed since, so no note gets stuck. None if it started no
    // note.
    pub fn release(&mut self, key: &str) -> Option<u8> {
        self.held.remove(key)
    }

    // Forgets every held key, for when every note is let go at once.
    pub fn release_all(&mut self) {
        self.held.clear();
    }
}

// These tests only ask the key map what keys do. They make no sound.
#[cfg(test)]
mod tests {
    use super::*;

    // Presses `keys` one after another and checks they play every note from
    // `lowest` up, each one step above the last, like a piano's keys from left
    // to right.
    fn assert_plays_in_order(keys: &[&str], lowest: u8) {
        let mut key_map = KeyMap::new();
        let mut note = lowest;
        for key in keys {
            let action = key_map.press(key);
            assert_eq!(action, Some(KeyAction::PlayNote(note)), "key {key}");
            note += 1;
        }
    }

    #[test]
    fn the_z_row_plays_c3_to_b3() {
        // The white keys Z X C V B N M with the black keys S D G H J between
        // them, in piano order. C3 is note 48.
        let keys = [
            "KeyZ", "KeyS", "KeyX", "KeyD", "KeyC", "KeyV", "KeyG", "KeyB", "KeyH", "KeyN", "KeyJ",
            "KeyM",
        ];
        assert_plays_in_order(&keys, 48);
    }

    #[test]
    fn the_q_row_plays_c4_to_b4() {
        // The white keys Q W E R T Y U with the black keys 2 3 5 6 7 between
        // them, in piano order. C4 is note 60, so Q is middle C.
        let keys = [
            "KeyQ", "Digit2", "KeyW", "Digit3", "KeyE", "KeyR", "Digit5", "KeyT", "Digit6", "KeyY",
            "Digit7", "KeyU",
        ];
        assert_plays_in_order(&keys, 60);
    }

    #[test]
    fn slash_switches_the_wave() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("Slash"), Some(KeyAction::SwitchWave));
    }

    #[test]
    fn keys_outside_the_layout_do_nothing() {
        // The black-key rows have gaps where a piano has no black key: A, F
        // and K, and 1, 4 and 8. The Z row's notes end at M and the Q row's
        // at U.
        let mut key_map = KeyMap::new();
        let keys = [
            "KeyA", "KeyF", "KeyK", "Digit1", "Digit4", "Digit8", "Comma", "KeyI", "Space",
        ];
        for key in keys {
            assert_eq!(key_map.press(key), None, "key {key}");
        }

        // The layout holds the 24 notes and the wave key, and nothing else.
        assert_eq!(key_map.layout.len(), 25);
    }

    #[test]
    fn pressing_a_held_key_again_does_nothing() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("KeyQ"), Some(KeyAction::PlayNote(60)));
        assert_eq!(key_map.press("KeyQ"), None);

        // Once it's let go, it plays again.
        assert_eq!(key_map.release("KeyQ"), Some(60));
        assert_eq!(key_map.press("KeyQ"), Some(KeyAction::PlayNote(60)));
    }

    #[test]
    fn letting_go_stops_the_note_the_key_started() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyQ");
        key_map.press("KeyZ");
        assert_eq!(key_map.release("KeyZ"), Some(48));
        assert_eq!(key_map.release("KeyQ"), Some(60));
    }

    #[test]
    fn letting_go_of_a_key_that_started_no_note_does_nothing() {
        let mut key_map = KeyMap::new();

        // Never pressed.
        assert_eq!(key_map.release("KeyQ"), None);

        // Pressed, but it plays no note.
        key_map.press("Slash");
        assert_eq!(key_map.release("Slash"), None);
        key_map.press("KeyA");
        assert_eq!(key_map.release("KeyA"), None);

        // Let go of already.
        key_map.press("KeyQ");
        key_map.release("KeyQ");
        assert_eq!(key_map.release("KeyQ"), None);
    }

    #[test]
    fn letting_go_stops_the_started_note_even_after_the_layout_changes() {
        // Hold Q, then move Q to D4 while it's held. Letting go still stops
        // middle C, the note Q started, so it can't get stuck.
        let mut key_map = KeyMap::new();
        key_map.press("KeyQ");
        let play_d4 = KeyAction::PlayNote(62);
        key_map.layout.insert(String::from("KeyQ"), play_d4);
        assert_eq!(key_map.release("KeyQ"), Some(60));

        // The next press plays the new note.
        assert_eq!(key_map.press("KeyQ"), Some(KeyAction::PlayNote(62)));
    }

    #[test]
    fn letting_go_of_everything_forgets_every_held_key() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyQ");
        key_map.press("KeyZ");
        key_map.release_all();

        // Nothing is left to let go of, and the keys play again.
        assert_eq!(key_map.release("KeyQ"), None);
        assert_eq!(key_map.release("KeyZ"), None);
        assert_eq!(key_map.press("KeyQ"), Some(KeyAction::PlayNote(60)));
    }
}
