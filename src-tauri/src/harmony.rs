// Chords and keys: what a chord is, how its name is spelled, and how well it
// fits a song's key (docs/design.md, "Chord guidance").
//
// A key is a song's home note and the seven notes around it: C major's are
// the white keys. A key has chords built from its own notes, and a few from
// outside it that still work, each for a reason this file can name.

use serde::{Deserialize, Serialize};

// Whether a chord sounds bright (major) or darker (minor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quality {
    Major,
    Minor,
}

// A chord of three notes, named after its lowest one, the root.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chord {
    // The root, as a MIDI note: C3 is 48.
    pub root: u8,
    pub quality: Quality,
}

impl Chord {
    // The chord's notes, lowest first: the root, the third and the fifth.
    // The third is four half steps above the root in a major chord and three
    // in a minor chord. The fifth is always seven. So C major is C E G, and
    // C minor is C Eb G.
    pub fn notes(&self) -> [u8; 3] {
        let third = match self.quality {
            Quality::Major => 4,
            Quality::Minor => 3,
        };
        [self.root, self.root + third, self.root + 7]
    }

    // Its root as a pitch, from 0 (C) to 11 (B), in any octave.
    pub fn pitch(&self) -> u8 {
        self.root % 12
    }
}

// Whether a key's home chord is major or minor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Major,
    Minor,
}

// A song's key, like C major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    // The home note, as a pitch from 0 (C) to 11 (B).
    pub tonic: u8,
    pub mode: Mode,
}

// How well a chord fits a key.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    // Built from the key's own notes.
    InKey,
    // From outside the key, but it works, for a known reason.
    CanWork,
    // From outside the key, and likely to clash. It still plays.
    Outside,
}

// What guidance says about one chord in a key.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Guide {
    // The chord's name, spelled for the key: Bb in F major, A# in B major.
    pub name: String,
    pub fit: Fit,
    // Why it fits, or doesn't, in a few plain words.
    pub why: String,
}

// The seven letters, and the pitch each one has with no sharp or flat.
const LETTERS: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];
const LETTER_PITCHES: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

// Names for the 12 pitches, from C up. Each black key has two: the sharp of
// the letter below it, or the flat of the letter above it.
const SHARP_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLAT_NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
// The names chord charts use most, for when there is no key to follow.
const COMMON_NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
// The same for minor chords, but G#m, not Abm: songs in E major and B major
// play it, and they write it with a sharp.
const COMMON_MINOR_NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "G#", "A", "Bb", "B"];

// Which names a key gives its black keys.
#[derive(Clone, Copy)]
enum Spelling {
    Sharps,
    Flats,
    Common,
}

// How each key spells its black keys, by its home note from C up: the way
// its own notes are written. Db major, not C# major, which would need seven
// sharps. C major and A minor have no black keys, so they use the common
// names.
const MAJOR_SPELLINGS: [Spelling; 12] = [
    Spelling::Common, // C
    Spelling::Flats,  // Db
    Spelling::Sharps, // D
    Spelling::Flats,  // Eb
    Spelling::Sharps, // E
    Spelling::Flats,  // F
    Spelling::Sharps, // F#
    Spelling::Sharps, // G
    Spelling::Flats,  // Ab
    Spelling::Sharps, // A
    Spelling::Flats,  // Bb
    Spelling::Sharps, // B
];
const MINOR_SPELLINGS: [Spelling; 12] = [
    Spelling::Flats,  // C
    Spelling::Sharps, // C#
    Spelling::Flats,  // D
    Spelling::Flats,  // Eb
    Spelling::Sharps, // E
    Spelling::Flats,  // F
    Spelling::Sharps, // F#
    Spelling::Flats,  // G
    Spelling::Sharps, // G#
    Spelling::Common, // A
    Spelling::Flats,  // Bb
    Spelling::Sharps, // B
];

