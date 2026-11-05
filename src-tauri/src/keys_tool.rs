// The song's keys (docs/adr/0047-song-key-mappings.md): jello_keys, the agent's
// tool for them, and the commands behind the mapping page, the scale pickers,
// the optimizer's card and the marks for what comes next.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::follow_picked_track;
use crate::compose::{ComposeView, Composer};
use crate::engine::{BEATS_PER_BAR, Engine, LooperState};
use crate::key_map::{KeyMap, LAPTOP_VELOCITY, LayoutForUi, name_of_notes};
use crate::guitar::{GuitarMapping, STRING_COUNT, STRING_KEYS, STRUM_SLOWEST_MS, shape_keys};
use crate::key_mapping::{KeyMapping, mappable_keys};
use crate::key_optimizer::{optimize, usage_of};
use crate::key_stats::KeyStats;
use crate::presets::{set_setting, setting};
use crate::scales::{Scale, ScaleKind};
use crate::song_file::{Song, SongLayer, beats_of, note_name, note_of_name};
use crate::song_session::{OpenSong, loop_now, song_now};
use crate::tools::{Studio, ToolAnswer, optional_text_argument, text_argument};

// The actions jello_keys takes.
pub const ACTIONS: [&str; 8] = ["look", "scale", "set", "lock", "reset", "own", "optimize", "guitar"];

// The setting that names the scale the Play tab opened on last.
const PLAY_SCALE: &str = "play_scale";

// How many offers are kept, so an old card's Try and Use still work.
const KEPT_OFFERS: usize = 8;

// How many marks ahead the keys show: the next press and the ones after it.
const MARKS_AHEAD: usize = 3;

// The tool as MCP's tools/list describes it.
pub fn description() -> Value {
    json!({
        "name": "jello_keys",
        "description": "The song's key mapping: which notes each laptop key plays. Nothing is fixed: \
            any key can play any note or any set of notes, and the player reads the lit keys. Keys \
            play nothing until a scale is picked. A song has a main mapping, and a section may have \
            its own. look: the scale and every key. scale: picks the song's scale, like \"A dorian\" \
            (major, minor, dorian, mixolydian, lydian, harmonic minor, melodic minor, phrygian, \
            major pentatonic, minor pentatonic, blues); every unlocked key goes to its default and \
            sections go back to the main mapping. set: puts notes (names like [\"C4\", \"E4\"], empty \
            clears it) on key (a place like \"KeyF\"). lock: locks key, or with locked false lets it \
            go; a locked key never moves. reset: puts unlocked keys back on the scale's default. \
            own: with section (a name), on true gives it its own copy of the main mapping, on false \
            puts it back on the main one. optimize: finds the most comfortable mapping for the \
            song's notes and chords, or a section's, and shows the player a card with the moves and \
            the comfort gain, where Try plays it unsaved and Use saves it. It changes nothing by \
            itself. guitar: on a track that plays a string kind the same keys are a guitar: U I O P \
            [ ] are the strings, thickest first, and the keys from 1QAZ to 5TGB hold shapes. Each \
            scale has a default. With key, name and strings (six note names, thickest first, \
            \"\" for a silent string) it changes one shape; with strings alone it tunes the open \
            strings; with strum_ms (0 to 100) it sets how long each string of a strum waits after \
            the one before (J strums down, K up; 25 to start). set, lock, reset, guitar and optimize take an optional section. Refused while the song \
            plays: scale and optimize. Answers with the mapping.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ACTIONS},
                "scale": {"type": "string"},
                "key": {"type": "string"},
                "notes": {"type": "array", "items": {"type": "string"}},
                "name": {"type": "string"},
                "strings": {"type": "array", "items": {"type": "string"}},
                "strum_ms": {"type": "integer"},
                "locked": {"type": "boolean"},
                "section": {"type": "string"},
                "on": {"type": "boolean"},
            },
            "required": ["action"],
        },
    })
}

// An optimized mapping waiting for Try or Use.
#[derive(Clone)]
struct KeyOffer {
    number: u64,
    section: Option<usize>,
    // The song's mapping when it was found: Use refuses if it moved since.
    before: KeyMapping,
    after: KeyMapping,
}

#[derive(Default)]
pub struct KeyOffers {
    next: u64,
    kept: VecDeque<KeyOffer>,
}

