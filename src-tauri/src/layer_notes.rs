// jello_notes (docs/design.md#the-composer-to-release-grade): the agent
// writes a loop layer's notes directly, at exact times and at any pitch,
// not only the notes the keys reach. The agent's melody options use it too.
//
// A layer's notes are in beats from its start:
//
//   {"bars": 2, "notes": [
//       {"note": "E4", "start": 0, "hold": 0.5},
//       {"chord": "Am", "start": 4, "hold": 4}]}
//
// With no loop yet, the layer becomes the loop. With "layer", that layer's
// notes are replaced; without it, a new layer is added on top.

use serde_json::{Map, Value};

use crate::engine::{BEATS_PER_BAR, Engine, LooperState, LoopLayer, LoopNote, MAX_LAYERS, FULL_VOLUME};
use crate::harmony::chord_of_name;
use crate::key_map::LAPTOP_VELOCITY;
use crate::song_file::{note_name, note_of_name, samples_of};

// The most bars a layer lasts, and the most notes it holds.
pub const LAYER_MOST_BARS: u32 = 64;
pub const LAYER_MOST_NOTES: usize = 2048;

// A note to write: its pitch, from 0 to 127, and when it starts and ends,
// in beats from the layer's start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrittenNote {
    pub note: u8,
    pub start: f64,
    pub end: f64,
}

// The notes `arguments` list under "notes", each a note like "E4" or a
// chord like "Am" (its three notes, from C3 up), with a start and a hold in
// beats.
pub fn notes_of(arguments: &Value) -> Result<Vec<WrittenNote>, String> {
    let Some(listed) = arguments.get("notes").and_then(Value::as_array) else {
        return Err(String::from("notes is needed: a list like [{\"note\": \"E4\", \"start\": 0, \"hold\": 1}]"));
    };
    if listed.is_empty() {
        return Err(String::from("notes is empty, so there is nothing to write"));
    }
    let mut notes = Vec::new();
    for (index, listed_note) in listed.iter().enumerate() {
        let place = format!("note {}", index + 1);
        let Some(fields) = listed_note.as_object() else {
            return Err(format!("{place} is an object, like {{\"note\": \"E4\", \"start\": 0, \"hold\": 1}}"));
        };
        let start = beats_in(fields, "start", &place)?;
        let hold = beats_in(fields, "hold", &place)?;
        if hold <= 0.0 {
            return Err(format!("{place}'s hold is more than 0, not {hold}"));
        }
        for pitch in pitches_of(fields, &place)? {
            notes.push(WrittenNote {
                note: pitch,
                start,
                end: start + hold,
            });
        }
    }
    if notes.len() > LAYER_MOST_NOTES {
        return Err(format!("a layer holds at most {LAYER_MOST_NOTES} notes, not {}", notes.len()));
    }
    Ok(notes)
}

// The pitches a note or chord field names.
fn pitches_of(fields: &Map<String, Value>, place: &str) -> Result<Vec<u8>, String> {
    if let Some(name) = fields.get("note") {
        let name = name.as_str().unwrap_or_default();
        let Some(note) = note_of_name(name) else {
            return Err(format!("{place}: {name} is not a note, like \"C4\" or \"F#3\""));
        };
        return Ok(vec![note]);
    }
    if let Some(name) = fields.get("chord") {
        let name = name.as_str().unwrap_or_default();
        let Some(chord) = chord_of_name(name) else {
            return Err(format!("{place}: {name} is not a chord, like \"Am\" or \"Bb\""));
        };
        return Ok(chord.notes().to_vec());
    }
    Err(format!("{place} names a note or a chord"))
}

fn beats_in(fields: &Map<String, Value>, name: &str, place: &str) -> Result<f64, String> {
    let Some(beats) = fields.get(name).and_then(Value::as_f64) else {
        return Err(format!("{place} needs its {name}, in beats"));
    };
    if beats < 0.0 {
        return Err(format!("{place}'s {name} is 0 or more, not {beats}"));
    }
    Ok(beats)
}

// The bars `notes` need, from 1: enough to hold the last one's end.
pub fn bars_for(notes: &[WrittenNote]) -> u32 {
    let mut last_end: f64 = 0.0;
    for note in notes {
        last_end = last_end.max(note.end);
    }
    let bars = (last_end / f64::from(BEATS_PER_BAR)).ceil() as u32;
    bars.max(1)
}

// What writing did, for the panel: which layer, and its line.
#[derive(Debug, PartialEq)]
pub struct Written {
    pub layer: u32,
    pub line: String,
}

