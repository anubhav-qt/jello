// The mix's values a section can move (docs/design.md#mix), alongside a
// track's sound (src/automation.rs). A track's own are named like its sound
// values: "pan", "send 1" for what it sends to bus 1, and "effect 2
// cutoff_hz" for a value of the effect in its slot 2. The song's own, moved
// with no track like the tempo: "bus 1 volume", "bus 1 pan", "bus 1 effect 3
// mix", "master effect 1 depth", and "limiter boost_db", "limiter
// ceiling_db" or "limiter release_ms".
//
// An effect's value is moved in the units the song file writes it in:
// percent for shares. Only numbers move; switches, choices, slopes and
// "listens to" don't.

use crate::effects::{EffectValue, Sort, Written, is_in_range, number_of, values_of, what_it_takes};
use crate::engine::{Engine, FULL_VOLUME, Limiter, PAN_MOST};
use crate::mix::{Mix, Place};

// One mix value as one engine message sets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MixPart {
    TrackPan(usize, i32),
    // Track, bus, percent.
    Send(usize, usize, u32),
    BusVolume(usize, u32),
    BusPan(usize, i32),
    // Value `value`, by its number in the C API, of the effect in slot
    // `slot` at `place`, in the engine's units.
    Effect { place: Place, slot: usize, value: i32, number: f32 },
    Limiter(Limiter),
}

// The words the song's own values start with, with no track.
const SONG_WIDE_STARTS: [&str; 3] = ["bus ", "master ", "limiter "];

// True if `name` names a value of the song's own mix, moved with no track.
pub fn is_song_wide(name: &str) -> bool {
    for start in SONG_WIDE_STARTS {
        if name.trim().starts_with(start) {
            return true;
        }
    }
    false
}

// True if `name` names a value of a track's mix rather than its sound.
pub fn is_track_mix_value(name: &str) -> bool {
    let first = name.split_whitespace().next();
    matches!(first, Some("pan") | Some("send") | Some("effect"))
}

// Removes a move of the removed bus and moves later bus numbers down. The
// bus is from 0; names in automation count from 1.
pub fn forget_bus(track: Option<usize>, name: &mut String, removed: usize) -> bool {
    let words: Vec<&str> = name.split_whitespace().collect();
    let bus_word = match (track, words.as_slice()) {
        (Some(_), ["send", bus]) => Some(*bus),
        (None, ["bus", bus, "volume"]) => Some(*bus),
        (None, ["bus", bus, "pan"]) => Some(*bus),
        (None, ["bus", bus, "effect", _, _]) => Some(*bus),
        _ => None,
    };
    let Some(bus_word) = bus_word else {
        return true;
    };
    let Ok(bus) = bus_word.parse::<usize>() else {
        return true;
    };
    if bus == removed + 1 {
        return false;
    }
    if bus > removed + 1 {
        let start = name.find(bus_word).unwrap();
        let end = start + bus_word.len();
        name.replace_range(start..end, &(bus - 1).to_string());
    }
    true
}

// What value `name` set to `number` changes in `mix`, on track `track`, or
// with None on the song's own mix, or why it can't: a name the mix doesn't
// have, a value that doesn't move, or a number outside its range.
pub fn mix_part_at(track: Option<usize>, name: &str, number: f64, mix: &Mix) -> Result<MixPart, String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    let Some(track) = track else {
        return song_wide_part(&words, name, number, mix);
    };
    if mix.chain(Place::Track(track)).is_none() {
        return Err(format!("the song has no track {}", track + 1));
    }
    match words.as_slice() {
        ["pan"] => Ok(MixPart::TrackPan(track, pan_of(name, number)?)),
        ["send", bus] => {
            let bus = bus_of(bus, name, mix)?;
            Ok(MixPart::Send(track, bus, percent_of(name, number)?))
        }
        ["effect", slot, value] => effect_part(Place::Track(track), slot, value, name, number, mix),
        _ => Err(format!("{name} is not a mix value, like \"pan\", \"send 1\" or \"effect 2 mix\"")),
    }
}

