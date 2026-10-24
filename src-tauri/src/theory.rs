// jello_theory (docs/design.md#the-tutor-to-mastery): music theory worked
// out exactly, so the model asks instead of counting half steps itself.
// It grows out of harmony.rs, which knows the 24 chords the keys play;
// this file knows every common chord, scale and interval.
//
// The questions:
//
//   chord          the chords some notes make, like ["E4", "G4", "C5"]
//   numerals       chords as numbers in a key, like "vi" or "V7"
//   scale          a scale or mode's notes, and its chords on each degree
//   interval       how far apart two notes are, or the note an interval up
//   voice_leading  how far each note moves from one chord to the next
//   keys           which laptop keys play notes or a chord at this octave

use serde_json::{Value, json};

use crate::harmony::{COMMON_NAMES, FLAT_NAMES, Key, LETTER_PITCHES, LETTERS, SHARP_NAMES, key_name, key_of_name, pitch_of_name};
use crate::song_file::{note_name, note_of_name};
use crate::tools::{Studio, ToolAnswer, optional_text_argument, text_argument};

// The questions jello_theory answers.
pub const QUESTIONS: [&str; 6] = ["chord", "numerals", "scale", "interval", "voice_leading", "keys"];

// A kind of chord: the letters after its root, its name in words, and its
// notes in half steps above the root. A 9th is 14, past the octave.
struct ChordKind {
    suffix: &'static str,
    words: &'static str,
    steps: &'static [u8],
}

const CHORD_KINDS: [ChordKind; 21] = [
    ChordKind { suffix: "", words: "major", steps: &[0, 4, 7] },
    ChordKind { suffix: "m", words: "minor", steps: &[0, 3, 7] },
    ChordKind { suffix: "dim", words: "diminished", steps: &[0, 3, 6] },
    ChordKind { suffix: "aug", words: "augmented", steps: &[0, 4, 8] },
    ChordKind { suffix: "sus2", words: "suspended second", steps: &[0, 2, 7] },
    ChordKind { suffix: "sus4", words: "suspended fourth", steps: &[0, 5, 7] },
    ChordKind { suffix: "5", words: "power chord", steps: &[0, 7] },
    ChordKind { suffix: "6", words: "major sixth", steps: &[0, 4, 7, 9] },
    ChordKind { suffix: "m6", words: "minor sixth", steps: &[0, 3, 7, 9] },
    ChordKind { suffix: "7", words: "dominant seventh", steps: &[0, 4, 7, 10] },
    ChordKind { suffix: "maj7", words: "major seventh", steps: &[0, 4, 7, 11] },
    ChordKind { suffix: "m7", words: "minor seventh", steps: &[0, 3, 7, 10] },
    ChordKind { suffix: "mMaj7", words: "minor major seventh", steps: &[0, 3, 7, 11] },
    ChordKind { suffix: "m7b5", words: "half diminished seventh", steps: &[0, 3, 6, 10] },
    ChordKind { suffix: "dim7", words: "diminished seventh", steps: &[0, 3, 6, 9] },
    ChordKind { suffix: "7sus4", words: "dominant seventh, suspended fourth", steps: &[0, 5, 7, 10] },
    ChordKind { suffix: "add9", words: "major, added ninth", steps: &[0, 4, 7, 14] },
    ChordKind { suffix: "madd9", words: "minor, added ninth", steps: &[0, 3, 7, 14] },
    ChordKind { suffix: "9", words: "dominant ninth", steps: &[0, 4, 7, 10, 14] },
    ChordKind { suffix: "maj9", words: "major ninth", steps: &[0, 4, 7, 11, 14] },
    ChordKind { suffix: "m9", words: "minor ninth", steps: &[0, 3, 7, 10, 14] },
];

// A scale or mode: its name and its notes in half steps above the home
// note.
struct ScaleKind {
    name: &'static str,
    steps: &'static [u8],
}

