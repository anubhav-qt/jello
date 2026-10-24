// The laptop keyboard: what each key does, and which notes each key held down
// started (docs/adr/0009-how-notes-become-sound.md).
//
// Keys are named by their place on the keyboard, the way the webview's
// `event.code` names them ("KeyQ", "Digit2", "Slash"), not by the letter
// printed on them. So every keyboard layout plays the same spots.

use std::collections::HashMap;

use serde::Serialize;

use crate::harmony::{Chord, Quality, chord_name};

// Laptop keys can't feel how hard they are hit, so every note they play uses
// this velocity (1 to 127).
pub const LAPTOP_VELOCITY: u8 = 100;

// The default layout (docs/design.md, "Computer keyboard"): the left hand
// plays chords and the right hand plays the melody, like on an accordion.

// The left hand's chords: all 24 major and minor chords, each on a key of
// its own (docs/adr/0020-every-chord-on-its-own-key.md), so no chord needs
// a key held down first. Each chord's root, the note it is named after, is
// in octave 3, from C3 up to B3.
const CHORDS: [(&str, Chord); 24] = [
    // Each letter key from A to G plays the major chord with its name, so a
    // chord chart like "C G F" is played by typing it. In touch typing, all
    // seven are left-hand keys.
    ("KeyC", major(48)), // C3 E3 G3
    ("KeyD", major(50)), // D
    ("KeyE", major(52)), // E
    ("KeyF", major(53)), // F
    ("KeyG", major(55)), // G
    ("KeyA", major(57)), // A
    ("KeyB", major(59)), // B
    // Each one's minor chord sits next to it: one key to the left, or one
    // key up where the key to the left already plays a chord.
    ("KeyX", minor(48)), // Cm, C3 Eb3 G3
    ("KeyS", minor(50)), // Dm
    ("KeyW", minor(52)), // Em
    ("KeyR", minor(53)), // Fm
    ("KeyT", minor(55)), // Gm
    ("KeyQ", minor(57)), // Am
    ("KeyV", minor(59)), // Bm
    // The major chords on the piano's five black keys sit on the number
    // row, low to high, the way the black keys sit above the white keys.
    ("Digit1", major(49)), // C#
    ("Digit2", major(51)), // Eb
    ("Digit3", major(54)), // F#
    ("Digit4", major(56)), // Ab
    ("Digit5", major(58)), // Bb
    // Their minor chords go around the left edge, low to high: from ` in the
    // top left corner down to Space, under the thumb. F#m sits on left
    // Control, since left Shift holds the vibrato key
    // (docs/adr/0032-the-vibrato-key.md).
    ("Backquote", minor(49)),   // C#m
    ("Tab", minor(51)),         // Ebm
    ("ControlLeft", minor(54)), // F#m
    ("KeyZ", minor(56)),        // G#m
    ("Space", minor(58)),       // Bbm
];

const fn major(root: u8) -> Chord {
    Chord {
        root,
        quality: Quality::Major,
    }
}

const fn minor(root: u8) -> Chord {
    Chord {
        root,
        quality: Quality::Minor,
    }
}

// The right hand's melody, one note per key. Each finger plays a column of
// four keys, from the bottom row up to the number row, each a half step
// above the one below it, and the next finger's column carries on from
// there. So an octave is always the same row, three fingers over: M is
// middle C and / is the C above it. The keys play G#3 to G5. Listed finger
// by finger, bottom row first.
const MELODY_NOTES: [(&str, u8); 24] = [
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
    ("Quote", 76),
    ("BracketLeft", 77),
    ("BracketRight", 78),
    ("Backslash", 79),
];

// F12 changes the wave; backslash now plays a melody note.
const WAVE_KEY: &str = "F12";

// The key held for vibrato, or whatever the song's routing list connects
// the vibrato key to (docs/adr/0032-the-vibrato-key.md): left Shift, under
// the left pinky.
const VIBRATO_KEY: &str = "ShiftLeft";

// The key held to glide: while it is held, each melody note slides in from
// the last one (docs/adr/0036-glide.md). Left Option on Mac, right Alt on Windows.
#[cfg(not(target_os = "windows"))]
pub(crate) const GLIDE_KEY: &str = "AltLeft";
#[cfg(target_os = "windows")]
pub(crate) const GLIDE_KEY: &str = "AltRight";

