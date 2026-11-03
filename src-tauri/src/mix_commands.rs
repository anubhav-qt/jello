// What the Mix panel sends (docs/design.md#mix): the same changes
// jello_mix makes (src/mix_tool.rs), each answered with the whole mix as
// the panel shows it, and the meters it reads every frame. Places are words
// counted from 1, as jello_mix takes them: "track 2", "bus 1" or "master".
// A track's volume stays with the track: the panel sets it with
// set_track_volume.

use std::sync::Mutex;

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::State;

use crate::compose::Composer;
use crate::effects::{kind_of_word, values_of, word_of_kind, written_of};
use crate::engine::{CHAIN_COUNT, EFFECT_SLOTS, EffectKind, Engine, Limiter};
use crate::mix::{Chain, Place};
use crate::mix_tool::{json_of_written, written_of_json};

// The whole mix, as the panel shows it.
#[derive(Serialize, Debug)]
pub struct MixView {
    tracks: Vec<TrackStripView>,
    buses: Vec<BusView>,
    master: MasterView,
}

// A track's strip: its name and volume, from the track, and its pan, sends
// (one for each of the song's buses, in percent) and slots.
#[derive(Serialize, Debug)]
struct TrackStripView {
    name: String,
    volume: u32,
    pan: i32,
    sends: Vec<u32>,
    chain: Vec<Option<SlotView>>,
}

#[derive(Serialize, Debug)]
struct BusView {
    name: String,
    volume: u32,
    pan: i32,
    chain: Vec<Option<SlotView>>,
}

#[derive(Serialize, Debug)]
struct MasterView {
    chain: Vec<Option<SlotView>>,
    limiter: Limiter,
}

// One slot's effect: its kind's word, whether it is on, and every value by
// the word the song file uses, written as jello_mix writes them (shares in
// percent, choices as words).
#[derive(Serialize, Debug)]
struct SlotView {
    kind: String,
    on: bool,
    values: Map<String, Value>,
}

// What the meters read since the panel last asked: each strip's loudest
// sample as a level from 0, the tracks' first, then the buses', then the
// master's after the limiter; the output's true peak, also a level; the most
// the limiter turned it down, in dB; its loudness over the last 3 s, in LUFS;
// and how far each slot of the place asked about turns the sound down now,
// in dB.
#[derive(Serialize, Debug)]
pub struct MetersView {
    peaks: Vec<f32>,
    true_peak: f32,
    limiter_reduction_db: f32,
    short_term_lufs: f32,
    reductions: Vec<f32>,
}

// The mix as the panel shows it.
pub fn mix_view_of(composer: &Composer) -> MixView {
    let mix = composer.mix_now();
    let mut tracks = Vec::new();
    for (index, strip) in mix.tracks.iter().enumerate() {
        let track = &composer.tracks().list()[index];
        tracks.push(TrackStripView {
            name: track.name(index + 1),
            volume: track.volume(),
            pan: strip.pan,
            sends: strip.sends[..mix.buses.len()].to_vec(),
            chain: chain_view(&strip.chain, Place::Track(index)),
        });
    }
    let mut buses = Vec::new();
    for (index, bus) in mix.buses.iter().enumerate() {
        buses.push(BusView {
            name: mix.bus_name(index),
            volume: bus.volume,
            pan: bus.pan,
            chain: chain_view(&bus.chain, Place::Bus(index)),
        });
    }
    MixView {
        tracks,
        buses,
        master: MasterView {
            chain: chain_view(&mix.master, Place::Master),
            limiter: mix.limiter,
        },
    }
}

fn chain_view(chain: &Chain, place: Place) -> Vec<Option<SlotView>> {
    let mut slots = Vec::new();
    for held in chain {
        let Some(effect) = held else {
            slots.push(None);
            continue;
        };
        let mut values = Map::new();
        let described = values_of(effect.kind, place.is_bus());
        for (value, set) in described.iter().zip(&effect.values) {
            values.insert(value.word.clone(), json_of_written(&written_of(value, *set)));
        }
        let kind = word_of_kind(effect.kind).unwrap_or("none");
        slots.push(Some(SlotView {
            kind: String::from(kind),
            on: effect.on,
            values,
        }));
    }
    slots
}

