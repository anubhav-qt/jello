// The 11 scales a song's keys can follow (docs/adr/0047-song-key-mappings.md,
// docs/design.md#song-key-mappings). A scale is a home note and a kind. Its
// notes are its core, the scale itself, and its color, the outside notes that
// work anyway. All of them are counted in half steps from the home note.
//
// Guidance (src/harmony.rs) knows major and minor keys, so each scale also
// says which of those it reads as.

use crate::harmony::{Key, Mode, Quality, key_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleKind {
    Major,
    Minor,
    Dorian,
    Mixolydian,
    Lydian,
    HarmonicMinor,
    MelodicMinor,
    Phrygian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
}

pub const SCALE_KINDS: [ScaleKind; 11] = [
    ScaleKind::Major,
    ScaleKind::Minor,
    ScaleKind::Dorian,
    ScaleKind::Mixolydian,
    ScaleKind::Lydian,
    ScaleKind::HarmonicMinor,
    ScaleKind::MelodicMinor,
    ScaleKind::Phrygian,
    ScaleKind::MajorPentatonic,
    ScaleKind::MinorPentatonic,
    ScaleKind::Blues,
];

// What a scale's default keys are made from. `home` is the five best notes,
// lowest first, with the home note second, for the home row. `upper` is the
// rest of the scale's notes, one for each key of the upper row from Y on.
// Each is placed so a note and the note it resolves to never share a finger.
pub(crate) struct ScaleShape {
    pub core: &'static [i16],
    pub color: &'static [i16],
    pub home: [i16; 5],
    pub upper: &'static [i16],
}

impl ScaleKind {
    // The kind's name, as a scale's name and the song file write it.
    pub fn word(self) -> &'static str {
        match self {
            ScaleKind::Major => "major",
            ScaleKind::Minor => "minor",
            ScaleKind::Dorian => "dorian",
            ScaleKind::Mixolydian => "mixolydian",
            ScaleKind::Lydian => "lydian",
            ScaleKind::HarmonicMinor => "harmonic minor",
            ScaleKind::MelodicMinor => "melodic minor",
            ScaleKind::Phrygian => "phrygian",
            ScaleKind::MajorPentatonic => "major pentatonic",
            ScaleKind::MinorPentatonic => "minor pentatonic",
            ScaleKind::Blues => "blues",
        }
    }

    pub fn of_word(word: &str) -> Option<ScaleKind> {
        for kind in SCALE_KINDS {
            if kind.word() == word {
                return Some(kind);
            }
        }
        None
    }

    // Whether guidance reads this kind as a major or a minor key.
    pub fn mode(self) -> Mode {
        match self {
            ScaleKind::Major | ScaleKind::Mixolydian | ScaleKind::Lydian | ScaleKind::MajorPentatonic => Mode::Major,
            _ => Mode::Minor,
        }
    }

    // The approved table in docs/design.md, "Notes per scale". The minor
    // kind has both sevenths.
    pub(crate) fn shape(self) -> ScaleShape {
        match self {
            ScaleKind::Major => ScaleShape {
                core: &[0, 2, 4, 5, 7, 9, 11],
                color: &[3, 10, 6],
                home: [-3, 0, 2, 4, 7],
                upper: &[6, 3, 5, -1, -2],
            },
            ScaleKind::Minor => ScaleShape {
                core: &[0, 2, 3, 5, 7, 8, 10, 11],
                color: &[9, 6],
                home: [-2, 0, 3, 5, 7],
                upper: &[9, 6, 8, 2, -1],
            },
            ScaleKind::Dorian => ScaleShape {
                core: &[0, 2, 3, 5, 7, 9, 10],
                color: &[4, 11, 6],
                home: [-2, 0, 3, 5, 7],
                upper: &[9, 6, 4, 2, -1],
            },
            ScaleKind::Mixolydian => ScaleShape {
                core: &[0, 2, 4, 5, 7, 9, 10],
                color: &[11, 3, 6],
                home: [-2, 0, 2, 4, 7],
                upper: &[9, 6, 5, 3, -1],
            },
            ScaleKind::Lydian => ScaleShape {
                core: &[0, 2, 4, 6, 7, 9, 11],
                color: &[5, 10, 3],
                home: [-3, 0, 2, 4, 7],
                upper: &[6, 3, 5, -1, -2],
            },
            ScaleKind::HarmonicMinor => ScaleShape {
                core: &[0, 2, 3, 5, 7, 8, 11],
                color: &[10, 9, 6],
                home: [-1, 0, 3, 5, 7],
                upper: &[9, 6, 8, 2, -2],
            },
            ScaleKind::MelodicMinor => ScaleShape {
                core: &[0, 2, 3, 5, 7, 9, 11],
                color: &[10, 8, 6],
                home: [-1, 0, 3, 5, 7],
                upper: &[9, 6, 8, 2, -2],
            },
            ScaleKind::Phrygian => ScaleShape {
                core: &[0, 1, 3, 5, 7, 8, 10],
                color: &[4, 6],
                home: [-2, 0, 1, 3, 7],
                upper: &[8, 6, 5, 4],
            },
            ScaleKind::MajorPentatonic => ScaleShape {
                core: &[0, 2, 4, 7, 9],
                color: &[3, 5, 10],
                home: [-3, 0, 2, 4, 7],
                upper: &[3, 5, -2],
            },
            ScaleKind::MinorPentatonic => ScaleShape {
                core: &[0, 3, 5, 7, 10],
                color: &[6, 2, 9],
                home: [-2, 0, 3, 5, 7],
                upper: &[6, 2, 9],
            },
            ScaleKind::Blues => ScaleShape {
                core: &[0, 3, 5, 6, 7, 10],
                color: &[4, 9, 2],
                home: [-2, 0, 3, 5, 7],
                upper: &[6, 4, 9, 2],
            },
        }
    }

    // The chords this kind's default keys hold, best first, each a root in
    // half steps from the home note and a quality. Major and minor keep the
    // chords guidance knows (src/harmony.rs), placed by role. Any other kind
    // takes the major and minor chords its own notes spell: those of the core
    // first, then those that need a color note.
    pub(crate) fn chords(self) -> Vec<(u8, Quality)> {
        use Quality::{Major, Minor};
        match self {
            ScaleKind::Major => vec![
                (0, Major),
                (7, Major),
                (5, Major),
                (9, Minor),
                (2, Minor),
                (4, Minor),
                (10, Major),
                (3, Major),
                (8, Major),
                (5, Minor),
                (4, Major),
                (11, Major),
                (9, Major),
                (2, Major),
            ],
            ScaleKind::Minor => vec![
                (0, Minor),
                (10, Major),
                (8, Major),
                (3, Major),
                (5, Minor),
                (7, Major),
                (7, Minor),
                (0, Major),
                (5, Major),
                (1, Major),
                (2, Major),
            ],
            _ => self.spelled_chords(),
        }
    }

    fn spelled_chords(self) -> Vec<(u8, Quality)> {
        const ROOTS_BY_ROLE: [u8; 12] = [0, 7, 5, 9, 2, 10, 3, 8, 4, 11, 1, 6];
        let shape = self.shape();
        let core = pitches_of(shape.core);
        let mut all = core.clone();
        all.extend(pitches_of(shape.color));

        let mut chords = Vec::new();
        for allowed in [&core, &all] {
            for root in ROOTS_BY_ROLE {
                for quality in [Quality::Major, Quality::Minor] {
                    let spelled = triad_fits(root, quality, allowed);
                    if spelled && !chords.contains(&(root, quality)) {
                        chords.push((root, quality));
                    }
                }
            }
        }
        chords
    }
}