// Writes `notes` into `engine`'s loop: replacing layer `layer`'s notes
// (from 1), or as a new layer on top with None. The layer lasts `bars`, or
// with None the bars it had, or for a new layer the bars its notes need.
// With no loop yet, the layer becomes the loop. A loop that was playing
// keeps playing. Refused while the looper records, changing nothing.
pub fn write_layer(
    engine: &mut Engine,
    layer: Option<u32>,
    bars: Option<u32>,
    volume: Option<u32>,
    notes: &[WrittenNote],
) -> Result<Written, String> {
    let status = engine.looper_status();
    let recording = matches!(
        status.state,
        LooperState::CountingIn | LooperState::Recording | LooperState::Layering
    );
    if recording {
        return Err(String::from("the looper is recording: wait until it has finished"));
    }
    let was_playing = status.state == LooperState::Looping;

    let (common, mut layers) = match engine.loop_to_keep() {
        Some(kept) => kept,
        None => (0, Vec::new()),
    };
    if let Some(number) = layer {
        if number == 0 || number as usize > layers.len() {
            return Err(format!("there is no layer {number}: the loop has {}", layers.len()));
        }
    } else if layers.len() >= MAX_LAYERS {
        return Err(format!("a loop holds at most {MAX_LAYERS} layers: replace one with layer"));
    }

    let mut layer_bars = bars_for(notes);
    if let Some(number) = layer {
        let had = layers[number as usize - 1].bars;
        if had == 0 {
            layer_bars = common;
        } else {
            layer_bars = had;
        }
    }
    if let Some(asked) = bars {
        layer_bars = asked;
    }
    if !(1..=LAYER_MOST_BARS).contains(&layer_bars) {
        return Err(format!("a layer lasts 1 to {LAYER_MOST_BARS} bars, not {layer_bars}"));
    }
    let layer_beats = f64::from(layer_bars * BEATS_PER_BAR);
    for note in notes {
        if note.end > layer_beats + 1e-9 {
            return Err(format!(
                "{} ends on beat {}, after the layer's {layer_bars} bars ({layer_beats} beats)",
                note_name(note.note),
                note.end
            ));
        }
    }
    if let Some(percent) = volume {
        if percent > FULL_VOLUME {
            return Err(format!("a layer's volume is 0 to {FULL_VOLUME} percent, not {percent}"));
        }
    }

    let tempo = engine.tempo();
    let sample_rate = engine.sample_rate();
    let mut loop_notes = Vec::new();
    for note in notes {
        loop_notes.push(LoopNote {
            note: note.note,
            start: samples_of(note.start, tempo, sample_rate),
            end: Some(samples_of(note.end, tempo, sample_rate)),
        });
    }
    loop_notes.sort_by_key(|note| note.start);

    let mut loop_bars = common;
    if layers.is_empty() {
        loop_bars = layer_bars;
    }
    // A layer as long as the loop inherits its length.
    let mut kept_bars = layer_bars;
    if layer_bars == loop_bars {
        kept_bars = 0;
    }

    let number;
    if let Some(asked) = layer {
        let replaced = &mut layers[asked as usize - 1];
        replaced.notes = loop_notes;
        replaced.bars = kept_bars;
        if let Some(percent) = volume {
            replaced.volume = percent;
        }
        number = asked;
    } else {
        let mut percent = FULL_VOLUME;
        if let Some(asked) = volume {
            percent = asked;
        }
        layers.push(LoopLayer {
            bars: kept_bars,
            notes: loop_notes,
            volume: percent,
            playing: true,
            recording: false,
        });
        number = layers.len() as u32;
    }
    // Layers that inherit the loop's length keep it, so another layer's new
    // length never stretches them.
    for kept in &mut layers {
        if kept.bars == 0 && loop_bars != 0 {
            kept.bars = loop_bars;
        }
        if kept.bars == loop_bars {
            kept.bars = 0;
        }
    }

    engine.open_loop(tempo, loop_bars, &layers, LAPTOP_VELOCITY)?;
    if was_playing {
        engine.play_loop();
    }

    let mut line = format!("Layer {number}: wrote {} notes over {layer_bars} bars", notes.len());
    if layer.is_none() {
        line = format!("New layer {number}: {} notes over {layer_bars} bars", notes.len());
    }
    Ok(Written { layer: number, line })
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn engine() -> Engine {
        Engine::new().unwrap()
    }

    #[test]
    fn notes_and_chords_are_read_in_beats() {
        let notes = notes_of(&json!({"notes": [
            {"note": "E4", "start": 0, "hold": 0.5},
            {"chord": "Am", "start": 4, "hold": 4},
        ]}))
        .unwrap();
        assert_eq!(notes.len(), 4);
        assert_eq!(notes[0], WrittenNote { note: 64, start: 0.0, end: 0.5 });
        // A minor from A3: A3 C4 E4.
        assert_eq!(notes[1].note, 57);
        assert_eq!(notes[2].note, 60);
        assert_eq!(notes[3].note, 64);
        assert_eq!(notes[3].end, 8.0);
        assert_eq!(bars_for(&notes), 2);
    }

    #[test]
    fn wrong_notes_are_refused_with_why() {
        let cases = [
            (json!({}), "notes is needed"),
            (json!({"notes": []}), "notes is empty"),
            (json!({"notes": [{"note": "H4", "start": 0, "hold": 1}]}), "H4 is not a note"),
            (json!({"notes": [{"chord": "Zm", "start": 0, "hold": 1}]}), "Zm is not a chord"),
            (json!({"notes": [{"note": "C4", "start": -1, "hold": 1}]}), "start is 0 or more"),
            (json!({"notes": [{"note": "C4", "start": 0, "hold": 0}]}), "hold is more than 0"),
            (json!({"notes": [{"start": 0, "hold": 1}]}), "names a note or a chord"),
        ];
        for (arguments, why) in cases {
            let error = notes_of(&arguments).unwrap_err();
            assert!(error.contains(why), "{error}");
        }
        let many: Vec<Value> = (0..LAYER_MOST_NOTES + 1)
            .map(|index| json!({"note": "C4", "start": index as f64 * 0.1, "hold": 0.1}))
            .collect();
        let error = notes_of(&json!({ "notes": many })).unwrap_err();
        assert!(error.contains("at most 2048 notes"), "{error}");
    }

    #[test]
    fn the_first_layer_becomes_the_loop_and_a_second_goes_on_top() {
        let mut engine = engine();
        let melody = notes_of(&json!({"notes": [
            {"note": "C4", "start": 0, "hold": 1},
            {"note": "G4", "start": 7, "hold": 1},
        ]}))
        .unwrap();
        let written = write_layer(&mut engine, None, None, None, &melody).unwrap();
        assert_eq!(written.layer, 1);
        assert_eq!(written.line, "New layer 1: 2 notes over 2 bars");
        let (bars, layers) = engine.loop_to_keep().unwrap();
        assert_eq!(bars, 2);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].notes.len(), 2);
        assert_eq!(layers[0].notes[1].note, 67);

        // A bass of 4 bars on top: the 2-bar layer keeps its 2 bars.
        let bass = notes_of(&json!({"notes": [{"note": "C2", "start": 12, "hold": 4}]})).unwrap();
        let written = write_layer(&mut engine, None, Some(4), Some(60), &bass).unwrap();
        assert_eq!(written.line, "New layer 2: 1 notes over 4 bars");
        let (_, layers) = engine.loop_to_keep().unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[1].volume, 60);
        assert_eq!(layers[1].notes[0].note, 36);
    }

    #[test]
    fn a_layer_named_is_replaced_and_keeps_its_bars() {
        let mut engine = engine();
        let first = notes_of(&json!({"notes": [{"note": "C4", "start": 0, "hold": 8}]})).unwrap();
        write_layer(&mut engine, None, None, None, &first).unwrap();
        let second = notes_of(&json!({"notes": [{"note": "D4", "start": 1, "hold": 1}]})).unwrap();
        let written = write_layer(&mut engine, Some(1), None, None, &second).unwrap();
        assert_eq!(written.line, "Layer 1: wrote 1 notes over 2 bars");
        let (bars, layers) = engine.loop_to_keep().unwrap();
        assert_eq!(bars, 2);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].notes[0].note, 62);
    }

    #[test]
    fn writing_is_refused_with_why_and_changes_nothing() {
        let mut engine = engine();
        let notes = notes_of(&json!({"notes": [{"note": "C4", "start": 0, "hold": 9}]})).unwrap();
        let error = write_layer(&mut engine, None, Some(2), None, &notes).unwrap_err();
        assert!(error.contains("after the layer's 2 bars"), "{error}");
        let error = write_layer(&mut engine, Some(1), None, None, &notes).unwrap_err();
        assert!(error.contains("there is no layer 1"), "{error}");
        let error = write_layer(&mut engine, None, Some(65), None, &notes).unwrap_err();
        assert!(error.contains("1 to 64 bars"), "{error}");
        let error = write_layer(&mut engine, None, None, Some(101), &notes).unwrap_err();
        assert!(error.contains("0 to 100 percent"), "{error}");
        assert!(engine.loop_to_keep().is_none());

        let one = notes_of(&json!({"notes": [{"note": "C4", "start": 0, "hold": 1}]})).unwrap();
        for _ in 0..MAX_LAYERS {
            write_layer(&mut engine, None, None, None, &one).unwrap();
        }
        let error = write_layer(&mut engine, None, None, None, &one).unwrap_err();
        assert!(error.contains("at most 8 layers"), "{error}");
    }
}
