// jello_mix (docs/design.md#mix): the agent's one tool for the mix, like
// jello_sound for the sound. It places, moves and switches effects, sets
// their values by the words the song file uses, and sets pan, sends, buses
// and the master's limiter. Places are words, counted from 1: "track 2",
// "bus 1" or "master". Every answer is the whole mix, as jello_look's mix
// part reads it.

use serde_json::{Map, Value, json};

use crate::compose::Composer;
use crate::effects::{
    Sort, Written, is_in_range, kind_of_word, kind_words, values_of, what_it_takes, word_of_kind, written_of,
};
use crate::engine::{EFFECT_MOST_MOVE, EFFECT_SLOTS, EffectKind, FULL_VOLUME, Limiter, PAN_MOST};
use crate::mix::{Chain, EffectLink, Place, check_track_routing, send_effect_links};
use crate::modulation::{CONNECTION_COUNT, LFO_COUNT, Lfo, LfoSpeed, Source};
use crate::sound_changes::{change_lfo, lfo_json};
use crate::tools::{Studio, ToolAnswer, optional_text_argument, text_argument};

// The actions jello_mix takes.
pub const ACTIONS: [&str; 16] = [
    "look",
    "describe",
    "effect",
    "move",
    "on",
    "set",
    "pan",
    "send",
    "add_bus",
    "remove_bus",
    "name_bus",
    "bus_volume",
    "limiter",
    "lfo",
    "connect",
    "disconnect",
];

// The word that empties a slot.
const NONE_WORD: &str = "none";

// The tool as MCP's tools/list describes it.
pub fn description() -> Value {
    json!({
        "name": "jello_mix",
        "description": "Mixes the song: each track's pan, sends and 8 effect slots, up to 4 send \
            buses (shared effects, like one reverb for every track) and the master, whose 8 slots \
            come before its limiter. Places are \"track 2\", \"bus 1\" or \"master\"; tracks, \
            buses and slots count from 1. look: the whole mix. describe: kind's values, what each \
            takes and where a new one starts. effect: puts a new effect of kind (eq, filter, \
            saturation, compressor, gate, volume-shaper, chorus, delay, reverb), at its starting \
            values, in place's slot, or with kind \"none\" empties it. move: moves place's slot \
            to slot to, shifting the slots between. on: switches place's slot on or off. set: \
            sets values of place's slot by name, like {\"band_2_gain_db\": -3, \"band_2_on\": \
            true}; shares are in percent, choices are words, and listens_to (the sidechain) is \
            \"own\", a track or a bus. pan: place's pan, -100 (left) to 100 (right). send: \
            amount percent of track to bus. add_bus: adds a bus, with name. remove_bus, \
            name_bus (blank lets it be \"Bus 2\"), bus_volume: bus, with name or volume. \
            lfo: set place's numbered LFO with values, as in jello_sound. connect: set the numbered \
            effect connection using source, slot, value, amount, optional one_way and scaled_by. \
            disconnect: remove the numbered effect connection. A connection moves an effect while \
            its source and scaling LFOs are shared and run in Hz or beats. Track effects share the \
            track's 64 connections with sound; buses and master each have 64 of their own. Use \
            percentage points for an effect's percent values; other amounts use that value's units. \
            Connections stay on their slot and value number when effects are replaced. \
            limiter: any of boost_db 0 to 24, ceiling_db -12 to 0, release_ms 1 to 1000. A value \
            outside its range, or a name Jello doesn't know, is refused with what is allowed, and \
            then nothing changes. Effects' values left out of look are at a new one's start. \
            Answers with the whole mix.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ACTIONS},
                "place": {"type": "string"},
                "slot": {"type": "integer"},
                "to": {"type": "integer"},
                "kind": {"type": "string"},
                "on": {"type": "boolean"},
                "values": {"type": "object"},
                "pan": {"type": "integer"},
                "track": {"type": "integer"},
                "bus": {"type": "integer"},
                "amount": {"type": "number"},
                "volume": {"type": "integer"},
                "name": {"type": "string"},
                "boost_db": {"type": "number"},
                "ceiling_db": {"type": "number"},
                "release_ms": {"type": "integer"},
                "number": {"type": "integer"},
                "source": {"type": "string"},
                "value": {"type": "string"},
                "scaled_by": {"type": "string"},
                "one_way": {"type": "boolean"},
            },
            "required": ["action"],
        },
    })
}

