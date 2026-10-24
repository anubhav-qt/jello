// The agent's cards and options (docs/design.md#showing-instead-of-telling).
// A reply is mostly Jello's own screens: jello_show puts a card in it, and
// jello_options offers 2 to 4 choices, each with Play and Use. Jello checks
// every field and refuses a wrong one, like any value, so the panel only
// ever draws cards that make sense.
//
// The card goes to the panel beside the tool's answer (ToolAnswer.card);
// the model is told only that it was shown.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::compose::Composer;
use crate::engine::Engine;
use crate::harmony::{Chord, Key, Quality, chord_name, chord_of_name, guide, key_name, key_of_name};
use crate::key_map::KeyMap;
use crate::layer_notes::{self, WrittenNote};
use crate::phrases::{self, Phrases};
use crate::practice::{self, Practice};
use crate::song_file::{note_name, note_of_name};
use crate::song_session::OpenSong;
use crate::sound_changes;
use crate::sound_file::Sound;
use crate::tool_server;
use crate::tools::{Studio, ToolAnswer};
use crate::undo::{self, Replies};

// The kinds of card jello_show draws.
pub const CARD_KINDS: [&str; 6] = ["keys", "lane", "progression", "compare", "chart", "sound"];

// The kinds of choice jello_options offers.
pub const OPTION_KINDS: [&str; 3] = ["chords", "melody", "sound"];

// Limits that keep a card readable in the panel.
const MOST_TITLE_CHARACTERS: usize = 80;
const MOST_NAME_CHARACTERS: usize = 40;
const MOST_MARKED_KEYS: usize = 48;
const MOST_LANE_PARTS: usize = 4;
const MOST_LANE_NOTES: usize = 512;
const MOST_LANE_BEATS: f64 = 256.0;
const MOST_PROGRESSION_CHORDS: usize = 32;
const MOST_COMPARED: usize = 4;
const MOST_MEASURES: usize = 8;
const MOST_CHART_POINTS: usize = 64;
const MOST_OPTIONS: usize = 4;
const MOST_OPTION_CHORDS: usize = 16;
const MOST_CHORD_BEATS: f64 = 16.0;

// How long each chord of a chord option is held when it doesn't say.
const CHORD_BEATS: f64 = 2.0;

// How many offers are kept, so Play and Use still work on recent ones.
const KEPT_OFFERS: usize = 20;

// The numbers for a chord's root counted in half steps from the key's home
// note, as roman numerals.
const NUMERALS: [&str; 12] = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"];

// jello_show: checks the card in `arguments` and hands it to the panel. A
// keys card also marks its keys on the real keyboard.
pub fn show(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let Some(card) = arguments.get("card").and_then(Value::as_object) else {
        return Err(String::from("jello_show needs a card, like {\"card\": {\"kind\": \"progression\", \"chords\": [\"Am\", \"F\"]}}"));
    };
    let kind = text_field(card, "kind", "the card")?;
    let mut checked = match kind.as_str() {
        "keys" => keys_card(studio, card)?,
        "lane" => lane_card(studio, card)?,
        "progression" => progression_card(studio, card)?,
        "compare" => compare_card(card)?,
        "chart" => chart_card(card)?,
        "sound" => sound_card(studio, card)?,
        _ => {
            let all = CARD_KINDS.join(", ");
            return Err(format!("there is no card called {kind}: there are {all}"));
        }
    };
    checked["kind"] = json!(kind);
    checked["title"] = json!(title_of(card)?);
    if kind == "keys" {
        studio.screen.show_marks(&checked["keys"]);
    }
    let answer = json!({"shown": kind});
    Ok(ToolAnswer::of(answer, Vec::new()).with_card(checked))
}

// The card's title, or "" without one.
fn title_of(card: &Map<String, Value>) -> Result<String, String> {
    let title = optional_text_field(card, "title", "the card")?.unwrap_or_default();
    if title.chars().count() > MOST_TITLE_CHARACTERS {
        return Err(format!("a title is at most {MOST_TITLE_CHARACTERS} characters"));
    }
    Ok(title)
}

// A keys card: keys marked on the real keyboard, each by its laptop key, a
// note or a chord at the current octave, with a short label (its number
// with none).
fn keys_card(studio: &Studio, card: &Map<String, Value>) -> Result<Value, String> {
    let listed = list_field(card, "keys", "a keys card", 1, MOST_MARKED_KEYS)?;
    let key_map = studio.key_map.lock().unwrap();
    let mut keys = Vec::new();
    for (index, listed_key) in listed.iter().enumerate() {
        let place = format!("key {}", index + 1);
        let Some(fields) = listed_key.as_object() else {
            return Err(format!("{place} is an object, like {{\"note\": \"E4\", \"label\": \"1\"}}"));
        };
        let (key, what) = phrases::key_of_press(fields, &key_map, &place)?;
        let mut label = (index + 1).to_string();
        if let Some(asked) = optional_text_field(fields, "label", &place)? {
            if asked.chars().count() > 4 {
                return Err(format!("{place}'s label is at most 4 characters, like \"1\" or \"R\""));
            }
            label = asked;
        }
        keys.push(json!({"key": key, "what": what, "label": label}));
    }
    Ok(json!({"keys": keys}))
}

