// A section's automation (docs/design.md#tracks-and-sections): a value
// moving in a straight line from a start to an end across the section. A
// value is named as jello_sound names it in its answers: "volume" for the
// track's volume, "tuning_hz" or "glide_ms", a part and a value like
// "filter cutoff_hz" or "loudness attack_ms", or a numbered part like
// "oscillator 2 level" or "lfo 1 depth". The tempo's automation is the
// arrangement's own (src/arrange.rs).
use serde_json::{Value, json};

use crate::engine::{Engine, Filter, FilterEnvelope, LoudnessEnvelope, Oscillator, Unison};
use crate::modulation::{Connection, Lfo, Source, VibratoKeyTimes};
use crate::sound_changes::changed;
use crate::sound_file::Sound;

// The word for a track's volume.
pub const VOLUME_WORD: &str = "volume";
// A track's volume goes from 0 to this, in percent.
const FULL_VOLUME: f64 = 100.0;

// One part of a track's sound, or its volume, as one engine message sets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Volume(u32),
    // With its index, from 0.
    Oscillator(usize, Oscillator),
    Loudness(LoudnessEnvelope),
    Filter(Filter),
    FilterEnvelope(FilterEnvelope),
    KeyTracking(u32),
    Unison(Unison),
    Tuning(f32),
    Lfo(usize, Lfo),
    Connection(usize, Connection),
    VibratoKey(VibratoKeyTimes),
    Glide(u32),
}

// What value `name` set to `number` changes on a track with `sound`, or why
// it can't: an unknown name, a value that isn't a number, or one outside its
// range. A whole-number value takes `number` rounded.
pub fn part_at(name: &str, number: f64, sound: &Sound) -> Result<Part, String> {
    if name == VOLUME_WORD {
        if !(0.0..=FULL_VOLUME).contains(&number) {
            return Err(format!("volume is 0 to 100%, not {number}"));
        }
        return Ok(Part::Volume(number.round() as u32));
    }
    let change = change_of(name, number)?;
    let after = match changed(sound, &change) {
        Ok(after) => after,
        Err(first) => {
            let rounded = change_of(name, number.round())?;
            match changed(sound, &rounded) {
                Ok(after) => after,
                Err(_) => return Err(first),
            }
        }
    };
    if after.instruments != sound.instruments {
        return Err(format!("{name} is not a number to move"));
    }
    match part_changed(sound, &after) {
        Some(part) => Ok(part),
        None => Ok(same_part(name, sound)?),
    }
}

// The change jello_sound reads for value `name` set to `number`.
fn change_of(name: &str, number: f64) -> Result<Value, String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() == 1 {
        return Ok(json!({ words[0]: number }));
    }
    if words.len() == 2 {
        return Ok(json!({ words[0]: { words[1]: number } }));
    }
    if words.len() == 3 {
        let Ok(place) = words[1].parse::<u32>() else {
            return Err(format!("{name} is not a value: a numbered part is like \"oscillator 2 level\""));
        };
        let list = match words[0] {
            "oscillator" => "oscillators",
            "lfo" => "lfos",
            "connection" => "connections",
            _ => return Err(format!("{name} is not a value: only oscillator, lfo and connection are numbered")),
        };
        return Ok(json!({ list: [{ "number": place, words[2]: number }] }));
    }
    Err(format!("{name} is not a value, like \"filter cutoff_hz\" or \"oscillator 2 level\""))
}

// The part of `before` that differs in `after`, or None if they are the
// same. A value names one part, so at most one differs.
pub fn part_changed(before: &Sound, after: &Sound) -> Option<Part> {
    for index in 0..before.oscillators.len() {
        if before.oscillators[index] != after.oscillators[index] {
            return Some(Part::Oscillator(index, after.oscillators[index]));
        }
    }
    if before.loudness_envelope != after.loudness_envelope {
        return Some(Part::Loudness(after.loudness_envelope));
    }
    if before.filter != after.filter {
        return Some(Part::Filter(after.filter));
    }
    if before.filter_envelope != after.filter_envelope {
        return Some(Part::FilterEnvelope(after.filter_envelope));
    }
    if before.key_tracking != after.key_tracking {
        return Some(Part::KeyTracking(after.key_tracking));
    }
    if before.unison != after.unison {
        return Some(Part::Unison(after.unison));
    }
    if before.tuning_hz != after.tuning_hz {
        return Some(Part::Tuning(after.tuning_hz));
    }
    for index in 0..before.lfos.len() {
        if before.lfos[index] != after.lfos[index] {
            return Some(Part::Lfo(index, after.lfos[index]));
        }
    }
    for index in 0..before.connections.len() {
        if before.connections[index] != after.connections[index] {
            return Some(Part::Connection(index, after.connections[index]));
        }
    }
    if before.vibrato_key_times != after.vibrato_key_times {
        return Some(Part::VibratoKey(after.vibrato_key_times));
    }
    if before.glide_ms != after.glide_ms {
        return Some(Part::Glide(after.glide_ms));
    }
    None
}

