// The mix written as part of a song file (src/song_file.rs): each track's
// pan and sends among its values, its effects under [[track.effect]], each
// bus under [[bus]] with its effects under [[bus.effect]], and the master's
// limiter under [master] with its effects under [[master.effect]].
//
// An effect says its slot, from 1, its kind and whether it is on, then only
// the values that differ from where a new one starts:
//
//   [[track.effect]]
//   slot = 1
//   kind = "eq"
//   on = true
//   band_2_on = true
//   band_2_gain_db = -3
//
// Reading is forgiving, like the rest of the song file:
// - A value left out, of the wrong sort, or a word Jello doesn't know takes
//   a new effect's.
// - An effect with no slot from 1 to 8, a slot already used, or a kind
//   Jello doesn't know is skipped.
// - Pan, sends and volumes are kept within their ranges, and sends to
//   buses the song doesn't have are dropped. A song keeps its first 4
//   buses.
// - "Listens to" a chain it may not listen to, or one that would make a
//   loop, listens to its own input instead.

use toml::{Table, Value};

use crate::effects::{Written, kind_of_word, number_of, values_of, word_of_kind, written_of};
use crate::engine::{BUS_COUNT, EFFECT_MOST_VALUES, EFFECT_SLOTS, FULL_VOLUME, Limiter, PAN_MOST, START_LIMITER, limiter_within_ranges};
use crate::mix::{Bus, Chain, EffectLink, EffectModulation, Mix, Place, Slot, Strip, effect_value_can_move, empty_chain};
use crate::modulation::{CONNECTION_COUNT, LFO_COUNT, Source};
use crate::sound_changes::change_lfo;
use crate::sound_file::{
    group, groups, read_count, read_f64, read_signed, read_word, start_group, write_name, write_number,
    write_word, write_flag, write_lfo,
};

// Track `track`'s pan and sends, as values of its [[track]]. Sends are
// written only for the song's buses.
pub fn write_strip_values(text: &mut String, mix: &Mix, track: usize) {
    let Some(strip) = mix.tracks.get(track) else {
        return;
    };
    write_number(text, "pan", strip.pan);
    if mix.buses.is_empty() {
        return;
    }
    let mut sends = Vec::new();
    for bus in 0..mix.buses.len() {
        sends.push(strip.sends[bus].to_string());
    }
    text.push_str(&format!("sends = [{}]\n", sends.join(", ")));
}

// Track `track`'s effects, under [[track.effect]].
pub fn write_strip_effects(text: &mut String, mix: &Mix, track: usize) {
    if let Some(strip) = mix.tracks.get(track) {
        write_chain(text, &strip.chain, Place::Track(track), "[[track.effect]]");
        write_links(text, &strip.links, "track");
    }
}

// The buses, then the master, after the tracks.
pub fn write_buses_and_master(text: &mut String, mix: &Mix) {
    for (number, bus) in mix.buses.iter().enumerate() {
        start_group(text, "[[bus]]");
        if let Some(name) = &bus.name {
            write_name(text, "name", name);
        }
        write_number(text, "volume", bus.volume);
        write_number(text, "pan", bus.pan);
        write_chain(text, &bus.chain, Place::Bus(number), "[[bus.effect]]");
        write_modulation(text, &bus.modulation, "bus");
    }

    start_group(text, "[master]");
    write_number(text, "boost_db", mix.limiter.boost_db);
    write_number(text, "ceiling_db", mix.limiter.ceiling_db);
    write_number(text, "release_ms", mix.limiter.release_ms);
    write_chain(text, &mix.master, Place::Master, "[[master.effect]]");
    write_modulation(text, &mix.master_modulation, "master");
}

fn write_links(text: &mut String, links: &[EffectLink], under: &str) {
    for link in links {
        start_group(text, &format!("[[{under}.effect_link]]"));
        write_word(text, "source", &link.source.name());
        write_number(text, "slot", link.slot + 1);
        write_number(text, "value", link.value);
        write_number(text, "amount", link.amount);
        write_flag(text, "one_way", link.one_way);
        write_word(text, "scaled_by", &link.scaled_by.name());
    }
}

fn write_modulation(text: &mut String, modulation: &EffectModulation, under: &str) {
    if modulation.lfos != EffectModulation::new().lfos {
        for lfo in &modulation.lfos {
            write_lfo(text, lfo, under);
        }
    }
    write_links(text, &modulation.links, under);
}

