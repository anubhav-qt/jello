// The laptop keyboard: what each key does, and which note each key held down
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

// The default layout's notes (docs/design.md, "Computer keyboard"): the
// accordion grid, named after the button grid of the chromatic accordion.
// Each finger plays a column of three keys. Going up a column, from the
// bottom row to the top row, each key is a half step higher, and the next
// finger's column carries on from there. So each hand's four resting fingers
// cover one octave, and a chord keeps its shape wherever it is played. The
// left hand plays C3 to D4 and the right hand A3 to B4, so A3 to D4 are under
// both hands. Listed finger by finger, bottom row first.
const DEFAULT_NOTES: [(&str, u8); 30] = [
    // Left pinky, ring, middle and index, then the index reaching in.
    ("KeyZ", 48), // C3
    ("KeyA", 49), // C#3
    ("KeyQ", 50), // D3
    ("KeyX", 51), // D#3
    ("KeyS", 52), // E3
    ("KeyW", 53), // F3
    ("KeyC", 54), // F#3
    ("KeyD", 55), // G3
    ("KeyE", 56), // G#3
    ("KeyV", 57), // A3
    ("KeyF", 58), // A#3
    ("KeyR", 59), // B3
    ("KeyB", 60), // C4, middle C
    ("KeyG", 61), // C#4
    ("KeyT", 62), // D4
    // Right index reaching in, then index, middle, ring and pinky.
    ("KeyN", 57),      // A3
    ("KeyH", 58),      // A#3
    ("KeyY", 59),      // B3
    ("KeyM", 60),      // C4, middle C
    ("KeyJ", 61),      // C#4
    ("KeyU", 62),      // D4
    ("Comma", 63),     // D#4
    ("KeyK", 64),      // E4
    ("KeyI", 65),      // F4
    ("Period", 66),    // F#4
    ("KeyL", 67),      // G4
    ("KeyO", 68),      // G#4
    ("Slash", 69),     // A4
    ("Semicolon", 70), // A#4
    ("KeyP", 71),      // B4
];

// The default layout's key for moving new notes on to the next wave: ` in the
// top left corner, away from every note.
const WAVE_KEY: &str = "Backquote";

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

// What each key in the layout does, for the UI to draw on the keys.
#[derive(Serialize)]
pub struct LayoutForUi {
    // Each key that plays a note, with its note: "KeyM" plays 60.
    notes: HashMap<String, u8>,
    // The keys that move new notes on to the next wave.
    wave_keys: Vec<String>,
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

