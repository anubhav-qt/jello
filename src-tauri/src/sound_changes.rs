// The agent's view of the song's sound, and its changes to it
// (docs/design.md#the-agent-harness). The agent reads the sound as JSON,
// with the same names a sound file uses (docs/adr/0037-presets.md), and
// changes it by sending only the values that change, in the same shape:
//
//   {"filter": {"cutoff_hz": 800}, "oscillators": [{"number": 2, "kind": "saw"}]}
//
// Oscillators, LFOs and connections are listed with their number, from 1,
// as the sound panel numbers them. The connections list only those in use.
// An oscillator's instrument, played when its kind is "sample", is an
// instrument folder's name or null for none.
//
// Unlike a sound file, a change is read strictly: a name or a word Jello
// does not know, or a value outside its range, is refused, saying what is
// allowed. Then nothing changes, and the agent can try again at once
// instead of hearing a different sound than it meant.

use serde_json::{Map, Value, json};

use crate::engine::{
    Engine, FILTER_LONGEST_MS, FILTER_LOWEST_HZ, FILTER_MOST_OCTAVES, FILTER_OPEN_HZ, FULL_PERCENT,
    FilterEnvelope, FilterType, GLIDE_LONGEST_MS, Kind, LOUDNESS_LONGEST_MS, LOUDNESS_SHORTEST_MS,
    LoudnessEnvelope, OSCILLATOR_COUNT, OSCILLATOR_MOST_CENTS, OSCILLATOR_MOST_HALF_STEPS,
    OSCILLATOR_MOST_OCTAVES, PULSE_NARROWEST_PERCENT, PULSE_WIDEST_PERCENT,
    TUNING_HIGHEST_HZ, TUNING_LOWEST_HZ, UNISON_MOST_COPIES, UNISON_MOST_DETUNE_CENTS, Unison,
};
use crate::modulation::{
    CONNECTION_COUNT, Connection, LFO_COUNT, LFO_FASTEST_HZ, LFO_LONGEST_BEATS, LFO_LONGEST_DELAY_BEATS,
    LFO_LONGEST_MS, LFO_MOST_PRESSES, LFO_MOST_STEPS, LFO_SHORTEST_BEATS, LFO_SLOWEST_HZ, Lfo, LfoShape,
    LfoSpeed, NO_CONNECTION, Source, Target, VIBRATO_KEY_LONGEST_MS, VibratoKeyTimes,
};
use crate::instruments;
use crate::sound_file::Sound;
use crate::sound_profile::SoundProfile;

// The parts of a sound, for the message refusing a part with another name.
const PARTS: &str = "tuning_hz, glide_ms, oscillators, loudness, filter, filter_envelope, unison, \
                     vibrato_key, lfos and connections";

// `sound` as the agent reads it.
pub fn json_of(sound: &Sound) -> Value {
    let mut oscillators = Vec::new();
    for (index, oscillator) in sound.oscillators.iter().enumerate() {
        oscillators.push(json!({
            "number": index + 1,
            "kind": oscillator.kind.name(),
            "level": oscillator.level,
            "octaves": oscillator.octaves,
            "half_steps": oscillator.half_steps,
            "cents": oscillator.cents,
            "width": oscillator.width,
            "instrument": sound.instruments[index],
        }));
    }

    let mut lfos = Vec::new();
    for (index, lfo) in sound.lfos.iter().enumerate() {
        lfos.push(lfo_json(index, lfo));
    }

    // Only the connections in use, which are what the agent needs to see.
    let mut connections = Vec::new();
    for (index, connection) in sound.connections.iter().enumerate() {
        if connection.source == Source::None {
            continue;
        }
        connections.push(json!({
            "number": index + 1,
            "source": connection.source.name(),
            "target": connection.target.name(),
            "amount": short(connection.amount),
            "unit": connection.target.unit().name(),
            "one_way": connection.one_way,
            "snap": connection.snap,
            "scaled_by": connection.scaled_by.name(),
        }));
    }

    let loudness = sound.loudness_envelope;
    let filter = sound.filter;
    let envelope = sound.filter_envelope;
    let unison = sound.unison;
    let key_times = sound.vibrato_key_times;
    json!({
        "tuning_hz": short(sound.tuning_hz),
        "glide_ms": sound.glide_ms,
        "oscillators": oscillators,
        "loudness": {
            "attack_ms": loudness.attack_ms,
            "decay_ms": loudness.decay_ms,
            "sustain": loudness.sustain,
            "release_ms": loudness.release_ms,
        },
        "filter": {
            "type": filter.filter_type.name(),
            "cutoff_hz": short(filter.cutoff_hz),
            "resonance": filter.resonance,
            "key_tracking": sound.key_tracking,
        },
        "filter_envelope": {
            "amount": short(envelope.amount),
            "attack_ms": envelope.attack_ms,
            "decay_ms": envelope.decay_ms,
            "sustain": envelope.sustain,
            "release_ms": envelope.release_ms,
        },
        "unison": {
            "copies": unison.copies,
            "detune_cents": unison.detune_cents,
            "spread": unison.spread,
        },
        "vibrato_key": {
            "rise_ms": key_times.rise_ms,
            "fall_ms": key_times.fall_ms,
        },
        "lfos": lfos,
        "connections": connections,
    })
}

