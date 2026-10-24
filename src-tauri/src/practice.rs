// jello_practice (docs/design.md#the-tutor-to-mastery): the Tutor sets a
// piece, Jello shows it on the Play tab's lane and the keyboard, plays
// Jello's parts and the count in, and scores every try in Rust
// (scoring.rs). The runner that plays a try is practice_run.rs.
//
// A piece has up to 4 parts, each the player's left or right hand or
// Jello's own. A note is a melody note, a chord key's chord, or several
// melody notes at once, with a start and a hold in beats: a number, or a
// fraction like "1/3" or "2 1/3" for triplets. A note can move the
// octave first, and the move holds for the notes after it, as the arrows
// do. Sections name bars. At most 64 bars and 2048 notes.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, State};

use crate::engine::{FASTEST_TEMPO, SLOWEST_TEMPO};
use crate::harmony::{chord_name, chord_of_name};
use crate::key_map::KeyMap;
use crate::scoring::{Expected, Hand};
use crate::song_file::{note_name, note_of_name};
use crate::tools::{Studio, ToolAnswer};

pub const BEATS_PER_BAR: f64 = 4.0;
const MOST_PARTS: usize = 4;
const MOST_BARS: f64 = 64.0;
const MOST_NOTES: usize = 2048;
// A fraction's bottom, at most: 64ths.
const MOST_PARTS_OF_A_BEAT: u32 = 64;
// The melody keys at octave 0, G#3 to G5, and the chord keys' roots, C3 to
// B3. Each octave moves both by 12.
const LOWEST_MELODY_NOTE: i32 = 56;
const HIGHEST_MELODY_NOTE: i32 = 79;
// How many tries in a row a ladder or a loop plays, at most.
pub const MOST_PASSES: usize = 100;

// The modes: playing along, the ear (Jello plays, the player answers),
// improvising over Jello's parts, and from memory (one listen, then the
// lane hides).
pub const MODES: [&str; 4] = ["along", "ear", "improvise", "memory"];
pub const HANDS: [&str; 3] = ["both", "left", "right"];