        // Remember the note, so letting go of the key stops this one. If
        // another held key is already playing the same note, the engine plays
        // it again on the voice it already has, so the note sounds once.
        if let KeyAction::PlayNote(note) = action {
            self.held.insert(String::from(key), note);
        }
        Some(action)
    }

    // The note to stop when `key` is let go: the one it started, even if the
    // layout has changed since, so no note gets stuck. None if it started no
    // note, or if another held key is still playing the same note: B and M
    // both play middle C, so it keeps sounding until both are let go.
    pub fn release(&mut self, key: &str) -> Option<u8> {
        let note = match self.held.remove(key) {
            Some(note) => note,
            None => return None,
        };

        if self.is_note_held(note) {
            return None;
        }
        Some(note)
    }

    // Whether any held key is playing `note`.
    fn is_note_held(&self, note: u8) -> bool {
        for held_note in self.held.values() {
            if *held_note == note {
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
        let mut wave_keys = Vec::new();
        for (key, action) in &self.layout {
            match action {
                KeyAction::PlayNote(note) => {
                    notes.insert(key.clone(), *note);
                }
                KeyAction::SwitchWave => {
                    wave_keys.push(key.clone());
                }
            }
        }

        // The layout keeps its keys in no order, so sort them to give the
        // same answer every time.
        wave_keys.sort();

        LayoutForUi { notes, wave_keys }
    }
}

// These tests only ask the key map what keys do. They make no sound.
#[cfg(test)]
mod tests {
    use super::*;

    // Presses each finger's keys, bottom row first, one finger after another,
    // and checks they play every note from `lowest` up, each one half step
    // above the last.
    fn assert_plays_in_order(fingers: &[[&str; 3]], lowest: u8) {
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
    fn the_left_hand_plays_c3_to_d4_finger_by_finger() {
        // C3 is note 48.
        let fingers = [
            ["KeyZ", "KeyA", "KeyQ"], // pinky
            ["KeyX", "KeyS", "KeyW"], // ring
            ["KeyC", "KeyD", "KeyE"], // middle
            ["KeyV", "KeyF", "KeyR"], // index
            ["KeyB", "KeyG", "KeyT"], // index reaching in
        ];
        assert_plays_in_order(&fingers, 48);
    }

    #[test]
    fn the_right_hand_plays_a3_to_b4_finger_by_finger() {
        // The same grid as the left hand, so chords keep the same shapes. A3
        // is note 57.
        let fingers = [
            ["KeyN", "KeyH", "KeyY"],       // index reaching in
            ["KeyM", "KeyJ", "KeyU"],       // index
            ["Comma", "KeyK", "KeyI"],      // middle
            ["Period", "KeyL", "KeyO"],     // ring
            ["Slash", "Semicolon", "KeyP"], // pinky
        ];
        assert_plays_in_order(&fingers, 57);
    }

    #[test]
    fn backquote_switches_the_wave() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("Backquote"), Some(KeyAction::SwitchWave));
    }

    #[test]
    fn keys_outside_the_layout_do_nothing() {
        // The number row, the keys past each little finger, Space and the
        // Shifts.
        let mut key_map = KeyMap::new();
        let keys = [
            "Digit1",
            "Digit5",
            "Digit0",
            "Tab",
            "CapsLock",
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

        // The layout holds the 30 notes and the wave key, and nothing else.
        assert_eq!(key_map.layout.len(), 31);
    }

    #[test]
    fn pressing_a_held_key_again_does_nothing() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
        assert_eq!(key_map.press("KeyM"), None);

        // Once it's let go, it plays again.
        assert_eq!(key_map.release("KeyM"), Some(60));
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
    }

    #[test]
    fn letting_go_stops_the_note_the_key_started() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        key_map.press("KeyZ");
        assert_eq!(key_map.release("KeyZ"), Some(48));
        assert_eq!(key_map.release("KeyM"), Some(60));
    }

    #[test]
    fn a_note_two_held_keys_play_stops_when_both_are_let_go() {
        // B and M both play middle C. The second press plays it again.
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("KeyB"), Some(KeyAction::PlayNote(60)));
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));

        // Letting go of one keeps it sounding, for the key still held.
        assert_eq!(key_map.release("KeyB"), None);
        assert_eq!(key_map.release("KeyM"), Some(60));
    }

    #[test]
    fn letting_go_of_a_key_that_started_no_note_does_nothing() {
        let mut key_map = KeyMap::new();

        // Never pressed.
        assert_eq!(key_map.release("KeyM"), None);

        // Pressed, but it plays no note.
        key_map.press("Backquote");
        assert_eq!(key_map.release("Backquote"), None);
        key_map.press("Space");
        assert_eq!(key_map.release("Space"), None);

        // Let go of already.
        key_map.press("KeyM");
        key_map.release("KeyM");
        assert_eq!(key_map.release("KeyM"), None);
    }

    #[test]
    fn letting_go_stops_the_started_note_even_after_the_layout_changes() {
        // Hold M, then move M to D4 while it's held. Letting go still stops
        // middle C, the note M started, so it can't get stuck.
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        let play_d4 = KeyAction::PlayNote(62);
        key_map.layout.insert(String::from("KeyM"), play_d4);
        assert_eq!(key_map.release("KeyM"), Some(60));

        // The next press plays the new note.
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(62)));
    }

    #[test]
    fn letting_go_of_everything_forgets_every_held_key() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        key_map.press("KeyZ");
        key_map.release_all();

        // Nothing is left to let go of, and the keys play again.
        assert_eq!(key_map.release("KeyM"), None);
        assert_eq!(key_map.release("KeyZ"), None);
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
    }

    #[test]
    fn the_ui_is_told_every_note_and_the_wave_key() {
        let key_map = KeyMap::new();
        let layout = key_map.layout_for_ui();

        assert_eq!(layout.notes.len(), 30);
        for (key, note) in DEFAULT_NOTES {
            assert_eq!(layout.notes.get(key), Some(&note), "key {key}");
        }
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
        // The UI reads `notes` and `wave_keys` (KeyLayout in ui/keyboard.ts),
        // so renaming them here would leave every key blank.
        let key_map = KeyMap::new();
        let json = serde_json::to_value(key_map.layout_for_ui()).unwrap();
        assert_eq!(json["notes"]["KeyM"], 60);
        assert_eq!(json["wave_keys"][0], "Backquote");
    }
}