// Where a chord sits in a key, measured from the key's home note: how far
// its root is above it, in letters and in half steps, and whether it is
// major or minor. In C major, Em is 2 letters (C to E) and 4 half steps up.
#[derive(Clone, Copy)]
struct Place {
    letters: u8,
    half_steps: u8,
    quality: Quality,
}

const fn major(letters: u8, half_steps: u8) -> Place {
    Place {
        letters,
        half_steps,
        quality: Quality::Major,
    }
}

const fn minor(letters: u8, half_steps: u8) -> Place {
    Place {
        letters,
        half_steps,
        quality: Quality::Minor,
    }
}

// A chord guidance knows, and what it says about it.
struct Role {
    place: Place,
    fit: Fit,
    // Why it fits. In it, {to} stands for the chord at `to`, and {twin} for
    // the key with the same home note and the other mode: C minor is
    // C major's twin.
    why: &'static str,
    to: Option<Place>,
}

// The chords a major key's guidance knows, with C major's in the comments.
// Its own seventh chord, Bdim, is left out: the chord keys play no
// diminished chords.
const MAJOR_KEY_ROLES: [Role; 14] = [
    // The key's own chords.
    Role {
        place: major(0, 0), // C
        fit: Fit::InKey,
        why: "Home. It sounds settled and finished.",
        to: None,
    },
    Role {
        place: minor(1, 2), // Dm
        fit: Fit::InKey,
        why: "A soft step away from home. It often moves on to {to}.",
        to: Some(major(4, 7)),
    },
    Role {
        place: minor(2, 4), // Em
        fit: Fit::InKey,
        why: "Gentle and a little sad. It often moves on to {to}.",
        to: Some(minor(5, 9)),
    },
    Role {
        place: major(3, 5), // F
        fit: Fit::InKey,
        why: "It lifts away from home, open and hopeful.",
        to: None,
    },
    Role {
        place: major(4, 7), // G
        fit: Fit::InKey,
        why: "Tension that pulls back home to {to}.",
        to: Some(major(0, 0)),
    },
    Role {
        place: minor(5, 9), // Am
        fit: Fit::InKey,
        why: "The key's sad side, and a surprise landing right after {to}.",
        to: Some(major(4, 7)),
    },
    // Lead-in chords: each pulls toward one of the key's own chords, the
    // way the five chord pulls home.
    Role {
        place: major(1, 2), // D
        fit: Fit::CanWork,
        why: "Outside the key, but it pulls toward {to}. The most common lead-in chord.",
        to: Some(major(4, 7)),
    },
    Role {
        place: major(2, 4), // E
        fit: Fit::CanWork,
        why: "Outside the key, but it pulls hard toward {to}.",
        to: Some(minor(5, 9)),
    },
    Role {
        place: major(5, 9), // A
        fit: Fit::CanWork,
        why: "Outside the key, but it pulls toward {to}.",
        to: Some(minor(1, 2)),
    },
    Role {
        place: major(6, 11), // B
        fit: Fit::CanWork,
        why: "Outside the key, but it pulls toward {to}. Rare in pop.",
        to: Some(minor(2, 4)),
    },
    // Chords borrowed from the twin minor key.
    Role {
        place: minor(3, 5), // Fm
        fit: Fit::CanWork,
        why: "Borrowed from {twin}. Tender and bittersweet, often right before {to}.",
        to: Some(major(0, 0)),
    },
    Role {
        place: major(2, 3), // Eb
        fit: Fit::CanWork,
        why: "Borrowed from {twin}. Bright and dark at the same time.",
        to: None,
    },
    Role {
        place: major(5, 8), // Ab
        fit: Fit::CanWork,
        why: "Borrowed from {twin}. Big and cinematic.",
        to: None,
    },
    Role {
        place: major(6, 10), // Bb
        fit: Fit::CanWork,
        why: "Borrowed from {twin}. Bold and rock-like, often going home to {to}.",
        to: Some(major(0, 0)),
    },
];