// Every effect in `chain` at `place`, each under `heading`.
fn write_chain(text: &mut String, chain: &Chain, place: Place, heading: &str) {
    for (slot, held) in chain.iter().enumerate() {
        let Some(effect) = held else {
            continue;
        };
        let Some(kind) = word_of_kind(effect.kind) else {
            continue;
        };
        start_group(text, heading);
        write_number(text, "slot", slot + 1);
        write_word(text, "kind", kind);
        write_number(text, "on", effect.on);
        let values = values_of(effect.kind, place.is_bus());
        for (value, set) in values.iter().zip(&effect.values) {
            if *set == value.start {
                continue;
            }
            match written_of(value, *set) {
                Written::Number(number) => write_number(text, &value.word, number),
                Written::Switch(on) => write_number(text, &value.word, on),
                Written::Word(word) => write_word(text, &value.word, &word),
            }
        }
    }
}

// The mix `table`, a whole song file, describes for the tracks in
// `track_tables`, its [[track]] groups in order.
pub fn read_mix(table: &Table, track_tables: &[&Table]) -> Result<Mix, String> {
    let mut mix = Mix::new(0);
    for bus_table in groups(table, "bus") {
        if mix.buses.len() == BUS_COUNT {
            break;
        }
        let mut bus = read_bus(bus_table, mix.buses.len());
        bus.modulation = read_modulation(bus_table)?;
        mix.buses.push(bus);
    }
    for (track, track_table) in track_tables.iter().enumerate() {
        let mut strip = read_strip(track_table, track, mix.buses.len());
        strip.links = read_links(track_table)?;
        mix.tracks.push(strip);
    }
    if let Some(master_table) = group(table, "master") {
        mix.limiter = read_limiter(master_table);
        mix.master = read_chain(master_table, Place::Master);
        mix.master_modulation = read_modulation(master_table)?;
    }
    mix.keep_only_good_listening();
    Ok(mix)
}

fn read_links(table: &Table) -> Result<Vec<EffectLink>, String> {
    if let Some(value) = table.get("effect_link") {
        if !value.is_array() {
            return Err(String::from("effect_link must be a list"));
        }
    }
    let mut links = Vec::new();
    for item in groups(table, "effect_link") {
        let source = read_word(item, "source").and_then(Source::from_name);
        let Some(Source::Lfo(source)) = source else {
            return Err(String::from("effect link source must name an LFO"));
        };
        if source >= LFO_COUNT {
            return Err(String::from("effect link LFO is out of range"));
        }
        let scaled_by = read_word(item, "scaled_by").and_then(Source::from_name)
            .ok_or_else(|| String::from("effect link scaled_by must name an LFO or none"))?;
        match scaled_by {
            Source::None => {},
            Source::Lfo(index) if index < LFO_COUNT => {},
            _ => return Err(String::from("effect link scaled_by must name an LFO or none")),
        }
        let slot = item.get("slot").and_then(Value::as_integer).unwrap_or(0);
        let value = item.get("value").and_then(Value::as_integer).unwrap_or(-1);
        let amount = item.get("amount").and_then(crate::sound_file::number_in)
            .ok_or_else(|| String::from("effect link needs a numeric amount"))?;
        let one_way = item.get("one_way").and_then(Value::as_bool)
            .ok_or_else(|| String::from("effect link needs one_way as true or false"))?;
        if slot < 1 || slot > EFFECT_SLOTS as i64 || value < 0 || value >= EFFECT_MOST_VALUES as i64 {
            return Err(String::from("effect link slot or value is out of range"));
        }
        if !effect_value_can_move(value as i32) {
            return Err(String::from("effect link value cannot be moved"));
        }
        if !amount.is_finite() || amount.abs() > 20_000.0 {
            return Err(String::from("effect link amount must be from -20000 to 20000"));
        }
        links.push(EffectLink { source: Source::Lfo(source), slot: slot as usize - 1, value: value as i32,
            amount: amount as f32, one_way, scaled_by });
        if links.len() > CONNECTION_COUNT {
            return Err(String::from("a chain has at most 64 effect links"));
        }
    }
    if let Some(Value::Array(items)) = table.get("effect_link") {
        if items.len() != links.len() {
            return Err(String::from("each effect link must be a table"));
        }
    }
    Ok(links)
}