// The part value `name` belongs to, as `sound` has it: for a value set to
// what it already is.
fn same_part(name: &str, sound: &Sound) -> Result<Part, String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    let place = words.get(1).and_then(|word| word.parse::<usize>().ok()).unwrap_or(1);
    let index = place.saturating_sub(1);
    let part = match words[0] {
        "tuning_hz" => Part::Tuning(sound.tuning_hz),
        "glide_ms" => Part::Glide(sound.glide_ms),
        "oscillator" => Part::Oscillator(index, sound.oscillators[index]),
        "loudness" => Part::Loudness(sound.loudness_envelope),
        "filter" => {
            if words.get(1) == Some(&"key_tracking") {
                Part::KeyTracking(sound.key_tracking)
            } else {
                Part::Filter(sound.filter)
            }
        }
        "filter_envelope" => Part::FilterEnvelope(sound.filter_envelope),
        "unison" => Part::Unison(sound.unison),
        "vibrato_key" => Part::VibratoKey(sound.vibrato_key_times),
        "lfo" => Part::Lfo(index, sound.lfos[index]),
        "connection" => Part::Connection(index, sound.connections[index]),
        _ => return Err(format!("{name} is not a value")),
    };
    Ok(part)
}

// The part of track `track` as its own sound and volume have it, for
// putting it back after a section moved it.
pub fn own_part(moved: &Part, sound: &Sound, volume: u32) -> Part {
    match moved {
        Part::Volume(_) => Part::Volume(volume),
        Part::Oscillator(index, _) => Part::Oscillator(*index, sound.oscillators[*index]),
        Part::Loudness(_) => Part::Loudness(sound.loudness_envelope),
        Part::Filter(_) => Part::Filter(sound.filter),
        Part::FilterEnvelope(_) => Part::FilterEnvelope(sound.filter_envelope),
        Part::KeyTracking(_) => Part::KeyTracking(sound.key_tracking),
        Part::Unison(_) => Part::Unison(sound.unison),
        Part::Tuning(_) => Part::Tuning(sound.tuning_hz),
        Part::Lfo(index, _) => Part::Lfo(*index, sound.lfos[*index]),
        Part::Connection(index, _) => Part::Connection(*index, sound.connections[*index]),
        Part::VibratoKey(_) => Part::VibratoKey(sound.vibrato_key_times),
        Part::Glide(_) => Part::Glide(sound.glide_ms),
    }
}