// The chords a minor key's guidance knows, with A minor's in the comments.
// Its own second chord, Bdim, is left out, like a major key's seventh.
const MINOR_KEY_ROLES: [Role; 11] = [
    // The key's own chords. The major five, E, is built from a raised note,
    // but minor-key songs use it so often that it counts as their own.
    Role {
        place: minor(0, 0), // Am
        fit: Fit::InKey,
        why: "Home. Settled, with a darker color.",
        to: None,
    },
    Role {
        place: major(2, 3), // C
        fit: Fit::InKey,
        why: "The key's bright side.",
        to: None,
    },
    Role {
        place: minor(3, 5), // Dm
        fit: Fit::InKey,
        why: "It moves away from home, darker still.",
        to: None,
    },
    Role {
        place: minor(4, 7), // Em
        fit: Fit::InKey,
        why: "A gentle pull back home to {to}.",
        to: Some(minor(0, 0)),
    },
    Role {
        place: major(4, 7), // E
        fit: Fit::InKey,
        why: "The strongest pull back home to {to}.",
        to: Some(minor(0, 0)),
    },
    Role {
        place: major(5, 8), // F
        fit: Fit::InKey,
        why: "Warm and wide. It often moves on to {to}.",
        to: Some(major(6, 10)),
    },
    Role {
        place: major(6, 10), // G
        fit: Fit::InKey,
        why: "It lifts up, toward {to} or back home.",
        to: Some(major(2, 3)),
    },
    // Chords from outside the key that still work.
    Role {
        place: major(0, 0), // A
        fit: Fit::CanWork,
        why: "Home turned major: a bright, surprising ending.",
        to: None,
    },
    Role {
        place: major(3, 5), // D
        fit: Fit::CanWork,
        why: "A major four, like a ray of light in a dark key.",
        to: None,
    },
    Role {
        place: major(1, 2), // B
        fit: Fit::CanWork,
        why: "Outside the key, but it pulls toward {to}.",
        to: Some(major(4, 7)),
    },
    Role {
        place: major(1, 1), // Bb
        fit: Fit::CanWork,
        why: "Dramatic, heard in film and classical music. It often moves on to {to}.",
        to: Some(major(4, 7)),
    },
];

// What guidance says about a chord it doesn't know.
const OUTSIDE_WHY: &str = "Outside the key, so it will likely clash. It still plays.";

// A major key's notes, as half steps above its home note: C D E F G A B in
// C major.
const MAJOR_KEY_NOTES: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
// A minor key's notes, the same way: A B C D E F G in A minor, and G#, the
// note just below home, raised so it pulls up to home. The key's own major
// five, E, is built on it, so the melody keys agree with the chord keys.
const MINOR_KEY_NOTES: [u8; 8] = [0, 2, 3, 5, 7, 8, 10, 11];

// All 24 keys: the 12 major keys from C up, then the 12 minor keys.
pub fn all_keys() -> Vec<Key> {
    let mut keys = Vec::new();
    for tonic in 0..12 {
        keys.push(Key {
            tonic,
            mode: Mode::Major,
        });
    }
    for tonic in 0..12 {
        keys.push(Key {
            tonic,
            mode: Mode::Minor,
        });
    }
    keys
}

// The key's name, like "Db major" or "F# minor".
pub fn key_name(key: Key) -> String {
    let tonic = names_for(key)[key.tonic as usize];
    match key.mode {
        Mode::Major => format!("{tonic} major"),
        Mode::Minor => format!("{tonic} minor"),
    }
}

// The chord's name, like "Bb" or "F#m": spelled for `key` if there is one,
// or with the common names if not.
pub fn chord_name(chord: Chord, key: Option<Key>) -> String {
    match key {
        Some(key) => guide(key, chord).name,
        None => {
            let names = match chord.quality {
                Quality::Major => COMMON_NAMES,
                Quality::Minor => COMMON_MINOR_NAMES,
            };
            let root = names[chord.pitch() as usize];
            with_quality(root, chord.quality)
        }
    }
}

