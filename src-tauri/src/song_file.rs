// A whole song written as text (docs/adr/0038-song-files.md): its key, its
// chords with how long each was held, its tempo, the loop, its tracks and
// its sections (docs/design.md#tracks-and-sections). Like a sound file
// (src/sound_file.rs) the text is TOML, and every choice is a word: the key
// is "A minor", a chord "Am", a note "E4". A track's sound is a sound
// file's text, under [track.sound].
//
// The chords keep their milliseconds, as held: Compose's chords are played
// freely, not on a beat. The loop is whole bars at the song's tempo, so its
// notes say the beat they start and end on, counted from the loop's start.
// That reads the same on any sound card.
//
// Tracks, layers and sections are numbered from 1 in the file, as on
// screen, and from 0 everywhere else.
//
// Reading is forgiving, like a sound file's:
// - A value left out, or of the wrong sort, takes a new song's.
// - A chord or a note Jello can't read is skipped, and so is a layer left
//   with no notes.
// - A song keeps its first 9 tracks, each track its first 8 layers, and
//   the loop its first 16384 notes.
// - The tempo is kept from 40 to 240, and a track's or a layer's volume
//   to 100.
// - A pad Jello doesn't know is skipped, and a pad's values are kept
//   within their ranges.
// - A section with no name, or the name of one before it, is skipped, and
//   so is a layer or a value of one that names a track or layer the song
//   doesn't have. The arrangement skips names no section has.
// - A song file from before tracks keeps none of its sound or loop.
// Text that is not TOML at all can't be read.

use toml::{Table, Value};

use crate::drums::{DrumPad, Drums, PAD_NAMES, pad_named, pad_of_note};
use crate::mix::Mix;
use crate::mix_automation::is_song_wide;
use crate::mix_file::{read_mix, write_buses_and_master, write_strip_effects, write_strip_values};
use crate::engine::{
    FASTEST_TEMPO, FULL_VOLUME, GLIDE_KEY_NOTE, MAX_LAYERS, SLOWEST_TEMPO, START_TEMPO, TRACK_COUNT, drum_note,
    VIBRATO_KEY_NOTE,
};
use crate::presets::name_problem;
use crate::harmony::{Chord, Key, chord_name, chord_of_name, key_name, key_of_name, pitch_of_name};
use crate::sound_file::{
    Sound, group, groups, number_in, read_count, read_flag, read_signed, read_word, sound_of_table, start_group, write_routing,
    write_flag, write_name, write_number, write_sound, write_word,
};
use crate::sound_profile::SoundProfile;

// The most notes a loop holds, in all its layers (`JELLO_LOOP_MAX_NOTES` in
// engine/include/jello_engine.h).
pub const LOOP_MAX_NOTES: usize = 16384;

// The most sections a song keeps, and the most times they are played in
// all, in its arrangement.
pub const MOST_SECTIONS: usize = 64;
pub const MOST_ARRANGED: usize = 256;

// A section lasts 1 to this many bars.
pub const SECTION_MOST_BARS: u32 = 64;

// A section's key shift goes from this many half steps down to as many up.
pub const MOST_KEY_SHIFT: i32 = 12;

// The most values a section moves.
pub const MOST_AUTOMATION: usize = 64;

// The word an automation entry with no track moves: the tempo.
pub const TEMPO_WORD: &str = "tempo";

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
    // The picked track, from 0.
    pub picked: usize,
    pub chords: Vec<HeldChord>,
    // None for a song with no loop. Each layer says its track.
    pub looped: Option<SongLoop>,
    // From 1 to TRACK_COUNT of them.
    pub tracks: Vec<SongTrack>,
    pub sections: Vec<SongSection>,
    // The sections in the order the song plays them, each a place in
    // `sections`.
    pub arrangement: Vec<usize>,
    // Pan, sends, effects, buses and the master (src/mix.rs), with a strip
    // for each track.
    pub mix: Mix,
}

// A chord of the song and how long it was held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldChord {
    pub chord: Chord,
    pub held_ms: u64,
}