const SCALES: [ScaleKind; 15] = [
    ScaleKind { name: "major", steps: &[0, 2, 4, 5, 7, 9, 11] },
    ScaleKind { name: "minor", steps: &[0, 2, 3, 5, 7, 8, 10] },
    ScaleKind { name: "ionian", steps: &[0, 2, 4, 5, 7, 9, 11] },
    ScaleKind { name: "dorian", steps: &[0, 2, 3, 5, 7, 9, 10] },
    ScaleKind { name: "phrygian", steps: &[0, 1, 3, 5, 7, 8, 10] },
    ScaleKind { name: "lydian", steps: &[0, 2, 4, 6, 7, 9, 11] },
    ScaleKind { name: "mixolydian", steps: &[0, 2, 4, 5, 7, 9, 10] },
    ScaleKind { name: "aeolian", steps: &[0, 2, 3, 5, 7, 8, 10] },
    ScaleKind { name: "locrian", steps: &[0, 1, 3, 5, 6, 8, 10] },
    ScaleKind { name: "harmonic minor", steps: &[0, 2, 3, 5, 7, 8, 11] },
    ScaleKind { name: "melodic minor", steps: &[0, 2, 3, 5, 7, 9, 11] },
    ScaleKind { name: "major pentatonic", steps: &[0, 2, 4, 7, 9] },
    ScaleKind { name: "minor pentatonic", steps: &[0, 3, 5, 7, 10] },
    ScaleKind { name: "blues", steps: &[0, 3, 5, 6, 7, 10] },
    ScaleKind { name: "whole tone", steps: &[0, 2, 4, 6, 8, 10] },
];

// The intervals by half steps, up to an octave.
const INTERVAL_NAMES: [&str; 13] = [
    "unison",
    "minor second",
    "major second",
    "minor third",
    "major third",
    "perfect fourth",
    "tritone",
    "perfect fifth",
    "minor sixth",
    "major sixth",
    "minor seventh",
    "major seventh",
    "octave",
];

const ROMAN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

// The numerals for each root counted from the key's home note.
const NUMERALS: [&str; 12] = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"];

// The most notes a question takes.
const MOST_NOTES: usize = 12;

// jello_theory: answers `arguments`' question.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let question = text_argument(arguments, "question")?;
    let answered = match question.as_str() {
        "chord" => chord_of_notes(arguments)?,
        "numerals" => numerals(studio, arguments)?,
        "scale" => scale(arguments)?,
        "interval" => interval(arguments)?,
        "voice_leading" => voice_leading(arguments)?,
        "keys" => keys_for(studio, arguments)?,
        _ => {
            let all = QUESTIONS.join(", ");
            return Err(format!("jello_theory has no question {question}: it has {all}"));
        }
    };
    Ok(ToolAnswer::of(answered, Vec::new()))
}

// The notes `arguments` list under `name`, each a note with its octave,
// like "E4", in their order.
fn notes_argument(arguments: &Value, name: &str) -> Result<Vec<u8>, String> {
    let Some(listed) = arguments[name].as_array() else {
        return Err(format!("{name} is needed, as a list of notes like [\"C4\", \"E4\", \"G4\"]"));
    };
    if listed.is_empty() || listed.len() > MOST_NOTES {
        return Err(format!("{name} has 1 to {MOST_NOTES} notes, not {}", listed.len()));
    }
    let mut notes = Vec::new();
    for listed_note in listed {
        let text = listed_note.as_str().unwrap_or_default();
        let Some(note) = note_of_name(text) else {
            return Err(format!("{text} is not a note with its octave, like \"C4\" or \"F#3\""));
        };
        notes.push(note);
    }
    Ok(notes)
}

// The different pitches `notes` play, from 0 (C) to 11 (B), in order.
fn pitches_of(notes: &[u8]) -> Vec<u8> {
    let mut pitches = Vec::new();
    for note in notes {
        let pitch = note % 12;
        if !pitches.contains(&pitch) {
            pitches.push(pitch);
        }
    }
    pitches.sort();
    pitches
}