// Sets `part` on track `track` of `engine`.
pub fn send_part(engine: &mut Engine, track: usize, part: Part) {
    if let Part::Volume(percent) = part {
        engine.set_track_volume(track, percent);
        return;
    }
    // An unused sound connection does not own an engine slot. That slot
    // may currently hold an effect link, so there is nothing to send.
    if let Part::Connection(_, connection) = part {
        if connection.source == Source::None {
            return;
        }
    }
    engine.on_track(track, |engine| match part {
        Part::Volume(_) => {}
        Part::Oscillator(index, oscillator) => {
            engine.set_oscillator(index, oscillator);
        }
        Part::Loudness(envelope) => {
            engine.set_loudness_envelope(envelope);
        }
        Part::Filter(filter) => {
            engine.set_filter(filter);
        }
        Part::FilterEnvelope(envelope) => {
            engine.set_filter_envelope(envelope);
        }
        Part::KeyTracking(key_tracking) => {
            engine.set_key_tracking(key_tracking);
        }
        Part::Unison(unison) => {
            engine.set_unison(unison);
        }
        Part::Tuning(a4_hz) => {
            engine.set_tuning(a4_hz);
        }
        Part::Lfo(index, lfo) => {
            engine.set_lfo(index, lfo);
        }
        Part::Connection(index, connection) => {
            engine.set_connection(index, connection);
        }
        Part::VibratoKey(times) => {
            engine.set_vibrato_key_times(times);
        }
        Part::Glide(glide_ms) => {
            engine.set_glide_time(glide_ms);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_profile::SoundProfile;

    fn sound() -> Sound {
        SoundProfile::new().sound()
    }

    #[test]
    fn an_unused_sound_connection_does_not_clear_an_effect_link() {
        use crate::engine::{EffectConnection, EffectKind, EQ_OUTPUT};
        use crate::modulation::{LfoShape, NO_CONNECTION, START_LFO};
        let mut actual = Engine::for_render().unwrap();
        let mut expected = Engine::for_render().unwrap();
        let mut lfo = START_LFO;
        lfo.per_note = false;
        lfo.shape = LfoShape::Steps;
        lfo.step_count = 1;
        lfo.steps[0] = 100.0;
        let link = EffectConnection { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: -6.0, one_way: false, scaled_by: Source::None };
        for engine in [&mut actual, &mut expected] {
            engine.set_effect(0, 0, EffectKind::Eq);
            engine.set_chain_lfo(0, 0, lfo);
            engine.set_effect_connection(0, 3, link);
        }
        send_part(&mut actual, 0, Part::Connection(3, NO_CONNECTION));
        actual.note_on(69, 100);
        expected.note_on(69, 100);
        assert_eq!(actual.render(4096), expected.render(4096));
    }

    #[test]
    fn a_value_is_named_as_jello_sound_names_it() {
        let sound = sound();
        let Part::Filter(filter) = part_at("filter cutoff_hz", 800.0, &sound).unwrap() else {
            panic!("not the filter");
        };
        assert_eq!(filter.cutoff_hz, 800.0);

        let Part::Oscillator(index, oscillator) = part_at("oscillator 2 level", 40.0, &sound).unwrap() else {
            panic!("not an oscillator");
        };
        assert_eq!(index, 1);
        assert_eq!(oscillator.level, 40);

        assert_eq!(part_at("filter key_tracking", 30.0, &sound), Ok(Part::KeyTracking(30)));
        assert_eq!(part_at("glide_ms", 250.0, &sound), Ok(Part::Glide(250)));
        assert_eq!(part_at("volume", 42.4, &sound), Ok(Part::Volume(42)));
    }

    #[test]
    fn a_whole_number_value_takes_the_number_rounded() {
        let sound = sound();
        let Part::Loudness(envelope) = part_at("loudness attack_ms", 120.6, &sound).unwrap() else {
            panic!("not the loudness");
        };
        assert_eq!(envelope.attack_ms, 121);
    }

    #[test]
    fn a_value_set_to_what_it_is_still_names_its_part() {
        let sound = sound();
        let cutoff = sound.filter.cutoff_hz as f64;
        assert_eq!(part_at("filter cutoff_hz", cutoff, &sound), Ok(Part::Filter(sound.filter)));
        let level = sound.oscillators[0].level as f64;
        assert_eq!(
            part_at("oscillator 1 level", level, &sound),
            Ok(Part::Oscillator(0, sound.oscillators[0]))
        );
    }

    #[test]
    fn unknown_names_words_and_ranges_are_refused() {
        let sound = sound();
        assert_eq!(
            part_at("filter cutoff_hz", 50_000.0, &sound),
            Err(String::from("filter cutoff_hz is 20 to 20000 Hz, not 50000"))
        );
        assert_eq!(part_at("volume", 101.0, &sound), Err(String::from("volume is 0 to 100%, not 101")));
        assert!(part_at("filter loudness", 1.0, &sound).is_err());
        assert!(part_at("oscillator two level", 1.0, &sound).is_err());
        assert!(part_at("delay 1 time", 1.0, &sound).is_err());
        assert!(part_at("oscillator 9 level", 1.0, &sound).is_err());
        assert!(part_at("oscillator 1 kind", 1.0, &sound).is_err());
        assert!(part_at("", 1.0, &sound).is_err());
    }

    #[test]
    fn the_own_part_is_the_track_s() {
        let sound = sound();
        let moved = part_at("filter cutoff_hz", 800.0, &sound).unwrap();
        assert_eq!(own_part(&moved, &sound, 70), Part::Filter(sound.filter));
        assert_eq!(own_part(&Part::Volume(10), &sound, 70), Part::Volume(70));
    }
}