fn lfo_json(index: usize, lfo: &Lfo) -> Value {
    // Only the steps that play.
    let mut steps = Vec::new();
    for height in lfo.steps.iter().take(lfo.step_count as usize) {
        steps.push(short(*height));
    }
    json!({
        "number": index + 1,
        "shape": lfo.shape.name(),
        "step_count": lfo.step_count,
        "steps": steps,
        "per_note": lfo.per_note,
        "speed": lfo.speed.name(),
        "hz": lfo.hz,
        "beats": lfo.beats,
        "presses": lfo.presses,
        "start": lfo.start,
        "spread": lfo.spread,
        "delay_ms": lfo.delay_ms,
        "fade_ms": lfo.fade_ms,
        "delay_beats": lfo.delay_beats,
        "fade_beats": lfo.fade_beats,
        "smooth": lfo.smooth,
        "once": lfo.once,
    })
}

// `value` as JSON, rounded to 4 decimals, so 0.1 reads as 0.1 and not as
// the 0.10000000149 an f32 turns into.
fn short(value: f32) -> f64 {
    let value = f64::from(value);
    (value * 10000.0).round() / 10000.0
}

// `sound` with `changes` made to it, or why they can't be, changing
// nothing.
pub fn changed(sound: &Sound, changes: &Value) -> Result<Sound, String> {
    let Some(changes) = changes.as_object() else {
        return Err(String::from(
            "the changes are an object, like {\"filter\": {\"cutoff_hz\": 800}}",
        ));
    };
    let mut sound = sound.clone();
    for (name, value) in changes {
        match name.as_str() {
            "tuning_hz" => {
                let hz = number_within(value, "tuning_hz", f64::from(TUNING_LOWEST_HZ), f64::from(TUNING_HIGHEST_HZ), " Hz")?;
                sound.tuning_hz = hz as f32;
            }
            "glide_ms" => {
                sound.glide_ms = count_within(value, "glide_ms", 0, GLIDE_LONGEST_MS, " ms")?;
            }
            "oscillators" => change_oscillators(&mut sound, value)?,
            "loudness" => change_loudness(&mut sound.loudness_envelope, value)?,
            "filter" => change_filter(&mut sound, value)?,
            "filter_envelope" => change_filter_envelope(&mut sound.filter_envelope, value)?,
            "unison" => change_unison(&mut sound.unison, value)?,
            "vibrato_key" => change_vibrato_key(&mut sound.vibrato_key_times, value)?,
            "lfos" => change_lfos(&mut sound.lfos, value)?,
            "connections" => change_connections(&mut sound.connections, value)?,
            _ => return Err(format!("the sound has no part called {name}: it has {PARTS}")),
        }
    }
    Ok(sound)
}

fn change_oscillators(sound: &mut Sound, value: &Value) -> Result<(), String> {
    for (index, fields) in numbered(value, "oscillators", OSCILLATOR_COUNT)? {
        let place = format!("oscillator {}", index + 1);
        let oscillator = &mut sound.oscillators[index];
        for (name, value) in fields {
            let named = format!("{place} {name}");
            match name.as_str() {
                "number" => {}
                "kind" => oscillator.kind = word(value, &named, Kind::from_name, &kind_names())?,
                "level" => oscillator.level = count_within(value, &named, 0, FULL_PERCENT, "%")?,
                "octaves" => oscillator.octaves = signed_within(value, &named, OSCILLATOR_MOST_OCTAVES, "")?,
                "half_steps" => oscillator.half_steps = signed_within(value, &named, OSCILLATOR_MOST_HALF_STEPS, "")?,
                "cents" => oscillator.cents = signed_within(value, &named, OSCILLATOR_MOST_CENTS, " cents")?,
                "width" => {
                    oscillator.width = count_within(value, &named, PULSE_NARROWEST_PERCENT, PULSE_WIDEST_PERCENT, "%")?;
                }
                "instrument" => sound.instruments[index] = instrument_named(value, &named)?,
                _ => {
                    let names = "kind, level, octaves, half_steps, cents, width and instrument";
                    return Err(no_such_value(&place, name, names));
                }
            }
        }
    }
    Ok(())
}