// A lane card: notes over beats, in 1 to 4 parts, like two hands. Any
// pitch, not only what the keys reach.
fn lane_card(studio: &Studio, card: &Map<String, Value>) -> Result<Value, String> {
    let listed = list_field(card, "parts", "a lane card", 1, MOST_LANE_PARTS)?;
    let mut parts = Vec::new();
    let mut count = 0;
    let mut last_end: f64 = 0.0;
    for (index, listed_part) in listed.iter().enumerate() {
        let place = format!("part {}", index + 1);
        let Some(fields) = listed_part.as_object() else {
            return Err(format!("{place} is an object, like {{\"name\": \"Right hand\", \"notes\": [...]}}"));
        };
        let name = name_field(fields, &place)?;
        let notes = list_field(fields, "notes", &place, 1, MOST_LANE_NOTES)?;
        let mut drawn = Vec::new();
        for (note_index, listed_note) in notes.iter().enumerate() {
            let note_place = format!("{place}, note {}", note_index + 1);
            let Some(note_fields) = listed_note.as_object() else {
                return Err(format!("{note_place} is an object, like {{\"note\": \"E4\", \"start\": 0, \"hold\": 1}}"));
            };
            let start = number_field(note_fields, "start", &note_place)?;
            let hold = number_field(note_fields, "hold", &note_place)?;
            if start < 0.0 || hold <= 0.0 {
                return Err(format!("{note_place} starts at 0 or later and holds for more than 0 beats"));
            }
            let end = start + hold;
            if end > MOST_LANE_BEATS {
                return Err(format!("{note_place} ends after beat {MOST_LANE_BEATS}, the longest a lane is"));
            }
            last_end = last_end.max(end);
            let (label, pitches) = pitches_named(note_fields, &note_place)?;
            drawn.push(json!({"label": label, "pitches": pitches, "start": start, "hold": hold}));
            count += 1;
        }
        parts.push(json!({"name": name, "notes": drawn}));
    }
    if count > MOST_LANE_NOTES {
        return Err(format!("a lane holds at most {MOST_LANE_NOTES} notes, not {count}"));
    }
    let beats = (last_end / 4.0).ceil() * 4.0;
    // The tempo, so the playhead can follow jello_play in beats.
    let tempo = studio.composer.lock().unwrap().tempo();
    Ok(json!({"beats": beats, "parts": parts, "tempo": tempo}))
}

// A note's or a chord's name and its pitches, as a lane draws them.
fn pitches_named(fields: &Map<String, Value>, place: &str) -> Result<(String, Vec<u8>), String> {
    if let Some(name) = fields.get("note") {
        let name = name.as_str().unwrap_or_default();
        let Some(note) = note_of_name(name) else {
            return Err(format!("{place}: {name} is not a note, like \"C4\" or \"F#3\""));
        };
        return Ok((note_name(note), vec![note]));
    }
    if let Some(name) = fields.get("chord") {
        let name = name.as_str().unwrap_or_default();
        let Some(chord) = chord_of_name(name) else {
            return Err(format!("{place}: {name} is not a chord, like \"Am\" or \"Bb\""));
        };
        return Ok((chord_name(chord, None), chord.notes().to_vec()));
    }
    Err(format!("{place} names a note or a chord"))
}

// A progression card: chords with their numbers in the key, and how each
// fits it. The key is the one given, or the song's.
fn progression_card(studio: &Studio, card: &Map<String, Value>) -> Result<Value, String> {
    let key = match optional_text_field(card, "key", "a progression card")? {
        Some(name) => match key_of_name(&name) {
            Some(key) => key,
            None => return Err(format!("{name} is not a key, like \"A minor\" or \"Db major\"")),
        },
        None => {
            let composer = studio.composer.lock().unwrap();
            match composer.key() {
                Some(key) => key,
                None => return Err(String::from("the song has no key yet, so give the card a key")),
            }
        }
    };
    let listed = list_field(card, "chords", "a progression card", 1, MOST_PROGRESSION_CHORDS)?;
    let mut chords = Vec::new();
    for (index, listed_chord) in listed.iter().enumerate() {
        let name = listed_chord.as_str().unwrap_or_default();
        let Some(chord) = chord_of_name(name) else {
            return Err(format!("chord {}: \"{name}\" is not a chord, like \"Am\" or \"Bb\"", index + 1));
        };
        let guided = guide(key, chord);
        chords.push(json!({
            "name": guided.name,
            "numeral": numeral_of(key, chord),
            "fit": guided.fit,
        }));
    }
    Ok(json!({"key": key_name(key), "chords": chords}))
}