// The levels and their windows: how far from a note a press still hits it.
pub const LEVELS: [(&str, f64); 6] = [
    ("beginner", 150.0),
    ("novice", 120.0),
    ("intermediate", 90.0),
    ("advanced", 65.0),
    ("expert", 50.0),
    ("master", 40.0),
];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Piece {
    pub name: String,
    pub octave: i8,
    pub bars: u32,
    pub parts: Vec<Part>,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Part {
    pub name: String,
    pub hand: Hand,
    pub notes: Vec<PieceNote>,
}

// Whether a note is played on the melody keys or a chord key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Played {
    Melody,
    Chord,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PieceNote {
    // Counted across the whole piece, from 0, in the order written.
    pub index: usize,
    pub label: String,
    pub pitches: Vec<u8>,
    pub played: Played,
    pub start: f64,
    pub hold: f64,
    // The octave the keys are at for this note, and the move to it, if the
    // note asks for one.
    pub octave: i8,
    pub moved: Option<i8>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Section {
    pub name: String,
    pub from_bar: u32,
    pub to_bar: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Ladder {
    // Beats per minute faster each step.
    pub step: u32,
    // Clean tries in a row before each step.
    pub clean: u32,
    pub top: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Settings {
    pub tempo: u32,
    // How long the first of two eighths is, in percent of the beat: 50 is
    // straight, about 67 a triplet swing.
    pub swing: u32,
    pub count_in: u32,
    pub click: bool,
    pub wait: bool,
    pub section: Option<String>,
    #[serde(rename = "loop")]
    pub loop_section: bool,
    pub ladder: Option<Ladder>,
    pub hands: String,
    pub mode: String,
    pub coach: bool,
    pub level: String,
}

impl Settings {
    fn starting(tempo: u32) -> Settings {
        Settings {
            tempo,
            swing: 50,
            count_in: 1,
            click: true,
            wait: false,
            section: None,
            loop_section: false,
            ladder: None,
            hands: String::from("both"),
            mode: String::from("along"),
            coach: true,
            level: String::from("beginner"),
        }
    }

    // How far from a note a press still hits it, for the level.
    pub fn window_ms(&self) -> f64 {
        for (level, window) in LEVELS {
            if level == self.level {
                return window;
            }
        }
        LEVELS[0].1
    }

    // The hands the player plays. Jello plays the rest.
    pub fn player_hands(&self) -> Vec<Hand> {
        if self.hands == "left" {
            return vec![Hand::Left];
        }
        if self.hands == "right" {
            return vec![Hand::Right];
        }
        vec![Hand::Left, Hand::Right]
    }
}

// What practice keeps: the piece set, its settings, this practice's tries,
// the lines the agent hasn't been told yet, and the try playing, if one
// is.
pub struct Practice {
    pub piece: Option<Piece>,
    pub settings: Settings,
    pub tries: Vec<Value>,
    unsent: Vec<String>,
    pub running: Option<Arc<AtomicBool>>,
    // What the try is doing, like "Counting in" or "Listen", for the view.
    pub phase: String,
    // The tempo the ladder is at, and how many clean tries in a row.
    pub tempo_now: u32,
    pub clean_in_row: u32,
    // Why practice plays at another tempo than the one set, if it does.
    pub tempo_note: Option<String>,
}

impl Default for Practice {
    fn default() -> Practice {
        Practice {
            piece: None,
            settings: Settings::starting(100),
            tries: Vec::new(),
            unsent: Vec::new(),
            running: None,
            phase: String::new(),
            tempo_now: 100,
            clean_in_row: 0,
            tempo_note: None,
        }
    }
}

impl Practice {
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    // Asks the try playing to stop. It stops within a few ms.
    pub fn stop(&self) {
        if let Some(stop) = &self.running {
            stop.store(true, Ordering::Relaxed);
        }
    }

    // A line for the agent's next message, like a used option's.
    pub fn tell_agent(&mut self, line: String) {
        self.unsent.push(line);
    }

    // The lines the agent hasn't been told yet, once.
    pub fn take_unsent(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unsent)
    }

    // What the Play tab and the agent see.
    pub fn view(&self) -> Value {
        let Some(piece) = &self.piece else {
            return json!({"set": false});
        };
        json!({
            "set": true,
            "piece": {
                "name": piece.name,
                "bars": piece.bars,
                "octave": piece.octave,
                "parts": parts_view(piece, &self.settings),
                "sections": piece.sections,
            },
            "settings": self.settings,
            "window_ms": self.settings.window_ms(),
            "tempo_now": self.tempo_now,
            "tempo_note": self.tempo_note,
            "lane": lane_of(piece, &self.settings, self.tempo_now),
            "running": self.is_running(),
            "phase": self.phase,
            "tries": self.tries,
        })
    }
}

fn parts_view(piece: &Piece, settings: &Settings) -> Value {
    let player = settings.player_hands();
    let mut parts = Vec::new();
    for part in &piece.parts {
        parts.push(json!({
            "name": part.name,
            "hand": part.hand,
            "notes": part.notes.len(),
            "player": player.contains(&part.hand),
        }));
    }
    json!(parts)
}

// The piece as a lane card draws it, in beats from the section's start,
// with each part's notes. Only the section's notes are drawn.
pub fn lane_of(piece: &Piece, settings: &Settings, tempo: u32) -> Value {
    let (from_beat, to_beat) = beats_played(piece, settings);
    let player = settings.player_hands();
    let mut parts = Vec::new();
    for part in &piece.parts {
        let mut notes = Vec::new();
        for note in &part.notes {
            if note.start < from_beat || note.start >= to_beat {
                continue;
            }
            notes.push(json!({
                "index": note.index,
                "label": note.label,
                "pitches": note.pitches,
                "start": note.start - from_beat,
                "hold": note.hold,
                "moved": note.moved,
            }));
        }
        parts.push(json!({
            "name": part.name,
            "hand": part.hand,
            "player": player.contains(&part.hand),
            "notes": notes,
        }));
    }
    json!({
        "kind": "lane",
        "title": piece.name,
        "beats": to_beat - from_beat,
        "tempo": tempo,
        "parts": parts,
    })
}

// The beats a try plays: the section's, or the whole piece's.
pub fn beats_played(piece: &Piece, settings: &Settings) -> (f64, f64) {
    if let Some(name) = &settings.section {
        for section in &piece.sections {
            if section.name == *name {
                let from = f64::from(section.from_bar - 1) * BEATS_PER_BAR;
                let to = f64::from(section.to_bar) * BEATS_PER_BAR;
                return (from, to);
            }
        }
    }
    (0.0, f64::from(piece.bars) * BEATS_PER_BAR)
}

// A note Jello plays: its pitch, and when it starts and ends, in ms from
// the try's first beat.
#[derive(Clone, Debug, PartialEq)]
pub struct Sounding {
    pub pitch: u8,
    pub at_ms: f64,
    pub end_ms: f64,
}

// One try, worked out at a tempo: the notes the player should play, the
// notes Jello plays, and how long it lasts.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub expected: Vec<Expected>,
    pub jello: Vec<Sounding>,
    pub length_ms: f64,
    pub ms_per_beat: f64,
    pub from_beat: f64,
}

// The plan for a try of `piece` at `tempo`: the player's parts to play,
// and the rest for Jello.
pub fn plan(piece: &Piece, settings: &Settings, tempo: u32) -> Plan {
    let player = settings.player_hands();
    plan_with(piece, settings, tempo, |hand| player.contains(&hand))
}

// The plan for a listen: Jello plays the player's parts, and nothing is
// scored.
pub fn listen_plan(piece: &Piece, settings: &Settings, tempo: u32) -> Plan {
    let player = settings.player_hands();
    let mut plan = plan_with(piece, settings, tempo, |hand| player.contains(&hand));
    plan.jello.clear();
    for expected in plan.expected.drain(..) {
        for pitch in expected.pitches {
            plan.jello.push(Sounding {
                pitch,
                at_ms: expected.at_ms,
                end_ms: expected.at_ms + expected.hold_ms,
            });
        }
    }
    plan.jello.sort_by(|first, second| first.at_ms.total_cmp(&second.at_ms));
    plan
}

fn plan_with(piece: &Piece, settings: &Settings, tempo: u32, is_player: impl Fn(Hand) -> bool) -> Plan {
    let ms_per_beat = 60_000.0 / f64::from(tempo);
    let (from_beat, to_beat) = beats_played(piece, settings);
    let mut expected = Vec::new();
    let mut jello = Vec::new();
    for part in &piece.parts {
        let player_part = part.hand != Hand::Jello && is_player(part.hand);
        for note in &part.notes {
            if note.start < from_beat || note.start >= to_beat {
                continue;
            }
            let at_ms = swung(note.start - from_beat, settings.swing) * ms_per_beat;
            let hold_ms = note.hold * ms_per_beat;
            if !player_part {
                for pitch in &note.pitches {
                    jello.push(Sounding {
                        pitch: *pitch,
                        at_ms,
                        end_ms: at_ms + hold_ms,
                    });
                }
                continue;
            }
            // A chord key is one press. Melody notes played at once are a
            // press each.
            let mut presses = Vec::new();
            if note.played == Played::Chord {
                presses.push(note.pitches.clone());
            } else {
                for pitch in &note.pitches {
                    presses.push(vec![*pitch]);
                }
            }
            for pitches in presses {
                expected.push(Expected {
                    index: note.index,
                    hand: part.hand,
                    pitches,
                    at_ms,
                    hold_ms,
                    section: section_of(piece, note.start),
                });
            }
        }
    }
    expected.sort_by(|first, second| first.at_ms.total_cmp(&second.at_ms));
    jello.sort_by(|first, second| first.at_ms.total_cmp(&second.at_ms));
    Plan {
        expected,
        jello,
        length_ms: (to_beat - from_beat) * ms_per_beat,
        ms_per_beat,
        from_beat,
    }
}

// An off-beat eighth moved later by the swing.
fn swung(beat: f64, swing: u32) -> f64 {
    let whole = beat.floor();
    let rest = beat - whole;
    if (rest - 0.5).abs() < 0.000_1 {
        return whole + f64::from(swing) / 100.0;
    }
    beat
}

fn section_of(piece: &Piece, beat: f64) -> Option<String> {
    let bar = (beat / BEATS_PER_BAR).floor() as u32 + 1;
    for section in &piece.sections {
        if bar >= section.from_bar && bar <= section.to_bar {
            return Some(section.name.clone());
        }
    }
    None
}

// The piece `fields` describe, checked.
pub fn piece_of(fields: &Value) -> Result<Piece, String> {
    let Some(fields) = fields.as_object() else {
        return Err(String::from("piece is an object, like {\"name\": \"C major warm up\", \"parts\": [...]}"));
    };
    let name = match fields.get("name").and_then(Value::as_str) {
        Some(name) => name.trim().to_string(),
        None => String::from("Practice"),
    };
    let octave = octave_field(fields, "octave", "the piece")?.unwrap_or(0);
    let Some(listed_parts) = fields.get("parts").and_then(Value::as_array) else {
        return Err(String::from("the piece needs parts, a list of 1 to 4"));
    };
    if listed_parts.is_empty() || listed_parts.len() > MOST_PARTS {
        return Err(format!("a piece has 1 to {MOST_PARTS} parts, not {}", listed_parts.len()));
    }

    // Each note as written, then the octave moves in time order.
    let mut parts = Vec::new();
    let mut count = 0;
    for (part_place, listed) in listed_parts.iter().enumerate() {
        let place = format!("part {}", part_place + 1);
        let part = part_of(listed, &place, &mut count)?;
        parts.push(part);
    }
    if count > MOST_NOTES {
        return Err(format!("a piece has at most {MOST_NOTES} notes, not {count}"));
    }
    give_octaves(&mut parts, octave)?;

    let mut last_end: f64 = 0.0;
    for part in &parts {
        for note in &part.notes {
            last_end = last_end.max(note.start + note.hold);
        }
    }
    let bars = (last_end / BEATS_PER_BAR).ceil().max(1.0);
    if bars > MOST_BARS {
        return Err(format!("a piece is at most {MOST_BARS} bars, and this one ends in bar {bars}"));
    }
    let bars = bars as u32;
    let sections = sections_of(fields.get("sections"), bars)?;
    Ok(Piece {
        name,
        octave,
        bars,
        parts,
        sections,
    })
}

fn part_of(listed: &Value, place: &str, count: &mut usize) -> Result<Part, String> {
    let Some(fields) = listed.as_object() else {
        return Err(format!("{place} is an object, like {{\"name\": \"Right hand\", \"hand\": \"right\", \"notes\": [...]}}"));
    };
    let name = match fields.get("name").and_then(Value::as_str) {
        Some(name) => name.trim().to_string(),
        None => place.to_string(),
    };
    let hand = match fields.get("hand").and_then(Value::as_str) {
        Some("left") => Hand::Left,
        Some("right") => Hand::Right,
        Some("jello") => Hand::Jello,
        _ => return Err(format!("{place}'s hand is left, right or jello")),
    };
    let Some(listed_notes) = fields.get("notes").and_then(Value::as_array) else {
        return Err(format!("{place} needs notes, a list"));
    };
    if listed_notes.is_empty() {
        return Err(format!("{place} has no notes"));
    }
    let mut notes = Vec::new();
    for (note_place, listed_note) in listed_notes.iter().enumerate() {
        let note_place = format!("{place}, note {}", note_place + 1);
        let note = note_of(listed_note, &note_place, *count)?;
        *count += 1;
        notes.push(note);
    }
    Ok(Part { name, hand, notes })
}

// A note as written. Its octave is given after, in time order.
fn note_of(listed: &Value, place: &str, index: usize) -> Result<PieceNote, String> {
    let Some(fields) = listed.as_object() else {
        return Err(format!("{place} is an object, like {{\"note\": \"E4\", \"start\": 0, \"hold\": 1}}"));
    };
    let Some(start) = fields.get("start") else {
        return Err(format!("{place} needs a start, in beats from 0"));
    };
    let start = beats_of(start, &format!("{place}'s start"))?;
    let Some(hold) = fields.get("hold") else {
        return Err(format!("{place} needs a hold, in beats"));
    };
    let hold = beats_of(hold, &format!("{place}'s hold"))?;
    if hold <= 0.0 {
        return Err(format!("{place} holds for more than 0 beats"));
    }
    let moved = octave_field(fields, "octave", place)?;

    let label;
    let pitches;
    let played;
    if let Some(name) = fields.get("note") {
        let name = name.as_str().unwrap_or_default();
        let Some(note) = note_of_name(name) else {
            return Err(format!("{place}: {name} is not a note, like \"C4\" or \"F#3\""));
        };
        label = note_name(note);
        pitches = vec![note];
        played = Played::Melody;
    } else if let Some(name) = fields.get("chord") {
        let name = name.as_str().unwrap_or_default();
        let Some(chord) = chord_of_name(name) else {
            return Err(format!(
                "{place}: {name} is not a chord a chord key plays, like \"Am\" or \"Bb\". Other chords go as \"notes\" on the melody keys"
            ));
        };
        label = chord_name(chord, None);
        // Moved to the octave once it is known.
        pitches = chord.notes().to_vec();
        played = Played::Chord;
    } else if let Some(listed) = fields.get("notes").and_then(Value::as_array) {
        if listed.is_empty() || listed.len() > 4 {
            return Err(format!("{place} plays 1 to 4 notes at once"));
        }
        let mut found = Vec::new();
        let mut names = Vec::new();
        for item in listed {
            let name = item.as_str().unwrap_or_default();
            let Some(note) = note_of_name(name) else {
                return Err(format!("{place}: {name} is not a note, like \"C4\" or \"F#3\""));
            };
            if !found.contains(&note) {
                found.push(note);
            }
        }
        found.sort();
        for note in &found {
            names.push(note_name(*note));
        }
        label = names.join(" ");
        pitches = found;
        played = Played::Melody;
    } else {
        return Err(format!("{place} names a note, a chord or notes"));
    }
    Ok(PieceNote {
        index,
        label,
        pitches,
        played,
        start,
        hold,
        octave: 0,
        moved,
    })
}

// Walks every note in time order, carrying the octave from move to move,
// and checks each of the player's notes is on a key at its octave. Jello
// plays any note.
fn give_octaves(parts: &mut [Part], starting: i8) -> Result<(), String> {
    let mut order = Vec::new();
    for (part_place, part) in parts.iter().enumerate() {
        for (note_place, note) in part.notes.iter().enumerate() {
            order.push((note.start, note.index, part_place, note_place));
        }
    }
    order.sort_by(|first, second| first.0.total_cmp(&second.0).then(first.1.cmp(&second.1)));

    let mut octave = starting;
    for (_, _, part_place, note_place) in order {
        let hand = parts[part_place].hand;
        let note = &mut parts[part_place].notes[note_place];
        if let Some(moved) = note.moved {
            octave = moved;
        }
        note.octave = octave;
        let shift = i32::from(octave) * 12;
        if note.played == Played::Chord {
            let mut pitches = Vec::new();
            for pitch in &note.pitches {
                pitches.push((i32::from(*pitch) + shift) as u8);
            }
            note.pitches = pitches;
            continue;
        }
        if hand == Hand::Jello {
            continue;
        }
        for pitch in &note.pitches {
            let lowest = LOWEST_MELODY_NOTE + shift;
            let highest = HIGHEST_MELODY_NOTE + shift;
            let pitch = i32::from(*pitch);
            if pitch < lowest || pitch > highest {
                return Err(format!(
                    "{} (note {} of the piece) is off the melody keys at octave {octave}, which reach {} to {}: move the octave before it",
                    note_name(pitch as u8),
                    note.index + 1,
                    note_name(lowest as u8),
                    note_name(highest as u8),
                ));
            }
        }
    }
    Ok(())
}

// An octave, from -4 to 4, or None if left out.
fn octave_field(fields: &Map<String, Value>, name: &str, place: &str) -> Result<Option<i8>, String> {
    let Some(value) = fields.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(number) = value.as_i64() else {
        return Err(format!("{place}'s {name} is a whole number from -4 to 4"));
    };
    if !(-4..=4).contains(&number) {
        return Err(format!("{place}'s {name} is from -4 to 4, not {number}"));
    }
    Ok(Some(number as i8))
}

// Beats as a number, like 2.5, or as text with a fraction, like "1/3" or
// "2 1/3".
pub fn beats_of(value: &Value, place: &str) -> Result<f64, String> {
    let wrong = format!("{place} is in beats, like 2, 0.5, \"1/3\" or \"2 1/3\"");
    if let Some(number) = value.as_f64() {
        if !number.is_finite() || number < 0.0 {
            return Err(wrong);
        }
        return Ok(number);
    }
    let Some(text) = value.as_str() else {
        return Err(wrong);
    };
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut beats = 0.0;
    let fraction;
    if words.len() == 1 {
        fraction = words[0];
    } else if words.len() == 2 {
        let Ok(whole) = words[0].parse::<u32>() else {
            return Err(wrong);
        };
        beats = f64::from(whole);
        fraction = words[1];
    } else {
        return Err(wrong);
    }
    let Some((top, bottom)) = fraction.split_once('/') else {
        let Ok(number) = fraction.parse::<f64>() else {
            return Err(wrong);
        };
        if words.len() == 2 || !number.is_finite() || number < 0.0 {
            return Err(wrong);
        }
        return Ok(number);
    };
    let Ok(top) = top.parse::<u32>() else {
        return Err(wrong);
    };
    let Ok(bottom) = bottom.parse::<u32>() else {
        return Err(wrong);
    };
    if bottom == 0 || bottom > MOST_PARTS_OF_A_BEAT {
        return Err(format!("{place}: a beat splits into at most {MOST_PARTS_OF_A_BEAT} parts"));
    }
    beats += f64::from(top) / f64::from(bottom);
    Ok(beats)
}

fn sections_of(listed: Option<&Value>, bars: u32) -> Result<Vec<Section>, String> {
    let Some(listed) = listed else {
        return Ok(Vec::new());
    };
    if listed.is_null() {
        return Ok(Vec::new());
    }
    let Some(listed) = listed.as_array() else {
        return Err(String::from("sections is a list, like [{\"name\": \"A\", \"from_bar\": 1, \"to_bar\": 4}]"));
    };
    let mut sections: Vec<Section> = Vec::new();
    for (place, item) in listed.iter().enumerate() {
        let place = format!("section {}", place + 1);
        let Some(name) = item["name"].as_str() else {
            return Err(format!("{place} needs a name"));
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(format!("{place} needs a name"));
        }
        let known = sections.iter().any(|section| section.name == name);
        if known {
            return Err(format!("two sections are called {name}"));
        }
        let (Some(from_bar), Some(to_bar)) = (item["from_bar"].as_u64(), item["to_bar"].as_u64()) else {
            return Err(format!("{place} needs from_bar and to_bar, counted from 1"));
        };
        if from_bar < 1 || to_bar < from_bar || to_bar > u64::from(bars) {
            return Err(format!("{place} runs from bar 1 or later to bar {bars} or earlier, and not backwards"));
        }
        sections.push(Section {
            name,
            from_bar: from_bar as u32,
            to_bar: to_bar as u32,
        });
    }
    Ok(sections)
}

// `settings` with the fields `changes` gives, checked against `piece`.
pub fn settings_with(settings: &Settings, changes: &Value, piece: &Piece) -> Result<Settings, String> {
    let Some(changes) = changes.as_object() else {
        return Err(String::from("settings is an object, like {\"tempo\": 80, \"level\": \"novice\"}"));
    };
    let mut changed = settings.clone();
    for (name, value) in changes {
        match name.as_str() {
            "tempo" => {
                let tempo = whole_in(value, name, SLOWEST_TEMPO, FASTEST_TEMPO)?;
                changed.tempo = tempo;
            }
            "swing" => changed.swing = whole_in(value, name, 50, 75)?,
            "count_in" => changed.count_in = whole_in(value, name, 0, 2)?,
            "click" => changed.click = flag(value, name)?,
            "wait" => changed.wait = flag(value, name)?,
            "loop" => changed.loop_section = flag(value, name)?,
            "coach" => changed.coach = flag(value, name)?,
            "section" => {
                if value.is_null() {
                    changed.section = None;
                    continue;
                }
                let Some(wanted) = value.as_str() else {
                    return Err(String::from("section is a section's name, or null for the whole piece"));
                };
                let known = piece.sections.iter().any(|section| section.name == wanted);
                if !known {
                    return Err(format!("the piece has no section called {wanted}"));
                }
                changed.section = Some(String::from(wanted));
            }
            "ladder" => {
                if value.is_null() {
                    changed.ladder = None;
                    continue;
                }
                let step = whole_in(&value["step"], "ladder's step", 1, 20)?;
                let clean = whole_in(&value["clean"], "ladder's clean", 1, 10)?;
                let top = whole_in(&value["top"], "ladder's top", SLOWEST_TEMPO, FASTEST_TEMPO)?;
                changed.ladder = Some(Ladder { step, clean, top });
            }
            "hands" => changed.hands = word_in(value, name, &HANDS)?,
            "mode" => changed.mode = word_in(value, name, &MODES)?,
            "level" => {
                let mut levels = Vec::new();
                for (level, _) in LEVELS {
                    levels.push(level);
                }
                changed.level = word_in(value, name, &levels)?;
            }
            _ => return Err(format!("practice has no setting called {name}")),
        }
    }
    if let Some(ladder) = changed.ladder {
        if ladder.top <= changed.tempo {
            return Err(format!("the ladder's top, {}, is faster than the tempo, {}", ladder.top, changed.tempo));
        }
    }
    let player = changed.player_hands();
    let has_player_part = piece.parts.iter().any(|part| player.contains(&part.hand));
    let has_jello_part = piece.parts.iter().any(|part| !player.contains(&part.hand));
    if changed.mode == "improvise" {
        if !has_jello_part {
            return Err(String::from("improvising needs a part for Jello to play, the chords to improvise over"));
        }
    } else if !has_player_part {
        return Err(format!("the piece has no part for the player's hands ({})", changed.hands));
    }
    Ok(changed)
}

fn whole_in(value: &Value, name: &str, lowest: u32, highest: u32) -> Result<u32, String> {
    let Some(number) = value.as_u64() else {
        return Err(format!("{name} is a whole number from {lowest} to {highest}"));
    };
    if number < u64::from(lowest) || number > u64::from(highest) {
        return Err(format!("{name} is from {lowest} to {highest}, not {number}"));
    }
    Ok(number as u32)
}

fn flag(value: &Value, name: &str) -> Result<bool, String> {
    match value.as_bool() {
        Some(flag) => Ok(flag),
        None => Err(format!("{name} is true or false")),
    }
}

fn word_in(value: &Value, name: &str, words: &[&str]) -> Result<String, String> {
    let Some(word) = value.as_str() else {
        return Err(format!("{name} is one of {}", words.join(", ")));
    };
    if !words.contains(&word) {
        return Err(format!("{name} is one of {}, not {word}", words.join(", ")));
    }
    Ok(String::from(word))
}

// The laptop key that plays `pitches` at the keys' octave now, if one does:
// a melody key for one note, or a chord key for a chord's.
pub fn key_for(key_map: &KeyMap, pitches: &[u8]) -> Option<String> {
    if pitches.len() == 1 {
        for (key, note) in key_map.note_keys() {
            if note == pitches[0] {
                return Some(key);
            }
        }
        return None;
    }
    for (key, chord) in key_map.chord_keys() {
        if chord.notes().as_slice() == pitches {
            return Some(key);
        }
    }
    None
}

// The tempo the next try plays at, after `clean` (whether the try just
// played was clean): the ladder steps up after enough clean tries in a
// row. Returns whether the ladder is done: at its top, with enough clean
// tries there.
pub fn climb(practice: &mut Practice, clean: bool) -> bool {
    let Some(ladder) = practice.settings.ladder else {
        return false;
    };
    if clean {
        practice.clean_in_row += 1;
    } else {
        practice.clean_in_row = 0;
    }
    if practice.clean_in_row < ladder.clean {
        return false;
    }
    if practice.tempo_now >= ladder.top {
        return true;
    }
    practice.clean_in_row = 0;
    practice.tempo_now = (practice.tempo_now + ladder.step).min(ladder.top);
    false
}

// jello_practice: sets a piece, changes its settings, starts or stops a
// try, reads the results, or closes practice.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let Some(action) = arguments["action"].as_str() else {
        return Err(String::from("action is needed: set, settings, start, stop, results or close"));
    };
    match action {
        "set" => set(studio, arguments),
        "settings" => change_settings(studio, arguments),
        "start" => {
            studio.screen.start_practice()?;
            let view = studio.practice.lock().unwrap().view();
            Ok(ToolAnswer::of(json!({"started": true, "practice": short_view(&view)}), vec![String::from("Started a practice try")]))
        }
        "stop" => {
            let practice = studio.practice.lock().unwrap();
            if !practice.is_running() {
                return Err(String::from("no try is playing"));
            }
            practice.stop();
            Ok(ToolAnswer::of(json!({"stopped": true}), vec![String::from("Stopped the practice try")]))
        }
        "results" => {
            let practice = studio.practice.lock().unwrap();
            Ok(ToolAnswer::of(json!({"tries": practice.tries, "running": practice.is_running()}), Vec::new()))
        }
        "close" => {
            let mut practice = studio.practice.lock().unwrap();
            practice.stop();
            practice.piece = None;
            practice.tries.clear();
            let view = practice.view();
            drop(practice);
            studio.screen.show_practice(&view);
            Ok(ToolAnswer::of(json!({"closed": true}), vec![String::from("Closed practice")]))
        }
        _ => Err(format!("practice has no action called {action}: set, settings, start, stop, results or close")),
    }
}