// What guidance says about `chord` in `key`.
pub fn guide(key: Key, chord: Chord) -> Guide {
    let half_steps = (chord.pitch() + 12 - key.tonic) % 12;
    for role in roles_of(key) {
        let place = role.place;
        if place.half_steps == half_steps && place.quality == chord.quality {
            return Guide {
                name: name_at(key, place),
                fit: role.fit,
                why: explain(key, role),
            };
        }
    }

    // A chord guidance doesn't know is spelled the way the key spells its
    // black keys.
    let root = names_for(key)[chord.pitch() as usize];
    Guide {
        name: with_quality(root, chord.quality),
        fit: Fit::Outside,
        why: String::from(OUTSIDE_WHY),
    }
}

// How well a melody `note` fits `key`: in the key, or outside it. A note is
// never "can work": an outside note is fine over a chord that has it, and
// the UI shows that by filling the held chord's notes.
pub fn note_fit(key: Key, note: u8) -> Fit {
    let half_steps = (note % 12 + 12 - key.tonic) % 12;
    let key_notes: &[u8] = match key.mode {
        Mode::Major => &MAJOR_KEY_NOTES,
        Mode::Minor => &MINOR_KEY_NOTES,
    };
    if key_notes.contains(&half_steps) {
        Fit::InKey
    } else {
        Fit::Outside
    }
}

fn roles_of(key: Key) -> &'static [Role] {
    match key.mode {
        Mode::Major => &MAJOR_KEY_ROLES,
        Mode::Minor => &MINOR_KEY_ROLES,
    }
}

// The role's words, with the chord names filled in.
fn explain(key: Key, role: &Role) -> String {
    let mut why = String::from(role.why);
    if let Some(to) = role.to {
        why = why.replace("{to}", &name_at(key, to));
    }
    let twin = Key {
        tonic: key.tonic,
        mode: twin_mode(key.mode),
    };
    why = why.replace("{twin}", &key_name(twin));
    why
}

fn twin_mode(mode: Mode) -> Mode {
    match mode {
        Mode::Major => Mode::Minor,
        Mode::Minor => Mode::Major,
    }
}

// The name of the chord at `place` in `key`.
fn name_at(key: Key, place: Place) -> String {
    let pitch = (key.tonic + place.half_steps) % 12;
    let root = spell(key, place.letters, pitch);
    with_quality(&root, place.quality)
}

// The name of `pitch` as the note `letters` letters above the key's home
// note. That letter decides between a black key's two names: in G major,
// the chord five letters up (G A B C D E) is Eb, not D#.
fn spell(key: Key, letters: u8, pitch: u8) -> String {
    // A white key keeps its own letter, so it is always E, never Fb.
    for index in 0..7 {
        if LETTER_PITCHES[index] == pitch {
            return String::from(LETTERS[index]);
        }
    }

    let letter = (home_letter(key) + letters as usize) % 7;
    let natural = LETTER_PITCHES[letter];
    if (natural + 1) % 12 == pitch {
        return format!("{}#", LETTERS[letter]);
    }
    if (pitch + 1) % 12 == natural {
        return format!("{}b", LETTERS[letter]);
    }

    // Too far from the letter for one sharp or flat: use the key's names.
    String::from(names_for(key)[pitch as usize])
}

// Which of the seven letters the key's home note is written with: D (1)
// for Db major.
fn home_letter(key: Key) -> usize {
    let tonic = names_for(key)[key.tonic as usize];
    for index in 0..7 {
        if tonic.starts_with(LETTERS[index]) {
            return index;
        }
    }
    0
}

// The names a key gives its black keys.
fn names_for(key: Key) -> [&'static str; 12] {
    let spelling = match key.mode {
        Mode::Major => MAJOR_SPELLINGS[key.tonic as usize],
        Mode::Minor => MINOR_SPELLINGS[key.tonic as usize],
    };
    match spelling {
        Spelling::Sharps => SHARP_NAMES,
        Spelling::Flats => FLAT_NAMES,
        Spelling::Common => COMMON_NAMES,
    }
}