impl KeyOffers {
    fn keep(&mut self, section: Option<usize>, before: KeyMapping, after: KeyMapping) -> u64 {
        self.next += 1;
        self.kept.push_back(KeyOffer {
            number: self.next,
            section,
            before,
            after,
        });
        if self.kept.len() > KEPT_OFFERS {
            self.kept.pop_front();
        }
        self.next
    }

    fn get(&self, number: u64) -> Result<KeyOffer, String> {
        for offer in &self.kept {
            if offer.number == number {
                return Ok(offer.clone());
            }
        }
        Err(String::from("that mapping is too old to use: ask the agent again"))
    }
}

// The mapping `section` plays, from 0, or the main one with None. A section
// with none of its own plays the main one.
fn mapping_of(composer: &Composer, section: Option<usize>) -> Result<KeyMapping, String> {
    let Some(number) = section else {
        return Ok(composer.main_keys().clone());
    };
    let Some(found) = composer.sections().get(number) else {
        return Err(format!("the song has no section {}", number + 1));
    };
    match &found.keys {
        Some(keys) => Ok(keys.clone()),
        None => Ok(composer.main_keys().clone()),
    }
}

// Keeps `keys` as `section`'s, or as the main mapping with None.
fn keep_mapping(composer: &mut Composer, section: Option<usize>, keys: KeyMapping) -> Result<(), String> {
    match section {
        Some(number) => composer.sections_mut().set_keys(number, Some(keys)),
        None => {
            composer.set_main_keys(keys);
            Ok(())
        }
    }
}

// The mapping as the agent and the page read it.
fn mapping_json(composer: &Composer, section: Option<usize>) -> Result<Value, String> {
    let keys = mapping_of(composer, section)?;
    let mut listed = Vec::new();
    for (key, notes) in &keys.keys {
        let mut names = Vec::new();
        for note in notes {
            names.push(note_name(*note));
        }
        listed.push(json!({"key": key, "notes": names, "plays": name_of_notes(notes)}));
    }
    let mut scale = Value::Null;
    if let Some(picked) = composer.scale() {
        scale = json!(picked.name());
    }
    let mut own_sections = Vec::new();
    for found in composer.sections() {
        if found.keys.is_some() {
            own_sections.push(found.name.clone());
        }
    }
    Ok(json!({
        "scale": scale,
        "section": section_name(composer, section),
        "keys": listed,
        "locked": keys.locked,
        "guitar": guitar_json(&keys.guitar),
        "sections_with_their_own": own_sections,
    }))
}

// A guitar as the agent reads it, by note name.
fn guitar_json(guitar: &GuitarMapping) -> Value {
    let mut shapes = Vec::new();
    for (key, shape) in &guitar.shapes {
        shapes.push(json!({"key": key, "name": shape.name, "strings": string_names(&shape.strings)}));
    }
    json!({"open": string_names(&guitar.open), "shapes": shapes, "strum_ms": guitar.strum_ms})
}

fn string_names(strings: &[Option<u8>; STRING_COUNT]) -> Vec<String> {
    let mut names = Vec::new();
    for note in strings {
        match note {
            Some(note) => names.push(note_name(*note)),
            None => names.push(String::new()),
        }
    }
    names
}

fn strings_argument(arguments: &Value) -> Result<[Option<u8>; STRING_COUNT], String> {
    let wanted = "strings is needed: six notes, thickest first, like [\"E2\", \"A2\", \"D3\", \"G3\", \"B3\", \"E4\"], \"\" for a silent string";
    let Some(names) = arguments.get("strings").and_then(Value::as_array) else {
        return Err(String::from(wanted));
    };
    if names.len() != STRING_COUNT {
        return Err(String::from(wanted));
    }
    let mut strings = [None; STRING_COUNT];
    for (string, name) in names.iter().enumerate() {
        let name = name.as_str().unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let Some(note) = note_of_name(name) else {
            return Err(format!("{name} is not a note, like \"C4\" or \"F#3\""));
        };
        strings[string] = Some(note);
    }
    Ok(strings)
}

fn section_name(composer: &Composer, section: Option<usize>) -> Value {
    let Some(number) = section else {
        return Value::Null;
    };
    match composer.sections().get(number) {
        Some(found) => json!(found.name),
        None => Value::Null,
    }
}