fn set(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let piece = piece_of(&arguments["piece"])?;
    let tempo = studio.composer.lock().unwrap().tempo();
    let mut practice = studio.practice.lock().unwrap();
    if practice.is_running() {
        return Err(String::from("a try is playing: stop it first"));
    }
    let mut settings = Settings::starting(tempo);
    if !arguments["settings"].is_null() {
        settings = settings_with(&settings, &arguments["settings"], &piece)?;
    } else {
        settings = settings_with(&settings, &json!({}), &piece)?;
    }
    let lines = vec![format!("Set the piece {}, {} bars", piece.name, piece.bars)];
    practice.tempo_now = settings.tempo;
    practice.clean_in_row = 0;
    practice.settings = settings;
    practice.piece = Some(piece);
    practice.tries.clear();
    practice.tempo_note = None;
    let view = practice.view();
    drop(practice);
    studio.screen.show_practice(&view);
    let card = json!({"kind": "practice", "title": "", "view": short_view(&view)});
    Ok(ToolAnswer::of(short_view(&view), lines).with_card(card))
}

fn change_settings(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let mut practice = studio.practice.lock().unwrap();
    if practice.is_running() {
        return Err(String::from("a try is playing: stop it first"));
    }
    let Some(piece) = &practice.piece else {
        return Err(String::from("no piece is set: set one first"));
    };
    let settings = settings_with(&practice.settings, &arguments["settings"], piece)?;
    let mut lines = Vec::new();
    if let Some(changes) = arguments["settings"].as_object() {
        for (name, value) in changes {
            lines.push(format!("Practice {name}: {value}"));
        }
    }
    if settings.tempo != practice.settings.tempo || settings.ladder != practice.settings.ladder {
        practice.tempo_now = settings.tempo;
        practice.clean_in_row = 0;
    }
    practice.settings = settings;
    let view = practice.view();
    drop(practice);
    studio.screen.show_practice(&view);
    let card = json!({"kind": "practice", "title": "", "view": short_view(&view)});
    Ok(ToolAnswer::of(short_view(&view), lines).with_card(card))
}