// A chord name from its root's name: "A" for A major, "Am" for A minor.
fn with_quality(root: &str, quality: Quality) -> String {
    match quality {
        Quality::Major => String::from(root),
        Quality::Minor => format!("{root}m"),
    }
}

// These tests only ask about chords and keys. They make no sound.
#[cfg(test)]
mod tests {
    use super::*;

    const C_MAJOR: Key = Key {
        tonic: 0,
        mode: Mode::Major,
    };
    const A_MINOR: Key = Key {
        tonic: 9,
        mode: Mode::Minor,
    };

    // The 24 chords the chord keys can play, as roots in octave 3: the 12
    // major chords from C up, then the 12 minor chords.
    fn every_chord() -> Vec<Chord> {
        let mut chords = Vec::new();
        for root in 48..60 {
            chords.push(Chord {
                root,
                quality: Quality::Major,
            });
        }
        for root in 48..60 {
            chords.push(Chord {
                root,
                quality: Quality::Minor,
            });
        }
        chords
    }

    // The names of the chords that fit `key` this way, from C up, majors
    // first.
    fn names_that_fit(key: Key, fit: Fit) -> Vec<String> {
        let mut names = Vec::new();
        for chord in every_chord() {
            let guide = guide(key, chord);
            if guide.fit == fit {
                names.push(guide.name);
            }
        }
        names
    }

    #[test]
    fn a_major_chord_is_root_major_third_and_fifth() {
        let c_major = Chord {
            root: 48,
            quality: Quality::Major,
        };
        assert_eq!(c_major.notes(), [48, 52, 55]); // C3 E3 G3
    }

    #[test]
    fn a_minor_chord_has_its_middle_note_a_half_step_lower() {
        let c_minor = Chord {
            root: 48,
            quality: Quality::Minor,
        };
        assert_eq!(c_minor.notes(), [48, 51, 55]); // C3 Eb3 G3
    }

    #[test]
    fn c_major_holds_its_six_own_chords() {
        let in_key = names_that_fit(C_MAJOR, Fit::InKey);
        assert_eq!(in_key, ["C", "F", "G", "Dm", "Em", "Am"]);
    }

    #[test]
    fn c_major_can_use_lead_ins_and_chords_borrowed_from_c_minor() {
        let can_work = names_that_fit(C_MAJOR, Fit::CanWork);
        assert_eq!(can_work, ["D", "Eb", "E", "Ab", "A", "Bb", "B", "Fm"]);
    }

    #[test]
    fn a_minor_holds_its_own_chords_and_the_major_five() {
        let in_key = names_that_fit(A_MINOR, Fit::InKey);
        assert_eq!(in_key, ["C", "E", "F", "G", "Dm", "Em", "Am"]);
    }

    #[test]
    fn a_minor_can_use_a_major_home_four_lead_in_and_flat_two() {
        let can_work = names_that_fit(A_MINOR, Fit::CanWork);
        assert_eq!(can_work, ["D", "A", "Bb", "B"]);
    }

    #[test]
    fn every_key_sorts_the_24_chords_the_same_way() {
        // Each key is its mode's pattern moved to a new home note, so every
        // major key has 6 own chords and 8 that can work, and every minor
        // key 7 and 4. The rest are outside.
        for key in all_keys() {
            let in_key = names_that_fit(key, Fit::InKey).len();
            let can_work = names_that_fit(key, Fit::CanWork).len();
            let outside = names_that_fit(key, Fit::Outside).len();
            match key.mode {
                Mode::Major => {
                    assert_eq!((in_key, can_work, outside), (6, 8, 10), "{}", key_name(key));
                }
                Mode::Minor => {
                    assert_eq!((in_key, can_work, outside), (7, 4, 13), "{}", key_name(key));
                }
            }
        }
    }

    #[test]
    fn a_chord_fits_the_same_in_every_octave() {
        let g3 = Chord {
            root: 55,
            quality: Quality::Major,
        };
        let g4 = Chord {
            root: 67,
            quality: Quality::Major,
        };
        assert_eq!(guide(C_MAJOR, g3), guide(C_MAJOR, g4));
    }

