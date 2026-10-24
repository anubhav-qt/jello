// The agent's phrases (docs/design.md#the-agent-harness): timed key
// presses, played by Rust through the same steps as the player's keys. So
// the keys light, the looper records them, and Compose and the recent
// presses follow, just as for a person playing.
//
// A phrase is a list of presses, each with a start and a hold, in beats at
// the song's tempo or in milliseconds:
//
//   {"unit": "beats", "presses": [
//       {"chord": "Am", "start": 0, "hold": 2},
//       {"note": "E4", "start": 0, "hold": 0.5},
//       {"key": "ShiftLeft", "start": 1, "hold": 1}]}
//
// A press names a laptop key, a note or a chord. For a note or a chord,
// Jello finds the key that plays it at the current octave, or says there is
// none. A phrase lasts at most PHRASE_LONGEST_MS, and only one plays at a
// time. The player's keys keep working while it plays, and Stop ends it,
// letting go of everything it holds.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::commands::{press_key_on, release_key_on};
use crate::harmony::chord_of_name;
use crate::key_map::KeyMap;
use crate::song_file::{note_name, pitch_of_note_name};
use crate::tools::Studio;

// The longest a phrase can last, and the most presses it can hold.
const PHRASE_LONGEST_MS: f64 = 60_000.0;
const PHRASE_MOST_PRESSES: usize = 1000;

// How often a playing phrase looks for Stop while it waits.
const STOP_CHECK: Duration = Duration::from_millis(5);

// Whether a phrase is playing, and whether Stop was asked for.
#[derive(Default)]
pub struct Phrases {
    playing: AtomicBool,
    stop: AtomicBool,
}

impl Phrases {
    // True while a phrase plays.
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }

    // Ends the phrase playing, if one is.
    pub fn stop(&self) {
        if self.playing.load(Ordering::SeqCst) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }
}

// A key going down or coming up, `at_ms` after the phrase starts.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub at_ms: u64,
    pub key: String,
    pub down: bool,
}

// A phrase ready to play: its key events in the order they happen, how long
// it lasts, and each press in words, like "Am on KeyA at 0 for 2 beats".
#[derive(Debug, PartialEq)]
pub struct Phrase {
    pub events: Vec<KeyEvent>,
    pub length_ms: u64,
    pub presses: Vec<String>,
}

// How a phrase ended: how many presses it started, and whether Stop ended
// it early, after how long.
#[derive(Debug, PartialEq)]
pub struct Played {
    pub started: usize,
    pub stopped: bool,
    pub after_ms: u64,
}

// The phrase `arguments` describe, with notes and chords found on the keys
// of `key_map`, and beats at `tempo` beats per minute. Fails, saying why,
// for a press that can't be played.
pub fn phrase_of(arguments: &Value, key_map: &KeyMap, tempo: u32) -> Result<Phrase, String> {
    let unit = match arguments.get("unit") {
        Some(unit) => unit.as_str().unwrap_or_default(),
        None => "beats",
    };
    let ms_per_unit = match unit {
        "beats" => 60_000.0 / f64::from(tempo),
        "ms" => 1.0,
        _ => return Err(format!("unit is beats or ms, not {unit}")),
    };
    let Some(presses) = arguments.get("presses").and_then(Value::as_array) else {
        return Err(String::from("presses is needed: a list like [{\"note\": \"C4\", \"start\": 0, \"hold\": 1}]"));
    };
    if presses.is_empty() {
        return Err(String::from("presses is empty, so there is nothing to play"));
    }
    if presses.len() > PHRASE_MOST_PRESSES {
        return Err(format!("a phrase holds at most {PHRASE_MOST_PRESSES} presses, not {}", presses.len()));
    }

    let mut events = Vec::new();
    let mut words = Vec::new();
    let mut length_ms = 0;
    for (index, press) in presses.iter().enumerate() {
        let place = format!("press {}", index + 1);
        let Some(fields) = press.as_object() else {
            return Err(format!("{place} is an object, like {{\"note\": \"C4\", \"start\": 0, \"hold\": 1}}"));
        };
        let (key, what) = key_of_press(fields, key_map, &place)?;
        let start = time_in(fields, "start", &place, false)?;
        let hold = time_in(fields, "hold", &place, true)?;

        let down_ms = start * ms_per_unit;
        let up_ms = (start + hold) * ms_per_unit;
        if up_ms > PHRASE_LONGEST_MS {
            return Err(format!("{place} ends {:.1} s in: a phrase lasts at most 60 s", up_ms / 1000.0));
        }
        let down_ms = down_ms.round() as u64;
        let up_ms = up_ms.round() as u64;
        length_ms = length_ms.max(up_ms);

        events.push(KeyEvent {
            at_ms: down_ms,
            key: key.clone(),
            down: true,
        });
        events.push(KeyEvent {
            at_ms: up_ms,
            key: key.clone(),
            down: false,
        });
        let when = format!("at {} for {} {unit}", short(start), short(hold));
        if what == key {
            words.push(format!("{key} {when}"));
        } else {
            words.push(format!("{what} on {key} {when}"));
        }
    }

    // In time order. At the same moment a key comes up before any goes
    // down, so a key pressed again right as it is let go plays again.
    events.sort_by_key(|event| (event.at_ms, event.down));
    check_no_overlaps(&events)?;
    Ok(Phrase {
        events,
        length_ms,
        presses: words,
    })
}