// `chord`'s roman numeral in `key`, counted from the key's home note:
// capitals for a major chord and small letters for a minor one, like "IV"
// or "vi". A root outside the key gets a flat or a sharp, like "bVII".
pub fn numeral_of(key: Key, chord: Chord) -> String {
    let steps = (chord.pitch() + 12 - key.tonic) % 12;
    let numeral = NUMERALS[steps as usize];
    match chord.quality {
        Quality::Major => String::from(numeral),
        Quality::Minor => numeral.to_lowercase(),
    }
}

// A compare card: 2 to 4 takes or options side by side, each with up to 8
// measures, each a name and a value.
fn compare_card(card: &Map<String, Value>) -> Result<Value, String> {
    let listed = list_field(card, "items", "a compare card", 2, MOST_COMPARED)?;
    let mut items = Vec::new();
    for (index, listed_item) in listed.iter().enumerate() {
        let place = format!("item {}", index + 1);
        let Some(fields) = listed_item.as_object() else {
            return Err(format!("{place} is an object, like {{\"name\": \"Take 1\", \"measures\": [...]}}"));
        };
        let name = name_field(fields, &place)?;
        let measures = list_field(fields, "measures", &place, 1, MOST_MEASURES)?;
        let mut checked = Vec::new();
        for (measure_index, measure) in measures.iter().enumerate() {
            let measure_place = format!("{place}, measure {}", measure_index + 1);
            let Some(measure_fields) = measure.as_object() else {
                return Err(format!("{measure_place} is an object, like {{\"name\": \"Late\", \"value\": \"40 ms\"}}"));
            };
            let measure_name = name_field(measure_fields, &measure_place)?;
            let value = short_value(measure_fields.get("value"), &measure_place)?;
            checked.push(json!({"name": measure_name, "value": value}));
        }
        items.push(json!({"name": name, "measures": checked}));
    }
    Ok(json!({"items": items}))
}

// A measure's value as text: a number or short text.
fn short_value(value: Option<&Value>, place: &str) -> Result<String, String> {
    if let Some(number) = value.and_then(Value::as_f64) {
        return Ok(format!("{}", (number * 100.0).round() / 100.0));
    }
    if let Some(text) = value.and_then(Value::as_str) {
        if text.chars().count() <= MOST_NAME_CHARACTERS {
            return Ok(String::from(text));
        }
    }
    Err(format!("{place}'s value is a number or at most {MOST_NAME_CHARACTERS} characters"))
}

// A chart card: a skill or measure over time, 2 to 64 points, each a short
// label and a number, with a unit like "ms".
fn chart_card(card: &Map<String, Value>) -> Result<Value, String> {
    let unit = optional_text_field(card, "unit", "a chart card")?.unwrap_or_default();
    if unit.chars().count() > 12 {
        return Err(String::from("a chart's unit is at most 12 characters, like \"ms\""));
    }
    let listed = list_field(card, "points", "a chart card", 2, MOST_CHART_POINTS)?;
    let mut points = Vec::new();
    for (index, point) in listed.iter().enumerate() {
        let place = format!("point {}", index + 1);
        let Some(fields) = point.as_object() else {
            return Err(format!("{place} is an object, like {{\"label\": \"Mon\", \"value\": 40}}"));
        };
        let label = name_field_called(fields, "label", &place, 20)?;
        let value = number_field(fields, "value", &place)?;
        points.push(json!({"label": label, "value": value}));
    }
    Ok(json!({"unit": unit, "points": points}))
}

// A sound card: the song's sound as drawn, or with `changes`, the sound
// those changes would make drawn over the song's, dashed.
fn sound_card(studio: &Studio, card: &Map<String, Value>) -> Result<Value, String> {
    let composer = studio.composer.lock().unwrap();
    let sound = composer.sound_profile().sound();
    drop(composer);
    let Some(changes) = card.get("changes") else {
        return Ok(json!({"after": sketch_of(&sound), "before": Value::Null}));
    };
    let changed = sound_changes::changed(&sound, changes)?;
    Ok(json!({
        "after": sketch_of(&changed),
        "before": sketch_of(&sound),
        "lines": sound_changes::change_lines(&sound, &changed),
    }))
}

// What a sound's drawing needs: its loudness envelope, its filter and its
// oscillators.
pub fn sketch_of(sound: &Sound) -> Value {
    let values = sound_changes::json_of(sound);
    let mut oscillators = Vec::new();
    if let Some(listed) = values["oscillators"].as_array() {
        for oscillator in listed {
            oscillators.push(json!({"kind": oscillator["kind"], "level": oscillator["level"]}));
        }
    }
    json!({
        "loudness": values["loudness"],
        "filter": values["filter"],
        "oscillators": oscillators,
    })
}

// The card a sound change draws itself with: the sound before, dashed,
// under the sound after.
pub fn sound_change_card(before: &Sound, after: &Sound, lines: &[String]) -> Value {
    json!({
        "kind": "sound",
        "title": "",
        "after": sketch_of(after),
        "before": sketch_of(before),
        "lines": lines,
    })
}