fn kind_pitches(root: u8, kind: &ChordKind) -> Vec<u8> {
    let mut notes = Vec::new();
    for step in kind.steps {
        notes.push(root + step);
    }
    pitches_of(&notes)
}

// "chord": every chord the notes make, its root, its kind in words, and
// which note is lowest, which makes it an inversion when it isn't the root.
fn chord_of_notes(arguments: &Value) -> Result<Value, String> {
    let mut notes = notes_argument(arguments, "notes")?;
    notes.sort();
    let pitches = pitches_of(&notes);
    let lowest = notes[0] % 12;
    let mut chords = Vec::new();
    for root in &pitches {
        for kind in &CHORD_KINDS {
            if kind_pitches(*root, kind) != pitches {
                continue;
            }
            let root_name = COMMON_NAMES[*root as usize];
            let mut name = format!("{root_name}{}", kind.suffix);
            let mut lowest_is = String::from("root");
            if lowest != *root {
                name = format!("{name}/{}", COMMON_NAMES[lowest as usize]);
                lowest_is = inversion_of(*root, lowest, kind);
            }
            chords.push(json!({
                "name": name,
                "root": root_name,
                "kind": kind.words,
                "lowest": lowest_is,
            }));
        }
    }
    let mut names = Vec::new();
    for note in &notes {
        names.push(note_name(*note));
    }
    Ok(json!({"notes": names, "chords": chords}))
}

// Which of the chord's notes `lowest` is, as an inversion: the third makes
// the first inversion, the fifth the second, the seventh the third.
fn inversion_of(root: u8, lowest: u8, kind: &ChordKind) -> String {
    let above = (lowest + 12 - root) % 12;
    let words = ["root", "first inversion (third lowest)", "second inversion (fifth lowest)", "third inversion (seventh lowest)"];
    for (place, step) in kind.steps.iter().enumerate() {
        if step % 12 == above {
            if place < words.len() {
                return String::from(words[place]);
            }
            return String::from("an added note lowest");
        }
    }
    String::from("an added note lowest")
}

// A chord's name, like "Am7" or "Bbmaj7", as its root's pitch and its kind,
// or None.
fn parsed_chord(name: &str) -> Option<(u8, &'static ChordKind)> {
    let name = name.trim();
    let mut root_length = 1;
    let mut letters = name.chars();
    letters.next()?;
    if let Some(next) = letters.next() {
        if next == '#' || next == 'b' {
            root_length = 2;
        }
    }
    if name.len() < root_length {
        return None;
    }
    let root = pitch_of_name(&name[..root_length])?;
    let suffix = &name[root_length..];
    for kind in &CHORD_KINDS {
        if kind.suffix == suffix {
            return Some((root, kind));
        }
    }
    None
}

// The chords `arguments` list under `name`, parsed.
fn chords_argument(arguments: &Value, name: &str) -> Result<Vec<(String, u8, &'static ChordKind)>, String> {
    let Some(listed) = arguments[name].as_array() else {
        return Err(format!("{name} is needed, as a list of chords like [\"Am\", \"F\", \"G7\"]"));
    };
    if listed.is_empty() || listed.len() > 32 {
        return Err(format!("{name} has 1 to 32 chords, not {}", listed.len()));
    }
    let mut chords = Vec::new();
    for listed_chord in listed {
        let text = listed_chord.as_str().unwrap_or_default();
        let Some((root, kind)) = parsed_chord(text) else {
            return Err(unknown_chord(text));
        };
        chords.push((String::from(text.trim()), root, kind));
    }
    Ok(chords)
}

fn unknown_chord(text: &str) -> String {
    let mut suffixes = Vec::new();
    for kind in &CHORD_KINDS {
        suffixes.push(format!("\"{}\"", kind.suffix));
    }
    format!("{text} is not a chord Jello knows: a root like \"A\" or \"Bb\", then one of {}", suffixes.join(", "))
}