// The loop: how many bars it lasts, and its layers in the order they were
// recorded, by track.
#[derive(Clone, Debug, PartialEq)]
pub struct SongLoop {
    pub bars: u32,
    pub layers: Vec<SongLayer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SongLayer {
    // The track it belongs to, from 0.
    pub track: usize,
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

// A track: its name, its volume and its sound, which is either a sound
// profile's or, for a drums track, a drum kit's.
#[derive(Clone, Debug, PartialEq)]
pub struct SongTrack {
    // As it shows on screen. None if the file leaves it out.
    pub name: Option<String>,
    // In percent, from 0 to 100.
    pub volume: u32,
    // The preset its sound came from, so its tile lights up again if the
    // sound still matches it. None if there is none.
    pub preset: Option<String>,
    // A new song's for a drums track.
    pub sound: Sound,
    // Some for a drums track.
    pub drums: Option<Drums>,
}

// A section of the song, like a verse: layers that play together for some
// bars, at its tempo and key shift, moving the values in its automation.
#[derive(Clone, Debug, PartialEq)]
pub struct SongSection {
    // No other section has it.
    pub name: String,
    pub bars: u32,
    pub tempo: u32,
    // In half steps, from -MOST_KEY_SHIFT to MOST_KEY_SHIFT.
    pub key_shift: i32,
    pub layers: Vec<SectionLayer>,
    pub automation: Vec<Automation>,
}

// A layer a section plays: layer `layer` of track `track`, both from 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionLayer {
    pub track: usize,
    pub layer: usize,
}

// A value a section moves in a straight line from `from` at its start to
// `to` at its end: one of track `track`'s values, named as jello_sound names
// them or as its mix does (src/mix_automation.rs), or with no track, the
// tempo or a value of the song's own mix, like "bus 1 volume".
#[derive(Clone, Debug, PartialEq)]
pub struct Automation {
    // From 0, or None for the tempo and the song's own mix.
    pub track: Option<usize>,
    pub value: String,
    pub from: f64,
    pub to: f64,
}

impl Song {
    // A new song's: no key, chords or loop, one instrument track on
    // `new_song`'s sound, and no sections.
    pub fn new(new_song: &SoundProfile) -> Song {
        Song {
            key: None,
            tempo: START_TEMPO,
            snap: true,
            picked: 0,
            chords: Vec::new(),
            looped: None,
            tracks: vec![SongTrack::new(new_song)],
            sections: Vec::new(),
            arrangement: Vec::new(),
            mix: Mix::new(1),
        }
    }

    // How many layers track `track`, from 0, has in the loop.
    pub fn layer_count(&self, track: usize) -> usize {
        let Some(looped) = &self.looped else {
            return 0;
        };
        let mut count = 0;
        for layer in &looped.layers {
            if layer.track == track {
                count += 1;
            }
        }
        count
    }
}

impl SongTrack {
    // An instrument track on `new_song`'s sound, with no name, at full
    // volume.
    pub fn new(new_song: &SoundProfile) -> SongTrack {
        SongTrack {
            name: None,
            volume: FULL_VOLUME,
            preset: None,
            sound: new_song.sound(),
            drums: None,
        }
    }
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
    write_number(&mut text, "picked", song.picked + 1);
    write_arrangement(&mut text, song);

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
        start_group(&mut text, "[loop]");
        write_number(&mut text, "bars", looped.bars);
    }

    for (number, track) in song.tracks.iter().enumerate() {
        write_track(&mut text, song, number, track);
    }
    write_buses_and_master(&mut text, &song.mix);
    for section in &song.sections {
        write_section(&mut text, section);
    }
    text
}

// The arrangement, as the names of its sections, like
// `arrangement = ["Verse", "Chorus", "Verse"]`.
fn write_arrangement(text: &mut String, song: &Song) {
    let mut names = Vec::new();
    for place in &song.arrangement {
        if let Some(section) = song.sections.get(*place) {
            names.push(quoted(&section.name));
        }
    }
    text.push_str(&format!("arrangement = [{}]\n", names.join(", ")));
}

// `name` in quotes, with any quote or backslash in it escaped as TOML
// wants.
fn quoted(name: &str) -> String {
    Value::String(String::from(name)).to_string()
}

// Track `number`, from 0: its values, its sound or drums, then its layers.
fn write_track(text: &mut String, song: &Song, number: usize, track: &SongTrack) {
    start_group(text, "[[track]]");
    if let Some(name) = &track.name {
        write_name(text, "name", name);
    }
    write_number(text, "volume", track.volume);
    if let Some(preset) = &track.preset {
        write_word(text, "preset", preset);
    }
    write_strip_values(text, &song.mix, number);
    match &track.drums {
        Some(drums) => {
            write_drums(text, drums);
            start_group(text, "[track.routing]");
            write_routing(text, &track.sound, "track.routing");
        }
        None => write_sound(text, &track.sound, "track.sound"),
    }
    write_strip_effects(text, &song.mix, number);

    let Some(looped) = &song.looped else {
        return;
    };
    for layer in &looped.layers {
        if layer.track == number {
            write_layer(text, layer);
        }
    }
}

