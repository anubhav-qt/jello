// Sampled instruments (docs/design.md#sampler): recordings of a real
// instrument, kept as folders in Music/Jello/Instruments. A folder holds
// either an SFZ file, which says which recording plays which notes, or
// WAV or FLAC files named by the note they were recorded at (C4.wav,
// F#2.flac) or by the drum they are (kick.wav, closed hat.wav).
//
// Reading one takes a while, so it is done once and kept: every engine
// that plays an instrument shares the one read. The recordings are turned
// into what the engine plays: 48,000 frames a second, one or two channels,
// only the middle velocity layer, each cut to 15 seconds unless it loops,
// and the whole instrument made as loud as the others.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use crate::drums::{gm_notes_of, pad_named};

// The engine plays at 48,000 frames a second, so every recording is turned
// into that.
pub const SAMPLE_RATE: u32 = 48000;

// Only the recordings made for this velocity play, one layer of each
// instrument: a medium hit. More layers are a todo.
const KEPT_VELOCITY: u8 = 80;

// How long a recording that plays once is kept, and how long it fades out
// over where it is cut, so it never clicks.
const LONGEST_SECONDS: u64 = 15;
const CUT_FADE_MS: u64 = 100;

// How loud an instrument's loudest frame is made, just under full.
const PEAK: f32 = 0.9;

// How deep SFZ files can #include one another.
const MOST_INCLUDES: usize = 8;

// The files that hold recordings.
const AUDIO_EXTENSIONS: [&str; 2] = ["wav", "flac"];

// One recording and the notes it plays (`JelloSampleZone` in
// engine/include/jello_engine.h).
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub lowest: u8,
    pub highest: u8,
    // The note it was recorded at, and how far off that it is, in cents.
    pub root: u8,
    pub tune_cents: f32,
    // 1 or 2; in stereo left and right take turns.
    pub channels: u16,
    pub frames: Vec<f32>,
    // Where its loop starts and ends, in frames; the same for none.
    pub loop_start: u64,
    pub loop_end: u64,
}

impl Zone {
    pub fn frame_count(&self) -> u64 {
        self.frames.len() as u64 / u64::from(self.channels)
    }
}

// An instrument as the engine plays it.
#[derive(Debug, PartialEq)]
pub struct Instrument {
    pub zones: Vec<Zone>,
}

impl Instrument {
    // The first zone that plays `note`, or None.
    pub fn exact_zone(&self, note: u8) -> Option<&Zone> {
        for zone in &self.zones {
            if zone.lowest <= note && note <= zone.highest {
                return Some(zone);
            }
        }
        None
    }
}

// Where an instrument is, as the screen and the agent are told.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Read,
    Loading,
    // It can't be read, and why.
    Failed(String),
    // Nothing has asked for it yet.
    Unread,
}

// Every instrument read so far, by name, and those being read now.
struct Library {
    folder: Option<PathBuf>,
    read: HashMap<String, Arc<Instrument>>,
    loading: HashSet<String>,
    failed: HashMap<String, String>,
    // Called once an instrument read in the background is ready.
    on_loaded: Option<Arc<dyn Fn() + Send + Sync>>,
}

fn library() -> &'static Mutex<Library> {
    static LIBRARY: OnceLock<Mutex<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        Mutex::new(Library {
            folder: None,
            read: HashMap::new(),
            loading: HashSet::new(),
            failed: HashMap::new(),
            on_loaded: None,
        })
    })
}

// Makes `folder` the one instruments are read from. The app does this as
// it starts.
pub fn set_folder(folder: PathBuf) {
    library().lock().unwrap().folder = Some(folder);
}

// The folder instruments are read from, if it is known.
pub fn folder() -> Option<PathBuf> {
    library().lock().unwrap().folder.clone()
}

// Calls `hook` each time an instrument read in the background is ready
// (or has failed), on the thread that read it.
pub fn on_loaded(hook: impl Fn() + Send + Sync + 'static) {
    library().lock().unwrap().on_loaded = Some(Arc::new(hook));
}

// The instruments in the folder, by name, in order, ignoring case.
pub fn list() -> Vec<String> {
    let folder = library().lock().unwrap().folder.clone();
    let Some(folder) = folder else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        names.push(name);
    }
    names.sort_by_key(|name| name.to_lowercase());
    names
}

// Lets instrument `name` be read again if it failed, say once its files
// are fixed. Picking an instrument does this.
pub fn try_again(name: &str) {
    library().lock().unwrap().failed.remove(name);
}

// Where instrument `name` is now.
pub fn status(name: &str) -> Status {
    let library = library().lock().unwrap();
    if library.read.contains_key(name) {
        return Status::Read;
    }
    if library.loading.contains(name) {
        return Status::Loading;
    }
    if let Some(why) = library.failed.get(name) {
        return Status::Failed(why.clone());
    }
    Status::Unread
}

// Instrument `name`, read now if it is not read yet, or why it can't be.
pub fn load(name: &str) -> Result<Arc<Instrument>, String> {
    let folder = {
        let library = library().lock().unwrap();
        if let Some(found) = library.read.get(name) {
            return Ok(found.clone());
        }
        library.folder.clone()
    };
    let Some(folder) = folder else {
        return Err(String::from("the Instruments folder can't be found"));
    };
    let result = read_named(&folder, name);
    let mut library = library().lock().unwrap();
    match result {
        Ok(instrument) => {
            let instrument = Arc::new(instrument);
            library.read.insert(String::from(name), instrument.clone());
            Ok(instrument)
        }
        Err(why) => {
            library.failed.insert(String::from(name), why.clone());
            Err(why)
        }
    }
}