fn section_named(composer: &Composer, name: &str) -> Result<usize, String> {
    for (number, found) in composer.sections().iter().enumerate() {
        if found.name == name {
            return Ok(number);
        }
    }
    Err(format!("the song has no section called {name}"))
}

fn section_argument(composer: &Composer, arguments: &Value) -> Result<Option<usize>, String> {
    match optional_text_argument(arguments, "section")? {
        Some(name) => Ok(Some(section_named(composer, &name)?)),
        None => Ok(None),
    }
}

// jello_keys.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    let mut key_map = studio.key_map.lock().unwrap();
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    let section = section_argument(&composer, arguments)?;

    let change = match action.as_str() {
        "look" => None,
        "scale" => {
            if composer.is_playing(&engine) {
                return Err(String::from("the song is playing: stop it before changing the scale"));
            }
            let name = text_argument(arguments, "scale")?;
            let Some(scale) = Scale::of_name(&name) else {
                return Err(format!("{name} is not a scale, like \"A minor\" or \"D dorian\""));
            };
            composer.pick_scale(scale);
            Some(format!("scale {}", scale.name()))
        }
        "set" => {
            let key = text_argument(arguments, "key")?;
            let notes = notes_argument(arguments)?;
            let mut keys = mapping_of(&composer, section)?;
            keys.set_key(&key, &notes)?;
            keep_mapping(&mut composer, section, keys)?;
            Some(format!("key {key}"))
        }
        "lock" => {
            let key = text_argument(arguments, "key")?;
            let locked = arguments.get("locked").and_then(Value::as_bool).unwrap_or(true);
            let mut keys = mapping_of(&composer, section)?;
            keys.set_locked(&key, locked)?;
            keep_mapping(&mut composer, section, keys)?;
            Some(format!("locked {key}"))
        }
        "guitar" => Some(change_guitar(&mut composer, section, arguments)?),
        "reset" => {
            let Some(scale) = composer.scale() else {
                return Err(String::from("pick a scale first"));
            };
            let keys = mapping_of(&composer, section)?;
            keep_mapping(&mut composer, section, keys.with_default_for(scale))?;
            Some(String::from("reset keys"))
        }
        "own" => {
            let Some(number) = section else {
                return Err(String::from("own needs a section, by its name"));
            };
            let on = arguments.get("on").and_then(Value::as_bool).unwrap_or(true);
            let mut keys = None;
            if on {
                keys = Some(mapping_of(&composer, section)?);
            }
            composer.sections_mut().set_keys(number, keys)?;
            Some(String::from("section keys"))
        }
        "optimize" => {
            if composer.is_playing(&engine) {
                return Err(String::from("the song is playing, and nothing changes while it plays: stop it first"));
            }
            let song = song_now(&composer, &mut engine);
            let card = offer_optimized(studio, &song, section)?;
            let answer = mapping_json(&composer, section)?;
            return Ok(ToolAnswer::of(answer, Vec::new()).with_card(card));
        }
        _ => {
            let all = ACTIONS.join(", ");
            return Err(format!("jello_keys has no action {action}: there are {all}"));
        }
    };

    key_map.follow(composer.keys_now());
    let answer = mapping_json(&composer, section)?;
    let changes = match change {
        Some(change) => vec![change],
        None => Vec::new(),
    };
    Ok(ToolAnswer::of(answer, changes))
}

// jello_keys guitar: the strum's speed, one shape, or the open strings.
fn change_guitar(composer: &mut Composer, section: Option<usize>, arguments: &Value) -> Result<String, String> {
    let mut keys = mapping_of(composer, section)?;
    let change;
    if let Some(ms) = arguments.get("strum_ms") {
        let Some(ms) = ms.as_u64() else {
            return Err(format!("strum_ms is a whole number of ms, 0 to {STRUM_SLOWEST_MS}"));
        };
        let ms = u32::try_from(ms).unwrap_or(u32::MAX);
        keys.guitar.set_strum_ms(ms)?;
        change = format!("strum {ms} ms");
    } else {
        let strings = strings_argument(arguments)?;
        match optional_text_argument(arguments, "key")? {
            Some(key) => {
                let name = optional_text_argument(arguments, "name")?.unwrap_or_default();
                keys.guitar.set_shape(&key, &name, strings)?;
                change = format!("guitar shape {key}");
            }
            None => {
                keys.guitar.set_open(strings)?;
                change = String::from("guitar tuning");
            }
        }
    }
    keep_mapping(composer, section, keys)?;
    Ok(change)
}