fn read_modulation(table: &Table) -> Result<EffectModulation, String> {
    let mut modulation = EffectModulation::new();
    if let Some(value) = table.get("lfos") {
        let Some(items) = value.as_array() else {
            return Err(String::from("mix lfos must be a list"));
        };
        if items.len() > LFO_COUNT {
            return Err(String::from("a chain has at most 8 LFOs"));
        }
        for (index, item) in items.iter().enumerate() {
            let fields = serde_json::to_value(item).map_err(|error| error.to_string())?;
            let fields = fields.as_object().ok_or_else(|| String::from("each mix LFO must be a table"))?;
            change_lfo(&mut modulation.lfos[index], &format!("lfo {}", index + 1), fields)?;
        }
    }
    modulation.links = read_links(table)?;
    Ok(modulation)
}

// A track's strip, from its [[track]] group. Sends past `bus_count` are
// dropped.
fn read_strip(track_table: &Table, track: usize, bus_count: usize) -> Strip {
    let mut strip = Strip::new();
    strip.pan = read_pan(track_table);
    if let Some(Value::Array(sends)) = track_table.get("sends") {
        for (bus, send) in sends.iter().enumerate() {
            if bus == bus_count {
                break;
            }
            strip.sends[bus] = percent_in(send, 0);
        }
    }
    strip.chain = read_chain(track_table, Place::Track(track));
    strip
}

fn read_bus(bus_table: &Table, bus: usize) -> Bus {
    let mut held = Bus::new(None);
    if let Some(name) = read_word(bus_table, "name") {
        if !name.trim().is_empty() {
            held.name = Some(String::from(name));
        }
    }
    held.volume = read_count(bus_table, "volume", FULL_VOLUME).min(FULL_VOLUME);
    held.pan = read_pan(bus_table);
    held.chain = read_chain(bus_table, Place::Bus(bus));
    held
}

fn read_pan(table: &Table) -> i32 {
    read_signed(table, "pan", 0).clamp(-PAN_MOST, PAN_MOST)
}

// A percent from 0 to FULL_VOLUME, or `missing` for anything not a number.
fn percent_in(value: &Value, missing: u32) -> u32 {
    let number = match value {
        Value::Integer(number) => *number as f64,
        Value::Float(number) => *number,
        _ => return missing,
    };
    if number.is_nan() {
        return missing;
    }
    number.round().clamp(0.0, FULL_VOLUME as f64) as u32
}

fn read_limiter(master_table: &Table) -> Limiter {
    let read = Limiter {
        boost_db: read_f64(master_table, "boost_db", START_LIMITER.boost_db as f64) as f32,
        ceiling_db: read_f64(master_table, "ceiling_db", START_LIMITER.ceiling_db as f64) as f32,
        release_ms: read_count(master_table, "release_ms", START_LIMITER.release_ms),
    };
    limiter_within_ranges(read)
}

// The effects under `table`'s "effect" groups, at `place`.
fn read_chain(table: &Table, place: Place) -> Chain {
    let mut chain = empty_chain();
    for effect_table in groups(table, "effect") {
        let slot = read_count(effect_table, "slot", 0) as usize;
        if slot < 1 || slot > EFFECT_SLOTS {
            continue;
        }
        if chain[slot - 1].is_some() {
            continue;
        }
        if let Some(effect) = read_effect(effect_table, place) {
            chain[slot - 1] = Some(effect);
        }
    }
    chain
}

// An effect from its group, or None for a kind Jello doesn't know.
fn read_effect(effect_table: &Table, place: Place) -> Option<Slot> {
    let kind = kind_of_word(read_word(effect_table, "kind")?)?;
    let mut effect = Slot::new(kind, place);
    if let Some(Value::Boolean(on)) = effect_table.get("on") {
        effect.on = *on;
    }
    let values = values_of(kind, place.is_bus());
    for (index, value) in values.iter().enumerate() {
        let Some(found) = effect_table.get(&value.word) else {
            continue;
        };
        let Some(written) = written_in(found) else {
            continue;
        };
        if let Ok(number) = number_of(value, &written) {
            effect.values[index] = number;
        }
    }
    Some(effect)
}