// The laptop key a press plays, and what it plays in words, like "E4" or
// "Am", or the key's own name.
pub(crate) fn key_of_press(fields: &Map<String, Value>, key_map: &KeyMap, place: &str) -> Result<(String, String), String> {
    if let Some(key) = fields.get("key") {
        let Some(key) = key.as_str() else {
            return Err(format!("{place}'s key is a key's name, like \"KeyC\""));
        };
        if !key_map.knows(key) {
            return Err(format!("{place}: {key} does nothing in Jello"));
        }
        return Ok((String::from(key), String::from(key)));
    }

    if let Some(name) = fields.get("note") {
        let name = name.as_str().unwrap_or_default();
        let Some(note) = pitch_of_note_name(name) else {
            return Err(format!("{place}: {name} is not a note, like \"C4\" or \"F#3\""));
        };
        for (key, played) in key_map.note_keys() {
            if played == note {
                return Ok((key, note_name(note)));
            }
        }
        return Err(format!("{place}: no melody key plays {name} at this octave"));
    }

    if let Some(name) = fields.get("chord") {
        let name = name.as_str().unwrap_or_default();
        let Some(chord) = chord_of_name(name) else {
            return Err(format!("{place}: {name} is not a chord, like \"Am\" or \"Bb\""));
        };
        // The chord keys play every root once, at the current octave.
        for (key, played) in key_map.chord_keys() {
            let same_root = played.root % 12 == chord.root % 12;
            if same_root && played.quality == chord.quality {
                return Ok((key, String::from(name)));
            }
        }
        return Err(format!("{place}: no chord key plays {name}"));
    }

    Err(format!("{place} names a key, a note or a chord"))
}

// Field `name` of a press: a number of beats or ms, from 0, or above 0 for
// `above_zero`.
fn time_in(fields: &Map<String, Value>, name: &str, place: &str, above_zero: bool) -> Result<f64, String> {
    let Some(time) = fields.get(name).and_then(Value::as_f64) else {
        return Err(format!("{place} needs its {name}, as a number"));
    };
    if above_zero {
        if time <= 0.0 {
            return Err(format!("{place}'s {name} is more than 0, not {time}"));
        }
    } else if time < 0.0 {
        return Err(format!("{place}'s {name} is 0 or more, not {time}"));
    }
    Ok(time)
}

// Refuses a key pressed again while the phrase still holds it, since the
// second press would do nothing.
fn check_no_overlaps(events: &[KeyEvent]) -> Result<(), String> {
    let mut held: Vec<&str> = Vec::new();
    for event in events {
        if !event.down {
            held.retain(|key| *key != event.key);
            continue;
        }
        if held.contains(&event.key.as_str()) {
            return Err(format!("{} is pressed again {} ms in, while the phrase still holds it", event.key, event.at_ms));
        }
        held.push(&event.key);
    }
    Ok(())
}

// `number` without a trailing ".0".
fn short(number: f64) -> String {
    format!("{number}")
}

// Plays `phrase` on `studio`, waiting for each event's time, and returns
// how it ended. It refuses to start while another phrase plays. Each event
// locks what it needs only for its press or release, so the player's keys
// keep working in between. If Stop ends it, every key it still holds is let
// go at once.
pub fn play(studio: &Studio, phrase: &Phrase) -> Result<Played, String> {
    let phrases = studio.phrases;
    let already = phrases.playing.swap(true, Ordering::SeqCst);
    if already {
        return Err(String::from("a phrase is playing already: wait for it to end, or stop it"));
    }
    phrases.stop.store(false, Ordering::SeqCst);
    studio.screen.show_phrase(phrase);

    let started_at = Instant::now();
    let mut held: Vec<String> = Vec::new();
    let mut started = 0;
    let mut stopped = false;
    for event in &phrase.events {
        let due = started_at + Duration::from_millis(event.at_ms);
        if !wait_until(due, &phrases.stop) {
            stopped = true;
            break;
        }
        if event.down {
            press(studio, &event.key);
            held.push(event.key.clone());
            started += 1;
        } else {
            release(studio, &event.key);
            held.retain(|key| *key != event.key);
        }
    }

    for key in &held {
        release(studio, key);
    }
    let after_ms = started_at.elapsed().as_millis() as u64;
    phrases.playing.store(false, Ordering::SeqCst);
    Ok(Played {
        started,
        stopped,
        after_ms,
    })
}

// Waits until `due`, looking for Stop every STOP_CHECK. Returns false if
// Stop came first.
fn wait_until(due: Instant, stop: &AtomicBool) -> bool {
    loop {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        let now = Instant::now();
        if now >= due {
            return true;
        }
        let left = due - now;
        thread::sleep(left.min(STOP_CHECK));
    }
}

