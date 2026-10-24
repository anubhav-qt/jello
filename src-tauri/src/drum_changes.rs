// The agent's view of the song's drums, and its changes to them
// (docs/design.md#sampler), read as strictly as its sound changes
// (src/sound_changes.rs): a pad, a value or an instrument Jello does not
// know is refused, saying what is allowed, and then nothing changes.
//
//   {"kit": "AVL Black Pearl", "pads": [{"pad": "snare", "level": 80}]}

use serde_json::{Map, Value, json};

use crate::commands::instrument_view_of;
use crate::compose::Composer;
use crate::drums::{DRUM_MOST_GROUP, DRUM_MOST_HALF_STEPS, Drums, PAD_NAMES, pad_named};
use crate::engine::{Engine, FULL_PERCENT};
use crate::sound_changes::{count_within, instrument_named, signed_within};

// `drums` as the agent reads them, with whether the kit is ready on
// `engine`.
pub fn json_of(drums: &Drums, engine: &Engine) -> Value {
    let (kit, playing) = engine.drum_kit();
    let mut pads = Vec::new();
    for (pad, settings) in drums.pads.iter().enumerate() {
        pads.push(json!({
            "pad": PAD_NAMES[pad],
            "level": settings.level,
            "half_steps": settings.half_steps,
            "pan": settings.pan,
            "group": settings.group,
        }));
    }
    let kit_view = instrument_view_of(kit, playing);
    json!({
        "kit": drums.kit,
        "kit_state": serde_json::to_value(kit_view).unwrap_or(Value::Null),
        "pads": pads,
    })
}

// `drums` with `changes` made to them, or why they can't be.
pub fn changed(drums: &Drums, changes: &Value) -> Result<Drums, String> {
    let Some(changes) = changes.as_object() else {
        return Err(String::from("the changes are an object, like {\"pads\": [{\"pad\": \"snare\", \"level\": 80}]}"));
    };
    let mut drums = drums.clone();
    for (name, value) in changes {
        match name.as_str() {
            "kit" => drums.kit = instrument_named(value, "kit")?,
            "pads" => change_pads(&mut drums, value)?,
            _ => return Err(format!("the drums have no part called {name}: they have kit and pads")),
        }
    }
    Ok(drums)
}

fn change_pads(drums: &mut Drums, value: &Value) -> Result<(), String> {
    let all = PAD_NAMES.join(", ");
    let Some(items) = value.as_array() else {
        return Err(String::from("pads is a list, like [{\"pad\": \"snare\", \"level\": 80}]"));
    };
    for item in items {
        let Some(fields) = item.as_object() else {
            return Err(format!("each of pads is an object, not {item}"));
        };
        let Some(name) = fields.get("pad").and_then(Value::as_str) else {
            return Err(format!("each of pads names its pad, one of {all}"));
        };
        let Some(pad) = pad_named(name) else {
            return Err(format!("pad is one of {all}, not {name}"));
        };
        change_pad(drums, pad, fields)?;
    }
    Ok(())
}

fn change_pad(drums: &mut Drums, pad: usize, fields: &Map<String, Value>) -> Result<(), String> {
    let settings = &mut drums.pads[pad];
    let place = PAD_NAMES[pad];
    for (name, value) in fields {
        let named = format!("{place} {name}");
        match name.as_str() {
            "pad" => {}
            "level" => settings.level = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            "half_steps" => settings.half_steps = signed_within(value, &named, DRUM_MOST_HALF_STEPS, "")?,
            "pan" => settings.pan = signed_within(value, &named, 100, "%")?,
            "group" => settings.group = count_within(value, &named, 0, DRUM_MOST_GROUP, "")?,
            _ => return Err(format!("{place} has no value called {name}: it has level, half_steps, pan and group")),
        }
    }
    Ok(())
}