fn link_argument(composer: &Composer, place: Place, arguments: &Value) -> Result<EffectLink, String> {
    let source_word = text_argument(arguments, "source")?;
    let source = Source::from_name(&source_word).ok_or_else(|| String::from("source must name an LFO"))?;
    let scaled_word = optional_text_argument(arguments, "scaled_by")?.unwrap_or_else(|| String::from("none"));
    let scaled_by = Source::from_name(&scaled_word).ok_or_else(|| String::from("scaled_by must name an LFO or none"))?;
    let slot = slot_argument(arguments, "slot")?;
    let word = text_argument(arguments, "value")?;
    let mix = composer.mix_now();
    let effect = mix.slot(place, slot)?;
    let values = values_of(effect.kind, place.is_bus());
    let value = values.iter().find(|value| value.word == word)
        .ok_or_else(|| format!("this effect has no {word}"))?;
    let mut maximum = f64::from(EFFECT_MOST_MOVE);
    if value.sort == Sort::Percent {
        maximum *= 100.0;
    }
    let mut amount = number_in(arguments, "amount", -maximum, maximum)? as f32;
    if value.sort == Sort::Percent {
        amount /= 100.0;
    }
    let one_way = match arguments.get("one_way") {
        None => false,
        Some(value) => value.as_bool().ok_or_else(|| String::from("one_way must be true or false"))?,
    };
    let link = EffectLink { source, slot, value: value.number, amount, one_way, scaled_by };
    mix.check_effect_link(place, link)?;
    Ok(link)
}