// The place `word` names, which the song must have.
fn place_of(composer: &Composer, word: &str) -> Result<Place, String> {
    let Some(place) = Place::of_word(word) else {
        return Err(format!("there is no place called \"{word}\""));
    };
    if composer.mix_now().chain(place).is_none() {
        return Err(format!("the song has no {}", place.name()));
    }
    Ok(place)
}

// A slot counted from 1, returned from 0.
fn slot_of(slot: usize) -> Result<usize, String> {
    if slot < 1 || slot > EFFECT_SLOTS {
        return Err(format!("there are {EFFECT_SLOTS} slots, from 1 to {EFFECT_SLOTS}"));
    }
    Ok(slot - 1)
}

// A bus counted from 1, returned from 0.
fn bus_of(bus: usize) -> Result<usize, String> {
    if bus < 1 {
        return Err(String::from("buses count from 1"));
    }
    Ok(bus - 1)
}

// Puts a new effect of the kind `kind` names, or "none" to empty it, in
// slot `slot` at `place`.
pub fn place_effect(composer: &mut Composer, engine: &mut Engine, place: &str, slot: usize, kind: &str) -> Result<(), String> {
    let place = place_of(composer, place)?;
    let slot = slot_of(slot)?;
    let mut effect_kind = EffectKind::None;
    if kind != "none" {
        let Some(named) = kind_of_word(kind) else {
            return Err(format!("there is no effect called {kind}"));
        };
        effect_kind = named;
    }
    composer.mix_mut().place_effect(engine, place, slot, effect_kind)
}

// Moves slot `from`'s effect at `place` to slot `to`, shifting the slots
// between. Moving an empty slot does nothing.
pub fn move_effect(composer: &mut Composer, engine: &mut Engine, place: &str, from: usize, to: usize) -> Result<(), String> {
    let place = place_of(composer, place)?;
    let from = slot_of(from)?;
    let to = slot_of(to)?;
    composer.mix_now().slot(place, from)?;
    if from == to {
        return Ok(());
    }
    composer.mix_mut().move_effect(engine, place, from, to)
}

pub fn set_effect_on(composer: &mut Composer, engine: &mut Engine, place: &str, slot: usize, on: bool) -> Result<(), String> {
    let place = place_of(composer, place)?;
    let slot = slot_of(slot)?;
    composer.mix_mut().set_effect_on(engine, place, slot, on)
}

// Sets the value called `word` of slot `slot`'s effect at `place`, written
// as jello_mix writes it, kept within its range.
pub fn set_effect_value(
    composer: &mut Composer,
    engine: &mut Engine,
    place: &str,
    slot: usize,
    word: &str,
    value: &Value,
) -> Result<(), String> {
    let place = place_of(composer, place)?;
    let slot = slot_of(slot)?;
    let Some(written) = written_of_json(value) else {
        return Err(format!("{word} can't be {value}"));
    };
    composer.mix_mut().set_effect_value(engine, place, slot, word, &written)?;
    Ok(())
}

pub fn set_pan(composer: &mut Composer, engine: &mut Engine, place: &str, pan: i32) -> Result<(), String> {
    let place = place_of(composer, place)?;
    composer.mix_mut().set_pan(engine, place, pan)?;
    Ok(())
}

// Track `track` and bus `bus` count from 1.
pub fn set_send(composer: &mut Composer, engine: &mut Engine, track: usize, bus: usize, amount: u32) -> Result<(), String> {
    if track < 1 {
        return Err(String::from("tracks count from 1"));
    }
    let bus = bus_of(bus)?;
    composer.mix_mut().set_send(engine, track - 1, bus, amount)?;
    Ok(())
}

// The live meters, with the slots of `place` turning down.
pub fn meters_of(composer: &Composer, engine: &mut Engine, place: &str) -> Result<MetersView, String> {
    let place = place_of(composer, place)?;
    let meters = engine.take_meters();
    let mut reductions = Vec::new();
    for slot in 0..EFFECT_SLOTS {
        reductions.push(engine.gain_reduction_db(place.chain(), slot));
    }
    let mut peaks = Vec::new();
    for chain in 0..CHAIN_COUNT {
        peaks.push(meters.peaks[chain]);
    }
    Ok(MetersView {
        peaks,
        true_peak: meters.true_peak,
        limiter_reduction_db: meters.limiter_reduction_db,
        short_term_lufs: meters.short_term_lufs,
        reductions,
    })
}