fn song_wide_part(words: &[&str], name: &str, number: f64, mix: &Mix) -> Result<MixPart, String> {
    match words {
        ["bus", bus, "volume"] => {
            let bus = bus_of(bus, name, mix)?;
            Ok(MixPart::BusVolume(bus, percent_of(name, number)?))
        }
        ["bus", bus, "pan"] => {
            let bus = bus_of(bus, name, mix)?;
            Ok(MixPart::BusPan(bus, pan_of(name, number)?))
        }
        ["bus", bus, "effect", slot, value] => {
            let bus = bus_of(bus, name, mix)?;
            effect_part(Place::Bus(bus), slot, value, name, number, mix)
        }
        ["master", "effect", slot, value] => effect_part(Place::Master, slot, value, name, number, mix),
        ["limiter", value] => limiter_part(value, name, number, mix.limiter),
        _ => Err(format!(
            "{name} is not a value of the song's mix, like \"bus 1 volume\" or \"master effect 1 mix\""
        )),
    }
}

// Bus `word`, counted from 1, as a bus of `mix` from 0.
fn bus_of(word: &str, name: &str, mix: &Mix) -> Result<usize, String> {
    let Ok(bus) = word.parse::<usize>() else {
        return Err(format!("{name} needs a bus number, like \"bus 1\""));
    };
    if bus < 1 || bus > mix.buses.len() {
        return Err(format!("the song has no bus {bus}"));
    }
    Ok(bus - 1)
}

fn pan_of(name: &str, number: f64) -> Result<i32, String> {
    let most = f64::from(PAN_MOST);
    if number < -most || number > most {
        return Err(format!("{name} is -{PAN_MOST} to {PAN_MOST}, not {number}"));
    }
    Ok(number.round() as i32)
}

fn percent_of(name: &str, number: f64) -> Result<u32, String> {
    if number < 0.0 || number > f64::from(FULL_VOLUME) {
        return Err(format!("{name} is 0 to {FULL_VOLUME}%, not {number}"));
    }
    Ok(number.round() as u32)
}

// Value `value` of the effect in slot `slot`, counted from 1, at `place`.
fn effect_part(place: Place, slot: &str, value: &str, name: &str, number: f64, mix: &Mix) -> Result<MixPart, String> {
    let Ok(slot) = slot.parse::<usize>() else {
        return Err(format!("{name} needs a slot number, like \"effect 2 mix\""));
    };
    if slot < 1 {
        return Err(format!("{name} needs a slot from 1"));
    }
    let effect = mix.slot(place, slot - 1)?;
    let values = values_of(effect.kind, place.is_bus());
    let Some(found) = values.iter().find(|each| each.word == value) else {
        return Err(format!("slot {slot} on {} has no {value}", place.name()));
    };
    if !moves(found) {
        return Err(format!("{name} is not a number to move"));
    }
    // A whole number takes `number` rounded, as a track's sound does.
    let mut given = number;
    if found.sort == Sort::Whole {
        given = number.round();
    }
    let written = Written::Number(given);
    if !is_in_range(found, &written) {
        return Err(format!("{name} takes {}, not {number}", what_it_takes(found)));
    }
    let engine_number = number_of(found, &written)?;
    Ok(MixPart::Effect {
        place,
        slot: slot - 1,
        value: found.number,
        number: engine_number,
    })
}

// Whether automation can move `value`: only numbers.
fn moves(value: &EffectValue) -> bool {
    matches!(value.sort, Sort::Number | Sort::Percent | Sort::Whole | Sort::Ratio)
}