// jello_mix: does `arguments`' action, then answers with the whole mix.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    if action == "describe" {
        return describe(arguments);
    }
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();

    let mut lines = Vec::new();
    match action.as_str() {
        "look" => {}
        "lfo" => {
            let place = place_argument(&composer, arguments)?;
            let number = whole_in(arguments, "number", 1, LFO_COUNT as i64)? as usize - 1;
            let fields = arguments.get("values").and_then(Value::as_object)
                .ok_or_else(|| String::from("lfo needs a values object"))?;
            if let Place::Track(track) = place {
                let mut lfo = composer.tracks().list()[track].sound_profile().lfos()[number];
                change_lfo(&mut lfo, &format!("lfo {}", number + 1), fields)?;
                let track = composer.tracks_mut().get_mut(track).unwrap();
                engine.on_track(place.chain(), |engine| track.sound_profile_mut().set_lfo(engine, number, lfo));
            } else {
                let modulation = composer.mix_mut().modulation_mut(place).unwrap();
                let mut lfo = modulation.lfos[number];
                change_lfo(&mut lfo, &format!("lfo {}", number + 1), fields)?;
                modulation.lfos[number] = engine.set_chain_lfo(place.chain(), number, lfo);
            }
            lines.push(format!("Set LFO {} on {}", number + 1, place.name()));
        }
        "connect" | "disconnect" => {
            let place = place_argument(&composer, arguments)?;
            let number = whole_in(arguments, "number", 1, CONNECTION_COUNT as i64)? as usize - 1;
            let mut links = composer.mix_now().links(place).unwrap().to_vec();
            if action == "disconnect" {
                if number >= links.len() {
                    return Err(String::from("there is no such effect connection"));
                }
                links.remove(number);
            } else {
                if number > links.len() {
                    return Err(String::from("add the next effect connection in order"));
                }
                let link = link_argument(&composer, place, arguments)?;
                if number == links.len() {
                    links.push(link);
                } else {
                    links[number] = link;
                }
            }
            if links.len() > CONNECTION_COUNT {
                return Err(String::from("a chain has at most 64 effect connections"));
            }
            if let Place::Track(track) = place {
                let sound = composer.tracks().list()[track].sound_profile();
                check_track_routing(sound.connections(), &links)?;
            }
            *composer.mix_mut().links_mut(place).unwrap() = links;
            if let Place::Track(_) = place {
                composer.send_track_routings(&mut engine);
            } else {
                let modulation = composer.mix_now().modulation(place).unwrap().clone();
                send_effect_links(&mut engine, place, &modulation.links);
            }
            lines.push(format!("Changed effect connections on {}", place.name()));
        }
        "effect" => {
            let place = place_argument(&composer, arguments)?;
            let slot = slot_argument(arguments, "slot")?;
            let word = text_argument(arguments, "kind")?;
            let kind = kind_argument(&word)?;
            composer.mix_mut().place_effect(&mut engine, place, slot, kind)?;
            if kind == EffectKind::None {
                lines.push(format!("Emptied slot {} on {}", slot + 1, place.name()));
            } else {
                lines.push(format!("Placed {word} in slot {} on {}", slot + 1, place.name()));
            }
        }
        "move" => {
            let place = place_argument(&composer, arguments)?;
            let from = slot_argument(arguments, "slot")?;
            let to = slot_argument(arguments, "to")?;
            composer.mix_mut().slot(place, from)?;
            composer.mix_mut().move_effect(&mut engine, place, from, to)?;
            lines.push(format!("Moved slot {} on {} to slot {}", from + 1, place.name(), to + 1));
        }
        "on" => {
            let place = place_argument(&composer, arguments)?;
            let slot = slot_argument(arguments, "slot")?;
            let Some(on) = arguments["on"].as_bool() else {
                return Err(String::from("on is needed, as true or false"));
            };
            composer.mix_mut().set_effect_on(&mut engine, place, slot, on)?;
            let mut switched = "off";
            if on {
                switched = "on";
            }
            lines.push(format!("Switched slot {} on {} {switched}", slot + 1, place.name()));
        }
        "set" => {
            let place = place_argument(&composer, arguments)?;
            let slot = slot_argument(arguments, "slot")?;
            set_values(&mut composer, &mut engine, place, slot, arguments, &mut lines)?;
        }
        "pan" => {
            let place = place_argument(&composer, arguments)?;
            let pan = whole_in(arguments, "pan", -i64::from(PAN_MOST), i64::from(PAN_MOST))?;
            let set = composer.mix_mut().set_pan(&mut engine, place, pan as i32)?;
            lines.push(format!("Panned {} to {set}", place.name()));
        }
        "send" => {
            let track = whole_in(arguments, "track", 1, i64::MAX)? as usize;
            let bus = whole_in(arguments, "bus", 1, i64::MAX)? as usize;
            let amount = whole_in(arguments, "amount", 0, i64::from(FULL_VOLUME))?;
            let set = composer.mix_mut().set_send(&mut engine, track - 1, bus - 1, amount as u32)?;
            lines.push(format!("Track {track} sends {set}% to bus {bus}"));
        }
        "add_bus" => {
            let name = optional_text_argument(arguments, "name")?;
            let bus = composer.mix_mut().add_bus(&mut engine, name.as_deref())?;
            let called = composer.mix_mut().bus_name(bus);
            lines.push(format!("Added bus {} ({called})", bus + 1));
        }
        "remove_bus" => {
            let bus = whole_in(arguments, "bus", 1, i64::MAX)? as usize;
            composer.remove_bus(&mut engine, bus - 1)?;
            lines.push(format!("Removed bus {bus}"));
        }
        "name_bus" => {
            let bus = whole_in(arguments, "bus", 1, i64::MAX)? as usize;
            let name = text_argument(arguments, "name")?;
            composer.mix_mut().rename_bus(bus - 1, Some(&name))?;
            let called = composer.mix_mut().bus_name(bus - 1);
            lines.push(format!("Bus {bus} is called {called}"));
        }
        "bus_volume" => {
            let bus = whole_in(arguments, "bus", 1, i64::MAX)? as usize;
            let volume = whole_in(arguments, "volume", 0, i64::from(FULL_VOLUME))?;
            let set = composer.mix_mut().set_bus_volume(&mut engine, bus - 1, volume as u32)?;
            lines.push(format!("Bus {bus} volume {set}%"));
        }
        "limiter" => {
            let limiter = limiter_argument(&composer, arguments)?;
            let set = composer.mix_mut().set_limiter(&mut engine, limiter);
            lines.push(format!(
                "Limiter: boost {} dB, ceiling {} dB, release {} ms",
                set.boost_db, set.ceiling_db, set.release_ms
            ));
        }
        _ => {
            return Err(format!("jello_mix has no action {action}: it has {}", ACTIONS.join(", ")));
        }
    }
    Ok(ToolAnswer::of(mix_json(&composer), lines))
}