// The most presses a presses strip shows: the newest.
const MOST_STRIP_PRESSES: usize = 32;

// The strip jello_look draws of the player's recent presses (Compose's
// performance): each press's name, when it went down from the first one
// shown, and how long it was held, or null while it still is.
pub fn presses_card(performance: &Value, key: Option<Key>) -> Value {
    let mut presses = Vec::new();
    if let Some(listed) = performance.as_array() {
        let mut first = 0;
        if listed.len() > MOST_STRIP_PRESSES {
            first = listed.len() - MOST_STRIP_PRESSES;
        }
        let shown = &listed[first..];
        let mut start_ms = 0;
        if let Some(earliest) = shown.first() {
            start_ms = earliest["pressed_ms"].as_u64().unwrap_or(0);
        }
        for press in shown {
            let mut notes = Vec::new();
            if let Some(listed_notes) = press["notes"].as_array() {
                for note in listed_notes {
                    notes.push(note.as_u64().unwrap_or(0) as u8);
                }
            }
            let pressed_ms = press["pressed_ms"].as_u64().unwrap_or(0);
            presses.push(json!({
                "label": press_label(&notes, key),
                "pitch": notes.first(),
                "at_ms": pressed_ms.saturating_sub(start_ms),
                "held_ms": press["held_ms"],
            }));
        }
    }
    json!({"kind": "presses", "title": "", "presses": presses})
}

// A press's name: a note's, or a chord key's chord, or its notes.
fn press_label(notes: &[u8], key: Option<Key>) -> String {
    if notes.len() == 1 {
        return note_name(notes[0]);
    }
    if notes.len() == 3 && notes[2] == notes[0] + 7 {
        let third = notes[1] - notes[0];
        let mut quality = None;
        if third == 4 {
            quality = Some(Quality::Major);
        } else if third == 3 {
            quality = Some(Quality::Minor);
        }
        if let Some(quality) = quality {
            let chord = Chord { root: notes[0], quality };
            return chord_name(chord, key);
        }
    }
    let mut names = Vec::new();
    for note in notes {
        names.push(note_name(*note));
    }
    names.join(" ")
}

// The meters jello_listen draws: the whole render's measures, the loudness
// beat by beat, and the silences.
pub fn listen_card(measured: &Value) -> Value {
    let mut loudness = Vec::new();
    if let Some(beats) = measured["beats"].as_array() {
        for beat in beats {
            loudness.push(beat["loudness_db"].clone());
        }
    }
    json!({
        "kind": "listen",
        "title": "",
        "rendered": measured["rendered"],
        "seconds": measured["seconds"],
        "whole": measured["whole"],
        "loudness": loudness,
        "silences": measured["silences"],
    })
}

// The offers the agent made, newest last, so the panel's Play and Use can
// reach them by number, and the choices the player used since the agent
// was last told.
#[derive(Default)]
pub struct Offers {
    last: u64,
    kept: VecDeque<Offer>,
    used: Vec<String>,
}

// One offer: its number, kind, and its choices as checked.
#[derive(Clone, Debug)]
struct Offer {
    number: u64,
    kind: String,
    choices: Vec<Choice>,
}

// One choice: its name, a phrase that plays it, and what Use puts in the
// song.
#[derive(Clone, Debug)]
struct Choice {
    name: String,
    // The phrase Play plays, as jello_play takes it, in beats.
    phrase: Value,
    // A chord option's chords and their holds in beats.
    chords: Vec<(Chord, f64)>,
    // A melody option's notes, for a new loop layer.
    notes: Vec<WrittenNote>,
    // A sound option's changes.
    changes: Value,
}

impl Offers {
    fn keep(&mut self, kind: &str, choices: Vec<Choice>) -> u64 {
        self.last += 1;
        self.kept.push_back(Offer {
            number: self.last,
            kind: String::from(kind),
            choices,
        });
        if self.kept.len() > KEPT_OFFERS {
            self.kept.pop_front();
        }
        self.last
    }

    // True while offer `offer` is kept.
    #[cfg(test)]
    pub fn has(&self, offer: u64) -> bool {
        self.kept.iter().any(|kept| kept.number == offer)
    }

    // What the player used since the agent was last told, for the next
    // message: like "the player used option 2, Darker, of offer 3".
    pub fn take_used(&mut self) -> Vec<String> {
        std::mem::take(&mut self.used)
    }

    fn choice(&self, offer: u64, choice: usize) -> Result<(String, Choice), String> {
        let Some(kept) = self.kept.iter().find(|kept| kept.number == offer) else {
            return Err(String::from("that offer is too old to use: ask the agent again"));
        };
        let Some(picked) = kept.choices.get(choice) else {
            return Err(format!("the offer has no choice {}", choice + 1));
        };
        Ok((kept.kind.clone(), picked.clone()))
    }
}

