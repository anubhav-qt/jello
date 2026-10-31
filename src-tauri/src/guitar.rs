// The keyboard guitar (docs/adr/0049-the-keyboard-guitar.md): the right hand's
// U I O P [ ] are the six strings, ] the thinnest, and the left hand's twenty
// keys from 1QAZ to 5TGB each hold a shape, which says the note every string
// plays. With no shape held, the strings play open.
//
// Like the rest of a song's keys (src/key_mapping.rs), nothing stays fixed:
// each scale has a default, and a song or a section can change any shape.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::engine::{Amp, Kind, LOUDNESS_SHORTEST_MS, LoudnessEnvelope, Oscillator, StringSound};
use crate::harmony::{Chord, Quality, chord_name};
use crate::key_map::name_of_notes;
use crate::presets;
use crate::scales::Scale;
use crate::sound_file::Sound;
use crate::sound_profile::SoundProfile;

pub const STRING_COUNT: usize = 6;

// The six strings, thickest first.
pub const STRING_KEYS: [&str; STRING_COUNT] = ["KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight"];

// The left hand's keys, by row then column, 1QAZ on the left.
const ROW_COUNT: usize = 4;
const COLUMN_COUNT: usize = 5;
const SHAPE_ROWS: [[&str; COLUMN_COUNT]; ROW_COUNT] = [
    ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5"],
    ["KeyQ", "KeyW", "KeyE", "KeyR", "KeyT"],
    ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG"],
    ["KeyZ", "KeyX", "KeyC", "KeyV", "KeyB"],
];

// The columns best chord first: index finger, middle, ring, little, then the
// index finger's reach, the same order as the piano's chord keys.
const COLUMNS_BY_ROLE: [usize; COLUMN_COUNT] = [3, 2, 1, 0, 4];

// A guitar in standard tuning: E2 A2 D3 G3 B3 E4.
const STANDARD_TUNING: [u8; STRING_COUNT] = [40, 45, 50, 55, 59, 64];
const LOW_E: usize = 0;
const A_STRING: usize = 1;

// How much softer and darker a palm-muted pluck is than a free one.
pub const MUTED_VELOCITY: u8 = 60;

// Barre shapes, frets above the root's, thickest string first. None is a
// string the shape leaves silent. Each chord has one with its root on the low
// E and one on the A.
type Frets = [Option<u8>; STRING_COUNT];
const fn on_low_e(frets: [u8; STRING_COUNT]) -> Frets {
    [
        Some(frets[0]),
        Some(frets[1]),
        Some(frets[2]),
        Some(frets[3]),
        Some(frets[4]),
        Some(frets[5]),
    ]
}
const fn on_a(frets: [u8; STRING_COUNT - 1]) -> Frets {
    [
        None,
        Some(frets[0]),
        Some(frets[1]),
        Some(frets[2]),
        Some(frets[3]),
        Some(frets[4]),
    ]
}

const MAJOR_ON_E: Frets = on_low_e([0, 2, 2, 1, 0, 0]);
const MAJOR_ON_A: Frets = on_a([0, 2, 2, 2, 0]);
const MINOR_ON_E: Frets = on_low_e([0, 2, 2, 0, 0, 0]);
const MINOR_ON_A: Frets = on_a([0, 2, 2, 1, 0]);
const SEVENTH_ON_E: Frets = on_low_e([0, 2, 0, 1, 0, 0]);
const SEVENTH_ON_A: Frets = on_a([0, 2, 0, 2, 0]);
const MINOR_SEVENTH_ON_E: Frets = on_low_e([0, 2, 0, 0, 0, 0]);
const MINOR_SEVENTH_ON_A: Frets = on_a([0, 2, 0, 1, 0]);

pub fn is_shape_key(key: &str) -> bool {
    shape_keys().contains(&key)
}

pub fn shape_keys() -> Vec<&'static str> {
    let mut keys = Vec::new();
    for row in SHAPE_ROWS {
        keys.extend(row);
    }
    keys
}

// One left-hand key: what to call it and each string's note, or None for a
// string it leaves silent.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct GuitarShape {
    pub name: String,
    pub strings: [Option<u8>; STRING_COUNT],
}