// A TOML value as a written one, or None for a sort no effect value has.
fn written_in(value: &Value) -> Option<Written> {
    match value {
        Value::Integer(number) => Some(Written::Number(*number as f64)),
        Value::Float(number) => Some(Written::Number(*number)),
        Value::Boolean(on) => Some(Written::Switch(*on)),
        Value::String(word) => Some(Written::Word(word.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_modulation_round_trips_and_rejects_bad_links() {
        let mut mix = Mix::new(1);
        mix.buses.push(Bus::new(Some(String::from("Room"))));
        mix.buses[0].modulation.lfos[2].per_note = false;
        mix.master_modulation.lfos[1].hz = 2.5;
        let link = EffectLink { source: Source::Lfo(2), slot: 1, value: crate::engine::EQ_OUTPUT,
            amount: 0.1, one_way: true, scaled_by: Source::None };
        mix.tracks[0].links.push(link);
        mix.buses[0].modulation.links.push(link);
        mix.master_modulation.links.push(link);
        assert_eq!(round_trip(&mix), mix);

        let text = "[[track]]\n[[track.effect_link]]\nsource = 'lfo 9'\nslot = 1\nvalue = 0\namount = 1\none_way = false\nscaled_by = 'none'\n";
        let table: Table = text.parse().unwrap();
        let tracks = groups(&table, "track");
        assert!(read_mix(&table, &tracks).is_err());
    }
    use crate::engine::{EffectKind, Engine, LISTEN_TO_OWN};

    // A song file's table from `text`, and its [[track]] groups.
    fn mix_of_text(text: &str) -> Mix {
        let table: Table = text.parse().unwrap();
        let tracks = groups(&table, "track");
        read_mix(&table, &tracks).unwrap()
    }

    // `mix` written and read back, for `track_count` tracks.
    fn round_trip(mix: &Mix) -> Mix {
        let mut text = String::new();
        for track in 0..mix.tracks.len() {
            start_group(&mut text, "[[track]]");
            write_strip_values(&mut text, mix, track);
            write_strip_effects(&mut text, mix, track);
        }
        write_buses_and_master(&mut text, mix);
        mix_of_text(&text)
    }

    fn word(text: &str) -> Written {
        Written::Word(String::from(text))
    }

    #[test]
    fn a_whole_mix_reads_back_exactly_as_it_was_written() {
        let mut engine = Engine::for_render().unwrap();
        let mut mix = Mix::new(2);
        mix.add_bus(&mut engine, Some("Big \"room\"")).unwrap();
        mix.add_bus(&mut engine, None).unwrap();
        mix.set_pan(&mut engine, Place::Track(1), -35).unwrap();
        mix.set_send(&mut engine, 0, 1, 25).unwrap();
        mix.set_bus_volume(&mut engine, 0, 80).unwrap();
        mix.set_pan(&mut engine, Place::Bus(1), 60).unwrap();

        mix.place_effect(&mut engine, Place::Track(0), 2, EffectKind::Eq).unwrap();
        let eq = |mix: &mut Mix, engine: &mut Engine, value: &str, written: Written| {
            mix.set_effect_value(engine, Place::Track(0), 2, value, &written).unwrap();
        };
        eq(&mut mix, &mut engine, "band_2_on", Written::Switch(true));
        eq(&mut mix, &mut engine, "band_2_shape", word("low-shelf"));
        eq(&mut mix, &mut engine, "band_2_gain_db", Written::Number(-3.5));
        eq(&mut mix, &mut engine, "band_8_slope", Written::Number(48.0));
        mix.place_effect(&mut engine, Place::Track(1), 0, EffectKind::Compressor).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(1), 0, "ratio", &word("limit")).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(1), 0, "listens_to", &word("track 1")).unwrap();
        mix.set_effect_on(&mut engine, Place::Track(1), 0, false).unwrap();
        mix.place_effect(&mut engine, Place::Bus(0), 7, EffectKind::Reverb).unwrap();
        mix.set_effect_value(&mut engine, Place::Bus(0), 7, "decay_s", &Written::Number(4.2)).unwrap();
        mix.place_effect(&mut engine, Place::Master, 1, EffectKind::VolumeShaper).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 1, "point_2_bend", &Written::Number(-0.75)).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 1, "depth", &Written::Number(62.5)).unwrap();
        mix.place_effect(&mut engine, Place::Master, 2, EffectKind::Gate).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 2, "listens_to", &word("bus 2")).unwrap();
        let limiter = Limiter {
            boost_db: 4.5,
            ceiling_db: -0.3,
            release_ms: 250,
        };
        mix.set_limiter(&mut engine, limiter);

        assert_eq!(round_trip(&mix), mix);
    }

    #[test]
    fn only_values_that_differ_from_a_new_effect_s_are_written() {
        let mut engine = Engine::for_render().unwrap();
        let mut mix = Mix::new(1);
        mix.place_effect(&mut engine, Place::Track(0), 0, EffectKind::Delay).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(0), 0, "feedback", &Written::Number(50.0)).unwrap();
        let mut text = String::new();
        write_strip_effects(&mut text, &mix, 0);
        assert_eq!(text, "\n[[track.effect]]\nslot = 1\nkind = \"delay\"\non = true\nfeedback = 50\n");
    }

    #[test]
    fn a_new_song_s_mix_writes_only_the_pan_and_the_limiter() {
        let mix = Mix::new(1);
        let mut text = String::new();
        write_strip_values(&mut text, &mix, 0);
        write_buses_and_master(&mut text, &mix);
        assert_eq!(text, "pan = 0\n\n[master]\nboost_db = 0\nceiling_db = -1\nrelease_ms = 100\n");
        assert_eq!(round_trip(&mix), mix);
    }

    #[test]
    fn a_song_file_with_no_mix_reads_as_a_new_song_s() {
        let mix = mix_of_text("[[track]]\nvolume = 50\n[[track]]\n");
        assert_eq!(mix, Mix::new(2));
    }

    #[test]
    fn values_out_of_range_are_kept_within_them() {
        let text = "
            [[track]]
            pan = -900
            sends = [400, -5, 30]
            [[bus]]
            volume = 500
            pan = 101
            [master]
            boost_db = 99
            ceiling_db = 3
            release_ms = 0
        ";
        let mix = mix_of_text(text);
        assert_eq!(mix.tracks[0].pan, -100);
        // Only the one bus the song has gets a send.
        assert_eq!(mix.tracks[0].sends, [100, 0, 0, 0]);
        assert_eq!(mix.buses[0].volume, 100);
        assert_eq!(mix.buses[0].pan, 100);
        assert_eq!(mix.limiter.boost_db, 24.0);
        assert_eq!(mix.limiter.ceiling_db, 0.0);
        assert_eq!(mix.limiter.release_ms, 1);
    }

    #[test]
    fn effects_jello_cannot_read_are_skipped_and_bad_values_take_a_new_one_s() {
        let text = "
            [[track]]
            [[track.effect]]
            slot = 0
            kind = \"eq\"
            [[track.effect]]
            slot = 9
            kind = \"eq\"
            [[track.effect]]
            slot = 2
            kind = \"banjo\"
            [[track.effect]]
            kind = \"eq\"
            [[track.effect]]
            slot = 3
            kind = \"saturation\"
            on = \"yes\"
            curve = \"spicy\"
            drive_db = true
            tone = 0.5
            sparkle = 7
            [[track.effect]]
            slot = 3
            kind = \"reverb\"
        ";
        let mix = mix_of_text(text);
        let chain = &mix.tracks[0].chain;
        let used: Vec<usize> = (0..EFFECT_SLOTS).filter(|slot| chain[*slot].is_some()).collect();
        assert_eq!(used, vec![2]);
        let saturation = chain[2].as_ref().unwrap();
        assert_eq!(saturation.kind, EffectKind::Saturation);
        assert!(saturation.on);
        let mut expected = Slot::new(EffectKind::Saturation, Place::Track(0));
        expected.values[2] = 0.5;
        assert_eq!(*saturation, expected);
    }

    #[test]
    fn a_hand_written_listener_that_may_not_listen_or_would_loop_listens_to_its_own_input() {
        let text = "
            [[track]]
            [[track.effect]]
            slot = 1
            kind = \"compressor\"
            listens_to = \"track 2\"
            [[track.effect]]
            slot = 2
            kind = \"gate\"
            listens_to = \"bus 1\"
            [[track]]
            [[track.effect]]
            slot = 1
            kind = \"gate\"
            listens_to = \"track 1\"
            [[track.effect]]
            slot = 2
            kind = \"filter\"
            listens_to = \"track 5\"
        ";
        let mix = mix_of_text(text);
        let listens = |track: usize, slot: usize| mix.listens_to_word(Place::Track(track), slot);
        assert_eq!(listens(0, 0), Some(String::from("track 2")));
        assert_eq!(listens(0, 1), None);
        assert_eq!(listens(1, 0), None);
        assert_eq!(listens(1, 1), None);
        let gate = mix.tracks[1].chain[0].as_ref().unwrap();
        assert_eq!(gate.values[5], LISTEN_TO_OWN);
    }
}