// jello_options: checks 2 to 4 choices of chords, a melody or a sound, and
// hands them to the panel, where each gets Play and Use.
pub fn offer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let Some(fields) = arguments.as_object() else {
        return Err(String::from("jello_options takes an object"));
    };
    let kind = text_field(fields, "kind", "jello_options")?;
    if !OPTION_KINDS.contains(&kind.as_str()) {
        let all = OPTION_KINDS.join(", ");
        return Err(format!("there are no {kind} options: there are {all}"));
    }
    let title = title_of(fields)?;
    let listed = list_field(fields, "options", "jello_options", 2, MOST_OPTIONS)?;

    let key_map = studio.key_map.lock().unwrap();
    let composer = studio.composer.lock().unwrap();
    let sound = composer.sound_profile().sound();
    let song_key = composer.key();
    let tempo = composer.tempo();
    let mut choices = Vec::new();
    let mut shown = Vec::new();
    for (index, listed_option) in listed.iter().enumerate() {
        let place = format!("option {}", index + 1);
        let Some(option) = listed_option.as_object() else {
            return Err(format!("{place} is an object, like {{\"name\": \"Darker\", ...}}"));
        };
        let name = name_field(option, &place)?;
        let (mut choice, said) = match kind.as_str() {
            "chords" => chord_choice(option, &place, song_key)?,
            "melody" => melody_choice(option, &place, &key_map)?,
            _ => sound_choice(option, &place, &sound)?,
        };
        choice.name = name.clone();
        // Every phrase must play on the keys at this octave.
        phrases::phrase_of(&choice.phrase, &key_map, tempo)?;
        let mut shown_option = said;
        shown_option["name"] = json!(name);
        shown.push(shown_option);
        choices.push(choice);
    }
    drop(composer);
    drop(key_map);

    let number = studio.offers.lock().unwrap().keep(&kind, choices);
    let card = json!({
        "kind": "options",
        "title": title,
        "offer": number,
        "choice": kind,
        "options": shown,
    });
    let answer = json!({"offered": shown.len(), "offer": number});
    Ok(ToolAnswer::of(answer, Vec::new()).with_card(card))
}

// A chord option: 1 to 16 chords, each a name or {chord, hold} in beats.
fn chord_choice(option: &Map<String, Value>, place: &str, key: Option<Key>) -> Result<(Choice, Value), String> {
    let listed = list_field(option, "chords", place, 1, MOST_OPTION_CHORDS)?;
    let mut chords = Vec::new();
    let mut presses = Vec::new();
    let mut said = Vec::new();
    let mut start = 0.0;
    for (index, listed_chord) in listed.iter().enumerate() {
        let chord_place = format!("{place}, chord {}", index + 1);
        let (name, hold) = match listed_chord {
            Value::String(name) => (name.clone(), CHORD_BEATS),
            Value::Object(fields) => {
                let name = text_field(fields, "chord", &chord_place)?;
                let mut hold = CHORD_BEATS;
                if fields.contains_key("hold") {
                    hold = number_field(fields, "hold", &chord_place)?;
                }
                (name, hold)
            }
            _ => return Err(format!("{chord_place} is a chord's name, or {{\"chord\": \"Am\", \"hold\": 2}}")),
        };
        if hold <= 0.0 || hold > MOST_CHORD_BEATS {
            return Err(format!("{chord_place} holds for more than 0 and at most {MOST_CHORD_BEATS} beats"));
        }
        let Some(chord) = chord_of_name(&name) else {
            return Err(format!("{chord_place}: {name} is not a chord, like \"Am\" or \"Bb\""));
        };
        presses.push(json!({"chord": name, "start": start, "hold": hold}));
        let mut shown = json!({"name": chord_name(chord, key), "hold": hold});
        if let Some(key) = key {
            shown["numeral"] = json!(numeral_of(key, chord));
        }
        said.push(shown);
        chords.push((chord, hold));
        start += hold;
    }
    let choice = Choice {
        name: String::new(),
        phrase: json!({"unit": "beats", "presses": presses}),
        chords,
        notes: Vec::new(),
        changes: Value::Null,
    };
    Ok((choice, json!({"chords": said})))
}

// A melody option: presses as jello_play takes them, in beats, naming
// notes or chords. Use writes it as a new loop layer.
fn melody_choice(option: &Map<String, Value>, place: &str, key_map: &KeyMap) -> Result<(Choice, Value), String> {
    let listed = list_field(option, "presses", place, 1, layer_notes::LAYER_MOST_NOTES)?;
    let mut said = Vec::new();
    for (index, press) in listed.iter().enumerate() {
        let press_place = format!("{place}, press {}", index + 1);
        let Some(fields) = press.as_object() else {
            return Err(format!("{press_place} is an object, like {{\"note\": \"E4\", \"start\": 0, \"hold\": 1}}"));
        };
        if fields.contains_key("key") {
            return Err(format!("{press_place}: a melody names notes or chords, not keys"));
        }
        let (label, _) = pitches_named(fields, &press_place)?;
        said.push(json!({
            "label": label,
            "start": fields.get("start"),
            "hold": fields.get("hold"),
        }));
    }
    let notes = layer_notes::notes_of(&json!({ "notes": listed }))?;
    let phrase = json!({"unit": "beats", "presses": listed});
    phrases::phrase_of(&phrase, key_map, 120)?;
    let choice = Choice {
        name: String::new(),
        phrase,
        chords: Vec::new(),
        notes,
        changes: Value::Null,
    };
    Ok((choice, json!({"presses": said})))
}