fn press(studio: &Studio, key: &str) {
    let now = Instant::now();
    let mut key_map = studio.key_map.lock().unwrap();
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    press_key_on(&mut key_map, &mut composer, &mut engine, key, now);
    drop(engine);
    drop(composer);
    drop(key_map);
    studio.screen.show_key(key, true);
}

fn release(studio: &Studio, key: &str) {
    let now = Instant::now();
    let mut key_map = studio.key_map.lock().unwrap();
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    release_key_on(&mut key_map, &mut composer, &mut engine, key, now);
    drop(engine);
    drop(composer);
    drop(key_map);
    studio.screen.show_key(key, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn phrase(arguments: Value) -> Result<Phrase, String> {
        phrase_of(&arguments, &KeyMap::new(), 120)
    }

    fn event(at_ms: u64, key: &str, down: bool) -> KeyEvent {
        KeyEvent {
            at_ms,
            key: String::from(key),
            down,
        }
    }

    #[test]
    fn beats_follow_the_tempo() {
        // At 120 beats per minute a beat lasts 500 ms.
        let played = phrase(json!({"presses": [{"key": "KeyC", "start": 1, "hold": 0.5}]})).unwrap();
        assert_eq!(played.events, vec![event(500, "KeyC", true), event(750, "KeyC", false)]);
        assert_eq!(played.length_ms, 750);
        assert_eq!(played.presses, vec![String::from("KeyC at 1 for 0.5 beats")]);

        let played = phrase(json!({"unit": "ms", "presses": [{"key": "KeyC", "start": 10, "hold": 20}]})).unwrap();
        assert_eq!(played.events, vec![event(10, "KeyC", true), event(30, "KeyC", false)]);
    }

    #[test]
    fn notes_and_chords_are_found_on_their_keys() {
        // KeyM plays middle C, and KeyC the C chord.
        let played = phrase(json!({"presses": [
            {"note": "C4", "start": 0, "hold": 1},
            {"chord": "C", "start": 0, "hold": 1},
        ]}))
        .unwrap();
        assert_eq!(played.presses[0], "C4 on KeyM at 0 for 1 beats");
        assert_eq!(played.presses[1], "C on KeyC at 0 for 1 beats");

        let error = phrase(json!({"presses": [{"note": "C9", "start": 0, "hold": 1}]})).unwrap_err();
        assert_eq!(error, "press 1: no melody key plays C9 at this octave");
        let error = phrase(json!({"presses": [{"chord": "Cmaj7", "start": 0, "hold": 1}]})).unwrap_err();
        assert_eq!(error, "press 1: Cmaj7 is not a chord, like \"Am\" or \"Bb\"");
    }

    #[test]
    fn a_shifted_octave_moves_where_a_note_is() {
        let mut key_map = KeyMap::new();
        let arguments = json!({"presses": [{"note": "C4", "start": 0, "hold": 1}]});
        let before = phrase_of(&arguments, &key_map, 120).unwrap();
        key_map.shift_octave(-1);
        let after = phrase_of(&arguments, &key_map, 120).unwrap();
        assert_ne!(before.events[0].key, after.events[0].key);
    }

    #[test]
    fn a_phrase_lasts_at_most_a_minute() {
        // 121 beats at 120 bpm is 60.5 s.
        let error = phrase(json!({"presses": [{"key": "KeyC", "start": 120, "hold": 1}]})).unwrap_err();
        assert_eq!(error, "press 1 ends 60.5 s in: a phrase lasts at most 60 s");
    }

    #[test]
    fn presses_it_can_t_play_are_refused() {
        let error = phrase(json!({"presses": [{"key": "KeyQQ", "start": 0, "hold": 1}]})).unwrap_err();
        assert_eq!(error, "press 1: KeyQQ does nothing in Jello");
        let error = phrase(json!({"presses": [{"key": "KeyC", "start": 0, "hold": 0}]})).unwrap_err();
        assert_eq!(error, "press 1's hold is more than 0, not 0");
        let error = phrase(json!({"presses": [{"key": "KeyC", "start": -1, "hold": 1}]})).unwrap_err();
        assert_eq!(error, "press 1's start is 0 or more, not -1");
        let error = phrase(json!({"unit": "bars", "presses": []})).unwrap_err();
        assert_eq!(error, "unit is beats or ms, not bars");
    }

    #[test]
    fn a_key_held_can_t_be_pressed_again_but_can_right_after() {
        let overlapping = json!({"presses": [
            {"key": "KeyC", "start": 0, "hold": 2},
            {"key": "KeyC", "start": 1, "hold": 1},
        ]});
        let error = phrase(overlapping).unwrap_err();
        assert_eq!(error, "KeyC is pressed again 500 ms in, while the phrase still holds it");

        // Let go and pressed at the same moment, it comes up first.
        let repeated = json!({"presses": [
            {"key": "KeyC", "start": 1, "hold": 1},
            {"key": "KeyC", "start": 0, "hold": 1},
        ]});
        let played = phrase(repeated).unwrap();
        let downs: Vec<bool> = played.events.iter().map(|event| event.down).collect();
        assert_eq!(downs, vec![true, false, true, false]);
    }
}