// The key `arguments` name, or the song's.
fn key_argument(studio: &Studio, arguments: &Value) -> Result<Key, String> {
    match optional_text_argument(arguments, "key")? {
        Some(name) => match key_of_name(&name) {
            Some(key) => Ok(key),
            None => Err(format!("{name} is not a key, like \"A minor\" or \"Db major\"")),
        },
        None => match studio.composer.lock().unwrap().key() {
            Some(key) => Ok(key),
            None => Err(String::from("the song has no key yet, so name one")),
        },
    }
}

// "numerals": each chord's roman numeral in the key, capitals when its
// third is major, with its kind after it, like "V7", "ii" or "vii°".
fn numerals(studio: &Studio, arguments: &Value) -> Result<Value, String> {
    let key = key_argument(studio, arguments)?;
    let chords = chords_argument(arguments, "chords")?;
    let mut numbered = Vec::new();
    for (name, root, kind) in chords {
        let steps = (root + 12 - key.tonic) % 12;
        let numeral = numeral_with_kind(NUMERALS[steps as usize], kind);
        numbered.push(json!({"chord": name, "numeral": numeral, "kind": kind.words}));
    }
    Ok(json!({"key": key_name(key), "chords": numbered}))
}

// `numeral` written for `kind`: small letters when the third is minor, and
// the kind's own mark after it.
fn numeral_with_kind(numeral: &str, kind: &ChordKind) -> String {
    let minor_third = kind.steps.contains(&3);
    let mut written = String::from(numeral);
    if minor_third {
        written = numeral.to_lowercase();
    }
    let mark = match kind.suffix {
        "" | "m" => "",
        "dim" => "°",
        "aug" => "+",
        "m7" => "7",
        "m6" => "6",
        "m9" => "9",
        "madd9" => "add9",
        "m7b5" => "ø7",
        "dim7" => "°7",
        "mMaj7" => "maj7",
        other => other,
    };
    format!("{written}{mark}")
}

// "scale": the scale's notes spelled with one letter each where it has
// seven, its steps, and for a seven-note scale the chord on each degree,
// three notes and four.
fn scale(arguments: &Value) -> Result<Value, String> {
    let tonic_name = text_argument(arguments, "tonic")?;
    let Some(tonic) = pitch_of_name(tonic_name.trim()) else {
        return Err(format!("{tonic_name} is not a note's name, like \"D\" or \"Eb\""));
    };
    let scale_name = text_argument(arguments, "scale")?;
    let mut found = None;
    for kind in &SCALES {
        if kind.name == scale_name.trim() {
            found = Some(kind);
        }
    }
    let Some(kind) = found else {
        let mut names = Vec::new();
        for kind in &SCALES {
            names.push(kind.name);
        }
        return Err(format!("there is no scale called {scale_name}: there are {}", names.join(", ")));
    };

    let names = spelled_scale(tonic_name.trim(), tonic, kind.steps);
    let mut answer = json!({
        "tonic": tonic_name.trim(),
        "scale": kind.name,
        "notes": names,
        "steps": kind.steps,
    });
    if kind.steps.len() == 7 {
        let mut triads = Vec::new();
        let mut sevenths = Vec::new();
        for degree in 0..7 {
            triads.push(chord_on_degree(&names, kind.steps, degree, 3));
            sevenths.push(chord_on_degree(&names, kind.steps, degree, 4));
        }
        answer["chords"] = json!(triads);
        answer["sevenths"] = json!(sevenths);
    }
    Ok(answer)
}