// Instrument `name` if it is read, or None while it is read on another
// thread, started now if it isn't yet. None too for one that failed.
pub fn load_in_background(name: &str) -> Option<Arc<Instrument>> {
    let mut shelf = library().lock().unwrap();
    if let Some(found) = shelf.read.get(name) {
        return Some(found.clone());
    }
    if shelf.loading.contains(name) {
        return None;
    }
    if shelf.failed.contains_key(name) {
        return None;
    }
    let folder = shelf.folder.clone()?;
    shelf.loading.insert(String::from(name));

    drop(shelf);
    let name = String::from(name);
    thread::spawn(move || {
        let result = read_named(&folder, &name);
        // The thread holds the instrument until the hook has run, so
        // let_go_of_unused can't free it before the engine takes it.
        let mut ready = None;
        let hook = {
            let mut library = library().lock().unwrap();
            library.loading.remove(&name);
            match result {
                Ok(instrument) => {
                    let instrument = Arc::new(instrument);
                    library.read.insert(name, instrument.clone());
                    ready = Some(instrument);
                }
                Err(why) => {
                    library.failed.insert(name, why);
                }
            }
            library.on_loaded.clone()
        };
        if let Some(hook) = hook {
            hook();
        }
        drop(ready);
    });
    None
}

// Lets go of every instrument read that no engine plays any more, so its
// memory is freed. Reading it again takes a while.
pub fn let_go_of_unused() {
    let mut library = library().lock().unwrap();
    library.read.retain(|_, instrument| Arc::strong_count(instrument) > 1);
}

// Instrument `name` from `folder`, or why it can't be read.
fn read_named(folder: &Path, name: &str) -> Result<Instrument, String> {
    if !is_plain_name(name) {
        return Err(format!("there is no instrument called {name}"));
    }
    let path = folder.join(name);
    if !path.is_dir() {
        return Err(format!("there is no instrument called {name}"));
    }
    read_instrument(&path)
}

// A name that stays inside the folder: no slashes, and not "." or "..".
fn is_plain_name(name: &str) -> bool {
    if name.trim().is_empty() {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    name != "." && name != ".."
}

// One recording an instrument plays, and how, before it is read.
#[derive(Clone, Debug, PartialEq)]
struct Region {
    sample: PathBuf,
    lowest: u8,
    highest: u8,
    root: u8,
    tune_cents: f32,
    // How much louder or quieter it plays, as a gain.
    gain: f32,
    // From the SFZ file: loop it, play it once, or as the file says.
    looping: Looping,
    loop_points: Option<(u64, u64)>,
    // Where in the recording it starts and ends, in frames; None for its
    // end.
    offset: u64,
    end: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Looping {
    Loops,
    Once,
    AsRecorded,
}

// The instrument in `folder`.
pub fn read_instrument(folder: &Path) -> Result<Instrument, String> {
    let regions = match first_sfz_in(folder) {
        Some(sfz) => read_sfz(&sfz)?,
        None => regions_from_names(folder)?,
    };
    if regions.is_empty() {
        return Err(format!("{} has no recordings Jello can play", folder_name(folder)));
    }

    let mut recordings: HashMap<PathBuf, Arc<Recording>> = HashMap::new();
    let mut zones = Vec::new();
    for region in &regions {
        let recording = match recordings.get(&region.sample) {
            Some(recording) => recording.clone(),
            None => {
                let recording = Arc::new(decode(&region.sample)?);
                recordings.insert(region.sample.clone(), recording.clone());
                recording
            }
        };
        zones.push(zone_of(region, &recording));
    }
    make_as_loud(&mut zones);
    Ok(Instrument { zones })
}

fn folder_name(folder: &Path) -> String {
    match folder.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => folder.to_string_lossy().to_string(),
    }
}

// The first SFZ file in `folder` by name, or None.
fn first_sfz_in(folder: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        if extension_of(&path) == "sfz" {
            found.push(path);
        }
    }
    found.sort();
    found.into_iter().next()
}

fn extension_of(path: &Path) -> String {
    match path.extension() {
        Some(extension) => extension.to_string_lossy().to_lowercase(),
        None => String::new(),
    }
}

// The regions of a folder of recordings named by note or drum. Drums come
// first. Each note's recording plays the notes from halfway to the one
// below it to halfway to the one above.
fn regions_from_names(folder: &Path) -> Result<Vec<Region>, String> {
    let entries = fs::read_dir(folder).map_err(|error| error.to_string())?;
    let mut drums = Vec::new();
    let mut notes = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !AUDIO_EXTENSIONS.contains(&extension_of(&path).as_str()) {
            continue;
        }
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let stem = stem.to_string_lossy().to_string();
        if let Some(pad) = pad_named(&stem) {
            let note = gm_notes_of(pad)[0];
            drums.push(plain_region(path, note, note, note));
        } else if let Some(note) = note_in_name(&stem) {
            notes.push((note, path));
        }
    }
    drums.sort_by_key(|region| region.root);
    notes.sort_by_key(|(note, _)| *note);
    notes.dedup_by_key(|(note, _)| *note);

    let mut regions = drums;
    for (place, (root, path)) in notes.iter().enumerate() {
        let mut lowest = 0;
        if place > 0 {
            let below = notes[place - 1].0;
            lowest = (below + root) / 2 + 1;
        }
        let mut highest = 127;
        if place + 1 < notes.len() {
            let above = notes[place + 1].0;
            highest = (root + above) / 2;
        }
        regions.push(plain_region(path.clone(), lowest, highest, *root));
    }
    Ok(regions)
}