// A sound option: changes as jello_sound takes them, and the presses Play
// plays it with, or two chords.
fn sound_choice(option: &Map<String, Value>, place: &str, sound: &Sound) -> Result<(Choice, Value), String> {
    let Some(changes) = option.get("changes") else {
        return Err(format!("{place} needs changes, as jello_sound takes them"));
    };
    let changed = match sound_changes::changed(sound, changes) {
        Ok(changed) => changed,
        Err(why) => return Err(format!("{place}: {why}")),
    };
    let lines = sound_changes::change_lines(sound, &changed);
    let mut phrase = json!({"unit": "beats", "presses": [
        {"chord": "C", "start": 0, "hold": 2},
        {"chord": "Am", "start": 2, "hold": 2},
    ]});
    if let Some(presses) = option.get("presses") {
        phrase = json!({"unit": "beats", "presses": presses});
    }
    let choice = Choice {
        name: String::new(),
        phrase,
        chords: Vec::new(),
        notes: Vec::new(),
        changes: changes.clone(),
    };
    let shown = json!({"lines": lines, "after": sketch_of(&changed), "before": sketch_of(sound)});
    Ok((choice, shown))
}

// The panel's Play on choice `choice` (from 0) of offer `offer`. It plays
// on a thread of its own, so the panel never waits; the player's Stop ends
// it. A sound choice plays with its sound, then the song's sound comes
// back.
#[tauri::command]
pub fn play_agent_option(app: AppHandle, offers: State<'_, Mutex<Offers>>, offer: u64, choice: usize) -> Result<(), String> {
    let (kind, picked) = offers.lock().unwrap().choice(offer, choice)?;
    if app.state::<Phrases>().is_playing() {
        return Err(String::from("something is playing: stop it first"));
    }
    app.state::<Mutex<Engine>>().lock().unwrap().start_audio()?;
    std::thread::spawn(move || {
        let result = tool_server::with_studio(&app, |studio| play_choice(studio, &kind, &picked));
        if let Err(why) = result {
            let _ = app.emit("agent-option-failed", json!({"offer": offer, "why": why}));
        }
    });
    Ok(())
}

fn play_choice(studio: &Studio, kind: &str, picked: &Choice) -> Result<(), String> {
    let key_map = studio.key_map.lock().unwrap();
    let composer = studio.composer.lock().unwrap();
    let phrase = phrases::phrase_of(&picked.phrase, &key_map, composer.tempo())?;
    drop(composer);
    drop(key_map);
    if kind != "sound" {
        phrases::play(studio, &phrase)?;
        return Ok(());
    }

    let (before, tried) = take_sound_changes(studio, &picked.changes)?;
    let played = phrases::play(studio, &phrase);
    // The song's sound comes back, unless the player changed it meanwhile.
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    if composer.sound_profile().sound() == tried {
        sound_changes::take_changes(composer.sound_profile_mut(), &mut engine, &tried, &before);
    }
    played?;
    Ok(())
}

// Makes `changes` to the song's sound, and returns the sound before and
// after.
// The panel's Practice on a melody choice: makes it the piece practised on
// the Play tab, for the player's right hand, and tells the agent so with the
// next message.
#[tauri::command]
pub fn practice_agent_option(app: AppHandle, offers: State<'_, Mutex<Offers>>, offer: u64, choice: usize) -> Result<(), String> {
    let (kind, picked) = offers.lock().unwrap().choice(offer, choice)?;
    if kind != "melody" {
        return Err(String::from("only a melody can be practised"));
    }
    let piece = practice::piece_of(&practice_piece_of(&picked))?;
    let tempo = app.state::<Mutex<Composer>>().lock().unwrap().tempo();
    let view = {
        let practice = app.state::<Mutex<Practice>>();
        let mut practice = practice.lock().unwrap();
        practice::set_piece(&mut practice, piece, &Value::Null, tempo)?;
        practice.view()
    };
    let _ = app.emit("practice", view);
    let practised = format!("the player set option {}, {}, of offer {offer} to practise", choice + 1, picked.name);
    offers.lock().unwrap().used.push(practised);
    Ok(())
}