// The scale's notes' names from `tonic_name`. A seven-note scale gives each
// letter once, so F major has Bb, not A#. Others take flats from a flat
// home note or F, and sharps otherwise.
fn spelled_scale(tonic_name: &str, tonic: u8, steps: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    if steps.len() != 7 {
        let mut table = SHARP_NAMES;
        if tonic_name.contains('b') || tonic_name == "F" {
            table = FLAT_NAMES;
        }
        for step in steps {
            names.push(String::from(table[((tonic + step) % 12) as usize]));
        }
        return names;
    }
    let first_letter = tonic_name.chars().next().unwrap_or('C').to_string();
    let mut letter_place = 0;
    for (place, letter) in LETTERS.iter().enumerate() {
        if **letter == first_letter {
            letter_place = place;
        }
    }
    for (degree, step) in steps.iter().enumerate() {
        let letter = (letter_place + degree) % 7;
        let natural = LETTER_PITCHES[letter];
        let wanted = (tonic + step) % 12;
        let off = (wanted + 12 - natural) % 12;
        let accidental = match off {
            0 => "",
            1 => "#",
            2 => "##",
            10 => "bb",
            11 => "b",
            _ => "?",
        };
        names.push(format!("{}{accidental}", LETTERS[letter]));
    }
    names
}

// The chord of `size` notes built on `degree` (from 0) of a seven-note
// scale, stacking every other note: its name and its numeral.
fn chord_on_degree(names: &[String], steps: &[u8], degree: usize, size: usize) -> Value {
    let root = steps[degree];
    let mut above = Vec::new();
    for stacked in 0..size {
        let place = degree + stacked * 2;
        let mut step = steps[place % 7];
        if place >= 7 {
            step += 12;
        }
        above.push((step + 24 - root) % 12);
    }
    let mut found = None;
    for kind in &CHORD_KINDS {
        let mut kind_steps = Vec::new();
        for step in kind.steps {
            kind_steps.push(step % 12);
        }
        if kind_steps == above {
            found = Some(kind);
        }
    }
    match found {
        Some(kind) => json!({
            "name": format!("{}{}", names[degree], kind.suffix),
            "numeral": numeral_with_kind(ROMAN[degree], kind),
            "kind": kind.words,
        }),
        None => json!({"name": names[degree], "numeral": ROMAN[degree], "kind": "none Jello names"}),
    }
}

// "interval": from one note to another, how many half steps and its name;
// or with half_steps, the note that far above (or below, negative).
fn interval(arguments: &Value) -> Result<Value, String> {
    let from_name = text_argument(arguments, "from")?;
    let Some(from) = note_of_name(&from_name) else {
        return Err(format!("{from_name} is not a note with its octave, like \"C4\""));
    };
    if let Some(half_steps) = arguments["half_steps"].as_i64() {
        let to = i64::from(from) + half_steps;
        if !(0..=127).contains(&to) {
            return Err(format!("{half_steps} half steps from {from_name} is outside every note"));
        }
        let to = to as u8;
        return Ok(json!({
            "from": note_name(from),
            "to": note_name(to),
            "half_steps": half_steps,
            "name": interval_name(half_steps.unsigned_abs() as u32),
        }));
    }
    let to_name = text_argument(arguments, "to")?;
    let Some(to) = note_of_name(&to_name) else {
        return Err(format!("{to_name} is not a note with its octave, like \"E4\""));
    };
    let half_steps = i64::from(to) - i64::from(from);
    let mut direction = "up";
    if half_steps < 0 {
        direction = "down";
    }
    Ok(json!({
        "from": note_name(from),
        "to": note_name(to),
        "half_steps": half_steps,
        "direction": direction,
        "name": interval_name(half_steps.unsigned_abs() as u32),
    }))
}

// An interval's name, like "minor third", or "octave and a major third"
// past an octave.
fn interval_name(half_steps: u32) -> String {
    if half_steps <= 12 {
        return String::from(INTERVAL_NAMES[half_steps as usize]);
    }
    let octaves = half_steps / 12;
    let rest = half_steps % 12;
    let mut octave_words = String::from("octave");
    if octaves > 1 {
        octave_words = format!("{octaves} octaves");
    }
    if rest == 0 {
        return octave_words;
    }
    format!("{octave_words} and a {}", INTERVAL_NAMES[rest as usize])
}

