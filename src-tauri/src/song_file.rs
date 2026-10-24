// A whole song written as text (docs/adr/0038-song-files.md): its key, its
// chords with how long each was held, its tempo, the loop, and its sound.
// Like a sound file (src/sound_file.rs) the text is TOML, and every choice
// is a word: the key is "A minor", a chord "Am", a note "E4". The sound is
// a sound file's text, under [sound].
//
// The chords keep their milliseconds, as held: Compose's chords are played
// freely, not on a beat. The loop is whole bars at the song's tempo, so its
// notes say the beat they start and end on, counted from the loop's start.
// That reads the same on any sound card.
//
// Reading is forgiving, like a sound file's:
// - A value left out, or of the wrong sort, takes a new song's.
// - A chord or a note Jello can't read is skipped, and so is a layer left
//   with no notes.
// - A loop keeps its first 8 layers and its first 4096 notes.
// - The tempo is kept from 40 to 240 and a layer's volume to 100.
// - A pad Jello doesn't know is skipped, and a pad's values are kept
//   within their ranges.
// Text that is not TOML at all can't be read.

use toml::{Table, Value};

use crate::drums::{DrumPad, Drums, PAD_NAMES, pad_named, pad_of_note};
use crate::engine::{
    FASTEST_TEMPO, FULL_VOLUME, GLIDE_KEY_NOTE, MAX_LAYERS, SLOWEST_TEMPO, START_TEMPO, drum_note,
    VIBRATO_KEY_NOTE,
};
use crate::presets::name_problem;
use crate::harmony::{Chord, Key, chord_name, chord_of_name, key_name, key_of_name, pitch_of_name};
use crate::sound_file::{
    Sound, group, groups, number_in, read_count, read_flag, read_signed, read_word, sound_of_table, start_group,
    write_flag, write_name, write_number, write_sound, write_word,
};
use crate::sound_profile::SoundProfile;

// The most notes a loop holds, in all its layers (`JELLO_LOOP_MAX_NOTES` in
// engine/include/jello_engine.h).
pub const LOOP_MAX_NOTES: usize = 4096;

// How a loop note's beats are written: to a millionth of a beat, far finer
// than a sample even at the fastest tempo, so a loop opens exactly as it
// was saved.
const BEAT_DECIMALS: usize = 6;

// The names of the notes in an octave, from C up, with sharps.
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

// The words for the presses of the vibrato and glide keys.
const VIBRATO_KEY_WORD: &str = "vibrato key";
const GLIDE_KEY_WORD: &str = "glide key";

// The first line of every song file, for whoever opens one.
const HEADING: &str = "# A Jello song. Any value left out takes a new song's.";

// Everything a song file holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    // None until a key is picked.
    pub key: Option<Key>,
    pub tempo: u32,
    pub snap: bool,
    // The preset the song's sound came from, so its tile lights up again if
    // the sound still matches it. None if there is none.
    pub preset: Option<String>,
    pub chords: Vec<HeldChord>,
    // None for a song with no loop.
    pub looped: Option<SongLoop>,
    pub sound: Sound,
    // The drum kit's instrument and how each pad sounds.
    pub drums: Drums,
}

// A chord of the song and how long it was held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldChord {
    pub chord: Chord,
    pub held_ms: u64,
}

// The loop: how many bars it lasts, and its layers in the order they were
// recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct SongLoop {
    pub bars: u32,
    pub layers: Vec<SongLayer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SongLayer {
    // Zero inherits the common loop; otherwise this layer repeats every bars.
    pub bars: u32,
    // In percent, from 0 to 100.
    pub volume: u32,
    // False while it is paused.
    pub playing: bool,
    pub notes: Vec<BeatNote>,
}

// One note of the loop, in beats from the loop's start. Its pitch is 0 to
// 127, VIBRATO_KEY_NOTE or GLIDE_KEY_NOTE for a press of those keys, or a
// drum pad's note for a hit (src/drums.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatNote {
    pub note: u8,
    pub start: f64,
    pub end: f64,
}