fn pitches_of(steps: &[i16]) -> Vec<u8> {
    let mut pitches = Vec::new();
    for step in steps {
        pitches.push(step.rem_euclid(12) as u8);
    }
    pitches
}

fn triad_fits(root: u8, quality: Quality, allowed: &[u8]) -> bool {
    let third = match quality {
        Quality::Major => 4,
        Quality::Minor => 3,
    };
    for step in [0, third, 7] {
        if !allowed.contains(&((root + step) % 12)) {
            return false;
        }
    }
    true
}

// A scale: a home note, as a pitch from 0 (C) to 11 (B), and a kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    pub tonic: u8,
    pub kind: ScaleKind,
}

impl Scale {
    // The major or minor scale of `key`.
    pub fn of_key(key: Key) -> Scale {
        let kind = match key.mode {
            Mode::Major => ScaleKind::Major,
            Mode::Minor => ScaleKind::Minor,
        };
        Scale { tonic: key.tonic, kind }
    }

    // The major or minor key guidance reads this scale as.
    pub fn key(self) -> Key {
        Key {
            tonic: self.tonic,
            mode: self.kind.mode(),
        }
    }

    // Its name, like "A dorian" or "Db major". The home note is spelled the
    // way the key it reads as spells it.
    pub fn name(self) -> String {
        let key = key_name(self.key());
        let tonic = key.split(' ').next().unwrap_or_default();
        format!("{tonic} {}", self.kind.word())
    }