// "voice_leading": each note of the first chord moved to the nearest note
// of the next, so the voices move as little as they can in all, with
// common notes staying. `from` is notes with octaves or a chord's name
// (played from C3 up, like the chord keys); `to` is a chord's name or
// notes.
fn voice_leading(arguments: &Value) -> Result<Value, String> {
    let from = voiced(arguments, "from")?;
    let to = pitches_of(&voiced(arguments, "to")?);
    if to.len() > from.len() {
        return Err(format!(
            "the next chord has {} different notes, more than the {} voices of the first",
            to.len(),
            from.len()
        ));
    }

    let mut best: Option<(u32, Vec<i32>)> = None;
    let mut choice = vec![0; from.len()];
    loop {
        if uses_every(&choice, to.len()) {
            let mut moves = Vec::new();
            let mut total = 0;
            for (voice, picked) in choice.iter().enumerate() {
                let moved = nearest_move(from[voice], to[*picked]);
                total += moved.unsigned_abs();
                moves.push(moved);
            }
            let better = match &best {
                Some((least, _)) => total < *least,
                None => true,
            };
            if better {
                best = Some((total, moves));
            }
        }
        if !next_choice(&mut choice, to.len()) {
            break;
        }
    }
    let Some((total, moves)) = best else {
        return Err(String::from("the chords can't be joined"));
    };

    let mut voices = Vec::new();
    let mut staying = 0;
    for (voice, moved) in moves.iter().enumerate() {
        let landed = (i32::from(from[voice]) + moved) as u8;
        if *moved == 0 {
            staying += 1;
        }
        voices.push(json!({
            "from": note_name(from[voice]),
            "to": note_name(landed),
            "moves": moved,
        }));
    }
    Ok(json!({"voices": voices, "total_half_steps": total, "notes_staying": staying}))
}

// The notes under `name`: a list of notes, or a chord's name voiced from
// its root at C3 to B3.
fn voiced(arguments: &Value, name: &str) -> Result<Vec<u8>, String> {
    if arguments[name].is_array() {
        let mut notes = notes_argument(arguments, name)?;
        notes.sort();
        return Ok(notes);
    }
    let text = text_argument(arguments, name)?;
    let Some((root, kind)) = parsed_chord(&text) else {
        return Err(unknown_chord(&text));
    };
    let mut notes = Vec::new();
    for step in kind.steps {
        notes.push(48 + root + step);
    }
    Ok(notes)
}

// The move, in half steps, from `note` to the nearest note of `pitch`: up
// to 6 up or 5 down.
fn nearest_move(note: u8, pitch: u8) -> i32 {
    let up = i32::from((pitch + 12 - note % 12) % 12);
    if up > 6 {
        return up - 12;
    }
    up
}

// Whether `choice` uses each of `count` pitches at least once.
fn uses_every(choice: &[usize], count: usize) -> bool {
    for pitch in 0..count {
        if !choice.contains(&pitch) {
            return false;
        }
    }
    true
}

// Counts `choice` on to the next way of picking a pitch for every voice.
// False once every way has been tried.
fn next_choice(choice: &mut [usize], count: usize) -> bool {
    for place in 0..choice.len() {
        choice[place] += 1;
        if choice[place] < count {
            return true;
        }
        choice[place] = 0;
    }
    false
}