fn plain_region(sample: PathBuf, lowest: u8, highest: u8, root: u8) -> Region {
    Region {
        sample,
        lowest,
        highest,
        root,
        tune_cents: 0.0,
        gain: 1.0,
        looping: Looping::AsRecorded,
        loop_points: None,
        offset: 0,
        end: None,
    }
}

// The note a file is named by, from the last word of its name that is a
// note: "Piano C4" and "piano_c#4" are both. None if no word is.
fn note_in_name(stem: &str) -> Option<u8> {
    let words: Vec<&str> = stem.split([' ', '_', '-', '.']).collect();
    for word in words.iter().rev() {
        if let Some(note) = note_named(word) {
            return Some(note);
        }
    }
    // "C-1" is split by its dash, so it is tried whole too.
    note_named(stem)
}

// The MIDI number of a note name, middle C being C4 = 60: a letter, a
// sharp (# or s) or flat (b), then the octave, from -1 to 9. Case does not
// matter. None for anything else, or a note past 127.
pub fn note_named(name: &str) -> Option<u8> {
    let lower = name.trim().to_lowercase();
    let mut chars = lower.chars();
    let letter = chars.next()?;
    let mut semitone: i32 = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let mut rest: &str = &lower[1..];
    if rest.starts_with('#') || rest.starts_with('s') {
        semitone += 1;
        rest = &rest[1..];
    } else if rest.starts_with('b') {
        semitone -= 1;
        rest = &rest[1..];
    }
    let octave: i32 = rest.parse().ok()?;
    if !(-1..=9).contains(&octave) {
        return None;
    }
    let note = (octave + 1) * 12 + semitone;
    if !(0..=127).contains(&note) {
        return None;
    }
    Some(note as u8)
}

// A key number in an SFZ file: a number or a note name.
fn key_of(value: &str) -> Option<u8> {
    if let Ok(number) = value.trim().parse::<i32>() {
        if (0..=127).contains(&number) {
            return Some(number as u8);
        }
        return None;
    }
    note_named(value)
}

// The SFZ file at `path` read into regions. Only the parts Jello plays are
// read: which recording, its notes and the note it was recorded at, the
// velocities it is for, its tuning and volume, its loop and where it starts
// and ends. A region for another velocity, a release, a round robin past
// the first or a pedal is left out.
fn read_sfz(path: &Path) -> Result<Vec<Region>, String> {
    let mut defines = Vec::new();
    let text = sfz_text(path, &mut defines, 0)?;
    let folder = path.parent().unwrap_or(Path::new("."));

    let mut control: HashMap<String, String> = HashMap::new();
    let mut global: HashMap<String, String> = HashMap::new();
    let mut master: HashMap<String, String> = HashMap::new();
    let mut group: HashMap<String, String> = HashMap::new();
    let mut region: Option<HashMap<String, String>> = None;
    let mut heading = String::new();
    let mut regions = Vec::new();

    for part in sfz_parts(&text) {
        match part {
            SfzPart::Heading(name) => {
                if let Some(finished) = region.take() {
                    let merged = merged_opcodes(&[&global, &master, &group, &finished]);
                    if let Some(made) = region_of(&merged, &control, folder) {
                        regions.push(made);
                    }
                }
                if name == "global" {
                    global.clear();
                    master.clear();
                    group.clear();
                } else if name == "master" {
                    master.clear();
                    group.clear();
                } else if name == "group" {
                    group.clear();
                } else if name == "region" {
                    region = Some(HashMap::new());
                }
                heading = name;
            }
            SfzPart::Opcode(name, value) => {
                if heading == "control" {
                    control.insert(name, value);
                } else if heading == "global" {
                    global.insert(name, value);
                } else if heading == "master" {
                    master.insert(name, value);
                } else if heading == "group" {
                    group.insert(name, value);
                } else if heading == "region" {
                    if let Some(opcodes) = region.as_mut() {
                        opcodes.insert(name, value);
                    }
                }
            }
        }
    }
    if let Some(finished) = region.take() {
        let merged = merged_opcodes(&[&global, &master, &group, &finished]);
        if let Some(made) = region_of(&merged, &control, folder) {
            regions.push(made);
        }
    }
    Ok(regions)
}

// The opcodes of every level, the later ones winning.
fn merged_opcodes(levels: &[&HashMap<String, String>]) -> HashMap<String, String> {
    let mut merged = HashMap::new();
    for level in levels {
        for (name, value) in level.iter() {
            merged.insert(name.clone(), value.clone());
        }
    }
    merged
}