// A melody choice as a practice piece: its notes as a right-hand part and
// any chords, on the chord keys, as a left-hand part.
fn practice_piece_of(picked: &Choice) -> Value {
    let mut melody = Vec::new();
    let mut chords = Vec::new();
    if let Some(presses) = picked.phrase["presses"].as_array() {
        for press in presses {
            let mut note = Map::new();
            for field in ["note", "chord", "notes", "start", "hold"] {
                if let Some(value) = press.get(field) {
                    note.insert(String::from(field), value.clone());
                }
            }
            if press.get("chord").is_some() {
                chords.push(Value::Object(note));
            } else {
                melody.push(Value::Object(note));
            }
        }
    }
    let mut parts = Vec::new();
    if !melody.is_empty() {
        parts.push(json!({"name": "Melody", "hand": "right", "notes": melody}));
    }
    if !chords.is_empty() {
        parts.push(json!({"name": "Chords", "hand": "left", "notes": chords}));
    }
    json!({
        "name": picked.name,
        "parts": parts,
    })
}

fn take_sound_changes(studio: &Studio, changes: &Value) -> Result<(Sound, Sound), String> {
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    let before = composer.sound_profile().sound();
    let wanted = sound_changes::changed(&before, changes)?;
    sound_changes::take_changes(composer.sound_profile_mut(), &mut engine, &before, &wanted);
    let after = composer.sound_profile().sound();
    Ok((before, after))
}

// The panel's Use on choice `choice` (from 0) of offer `offer`: chords go
// on the end of the song, a melody becomes a new loop layer, a sound
// becomes the song's. The song is kept first, so it can be undone, and the
// number its Undo takes is returned.
#[tauri::command]
pub fn use_agent_option(
    app: AppHandle,
    offers: State<'_, Mutex<Offers>>,
    replies: State<'_, Mutex<Replies>>,
    offer: u64,
    choice: usize,
) -> Result<u64, String> {
    let (kind, picked) = offers.lock().unwrap().choice(offer, choice)?;
    if app.state::<Phrases>().is_playing() {
        return Err(String::from("something is playing: stop it first"));
    }
    let open = app.state::<Mutex<OpenSong>>();
    let composer = app.state::<Mutex<Composer>>();
    let engine = app.state::<Mutex<Engine>>();
    engine.lock().unwrap().start_audio()?;

    let reply = {
        let open = open.lock().unwrap();
        let composer = composer.lock().unwrap();
        let mut engine = engine.lock().unwrap();
        let mut replies = replies.lock().unwrap();
        undo::start_reply(&mut replies, &open, &composer, &mut engine)
    };
    let lines = use_choice(&composer, &engine, &kind, &picked, Instant::now())?;
    let used = format!("the player used option {}, {}, of offer {offer}", choice + 1, picked.name);
    offers.lock().unwrap().used.push(used);
    let _ = app.emit("agent-acted", json!({"tool": "jello_options", "changes": lines}));
    Ok(reply)
}

fn use_choice(
    composer: &Mutex<Composer>,
    engine: &Mutex<Engine>,
    kind: &str,
    picked: &Choice,
    now: Instant,
) -> Result<Vec<String>, String> {
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    if kind == "chords" {
        let ms_per_beat = 60_000.0 / f64::from(composer.tempo());
        let mut names = Vec::new();
        for (chord, hold) in &picked.chords {
            let held_ms = (hold * ms_per_beat).round() as u64;
            composer.add_chord(*chord, held_ms, now);
            names.push(chord_name(*chord, composer.key()));
        }
        return Ok(vec![format!("Added {} to the song", names.join(", "))]);
    }
    if kind == "melody" {
        let written = layer_notes::write_layer(&mut engine, None, None, None, &picked.notes)?;
        return Ok(vec![written.line]);
    }
    let before = composer.sound_profile().sound();
    let wanted = sound_changes::changed(&before, &picked.changes)?;
    sound_changes::take_changes(composer.sound_profile_mut(), &mut engine, &before, &wanted);
    let after = composer.sound_profile().sound();
    Ok(sound_changes::change_lines(&before, &after))
}

// Field `name` as text.
fn text_field(fields: &Map<String, Value>, name: &str, place: &str) -> Result<String, String> {
    match fields.get(name).and_then(Value::as_str) {
        Some(text) => Ok(String::from(text)),
        None => Err(format!("{place} needs {name}, as text")),
    }
}

fn optional_text_field(fields: &Map<String, Value>, name: &str, place: &str) -> Result<Option<String>, String> {
    match fields.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => Ok(Some(text_field(fields, name, place)?)),
    }
}

// Field "name": short text, not empty.
fn name_field(fields: &Map<String, Value>, place: &str) -> Result<String, String> {
    name_field_called(fields, "name", place, MOST_NAME_CHARACTERS)
}

fn name_field_called(fields: &Map<String, Value>, name: &str, place: &str, most: usize) -> Result<String, String> {
    let text = text_field(fields, name, place)?;
    let text = text.trim();
    if text.is_empty() || text.chars().count() > most {
        return Err(format!("{place}'s {name} is 1 to {most} characters"));
    }
    Ok(String::from(text))
}