// "keys": which laptop key plays each note, or the chord, at the current
// octave; a note out of reach says how many octaves to move, with the
// arrows.
fn keys_for(studio: &Studio, arguments: &Value) -> Result<Value, String> {
    let key_map = studio.key_map.lock().unwrap();
    let octave = key_map.octave();
    if let Some(name) = arguments["chord"].as_str() {
        let Some((root, kind)) = parsed_chord(name) else {
            return Err(unknown_chord(name));
        };
        let mut found = None;
        for (key, chord) in key_map.chord_keys() {
            if chord.pitch() != root {
                continue;
            }
            let chord_steps = [0, chord.notes()[1] - chord.notes()[0], 7];
            if kind.steps == chord_steps {
                found = Some(key);
            }
        }
        return match found {
            Some(key) => Ok(json!({"octave": octave, "chord": name, "key": key})),
            None => Ok(json!({
                "octave": octave,
                "chord": name,
                "key": Value::Null,
                "why": "the chord keys play only major and minor chords: play its notes on the melody keys, or a major or minor chord inside it",
            })),
        };
    }

    let notes = notes_argument(arguments, "notes")?;
    let note_keys = key_map.note_keys();
    let mut lowest = 127;
    let mut highest = 0;
    for note in note_keys.values() {
        lowest = lowest.min(*note);
        highest = highest.max(*note);
    }
    let mut found = Vec::new();
    for note in notes {
        let mut key = Value::Null;
        for (code, played) in &note_keys {
            if *played == note {
                key = json!(code);
            }
        }
        // Each octave moves the keys 12 half steps, so as many as it takes
        // to bring the note inside their range.
        let mut move_octaves = 0;
        if note < lowest {
            let below = i32::from(lowest - note);
            move_octaves = -((below + 11) / 12);
        } else if note > highest {
            let above = i32::from(note - highest);
            move_octaves = (above + 11) / 12;
        }
        found.push(json!({"note": note_name(note), "key": key, "octaves_to_move": move_octaves}));
    }
    Ok(json!({
        "octave": octave,
        "melody_range": [note_name(lowest), note_name(highest)],
        "notes": found,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(answer: &Value, field: &str) -> Vec<String> {
        let mut names = Vec::new();
        for item in answer[field].as_array().unwrap() {
            names.push(String::from(item["name"].as_str().unwrap()));
        }
        names
    }

    #[test]
    fn notes_name_their_chord_and_inversion() {
        let answer = chord_of_notes(&json!({"notes": ["E4", "G4", "C5"]})).unwrap();
        assert_eq!(named(&answer, "chords"), vec!["C/E"]);
        assert_eq!(answer["chords"][0]["lowest"], "first inversion (third lowest)");

        // C E G A is both C6 and Am7 over C.
        let answer = chord_of_notes(&json!({"notes": ["C4", "E4", "G4", "A4"]})).unwrap();
        assert_eq!(named(&answer, "chords"), vec!["C6", "Am7/C"]);

        // B D F A is Bm7b5, and Dm6 over B.
        let answer = chord_of_notes(&json!({"notes": ["B3", "D4", "F4", "A4"]})).unwrap();
        assert_eq!(named(&answer, "chords"), vec!["Dm6/B", "Bm7b5"]);
        assert!(chord_of_notes(&json!({"notes": ["C"]})).is_err());
    }

    #[test]
    fn scales_are_spelled_with_one_letter_each() {
        let answer = scale(&json!({"tonic": "F", "scale": "major"})).unwrap();
        assert_eq!(answer["notes"], json!(["F", "G", "A", "Bb", "C", "D", "E"]));
        let chords: Vec<String> = named(&answer, "chords");
        assert_eq!(chords, vec!["F", "Gm", "Am", "Bb", "C", "Dm", "Edim"]);
        assert_eq!(answer["chords"][6]["numeral"], "vii°");
        assert_eq!(answer["sevenths"][4]["numeral"], "V7");
        assert_eq!(answer["sevenths"][0]["name"], "Fmaj7");

        let answer = scale(&json!({"tonic": "C#", "scale": "harmonic minor"})).unwrap();
        assert_eq!(answer["notes"], json!(["C#", "D#", "E", "F#", "G#", "A", "B#"]));
        assert_eq!(answer["chords"][4]["numeral"], "V");

        let answer = scale(&json!({"tonic": "A", "scale": "minor pentatonic"})).unwrap();
        assert_eq!(answer["notes"], json!(["A", "C", "D", "E", "G"]));
        assert!(answer.get("chords").is_none());
        let wrong = scale(&json!({"tonic": "A", "scale": "dorian flat"})).unwrap_err();
        assert!(wrong.starts_with("there is no scale called dorian flat"));
    }

    #[test]
    fn intervals_are_counted_and_named() {
        let answer = interval(&json!({"from": "C4", "to": "Eb4"})).unwrap();
        assert_eq!(answer["half_steps"], 3);
        assert_eq!(answer["name"], "minor third");
        let answer = interval(&json!({"from": "C4", "to": "E5"})).unwrap();
        assert_eq!(answer["name"], "octave and a major third");
        let answer = interval(&json!({"from": "G4", "half_steps": -7})).unwrap();
        assert_eq!(answer["to"], "C4");
        assert_eq!(answer["name"], "perfect fifth");
    }

    #[test]
    fn voices_move_as_little_as_they_can() {
        // C to F: C stays, E up to F, G up to A.
        let answer = voice_leading(&json!({"from": "C", "to": "F"})).unwrap();
        assert_eq!(answer["total_half_steps"], 3);
        assert_eq!(answer["notes_staying"], 1);
        assert_eq!(answer["voices"][1], json!({"from": "E3", "to": "F3", "moves": 1}));

        // G7 to C: the leading tone B rises to C, F falls to E.
        let answer = voice_leading(&json!({"from": ["G3", "B3", "D4", "F4"], "to": "C"})).unwrap();
        assert_eq!(answer["voices"][1]["to"], "C4");
        assert_eq!(answer["voices"][3]["to"], "E4");
        let too_many = voice_leading(&json!({"from": "C5", "to": "G7"})).unwrap_err();
        assert!(too_many.starts_with("the next chord has 4 different notes"));
    }

    #[test]
    fn keys_out_of_reach_say_how_far_to_move() {
        use crate::cards::Offers;
        use crate::compose::Composer;
        use crate::engine::Engine;
        use crate::key_map::KeyMap;
        use crate::phrases::Phrases;
        use crate::song_session::OpenSong;
        use crate::tools::Screen;
        use std::sync::Mutex;

        struct NoScreen;
        impl Screen for NoScreen {
            fn ask(&self, _question: &str) -> bool {
                false
            }
            fn show_key(&self, _key: &str, _down: bool) {}
        }
        let open_song = Mutex::new(OpenSong::new(None));
        let key_map = Mutex::new(KeyMap::new());
        let composer = Mutex::new(Composer::new());
        let engine = Mutex::new(Engine::new().unwrap());
        let phrases = Phrases::default();
        let offers = Mutex::new(Offers::default());
        let practice = Mutex::new(crate::practice::Practice::default());
        let studio = Studio {
            open_song: &open_song,
            key_map: &key_map,
            composer: &composer,
            engine: &engine,
            phrases: &phrases,
            offers: &offers,
            practice: &practice,
            screen: &NoScreen,
        };
        let answer = keys_for(&studio, &json!({"notes": ["C4", "G5", "A5", "C2"]})).unwrap();
        assert_eq!(answer["melody_range"], json!(["G#3", "G5"]));
        assert_eq!(answer["notes"][0]["key"], "KeyM");
        assert_eq!(answer["notes"][1]["octaves_to_move"], 0);
        assert_eq!(answer["notes"][2]["octaves_to_move"], 1);
        assert_eq!(answer["notes"][3]["octaves_to_move"], -2);
        let answer = keys_for(&studio, &json!({"chord": "Am"})).unwrap();
        assert_eq!(answer["key"], "KeyQ");
        let answer = keys_for(&studio, &json!({"chord": "G7"})).unwrap();
        assert_eq!(answer["key"], Value::Null);
    }

    #[test]
    fn numerals_carry_the_chord_s_kind() {
        let kinds = [("", "V"), ("7", "V7"), ("m7", "v7"), ("dim", "v°"), ("m7b5", "vø7")];
        for (suffix, expected) in kinds {
            let kind = CHORD_KINDS.iter().find(|kind| kind.suffix == suffix).unwrap();
            assert_eq!(numeral_with_kind("V", kind), expected);
        }
        assert!(parsed_chord("Bbmaj7").is_some());
        assert!(parsed_chord("H7").is_none());
        assert!(parsed_chord("Cfoo").is_none());
    }
}