fn change_loudness(envelope: &mut LoudnessEnvelope, value: &Value) -> Result<(), String> {
    let lowest = LOUDNESS_SHORTEST_MS;
    let longest = LOUDNESS_LONGEST_MS;
    for (name, value) in fields(value, "loudness")? {
        let named = format!("loudness {name}");
        match name.as_str() {
            "attack_ms" => envelope.attack_ms = count_within(value, &named, lowest, longest, " ms")?,
            "decay_ms" => envelope.decay_ms = count_within(value, &named, lowest, longest, " ms")?,
            "sustain" => envelope.sustain = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            "release_ms" => envelope.release_ms = count_within(value, &named, lowest, longest, " ms")?,
            _ => return Err(no_such_value("loudness", name, "attack_ms, decay_ms, sustain and release_ms")),
        }
    }
    Ok(())
}

// The filter's group also holds key tracking, as a sound file does.
fn change_filter(sound: &mut Sound, value: &Value) -> Result<(), String> {
    let lowest = f64::from(FILTER_LOWEST_HZ);
    let highest = f64::from(FILTER_OPEN_HZ);
    for (name, value) in fields(value, "filter")? {
        let named = format!("filter {name}");
        match name.as_str() {
            "type" => sound.filter.filter_type = word(value, &named, FilterType::from_name, &filter_type_names())?,
            "cutoff_hz" => sound.filter.cutoff_hz = number_within(value, &named, lowest, highest, " Hz")? as f32,
            "resonance" => sound.filter.resonance = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            "key_tracking" => sound.key_tracking = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            _ => return Err(no_such_value("filter", name, "type, cutoff_hz, resonance and key_tracking")),
        }
    }
    Ok(())
}

fn change_filter_envelope(envelope: &mut FilterEnvelope, value: &Value) -> Result<(), String> {
    let longest = FILTER_LONGEST_MS;
    for (name, value) in fields(value, "filter_envelope")? {
        let named = format!("filter_envelope {name}");
        match name.as_str() {
            "amount" => {
                let octaves = number_within(value, &named, 0.0, f64::from(FILTER_MOST_OCTAVES), " octaves")?;
                envelope.amount = octaves as f32;
            }
            "attack_ms" => envelope.attack_ms = count_within(value, &named, 0, longest, " ms")?,
            "decay_ms" => envelope.decay_ms = count_within(value, &named, 0, longest, " ms")?,
            "sustain" => envelope.sustain = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            "release_ms" => envelope.release_ms = count_within(value, &named, 0, longest, " ms")?,
            _ => {
                let names = "amount, attack_ms, decay_ms, sustain and release_ms";
                return Err(no_such_value("filter_envelope", name, names));
            }
        }
    }
    Ok(())
}

fn change_unison(unison: &mut Unison, value: &Value) -> Result<(), String> {
    for (name, value) in fields(value, "unison")? {
        let named = format!("unison {name}");
        match name.as_str() {
            "copies" => unison.copies = count_within(value, &named, 1, UNISON_MOST_COPIES, "")?,
            "detune_cents" => {
                unison.detune_cents = count_within(value, &named, 0, UNISON_MOST_DETUNE_CENTS, " cents")?;
            }
            "spread" => unison.spread = count_within(value, &named, 0, FULL_PERCENT, "%")?,
            _ => return Err(no_such_value("unison", name, "copies, detune_cents and spread")),
        }
    }
    Ok(())
}

fn change_vibrato_key(times: &mut VibratoKeyTimes, value: &Value) -> Result<(), String> {
    let longest = VIBRATO_KEY_LONGEST_MS;
    for (name, value) in fields(value, "vibrato_key")? {
        let named = format!("vibrato_key {name}");
        match name.as_str() {
            "rise_ms" => times.rise_ms = count_within(value, &named, 0, longest, " ms")?,
            "fall_ms" => times.fall_ms = count_within(value, &named, 0, longest, " ms")?,
            _ => return Err(no_such_value("vibrato_key", name, "rise_ms and fall_ms")),
        }
    }
    Ok(())
}

fn change_lfos(lfos: &mut [Lfo; LFO_COUNT], value: &Value) -> Result<(), String> {
    for (index, fields) in numbered(value, "lfos", LFO_COUNT)? {
        let place = format!("lfo {}", index + 1);
        change_lfo(&mut lfos[index], &place, fields)?;
    }
    Ok(())
}