fn notes_argument(arguments: &Value) -> Result<Vec<u8>, String> {
    let Some(names) = arguments.get("notes").and_then(Value::as_array) else {
        return Err(String::from("notes is needed: a list like [\"C4\", \"E4\"], empty to clear the key"));
    };
    let mut notes = Vec::new();
    for name in names {
        let name = name.as_str().unwrap_or_default();
        let Some(note) = note_of_name(name) else {
            return Err(format!("{name} is not a note, like \"C4\" or \"F#3\""));
        };
        notes.push(note);
    }
    Ok(notes)
}

// Finds the most comfortable mapping for `song` and keeps it as an offer. The
// card shows what moves, and how much more comfortable it is.
fn offer_optimized(studio: &Studio, song: &Song, section: Option<usize>) -> Result<Value, String> {
    let Some(scale) = song.scale else {
        return Err(String::from("pick a scale first"));
    };
    let mut current = song.keys.clone();
    let mut main = None;
    if let Some(number) = section {
        let Some(found) = song.sections.get(number) else {
            return Err(format!("the song has no section {}", number + 1));
        };
        main = Some(&song.keys);
        if let Some(keys) = &found.keys {
            current = keys.clone();
        }
    }
    if current.is_empty() {
        current = KeyMapping::default_for(scale);
    }
    let usage = usage_of(song, section, &current);
    if usage.is_empty() {
        return Err(String::from("there are no notes or chords to optimize from yet"));
    }
    let found = {
        let stats = studio.key_stats.lock().unwrap();
        optimize(&current, &usage, &stats, main)
    };

    let moves = moves_between(&current, &found.after);
    let mut left_out = Vec::new();
    for notes in &found.left_out {
        left_out.push(name_of_notes(notes));
    }
    let number = studio.key_offers.lock().unwrap().keep(section, current, found.after);
    let mut scope = String::from("the whole song");
    if let Some(number) = section {
        scope = song.sections[number].name.clone();
    }
    let gain = (found.gain_percent * 10.0).round() / 10.0;
    Ok(json!({
        "kind": "mapping",
        "title": format!("More comfortable keys for {scope}"),
        "offer": number,
        "gain_percent": gain,
        "moves": moves,
        "left_out": left_out,
    }))
}

// Each key whose notes differ between two mappings.
fn moves_between(before: &KeyMapping, after: &KeyMapping) -> Vec<Value> {
    let mut keys: Vec<&String> = before.keys.keys().collect();
    for key in after.keys.keys() {
        if !before.keys.contains_key(key) {
            keys.push(key);
        }
    }
    keys.sort();
    let mut moves = Vec::new();
    for key in keys {
        let was = before.keys.get(key);
        let now = after.keys.get(key);
        if was == now {
            continue;
        }
        moves.push(json!({"key": key, "was": words_of(was), "now": words_of(now)}));
    }
    moves
}

fn words_of(notes: Option<&Vec<u8>>) -> String {
    match notes {
        Some(notes) => name_of_notes(notes),
        None => String::from("nothing"),
    }
}

// Try on an optimized mapping's card: plays it, unsaved, until Stop, Use or
// any change to the song's own keys.
#[tauri::command]
pub fn try_key_offer(
    app: AppHandle,
    offers: State<'_, Mutex<KeyOffers>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    offer: u64,
) -> Result<LayoutForUi, String> {
    let offer = offers.lock().unwrap().get(offer)?;
    let mut key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    try_offer(offer, &mut key_map, &composer, &engine)?;
    let _ = app.emit("agent-acted", json!({"tool": "jello_keys", "changes": []}));
    Ok(key_map.layout_for_ui())
}

#[tauri::command]
pub fn stop_trying_keys(
    app: AppHandle,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
) -> LayoutForUi {
    let mut key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    key_map.set_trying(None);
    follow_picked_track(&mut key_map, &composer);
    let _ = app.emit("agent-acted", json!({"tool": "jello_keys", "changes": []}));
    key_map.layout_for_ui()
}