// Locks the song, then the engine, starts the speakers, makes `change`,
// and answers with the whole mix.
fn changed(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    change: impl FnOnce(&mut Composer, &mut Engine) -> Result<(), String>,
) -> Result<MixView, String> {
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    change(&mut composer, &mut engine)?;
    Ok(mix_view_of(&composer))
}

#[tauri::command]
pub fn mix_view(composer: State<'_, Mutex<Composer>>) -> MixView {
    let composer = composer.lock().unwrap();
    mix_view_of(&composer)
}

#[tauri::command]
pub fn mix_meters(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
) -> Result<MetersView, String> {
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    meters_of(&composer, &mut engine, &place)
}

#[tauri::command]
pub fn set_mix_effect(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
    slot: usize,
    kind: String,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| place_effect(composer, engine, &place, slot, &kind))
}

#[tauri::command]
pub fn move_mix_effect(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
    from: usize,
    to: usize,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| move_effect(composer, engine, &place, from, to))
}

#[tauri::command]
pub fn set_mix_effect_on(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
    slot: usize,
    on: bool,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| set_effect_on(composer, engine, &place, slot, on))
}

#[tauri::command]
pub fn set_mix_effect_value(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
    slot: usize,
    word: String,
    value: Value,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| {
        set_effect_value(composer, engine, &place, slot, &word, &value)
    })
}

#[tauri::command]
pub fn set_mix_pan(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: String,
    pan: i32,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| set_pan(composer, engine, &place, pan))
}

#[tauri::command]
pub fn set_mix_send(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    track: usize,
    bus: usize,
    amount: u32,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| set_send(composer, engine, track, bus, amount))
}

#[tauri::command]
pub fn set_bus_volume(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    bus: usize,
    volume: u32,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| {
        let bus = bus_of(bus)?;
        composer.mix_mut().set_bus_volume(engine, bus, volume)?;
        Ok(())
    })
}

#[tauri::command]
pub fn add_bus(composer: State<'_, Mutex<Composer>>, engine: State<'_, Mutex<Engine>>) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| {
        composer.mix_mut().add_bus(engine, None)?;
        Ok(())
    })
}

#[tauri::command]
pub fn remove_bus(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    bus: usize,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| {
        let bus = bus_of(bus)?;
        composer.remove_bus(engine, bus)
    })
}

#[tauri::command]
pub fn rename_bus(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    bus: usize,
    name: String,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, _engine| {
        let bus = bus_of(bus)?;
        composer.mix_mut().rename_bus(bus, Some(&name))
    })
}