// How many beats `samples` samples last at `tempo`, at `sample_rate`.
pub fn beats_of(samples: u64, tempo: u32, sample_rate: u32) -> f64 {
    let samples_per_beat = f64::from(sample_rate) * 60.0 / f64::from(tempo);
    samples as f64 / samples_per_beat
}

// The sample `beats` beats fall on at `tempo`, at `sample_rate`, to the
// nearest one.
pub fn samples_of(beats: f64, tempo: u32, sample_rate: u32) -> u64 {
    let samples_per_beat = f64::from(sample_rate) * 60.0 / f64::from(tempo);
    let samples = (beats * samples_per_beat).round();
    if samples < 0.0 {
        return 0;
    }
    samples as u64
}

// A note's name, like "C4" for middle C or "F#3", the word for a press
// of the vibrato or glide key, or a drum's name, like "kick".
pub fn note_name(note: u8) -> String {
    if let Some(pad) = pad_of_note(note) {
        return String::from(PAD_NAMES[pad]);
    }
    if note == VIBRATO_KEY_NOTE {
        return String::from(VIBRATO_KEY_WORD);
    }
    if note == GLIDE_KEY_NOTE {
        return String::from(GLIDE_KEY_WORD);
    }
    let octave = i32::from(note / 12) - 1;
    let name = NOTE_NAMES[usize::from(note % 12)];
    format!("{name}{octave}")
}

// The note a name gives, as note_name writes it, or None. Either name of a
// black key works, so "Bb3" is "A#3".
pub fn note_of_name(name: &str) -> Option<u8> {
    let name = name.trim();
    if name == VIBRATO_KEY_WORD {
        return Some(VIBRATO_KEY_NOTE);
    }
    if name == GLIDE_KEY_WORD {
        return Some(GLIDE_KEY_NOTE);
    }
    if let Some(pad) = pad_named(name) {
        return Some(drum_note(pad));
    }

    // The octave starts at its first digit, or at a minus sign: C-1 is the
    // lowest note.
    let mut octave_starts = None;
    for (place, letter) in name.char_indices() {
        if letter.is_ascii_digit() || letter == '-' {
            octave_starts = Some(place);
            break;
        }
    }
    let octave_starts = octave_starts?;
    let octave: i32 = name[octave_starts..].parse().ok()?;

    // The letter's own note, then its sharp or flat, which can cross into
    // the next octave: Cb4 is B3, and B#3 is C4.
    let mut letters = name[..octave_starts].chars();
    let letter = letters.next()?;
    let natural = pitch_of_name(&letter.to_string())?;
    let rest: String = letters.collect();
    let shift = match rest.as_str() {
        "" => 0,
        "#" => 1,
        "b" => -1,
        _ => return None,
    };

    let note = (octave + 1) * 12 + i32::from(natural) + shift;
    if note < 0 {
        return None;
    }
    if note > 127 {
        return None;
    }
    Some(note as u8)
}

// The pitch a note name gives, from 0 to 127, as note_of_name reads it, or
// None for anything else: a drum or a press of the vibrato or glide key.
// Lanes, cards and practice take only these.
pub fn pitch_of_note_name(name: &str) -> Option<u8> {
    let note = note_of_name(name)?;
    if note > 127 {
        return None;
    }
    Some(note)
}