fn change_lfo(lfo: &mut Lfo, place: &str, fields: &Map<String, Value>) -> Result<(), String> {
    let percent = FULL_PERCENT;
    for (name, value) in fields {
        let named = format!("{place} {name}");
        match name.as_str() {
            "number" => {}
            "shape" => lfo.shape = word(value, &named, LfoShape::from_name, &lfo_shape_names())?,
            "step_count" => lfo.step_count = count_within(value, &named, 1, LFO_MOST_STEPS as u32, "")?,
            "steps" => change_steps(lfo, &named, value)?,
            "per_note" => lfo.per_note = flag(value, &named)?,
            "speed" => lfo.speed = word(value, &named, LfoSpeed::from_name, &lfo_speed_names())?,
            "hz" => lfo.hz = number_within(value, &named, LFO_SLOWEST_HZ, LFO_FASTEST_HZ, " Hz")?,
            "beats" => lfo.beats = number_within(value, &named, LFO_SHORTEST_BEATS, LFO_LONGEST_BEATS, " beats")?,
            "presses" => lfo.presses = count_within(value, &named, 1, LFO_MOST_PRESSES, " presses")?,
            "start" => lfo.start = count_within(value, &named, 0, percent, "%")?,
            "spread" => lfo.spread = count_within(value, &named, 0, percent, "%")?,
            "delay_ms" => lfo.delay_ms = count_within(value, &named, 0, LFO_LONGEST_MS, " ms")?,
            "fade_ms" => lfo.fade_ms = count_within(value, &named, 0, LFO_LONGEST_MS, " ms")?,
            "delay_beats" => lfo.delay_beats = number_within(value, &named, 0.0, LFO_LONGEST_DELAY_BEATS, " beats")?,
            "fade_beats" => lfo.fade_beats = number_within(value, &named, 0.0, LFO_LONGEST_DELAY_BEATS, " beats")?,
            "smooth" => lfo.smooth = count_within(value, &named, 0, percent, "%")?,
            "once" => lfo.once = flag(value, &named)?,
            _ => {
                let names = "shape, step_count, steps, per_note, speed, hz, beats, presses, start, spread, \
                             delay_ms, fade_ms, delay_beats, fade_beats, smooth and once";
                return Err(no_such_value(place, name, names));
            }
        }
    }

    // Steps sent without a step count play exactly the steps sent.
    if fields.contains_key("steps") && !fields.contains_key("step_count") {
        if let Some(steps) = fields["steps"].as_array() {
            lfo.step_count = steps.len() as u32;
        }
    }
    Ok(())
}

// Sets the first steps of `lfo` to the heights in `value`, a list of 1 to
// LFO_MOST_STEPS numbers from -100 to 100. The steps after them stay.
fn change_steps(lfo: &mut Lfo, named: &str, value: &Value) -> Result<(), String> {
    let Some(heights) = value.as_array() else {
        return Err(format!("{named} is a list of heights from -100 to 100, like [100, -50, 0]"));
    };
    if heights.is_empty() || heights.len() > LFO_MOST_STEPS {
        return Err(format!("{named} lists 1 to {LFO_MOST_STEPS} heights, not {}", heights.len()));
    }
    let most = f64::from(FULL_PERCENT);
    for (step, height) in lfo.steps.iter_mut().zip(heights) {
        *step = number_within(height, named, -most, most, "%")? as f32;
    }
    Ok(())
}

fn change_connections(connections: &mut [Connection; CONNECTION_COUNT], value: &Value) -> Result<(), String> {
    for (index, fields) in numbered(value, "connections", CONNECTION_COUNT)? {
        let place = format!("connection {}", index + 1);
        connections[index] = changed_connection(connections[index], &place, fields)?;
    }
    Ok(())
}

// `connection` with `fields` changed. A source of "none" empties it.
fn changed_connection(connection: Connection, place: &str, fields: &Map<String, Value>) -> Result<Connection, String> {
    let was_empty = connection.source == Source::None;
    let mut connection = connection;
    for (name, value) in fields {
        let named = format!("{place} {name}");
        match name.as_str() {
            "number" => {}
            "unit" => {}
            "source" => connection.source = word(value, &named, Source::from_name, &source_names())?,
            "target" => connection.target = word(value, &named, Target::from_name, &target_names())?,
            // Checked below, once the target is known.
            "amount" => connection.amount = number_within(value, &named, f64::MIN, f64::MAX, "")? as f32,
            "one_way" => connection.one_way = flag(value, &named)?,
            "snap" => connection.snap = flag(value, &named)?,
            "scaled_by" => connection.scaled_by = word(value, &named, Source::from_name, &source_names())?,
            _ => {
                let names = "source, target, amount, one_way, snap and scaled_by";
                return Err(no_such_value(place, name, names));
            }
        }
    }

    if connection.source == Source::None {
        return Ok(NO_CONNECTION);
    }
    if was_empty && !fields.contains_key("target") {
        return Err(format!("{place} is empty, so it needs a target too"));
    }
    let most = connection.target.most_amount();
    if connection.amount.abs() > most {
        let target = connection.target.name();
        let unit = connection.target.unit().name();
        return Err(format!(
            "{place} amount moves {target} -{most} to {most} {unit}, not {}",
            short(connection.amount)
        ));
    }
    Ok(connection)
}