// Sets every value `arguments`' values name on the effect in slot `slot`
// at `place`, after checking them all: one that can't be set refuses them
// all, and then nothing changes.
fn set_values(
    composer: &mut Composer,
    engine: &mut crate::engine::Engine,
    place: Place,
    slot: usize,
    arguments: &Value,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    let Some(asked) = arguments["values"].as_object() else {
        return Err(String::from("values is needed, like {\"mix\": 40}"));
    };
    if asked.is_empty() {
        return Err(String::from("values is empty: name at least one, like {\"mix\": 40}"));
    }
    let mix = composer.mix_mut();
    let effect = mix.slot(place, slot)?;
    let values = values_of(effect.kind, place.is_bus());

    let mut checked = Vec::new();
    for (word, given) in asked {
        let Some(written) = written_of_json(given) else {
            return Err(format!("{word} can't be {given}"));
        };
        mix.checked_effect_value(place, slot, word, &written)?;
        let value = values.iter().find(|value| value.word == *word).unwrap();
        if !is_in_range(value, &written) {
            return Err(format!("{word} takes {}, not {given}", what_it_takes(value)));
        }
        checked.push((word.clone(), written));
    }
    for (word, written) in checked {
        let set = mix.set_effect_value(engine, place, slot, &word, &written)?;
        lines.push(format!("Set {word} of slot {} on {} to {}", slot + 1, place.name(), json_of_written(&set)));
    }
    Ok(())
}

// The place `arguments` names, which the song must have.
fn place_argument(composer: &Composer, arguments: &Value) -> Result<Place, String> {
    let word = text_argument(arguments, "place")?;
    let Some(place) = Place::of_word(&word) else {
        return Err(format!("place is \"track 2\", \"bus 1\" or \"master\", not \"{word}\""));
    };
    let mix = composer.mix_now();
    if mix.chain(place).is_none() {
        return Err(format!("the song has no {}", place.name()));
    }
    Ok(place)
}

// Argument `name` as a slot, counted from 1, returned from 0.
fn slot_argument(arguments: &Value, name: &str) -> Result<usize, String> {
    let slot = whole_in(arguments, name, 1, EFFECT_SLOTS as i64)?;
    Ok(slot as usize - 1)
}

// The kind `word` names, or None for "none".
fn kind_argument(word: &str) -> Result<EffectKind, String> {
    if word == NONE_WORD {
        return Ok(EffectKind::None);
    }
    let Some(kind) = kind_of_word(word) else {
        return Err(format!("kind is one of {}, or {NONE_WORD}, not {word}", kind_words().join(", ")));
    };
    Ok(kind)
}

// Argument `name` as a whole number from `lowest` to `highest`.
fn whole_in(arguments: &Value, name: &str, lowest: i64, highest: i64) -> Result<i64, String> {
    let Some(number) = arguments[name].as_f64() else {
        return Err(format!("{name} is needed, as a whole number"));
    };
    if number.fract() != 0.0 {
        return Err(format!("{name} is a whole number, not {number}"));
    }
    let number = number as i64;
    if number < lowest || number > highest {
        if highest == i64::MAX {
            return Err(format!("{name} counts from {lowest}, not {number}"));
        }
        return Err(format!("{name} is {lowest} to {highest}, not {number}"));
    }
    Ok(number)
}