// `song` as text. Every value is written.
pub fn text_of(song: &Song) -> String {
    let mut text = String::new();
    text.push_str(HEADING);
    text.push('\n');

    // Single values come first: TOML wants them before any heading.
    text.push('\n');
    if let Some(key) = song.key {
        write_word(&mut text, "key", &key_name(key));
    }
    write_number(&mut text, "tempo", song.tempo);
    write_flag(&mut text, "snap", song.snap);
    if let Some(preset) = &song.preset {
        write_word(&mut text, "preset", preset);
    }

    // One chord a line, named for the key.
    text.push('\n');
    if song.chords.is_empty() {
        text.push_str("chords = []\n");
    } else {
        text.push_str("chords = [\n");
        for held in &song.chords {
            let name = chord_name(held.chord, song.key);
            let mut root = String::new();
            if !(48..=59).contains(&held.chord.root) {
                root = format!(", root_note = \"{}\"", note_name(held.chord.root));
            }
            text.push_str(&format!("  {{ chord = \"{name}\", held_ms = {}{root} }},\n", held.held_ms));
        }
        text.push_str("]\n");
    }

    if let Some(looped) = &song.looped {
        write_loop(&mut text, looped);
    }

    write_sound(&mut text, &song.sound, "sound");
    write_drums(&mut text, &song.drums);
    text
}

// The drums: the kit's instrument, if one is picked, then one pad a line.
fn write_drums(text: &mut String, drums: &Drums) {
    start_group(text, "[drums]");
    if let Some(kit) = &drums.kit {
        write_name(text, "kit", kit);
    }
    text.push_str("pads = [\n");
    for (pad, settings) in drums.pads.iter().enumerate() {
        let name = PAD_NAMES[pad];
        text.push_str(&format!(
            "  {{ pad = \"{name}\", level = {}, half_steps = {}, pan = {}, group = {} }},\n",
            settings.level, settings.half_steps, settings.pan, settings.group
        ));
    }
    text.push_str("]\n");
}

// The drums `table`, a song file's [drums], describes, starting from a new
// song's.
fn read_drums(table: &Table) -> Drums {
    let mut drums = Drums::new();
    if let Some(kit) = read_word(table, "kit") {
        drums.kit = Some(String::from(kit));
    }
    for item in items(table, "pads") {
        let Some(name) = read_word(item, "pad") else {
            continue;
        };
        let Some(pad) = pad_named(name) else {
            continue;
        };
        let missing = drums.pads[pad];
        let settings = DrumPad {
            level: read_count(item, "level", missing.level),
            half_steps: read_signed(item, "half_steps", missing.half_steps),
            pan: read_signed(item, "pan", missing.pan),
            group: read_count(item, "group", missing.group),
        };
        drums.pads[pad] = settings.within_ranges();
    }
    drums
}

fn write_loop(text: &mut String, looped: &SongLoop) {
    start_group(text, "[loop]");
    write_number(text, "bars", looped.bars);

    for layer in &looped.layers {
        start_group(text, "[[loop.layer]]");
        if layer.bars > 0 {
            write_number(text, "bars", layer.bars);
        }
        write_number(text, "volume", layer.volume);
        write_flag(text, "playing", layer.playing);
        text.push_str("notes = [\n");
        for note in &layer.notes {
            let name = note_name(note.note);
            let start = beat_text(note.start);
            let end = beat_text(note.end);
            text.push_str(&format!("  {{ note = \"{name}\", start = {start}, end = {end} }},\n"));
        }
        text.push_str("]\n");
    }
}