// The region a set of opcodes makes, or None for one Jello leaves out.
fn region_of(opcodes: &HashMap<String, String>, control: &HashMap<String, String>, folder: &Path) -> Option<Region> {
    let sample = opcodes.get("sample")?;

    // Only one layer, the first round robin, and no releases or pedals.
    let lovel = number_or(opcodes, "lovel", 1.0);
    let hivel = number_or(opcodes, "hivel", 127.0);
    let velocity = f64::from(KEPT_VELOCITY);
    if velocity < lovel || velocity > hivel {
        return None;
    }
    if let Some(trigger) = opcodes.get("trigger") {
        if trigger != "attack" {
            return None;
        }
    }
    if number_or(opcodes, "seq_position", 1.0) != 1.0 {
        return None;
    }
    if number_or(opcodes, "lorand", 0.0) > 0.0 {
        return None;
    }
    for name in opcodes.keys() {
        if name.starts_with("on_lo") || name.starts_with("on_hi") {
            return None;
        }
    }

    let mut lowest = key_or(opcodes, "lokey", 0)?;
    let mut highest = key_or(opcodes, "hikey", 127)?;
    let mut root = key_or(opcodes, "pitch_keycenter", 60).unwrap_or(60);
    if let Some(key) = opcodes.get("key") {
        let key = key_of(key)?;
        lowest = key;
        highest = key;
        root = key;
    }
    if lowest > highest {
        return None;
    }
    let shift = number_or(control, "note_offset", 0.0) + 12.0 * number_or(control, "octave_offset", 0.0);
    lowest = shifted(lowest, shift);
    highest = shifted(highest, shift);
    root = shifted(root, shift);

    // Tuning up plays the recording higher, as if it had been made lower,
    // so it counts against how far off the recording is. Whole half steps
    // go into the note it was recorded at.
    let off_cents = -number_or(opcodes, "tune", 0.0) - 100.0 * number_or(opcodes, "transpose", 0.0);
    let half_steps = (off_cents / 100.0).round();
    let recorded_at = f64::from(root) + half_steps;
    root = recorded_at.clamp(0.0, 127.0) as u8;
    let tune_cents = (off_cents - half_steps * 100.0) as f32;

    let decibels = number_or(opcodes, "volume", 0.0);
    let amplitude = number_or(opcodes, "amplitude", 100.0) / 100.0;
    let gain = (10f64.powf(decibels / 20.0) * amplitude) as f32;

    let looping = match opcodes.get("loop_mode").map(String::as_str) {
        Some("loop_continuous") => Looping::Loops,
        Some("loop_sustain") => Looping::Loops,
        Some("no_loop") => Looping::Once,
        Some("one_shot") => Looping::Once,
        _ => Looping::AsRecorded,
    };
    let loop_start = whole_number(opcodes, "loop_start").or(whole_number(opcodes, "loopstart"));
    let loop_end = whole_number(opcodes, "loop_end").or(whole_number(opcodes, "loopend"));
    let mut loop_points = None;
    if let (Some(start), Some(end)) = (loop_start, loop_end) {
        // An SFZ loop ends on its last frame; Jello's ends just after.
        if end >= start {
            loop_points = Some((start, end + 1));
        }
    }

    let default_path = control.get("default_path").cloned().unwrap_or_default();
    let relative = format!("{default_path}{sample}").replace('\\', "/");
    Some(Region {
        sample: folder.join(relative),
        lowest,
        highest,
        root,
        tune_cents,
        gain,
        looping,
        loop_points,
        offset: whole_number(opcodes, "offset").unwrap_or(0),
        end: whole_number(opcodes, "end").map(|last| last + 1),
    })
}

fn number_or(opcodes: &HashMap<String, String>, name: &str, otherwise: f64) -> f64 {
    match opcodes.get(name) {
        Some(value) => value.trim().parse().unwrap_or(otherwise),
        None => otherwise,
    }
}

fn whole_number(opcodes: &HashMap<String, String>, name: &str) -> Option<u64> {
    opcodes.get(name)?.trim().parse().ok()
}

// A key opcode's note, `otherwise` when it is left out, or None for one
// that can't be read.
fn key_or(opcodes: &HashMap<String, String>, name: &str, otherwise: u8) -> Option<u8> {
    match opcodes.get(name) {
        Some(value) => key_of(value),
        None => Some(otherwise),
    }
}

fn shifted(note: u8, shift: f64) -> u8 {
    (f64::from(note) + shift).clamp(0.0, 127.0) as u8
}

// The text of the SFZ file at `path` with its comments taken out, its
// #include lines replaced by the files they name and its #define names
// replaced by their values.
fn sfz_text(path: &Path, defines: &mut Vec<(String, String)>, depth: usize) -> Result<String, String> {
    if depth > MOST_INCLUDES {
        return Err(String::from("the SFZ files include each other too deeply"));
    }
    let raw = fs::read(path).map_err(|error| format!("{} can't be read: {error}", path.display()))?;
    let raw = String::from_utf8_lossy(&raw).to_string();
    let without_blocks = without_block_comments(&raw);
    let folder = path.parent().unwrap_or(Path::new("."));

    let mut text = String::new();
    for line in without_blocks.lines() {
        let line = match line.find("//") {
            Some(start) => &line[..start],
            None => line,
        };
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#define") {
            let mut words = rest.split_whitespace();
            if let (Some(name), Some(value)) = (words.next(), words.next()) {
                defines.push((String::from(name), String::from(value)));
                // The longest first, so $NOTE2 is never read as $NOTE.
                defines.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#include") {
            let included = rest.trim().trim_matches('"');
            let included = folder.join(included.replace('\\', "/"));
            text.push_str(&sfz_text(&included, defines, depth + 1)?);
            text.push('\n');
            continue;
        }
        let mut line = String::from(line);
        for (name, value) in defines.iter() {
            line = line.replace(name.as_str(), value);
        }
        text.push_str(&line);
        text.push('\n');
    }
    Ok(text)
}

fn without_block_comments(text: &str) -> String {
    let mut kept = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        kept.push_str(&rest[..start]);
        match rest[start..].find("*/") {
            Some(end) => rest = &rest[start + end + 2..],
            None => rest = "",
        }
    }
    kept.push_str(rest);
    kept
}