// The fields of part `part`, which must be an object.
fn fields<'a>(value: &'a Value, part: &str) -> Result<&'a Map<String, Value>, String> {
    match value.as_object() {
        Some(fields) => Ok(fields),
        None => Err(format!("{part} is an object of the values that change, not {value}")),
    }
}

// The items of list `part`, each with its index from 0 and its fields. Each
// is an object with a "number", from 1 to `count`.
fn numbered<'a>(value: &'a Value, part: &str, count: usize) -> Result<Vec<(usize, &'a Map<String, Value>)>, String> {
    let Some(items) = value.as_array() else {
        return Err(format!("{part} is a list, like [{{\"number\": 1, ...}}], not {value}"));
    };
    let mut numbered = Vec::new();
    for item in items {
        let Some(fields) = item.as_object() else {
            return Err(format!("each of {part} is an object, not {item}"));
        };
        let Some(number) = fields.get("number") else {
            return Err(format!("each of {part} says its number, from 1 to {count}"));
        };
        let name = format!("{part} number");
        let number = count_within(number, &name, 1, count as u32, "")?;
        numbered.push((number as usize - 1, fields));
    }
    Ok(numbered)
}

// `value` as a number from `lowest` to `highest`, or why not, calling it
// `named` and the unit `unit`.
fn number_within(value: &Value, named: &str, lowest: f64, highest: f64, unit: &str) -> Result<f64, String> {
    let Some(number) = value.as_f64() else {
        return Err(format!("{named} is a number, not {value}"));
    };
    if number < lowest || number > highest {
        return Err(format!("{named} is {lowest} to {highest}{unit}, not {number}"));
    }
    Ok(number)
}

// The same for a whole number from `lowest` to `highest`.
pub(crate) fn count_within(value: &Value, named: &str, lowest: u32, highest: u32, unit: &str) -> Result<u32, String> {
    let number = number_within(value, named, f64::from(lowest), f64::from(highest), unit)?;
    if number.fract() != 0.0 {
        return Err(format!("{named} is a whole number, not {number}"));
    }
    Ok(number as u32)
}

// The same for a whole number from -`most` to `most`.
pub(crate) fn signed_within(value: &Value, named: &str, most: i32, unit: &str) -> Result<i32, String> {
    let most = f64::from(most);
    let number = number_within(value, named, -most, most, unit)?;
    if number.fract() != 0.0 {
        return Err(format!("{named} is a whole number, not {number}"));
    }
    Ok(number as i32)
}

fn flag(value: &Value, named: &str) -> Result<bool, String> {
    match value.as_bool() {
        Some(flag) => Ok(flag),
        None => Err(format!("{named} is true or false, not {value}")),
    }
}

// The choice `value` names, through `from_name`, or why not, listing
// `words`.
fn word<T>(value: &Value, named: &str, from_name: impl Fn(&str) -> Option<T>, words: &[String]) -> Result<T, String> {
    let choices = words.join(", ");
    let Some(text) = value.as_str() else {
        return Err(format!("{named} is one of {choices}, not {value}"));
    };
    match from_name(text) {
        Some(choice) => Ok(choice),
        None => Err(format!("{named} is one of {choices}, not {text}")),
    }
}

// The instrument `value` names, which must be one in the Instruments
// folder, or None for null.
pub fn instrument_named(value: &Value, named: &str) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let names = instruments::list();
    if names.is_empty() {
        return Err(format!("{named} can't be set: there are no instruments in the Instruments folder yet"));
    }
    let choices = names.join(", ");
    let Some(text) = value.as_str() else {
        return Err(format!("{named} is one of {choices} or null, not {value}"));
    };
    if !names.contains(&String::from(text)) {
        return Err(format!("{named} is one of {choices} or null, not {text}"));
    }
    Ok(Some(String::from(text)))
}

fn no_such_value(place: &str, name: &str, names: &str) -> String {
    format!("{place} has no value called {name}: it has {names}")
}