fn limiter_part(value: &str, name: &str, number: f64, limiter: Limiter) -> Result<MixPart, String> {
    let mut moved = limiter;
    match value {
        "boost_db" => {
            if number < 0.0 || number > 24.0 {
                return Err(format!("{name} is 0 to 24, not {number}"));
            }
            moved.boost_db = number as f32;
        }
        "ceiling_db" => {
            if number < -12.0 || number > 0.0 {
                return Err(format!("{name} is -12 to 0, not {number}"));
            }
            moved.ceiling_db = number as f32;
        }
        "release_ms" => {
            if number < 1.0 || number > 1000.0 {
                return Err(format!("{name} is 1 to 1000, not {number}"));
            }
            moved.release_ms = number.round() as u32;
        }
        _ => return Err(format!("the limiter has boost_db, ceiling_db and release_ms, not {value}")),
    }
    Ok(MixPart::Limiter(moved))
}

// The part as `mix` itself has it, for putting it back after a section
// moved it.
pub fn own_mix_part(moved: &MixPart, mix: &Mix) -> MixPart {
    match *moved {
        MixPart::TrackPan(track, _) => MixPart::TrackPan(track, mix.tracks[track].pan),
        MixPart::Send(track, bus, _) => MixPart::Send(track, bus, mix.tracks[track].sends[bus]),
        MixPart::BusVolume(bus, _) => MixPart::BusVolume(bus, mix.buses[bus].volume),
        MixPart::BusPan(bus, _) => MixPart::BusPan(bus, mix.buses[bus].pan),
        MixPart::Effect { place, slot, value, .. } => {
            let mut number = 0.0;
            if let Ok(effect) = mix.slot(place, slot) {
                let values = values_of(effect.kind, place.is_bus());
                if let Some(index) = values.iter().position(|each| each.number == value) {
                    number = effect.values[index];
                }
            }
            MixPart::Effect {
                place,
                slot,
                value,
                number,
            }
        }
        MixPart::Limiter(_) => MixPart::Limiter(mix.limiter),
    }
}