// Use on an optimized mapping's card: saves it as the song's or the section's.
// Refused if those keys changed since it was found, so nothing is overwritten.
#[tauri::command]
pub fn use_key_offer(
    app: AppHandle,
    offers: State<'_, Mutex<KeyOffers>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    offer: u64,
) -> Result<LayoutForUi, String> {
    let offer = offers.lock().unwrap().get(offer)?;
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    use_offer(offer, &mut key_map, &mut composer, &engine)?;
    let _ = app.emit("agent-acted", json!({"tool": "jello_keys", "changes": ["keys"]}));
    Ok(key_map.layout_for_ui())
}

fn refuse_while_playing(composer: &Composer, engine: &Engine) -> Result<(), String> {
    if composer.is_playing(engine) {
        return Err(String::from("the song is playing, and nothing changes while it plays"));
    }
    Ok(())
}

fn try_offer(offer: KeyOffer, key_map: &mut KeyMap, composer: &Composer, engine: &Engine) -> Result<(), String> {
    refuse_while_playing(composer, engine)?;
    key_map.set_trying(Some(offer.after));
    follow_picked_track(key_map, composer);
    Ok(())
}

fn use_offer(offer: KeyOffer, key_map: &mut KeyMap, composer: &mut Composer, engine: &Engine) -> Result<(), String> {
    refuse_while_playing(composer, engine)?;
    let now_keys = mapping_of(composer, offer.section)?;
    if now_keys != offer.before {
        return Err(String::from("these keys changed since then: ask the agent to optimize again"));
    }
    keep_mapping(composer, offer.section, offer.after)?;
    key_map.set_trying(None);
    follow_picked_track(key_map, composer);
    Ok(())
}

// Reads the Play tab's last scale and the player's timing as Jello starts. The
// page opens on Play.
pub fn start_keys(app: &AppHandle) {
    let open = app.state::<Mutex<OpenSong>>();
    let Ok(folders) = open.lock().unwrap().folders() else {
        return;
    };
    let mut scale = None;
    if let Some(name) = setting(&folders.settings, PLAY_SCALE) {
        scale = Scale::of_name(&name);
    }
    let key_map = app.state::<Mutex<KeyMap>>();
    let mut key_map = key_map.lock().unwrap();
    key_map.set_play_scale(scale);
    key_map.set_on_play_tab(true);
    let stats = app.state::<Mutex<KeyStats>>();
    stats.lock().unwrap().load(&folders.settings);
}

// Saves the player's timing a moment after it changes.
pub fn save_key_stats(app: &AppHandle, open: &OpenSong) {
    let Ok(folders) = open.folders() else {
        return;
    };
    let stats = app.state::<Mutex<KeyStats>>();
    stats.lock().unwrap().save_if_changed(&folders.settings);
}

// The page moved between tabs: Play's keys follow its last scale, and every
// other tab's the song's.
#[tauri::command]
pub fn keys_tab_changed(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    on_play: bool,
) -> LayoutForUi {
    let mut key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    key_map.set_on_play_tab(on_play);
    follow_picked_track(&mut key_map, &composer);
    key_map.layout_for_ui()
}

fn scale_of(tonic: u8, kind: &str) -> Result<Scale, String> {
    if tonic > 11 {
        return Err(format!("no scale has home note {tonic}: it goes from 0 (C) to 11 (B)"));
    }
    let Some(kind) = ScaleKind::of_word(kind) else {
        return Err(format!("{kind} is not a kind of scale"));
    };
    Ok(Scale { tonic, kind })
}

// Picks the scale the Play tab opens on from now on.
#[tauri::command]
pub fn pick_play_scale(
    open: State<'_, Mutex<OpenSong>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    tonic: u8,
    kind: String,
) -> Result<LayoutForUi, String> {
    let scale = scale_of(tonic, &kind)?;
    let open = open.lock().unwrap();
    let mut key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    key_map.set_play_scale(Some(scale));
    follow_picked_track(&mut key_map, &composer);
    if let Ok(folders) = open.folders() {
        let _ = set_setting(&folders.settings, PLAY_SCALE, Some(&scale.name()));
    }
    Ok(key_map.layout_for_ui())
}

// Picks the song's scale, in Compose.
#[tauri::command]
pub fn pick_song_scale(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    tonic: u8,
    kind: String,
) -> Result<ComposeView, String> {
    let scale = scale_of(tonic, &kind)?;
    let now = Instant::now();
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    composer.pick_scale(scale);
    follow_picked_track(&mut key_map, &composer);
    Ok(composer.view(&key_map, engine.song_place(), now))
}