    // The scale a name like "A harmonic minor" names, or None.
    pub fn of_name(name: &str) -> Option<Scale> {
        let (tonic_name, kind_word) = name.trim().split_once(' ')?;
        let kind = ScaleKind::of_word(kind_word.trim())?;
        let tonic = crate::harmony::pitch_of_name(tonic_name)?;
        Some(Scale { tonic, kind })
    }

    // Whether `note`, in any octave, is one of the scale's notes, core or
    // color.
    #[cfg(test)]
    pub fn holds(self, note: u8) -> bool {
        let shape = self.kind.shape();
        let step = (i16::from(note) - i16::from(self.tonic)).rem_euclid(12);
        shape.core.contains(&step) || shape.color.contains(&step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_is_named_and_read_back_by_its_name() {
        for tonic in 0..12 {
            for kind in SCALE_KINDS {
                let scale = Scale { tonic, kind };
                assert_eq!(Scale::of_name(&scale.name()), Some(scale), "{}", scale.name());
            }
        }
        assert_eq!(Scale { tonic: 9, kind: ScaleKind::Dorian }.name(), "A dorian");
        assert_eq!(Scale { tonic: 1, kind: ScaleKind::Major }.name(), "Db major");
        assert_eq!(Scale::of_name("A nothing"), None);
        assert_eq!(Scale::of_name("H major"), None);
        assert_eq!(Scale::of_name(""), None);
    }

    #[test]
    fn notes_per_scale_follow_the_approved_table() {
        // (kind, left-out notes), from docs/design.md.
        let left_out: [(ScaleKind, &[i16]); 11] = [
            (ScaleKind::Major, &[1, 8]),
            (ScaleKind::Minor, &[1, 4]),
            (ScaleKind::Dorian, &[1, 8]),
            (ScaleKind::Mixolydian, &[1, 8]),
            (ScaleKind::Lydian, &[1, 8]),
            (ScaleKind::HarmonicMinor, &[1, 4]),
            (ScaleKind::MelodicMinor, &[1, 4]),
            (ScaleKind::Phrygian, &[2, 9, 11]),
            (ScaleKind::MajorPentatonic, &[1, 6, 8, 11]),
            (ScaleKind::MinorPentatonic, &[1, 4, 8, 11]),
            (ScaleKind::Blues, &[1, 8, 11]),
        ];
        for (kind, missing) in left_out {
            let shape = kind.shape();
            for step in 0..12i16 {
                let held = shape.core.contains(&step) || shape.color.contains(&step);
                assert_eq!(held, !missing.contains(&step), "{kind:?} step {step}");
            }
        }
    }

    #[test]
    fn each_shape_holds_every_note_of_its_scale_once() {
        for kind in SCALE_KINDS {
            let shape = kind.shape();
            let mut placed = pitches_of(&shape.home);
            placed.extend(pitches_of(shape.upper));
            let mut wanted = pitches_of(shape.core);
            wanted.extend(pitches_of(shape.color));
            placed.sort();
            wanted.sort();
            assert_eq!(placed, wanted, "{kind:?}");
        }
    }

    #[test]
    fn every_kind_has_the_home_chord_first() {
        for kind in SCALE_KINDS {
            let chords = kind.chords();
            assert!(chords.len() >= 3, "{kind:?} has {chords:?}");
            assert_eq!(chords[0].0, 0, "{kind:?}");
            let expected = match kind.mode() {
                Mode::Major => Quality::Major,
                Mode::Minor => Quality::Minor,
            };
            assert_eq!(chords[0].1, expected, "{kind:?}");
        }
    }

    #[test]
    fn modes_read_as_the_key_with_their_third() {
        assert_eq!(ScaleKind::Mixolydian.mode(), Mode::Major);
        assert_eq!(ScaleKind::Blues.mode(), Mode::Minor);
        assert_eq!(Scale { tonic: 9, kind: ScaleKind::Dorian }.key(), Key { tonic: 9, mode: Mode::Minor });
    }

    #[test]
    fn a_scale_holds_its_notes_in_every_octave() {
        let c_major = Scale { tonic: 0, kind: ScaleKind::Major };
        assert!(c_major.holds(60));
        assert!(c_major.holds(63));
        assert!(!c_major.holds(61));
        assert!(!c_major.holds(68 + 12));
    }
}