// The keys that run the beat and the looper
// (docs/adr/0021-a-looper-on-a-beat.md). Right Shift counts in, records and
// closes the loop, then starts and stops layers, and Esc removes a layer
// (docs/adr/0026-layers.md). - and = slow the beat down and speed it up.
const LOOPER_KEY: &str = "ShiftRight";
const CLEAR_KEY: &str = "Escape";
const SLOWER_KEY: &str = "Minus";
const FASTER_KEY: &str = "Equal";

// While Esc waits, the number row's 1 to 8 pick which layer goes instead of
// the newest (docs/adr/0026-layers.md).
const LAYER_KEYS: [(&str, u32); 8] = [
    ("Digit1", 1),
    ("Digit2", 2),
    ("Digit3", 3),
    ("Digit4", 4),
    ("Digit5", 5),
    ("Digit6", 6),
    ("Digit7", 7),
    ("Digit8", 8),
];

// The layer `key` picks while Esc waits, or None for a key that picks none.
pub fn layer_number_of(key: &str) -> Option<u32> {
    for (layer_key, number) in LAYER_KEYS {
        if layer_key == key {
            return Some(number);
        }
    }
    None
}

// What a key in the layout is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum KeyRole {
    // Plays this note.
    Note(u8),
    // Plays this chord.
    Chord(Chord),
    // Holds the vibrato key.
    Vibrato,
    // Holds the glide key.
    Glide,
    // Moves new notes on to the next wave.
    Wave,
    // Runs the looper.
    Looper,
    // Removes a layer of the loop.
    RemoveLayer,
    // Slows the beat down.
    Slower,
    // Speeds the beat up.
    Faster,
}

// What pressing a key does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyAction {
    // Plays this note until the key is let go.
    PlayNote(u8),
    // Plays this chord's notes together until the key is let go.
    PlayChord(Chord),
    // Holds the vibrato key until the key is let go.
    HoldVibratoKey,
    // Holds the glide key until the key is let go.
    HoldGlideKey,
    // Moves new notes on to the next wave.
    SwitchWave,
    // Counts in, records or closes the loop, or starts or stops a layer,
    // depending on what the looper is doing.
    PressLooper,
    // Removes the newest layer of the loop, or the one whose number comes
    // next (docs/adr/0026-layers.md).
    RemoveLayer,
    // Slows the beat down.
    SlowDown,
    // Speeds the beat up.
    SpeedUp,
}

// Cloned only for jello_listen's renders (src/listening.rs).
#[derive(Clone)]
pub struct KeyMap {
    octave: i8,
    // What each key does. A key that isn't in here does nothing.
    layout: HashMap<String, KeyRole>,
    // The keys held down right now, each with the notes it started.
    held: HashMap<String, Vec<u8>>,
}

// A chord as the UI draws it on its key.
#[derive(Serialize)]
pub struct ChordForUi {
    // Its name the way chord charts write it, like "Bb" or "F#m".
    name: String,
    // Its notes, lowest first.
    notes: [u8; 3],
}

// What each key in the layout does, for the UI to draw on the keys.
#[derive(Serialize)]
pub struct LayoutForUi {
    pub octave: i8,
    // Each key that plays one note, with its note: "KeyM" plays 60.
    notes: HashMap<String, u8>,
    // Each key that plays a chord, with its chord: "KeyX" plays Cm.
    chords: HashMap<String, ChordForUi>,
    // The keys that move new notes on to the next wave.
    wave_keys: Vec<String>,
    // The keys that hold the vibrato key.
    vibrato_keys: Vec<String>,
    // The keys that hold the glide key.
    glide_keys: Vec<String>,
    // The keys that run the beat and the looper, each with what it does:
    // "looper", "clear", "slower" or "faster".
    beat_keys: HashMap<String, &'static str>,
}

impl KeyMap {
    // The default layout, with no keys held.
    pub fn new() -> KeyMap {
        let mut layout = HashMap::new();
        for (key, chord) in CHORDS {
            layout.insert(String::from(key), KeyRole::Chord(chord));
        }
        for (key, note) in MELODY_NOTES {
            layout.insert(String::from(key), KeyRole::Note(note));
        }
        layout.insert(String::from(WAVE_KEY), KeyRole::Wave);
        layout.insert(String::from(VIBRATO_KEY), KeyRole::Vibrato);
        layout.insert(String::from(GLIDE_KEY), KeyRole::Glide);
        layout.insert(String::from(LOOPER_KEY), KeyRole::Looper);
        layout.insert(String::from(CLEAR_KEY), KeyRole::RemoveLayer);
        layout.insert(String::from(SLOWER_KEY), KeyRole::Slower);
        layout.insert(String::from(FASTER_KEY), KeyRole::Faster);

        KeyMap {
            octave: 0,
            layout,
            held: HashMap::new(),
        }
    }