// What the mapping page shows.
#[derive(Serialize)]
pub struct MappingView {
    scale: Option<String>,
    // The section being edited, from 0, or None for the main mapping.
    section: Option<usize>,
    sections: Vec<SectionKeys>,
    keys: std::collections::BTreeMap<String, Vec<u8>>,
    names: std::collections::BTreeMap<String, String>,
    locked: Vec<String>,
    mappable: Vec<&'static str>,
    // What the same keys play on a guitar's track, its strings thickest first.
    guitar: GuitarMapping,
    guitar_strings: Vec<&'static str>,
    guitar_shape_keys: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct SectionKeys {
    name: String,
    own: bool,
}

fn view_of(composer: &Composer, section: Option<usize>) -> Result<MappingView, String> {
    let keys = mapping_of(composer, section)?;
    let mut sections = Vec::new();
    for found in composer.sections() {
        sections.push(SectionKeys {
            name: found.name.clone(),
            own: found.keys.is_some(),
        });
    }
    let mut names = std::collections::BTreeMap::new();
    for (key, notes) in &keys.keys {
        names.insert(key.clone(), name_of_notes(notes));
    }
    let mut scale = None;
    if let Some(picked) = composer.scale() {
        scale = Some(picked.name());
    }
    Ok(MappingView {
        scale,
        section,
        sections,
        keys: keys.keys,
        names,
        locked: keys.locked.into_iter().collect(),
        mappable: mappable_keys(),
        guitar: keys.guitar,
        guitar_strings: STRING_KEYS.to_vec(),
        guitar_shape_keys: shape_keys(),
    })
}

#[tauri::command]
pub fn mapping_view(composer: State<'_, Mutex<Composer>>, section: Option<usize>) -> Result<MappingView, String> {
    let composer = composer.lock().unwrap();
    view_of(&composer, section)
}

// Puts `notes` on `key` on the mapping page. A section's first edit starts from
// a copy of the main mapping, as every section starts on it.
#[tauri::command]
pub fn set_key_notes(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    section: Option<usize>,
    key: String,
    notes: Vec<u8>,
) -> Result<MappingView, String> {
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut keys = mapping_of(&composer, section)?;
    keys.set_key(&key, &notes)?;
    keep_mapping(&mut composer, section, keys)?;
    follow_picked_track(&mut key_map, &composer);
    view_of(&composer, section)
}

// Puts a guitar shape on `key` on the mapping page, or with no key tunes the
// open strings. Like a key's notes, a section's first edit starts from a copy of
// the main mapping.
#[tauri::command]
pub fn set_guitar_strings(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    section: Option<usize>,
    key: Option<String>,
    name: String,
    strings: [Option<u8>; STRING_COUNT],
) -> Result<MappingView, String> {
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut keys = mapping_of(&composer, section)?;
    match key {
        Some(key) => keys.guitar.set_shape(&key, &name, strings)?,
        None => keys.guitar.set_open(strings)?,
    }
    keep_mapping(&mut composer, section, keys)?;
    follow_picked_track(&mut key_map, &composer);
    view_of(&composer, section)
}

// Sets how long each string of a strum waits after the one before, on the
// mapping page.
#[tauri::command]
pub fn set_strum_ms(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    section: Option<usize>,
    ms: u32,
) -> Result<MappingView, String> {
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut keys = mapping_of(&composer, section)?;
    keys.guitar.set_strum_ms(ms)?;
    keep_mapping(&mut composer, section, keys)?;
    follow_picked_track(&mut key_map, &composer);
    view_of(&composer, section)
}

#[tauri::command]
pub fn set_key_locked(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    section: Option<usize>,
    key: String,
    locked: bool,
) -> Result<MappingView, String> {
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut keys = mapping_of(&composer, section)?;
    keys.set_locked(&key, locked)?;
    keep_mapping(&mut composer, section, keys)?;
    follow_picked_track(&mut key_map, &composer);
    view_of(&composer, section)
}

// Puts unlocked keys back on the scale's default, or a section back on the main
// mapping.
#[tauri::command]
pub fn reset_keys(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    section: Option<usize>,
) -> Result<MappingView, String> {
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    match section {
        Some(number) => composer.sections_mut().set_keys(number, None)?,
        None => {
            let Some(scale) = composer.scale() else {
                return Err(String::from("pick a scale first"));
            };
            let keys = composer.main_keys().with_default_for(scale);
            composer.set_main_keys(keys);
        }
    }
    follow_picked_track(&mut key_map, &composer);
    view_of(&composer, section)
}

// A note on the drawn piano sounds as it is clicked, until it is let go.
#[tauri::command]
pub fn sound_mapped_note(engine: State<'_, Mutex<Engine>>, note: u8, on: bool) -> Result<(), String> {
    if note > 127 {
        return Err(format!("{note} is not a note"));
    }
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    if on {
        engine.note_on(note, LAPTOP_VELOCITY);
    } else {
        engine.note_off(note);
    }
    Ok(())
}

// A key to light for what comes next. The first group is "next"; the ones after
// it "later", numbered.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ComingMark {
    key: String,
    label: String,
    state: &'static str,
}