// The limiter with the values `arguments` gives, the others as they are.
fn limiter_argument(composer: &Composer, arguments: &Value) -> Result<Limiter, String> {
    let mut limiter = composer.mix_now().limiter;
    let mut named = false;
    if !arguments["boost_db"].is_null() {
        limiter.boost_db = number_in(arguments, "boost_db", 0.0, 24.0)? as f32;
        named = true;
    }
    if !arguments["ceiling_db"].is_null() {
        limiter.ceiling_db = number_in(arguments, "ceiling_db", -12.0, 0.0)? as f32;
        named = true;
    }
    if !arguments["release_ms"].is_null() {
        limiter.release_ms = whole_in(arguments, "release_ms", 1, 1000)? as u32;
        named = true;
    }
    if !named {
        return Err(String::from("limiter needs boost_db, ceiling_db or release_ms"));
    }
    Ok(limiter)
}

// Argument `name` as a number from `lowest` to `highest`.
fn number_in(arguments: &Value, name: &str, lowest: f64, highest: f64) -> Result<f64, String> {
    let Some(number) = arguments[name].as_f64() else {
        return Err(format!("{name} is needed, as a number"));
    };
    if number < lowest || number > highest {
        return Err(format!("{name} is {lowest} to {highest}, not {number}"));
    }
    Ok(number)
}

// A JSON value as a written one, or None for a sort no value has.
fn written_of_json(given: &Value) -> Option<Written> {
    if let Some(on) = given.as_bool() {
        return Some(Written::Switch(on));
    }
    if let Some(number) = given.as_f64() {
        return Some(Written::Number(number));
    }
    if let Some(word) = given.as_str() {
        return Some(Written::Word(String::from(word)));
    }
    None
}

fn json_of_written(written: &Written) -> Value {
    match written {
        Written::Number(number) => json!(number),
        Written::Switch(on) => json!(on),
        Written::Word(word) => json!(word),
    }
}

// jello_mix describe: every value of the kind `arguments` names.
fn describe(arguments: &Value) -> Result<ToolAnswer, String> {
    let word = text_argument(arguments, "kind")?;
    let kind = kind_argument(&word)?;
    if kind == EffectKind::None {
        return Err(String::from("describe needs a kind of effect"));
    }
    let mut values = Vec::new();
    for value in values_of(kind, false) {
        values.push(json!({
            "value": value.word,
            "takes": what_it_takes(&value),
            "start": json_of_written(&written_of(&value, value.start)),
        }));
    }
    let state = json!({
        "kind": word,
        "values": values,
        "note": "On a bus a chorus, delay or reverb starts at a mix of 100.",
    });
    Ok(ToolAnswer::of(state, Vec::new()))
}