#[derive(Debug, PartialEq)]
enum SfzPart {
    Heading(String),
    Opcode(String, String),
}

// The headings and opcodes of SFZ text, in order. An opcode's value runs to
// the end of its line or to the next opcode or heading, so a recording's
// name may have spaces in it.
fn sfz_parts(text: &str) -> Vec<SfzPart> {
    let mut parts = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim();
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('<') {
                let Some(end) = after.find('>') else {
                    break;
                };
                parts.push(SfzPart::Heading(after[..end].trim().to_lowercase()));
                rest = after[end + 1..].trim_start();
                continue;
            }
            let Some(equals) = rest.find('=') else {
                break;
            };
            let name = rest[..equals].trim();
            let after = &rest[equals + 1..];
            let value_end = next_part_start(after);
            let value = after[..value_end].trim();
            if is_opcode_name(name) {
                parts.push(SfzPart::Opcode(String::from(name), String::from(value)));
            }
            rest = after[value_end..].trim_start();
        }
    }
    parts
}

// Where the next opcode or heading starts in `text`, or its end.
fn next_part_start(text: &str) -> usize {
    let bytes = text.as_bytes();
    for place in 0..bytes.len() {
        if bytes[place] == b'<' {
            return place;
        }
        if !bytes[place].is_ascii_whitespace() {
            continue;
        }
        // Whitespace, then a name, then "=".
        let mut end = place + 1;
        while end < bytes.len() && is_name_byte(bytes[end]) {
            end += 1;
        }
        if end > place + 1 && end < bytes.len() && bytes[end] == b'=' {
            return place;
        }
    }
    bytes.len()
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_opcode_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.bytes().all(is_name_byte)
}

// A recording as its file holds it.
#[derive(Debug)]
struct Recording {
    channels: u16,
    rate: u32,
    frames: Vec<f32>,
    // Its own loop, as a WAV file's sampler chunk says, ending just after.
    loop_points: Option<(u64, u64)>,
}

impl Recording {
    fn frame_count(&self) -> u64 {
        self.frames.len() as u64 / u64::from(self.channels)
    }
}

fn decode(path: &Path) -> Result<Recording, String> {
    let extension = extension_of(path);
    if extension == "wav" {
        return decode_wav(path);
    }
    if extension == "flac" {
        return decode_flac(path);
    }
    Err(format!("{} is not a WAV or FLAC file", path.display()))
}

fn decode_wav(path: &Path) -> Result<Recording, String> {
    let cant = |error: hound::Error| format!("{} can't be read: {error}", path.display());
    let mut reader = hound::WavReader::open(path).map_err(cant)?;
    let spec = reader.spec();
    let mut samples = Vec::new();
    if spec.sample_format == hound::SampleFormat::Float {
        for sample in reader.samples::<f32>() {
            samples.push(sample.map_err(cant)?);
        }
    } else {
        let full = (1u64 << (spec.bits_per_sample - 1)) as f32;
        for sample in reader.samples::<i32>() {
            samples.push(sample.map_err(cant)? as f32 / full);
        }
    }
    let (channels, frames) = first_two_channels(samples, spec.channels);
    Ok(Recording {
        channels,
        rate: spec.sample_rate,
        frames,
        loop_points: wav_loop(path),
    })
}

fn decode_flac(path: &Path) -> Result<Recording, String> {
    let cant = |error: claxon::Error| format!("{} can't be read: {error}", path.display());
    let mut reader = claxon::FlacReader::open(path).map_err(cant)?;
    let info = reader.streaminfo();
    let full = (1u64 << (info.bits_per_sample - 1)) as f32;
    let mut samples = Vec::new();
    for sample in reader.samples() {
        samples.push(sample.map_err(cant)? as f32 / full);
    }
    let (channels, frames) = first_two_channels(samples, info.channels as u16);
    Ok(Recording {
        channels,
        rate: info.sample_rate,
        frames,
        loop_points: None,
    })
}

// Interleaved `samples` with `channels` channels, keeping at most the first
// two.
fn first_two_channels(samples: Vec<f32>, channels: u16) -> (u16, Vec<f32>) {
    if channels <= 2 {
        return (channels.max(1), samples);
    }
    let mut kept = Vec::new();
    for frame in samples.chunks(usize::from(channels)) {
        if frame.len() >= 2 {
            kept.push(frame[0]);
            kept.push(frame[1]);
        }
    }
    (2, kept)
}

// The first loop in a WAV file's sampler ("smpl") chunk, or None.
fn wav_loop(path: &Path) -> Option<(u64, u64)> {
    let mut bytes = Vec::new();
    fs::File::open(path).ok()?.read_to_end(&mut bytes).ok()?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut place = 12;
    while place + 8 <= bytes.len() {
        let id = &bytes[place..place + 4];
        let size = u32_at(&bytes, place + 4)? as usize;
        let body = place + 8;
        if id == b"smpl" {
            let loop_count = u32_at(&bytes, body + 28)?;
            if loop_count == 0 {
                return None;
            }
            // Each loop: id, type, start, end (its last frame), ...
            let start = u32_at(&bytes, body + 36 + 8)?;
            let end = u32_at(&bytes, body + 36 + 12)?;
            if end < start {
                return None;
            }
            return Some((u64::from(start), u64::from(end) + 1));
        }
        // Chunks are padded to an even length.
        place = body + size + (size % 2);
    }
    None
}

fn u32_at(bytes: &[u8], place: usize) -> Option<u32> {
    let four = bytes.get(place..place + 4)?;
    Some(u32::from_le_bytes([four[0], four[1], four[2], four[3]]))
}