// While the loop plays, the keys for the picked track's next presses, chords and
// melody alike (docs/design.md#song-key-mappings). A drums track, or a loop not
// playing, lights nothing.
#[tauri::command]
pub fn coming_keys(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> Vec<ComingMark> {
    let key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();

    let state = engine.looper_status().state;
    if state != LooperState::Looping && state != LooperState::Layering {
        return Vec::new();
    }
    if composer.tracks().picked_track().is_drums() {
        return Vec::new();
    }
    let track = composer.tracks().picked();
    let tempo = engine.tempo();
    let rate = engine.sample_rate();
    let beat = beats_of(engine.looper_position(), tempo, rate);
    let Some(looped) = loop_now(&mut engine) else {
        return Vec::new();
    };
    let mut layers: Vec<(&SongLayer, f64)> = Vec::new();
    for layer in &looped.layers {
        if layer.track != track || !layer.playing {
            continue;
        }
        let mut bars = layer.bars;
        if bars == 0 {
            bars = looped.bars;
        }
        layers.push((layer, f64::from(bars * BEATS_PER_BAR)));
    }
    marks_ahead(&layers, beat, &key_map)
}

// The marks for the next MARKS_AHEAD presses after `beat`. Each layer repeats
// every so many beats.
fn marks_ahead(layers: &[(&SongLayer, f64)], beat: f64, key_map: &KeyMap) -> Vec<ComingMark> {
    let mut coming: Vec<(f64, Vec<u8>)> = Vec::new();
    for (layer, length) in layers {
        if *length <= 0.0 {
            continue;
        }
        let into = beat.rem_euclid(*length);
        for note in &layer.notes {
            if note.note > 127 {
                continue;
            }
            let mut wait = note.start - into;
            if wait <= 0.0 {
                wait += *length;
            }
            coming.push((wait, vec![note.note]));
        }
    }
    coming.sort_by(|one, other| one.0.total_cmp(&other.0));

    // Notes starting together are one press.
    let mut groups: Vec<(f64, Vec<u8>)> = Vec::new();
    for (wait, notes) in coming {
        match groups.last_mut() {
            Some(last) if wait - last.0 <= 0.02 => last.1.extend(notes),
            _ => groups.push((wait, notes)),
        }
    }

    let mut marks: Vec<ComingMark> = Vec::new();
    for (place, (_, notes)) in groups.iter().take(MARKS_AHEAD).enumerate() {
        let mut sorted = notes.clone();
        sorted.sort();
        sorted.dedup();
        let state = if place == 0 { "next" } else { "later" };
        let mut keys = key_map.keys_playing(&sorted);
        keys.truncate(1);
        if keys.is_empty() {
            for note in &sorted {
                keys.extend(key_map.keys_playing(&[*note]).into_iter().take(1));
            }
        }
        for key in keys {
            let marked = marks.iter().any(|mark| mark.key == key);
            if !marked {
                marks.push(ComingMark {
                    key,
                    label: (place + 1).to_string(),
                    state,
                });
            }
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_file::BeatNote;

    #[test]
    fn an_offer_is_refused_while_the_song_plays_and_used_once_it_stops() {
        let scale = Scale {
            tonic: 0,
            kind: ScaleKind::Major,
        };
        let mut composer = Composer::new();
        composer.pick_scale(scale);
        let before = composer.main_keys().clone();
        let mut after = before.clone();
        after.set_key("KeyF", &[61]).unwrap();
        let offer = KeyOffer {
            number: 1,
            section: None,
            before,
            after: after.clone(),
        };
        let now = Instant::now();
        let chord = crate::harmony::chord_of_name("C").unwrap();
        composer.add_chord(chord, 500, now);
        let mut engine = Engine::new().unwrap();
        composer.play_song(&mut engine, now).unwrap();
        let mut key_map = c_major_keys();

        let tried = try_offer(offer.clone(), &mut key_map, &composer, &engine);
        assert!(tried.unwrap_err().contains("playing"));
        assert_ne!(key_map.notes_of("KeyF"), vec![61]);
        let used = use_offer(offer.clone(), &mut key_map, &mut composer, &engine);
        assert!(used.unwrap_err().contains("playing"));
        assert_ne!(*composer.main_keys(), after);

        engine.stop_song();
        use_offer(offer, &mut key_map, &mut composer, &engine).unwrap();
        assert_eq!(*composer.main_keys(), after);
    }

    fn c_major_keys() -> KeyMap {
        let mut key_map = KeyMap::new();
        key_map.follow(&KeyMapping::default_for(Scale {
            tonic: 0,
            kind: ScaleKind::Major,
        }));
        key_map
    }

    fn layer_of(notes: &[(u8, f64)]) -> SongLayer {
        let mut beat_notes = Vec::new();
        for (note, start) in notes {
            beat_notes.push(BeatNote {
                note: *note,
                start: *start,
                end: start + 0.25,
            });
        }
        SongLayer {
            track: 0,
            bars: 0,
            volume: 100,
            playing: true,
            notes: beat_notes,
        }
    }

    #[test]
    fn the_next_melody_notes_light_in_order_and_wrap_around_the_loop() {
        let key_map = c_major_keys();
        let layer = layer_of(&[(60, 0.0), (62, 1.0), (64, 2.0), (67, 3.0)]);
        let marks = marks_ahead(&[(&layer, 4.0)], 0.5, &key_map);
        assert_eq!(marks.len(), 3);
        assert_eq!(marks[0].key, "KeyK");
        assert_eq!(marks[0].state, "next");
        assert_eq!(marks[1].key, "KeyL");
        assert_eq!(marks[1].state, "later");
        assert_eq!(marks[1].label, "2");

        // Near the end, the first note comes round again.
        let marks = marks_ahead(&[(&layer, 4.0)], 3.5, &key_map);
        assert_eq!(marks[0].key, "KeyJ");
    }

    #[test]
    fn notes_starting_together_light_the_key_that_plays_them_all() {
        let key_map = c_major_keys();
        let layer = layer_of(&[(48, 1.0), (52, 1.0), (55, 1.0), (60, 2.0)]);
        let marks = marks_ahead(&[(&layer, 4.0)], 0.0, &key_map);
        assert_eq!(marks[0].key, "KeyF");
        assert_eq!(marks[1].key, "KeyJ");
    }

    #[test]
    fn a_note_with_no_key_lights_nothing_and_a_key_note_is_not_a_press() {
        let key_map = c_major_keys();
        let layer = layer_of(&[(61, 1.0), (crate::engine::VIBRATO_KEY_NOTE, 1.5)]);
        assert!(marks_ahead(&[(&layer, 4.0)], 0.0, &key_map).is_empty());
    }

    #[test]
    fn moves_list_only_the_keys_that_changed() {
        let mut before = KeyMapping::default();
        before.set_key("KeyF", &[48, 52, 55]).unwrap();
        before.set_key("KeyJ", &[60]).unwrap();
        let mut after = before.clone();
        after.set_key("KeyJ", &[62]).unwrap();
        after.set_key("KeyK", &[64]).unwrap();
        let moves = moves_between(&before, &after);
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0]["key"], "KeyJ");
        assert_eq!(moves[0]["was"], "C4");
        assert_eq!(moves[0]["now"], "D4");
        assert_eq!(moves[1]["was"], "nothing");
    }

    #[test]
    fn an_offer_too_old_is_refused() {
        let mut offers = KeyOffers::default();
        let first = offers.keep(None, KeyMapping::default(), KeyMapping::default());
        for _ in 0..KEPT_OFFERS {
            offers.keep(None, KeyMapping::default(), KeyMapping::default());
        }
        assert!(offers.get(first).is_err());
        assert!(offers.get(first + 1).is_ok());
    }
}