fn number_field(fields: &Map<String, Value>, name: &str, place: &str) -> Result<f64, String> {
    match fields.get(name).and_then(Value::as_f64) {
        Some(number) => Ok(number),
        None => Err(format!("{place} needs {name}, as a number")),
    }
}

// Field `name`: a list of `fewest` to `most` things.
fn list_field<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
    place: &str,
    fewest: usize,
    most: usize,
) -> Result<&'a Vec<Value>, String> {
    let Some(listed) = fields.get(name).and_then(Value::as_array) else {
        return Err(format!("{place} needs {name}, as a list"));
    };
    if listed.len() < fewest || listed.len() > most {
        return Err(format!("{place} has {fewest} to {most} {name}, not {}", listed.len()));
    }
    Ok(listed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerals_count_from_the_key_s_home_note() {
        let key = key_of_name("C major").unwrap();
        let chords = ["C", "Dm", "F", "G", "Am", "Bb", "Eb"];
        let mut numerals = Vec::new();
        for name in chords {
            numerals.push(numeral_of(key, chord_of_name(name).unwrap()));
        }
        assert_eq!(numerals, vec!["I", "ii", "IV", "V", "vi", "bVII", "bIII"]);
    }

    #[test]
    fn using_a_choice_puts_it_in_the_song() {
        let mut engine = Engine::new().unwrap();
        let composer = Composer::new();
        composer.send_all(&mut engine);
        let composer = Mutex::new(composer);
        let engine = Mutex::new(engine);

        let chords = Choice {
            name: String::from("Darker"),
            phrase: Value::Null,
            chords: vec![(chord_of_name("Am").unwrap(), 2.0), (chord_of_name("F").unwrap(), 4.0)],
            notes: Vec::new(),
            changes: Value::Null,
        };
        let lines = use_choice(&composer, &engine, "chords", &chords, Instant::now()).unwrap();
        assert_eq!(lines, vec![String::from("Added Am, F to the song")]);
        // At 120 beats per minute, 2 beats are 1 s.
        let held = composer.lock().unwrap().held_chords();
        assert_eq!(held.len(), 2);
        assert_eq!(held[0].held_ms, 1000);
        assert_eq!(held[1].held_ms, 2000);

        let sound = Choice {
            name: String::from("Warm"),
            phrase: Value::Null,
            chords: Vec::new(),
            notes: Vec::new(),
            changes: json!({"filter": {"cutoff_hz": 900}}),
        };
        let lines = use_choice(&composer, &engine, "sound", &sound, Instant::now()).unwrap();
        assert_eq!(lines, vec![String::from("Filter cutoff 20000 -> 900 Hz")]);

        let melody = Choice {
            name: String::from("Up"),
            phrase: Value::Null,
            chords: Vec::new(),
            notes: vec![WrittenNote { note: 64, start: 0.0, end: 1.0 }],
            changes: Value::Null,
        };
        let lines = use_choice(&composer, &engine, "melody", &melody, Instant::now()).unwrap();
        assert_eq!(lines, vec![String::from("New layer 1: 1 notes over 1 bars")]);
    }

    #[test]
    fn a_melody_choice_becomes_a_piece_for_both_hands() {
        let melody = Choice {
            name: String::from("As sung"),
            phrase: json!({"unit": "beats", "presses": [
                {"note": "C4", "start": 0, "hold": 1},
                {"chord": "Am", "start": 1, "hold": "1/2"},
            ]}),
            chords: Vec::new(),
            notes: Vec::new(),
            changes: Value::Null,
        };
        let piece = practice::piece_of(&practice_piece_of(&melody)).unwrap();
        assert_eq!(piece.name, "As sung");
        assert_eq!(piece.parts.len(), 2);
        assert_eq!(piece.parts[0].name, "Melody");
        assert_eq!(piece.parts[0].hand, crate::scoring::Hand::Right);
        assert_eq!(piece.parts[0].notes.len(), 1);
        assert_eq!(piece.parts[1].name, "Chords");
        assert_eq!(piece.parts[1].hand, crate::scoring::Hand::Left);
        assert_eq!(piece.parts[1].notes.len(), 1);
        let mut practice = Practice::default();
        practice::set_piece(&mut practice, piece, &Value::Null, 90).unwrap();
        assert_eq!(practice.settings.tempo, 90);
        assert!(practice.piece.is_some());
    }

    #[test]
    fn old_offers_are_forgotten() {
        let mut offers = Offers::default();
        for _ in 0..(KEPT_OFFERS + 1) {
            offers.keep("chords", Vec::new());
        }
        assert!(!offers.has(1));
        assert!(offers.has(2));
        let too_old = offers.choice(1, 0).unwrap_err();
        assert_eq!(too_old, "that offer is too old to use: ask the agent again");
        assert_eq!(offers.choice(2, 0).unwrap_err(), "the offer has no choice 1");
    }
}