// The zone `region` plays from `recording`: cut to where the region starts
// and ends, at 48,000 frames a second, at its gain, and, unless it loops,
// at most LONGEST_SECONDS long.
fn zone_of(region: &Region, recording: &Recording) -> Zone {
    let channels = usize::from(recording.channels);
    let count = recording.frame_count();
    let start = region.offset.min(count);
    let end = region.end.unwrap_or(count).clamp(start, count);
    let cut = &recording.frames[start as usize * channels..end as usize * channels];
    let mut frames = resampled(cut, recording.channels, recording.rate);
    for sample in frames.iter_mut() {
        *sample *= region.gain;
    }

    let mut loop_start = 0;
    let mut loop_end = 0;
    let loop_points = match region.looping {
        Looping::Loops => region.loop_points.or(recording.loop_points),
        Looping::Once => None,
        Looping::AsRecorded => region.loop_points.or(recording.loop_points),
    };
    let frame_total = frames.len() as u64 / u64::from(recording.channels);
    if let Some((first, after)) = loop_points {
        let first = rescaled(first.saturating_sub(start), recording.rate);
        let after = rescaled(after.saturating_sub(start), recording.rate).min(frame_total);
        if after > first {
            loop_start = first;
            loop_end = after;
        }
    }
    if loop_end == loop_start {
        cut_long(&mut frames, recording.channels);
    }

    Zone {
        lowest: region.lowest,
        highest: region.highest,
        root: region.root,
        tune_cents: region.tune_cents,
        channels: recording.channels,
        frames,
        loop_start,
        loop_end,
    }
}

// A frame number at `rate` turned into one at SAMPLE_RATE.
fn rescaled(frame: u64, rate: u32) -> u64 {
    frame * u64::from(SAMPLE_RATE) / u64::from(rate)
}

// `frames` at `rate` frames a second, turned into SAMPLE_RATE, read between
// frames the way the engine does (4-point Hermite).
fn resampled(frames: &[f32], channels: u16, rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE || rate == 0 {
        return frames.to_vec();
    }
    let channels = usize::from(channels);
    let count = frames.len() / channels;
    let new_count = (count as u64 * u64::from(SAMPLE_RATE) / u64::from(rate)) as usize;
    let step = f64::from(rate) / f64::from(SAMPLE_RATE);
    let at = |index: i64, channel: usize| -> f64 {
        if index < 0 || index as usize >= count {
            return 0.0;
        }
        f64::from(frames[index as usize * channels + channel])
    };
    let mut made = Vec::with_capacity(new_count * channels);
    for frame in 0..new_count {
        let place = frame as f64 * step;
        let whole = place.floor();
        let share = place - whole;
        let index = whole as i64;
        for channel in 0..channels {
            let height = hermite(
                at(index - 1, channel),
                at(index, channel),
                at(index + 1, channel),
                at(index + 2, channel),
                share,
            );
            made.push(height as f32);
        }
    }
    made
}

fn hermite(before: f64, a: f64, b: f64, after: f64, t: f64) -> f64 {
    let c1 = 0.5 * (b - before);
    let c2 = before - 2.5 * a + 2.0 * b - 0.5 * after;
    let c3 = 0.5 * (after - before) + 1.5 * (a - b);
    ((c3 * t + c2) * t + c1) * t + a
}

// Cuts a recording longer than LONGEST_SECONDS to that, fading out over
// its last CUT_FADE_MS.
fn cut_long(frames: &mut Vec<f32>, channels: u16) {
    let channels = usize::from(channels);
    let longest = LONGEST_SECONDS as usize * SAMPLE_RATE as usize;
    if frames.len() / channels <= longest {
        return;
    }
    frames.truncate(longest * channels);
    let fade = (CUT_FADE_MS * u64::from(SAMPLE_RATE) / 1000) as usize;
    for step in 0..fade {
        let frame = longest - fade + step;
        let left = (fade - step) as f32 / fade as f32;
        for channel in 0..channels {
            frames[frame * channels + channel] *= left;
        }
    }
}

// Makes every zone louder or quieter by the same amount, so the loudest
// frame of all is at PEAK.
fn make_as_loud(zones: &mut [Zone]) {
    let mut loudest: f32 = 0.0;
    for zone in zones.iter() {
        for sample in &zone.frames {
            loudest = loudest.max(sample.abs());
        }
    }
    if loudest == 0.0 {
        return;
    }
    let gain = PEAK / loudest;
    for zone in zones.iter_mut() {
        for sample in zone.frames.iter_mut() {
            *sample *= gain;
        }
    }
}

// Test instruments, made once in a folder of their own, for the tests that
// play them: "Test tone", a sine recorded at C4 and at C5, and "Test kit",
// a kick, a closed hat and an open hat.
#[cfg(test)]
pub mod testing {
    use super::*;

    pub const TONE: &str = "Test tone";
    pub const KIT: &str = "Test kit";

    // How long each test recording lasts, in frames.
    pub const TONE_FRAMES: usize = 48000;
    pub const KICK_FRAMES: usize = 4800;
    pub const OPEN_HAT_FRAMES: usize = 24000;