// What the guitar plays: its open strings and every left-hand key's shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct GuitarMapping {
    pub open: [Option<u8>; STRING_COUNT],
    pub shapes: BTreeMap<String, GuitarShape>,
}

impl GuitarMapping {
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.open.iter().all(Option::is_none)
    }

    // `scale`'s default. The strings tune down to its notes, so every open
    // string and the top row stay in the scale. The top row walks every
    // string up the scale, a step for each column, for melodies. The other
    // rows are the scale's chords by role: major, minor and sevenths.
    pub fn default_for(scale: Scale) -> GuitarMapping {
        let mut mapping = GuitarMapping::default();
        let notes = core_notes(scale);
        for string in 0..STRING_COUNT {
            mapping.open[string] = Some(at_or_below(&notes, STANDARD_TUNING[string]));
        }
        mapping.place_melody_row(&notes);

        let chords = scale.kind.chords();
        let mut majors = Vec::new();
        let mut minors = Vec::new();
        for (step, quality) in &chords {
            let pitch = (scale.tonic + step) % 12;
            match quality {
                Quality::Major => majors.push((pitch, *quality)),
                Quality::Minor => minors.push((pitch, *quality)),
            }
        }
        let mut best = Vec::new();
        for (step, quality) in &chords {
            best.push(((scale.tonic + step) % 12, *quality));
        }
        mapping.place_chord_row(1, &majors, false);
        mapping.place_chord_row(2, &minors, false);
        mapping.place_chord_row(3, &best, true);
        mapping
    }

    fn place_melody_row(&mut self, notes: &[u8]) {
        for (column, key) in SHAPE_ROWS[0].iter().enumerate() {
            let steps = column + 1;
            let mut strings = [None; STRING_COUNT];
            for (string, played) in strings.iter_mut().enumerate() {
                if let Some(open) = self.open[string] {
                    *played = Some(steps_above(notes, open, steps));
                }
            }
            let shape = GuitarShape {
                name: format!("+{steps}"),
                strings,
            };
            self.shapes.insert(String::from(*key), shape);
        }
    }

    // A row of `chords`, best under the index finger. A scale with too few
    // chords repeats its best ones with their root on the other string.
    fn place_chord_row(&mut self, row: usize, chords: &[(u8, Quality)], seventh: bool) {
        if chords.is_empty() {
            return;
        }
        for (place, column) in COLUMNS_BY_ROLE.iter().enumerate() {
            let (pitch, quality) = chords[place % chords.len()];
            let other_string = place >= chords.len();
            let shape = chord_shape(pitch, quality, seventh, other_string);
            self.shapes.insert(String::from(SHAPE_ROWS[row][*column]), shape);
        }
    }

    // Puts `strings` on `key`, named `name`. Refuses a key that holds no shape
    // and a note past 127.
    pub fn set_shape(&mut self, key: &str, name: &str, strings: [Option<u8>; STRING_COUNT]) -> Result<(), String> {
        if !is_shape_key(key) {
            return Err(format!("{key} can't hold a guitar shape: only the keys from 1QAZ to 5TGB can"));
        }
        check_notes(&strings)?;
        let shape = GuitarShape {
            name: name_or_notes(name, &strings),
            strings,
        };
        self.shapes.insert(String::from(key), shape);
        Ok(())
    }

    // Tunes the open strings.
    pub fn set_open(&mut self, strings: [Option<u8>; STRING_COUNT]) -> Result<(), String> {
        check_notes(&strings)?;
        self.open = strings;
        Ok(())
    }

    // Whether every note stays from 0 to 127, moved by `shift`.
    pub fn fits_when_shifted(&self, shift: i16) -> bool {
        if !fits(&self.open, shift) {
            return false;
        }
        for shape in self.shapes.values() {
            if !fits(&shape.strings, shift) {
                return false;
            }
        }
        true
    }
}

// `name`, or with none given, what the strings' notes spell.
fn name_or_notes(name: &str, strings: &[Option<u8>; STRING_COUNT]) -> String {
    if !name.trim().is_empty() {
        return String::from(name);
    }
    let mut notes: Vec<u8> = strings.iter().flatten().copied().collect();
    notes.sort();
    notes.dedup();
    if notes.is_empty() {
        return String::from("silent");
    }
    name_of_notes(&notes)
}