fn kind_names() -> Vec<String> {
    let mut names = Vec::new();
    for kind in Kind::ALL {
        names.push(String::from(kind.name()));
    }
    names
}

fn filter_type_names() -> Vec<String> {
    let mut names = Vec::new();
    for filter_type in FilterType::ALL {
        names.push(String::from(filter_type.name()));
    }
    names
}

fn lfo_shape_names() -> Vec<String> {
    let mut names = Vec::new();
    for shape in LfoShape::ALL {
        names.push(String::from(shape.name()));
    }
    names
}

fn lfo_speed_names() -> Vec<String> {
    let mut names = Vec::new();
    for speed in LfoSpeed::ALL {
        names.push(String::from(speed.name()));
    }
    names
}

// Every source's name, "none" first.
pub fn source_names() -> Vec<String> {
    let mut names = vec![Source::None.name()];
    for index in 0..LFO_COUNT {
        names.push(Source::Lfo(index).name());
    }
    let others = [
        Source::VibratoKey,
        Source::NoteHeight,
        Source::Random,
        Source::LoudnessEnvelope,
        Source::FilterEnvelope,
    ];
    for source in others {
        names.push(source.name());
    }
    names
}

// Every target's name.
pub fn target_names() -> Vec<String> {
    let mut names = Vec::new();
    for target in Target::OTHERS {
        names.push(target.name());
    }
    for index in 0..LFO_COUNT {
        names.push(Target::LfoSpeed(index).name());
    }
    for index in 0..OSCILLATOR_COUNT {
        names.push(Target::OscillatorLevel(index).name());
        names.push(Target::OscillatorPitch(index).name());
        names.push(Target::OscillatorWidth(index).name());
        names.push(Target::OscillatorKind(index).name());
    }
    names
}

// Makes the song's sound `after`, on `engine` too, changing only the parts
// that differ from `before`, each through the same call the sound panel
// makes. So the parts left alone don't restart, and each changed one is
// brought within its range by the engine, as the panel's are.
pub fn take_changes(profile: &mut SoundProfile, engine: &mut Engine, before: &Sound, after: &Sound) {
    for index in 0..OSCILLATOR_COUNT {
        if after.oscillators[index] != before.oscillators[index] {
            profile.set_oscillator(engine, index, after.oscillators[index]);
        }
        if after.instruments[index] != before.instruments[index] {
            profile.set_instrument(engine, index, after.instruments[index].as_deref());
        }
    }
    if after.loudness_envelope != before.loudness_envelope {
        profile.set_loudness_envelope(engine, after.loudness_envelope);
    }
    if after.filter != before.filter {
        profile.set_filter(engine, after.filter);
    }
    if after.filter_envelope != before.filter_envelope {
        profile.set_filter_envelope(engine, after.filter_envelope);
    }
    if after.key_tracking != before.key_tracking {
        profile.set_key_tracking(engine, after.key_tracking);
    }
    if after.unison != before.unison {
        profile.set_unison(engine, after.unison);
    }
    if after.tuning_hz != before.tuning_hz {
        profile.set_tuning(engine, after.tuning_hz);
    }
    for index in 0..LFO_COUNT {
        if after.lfos[index] != before.lfos[index] {
            profile.set_lfo(engine, index, after.lfos[index]);
        }
    }
    for index in 0..CONNECTION_COUNT {
        if after.connections[index] != before.connections[index] {
            profile.set_connection(engine, index, after.connections[index]);
        }
    }
    if after.vibrato_key_times != before.vibrato_key_times {
        profile.set_vibrato_key_times(engine, after.vibrato_key_times);
    }
    if after.glide_ms != before.glide_ms {
        profile.set_glide_time(engine, after.glide_ms);
    }
}

// One line for each value that differs between `before` and `after`, like
// "Filter cutoff 1200 -> 800 Hz", for the agent's panel.
pub fn change_lines(before: &Sound, after: &Sound) -> Vec<String> {
    let before_facts = facts_of(before);
    let after_facts = facts_of(after);
    let mut lines = Vec::new();
    // Both lists name the same values in the same order.
    for ((label, was), (_, is)) in before_facts.iter().zip(&after_facts) {
        if was == is {
            continue;
        }
        lines.push(line_of(label, was, is));
    }

    for index in 0..CONNECTION_COUNT {
        let was = before.connections[index];
        let is = after.connections[index];
        if was == is {
            continue;
        }
        let label = format!("Connection {}", index + 1);
        lines.push(format!("{label} {} -> {}", connection_text(&was), connection_text(&is)));
    }
    lines
}