    // Makes the test instruments' folder the one read from.
    pub fn use_test_folder() {
        static MADE: OnceLock<PathBuf> = OnceLock::new();
        let folder = MADE.get_or_init(|| {
            let folder = std::env::temp_dir().join(format!("jello-test-instruments-{}", std::process::id()));
            let _ = fs::remove_dir_all(&folder);
            let tone = folder.join(TONE);
            fs::create_dir_all(&tone).unwrap();
            write_wav(&tone.join("C4.wav"), &sine(261.6256, TONE_FRAMES));
            write_wav(&tone.join("C5.wav"), &sine(523.2511, TONE_FRAMES));
            let kit = folder.join(KIT);
            fs::create_dir_all(&kit).unwrap();
            write_wav(&kit.join("kick.wav"), &sine(60.0, KICK_FRAMES));
            write_wav(&kit.join("closed hat.wav"), &sine(3000.0, KICK_FRAMES));
            write_wav(&kit.join("open_hat.wav"), &sine(5000.0, OPEN_HAT_FRAMES));
            folder
        });
        set_folder(folder.clone());
    }

    pub fn sine(hz: f64, frames: usize) -> Vec<f32> {
        let mut samples = Vec::new();
        for frame in 0..frames {
            let phase = frame as f64 * hz / f64::from(SAMPLE_RATE);
            samples.push((phase * std::f64::consts::TAU).sin() as f32 * 0.5);
        }
        samples
    }

    // A mono float WAV at SAMPLE_RATE.
    pub fn write_wav(path: &Path, samples: &[f32]) {
        write_wav_at(path, samples, 1, SAMPLE_RATE);
    }