    #[test]
    fn there_are_24_keys_each_named_once() {
        let mut names = Vec::new();
        for key in all_keys() {
            names.push(key_name(key));
        }
        assert_eq!(names.len(), 24);
        assert_eq!(names[0], "C major");
        assert_eq!(names[1], "Db major");
        assert_eq!(names[6], "F# major");
        assert_eq!(names[12], "C minor");
        assert_eq!(names[13], "C# minor");
        assert_eq!(names[15], "Eb minor");
        assert_eq!(names[20], "G# minor");
        assert_eq!(names[22], "Bb minor");

        // Sorted, a name used twice would sit next to itself.
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 24);
    }

    #[test]
    fn chord_names_follow_the_key() {
        let f_major = Key {
            tonic: 5,
            mode: Mode::Major,
        };
        let b_major = Key {
            tonic: 11,
            mode: Mode::Major,
        };
        let g_major = Key {
            tonic: 7,
            mode: Mode::Major,
        };
        let d_major = Key {
            tonic: 2,
            mode: Mode::Major,
        };
        let b_flat = Chord {
            root: 58,
            quality: Quality::Major,
        };
        let f_sharp_minor = Chord {
            root: 54,
            quality: Quality::Minor,
        };
        let f_sharp = Chord {
            root: 54,
            quality: Quality::Major,
        };
        let e_flat = Chord {
            root: 51,
            quality: Quality::Major,
        };

        // The same chord, named for each key's own letters.
        assert_eq!(chord_name(b_flat, Some(f_major)), "Bb");
        assert_eq!(chord_name(b_flat, Some(b_major)), "A#");
        assert_eq!(chord_name(f_sharp_minor, Some(d_major)), "F#m");

        // G major borrows Eb from G minor, but leads into Bm with F#.
        assert_eq!(chord_name(e_flat, Some(g_major)), "Eb");
        assert_eq!(chord_name(f_sharp, Some(g_major)), "F#");

        // With no key, the common names.
        assert_eq!(chord_name(b_flat, None), "Bb");
        assert_eq!(chord_name(f_sharp_minor, None), "F#m");
        assert_eq!(chord_name(e_flat, None), "Eb");

        // Between G and A, the major chord is Ab but the minor chord is G#m.
        let a_flat = Chord {
            root: 56,
            quality: Quality::Major,
        };
        let g_sharp_minor = Chord {
            root: 56,
            quality: Quality::Minor,
        };
        assert_eq!(chord_name(a_flat, None), "Ab");
        assert_eq!(chord_name(g_sharp_minor, None), "G#m");
    }

    #[test]
    fn chords_outside_a_key_are_spelled_like_its_black_keys() {
        let d_major = Key {
            tonic: 2,
            mode: Mode::Major,
        };
        let f_major = Key {
            tonic: 5,
            mode: Mode::Major,
        };
        let d_sharp = Chord {
            root: 51,
            quality: Quality::Major,
        };
        let g_flat = Chord {
            root: 54,
            quality: Quality::Major,
        };
        assert_eq!(guide(d_major, d_sharp).fit, Fit::Outside);
        assert_eq!(guide(d_major, d_sharp).name, "D#");
        assert_eq!(guide(f_major, g_flat).fit, Fit::Outside);
        assert_eq!(guide(f_major, g_flat).name, "Gb");
    }

    #[test]
    fn white_keys_keep_their_own_letter() {
        // Db major borrows its flat three from Db minor: that is Fb on
        // paper, but it is the white key E, so it is called E.
        let d_flat_major = Key {
            tonic: 1,
            mode: Mode::Major,
        };
        let e_major = Chord {
            root: 52,
            quality: Quality::Major,
        };
        assert_eq!(guide(d_flat_major, e_major).fit, Fit::CanWork);
        assert_eq!(guide(d_flat_major, e_major).name, "E");
    }

    #[test]
    fn the_words_name_the_chords_they_mean() {
        let g = Chord {
            root: 55,
            quality: Quality::Major,
        };
        let f_minor = Chord {
            root: 53,
            quality: Quality::Minor,
        };
        let e = Chord {
            root: 52,
            quality: Quality::Major,
        };
        assert_eq!(guide(C_MAJOR, g).why, "Tension that pulls back home to C.");
        assert_eq!(
            guide(C_MAJOR, f_minor).why,
            "Borrowed from C minor. Tender and bittersweet, often right before C."
        );
        assert_eq!(guide(A_MINOR, e).why, "The strongest pull back home to Am.");
    }

    #[test]
    fn no_words_are_left_unfilled() {
        for key in all_keys() {
            for chord in every_chord() {
                let why = guide(key, chord).why;
                assert!(!why.contains('{'), "{} in {}: {why}", chord_name(chord, None), key_name(key));
            }
        }
    }

    #[test]
    fn no_two_roles_in_a_mode_are_the_same_chord() {
        for roles in [&MAJOR_KEY_ROLES[..], &MINOR_KEY_ROLES[..]] {
            for (index, role) in roles.iter().enumerate() {
                for other in &roles[index + 1..] {
                    let same_pitch = role.place.half_steps == other.place.half_steps;
                    let same_quality = role.place.quality == other.place.quality;
                    if same_pitch && same_quality {
                        panic!("two roles are the same chord: {}", role.why);
                    }
                }
            }
        }
    }

    // The notes from C4 up to B4 that are in `key`, by name.
    fn notes_in(key: Key) -> Vec<&'static str> {
        let mut names = Vec::new();
        for note in 60..72 {
            if note_fit(key, note) == Fit::InKey {
                names.push(SHARP_NAMES[(note % 12) as usize]);
            }
        }
        names
    }

    #[test]
    fn c_major_s_notes_are_the_white_keys() {
        assert_eq!(notes_in(C_MAJOR), ["C", "D", "E", "F", "G", "A", "B"]);
    }

    #[test]
    fn a_minor_s_notes_are_the_white_keys_and_g_sharp() {
        assert_eq!(notes_in(A_MINOR), ["C", "D", "E", "F", "G", "G#", "A", "B"]);
    }

    #[test]
    fn a_key_s_notes_start_from_its_home_note() {
        let d_major = Key {
            tonic: 2,
            mode: Mode::Major,
        };
        let e_minor = Key {
            tonic: 4,
            mode: Mode::Minor,
        };
        assert_eq!(notes_in(d_major), ["C#", "D", "E", "F#", "G", "A", "B"]);
        assert_eq!(notes_in(e_minor), ["C", "D", "D#", "E", "F#", "G", "A", "B"]);
    }

    #[test]
    fn a_note_fits_the_same_in_every_octave() {
        for key in all_keys() {
            for note in 0..12 {
                let fit = note_fit(key, note);
                for octave in 1..10 {
                    assert_eq!(note_fit(key, note + 12 * octave), fit, "{} in {}", note, key_name(key));
                }
            }
        }
    }

    #[test]
    fn every_chord_in_a_key_is_built_from_its_notes() {
        // So the melody keys never dim a note of a chord the chord keys call
        // the key's own.
        for key in all_keys() {
            for chord in every_chord() {
                if guide(key, chord).fit != Fit::InKey {
                    continue;
                }
                for note in chord.notes() {
                    assert_eq!(note_fit(key, note), Fit::InKey, "{} in {}", note, key_name(key));
                }
            }
        }
    }

    #[test]
    fn the_ui_reads_fits_by_these_names() {
        // ui/compose.ts reads "in_key", "can_work" and "outside".
        assert_eq!(serde_json::to_value(Fit::InKey).unwrap(), "in_key");
        assert_eq!(serde_json::to_value(Fit::CanWork).unwrap(), "can_work");
        assert_eq!(serde_json::to_value(Fit::Outside).unwrap(), "outside");
        assert_eq!(serde_json::to_value(Mode::Minor).unwrap(), "minor");
    }
}