// A beat to a millionth, without the zeros at the end: "5.25", not
// "5.250000", and "2", not "2.000000".
fn beat_text(beat: f64) -> String {
    let mut text = format!("{beat:.BEAT_DECIMALS$}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

// The song `text` describes, with `new_song`'s sound for whatever its sound
// leaves out. Fails only on text that is not TOML.
pub fn song_of(text: &str, new_song: &SoundProfile) -> Result<Song, String> {
    let table = match text.parse::<Table>() {
        Ok(table) => table,
        Err(error) => return Err(format!("it is not a song file: {}", error.message())),
    };

    let mut key = None;
    if let Some(name) = read_word(&table, "key") {
        key = key_of_name(name);
    }

    let tempo = read_count(&table, "tempo", START_TEMPO);
    let tempo = tempo.clamp(SLOWEST_TEMPO, FASTEST_TEMPO);
    let snap = read_flag(&table, "snap", true);

    // Only a name a preset could have, so it is written back as it was.
    let mut preset = None;
    if let Some(name) = read_word(&table, "preset") {
        if name_problem(name).is_none() {
            preset = Some(String::from(name));
        }
    }

    let mut chords = Vec::new();
    for item in items(&table, "chords") {
        if let Some(held) = read_chord(item) {
            chords.push(held);
        }
    }

    let mut looped = None;
    if let Some(loop_table) = group(&table, "loop") {
        looped = read_loop(loop_table);
    }

    // A file with no [sound] keeps a new song's whole sound, its routing
    // list included.
    let sound = match group(&table, "sound") {
        Some(sound_table) => sound_of_table(sound_table, new_song),
        None => new_song.sound(),
    };

    let drums = match group(&table, "drums") {
        Some(drums_table) => read_drums(drums_table),
        None => Drums::new(),
    };

    Ok(Song {
        key,
        tempo,
        snap,
        preset,
        chords,
        looped,
        sound,
        drums,
    })
}

// The tables in the list called `name`, like the chords, in order. Anything
// in it that is not a table is skipped.
fn items<'a>(table: &'a Table, name: &str) -> Vec<&'a Table> {
    let mut found = Vec::new();
    if let Some(Value::Array(list)) = table.get(name) {
        for item in list {
            if let Value::Table(entry) = item {
                found.push(entry);
            }
        }
    }
    found
}

// A chord of the list, or None if its name or its length can't be read.
fn read_chord(item: &Table) -> Option<HeldChord> {
    let name = read_word(item, "chord")?;
    let mut chord = chord_of_name(name)?;
    if let Some(root_name) = read_word(item, "root_note") {
        let root = note_of_name(root_name)?;
        if root > 120 || root % 12 != chord.pitch() {
            return None;
        }
        chord.root = root;
    }
    let held = item.get("held_ms")?;
    let held_ms = number_in(held)?;
    if held_ms.is_nan() {
        return None;
    }
    if held_ms < 0.0 {
        return None;
    }
    Some(HeldChord {
        chord,
        held_ms: held_ms.round() as u64,
    })
}

// The loop, or None if it has no bars or no layer with a note.
fn read_loop(loop_table: &Table) -> Option<SongLoop> {
    let bars = read_count(loop_table, "bars", 0);
    if bars < 1 {
        return None;
    }

    let mut layers = Vec::new();
    let mut note_count = 0;
    for layer_table in groups(loop_table, "layer") {
        if layers.len() == MAX_LAYERS {
            break;
        }

        let mut notes = Vec::new();
        for item in items(layer_table, "notes") {
            if note_count == LOOP_MAX_NOTES {
                break;
            }
            if let Some(note) = read_note(item) {
                notes.push(note);
                note_count += 1;
            }
        }
        if notes.is_empty() {
            continue;
        }

        let volume = read_count(layer_table, "volume", FULL_VOLUME);
        let volume = volume.min(FULL_VOLUME);
        let playing = read_flag(layer_table, "playing", true);
        layers.push(SongLayer {
            bars: read_count(layer_table, "bars", 0).min(64),
            volume,
            playing,
            notes,
        });
    }

    if layers.is_empty() {
        return None;
    }
    Some(SongLoop { bars, layers })
}

// A note of a layer, or None if its name or its beats can't be read, or it
// ends before it starts.
fn read_note(item: &Table) -> Option<BeatNote> {
    let name = read_word(item, "note")?;
    let note = note_of_name(name)?;
    let start = beat_in(item, "start")?;
    let end = beat_in(item, "end")?;
    if end < start {
        return None;
    }
    Some(BeatNote { note, start, end })
}