    pub fn write_wav_at(path: &Path, samples: &[f32], channels: u16, rate: u32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    // A new empty folder for one test.
    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("jello-instruments-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    // How many times `samples` (mono) cross zero going up.
    fn rises(samples: &[f32]) -> usize {
        let mut count = 0;
        for pair in samples.windows(2) {
            if pair[0] < 0.0 && pair[1] >= 0.0 {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn note_names_are_read_with_middle_c_at_60() {
        assert_eq!(note_named("C4"), Some(60));
        assert_eq!(note_named("c#4"), Some(61));
        assert_eq!(note_named("Db4"), Some(61));
        assert_eq!(note_named("Fs2"), Some(42));
        assert_eq!(note_named("A0"), Some(21));
        assert_eq!(note_named("C-1"), Some(0));
        assert_eq!(note_named("G9"), Some(127));
        assert_eq!(note_named("G#9"), None);
        assert_eq!(note_named("H4"), None);
        assert_eq!(note_named("C10"), None);
        assert_eq!(note_named("kick"), None);
        assert_eq!(note_in_name("Piano_C#4"), Some(61));
        assert_eq!(note_in_name("Grand A0 soft"), Some(21));
        assert_eq!(note_in_name("C-1"), Some(0));
    }

    #[test]
    fn names_that_reach_outside_the_folder_are_refused() {
        assert!(!is_plain_name("../Secret"));
        assert!(!is_plain_name("a/b"));
        assert!(!is_plain_name(".."));
        assert!(!is_plain_name(" "));
        assert!(is_plain_name("Grand piano"));
    }

    #[test]
    fn recordings_named_by_note_play_halfway_to_their_neighbours() {
        let folder = scratch("names");
        for name in ["C4.wav", "E4.wav", "C5.wav", "notes.txt"] {
            write_wav(&folder.join(name), &sine(440.0, 100));
        }
        write_wav(&folder.join("snare.wav"), &sine(200.0, 100));
        let regions = regions_from_names(&folder).unwrap();
        let ranges: Vec<(u8, u8, u8)> = regions.iter().map(|r| (r.lowest, r.highest, r.root)).collect();
        // The snare first, on its General MIDI note, then the notes.
        assert_eq!(ranges, vec![(38, 38, 38), (0, 62, 60), (63, 68, 64), (69, 127, 72)]);
    }

    #[test]
    fn an_sfz_file_keeps_one_layer_and_reads_its_tuning_loops_and_includes() {
        let folder = scratch("sfz");
        fs::create_dir_all(folder.join("samples")).unwrap();
        write_wav(&folder.join("samples/low soft.wav"), &sine(100.0, 480));
        write_wav(&folder.join("samples/low hard.wav"), &sine(100.0, 480));
        write_wav(&folder.join("samples/high.wav"), &sine(100.0, 480));
        write_wav(&folder.join("samples/release.wav"), &sine(100.0, 480));
        fs::write(
            folder.join("more.sfz"),
            "<region> sample=high.wav lokey=c5 hikey=127 pitch_keycenter=72 transpose=1 tune=-30\n\
             <region> sample=high.wav lokey=c5 hikey=127 seq_position=2\n\
             <region> sample=high.wav lokey=c5 hikey=127 lorand=0.5 hirand=1\n",
        )
        .unwrap();
        fs::write(
            folder.join("piano.sfz"),
            "/* a block\n comment */ #define $LOW 40\n\
             <control> default_path=samples/ // a comment\n\
             <global> volume=-6\n\
             <group> lokey=0 hikey=$LOW pitch_keycenter=36\n\
             <region> sample=low soft.wav lovel=1 hivel=64\n\
             <region> sample=low hard.wav lovel=65 hivel=127 loop_mode=loop_continuous loop_start=10 loop_end=109\n\
             <group> trigger=release\n\
             <region> sample=release.wav\n\
             <group>\n\
             #include \"more.sfz\"\n",
        )
        .unwrap();
        let regions = read_sfz(&folder.join("piano.sfz")).unwrap();
        assert_eq!(regions.len(), 2);

        let low = &regions[0];
        assert_eq!(low.sample, folder.join("samples/low hard.wav"));
        assert_eq!((low.lowest, low.highest, low.root), (0, 40, 36));
        assert!((low.gain - 0.501).abs() < 0.001);
        assert_eq!(low.looping, Looping::Loops);
        assert_eq!(low.loop_points, Some((10, 110)));

        // Up a half step and down 30 cents: as if recorded 70 cents lower
        // than C5, which is 30 cents above B4.
        let high = &regions[1];
        assert_eq!((high.lowest, high.highest), (72, 127));
        assert_eq!(high.root, 71);
        assert!((high.tune_cents - 30.0).abs() < 0.001);
        // The global volume reaches every region after it.
        assert!((high.gain - 0.501).abs() < 0.001);
    }

    #[test]
    fn a_recording_at_another_rate_keeps_its_pitch() {
        let folder = scratch("rate");
        let mut samples = Vec::new();
        for frame in 0..44100 {
            let phase = frame as f64 * 441.0 / 44100.0;
            samples.push((phase * std::f64::consts::TAU).sin() as f32);
        }
        write_wav_at(&folder.join("A4.wav"), &samples, 1, 44100);
        let instrument = read_instrument(&folder).unwrap();
        let zone = &instrument.zones[0];
        assert_eq!(zone.frame_count(), 48000);
        let rises = rises(&zone.frames);
        assert!((440..=442).contains(&rises), "{rises} rises in a second");
        // Made as loud as an instrument is.
        let loudest = zone.frames.iter().fold(0.0f32, |most, s| most.max(s.abs()));
        assert!((loudest - PEAK).abs() < 0.001);
    }

    #[test]
    fn stereo_stays_stereo_and_more_channels_keep_the_first_two() {
        let folder = scratch("channels");
        let mut four = Vec::new();
        for frame in 0..100 {
            let height = frame as f32 / 200.0;
            four.extend([height, -height, 0.9, 0.9]);
        }
        write_wav_at(&folder.join("C4.wav"), &four, 4, SAMPLE_RATE);
        let instrument = read_instrument(&folder).unwrap();
        let zone = &instrument.zones[0];
        assert_eq!(zone.channels, 2);
        assert_eq!(zone.frame_count(), 100);
        assert!(zone.frames[3] < 0.0);
    }

    #[test]
    fn a_long_recording_is_cut_with_a_fade_unless_it_loops() {
        let mut long = vec![0.5f32; (LONGEST_SECONDS as usize + 2) * SAMPLE_RATE as usize];
        cut_long(&mut long, 1);
        assert_eq!(long.len(), LONGEST_SECONDS as usize * SAMPLE_RATE as usize);
        assert!(long[long.len() - 1] < 0.001);
        assert_eq!(long[0], 0.5);

        let region = Region {
            looping: Looping::Loops,
            loop_points: Some((100, 200)),
            ..plain_region(PathBuf::new(), 0, 127, 60)
        };
        let recording = Recording {
            channels: 1,
            rate: SAMPLE_RATE,
            frames: vec![0.5; (LONGEST_SECONDS as usize + 2) * SAMPLE_RATE as usize],
            loop_points: None,
        };
        let zone = zone_of(&region, &recording);
        assert_eq!(zone.frame_count(), recording.frame_count());
        assert_eq!((zone.loop_start, zone.loop_end), (100, 200));
    }

    #[test]
    fn a_wav_file_s_own_loop_is_read() {
        let folder = scratch("smpl");
        let path = folder.join("C4.wav");
        write_wav(&path, &sine(440.0, 1000));
        // A sampler chunk with one loop from frame 100 to 899.
        let mut chunk = vec![0u8; 36 + 24];
        chunk[28..32].copy_from_slice(&1u32.to_le_bytes());
        chunk[44..48].copy_from_slice(&100u32.to_le_bytes());
        chunk[48..52].copy_from_slice(&899u32.to_le_bytes());
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend(b"smpl");
        bytes.extend((chunk.len() as u32).to_le_bytes());
        bytes.extend(chunk);
        let riff_size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
        fs::write(&path, bytes).unwrap();

        assert_eq!(wav_loop(&path), Some((100, 900)));
        let instrument = read_instrument(&folder).unwrap();
        assert_eq!((instrument.zones[0].loop_start, instrument.zones[0].loop_end), (100, 900));
    }

    #[test]
    fn a_flac_file_is_read() {
        let folder = scratch("flac");
        fs::write(folder.join("A4.flac"), include_bytes!("test_data/a4.flac")).unwrap();
        let instrument = read_instrument(&folder).unwrap();
        let zone = &instrument.zones[0];
        assert_eq!(zone.root, 69);
        // A tenth of a second of 440 Hz, at 44,100 frames a second.
        assert_eq!(zone.frame_count(), 4800);
        let rises = rises(&zone.frames);
        assert!((43..=45).contains(&rises), "{rises} rises");
    }

    #[test]
    fn an_empty_or_missing_instrument_says_why() {
        let folder = scratch("empty");
        let empty = read_instrument(&folder);
        assert!(empty.unwrap_err().contains("no recordings"));
        let missing = read_named(&folder, "Nothing here");
        assert_eq!(missing.unwrap_err(), "there is no instrument called Nothing here");
    }

    #[test]
    fn the_test_instruments_are_listed_and_read_once() {
        use_test_folder();
        let names = list();
        assert!(names.contains(&String::from(TONE)));
        assert!(names.contains(&String::from(KIT)));
        let first = load(TONE).unwrap();
        let again = load(TONE).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(status(TONE), Status::Read);
        assert_eq!(first.zones.len(), 2);
    }
}