fn check_notes(strings: &[Option<u8>; STRING_COUNT]) -> Result<(), String> {
    for note in strings.iter().flatten() {
        if *note > 127 {
            return Err(format!("{note} is not a note: they go from 0 to 127"));
        }
    }
    Ok(())
}

fn fits(strings: &[Option<u8>; STRING_COUNT], shift: i16) -> bool {
    for note in strings.iter().flatten() {
        let moved = i16::from(*note) + shift;
        if !(0..=127).contains(&moved) {
            return false;
        }
    }
    true
}

// Every note of `scale`'s core, lowest first.
fn core_notes(scale: Scale) -> Vec<u8> {
    let mut pitches = Vec::new();
    for step in scale.kind.shape().core {
        pitches.push((i16::from(scale.tonic) + *step).rem_euclid(12) as u8);
    }
    let mut notes = Vec::new();
    for note in 0..=127u8 {
        if pitches.contains(&(note % 12)) {
            notes.push(note);
        }
    }
    notes
}

fn at_or_below(notes: &[u8], note: u8) -> u8 {
    let mut found = note;
    for candidate in notes {
        if *candidate > note {
            break;
        }
        found = *candidate;
    }
    found
}

fn steps_above(notes: &[u8], note: u8, steps: usize) -> u8 {
    let mut place = 0;
    for (index, candidate) in notes.iter().enumerate() {
        if *candidate == note {
            place = index;
            break;
        }
    }
    let last = notes.len() - 1;
    notes[(place + steps).min(last)]
}

fn chord_shape(pitch: u8, quality: Quality, seventh: bool, other_string: bool) -> GuitarShape {
    let e_fret = (i16::from(pitch) - i16::from(STANDARD_TUNING[LOW_E])).rem_euclid(12) as u8;
    let a_fret = (i16::from(pitch) - i16::from(STANDARD_TUNING[A_STRING])).rem_euclid(12) as u8;
    // Its root on whichever string plays it lowest on the neck.
    let mut on_e = e_fret <= a_fret;
    if other_string {
        on_e = !on_e;
    }
    let frets = frets_of(quality, seventh, on_e);
    let root_fret = if on_e { e_fret } else { a_fret };
    let mut strings = [None; STRING_COUNT];
    for (string, played) in strings.iter_mut().enumerate() {
        if let Some(fret) = frets[string] {
            *played = Some(STANDARD_TUNING[string] + root_fret + fret);
        }
    }
    let chord = Chord {
        root: 48 + pitch,
        quality,
    };
    let mut name = chord_name(chord, None);
    if seventh {
        name.push('7');
    }
    GuitarShape { name, strings }
}

fn frets_of(quality: Quality, seventh: bool, on_e: bool) -> Frets {
    match (quality, seventh, on_e) {
        (Quality::Major, false, true) => MAJOR_ON_E,
        (Quality::Major, false, false) => MAJOR_ON_A,
        (Quality::Minor, false, true) => MINOR_ON_E,
        (Quality::Minor, false, false) => MINOR_ON_A,
        (Quality::Major, true, true) => SEVENTH_ON_E,
        (Quality::Major, true, false) => SEVENTH_ON_A,
        (Quality::Minor, true, true) => MINOR_SEVENTH_ON_E,
        (Quality::Minor, true, false) => MINOR_SEVENTH_ON_A,
    }
}

// The presets the guitars come with: only places to start from, every value
// the song's own to change once loaded. A note rings until its string is played
// again or damped, so the sound never holds it up itself.
const ACOUSTIC_PRESET: &str = "Acoustic Guitar";
const GUITAR_RELEASE_MS: u32 = 90;

const ACOUSTIC_STRING: StringSound = StringSound {
    decay_ms: 5000,
    tone: 62,
    pick: 16,
    pickup: 0,
    pickup_tone: 60,
    body: 80,
};

// One electric guitar through one amp, with every knob set its own way: which
// pickup, how dark its coil, how hard the amp is pushed and what the speaker
// keeps. Near the neck is round and near the bridge thin and biting.
struct Electric {
    name: &'static str,
    decay_ms: u32,
    tone: u32,
    pick: u32,
    pickup: u32,
    pickup_tone: u32,
    amp: Amp,
}