// The view without the lane's notes and old tries, for the agent: it wrote
// the notes itself.
fn short_view(view: &Value) -> Value {
    let mut short = view.clone();
    if let Some(fields) = short.as_object_mut() {
        fields.remove("lane");
        let tries = fields.get("tries").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        fields.remove("tries");
        fields.insert(String::from("tries_so_far"), json!(tries));
    }
    short
}

// The Play tab's practice: its view, as "practice" sends it too.
#[tauri::command]
pub fn practice_view(practice: State<'_, Mutex<Practice>>) -> Value {
    practice.lock().unwrap().view()
}

// The Start button.
#[tauri::command]
pub fn start_practice(app: AppHandle) -> Result<(), String> {
    crate::practice_run::start(&app)
}

// The Stop button, and Escape while a try plays.
#[tauri::command]
pub fn stop_practice(practice: State<'_, Mutex<Practice>>) {
    practice.lock().unwrap().stop();
}

// A setting tile changed on the Play tab.
#[tauri::command]
pub fn practice_settings(app: AppHandle, practice: State<'_, Mutex<Practice>>, settings: Value) -> Result<Value, String> {
    let mut practice = practice.lock().unwrap();
    if practice.is_running() {
        return Err(String::from("a try is playing: stop it first"));
    }
    let Some(piece) = &practice.piece else {
        return Err(String::from("no piece is set"));
    };
    let changed = settings_with(&practice.settings, &settings, piece)?;
    if changed.tempo != practice.settings.tempo || changed.ladder != practice.settings.ladder {
        practice.tempo_now = changed.tempo;
        practice.clean_in_row = 0;
    }
    let mut lines = Vec::new();
    if let Some(changes) = settings.as_object() {
        for (name, value) in changes {
            lines.push(format!("{name} {value}"));
        }
    }
    practice.tell_agent(format!("the player changed practice settings: {}", lines.join(", ")));
    practice.settings = changed;
    let view = practice.view();
    let _ = app.emit("practice", &view);
    Ok(view)
}