#[tauri::command]
pub fn set_limiter(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    limiter: Limiter,
) -> Result<MixView, String> {
    changed(composer, engine, |composer, engine| {
        composer.mix_mut().set_limiter(engine, limiter);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn studio() -> (Composer, Engine) {
        (Composer::new(), Engine::new().unwrap())
    }

    #[test]
    fn the_view_shows_every_slot_and_every_value_of_an_effect() {
        let (mut composer, mut engine) = studio();
        place_effect(&mut composer, &mut engine, "track 1", 2, "compressor").unwrap();
        let view = mix_view_of(&composer);
        let chain = &view.tracks[0].chain;
        assert_eq!(chain.len(), EFFECT_SLOTS);
        assert!(chain[0].is_none());
        let slot = chain[1].as_ref().unwrap();
        assert_eq!(slot.kind, "compressor");
        assert!(slot.on);
        assert_eq!(slot.values["ratio"], json!(4.0));
        assert_eq!(slot.values["mix"], json!(100.0));
        assert_eq!(slot.values["listens_to"], json!("own"));
        assert_eq!(view.master.limiter.ceiling_db, -1.0);
    }

    #[test]
    fn values_are_set_by_word_and_kept_within_range() {
        let (mut composer, mut engine) = studio();
        place_effect(&mut composer, &mut engine, "master", 1, "saturation").unwrap();
        set_effect_value(&mut composer, &mut engine, "master", 1, "drive_db", &json!(99)).unwrap();
        set_effect_value(&mut composer, &mut engine, "master", 1, "curve", &json!("fold")).unwrap();
        set_effect_value(&mut composer, &mut engine, "master", 1, "match", &json!(true)).unwrap();
        let view = mix_view_of(&composer);
        let values = &view.master.chain[0].as_ref().unwrap().values;
        assert_eq!(values["drive_db"], json!(36.0));
        assert_eq!(values["curve"], json!("fold"));
        assert_eq!(values["match"], json!(true));

        assert!(set_effect_value(&mut composer, &mut engine, "master", 1, "curve", &json!("banjo")).is_err());
        assert!(set_effect_value(&mut composer, &mut engine, "master", 1, "banjo", &json!(1)).is_err());
        assert!(set_effect_value(&mut composer, &mut engine, "master", 2, "drive_db", &json!(1)).is_err());
        assert!(set_effect_value(&mut composer, &mut engine, "master", 1, "drive_db", &json!(null)).is_err());
    }

    #[test]
    fn places_slots_and_kinds_that_don_t_exist_are_refused() {
        let (mut composer, mut engine) = studio();
        assert!(place_effect(&mut composer, &mut engine, "track 0", 1, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "track 9", 1, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "bus 1", 1, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "drums", 1, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "track 1", 0, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "track 1", 9, "eq").is_err());
        assert!(place_effect(&mut composer, &mut engine, "track 1", 1, "banjo").is_err());
        assert!(mix_view_of(&composer).tracks[0].chain.iter().all(Option::is_none));
    }

    #[test]
    fn effects_move_switch_off_and_empty() {
        let (mut composer, mut engine) = studio();
        place_effect(&mut composer, &mut engine, "track 1", 1, "eq").unwrap();
        place_effect(&mut composer, &mut engine, "track 1", 2, "delay").unwrap();
        move_effect(&mut composer, &mut engine, "track 1", 2, 1).unwrap();
        set_effect_on(&mut composer, &mut engine, "track 1", 2, false).unwrap();
        let view = mix_view_of(&composer);
        let chain = &view.tracks[0].chain;
        assert_eq!(chain[0].as_ref().unwrap().kind, "delay");
        assert_eq!(chain[1].as_ref().unwrap().kind, "eq");
        assert!(!chain[1].as_ref().unwrap().on);

        // Moving onto itself or moving an empty slot changes nothing.
        move_effect(&mut composer, &mut engine, "track 1", 1, 1).unwrap();
        assert!(move_effect(&mut composer, &mut engine, "track 1", 5, 1).is_err());

        place_effect(&mut composer, &mut engine, "track 1", 1, "none").unwrap();
        assert!(mix_view_of(&composer).tracks[0].chain[0].is_none());
    }

    #[test]
    fn buses_come_and_go_with_their_sends() {
        let (mut composer, mut engine) = studio();
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        set_send(&mut composer, &mut engine, 1, 1, 30).unwrap();
        set_pan(&mut composer, &mut engine, "bus 1", -40).unwrap();
        let view = mix_view_of(&composer);
        assert_eq!(view.buses.len(), 1);
        assert_eq!(view.buses[0].name, "Bus 1");
        assert_eq!(view.buses[0].pan, -40);
        assert_eq!(view.tracks[0].sends, vec![30]);
        assert!(set_send(&mut composer, &mut engine, 1, 2, 30).is_err());
        assert!(set_send(&mut composer, &mut engine, 0, 1, 30).is_err());
        assert!(set_pan(&mut composer, &mut engine, "master", 10).is_err());

        composer.remove_bus(&mut engine, 0).unwrap();
        let view = mix_view_of(&composer);
        assert!(view.buses.is_empty());
        assert!(view.tracks[0].sends.is_empty());
    }

    #[test]
    fn the_meters_read_every_strip_and_the_asked_place_s_slots() {
        let (composer, mut engine) = studio();
        let meters = meters_of(&composer, &mut engine, "master").unwrap();
        assert_eq!(meters.peaks.len(), CHAIN_COUNT);
        assert_eq!(meters.reductions.len(), EFFECT_SLOTS);
        assert!(meters_of(&composer, &mut engine, "bus 3").is_err());
    }
}