// Sets `part` on `engine`.
pub fn send_mix_part(engine: &mut Engine, part: MixPart) {
    match part {
        MixPart::TrackPan(track, pan) => {
            engine.set_track_pan(track, pan);
        }
        MixPart::Send(track, bus, percent) => {
            engine.set_send(track, bus, percent);
        }
        MixPart::BusVolume(bus, percent) => {
            engine.set_bus_volume(bus, percent);
        }
        MixPart::BusPan(bus, pan) => {
            engine.set_bus_pan(bus, pan);
        }
        MixPart::Effect {
            place,
            slot,
            value,
            number,
        } => {
            engine.set_effect_value(place.chain(), slot, value, number);
        }
        MixPart::Limiter(limiter) => {
            engine.set_limiter(limiter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EQ_OUTPUT, EffectKind, REVERB_DECAY, START_LIMITER};

    // Two tracks and a bus, with an EQ on track 1, a reverb on the bus and
    // a compressor on the master.
    fn mix() -> Mix {
        let mut engine = Engine::for_render().unwrap();
        let mut mix = Mix::new(2);
        mix.add_bus(&mut engine, None).unwrap();
        mix.place_effect(&mut engine, Place::Track(0), 1, EffectKind::Eq).unwrap();
        mix.place_effect(&mut engine, Place::Bus(0), 0, EffectKind::Reverb).unwrap();
        mix.place_effect(&mut engine, Place::Master, 0, EffectKind::Compressor).unwrap();
        mix
    }

    #[test]
    fn a_track_s_pan_sends_and_effects_are_named_like_its_sound() {
        let mix = mix();
        assert_eq!(mix_part_at(Some(1), "pan", -40.4, &mix), Ok(MixPart::TrackPan(1, -40)));
        assert_eq!(mix_part_at(Some(0), "send 1", 25.0, &mix), Ok(MixPart::Send(0, 0, 25)));
        let output = mix_part_at(Some(0), "effect 2 output_db", -6.0, &mix);
        let expected = MixPart::Effect {
            place: Place::Track(0),
            slot: 1,
            value: EQ_OUTPUT,
            number: -6.0,
        };
        assert_eq!(output, Ok(expected));
        let gain = mix_part_at(Some(0), "effect 2 band_3_gain_db", 2.0, &mix).unwrap();
        assert!(matches!(gain, MixPart::Effect { .. }));
    }

    #[test]
    fn the_song_s_own_values_are_moved_with_no_track() {
        let mix = mix();
        assert_eq!(mix_part_at(None, "bus 1 volume", 70.0, &mix), Ok(MixPart::BusVolume(0, 70)));
        assert_eq!(mix_part_at(None, "bus 1 pan", 10.0, &mix), Ok(MixPart::BusPan(0, 10)));
        let decay = mix_part_at(None, "bus 1 effect 1 decay_s", 3.0, &mix);
        let expected = MixPart::Effect {
            place: Place::Bus(0),
            slot: 0,
            value: REVERB_DECAY,
            number: 3.0,
        };
        assert_eq!(decay, Ok(expected));
        let mix_share = mix_part_at(None, "master effect 1 mix", 50.0, &mix).unwrap();
        let MixPart::Effect { number, .. } = mix_share else {
            panic!("not an effect");
        };
        assert_eq!(number, 0.5);
        let boost = mix_part_at(None, "limiter boost_db", 3.0, &mix).unwrap();
        let mut limiter = START_LIMITER;
        limiter.boost_db = 3.0;
        assert_eq!(boost, MixPart::Limiter(limiter));
        assert!(is_song_wide("bus 1 volume"));
        assert!(is_song_wide("limiter ceiling_db"));
        assert!(!is_song_wide("tempo"));
        assert!(is_track_mix_value("effect 1 mix"));
        assert!(!is_track_mix_value("filter cutoff_hz"));
    }

    #[test]
    fn names_the_mix_lacks_values_that_do_not_move_and_ranges_are_refused() {
        let mix = mix();
        let refused = |track: Option<usize>, name: &str, number: f64| mix_part_at(track, name, number, &mix).unwrap_err();
        assert_eq!(refused(Some(2), "pan", 0.0), "the song has no track 3");
        assert_eq!(refused(Some(0), "pan", 101.0), "pan is -100 to 100, not 101");
        assert_eq!(refused(Some(0), "send 2", 10.0), "the song has no bus 2");
        assert_eq!(refused(Some(0), "send 1", -1.0), "send 1 is 0 to 100%, not -1");
        assert_eq!(refused(Some(0), "effect 1 mix", 10.0), "slot 1 on track 1 is empty");
        assert_eq!(refused(Some(0), "effect 2 sparkle", 10.0), "slot 2 on track 1 has no sparkle");
        assert_eq!(refused(Some(0), "effect 2 band_1_on", 1.0), "effect 2 band_1_on is not a number to move");
        assert_eq!(refused(Some(0), "effect 2 band_1_slope", 24.0), "effect 2 band_1_slope is not a number to move");
        assert_eq!(
            refused(Some(0), "effect 2 output_db", 30.0),
            "effect 2 output_db takes a number from -24 to 24, not 30"
        );
        assert_eq!(refused(None, "master effect 1 listens_to", 1.0), "master effect 1 listens_to is not a number to move");
        assert_eq!(refused(None, "limiter ceiling_db", 1.0), "limiter ceiling_db is -12 to 0, not 1");
        assert!(refused(None, "limiter sparkle", 1.0).starts_with("the limiter has"));
        assert!(refused(None, "bus one volume", 1.0).contains("needs a bus number"));
        assert!(refused(Some(0), "filter cutoff_hz", 1.0).contains("is not a mix value"));
    }

    #[test]
    fn the_own_part_is_the_mix_s() {
        let mut engine = Engine::for_render().unwrap();
        let mut mix = mix();
        mix.set_pan(&mut engine, Place::Track(1), 30).unwrap();
        let moved = mix_part_at(Some(1), "pan", -90.0, &mix).unwrap();
        assert_eq!(own_mix_part(&moved, &mix), MixPart::TrackPan(1, 30));
        let moved = mix_part_at(None, "bus 1 effect 1 decay_s", 9.0, &mix).unwrap();
        let MixPart::Effect { number, .. } = own_mix_part(&moved, &mix) else {
            panic!("not an effect");
        };
        assert_eq!(number, 1.5);
    }
}