// The Play tab's Close: practice ends, and the readout comes back.
#[tauri::command]
pub fn close_practice(app: AppHandle, practice: State<'_, Mutex<Practice>>) -> Value {
    let mut practice = practice.lock().unwrap();
    practice.stop();
    practice.piece = None;
    practice.tries.clear();
    practice.tell_agent(String::from("the player closed practice"));
    let view = practice.view();
    let _ = app.emit("practice", &view);
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    // The chord keys' lowest root, so tests read plainly.
    const C3: u8 = crate::harmony::LOWEST_ROOT;

    fn simple_piece() -> Value {
        json!({
            "name": "Steps",
            "parts": [
                {"name": "Right hand", "hand": "right", "notes": [
                    {"note": "C4", "start": 0, "hold": 1},
                    {"note": "E4", "start": "1/2", "hold": "1/2"},
                    {"notes": ["G4", "C5"], "start": 2, "hold": 2},
                ]},
                {"name": "Left hand", "hand": "left", "notes": [
                    {"chord": "C", "start": 0, "hold": 4},
                    {"chord": "Am", "start": 4, "hold": 4, "octave": -1},
                ]},
                {"name": "Bass", "hand": "jello", "notes": [
                    {"note": "C2", "start": 0, "hold": 8},
                ]},
            ],
            "sections": [{"name": "A", "from_bar": 1, "to_bar": 1}, {"name": "B", "from_bar": 2, "to_bar": 2}],
        })
    }

    #[test]
    fn beats_take_numbers_and_fractions() {
        assert_eq!(beats_of(&json!(2.5), "start"), Ok(2.5));
        assert_eq!(beats_of(&json!("1/4"), "start"), Ok(0.25));
        let triplet = beats_of(&json!("2 1/3"), "start").unwrap();
        assert!((triplet - 2.333_333).abs() < 0.001);
        assert!(beats_of(&json!("1/0"), "start").is_err());
        assert!(beats_of(&json!("two"), "start").is_err());
        assert!(beats_of(&json!(-1), "start").is_err());
        assert!(beats_of(&json!("2 0.5"), "start").is_err());
    }

    #[test]
    fn a_piece_carries_its_octave_moves_in_time_order() {
        let piece = piece_of(&simple_piece()).unwrap();
        assert_eq!(piece.bars, 2);
        let left = &piece.parts[1];
        assert_eq!(left.notes[0].pitches, vec![C3, C3 + 4, C3 + 7]);
        // Am at octave -1: its root A2.
        assert_eq!(left.notes[1].pitches, vec![45, 48, 52]);
        assert_eq!(left.notes[1].octave, -1);
        assert_eq!(piece.parts[0].notes[2].label, "G4 C5");
        // Bass is Jello's, so any note goes.
        assert_eq!(piece.parts[2].notes[0].pitches, vec![36]);
    }

    #[test]
    fn notes_off_the_keys_are_refused_with_the_move_they_need() {
        let mut fields = simple_piece();
        fields["parts"][0]["notes"][0]["note"] = json!("C6");
        let why = piece_of(&fields).unwrap_err();
        assert!(why.contains("C6"), "{why}");
        assert!(why.contains("move the octave"), "{why}");
        fields["parts"][0]["notes"][0]["octave"] = json!(1);
        // C6 is on the keys an octave up, and so is everything after it.
        fields["parts"][0]["notes"][1]["note"] = json!("E5");
        fields["parts"][0]["notes"][2]["notes"] = json!(["G5", "C6"]);
        assert!(piece_of(&fields).is_ok());
    }

    #[test]
    fn wrong_pieces_say_what_is_wrong() {
        let mut fields = simple_piece();
        fields["parts"][1]["notes"][0]["chord"] = json!("Cmaj7");
        assert!(piece_of(&fields).unwrap_err().contains("\"notes\""));
        let mut fields = simple_piece();
        fields["parts"][0]["hand"] = json!("both");
        assert!(piece_of(&fields).unwrap_err().contains("left, right or jello"));
        let mut fields = simple_piece();
        fields["parts"][0]["notes"][0]["start"] = json!(300);
        assert!(piece_of(&fields).unwrap_err().contains("64 bars"));
        let mut fields = simple_piece();
        fields["sections"][1]["to_bar"] = json!(3);
        assert!(piece_of(&fields).is_err());
    }

    #[test]
    fn a_plan_splits_melody_notes_into_presses_and_swings_off_beats() {
        let piece = piece_of(&simple_piece()).unwrap();
        let mut settings = Settings::starting(120);
        settings.swing = 66;
        let plan = plan(&piece, &settings, 120);
        assert_eq!(plan.ms_per_beat, 500.0);
        assert_eq!(plan.length_ms, 4000.0);
        // C4, E4, G4, C5 and two chords.
        assert_eq!(plan.expected.len(), 6);
        let e4 = plan.expected.iter().find(|note| note.pitches == vec![64]).unwrap();
        assert_eq!(e4.at_ms, 330.0);
        assert_eq!(e4.section.as_deref(), Some("A"));
        assert_eq!(plan.jello, vec![Sounding { pitch: 36, at_ms: 0.0, end_ms: 4000.0 }]);
    }

    #[test]
    fn hands_separate_hand_the_other_part_to_jello() {
        let piece = piece_of(&simple_piece()).unwrap();
        let mut settings = Settings::starting(60);
        settings.hands = String::from("right");
        let plan = plan(&piece, &settings, 60);
        assert!(plan.expected.iter().all(|note| note.hand == Hand::Right));
        // The bass and both chords' notes.
        assert_eq!(plan.jello.len(), 7);

        let listen = listen_plan(&piece, &settings, 60);
        assert!(listen.expected.is_empty());
        assert_eq!(listen.jello.len(), 4);
    }

    #[test]
    fn a_section_plays_on_its_own_from_its_first_beat() {
        let piece = piece_of(&simple_piece()).unwrap();
        let mut settings = Settings::starting(60);
        settings = settings_with(&settings, &json!({"section": "B"}), &piece).unwrap();
        let plan = plan(&piece, &settings, 60);
        assert_eq!(plan.from_beat, 4.0);
        assert_eq!(plan.expected.len(), 1);
        assert_eq!(plan.expected[0].at_ms, 0.0);
        assert_eq!(lane_of(&piece, &settings, 60)["beats"], 4.0);
    }

    #[test]
    fn settings_are_checked() {
        let piece = piece_of(&simple_piece()).unwrap();
        let settings = Settings::starting(90);
        assert!(settings_with(&settings, &json!({"tempo": 10}), &piece).is_err());
        assert!(settings_with(&settings, &json!({"level": "wizard"}), &piece).is_err());
        assert!(settings_with(&settings, &json!({"section": "C"}), &piece).is_err());
        assert!(settings_with(&settings, &json!({"volume": 3}), &piece).is_err());
        let ladder = json!({"ladder": {"step": 5, "clean": 2, "top": 80}});
        assert!(settings_with(&settings, &ladder, &piece).unwrap_err().contains("top"));
        let changed = settings_with(&settings, &json!({"level": "master", "swing": 60}), &piece).unwrap();
        assert_eq!(changed.window_ms(), 40.0);
        assert_eq!(changed.swing, 60);
    }

    #[test]
    fn the_ladder_steps_up_after_clean_tries_in_a_row() {
        let piece = piece_of(&simple_piece()).unwrap();
        let mut practice = Practice::default();
        practice.settings = settings_with(
            &Settings::starting(60),
            &json!({"ladder": {"step": 10, "clean": 2, "top": 75}}),
            &piece,
        )
        .unwrap();
        practice.tempo_now = 60;
        assert!(!climb(&mut practice, true));
        assert!(!climb(&mut practice, false));
        assert!(!climb(&mut practice, true));
        assert_eq!(practice.tempo_now, 60);
        assert!(!climb(&mut practice, true));
        assert_eq!(practice.tempo_now, 70);
        assert!(!climb(&mut practice, true));
        assert!(!climb(&mut practice, true));
        assert_eq!(practice.tempo_now, 75);
        assert!(!climb(&mut practice, true));
        assert!(climb(&mut practice, true));
    }

    #[test]
    fn keys_are_found_for_notes_and_chords() {
        let mut key_map = KeyMap::new();
        assert_eq!(key_for(&key_map, &[60]).as_deref(), Some("KeyM"));
        assert!(key_for(&key_map, &[C3, C3 + 4, C3 + 7]).is_some());
        assert_eq!(key_for(&key_map, &[90]), None);
        key_map.shift_octave(1);
        assert_eq!(key_for(&key_map, &[72]).as_deref(), Some("KeyM"));
    }

    #[test]
    fn improvising_needs_a_part_for_jello() {
        let mut fields = simple_piece();
        fields["parts"].as_array_mut().unwrap().pop();
        let piece = piece_of(&fields).unwrap();
        let settings = Settings::starting(90);
        assert!(settings_with(&settings, &json!({"mode": "improvise"}), &piece).is_err());
        assert!(settings_with(&settings, &json!({"mode": "improvise", "hands": "left"}), &piece).is_ok());
    }
}