// A beat from 0 up, or None.
fn beat_in(item: &Table, name: &str) -> Option<f64> {
    let beat = number_in(item.get(name)?)?;
    if beat.is_nan() {
        return None;
    }
    if beat < 0.0 {
        return None;
    }
    Some(beat)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::drums::{DRUM_MOST_GROUP, DRUM_MOST_HALF_STEPS};
    use crate::engine::{Kind, SLOWEST_TEMPO};
    use crate::harmony::{Mode, Quality};

    fn major(root: u8) -> Chord {
        Chord {
            root,
            quality: Quality::Major,
        }
    }

    fn minor(root: u8) -> Chord {
        Chord {
            root,
            quality: Quality::Minor,
        }
    }

    fn note(note: u8, start: f64, end: f64) -> BeatNote {
        BeatNote { note, start, end }
    }

    // A song with every part filled in and moved away from a new song's.
    fn full_song() -> Song {
        let mut sound = SoundProfile::new().sound();
        sound.tuning_hz = 432.5;
        sound.oscillators[0].kind = Kind::Saw;
        sound.filter.cutoff_hz = 1234.5;
        sound.oscillators[1].kind = Kind::Sample;
        sound.instruments[1] = Some(String::from("The \"Big\" \\ piano"));

        let mut drums = Drums::new();
        drums.kit = Some(String::from("Black Pearl"));
        drums.pads[1] = DrumPad {
            level: 70,
            half_steps: -2,
            pan: 30,
            group: 2,
        };

        let first = SongLayer {
            bars: 0,
            volume: 80,
            playing: true,
            notes: vec![
                note(64, 0.0, 0.5),
                note(VIBRATO_KEY_NOTE, 1.0, 2.0),
                note(drum_note(0), 1.0, 1.1),
                note(drum_note(4), 1.5, 1.6),
                note(67, 5.237396, 5.75),
            ],
        };
        let second = SongLayer {
            bars: 0,
            volume: 35,
            playing: false,
            notes: vec![note(GLIDE_KEY_NOTE, 0.25, 0.5), note(70, 2.0, 7.999999)],
        };
        Song {
            key: Some(Key {
                tonic: 9,
                mode: Mode::Minor,
            }),
            tempo: 96,
            snap: false,
            preset: Some(String::from("Warm pad")),
            chords: vec![
                HeldChord {
                    chord: minor(57),
                    held_ms: 1250,
                },
                HeldChord {
                    chord: major(53),
                    held_ms: 980,
                },
                HeldChord {
                    chord: major(58),
                    held_ms: 3,
                },
            ],
            looped: Some(SongLoop {
                bars: 2,
                layers: vec![first, second],
            }),
            sound,
            drums,
        }
    }

    fn read(text: &str) -> Song {
        song_of(text, &SoundProfile::new()).unwrap()
    }

    #[test]
    fn a_song_reads_back_exactly_as_it_was_written() {
        let song = full_song();
        assert_eq!(read(&text_of(&song)), song);

        // And a new song's, with no key, chords or loop.
        let empty = read("");
        assert_eq!(read(&text_of(&empty)), empty);
    }

    #[test]
    fn the_text_says_everything_in_words() {
        let text = text_of(&full_song());
        assert!(text.starts_with("# A Jello song."));
        assert!(text.contains("key = \"A minor\"\n"));
        assert!(text.contains("tempo = 96\n"));
        assert!(text.contains("snap = false\n"));
        assert!(text.contains("preset = \"Warm pad\"\n"));
        assert!(text.contains("  { chord = \"Am\", held_ms = 1250 },\n"));
        assert!(text.contains("  { chord = \"Bb\", held_ms = 3 },\n"));
        assert!(text.contains("[loop]\nbars = 2\n"));
        assert!(text.contains("[[loop.layer]]\nvolume = 80\nplaying = true\n"));
        assert!(text.contains("  { note = \"E4\", start = 0, end = 0.5 },\n"));
        assert!(text.contains("  { note = \"vibrato key\", start = 1, end = 2 },\n"));
        assert!(text.contains("  { note = \"glide key\", start = 0.25, end = 0.5 },\n"));
        assert!(text.contains("  { note = \"G4\", start = 5.237396, end = 5.75 },\n"));
        assert!(text.contains("[sound]\ntuning_hz = 432.5\n"));
        assert!(text.contains("[[sound.oscillators]]\nkind = \"saw\"\n"));
        assert!(text.contains("[sound.filter]\n"));
        assert!(text.contains("kind = \"sample\"\ninstrument = 'The \"Big\" \\ piano'\n"));
        assert!(text.contains("  { note = \"kick\", start = 1, end = 1.1 },\n"));
        assert!(text.contains("  { note = \"closed hat\", start = 1.5, end = 1.6 },\n"));
        assert!(text.contains("[drums]\nkit = \"Black Pearl\"\npads = [\n"));
        assert!(text.contains("  { pad = \"snare\", level = 70, half_steps = -2, pan = 30, group = 2 },\n"));
        assert!(text.contains("  { pad = \"open hat\", level = 100, half_steps = 0, pan = 0, group = 1 },\n"));
    }

    #[test]
    fn drums_are_read_by_pad_name_within_their_ranges() {
        let text = r#"
[drums]
kit = "Kit"
pads = [
  { pad = "Closed_Hat", level = 50, group = 0 },
  { pad = "cowbell", level = 10 },
  { pad = "ride", level = 900, half_steps = -40, pan = 300, group = 9 },
  { level = 20 },
]
"#;
        let song = read(text);
        let mut expected = Drums::new();
        expected.kit = Some(String::from("Kit"));
        expected.pads[4].level = 50;
        expected.pads[4].group = 0;
        expected.pads[12] = DrumPad {
            level: 100,
            half_steps: -DRUM_MOST_HALF_STEPS,
            pan: 100,
            group: DRUM_MOST_GROUP,
        };
        assert_eq!(song.drums, expected);

        // A song with no [drums] has a new song's, and hits read by name.
        assert_eq!(read("").drums, Drums::new());
        let hits = "[loop]\nbars = 1\n[[loop.layer]]\nnotes = [{ note = \"snare\", start = 0, end = 0.1 }, { note = \"cowbell\", start = 1, end = 2 }]\n";
        let looped = read(hits).looped.unwrap();
        assert_eq!(looped.layers[0].notes, vec![note(drum_note(1), 0.0, 0.1)]);
    }

    #[test]
    fn chords_are_spelled_for_the_key_and_read_in_either_spelling() {
        // C#3 major is Db in Db major, and C# with no key.
        let mut song = read("");
        song.chords = vec![HeldChord {
            chord: major(49),
            held_ms: 500,
        }];
        assert!(text_of(&song).contains("chord = \"C#\""));
        song.key = Some(Key {
            tonic: 1,
            mode: Mode::Major,
        });
        assert!(text_of(&song).contains("chord = \"Db\""));
        assert!(text_of(&song).contains("key = \"Db major\""));

        let text = "key = \"C# minor\"\nchords = [\n  { chord = \"A#m\", held_ms = 10 },\n  { chord = \"Bbm\", held_ms = 20 },\n]\n";
        let song = read(text);
        let expected_key = Key {
            tonic: 1,
            mode: Mode::Minor,
        };
        assert_eq!(song.key, Some(expected_key));
        let roots: Vec<Chord> = song.chords.iter().map(|held| held.chord).collect();
        assert_eq!(roots, vec![minor(58), minor(58)]);
    }

    #[test]
    fn notes_are_named_with_their_octave() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(0), "C-1");
        assert_eq!(note_name(127), "G9");
        assert_eq!(note_name(VIBRATO_KEY_NOTE), "vibrato key");
        assert_eq!(note_name(GLIDE_KEY_NOTE), "glide key");

        assert_eq!(note_name(drum_note(0)), "kick");
        assert_eq!(note_name(drum_note(15)), "tambourine");

        for pitch in 0..=GLIDE_KEY_NOTE + 16 {
            assert_eq!(note_of_name(&note_name(pitch)), Some(pitch));
        }
        assert_eq!(note_of_name("open_hat"), Some(drum_note(6)));
        assert_eq!(note_of_name("Bb3"), Some(58));
        assert_eq!(note_of_name("Cb4"), Some(59));
        assert_eq!(note_of_name("B#3"), Some(60));
        assert_eq!(note_of_name("G#9"), None);
        assert_eq!(note_of_name("Cb-1"), None);
        assert_eq!(note_of_name("H4"), None);
        assert_eq!(note_of_name("C"), None);
        assert_eq!(note_of_name("c4"), None);
    }

    #[test]
    fn missing_values_take_a_new_song_s() {
        let song = read("");
        let new_song = Song {
            key: None,
            tempo: START_TEMPO,
            snap: true,
            preset: None,
            chords: Vec::new(),
            looped: None,
            sound: SoundProfile::new().sound(),
            drums: crate::drums::Drums::new(),
        };
        assert_eq!(song, new_song);

        // A [sound] that leaves things out takes a new song's for them.
        let song = read("[sound]\ntuning_hz = 450\n");
        let mut sound = SoundProfile::new().sound();
        sound.tuning_hz = 450.0;
        sound.connections = song.sound.connections;
        assert_eq!(song.sound, sound);
    }

    #[test]
    fn what_can_t_be_read_is_skipped_or_takes_a_new_song_s() {
        let text = r#"
key = "H major"
tempo = 999
snap = "yes"
preset = "../Secret"
chords = [
  { chord = "Am", held_ms = 100 },
  { chord = "H", held_ms = 100 },
  { chord = "F" },
  { chord = "G", held_ms = -5 },
  "C",
  { chord = "C", held_ms = 200.4 },
]

[loop]
bars = 1

[[loop.layer]]
volume = 150
playing = 3
notes = [
  { note = "E4", start = 0, end = 1 },
  { note = "E9000", start = 0, end = 1 },
  { note = "E4", start = 2, end = 1 },
  { note = "E4", start = -1, end = 1 },
  { note = "E4", start = 1 },
  { note = "E4", start = "one", end = 2 },
]

[[loop.layer]]
notes = []

[[loop.layer]]
notes = [{ note = "H2", start = 0, end = 1 }]

[[loop.layer]]
volume = 0
notes = [{ note = "C3", start = 3, end = 3.5 }]
"#;
        let song = read(text);
        assert_eq!(song.key, None);
        assert_eq!(song.tempo, FASTEST_TEMPO);
        assert!(song.snap);
        assert_eq!(song.preset, None);
        let expected_chords = vec![
            HeldChord {
                chord: minor(57),
                held_ms: 100,
            },
            HeldChord {
                chord: major(48),
                held_ms: 200,
            },
        ];
        assert_eq!(song.chords, expected_chords);

        let expected_loop = SongLoop {
            bars: 1,
            layers: vec![
                SongLayer {
                    bars: 0,
                    volume: FULL_VOLUME,
                    playing: true,
                    notes: vec![note(64, 0.0, 1.0)],
                },
                SongLayer {
                    bars: 0,
                    volume: 0,
                    playing: true,
                    notes: vec![note(48, 3.0, 3.5)],
                },
            ],
        };
        assert_eq!(song.looped, Some(expected_loop));

        // A tempo too slow is brought up to the slowest.
        assert_eq!(read("tempo = 3").tempo, SLOWEST_TEMPO);
    }

    #[test]
    fn a_loop_needs_bars_and_a_layer_with_a_note() {
        let layer = "\n[[loop.layer]]\nnotes = [{ note = \"C4\", start = 0, end = 1 }]\n";
        assert!(read(&format!("[loop]\nbars = 2\n{layer}")).looped.is_some());
        assert_eq!(read(&format!("[loop]\nbars = 0\n{layer}")).looped, None);
        assert_eq!(read(&format!("[loop]\n{layer}")).looped, None);
        assert_eq!(read("[loop]\nbars = 2\n").looped, None);
    }

    #[test]
    fn a_loop_keeps_its_first_8_layers_and_its_first_4096_notes() {
        // Ten layers of a note each: the first 8 stay.
        let mut text = String::from("[loop]\nbars = 1\n");
        for layer in 0..10 {
            text.push_str(&format!("\n[[loop.layer]]\nvolume = {layer}\n"));
            text.push_str("notes = [{ note = \"C4\", start = 0, end = 1 }]\n");
        }
        let looped = read(&text).looped.unwrap();
        let volumes: Vec<u32> = looped.layers.iter().map(|layer| layer.volume).collect();
        assert_eq!(volumes, vec![0, 1, 2, 3, 4, 5, 6, 7]);

        // Three layers of 3,000 notes: the second takes the rest of the
        // 4096, and the third, with none left, is left out.
        let mut text = String::from("[loop]\nbars = 1\n");
        for _ in 0..3 {
            text.push_str("\n[[loop.layer]]\nnotes = [\n");
            for _ in 0..3000 {
                text.push_str("  { note = \"C4\", start = 0, end = 1 },\n");
            }
            text.push_str("]\n");
        }
        let looped = read(&text).looped.unwrap();
        assert_eq!(looped.layers.len(), 2);
        assert_eq!(looped.layers[0].notes.len(), 3000);
        assert_eq!(looped.layers[1].notes.len(), LOOP_MAX_NOTES - 3000);
    }

    #[test]
    fn loop_notes_open_on_the_very_sample_they_were_saved_on() {
        let sample_rate = 48_000;
        for tempo in [SLOWEST_TEMPO, 96, 120, 133, FASTEST_TEMPO] {
            for sample in [0, 1, 7, 23_999, 24_000, 95_999, 191_997, 2_000_003] {
                let beats = beats_of(sample, tempo, sample_rate);
                // Written to a millionth of a beat, then read back.
                let written: f64 = beat_text(beats).parse().unwrap();
                assert_eq!(samples_of(written, tempo, sample_rate), sample, "at {tempo}");
            }
        }
    }

    #[test]
    fn text_that_is_not_toml_can_t_be_read() {
        assert!(song_of("this is [not toml", &SoundProfile::new()).is_err());
    }
    #[test]
    fn shifted_chords_round_trip_at_their_recorded_octave() {
        let text = "chords = [{ chord = \"C\", held_ms = 500, root_note = \"C5\" }]";
        let song = read(text);
        assert_eq!(song.chords[0].chord.root, 72);
        let reopened = read(&text_of(&song));
        assert_eq!(reopened.chords[0].chord.root, 72);
        assert_eq!(reopened.chords[0].held_ms, 500);
    }

}

#[cfg(test)]
mod independent_length_tests {
    use super::*;
    fn read(text: &str) -> Song {
        song_of(text, &SoundProfile::new()).unwrap()
    }
    #[test]
    fn layer_lengths_survive_save_and_open() {
        let song = read("[loop]\nbars = 6\n[[loop.layer]]\nbars = 3\nnotes = [{ note = \"C4\", start = 0, end = 1 }]\n[[loop.layer]]\nbars = 2\nnotes = [{ note = \"E4\", start = 0, end = 1 }]\n");
        let reopened = read(&text_of(&song));
        assert_eq!(song.looped, reopened.looped);
        assert_eq!(reopened.looped.unwrap().layers.iter().map(|layer| layer.bars).collect::<Vec<_>>(), vec![3, 2]);
    }
}