// "Filter cutoff 1200 -> 800 Hz" from the label "Filter cutoff" and the
// values "1200 Hz" and "800 Hz": a unit both share is said once, at the end.
fn line_of(label: &str, was: &(String, String), is: &(String, String)) -> String {
    let (was_value, was_unit) = was;
    let (is_value, is_unit) = is;
    if was_unit == is_unit {
        format!("{label} {was_value} -> {is_value}{was_unit}")
    } else {
        format!("{label} {was_value}{was_unit} -> {is_value}{is_unit}")
    }
}

// Every value of `sound` but its connections, each with a label like
// "Filter cutoff" and its value and unit, like ("1200", " Hz").
fn facts_of(sound: &Sound) -> Vec<(String, (String, String))> {
    let json = json_of(sound);
    let mut facts = Vec::new();
    add_facts(&mut facts, "", &json);
    facts
}

fn add_facts(facts: &mut Vec<(String, (String, String))>, title: &str, value: &Value) {
    let Some(fields) = value.as_object() else {
        return;
    };
    for (name, value) in fields {
        match name.as_str() {
            "number" => {}
            "connections" => {}
            "oscillators" => add_numbered_facts(facts, "Oscillator", value),
            "lfos" => add_numbered_facts(facts, "LFO", value),
            _ => {
                if value.is_object() {
                    add_facts(facts, &title_of(name), value);
                } else {
                    let (label, unit) = label_and_unit(name);
                    let label = format!("{title} {label}");
                    facts.push((label_case(label.trim()), (text_of(value), String::from(unit))));
                }
            }
        }
    }
}

fn add_numbered_facts(facts: &mut Vec<(String, (String, String))>, title: &str, value: &Value) {
    let Some(items) = value.as_array() else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        add_facts(facts, &format!("{title} {}", index + 1), item);
    }
}

// "filter_envelope" as "Filter envelope".
fn title_of(name: &str) -> String {
    label_case(&name.replace('_', " "))
}

// A value's name without its unit, and the unit: "cutoff_hz" is "cutoff"
// in " Hz".
fn label_and_unit(name: &str) -> (String, &'static str) {
    if let Some(label) = name.strip_suffix("_hz") {
        return (label.replace('_', " "), " Hz");
    }
    if let Some(label) = name.strip_suffix("_ms") {
        return (label.replace('_', " "), " ms");
    }
    if let Some(label) = name.strip_suffix("_cents") {
        return (label.replace('_', " "), " cents");
    }
    if let Some(label) = name.strip_suffix("_beats") {
        return (label.replace('_', " "), " beats");
    }
    let unit = match name {
        "level" => "%",
        "width" => "%",
        "sustain" => "%",
        "resonance" => "%",
        "key_tracking" => "%",
        "spread" => "%",
        "start" => "%",
        "smooth" => "%",
        "cents" => " cents",
        "hz" => " Hz",
        "amount" => " octaves",
        _ => "",
    };
    (name.replace('_', " "), unit)
}

// `label` with its first letter in upper case.
fn label_case(label: &str) -> String {
    let mut letters = label.chars();
    match letters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), letters.as_str()),
        None => String::new(),
    }
}

// A JSON value as the panel shows it: words without quotes, and whole
// numbers without ".0".
fn text_of(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return String::from(text);
    }
    if let Some(number) = value.as_f64() {
        return format!("{number}");
    }
    value.to_string()
}