// The whole mix, as jello_look and jello_mix read it. An effect lists only
// the values that differ from a new one's.
pub fn mix_json(composer: &Composer) -> Value {
    let mix = composer.mix_now();
    let mut tracks = Vec::new();
    for (index, strip) in mix.tracks.iter().enumerate() {
        let name = composer.tracks().list()[index].name(index + 1);
        let sends: Vec<u32> = strip.sends[..mix.buses.len()].to_vec();
        tracks.push(json!({
            "track": index + 1,
            "name": name,
            "pan": strip.pan,
            "sends": sends,
            "effects": chain_json(&strip.chain, Place::Track(index)),
            "lfos": composer.tracks().list()[index].sound_profile().lfos().iter().enumerate().map(|(number, lfo)| lfo_json(number, lfo)).collect::<Vec<_>>(),
            "effect_connections": links_json(&strip.links, &strip.chain, Place::Track(index), composer.tracks().list()[index].sound_profile().lfos()),
        }));
    }
    let mut buses = Vec::new();
    for (index, bus) in mix.buses.iter().enumerate() {
        buses.push(json!({
            "bus": index + 1,
            "name": mix.bus_name(index),
            "volume": bus.volume,
            "pan": bus.pan,
            "effects": chain_json(&bus.chain, Place::Bus(index)),
            "lfos": bus.modulation.lfos.iter().enumerate().map(|(number, lfo)| lfo_json(number, lfo)).collect::<Vec<_>>(),
            "effect_connections": links_json(&bus.modulation.links, &bus.chain, Place::Bus(index), &bus.modulation.lfos),
        }));
    }
    json!({
        "tracks": tracks,
        "buses": buses,
        "master": {
            "boost_db": mix.limiter.boost_db,
            "ceiling_db": mix.limiter.ceiling_db,
            "release_ms": mix.limiter.release_ms,
            "effects": chain_json(&mix.master, Place::Master),
            "lfos": mix.master_modulation.lfos.iter().enumerate().map(|(number, lfo)| lfo_json(number, lfo)).collect::<Vec<_>>(),
            "effect_connections": links_json(&mix.master_modulation.links, &mix.master, Place::Master, &mix.master_modulation.lfos),
        },
    })
}

fn links_json(links: &[EffectLink], chain: &Chain, place: Place, lfos: &[Lfo; LFO_COUNT]) -> Value {
    let mut shown = Vec::new();
    for (index, link) in links.iter().enumerate() {
        let mut word = None;
        let mut amount = None;
        let mut unit = None;
        let mut target_reason = Some(String::from("the effect target is unavailable"));
        if let Some(Some(effect)) = chain.get(link.slot) {
            let values = values_of(effect.kind, place.is_bus());
            if let Some(value) = values.into_iter().find(|value| value.number == link.value) {
                word = Some(value.word.clone());
                match value.sort {
                    Sort::Switch | Sort::Choice(_) | Sort::Steps(_) | Sort::ListensTo => {
                        target_reason = Some(String::from("the current effect value cannot be moved"));
                    }
                    _ => {
                        target_reason = None;
                        if value.sort == Sort::Percent {
                            if let Written::Number(number) = written_of(&value, link.amount) {
                                amount = Some(number);
                            }
                            unit = Some(String::from("percentage points"));
                        } else {
                            amount = Some(f64::from(link.amount));
                            unit = Some(format!("units of {}", value.word));
                        }
                    }
                }
            }
        }
        let mut reason = inactive_reason(*link, lfos);
        if target_reason.is_some() {
            reason = target_reason;
        }
        shown.push(json!({
            "number": index + 1,
            "source": link.source.name(),
            "slot": link.slot + 1,
            "value_number": link.value,
            "value": word,
            "amount": amount,
            "amount_unit": unit,
            "native_amount": link.amount,
            "one_way": link.one_way,
            "scaled_by": link.scaled_by.name(),
            "inactive_reason": reason,
        }));
    }
    json!(shown)
}

fn inactive_reason(link: EffectLink, lfos: &[Lfo; LFO_COUNT]) -> Option<String> {
    for source in [link.source, link.scaled_by] {
        if let Source::Lfo(index) = source {
            if lfos[index].per_note {
                return Some(format!("lfo {} is per-note", index + 1));
            }
            if lfos[index].speed == LfoSpeed::Presses {
                return Some(format!("lfo {} runs in presses", index + 1));
            }
        }
    }
    None
}

fn chain_json(chain: &Chain, place: Place) -> Value {
    let mut effects = Vec::new();
    for (slot, held) in chain.iter().enumerate() {
        let Some(effect) = held else {
            continue;
        };
        let mut changed = Map::new();
        let values = values_of(effect.kind, place.is_bus());
        for (value, set) in values.iter().zip(&effect.values) {
            if *set != value.start {
                changed.insert(value.word.clone(), json_of_written(&written_of(value, *set)));
            }
        }
        effects.push(json!({
            "slot": slot + 1,
            "kind": word_of_kind(effect.kind),
            "on": effect.on,
            "values": changed,
        }));
    }
    json!(effects)
}