const ELECTRICS: [Electric; 4] = [
    Electric {
        name: "Electric Clean",
        decay_ms: 8000,
        tone: 70,
        pick: 20,
        pickup: 38,
        pickup_tone: 75,
        amp: Amp { on: true, gain: 5, bass: 55, middle: 45, treble: 62, presence: 60, speaker: 90, volume: 62 },
    },
    Electric {
        name: "Electric Crunch",
        decay_ms: 9000,
        tone: 82,
        pick: 10,
        pickup: 9,
        pickup_tone: 70,
        amp: Amp { on: true, gain: 48, bass: 42, middle: 68, treble: 60, presence: 62, speaker: 58, volume: 50 },
    },
    Electric {
        name: "Electric Lead",
        decay_ms: 20000,
        tone: 80,
        pick: 14,
        pickup: 12,
        pickup_tone: 42,
        amp: Amp { on: true, gain: 80, bass: 45, middle: 75, treble: 50, presence: 58, speaker: 45, volume: 68 },
    },
    Electric {
        name: "Electric Metal",
        decay_ms: 12000,
        tone: 85,
        pick: 8,
        pickup: 8,
        pickup_tone: 50,
        amp: Amp { on: true, gain: 100, bass: 68, middle: 22, treble: 68, presence: 66, speaker: 50, volume: 100 },
    },
];

// Saves every preset in `folder`, unless the player already has one called that.
pub fn save_presets(folder: &Path) -> Result<(), String> {
    save_preset(folder, ACOUSTIC_PRESET, guitar_sound(ACOUSTIC_STRING))?;
    for electric in &ELECTRICS {
        let string = StringSound {
            decay_ms: electric.decay_ms,
            tone: electric.tone,
            pick: electric.pick,
            pickup: electric.pickup,
            pickup_tone: electric.pickup_tone,
            body: 0,
        };
        let mut sound = guitar_sound(string);
        sound.amp = electric.amp;
        save_preset(folder, electric.name, sound)?;
    }
    Ok(())
}

fn save_preset(folder: &Path, name: &str, sound: Sound) -> Result<(), String> {
    for preset in presets::list(folder, None) {
        if preset.name == name {
            return Ok(());
        }
    }
    presets::save(folder, name, &sound)?;
    Ok(())
}