// One line for each change between `before` and `after`, like
// "Snare level 100 -> 80%", for the agent's panel.
pub fn change_lines(before: &Drums, after: &Drums) -> Vec<String> {
    let mut lines = Vec::new();
    if before.kit != after.kit {
        lines.push(format!("Drum kit {} -> {}", kit_text(&before.kit), kit_text(&after.kit)));
    }
    for pad in 0..PAD_NAMES.len() {
        let was = before.pads[pad];
        let is = after.pads[pad];
        let title = title_of(PAD_NAMES[pad]);
        if was.level != is.level {
            lines.push(format!("{title} level {} -> {}%", was.level, is.level));
        }
        if was.half_steps != is.half_steps {
            lines.push(format!("{title} half steps {} -> {}", was.half_steps, is.half_steps));
        }
        if was.pan != is.pan {
            lines.push(format!("{title} pan {} -> {}%", was.pan, is.pan));
        }
        if was.group != is.group {
            lines.push(format!("{title} group {} -> {}", was.group, is.group));
        }
    }
    lines
}

fn kit_text(kit: &Option<String>) -> String {
    match kit {
        Some(name) => name.clone(),
        None => String::from("none"),
    }
}

// "closed hat" as "Closed hat".
fn title_of(name: &str) -> String {
    let mut letters = name.chars();
    let Some(first) = letters.next() else {
        return String::new();
    };
    let rest: String = letters.collect();
    format!("{}{rest}", first.to_uppercase())
}

// Takes the changes from `before` to `after` into the song on `engine`,
// each through the same call the Drums page makes.
pub fn take_changes(composer: &mut Composer, engine: &mut Engine, before: &Drums, after: &Drums) {
    if after.kit != before.kit {
        composer.set_drum_kit(engine, after.kit.as_deref());
    }
    for pad in 0..PAD_NAMES.len() {
        if after.pads[pad] != before.pads[pad] {
            composer.set_drum_pad(engine, pad, after.pads[pad]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::instruments::testing::{KIT, use_test_folder};

    #[test]
    fn pads_change_by_name_and_say_what_changed() {
        use_test_folder();
        let before = Drums::new();
        let changes = json!({"kit": KIT, "pads": [{"pad": "closed_hat", "level": 60, "group": 0}, {"pad": "kick", "pan": -20}]});
        let after = changed(&before, &changes).unwrap();
        assert_eq!(after.kit.as_deref(), Some(KIT));
        assert_eq!(after.pads[4].level, 60);
        assert_eq!(after.pads[4].group, 0);
        assert_eq!(after.pads[0].pan, -20);
        let lines = change_lines(&before, &after);
        assert_eq!(
            lines,
            [
                format!("Drum kit none -> {KIT}"),
                String::from("Kick pan 0 -> -20%"),
                String::from("Closed hat level 100 -> 60%"),
                String::from("Closed hat group 1 -> 0"),
            ]
        );
    }

    #[test]
    fn what_jello_does_not_know_is_refused_with_what_there_is() {
        use_test_folder();
        let before = Drums::new();
        let error = changed(&before, &json!({"pads": [{"pad": "cowbell", "level": 50}]})).unwrap_err();
        assert!(error.starts_with("pad is one of kick, snare, rim"), "{error}");
        let error = changed(&before, &json!({"pads": [{"pad": "snare", "level": 150}]})).unwrap_err();
        assert_eq!(error, "snare level is 0 to 100%, not 150");
        let error = changed(&before, &json!({"pads": [{"pad": "snare", "half_steps": 13}]})).unwrap_err();
        assert_eq!(error, "snare half_steps is -12 to 12, not 13");
        let error = changed(&before, &json!({"pads": [{"pad": "snare", "swing": 1}]})).unwrap_err();
        assert_eq!(error, "snare has no value called swing: it has level, half_steps, pan and group");
        let error = changed(&before, &json!({"kit": "Tin cans"})).unwrap_err();
        assert!(error.starts_with("kit is one of "), "{error}");
        assert!(error.contains(KIT), "{error}");
        let error = changed(&before, &json!({"reverb": 1})).unwrap_err();
        assert_eq!(error, "the drums have no part called reverb: they have kit and pads");

        // Null takes the kit away.
        let mut with_kit = before.clone();
        with_kit.kit = Some(String::from(KIT));
        let after = changed(&with_kit, &json!({"kit": null})).unwrap();
        assert_eq!(after.kit, None);
    }
}