    // Move the whole layout, preserving each held key's original notes.
    // Refuse a shift as a whole if it would leave the supported note range.
    pub fn shift_octave(&mut self, steps: i8) {
        let next = self.octave.saturating_add(steps);
        if !(-4..=4).contains(&next) {
            return;
        }
        let shift = i16::from(next - self.octave) * 12;
        for role in self.layout.values_mut() {
            match role {
                KeyRole::Note(note) => *note = (i16::from(*note) + shift) as u8,
                KeyRole::Chord(chord) => chord.root = (i16::from(chord.root) + shift) as u8,
                _ => {}
            }
        }
        self.octave = next;
    }

    // How many octaves the keys are shifted, from -4 to 4.
    pub fn octave(&self) -> i8 {
        self.octave
    }

    // Whether `key` does anything at all.
    pub fn knows(&self, key: &str) -> bool {
        self.layout.contains_key(key)
    }

    // What pressing `key` should do, or None for nothing.
    pub fn press(&mut self, key: &str) -> Option<KeyAction> {
        // A held key is already playing its notes. Doing nothing means one key
        // can never start them twice.
        if self.held.contains_key(key) {
            return None;
        }

        // A key that isn't in the layout does nothing.
        let role = match self.layout.get(key) {
            Some(role) => *role,
            None => return None,
        };

        // Remember the notes, so letting go of the key stops these ones. If
        // another held key is already playing one of them, the engine plays
        // it again on the voice it already has, so the note sounds once.
        match role {
            KeyRole::Note(note) => {
                self.held.insert(String::from(key), vec![note]);
                Some(KeyAction::PlayNote(note))
            }
            KeyRole::Chord(chord) => {
                self.held.insert(String::from(key), chord.notes().to_vec());
                Some(KeyAction::PlayChord(chord))
            }
            // Held with no notes, so letting go of it is known.
            KeyRole::Vibrato => {
                self.held.insert(String::from(key), Vec::new());
                Some(KeyAction::HoldVibratoKey)
            }
            // The same for the glide key.
            KeyRole::Glide => {
                self.held.insert(String::from(key), Vec::new());
                Some(KeyAction::HoldGlideKey)
            }
            KeyRole::Wave => Some(KeyAction::SwitchWave),
            KeyRole::Looper => Some(KeyAction::PressLooper),
            KeyRole::RemoveLayer => Some(KeyAction::RemoveLayer),
            KeyRole::Slower => Some(KeyAction::SlowDown),
            KeyRole::Faster => Some(KeyAction::SpeedUp),
        }
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

    // Whether `key` is held down holding the vibrato key, so letting go of
    // it lets go of the vibrato key. Ask before `release`.
    pub fn holds_vibrato_key(&self, key: &str) -> bool {
        if !self.held.contains_key(key) {
            return false;
        }
        self.layout.get(key) == Some(&KeyRole::Vibrato)
    }

    // Whether `key` is held down holding the glide key, so letting go of it
    // lets go of the glide key. Ask before `release`.
    pub fn holds_glide_key(&self, key: &str) -> bool {
        if !self.held.contains_key(key) {
            return false;
        }
        self.layout.get(key) == Some(&KeyRole::Glide)
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

    // Each chord key, with the chord it plays.
    pub fn chord_keys(&self) -> HashMap<String, Chord> {
        let mut chord_keys = HashMap::new();
        for (key, role) in &self.layout {
            if let KeyRole::Chord(chord) = role {
                chord_keys.insert(key.clone(), *chord);
            }
        }
        chord_keys
    }

    // Each melody key, with the note it plays.
    pub fn note_keys(&self) -> HashMap<String, u8> {
        let mut note_keys = HashMap::new();
        for (key, role) in &self.layout {
            if let KeyRole::Note(note) = role {
                note_keys.insert(key.clone(), *note);
            }
        }
        note_keys
    }

    // What every key in the layout does right now, for the UI to draw.
    pub fn layout_for_ui(&self) -> LayoutForUi {
        let mut notes = HashMap::new();
        let mut chords = HashMap::new();
        let mut wave_keys = Vec::new();
        let mut vibrato_keys = Vec::new();
        let mut glide_keys = Vec::new();
        let mut beat_keys = HashMap::new();
        for (key, role) in &self.layout {
            match role {
                KeyRole::Note(note) => {
                    notes.insert(key.clone(), *note);
                }
                KeyRole::Chord(chord) => {
                    chords.insert(key.clone(), chord_for_ui(*chord));
                }
                KeyRole::Vibrato => {
                    vibrato_keys.push(key.clone());
                }
                KeyRole::Glide => {
                    glide_keys.push(key.clone());
                }
                KeyRole::Wave => {
                    wave_keys.push(key.clone());
                }
                KeyRole::Looper => {
                    beat_keys.insert(key.clone(), "looper");
                }
                KeyRole::RemoveLayer => {
                    beat_keys.insert(key.clone(), "clear");
                }
                KeyRole::Slower => {
                    beat_keys.insert(key.clone(), "slower");
                }
                KeyRole::Faster => {
                    beat_keys.insert(key.clone(), "faster");
                }
            }
        }

        // The layout keeps its keys in no order, so sort them to give the
        // same answer every time.
        wave_keys.sort();
        vibrato_keys.sort();
        glide_keys.sort();

        LayoutForUi {
            octave: self.octave,
            notes,
            chords,
            wave_keys,
            vibrato_keys,
            glide_keys,
            beat_keys,
        }
    }
}

// With no song key to follow, chords get the names chord charts use most.
fn chord_for_ui(chord: Chord) -> ChordForUi {
    ChordForUi {
        name: chord_name(chord, None),
        notes: chord.notes(),
    }
}

// These tests only ask the key map what keys do. They make no sound.
#[cfg(test)]
mod tests {
    use super::*;

    // The notes a press played, if it played a chord.
    fn chord_notes(action: Option<KeyAction>) -> Option<[u8; 3]> {
        match action {
            Some(KeyAction::PlayChord(chord)) => Some(chord.notes()),
            _ => None,
        }
    }

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

    // Presses each key on a fresh key map, and checks it plays its chord's
    // notes, given as (key, notes).
    fn assert_chords(chords: &[(&str, [u8; 3])]) {
        for (key, notes) in chords {
            let mut key_map = KeyMap::new();
            assert_eq!(chord_notes(key_map.press(key)), Some(*notes), "key {key}");
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
        assert_chords(&[
            ("KeyC", [48, 52, 55]), // C3 E3 G3
            ("KeyD", [50, 54, 57]), // D3 F#3 A3
            ("KeyE", [52, 56, 59]), // E3 G#3 B3
            ("KeyF", [53, 57, 60]), // F3 A3 C4
            ("KeyG", [55, 59, 62]), // G3 B3 D4
            ("KeyA", [57, 61, 64]), // A3 C#4 E4
            ("KeyB", [59, 63, 66]), // B3 D#4 F#4
        ]);
    }

    #[test]
    fn each_minor_chord_sits_next_to_its_major() {
        // The middle note is a half step lower than in the major chord.
        assert_chords(&[
            ("KeyX", [48, 51, 55]), // Cm: C3 Eb3 G3, left of C
            ("KeyS", [50, 53, 57]), // Dm: D3 F3 A3, left of D
            ("KeyW", [52, 55, 59]), // Em: E3 G3 B3, left of E
            ("KeyR", [53, 56, 60]), // Fm: F3 Ab3 C4, above F
            ("KeyT", [55, 58, 62]), // Gm: G3 Bb3 D4, above G
            ("KeyQ", [57, 60, 64]), // Am: A3 C4 E4, above A
            ("KeyV", [59, 62, 66]), // Bm: B3 D4 F#4, left of B
        ]);
    }

    #[test]
    fn the_number_row_plays_the_black_key_majors_low_to_high() {
        assert_chords(&[
            ("Digit1", [49, 53, 56]), // C#3 F3 G#3
            ("Digit2", [51, 55, 58]), // Eb3 G3 Bb3
            ("Digit3", [54, 58, 61]), // F#3 A#3 C#4
            ("Digit4", [56, 60, 63]), // Ab3 C4 Eb4
            ("Digit5", [58, 62, 65]), // Bb3 D4 F4
        ]);
    }

    #[test]
    fn the_left_edge_plays_the_black_key_minors_low_to_high() {
        assert_chords(&[
            ("Backquote", [49, 52, 56]), // C#m: C#3 E3 G#3
            ("Tab", [51, 54, 58]),         // Ebm: Eb3 Gb3 Bb3
            ("ControlLeft", [54, 57, 61]), // F#m: F#3 A3 C#4
            ("KeyZ", [56, 59, 63]),        // G#m: G#3 B3 D#4
            ("Space", [58, 61, 65]),       // Bbm: Bb3 Db4 F4
        ]);
    }

    #[test]
    fn all_24_chords_are_playable_each_on_one_key() {
        let chord_keys = KeyMap::new().chord_keys();
        assert_eq!(chord_keys.len(), 24);

        // Every root from C3 to B3, once as a major chord and once as a
        // minor chord.
        for root in 48..60 {
            for quality in [Quality::Major, Quality::Minor] {
                let chord = Chord { root, quality };
                let mut keys = Vec::new();
                for (key, played) in &chord_keys {
                    if *played == chord {
                        keys.push(key.clone());
                    }
                }
                assert_eq!(keys.len(), 1, "{chord:?} is on {keys:?}");
            }
        }
    }

    #[test]
    fn holding_a_chord_key_changes_no_other_key() {
        // Space and left Control play chords of their own now, and left
        // Shift holds the vibrato key, so holding them leaves C a C chord
        // and M middle C.
        let mut key_map = KeyMap::new();
        key_map.press("Space");
        key_map.press("ControlLeft");
        key_map.press("ShiftLeft");
        assert_eq!(chord_notes(key_map.press("KeyC")), Some([48, 52, 55]));
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
    }

    #[test]
    fn left_shift_holds_the_vibrato_key_until_it_is_let_go() {
        let mut key_map = KeyMap::new();
        assert!(!key_map.holds_vibrato_key("ShiftLeft"));
        assert_eq!(key_map.press("ShiftLeft"), Some(KeyAction::HoldVibratoKey));
        assert!(key_map.holds_vibrato_key("ShiftLeft"));

        // Held, it is not held again, and it starts no notes to stop.
        assert_eq!(key_map.press("ShiftLeft"), None);
        assert_eq!(key_map.release("ShiftLeft"), Vec::<u8>::new());
        assert!(!key_map.holds_vibrato_key("ShiftLeft"));

        // No other key holds it, held or not.
        key_map.press("KeyC");
        assert!(!key_map.holds_vibrato_key("KeyC"));
        assert!(!key_map.holds_vibrato_key("ShiftRight"));
    }

    #[test]
    fn left_option_holds_the_glide_key_until_it_is_let_go() {
        let mut key_map = KeyMap::new();
        assert!(!key_map.holds_glide_key(GLIDE_KEY));
        assert_eq!(key_map.press(GLIDE_KEY), Some(KeyAction::HoldGlideKey));
        assert!(key_map.holds_glide_key(GLIDE_KEY));

        // Held, it is not held again, and it starts no notes to stop.
        assert_eq!(key_map.press(GLIDE_KEY), None);
        assert_eq!(key_map.release(GLIDE_KEY), Vec::<u8>::new());
        assert!(!key_map.holds_glide_key(GLIDE_KEY));

        // No other key holds it, held or not: not right Option, and not the
        // vibrato key.
        key_map.press("ShiftLeft");
        assert!(!key_map.holds_glide_key("ShiftLeft"));
        assert!(!key_map.holds_vibrato_key(GLIDE_KEY));

    }

    #[test]
    fn f12_switches_the_wave() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("F12"), Some(KeyAction::SwitchWave));
    }

    #[test]
    fn right_shift_runs_the_looper_and_esc_removes_a_layer() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("ShiftRight"), Some(KeyAction::PressLooper));
        assert_eq!(key_map.press("Escape"), Some(KeyAction::RemoveLayer));
    }

    #[test]
    fn minus_and_equal_slow_the_beat_down_and_speed_it_up() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_map.press("Minus"), Some(KeyAction::SlowDown));
        assert_eq!(key_map.press("Equal"), Some(KeyAction::SpeedUp));
    }

    #[test]
    fn keys_outside_the_layout_do_nothing() {
        // The keys past the right pinky, and the keys every tab keeps for
        // itself.
        let mut key_map = KeyMap::new();
        let keys = [
            "CapsLock",
            "Enter",
            "Backspace",
            "ControlRight",
            "MetaLeft",
        ];
        for key in keys {
            assert_eq!(key_map.press(key), None, "key {key}");
        }

        // The layout holds the 24 chords, the 24 notes, the vibrato key, the
        // glide key, the wave key and the 4 beat and looper keys, and
        // nothing else.
        assert_eq!(key_map.layout.len(), 55);
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
        assert_eq!(chord_notes(key_map.press("KeyC")), Some([48, 52, 55]));
        assert_eq!(key_map.press("KeyC"), None);
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
        assert_eq!(chord_notes(key_map.press("KeyC")), Some([48, 52, 55]));
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
        assert_eq!(chord_notes(key_map.press("KeyA")), Some([57, 61, 64]));
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
    fn a_chord_and_its_minor_share_notes_until_both_are_let_go() {
        // C (C3 E3 G3) and Cm (C3 Eb3 G3), on C and X side by side, both play
        // C3 and G3.
        let mut key_map = KeyMap::new();
        key_map.press("KeyC");
        key_map.press("KeyX");
        assert_eq!(key_map.release("KeyC"), vec![52]);
        assert_eq!(key_map.release("KeyX"), vec![48, 51, 55]);
    }

    #[test]
    fn letting_go_of_a_key_that_started_no_note_does_nothing() {
        let mut key_map = KeyMap::new();
        let nothing: Vec<u8> = Vec::new();

        // Never pressed.
        assert_eq!(key_map.release("KeyM"), nothing);

        // Pressed, but it plays no note.
        key_map.press("F12");
        assert_eq!(key_map.release("F12"), nothing);
        key_map.press("ShiftRight");
        assert_eq!(key_map.release("ShiftRight"), nothing);

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
        key_map.layout.insert(String::from("KeyM"), KeyRole::Note(62));
        assert_eq!(key_map.release("KeyM"), vec![60]);

        // The next press plays the new note.
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(62)));

        // The same goes for a chord: hold C, then move C to the D chord.
        key_map.press("KeyC");
        key_map.layout.insert(String::from("KeyC"), KeyRole::Chord(major(50)));
        assert_eq!(key_map.release("KeyC"), vec![48, 52, 55]);
    }

    #[test]
    fn letting_go_of_everything_forgets_every_held_key() {
        let mut key_map = KeyMap::new();
        key_map.press("KeyM");
        key_map.press("KeyC");
        key_map.press("Space");
        key_map.release_all();

        // Nothing is left to let go of, and the keys play again.
        let nothing: Vec<u8> = Vec::new();
        assert_eq!(key_map.release("KeyM"), nothing);
        assert_eq!(key_map.release("KeyC"), nothing);
        assert_eq!(key_map.release("Space"), nothing);
        assert_eq!(key_map.press("KeyM"), Some(KeyAction::PlayNote(60)));
        assert_eq!(chord_notes(key_map.press("KeyC")), Some([48, 52, 55]));
        assert_eq!(chord_notes(key_map.press("Space")), Some([58, 61, 65]));
    }

    #[test]
    fn the_chord_keys_list_what_each_press_plays() {
        // Compose shows guidance for each of these, so each must be what
        // the key really plays.
        let chord_keys = KeyMap::new().chord_keys();
        for (key, chord) in chord_keys {
            let mut key_map = KeyMap::new();
            assert_eq!(key_map.press(&key), Some(KeyAction::PlayChord(chord)), "key {key}");
        }
    }

    #[test]
    fn the_note_keys_list_what_each_press_plays() {
        // Compose shows guidance for each of these too.
        let note_keys = KeyMap::new().note_keys();
        assert_eq!(note_keys.len(), 24);
        for (key, note) in note_keys {
            let mut key_map = KeyMap::new();
            assert_eq!(key_map.press(&key), Some(KeyAction::PlayNote(note)), "key {key}");
        }
    }

    #[test]
    fn the_ui_is_told_every_note_chord_wave_and_beat_key() {
        let key_map = KeyMap::new();
        let layout = key_map.layout_for_ui();

        assert_eq!(layout.notes.len(), 24);
        for (key, note) in MELODY_NOTES {
            assert_eq!(layout.notes.get(key), Some(&note), "key {key}");
        }
        assert_eq!(layout.chords.len(), 24);
        for (key, chord) in CHORDS {
            assert_eq!(layout.chords[key].notes, chord.notes(), "key {key}");
        }
        assert_eq!(layout.wave_keys, vec![String::from("F12")]);
        assert_eq!(layout.beat_keys.len(), 4);
        assert_eq!(layout.beat_keys["ShiftRight"], "looper");
        assert_eq!(layout.beat_keys["Escape"], "clear");
        assert_eq!(layout.beat_keys["Minus"], "slower");
        assert_eq!(layout.beat_keys["Equal"], "faster");
    }

    #[test]
    fn the_ui_is_told_each_chord_by_its_common_name() {
        let chords = KeyMap::new().layout_for_ui().chords;
        let names = [
            ("KeyC", "C"),
            ("KeyX", "Cm"),
            ("KeyQ", "Am"),
            ("Digit1", "C#"),
            ("Digit2", "Eb"),
            ("Digit3", "F#"),
            ("Digit4", "Ab"),
            ("Digit5", "Bb"),
            ("Backquote", "C#m"),
            ("Tab", "Ebm"),
            ("ControlLeft", "F#m"),
            ("KeyZ", "G#m"),
            ("Space", "Bbm"),
        ];
        for (key, name) in names {
            assert_eq!(chords[key].name, name, "key {key}");
        }
    }

    #[test]
    fn the_ui_is_told_the_layout_as_it_is_now() {
        // Move M to D4, and the UI is told M plays D4.
        let mut key_map = KeyMap::new();
        key_map.layout.insert(String::from("KeyM"), KeyRole::Note(62));

        let layout = key_map.layout_for_ui();
        assert_eq!(layout.notes.get("KeyM"), Some(&62));
    }

    #[test]
    fn the_ui_reads_the_layout_by_these_names() {
        // The UI reads these names (KeyLayout in ui/keyboard.ts), so
        // renaming them here would leave keys blank.
        let key_map = KeyMap::new();
        let json = serde_json::to_value(key_map.layout_for_ui()).unwrap();
        assert_eq!(json["notes"]["KeyM"], 60);
        assert_eq!(json["chords"]["KeyC"]["name"], "C");
        assert_eq!(json["chords"]["KeyC"]["notes"][0], 48);
        assert_eq!(json["chords"]["KeyC"]["notes"][2], 55);
        assert_eq!(json["chords"]["KeyX"]["name"], "Cm");
        assert_eq!(json["wave_keys"][0], "F12");
        assert_eq!(json["vibrato_keys"][0], "ShiftLeft");
        assert_eq!(json["glide_keys"][0], GLIDE_KEY);
        assert_eq!(json["beat_keys"]["ShiftRight"], "looper");
    }
    #[test]
    fn shifting_changes_all_future_notes_but_releases_original_pitches() {
        let mut map = KeyMap::new();
        map.press("KeyM");
        map.press("KeyC");
        map.shift_octave(2);
        assert_eq!(map.layout_for_ui().notes["KeyM"], 84);
        assert_eq!(map.layout_for_ui().chords["KeyC"].notes, [72, 76, 79]);
        // C4 is shared with neither C3 chord note, so both releases keep their pitches.
        assert_eq!(map.release("KeyM"), vec![60]);
        assert_eq!(map.release("KeyC"), vec![48, 52, 55]);
        assert_eq!(map.press("KeyM"), Some(KeyAction::PlayNote(84)));
    }

    #[test]
    fn octave_limits_keep_the_complete_mapping_in_midi_range() {
        let mut map = KeyMap::new();
        map.shift_octave(-4);
        assert_eq!(map.layout_for_ui().octave, -4);
        assert_eq!(map.layout_for_ui().chords["KeyC"].notes[0], 0);
        map.shift_octave(-1);
        assert_eq!(map.layout_for_ui().octave, -4);
        map.shift_octave(8);
        assert_eq!(map.layout_for_ui().notes["Backslash"], 127);
        map.shift_octave(1);
        assert_eq!(map.layout_for_ui().octave, 4);
    }

}