fn guitar_sound(string: StringSound) -> Sound {
    let mut sound = SoundProfile::new().sound();
    sound.oscillators[0] = Oscillator {
        kind: Kind::String,
        string,
        ..sound.oscillators[0]
    };
    sound.loudness_envelope = LoudnessEnvelope {
        attack_ms: LOUDNESS_SHORTEST_MS,
        decay_ms: LOUDNESS_SHORTEST_MS,
        sustain: 100,
        release_ms: GUITAR_RELEASE_MS,
    };
    sound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::presets::test_folder;
    use crate::scales::{SCALE_KINDS, ScaleKind};

    fn c_major() -> GuitarMapping {
        GuitarMapping::default_for(Scale {
            tonic: 0,
            kind: ScaleKind::Major,
        })
    }

    #[test]
    fn the_strings_tune_down_to_the_scale() {
        // Standard tuning is already in C major.
        assert_eq!(c_major().open, STANDARD_TUNING.map(Some));
        let e_flat = GuitarMapping::default_for(Scale::of_name("Eb major").unwrap());
        // E2 is outside Eb major, so it drops to Eb2.
        assert_eq!(e_flat.open[LOW_E], Some(39));
    }

    #[test]
    fn the_top_row_walks_every_string_up_the_scale() {
        let mapping = c_major();
        // Low E: F, G, A, B, C.
        let mut lowest = Vec::new();
        for key in SHAPE_ROWS[0] {
            lowest.push(mapping.shapes[key].strings[LOW_E].unwrap());
        }
        assert_eq!(lowest, vec![41, 43, 45, 47, 48]);
    }

    #[test]
    fn c_major_keeps_its_best_chords_under_the_index_finger() {
        let mapping = c_major();
        // Second row is major, third minor, fourth sevenths.
        assert_eq!(mapping.shapes["KeyR"].name, "C");
        assert_eq!(mapping.shapes["KeyE"].name, "G");
        assert_eq!(mapping.shapes["KeyF"].name, "Am");
        assert_eq!(mapping.shapes["KeyV"].name, "C7");
        // C on the A string: x 3 5 5 5 3.
        assert_eq!(
            mapping.shapes["KeyR"].strings,
            [None, Some(48), Some(55), Some(60), Some(64), Some(67)]
        );
        // G on the low E: 3 5 5 4 3 3.
        assert_eq!(
            mapping.shapes["KeyE"].strings,
            [Some(43), Some(50), Some(55), Some(59), Some(62), Some(67)]
        );
    }

    #[test]
    fn every_scale_on_every_home_note_fills_its_keys_in_range() {
        for kind in SCALE_KINDS {
            for tonic in 0..12 {
                let scale = Scale { tonic, kind };
                let mapping = GuitarMapping::default_for(scale);
                for key in SHAPE_ROWS[0] {
                    let shape = &mapping.shapes[key];
                    for note in shape.strings.iter().flatten() {
                        assert!(scale.holds(*note), "{} {key} plays {note}", scale.name());
                    }
                }
                for note in mapping.open.iter().flatten() {
                    assert!(scale.holds(*note), "{} plays {note} open", scale.name());
                }
                assert!(mapping.fits_when_shifted(0));
                assert!(mapping.shapes.len() >= COLUMN_COUNT, "{}", scale.name());
            }
        }
    }

    #[test]
    fn a_shape_can_be_changed_but_only_on_its_keys() {
        let mut mapping = c_major();
        let power = [Some(40), Some(47), Some(52), None, None, None];
        mapping.set_shape("KeyF", "E5", power).unwrap();
        assert_eq!(mapping.shapes["KeyF"].name, "E5");
        assert!(mapping.set_shape("KeyU", "E5", power).is_err());
        assert!(mapping.set_shape("KeyF", "E5", [Some(128), None, None, None, None, None]).is_err());
        assert!(mapping.set_open([Some(200), None, None, None, None, None]).is_err());
        mapping.set_open([Some(38), None, None, None, None, None]).unwrap();
        assert_eq!(mapping.open[LOW_E], Some(38));
    }

    #[test]
    fn every_preset_is_a_string_and_never_replaces_the_player_s() {
        let folder = test_folder("guitars");
        save_presets(&folder).unwrap();
        let new_song = SoundProfile::new();
        let acoustic = presets::read(&folder, ACOUSTIC_PRESET, &new_song).unwrap();
        assert_eq!(acoustic.oscillators[0].kind, Kind::String);
        assert_eq!(acoustic.oscillators[0].string, ACOUSTIC_STRING);
        assert!(!acoustic.amp.on);
        for electric in &ELECTRICS {
            let saved = presets::read(&folder, electric.name, &new_song).unwrap();
            assert_eq!(saved.oscillators[0].kind, Kind::String);
            assert_eq!(saved.oscillators[0].string.pickup, electric.pickup);
            assert_eq!(saved.oscillators[0].string.pickup_tone, electric.pickup_tone);
            assert_eq!(saved.oscillators[0].string.body, 0);
            assert_eq!(saved.amp, electric.amp);
        }

        let mut changed = guitar_sound(ACOUSTIC_STRING);
        changed.oscillators[0].level = 50;
        presets::save(&folder, ACOUSTIC_PRESET, &changed).unwrap();
        save_presets(&folder).unwrap();
        let kept = presets::read(&folder, ACOUSTIC_PRESET, &new_song).unwrap();
        assert_eq!(kept.oscillators[0].level, 50);
    }

    // An open A chord through `sound`, for a second and a half.
    fn chord_through(sound: &Sound) -> Vec<f32> {
        let mut engine = Engine::new().unwrap();
        engine.set_oscillator(0, sound.oscillators[0]);
        engine.set_oscillator(1, sound.oscillators[1]);
        engine.set_oscillator(2, sound.oscillators[2]);
        engine.set_loudness_envelope(sound.loudness_envelope);
        engine.set_amp(sound.amp);
        for note in [45, 52, 57, 60, 64] {
            engine.note_on(note, 100);
        }
        let stereo = engine.render(72_000);
        let mut left = Vec::new();
        for frame in stereo.chunks(2) {
            left.push(frame[0]);
        }
        left
    }

    fn rms_of(samples: &[f32]) -> f32 {
        let mut sum = 0.0;
        for sample in samples {
            sum += sample * sample;
        }
        (sum / samples.len() as f32).sqrt()
    }

    // How much of the sound is its fast movement: higher is brighter.
    fn brightness_of(samples: &[f32]) -> f32 {
        let mut changes = Vec::new();
        for pair in samples.windows(2) {
            changes.push(pair[1] - pair[0]);
        }
        rms_of(&changes) / rms_of(samples)
    }

    // How far the wave's peak stands over its body: lower is more squashed.
    fn crest_of(samples: &[f32]) -> f32 {
        let mut peak = 0.0f32;
        for sample in samples {
            peak = peak.max(sample.abs());
        }
        peak / rms_of(samples)
    }

    fn electric_sound(electric: &Electric) -> Sound {
        let string = StringSound {
            decay_ms: electric.decay_ms,
            tone: electric.tone,
            pick: electric.pick,
            pickup: electric.pickup,
            pickup_tone: electric.pickup_tone,
            body: 0,
        };
        let mut sound = guitar_sound(string);
        sound.amp = electric.amp;
        sound
    }

    #[test]
    fn every_electric_sounds_its_own_way_and_about_as_loud_as_the_acoustic() {
        let acoustic = chord_through(&guitar_sound(ACOUSTIC_STRING));
        let acoustic_level = rms_of(&acoustic);
        println!(
            "acoustic: rms {acoustic_level}, brightness {}, crest {}",
            brightness_of(&acoustic),
            crest_of(&acoustic)
        );
        let mut heard = vec![(brightness_of(&acoustic), crest_of(&acoustic))];
        for electric in &ELECTRICS {
            let samples = chord_through(&electric_sound(electric));
            let level = rms_of(&samples);
            println!(
                "{}: rms {level}, brightness {}, crest {}",
                electric.name,
                brightness_of(&samples),
                crest_of(&samples)
            );
            assert!(samples.iter().all(|sample| sample.is_finite()));
            let ratio = level / acoustic_level;
            assert!((0.5..2.0).contains(&ratio), "{} is {ratio} times the acoustic", electric.name);
            heard.push((brightness_of(&samples), crest_of(&samples)));
        }
        for (first, a) in heard.iter().enumerate() {
            for b in &heard[first + 1..] {
                let brighter = (a.0 / b.0).max(b.0 / a.0);
                let squashed = (a.1 / b.1).max(b.1 / a.1);
                assert!(brighter > 1.1 || squashed > 1.1, "{a:?} and {b:?} are too alike");
            }
        }
    }

    // How much the top of the sound jumps about from moment to moment. A wave
    // that breaks up into sparse spikes sounds torn, and scores high.
    fn flutter_of(samples: &[f32]) -> f32 {
        let mut top = Vec::new();
        for three in samples.windows(3) {
            top.push(three[0] - 2.0 * three[1] + three[2]);
        }
        let mut levels = Vec::new();
        for moment in top.chunks(96) {
            levels.push(rms_of(moment));
        }
        let mut mean = 0.0;
        for level in &levels {
            mean += level;
        }
        mean /= levels.len() as f32;
        let mut spread = 0.0;
        for level in &levels {
            spread += (level - mean) * (level - mean);
        }
        (spread / levels.len() as f32).sqrt() / mean
    }

    #[test]
    fn the_lead_does_not_tear_on_a_strum() {
        // Most flutter a strum through the Lead's high gain can have. A pickup
        // further along the string, at 24, goes past it and sounds torn.
        let most = 0.6;
        let lead = &ELECTRICS[2];
        assert_eq!(lead.name, "Electric Lead");
        let flutter = flutter_of(&chord_through(&electric_sound(lead)));
        assert!(flutter < most, "{flutter}");

        let torn = Electric { pickup: 24, ..ELECTRICS[2] };
        let flutter = flutter_of(&chord_through(&electric_sound(&torn)));
        assert!(flutter > most, "{flutter}");
    }

    #[test]
    fn only_the_clean_electric_keeps_its_peaks() {
        let mut crests = Vec::new();
        for electric in &ELECTRICS {
            crests.push(crest_of(&chord_through(&electric_sound(electric))));
        }
        for pushed in &crests[1..] {
            assert!(crests[0] > *pushed * 1.5, "{crests:?}");
        }
    }
}