// The drums: the kit's instrument, if one is picked, then one pad a line.
fn write_drums(text: &mut String, drums: &Drums) {
    start_group(text, "[track.drums]");
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

fn write_layer(text: &mut String, layer: &SongLayer) {
    start_group(text, "[[track.layer]]");
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

// A section: its values, its layers as track and layer numbers from 1, and
// its automation, the tempo's with track 0.
fn write_section(text: &mut String, section: &SongSection) {
    start_group(text, "[[section]]");
    write_name(text, "name", &section.name);
    write_number(text, "bars", section.bars);
    write_number(text, "tempo", section.tempo);
    write_number(text, "key_shift", section.key_shift);

    if section.layers.is_empty() {
        text.push_str("layers = []\n");
    } else {
        text.push_str("layers = [\n");
        for layer in &section.layers {
            let track = layer.track + 1;
            let number = layer.layer + 1;
            text.push_str(&format!("  {{ track = {track}, layer = {number} }},\n"));
        }
        text.push_str("]\n");
    }

    if section.automation.is_empty() {
        text.push_str("automation = []\n");
    } else {
        text.push_str("automation = [\n");
        for moved in &section.automation {
            let mut track = 0;
            if let Some(number) = moved.track {
                track = number + 1;
            }
            let value = quoted(&moved.value);
            text.push_str(&format!(
                "  {{ track = {track}, value = {value}, from = {}, to = {} }},\n",
                moved.from, moved.to
            ));
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

// The song `text` describes, with `new_song`'s sound for whatever a track's
// sound leaves out. Invalid TOML, connection indices or mix routing fail.
pub fn song_of(text: &str, new_song: &SoundProfile) -> Result<Song, String> {
    let table = match text.parse::<Table>() {
        Ok(table) => table,
        Err(error) => return Err(format!("it is not a song file: {}", error.message())),
    };

    let mut song = Song::new(new_song);
    if let Some(name) = read_word(&table, "key") {
        song.key = key_of_name(name);
    }

    let tempo = read_count(&table, "tempo", START_TEMPO);
    song.tempo = tempo.clamp(SLOWEST_TEMPO, FASTEST_TEMPO);
    song.snap = read_flag(&table, "snap", true);

    for item in items(&table, "chords") {
        if let Some(held) = read_chord(item) {
            song.chords.push(held);
        }
    }

    let mut loop_bars = 0;
    if let Some(loop_table) = group(&table, "loop") {
        loop_bars = read_count(loop_table, "bars", 0);
    }

    // The tracks, and their layers if the loop has bars.
    let mut tracks = Vec::new();
    let mut track_tables = Vec::new();
    let mut layers = Vec::new();
    let mut note_count = 0;
    for track_table in groups(&table, "track") {
        if tracks.len() == TRACK_COUNT {
            break;
        }
        let number = tracks.len();
        tracks.push(read_track(track_table, new_song).map_err(|error| format!("track {}: {error}", number + 1))?);
        track_tables.push(track_table);
        if loop_bars < 1 {
            continue;
        }
        let mut track_layers = 0;
        for layer_table in groups(track_table, "layer") {
            if track_layers == MAX_LAYERS {
                break;
            }
            let Some(layer) = read_layer(layer_table, number, &mut note_count) else {
                continue;
            };
            layers.push(layer);
            track_layers += 1;
        }
    }
    if !tracks.is_empty() {
        song.tracks = tracks;
    }
    song.mix = read_mix(&table, &track_tables)?;
    song.mix.fit_tracks(song.tracks.len());
    for (number, track) in song.tracks.iter().enumerate() {
        crate::mix::check_track_routing(&track.sound.connections, &song.mix.tracks[number].links)
            .map_err(|error| format!("track {}: {error}", number + 1))?;
    }
    if !layers.is_empty() {
        song.looped = Some(SongLoop {
            bars: loop_bars,
            layers,
        });
    }

    let picked = read_count(&table, "picked", 1);
    if picked >= 1 && (picked as usize) <= song.tracks.len() {
        song.picked = picked as usize - 1;
    }

    for section_table in groups(&table, "section") {
        if song.sections.len() == MOST_SECTIONS {
            break;
        }
        if let Some(section) = read_section(section_table, &song) {
            song.sections.push(section);
        }
    }
    song.arrangement = read_arrangement(&table, &song.sections);
    Ok(song)
}

// A track: an instrument track on its [track.sound], or with a
// [track.drums], a drums track.
fn read_track(track_table: &Table, new_song: &SoundProfile) -> Result<SongTrack, String> {
    let mut track = SongTrack::new(new_song);
    if let Some(name) = read_word(track_table, "name") {
        track.name = Some(String::from(name));
    }
    track.volume = read_count(track_table, "volume", FULL_VOLUME).min(FULL_VOLUME);

    // Only a name a preset could have, so it is written back as it was.
    if let Some(name) = read_word(track_table, "preset") {
        if name_problem(name).is_none() {
            track.preset = Some(String::from(name));
        }
    }

    if let Some(drums_table) = group(track_table, "drums") {
        track.drums = Some(read_drums(drums_table));
        if let Some(routing_table) = group(track_table, "routing") {
            track.sound = sound_of_table(routing_table, new_song)?;
        }
    } else if let Some(sound_table) = group(track_table, "sound") {
        track.sound = sound_of_table(sound_table, new_song)?;
    }
    Ok(track)
}

// The drums `table`, a track's [track.drums], describes, starting from a
// new song's.
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

// A layer of track `track`, from 0, counting its notes into `note_count`,
// or None if it is left with no notes, with LOOP_MAX_NOTES counted already
// or none of its own read.
fn read_layer(layer_table: &Table, track: usize, note_count: &mut usize) -> Option<SongLayer> {
    let mut notes = Vec::new();
    for item in items(layer_table, "notes") {
        if *note_count == LOOP_MAX_NOTES {
            break;
        }
        if let Some(note) = read_note(item) {
            notes.push(note);
            *note_count += 1;
        }
    }
    if notes.is_empty() {
        return None;
    }

    let volume = read_count(layer_table, "volume", FULL_VOLUME);
    Some(SongLayer {
        track,
        bars: read_count(layer_table, "bars", 0).min(64),
        volume: volume.min(FULL_VOLUME),
        playing: read_flag(layer_table, "playing", true),
        notes,
    })
}

// A section, or None if it has no name or the name of one before it. Its
// values are kept within their ranges, and its layers and automation keep
// only what `song`'s tracks and layers have.
fn read_section(section_table: &Table, song: &Song) -> Option<SongSection> {
    let name = read_word(section_table, "name")?.trim();
    if name.is_empty() {
        return None;
    }
    for before in &song.sections {
        if before.name == name {
            return None;
        }
    }

    let bars = read_count(section_table, "bars", 1).clamp(1, SECTION_MOST_BARS);
    let tempo = read_count(section_table, "tempo", song.tempo).clamp(SLOWEST_TEMPO, FASTEST_TEMPO);
    let key_shift = read_signed(section_table, "key_shift", 0).clamp(-MOST_KEY_SHIFT, MOST_KEY_SHIFT);

    let mut layers: Vec<SectionLayer> = Vec::new();
    for item in items(section_table, "layers") {
        let Some(layer) = read_section_layer(item, song) else {
            continue;
        };
        if !layers.contains(&layer) {
            layers.push(layer);
        }
    }

    let mut automation = Vec::new();
    for item in items(section_table, "automation") {
        if automation.len() == MOST_AUTOMATION {
            break;
        }
        if let Some(moved) = read_automation(item, song) {
            automation.push(moved);
        }
    }

    Some(SongSection {
        name: String::from(name),
        bars,
        tempo,
        key_shift,
        layers,
        automation,
    })
}

// A section's `{ track = 1, layer = 2 }`, or None if `song` has no such
// layer.
fn read_section_layer(item: &Table, song: &Song) -> Option<SectionLayer> {
    let track = read_count(item, "track", 0) as usize;
    let layer = read_count(item, "layer", 0) as usize;
    if track < 1 || layer < 1 {
        return None;
    }
    if layer > song.layer_count(track - 1) {
        return None;
    }
    Some(SectionLayer {
        track: track - 1,
        layer: layer - 1,
    })
}

// A section's `{ track = 1, value = "filter cutoff", from = 200, to = 8000 }`,
// or with track 0, the tempo's. None if `song` has no such track, or a
// value or number is missing.
fn read_automation(item: &Table, song: &Song) -> Option<Automation> {
    let track = read_count(item, "track", u32::MAX) as usize;
    let value = read_word(item, "value")?;
    let from = number_in(item.get("from")?)?;
    let to = number_in(item.get("to")?)?;
    if from.is_nan() || to.is_nan() {
        return None;
    }
    // With no track: the tempo, or a value of the song's own mix.
    if track == 0 {
        let mut word = String::from(TEMPO_WORD);
        if is_song_wide(value) {
            word = String::from(value);
        }
        return Some(Automation {
            track: None,
            value: word,
            from,
            to,
        });
    }
    if track > song.tracks.len() {
        return None;
    }
    Some(Automation {
        track: Some(track - 1),
        value: String::from(value),
        from,
        to,
    })
}

// The arrangement's sections, as places in `sections`, skipping names none
// of them has.
fn read_arrangement(table: &Table, sections: &[SongSection]) -> Vec<usize> {
    let mut arrangement = Vec::new();
    let Some(Value::Array(names)) = table.get("arrangement") else {
        return arrangement;
    };
    for name in names {
        if arrangement.len() == MOST_ARRANGED {
            break;
        }
        let Value::String(name) = name else {
            continue;
        };
        for (place, section) in sections.iter().enumerate() {
            if section.name == *name {
                arrangement.push(place);
                break;
            }
        }
    }
    arrangement
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

    #[test]
    fn song_rejects_effect_and_sound_connections_past_shared_capacity() {
        use crate::engine::{EffectConnection, EQ_OUTPUT};
        use crate::modulation::{CONNECTION_COUNT, Source};
        let profile = SoundProfile::new();
        let mut song = Song::new(&profile);
        let connection = song.tracks[0].sound.connections[0];
        song.tracks[0].sound.connections = [connection; CONNECTION_COUNT];
        let link = EffectConnection { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: 0.0, one_way: false, scaled_by: Source::None };
        song.mix.tracks[0].links.push(link);
        let text = text_of(&song);
        assert!(song_of(&text, &profile).is_err());
        song.tracks[0].sound.connections[CONNECTION_COUNT - 1].source = Source::None;
        assert!(song_of(&text_of(&song), &profile).is_ok());
    }

    #[test]
    fn old_drum_tracks_without_a_routing_table_keep_new_song_routing() {
        let profile = SoundProfile::new();
        let song = song_of("[[track]]\n[track.drums]\n", &profile).unwrap();
        assert!(song.tracks[0].drums.is_some());
        assert_eq!(song.tracks[0].sound.lfos, *profile.lfos());
        assert_eq!(song.tracks[0].sound.connections, *profile.connections());
    }

    use crate::drums::{DRUM_MOST_GROUP, DRUM_MOST_HALF_STEPS};
    use crate::engine::{EffectKind, Kind, SLOWEST_TEMPO};
    use crate::mix::{Bus, Place, Slot};
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

    fn layer(track: usize, notes: Vec<BeatNote>) -> SongLayer {
        SongLayer {
            track,
            bars: 0,
            volume: FULL_VOLUME,
            playing: true,
            notes,
        }
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
            track: 0,
            bars: 0,
            volume: 80,
            playing: true,
            notes: vec![
                note(64, 0.0, 0.5),
                note(VIBRATO_KEY_NOTE, 1.0, 2.0),
                note(67, 5.237396, 5.75),
            ],
        };
        let second = SongLayer {
            track: 0,
            bars: 1,
            volume: 35,
            playing: false,
            notes: vec![note(GLIDE_KEY_NOTE, 0.25, 0.5), note(70, 2.0, 7.999999)],
        };
        let hits = layer(1, vec![note(drum_note(0), 1.0, 1.1), note(drum_note(4), 1.5, 1.6)]);

        let lead = SongTrack {
            name: Some(String::from("Lead")),
            volume: 90,
            preset: Some(String::from("Warm pad")),
            sound,
            drums: None,
        };
        let kit = SongTrack {
            name: Some(String::from("Black Pearl")),
            volume: 60,
            preset: None,
            sound: SoundProfile::new().sound(),
            drums: Some(drums),
        };
        let verse = SongSection {
            name: String::from("Verse \"one\""),
            bars: 4,
            tempo: 90,
            key_shift: -3,
            layers: vec![
                SectionLayer { track: 0, layer: 1 },
                SectionLayer { track: 1, layer: 0 },
            ],
            automation: vec![
                Automation {
                    track: Some(0),
                    value: String::from("filter cutoff"),
                    from: 200.0,
                    to: 8000.5,
                },
                Automation {
                    track: None,
                    value: String::from(TEMPO_WORD),
                    from: 90.0,
                    to: 100.0,
                },
                Automation {
                    track: None,
                    value: String::from("bus 1 volume"),
                    from: 20.0,
                    to: 100.0,
                },
                Automation {
                    track: Some(0),
                    value: String::from("effect 1 band_1_gain_db"),
                    from: -6.0,
                    to: 0.0,
                },
            ],
        };
        let chorus = SongSection {
            name: String::from("Chorus"),
            bars: 8,
            tempo: 96,
            key_shift: 0,
            layers: Vec::new(),
            automation: Vec::new(),
        };

        Song {
            key: Some(Key {
                tonic: 9,
                mode: Mode::Minor,
            }),
            tempo: 96,
            snap: false,
            picked: 1,
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
                layers: vec![first, second, hits],
            }),
            tracks: vec![lead, kit],
            sections: vec![verse, chorus],
            arrangement: vec![0, 1, 0],
            mix: full_mix(),
        }
    }

    // Two tracks' mix: the lead panned left, sending to a reverb bus, with
    // an EQ, and the master ducking under the drums.
    fn full_mix() -> Mix {
        let mut mix = Mix::new(2);
        mix.tracks[0].pan = -20;
        mix.tracks[0].sends[0] = 25;
        let mut eq = Slot::new(EffectKind::Eq, Place::Track(0));
        eq.values[0] = 1.0;
        mix.tracks[0].chain[0] = Some(eq);
        let mut room = Bus::new(Some(String::from("Room")));
        room.chain[0] = Some(Slot::new(EffectKind::Reverb, Place::Bus(0)));
        mix.buses.push(room);
        let mut ducking = Slot::new(EffectKind::Compressor, Place::Master);
        ducking.values[7] = 1.0;
        mix.master[3] = Some(ducking);
        mix
    }

    fn read(text: &str) -> Song {
        song_of(text, &SoundProfile::new()).unwrap()
    }

    #[test]
    fn a_song_reads_back_exactly_as_it_was_written() {
        let song = full_song();
        assert_eq!(read(&text_of(&song)), song);

        // And a new song's, with no key, chords, loop or sections.
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
        assert!(text.contains("picked = 2\n"));
        assert!(text.contains("arrangement = ['Verse \"one\"', \"Chorus\", 'Verse \"one\"']\n"));
        assert!(text.contains("  { chord = \"Am\", held_ms = 1250 },\n"));
        assert!(text.contains("  { chord = \"Bb\", held_ms = 3 },\n"));
        assert!(text.contains("[loop]\nbars = 2\n"));
        assert!(text.contains("[[track]]\nname = \"Lead\"\nvolume = 90\npreset = \"Warm pad\"\npan = -20\nsends = [25]\n"));
        assert!(text.contains("[[track.effect]]\nslot = 1\nkind = \"eq\"\non = true\nband_1_on = true\n"));
        assert!(text.contains("[[bus]]\nname = \"Room\"\nvolume = 100\npan = 0\n\n[[bus.effect]]\nslot = 1\nkind = \"reverb\"\n"));
        assert!(text.contains("[master]\nboost_db = 0\nceiling_db = -1\nrelease_ms = 100\n"));
        assert!(text.contains("[[master.effect]]\nslot = 4\nkind = \"compressor\"\non = true\nlistens_to = \"track 2\"\n"));
        assert!(text.contains("[track.sound]\ntuning_hz = 432.5\n"));
        assert!(text.contains("[[track.sound.oscillators]]\nkind = \"saw\"\n"));
        assert!(text.contains("[track.sound.filter]\n"));
        assert!(text.contains("kind = \"sample\"\ninstrument = 'The \"Big\" \\ piano'\n"));
        assert!(text.contains("[[track.layer]]\nvolume = 80\nplaying = true\n"));
        assert!(text.contains("[[track.layer]]\nbars = 1\nvolume = 35\nplaying = false\n"));
        assert!(text.contains("  { note = \"E4\", start = 0, end = 0.5 },\n"));
        assert!(text.contains("  { note = \"vibrato key\", start = 1, end = 2 },\n"));
        assert!(text.contains("  { note = \"glide key\", start = 0.25, end = 0.5 },\n"));
        assert!(text.contains("  { note = \"G4\", start = 5.237396, end = 5.75 },\n"));
        assert!(text.contains("[[track]]\nname = \"Black Pearl\"\nvolume = 60\npan = 0\nsends = [0]\n\n[track.drums]\nkit = \"Black Pearl\"\npads = [\n"));
        assert!(text.contains("  { note = \"kick\", start = 1, end = 1.1 },\n"));
        assert!(text.contains("  { note = \"closed hat\", start = 1.5, end = 1.6 },\n"));
        assert!(text.contains("  { pad = \"snare\", level = 70, half_steps = -2, pan = 30, group = 2 },\n"));
        assert!(text.contains("  { pad = \"open hat\", level = 100, half_steps = 0, pan = 0, group = 1 },\n"));
        assert!(text.contains("[[section]]\nname = 'Verse \"one\"'\nbars = 4\ntempo = 90\nkey_shift = -3\n"));
        assert!(text.contains("layers = [\n  { track = 1, layer = 2 },\n  { track = 2, layer = 1 },\n]\n"));
        assert!(text.contains("  { track = 1, value = \"filter cutoff\", from = 200, to = 8000.5 },\n"));
        assert!(text.contains("  { track = 0, value = \"tempo\", from = 90, to = 100 },\n"));
        assert!(text.contains("[[section]]\nname = \"Chorus\"\nbars = 8\ntempo = 96\nkey_shift = 0\nlayers = []\nautomation = []\n"));

        // A drums track writes no sound, and each layer comes under its track.
        assert_eq!(text.matches("[track.sound]").count(), 1);
        let drums_at = text.find("[track.drums]").unwrap();
        assert!(text.find("note = \"kick\"").unwrap() > drums_at);
        assert!(text.find("note = \"E4\"").unwrap() < drums_at);
    }

    #[test]
    fn drums_are_read_by_pad_name_within_their_ranges() {
        let text = r#"
[[track]]

[track.drums]
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
        assert_eq!(song.tracks[0].drums, Some(expected));

        // A song with no [track.drums] has an instrument track, and hits
        // read by name.
        assert_eq!(read("").tracks[0].drums, None);
        let hits = "[loop]\nbars = 1\n[[track]]\n[[track.layer]]\nnotes = [{ note = \"snare\", start = 0, end = 0.1 }, { note = \"cowbell\", start = 1, end = 2 }]\n";
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
            picked: 0,
            chords: Vec::new(),
            looped: None,
            tracks: vec![SongTrack {
                name: None,
                volume: FULL_VOLUME,
                preset: None,
                sound: SoundProfile::new().sound(),
                drums: None,
            }],
            sections: Vec::new(),
            arrangement: Vec::new(),
            mix: Mix::new(1),
        };
        assert_eq!(song, new_song);
        assert_eq!(song, Song::new(&SoundProfile::new()));

        // A [track.sound] that leaves things out takes a new song's for them.
        let song = read("[[track]]\n[track.sound]\ntuning_hz = 450\n");
        let mut sound = SoundProfile::new().sound();
        sound.tuning_hz = 450.0;
        sound.connections = song.tracks[0].sound.connections;
        assert_eq!(song.tracks[0].sound, sound);
    }

    #[test]
    fn a_song_file_from_before_tracks_keeps_none_of_its_sound_or_loop() {
        let text = "tempo = 90\npreset = \"Old\"\n[loop]\nbars = 1\n[[loop.layer]]\nnotes = [{ note = \"C4\", start = 0, end = 1 }]\n[sound]\ntuning_hz = 450\n";
        let song = read(text);
        assert_eq!(song.tempo, 90);
        assert_eq!(song.looped, None);
        assert_eq!(song.tracks, Song::new(&SoundProfile::new()).tracks);
    }

    #[test]
    fn what_can_t_be_read_is_skipped_or_takes_a_new_song_s() {
        let text = r#"
key = "H major"
tempo = 999
snap = "yes"
picked = 5
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

[[track]]
volume = 150
preset = "../Secret"

[[track.layer]]
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

[[track.layer]]
notes = []

[[track.layer]]
notes = [{ note = "H2", start = 0, end = 1 }]

[[track.layer]]
volume = 0
notes = [{ note = "C3", start = 3, end = 3.5 }]
"#;
        let song = read(text);
        assert_eq!(song.key, None);
        assert_eq!(song.tempo, FASTEST_TEMPO);
        assert!(song.snap);
        assert_eq!(song.picked, 0);
        assert_eq!(song.tracks[0].preset, None);
        assert_eq!(song.tracks[0].volume, FULL_VOLUME);
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

        let mut quiet = layer(0, vec![note(48, 3.0, 3.5)]);
        quiet.volume = 0;
        let expected_loop = SongLoop {
            bars: 1,
            layers: vec![layer(0, vec![note(64, 0.0, 1.0)]), quiet],
        };
        assert_eq!(song.looped, Some(expected_loop));

        // A tempo too slow is brought up to the slowest.
        assert_eq!(read("tempo = 3").tempo, SLOWEST_TEMPO);
    }

    #[test]
    fn a_loop_needs_bars_and_a_layer_with_a_note() {
        let layer = "\n[[track]]\n[[track.layer]]\nnotes = [{ note = \"C4\", start = 0, end = 1 }]\n";
        assert!(read(&format!("[loop]\nbars = 2\n{layer}")).looped.is_some());
        assert_eq!(read(&format!("[loop]\nbars = 0\n{layer}")).looped, None);
        assert_eq!(read(&format!("[loop]\n{layer}")).looped, None);
        assert_eq!(read(layer).looped, None);
        assert_eq!(read("[loop]\nbars = 2\n[[track]]\n").looped, None);
    }

    #[test]
    fn a_song_keeps_9_tracks_8_layers_a_track_and_16384_notes() {
        // Ten tracks: the first 9 stay.
        let mut text = String::new();
        for number in 0..10 {
            text.push_str(&format!("\n[[track]]\nvolume = {number}\n"));
        }
        let song = read(&text);
        let volumes: Vec<u32> = song.tracks.iter().map(|track| track.volume).collect();
        assert_eq!(volumes, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);

        // Ten layers on each of two tracks: the first 8 of each stay.
        let mut text = String::from("[loop]\nbars = 1\n");
        for _ in 0..2 {
            text.push_str("\n[[track]]\n");
            for layer in 0..10 {
                text.push_str(&format!("\n[[track.layer]]\nvolume = {layer}\n"));
                text.push_str("notes = [{ note = \"C4\", start = 0, end = 1 }]\n");
            }
        }
        let looped = read(&text).looped.unwrap();
        let volumes: Vec<(usize, u32)> = looped.layers.iter().map(|layer| (layer.track, layer.volume)).collect();
        let mut expected = Vec::new();
        for track in 0..2 {
            for volume in 0..8 {
                expected.push((track, volume));
            }
        }
        assert_eq!(volumes, expected);

        // Three tracks of 9,000 notes: the second takes the rest of the
        // 16384, and the third, with none left, has no layer.
        let mut text = String::from("[loop]\nbars = 1\n");
        for _ in 0..3 {
            text.push_str("\n[[track]]\n[[track.layer]]\nnotes = [\n");
            for _ in 0..9000 {
                text.push_str("  { note = \"C4\", start = 0, end = 1 },\n");
            }
            text.push_str("]\n");
        }
        let song = read(&text);
        assert_eq!(song.tracks.len(), 3);
        let looped = song.looped.unwrap();
        assert_eq!(looped.layers.len(), 2);
        assert_eq!(looped.layers[0].notes.len(), 9000);
        assert_eq!(looped.layers[1].notes.len(), LOOP_MAX_NOTES - 9000);
    }

    #[test]
    fn sections_keep_only_what_the_song_has_within_their_ranges() {
        let text = r#"
tempo = 100
arrangement = ["B", "Nope", "A", 3, "B"]

[loop]
bars = 1

[[track]]
[[track.layer]]
notes = [{ note = "C4", start = 0, end = 1 }]

[[track]]

[[section]]
name = " A "
bars = 900
tempo = 9
key_shift = 30
layers = [
  { track = 1, layer = 1 },
  { track = 1, layer = 1 },
  { track = 1, layer = 2 },
  { track = 2, layer = 1 },
  { track = 0, layer = 1 },
  { layer = 1 },
]
automation = [
  { track = 2, value = "volume", from = 0, to = 100 },
  { track = 0, value = "anything", from = 90, to = 120 },
  { track = 3, value = "volume", from = 0, to = 100 },
  { track = 1, value = "volume", from = 0 },
  { value = "volume", from = 0, to = 1 },
]

[[section]]
name = "A"

[[section]]
bars = 2

[[section]]
name = "B"
"#;
        let song = read(text);
        assert_eq!(song.sections.len(), 2);
        let a = &song.sections[0];
        assert_eq!(a.name, "A");
        assert_eq!(a.bars, SECTION_MOST_BARS);
        assert_eq!(a.tempo, SLOWEST_TEMPO);
        assert_eq!(a.key_shift, MOST_KEY_SHIFT);
        assert_eq!(a.layers, vec![SectionLayer { track: 0, layer: 0 }]);
        let expected = vec![
            Automation {
                track: Some(1),
                value: String::from("volume"),
                from: 0.0,
                to: 100.0,
            },
            Automation {
                track: None,
                value: String::from(TEMPO_WORD),
                from: 90.0,
                to: 120.0,
            },
        ];
        assert_eq!(a.automation, expected);

        // A section left plain takes 1 bar at the song's tempo.
        let b = &song.sections[1];
        assert_eq!((b.bars, b.tempo, b.key_shift), (1, 100, 0));
        assert_eq!(song.arrangement, vec![1, 0, 1]);
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

    #[test]
    fn layer_lengths_survive_save_and_open() {
        let song = read("[loop]\nbars = 6\n[[track]]\n[[track.layer]]\nbars = 3\nnotes = [{ note = \"C4\", start = 0, end = 1 }]\n[[track.layer]]\nbars = 2\nnotes = [{ note = \"E4\", start = 0, end = 1 }]\n");
        let reopened = read(&text_of(&song));
        assert_eq!(song.looped, reopened.looped);
        let bars: Vec<u32> = reopened.looped.unwrap().layers.iter().map(|layer| layer.bars).collect();
        assert_eq!(bars, vec![3, 2]);
    }
}