// A connection in words, like "lfo 1 moves cutoff by 2 octaves", or "off".
fn connection_text(connection: &Connection) -> String {
    if connection.source == Source::None {
        return String::from("off");
    }
    let mut unit = String::from(connection.target.unit().name());
    if unit != "%" {
        unit.insert(0, ' ');
    }
    let mut text = format!(
        "{} moves {} by {}{unit}",
        connection.source.name(),
        connection.target.name(),
        short(connection.amount),
    );
    if connection.one_way {
        text.push_str(", one way");
    }
    if connection.snap {
        text.push_str(", snapped");
    }
    if connection.scaled_by != Source::None {
        text.push_str(&format!(", scaled by {}", connection.scaled_by.name()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_sound() -> Sound {
        SoundProfile::new().sound()
    }

    #[test]
    fn a_change_names_only_what_changes() {
        let before = new_sound();
        let changes = json!({"filter": {"cutoff_hz": 800}, "oscillators": [{"number": 2, "kind": "saw"}]});
        let after = changed(&before, &changes).unwrap();

        assert_eq!(after.filter.cutoff_hz, 800.0);
        assert_eq!(after.oscillators[1].kind, Kind::Saw);
        // Everything else stays.
        assert_eq!(after.filter.resonance, before.filter.resonance);
        assert_eq!(after.oscillators[0], before.oscillators[0]);
        assert_eq!(after.lfos, before.lfos);
    }

    #[test]
    fn the_sound_reads_back_as_a_change_that_changes_nothing() {
        // What the agent reads it can send back as it is.
        let before = new_sound();
        let after = changed(&before, &json_of(&before)).unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn values_outside_their_ranges_are_refused_with_the_range() {
        let before = new_sound();
        let error = changed(&before, &json!({"filter": {"cutoff_hz": 30000}})).unwrap_err();
        assert_eq!(error, "filter cutoff_hz is 20 to 20000 Hz, not 30000");

        let error = changed(&before, &json!({"oscillators": [{"number": 1, "level": 50.5}]})).unwrap_err();
        assert_eq!(error, "oscillator 1 level is a whole number, not 50.5");

        let error = changed(&before, &json!({"oscillators": [{"number": 4, "level": 50}]})).unwrap_err();
        assert_eq!(error, "oscillators number is 1 to 3, not 4");
    }

    #[test]
    fn unknown_names_and_words_are_refused_with_what_there_is() {
        let before = new_sound();
        let error = changed(&before, &json!({"oscillators": [{"number": 1, "kind": "banjo"}]})).unwrap_err();
        assert_eq!(error, "oscillator 1 kind is one of sine, triangle, square, saw, noise, sample, not banjo");

        let error = changed(&before, &json!({"reverb": {"size": 3}})).unwrap_err();
        assert!(error.starts_with("the sound has no part called reverb: it has tuning_hz"));

        let error = changed(&before, &json!({"unison": {"voices": 3}})).unwrap_err();
        assert_eq!(error, "unison has no value called voices: it has copies, detune_cents and spread");
    }

    #[test]
    fn steps_sent_alone_set_how_many_steps_play() {
        let before = new_sound();
        let changes = json!({"lfos": [{"number": 3, "shape": "steps", "steps": [100, -50, 0]}]});
        let after = changed(&before, &changes).unwrap();
        let lfo = after.lfos[2];
        assert_eq!(lfo.shape, LfoShape::Steps);
        assert_eq!(lfo.step_count, 3);
        assert_eq!(&lfo.steps[..3], &[100.0, -50.0, 0.0]);
    }

    #[test]
    fn connections_are_added_checked_against_their_target_and_removed() {
        let before = new_sound();
        let add = json!({"connections": [{"number": 5, "source": "lfo 1", "target": "cutoff", "amount": 2}]});
        let added = changed(&before, &add).unwrap();
        assert_eq!(added.connections[4].source, Source::Lfo(0));
        assert_eq!(added.connections[4].target, Target::Cutoff);
        assert_eq!(added.connections[4].amount, 2.0);

        // An empty connection needs a target, so it never moves pitch by
        // accident.
        let no_target = json!({"connections": [{"number": 6, "source": "random"}]});
        let error = changed(&added, &no_target).unwrap_err();
        assert_eq!(error, "connection 6 is empty, so it needs a target too");

        // Cutoff moves at most 10 octaves either way.
        let too_far = json!({"connections": [{"number": 5, "amount": -11}]});
        let error = changed(&added, &too_far).unwrap_err();
        assert_eq!(error, "connection 5 amount moves cutoff -10 to 10 octaves, not -11");

        let remove = json!({"connections": [{"number": 5, "source": "none"}]});
        let removed = changed(&added, &remove).unwrap();
        assert_eq!(removed.connections[4], NO_CONNECTION);
    }

    #[test]
    fn change_lines_say_what_changed_with_units() {
        let before = new_sound();
        let changes = json!({
            "filter": {"cutoff_hz": 800, "type": "notch"},
            "oscillators": [{"number": 2, "level": 40}],
            "connections": [{"number": 10, "source": "lfo 2", "target": "pan", "amount": 30, "one_way": true}],
        });
        let after = changed(&before, &changes).unwrap();
        let was_cutoff = short(before.filter.cutoff_hz);
        let was_level = before.oscillators[1].level;

        // The values come in the order the JSON lists them, so the test
        // sorts them.
        let mut lines = change_lines(&before, &after);
        lines.sort();
        assert_eq!(
            lines,
            vec![
                String::from("Connection 10 off -> lfo 2 moves pan by 30%, one way"),
                format!("Filter cutoff {was_cutoff} -> 800 Hz"),
                String::from("Filter type low-pass -> notch"),
                format!("Oscillator 2 level {was_level} -> 40%"),
            ]
        );
        assert!(change_lines(&after, &after).is_empty());
    }
}
