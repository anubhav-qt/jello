// Named commands the UI can call (docs/adr/0003-every-action-is-a-command.md).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::compose::{ComposeView, Composer};
use crate::engine::{
    BEATS_PER_BAR, Engine, Filter, FilterEnvelope, LoopLayer, LoopNote, LooperState, LooperStatus, LoudnessEnvelope,
    Unison, VIBRATO_KEY_NOTE,
};
use crate::harmony::{Key, Mode};
use crate::key_map::{KeyAction, KeyMap, LAPTOP_VELOCITY, LayoutForUi, layer_number_of};
use crate::modulation::{
    CONNECTION_COUNT, Connection, LFO_COUNT, LFO_MOST_STEPS, Lfo, LfoShape, LfoSpeed, Source, Target,
    VibratoKeyTimes,
};
use crate::sound_profile::SoundProfile;
use crate::wav::write_wav;

// The test file holds middle C at velocity 100 for 2 seconds, then lets go.
// It keeps going for 400 ms after that: the 300 ms fade-out plus a little
// silence, so it ends cleanly.
const MIDDLE_C: u8 = 60;
const TEST_VELOCITY: u8 = 100;
const HOLD_MS: u32 = 2000;
const TAIL_MS: u32 = 400;

// What the screen shows about the sound: the wave new notes play, the song's
// tempo in beats per minute, whether loops snap to the beat, what the looper
// is doing, the loudness envelope, where the filter is, unison's copies, the
// tuning, the LFOs, the routing list and the vibrato key's times. Everything
// but the looper and snap comes from the song: its tempo and its sound
// profile.
#[derive(Serialize)]
pub struct SoundView {
    wave: &'static str,
    tempo: u32,
    snap: bool,
    looper: LooperView,
    loudness: LoudnessView,
    filter: FilterView,
    unison: UnisonView,
    // A4's pitch in Hz, which every other note is tuned from.
    tuning_hz: f32,
    // All 8 LFOs and all 64 connections, in order, and the vibrato key's
    // times (docs/adr/0031-lfos-and-a-routing-list.md).
    lfos: Vec<LfoView>,
    connections: Vec<ConnectionView>,
    vibrato_key: VibratoKeyView,
}

// One LFO, as the screen shows it and sends it back to `set_lfo`. Its shape
// and speed are named: "sine", "saw up", "hz", "presses" and so on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LfoView {
    shape: String,
    // For the steps shape: how many steps play, and every step's height in
    // percent, from -100 to 100. It always lists LFO_MOST_STEPS of them.
    step_count: u32,
    steps: Vec<f32>,
    per_note: bool,
    speed: String,
    hz: f64,
    beats: f64,
    presses: u32,
    // In percent.
    start: u32,
    spread: u32,
    delay_ms: u32,
    fade_ms: u32,
    delay_beats: f64,
    fade_beats: f64,
    // In percent.
    smooth: u32,
    once: bool,
}

fn lfo_view_of(lfo: &Lfo) -> LfoView {
    LfoView {
        shape: String::from(lfo.shape.name()),
        step_count: lfo.step_count,
        steps: lfo.steps.to_vec(),
        per_note: lfo.per_note,
        speed: String::from(lfo.speed.name()),
        hz: lfo.hz,
        beats: lfo.beats,
        presses: lfo.presses,
        start: lfo.start,
        spread: lfo.spread,
        delay_ms: lfo.delay_ms,
        fade_ms: lfo.fade_ms,
        delay_beats: lfo.delay_beats,
        fade_beats: lfo.fade_beats,
        smooth: lfo.smooth,
        once: lfo.once,
    }
}

// The LFO `view` describes. Steps past LFO_MOST_STEPS are left out, and
// missing ones are 0. Fails on a shape or speed with no such name.
fn lfo_of_view(view: &LfoView) -> Result<Lfo, String> {
    let shape = match LfoShape::from_name(&view.shape) {
        Some(shape) => shape,
        None => return Err(format!("there is no LFO shape called {}", view.shape)),
    };
    let speed = match LfoSpeed::from_name(&view.speed) {
        Some(speed) => speed,
        None => return Err(format!("there is no LFO speed called {}", view.speed)),
    };
    let mut steps = [0.0; LFO_MOST_STEPS];
    for (step, height) in steps.iter_mut().zip(&view.steps) {
        *step = *height;
    }
    let lfo = Lfo {
        shape,
        step_count: view.step_count,
        steps,
        per_note: view.per_note,
        speed,
        hz: view.hz,
        beats: view.beats,
        presses: view.presses,
        start: view.start,
        spread: view.spread,
        delay_ms: view.delay_ms,
        fade_ms: view.fade_ms,
        delay_beats: view.delay_beats,
        fade_beats: view.fade_beats,
        smooth: view.smooth,
        once: view.once,
    };
    Ok(lfo)
}

// One connection of the routing list, as the screen shows it and sends it
// back to `set_connection`. Its sources and target are named, like "lfo 1",
// "vibrato key" or "cutoff", and an unused one's source is "none". Its
// amount is in the target's own unit, named in `unit`, which the screen need
// not send back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionView {
    source: String,
    target: String,
    amount: f32,
    one_way: bool,
    snap: bool,
    scaled_by: String,
    #[serde(default)]
    unit: String,
}

fn connection_view_of(connection: &Connection) -> ConnectionView {
    ConnectionView {
        source: connection.source.name(),
        target: connection.target.name(),
        amount: connection.amount,
        one_way: connection.one_way,
        snap: connection.snap,
        scaled_by: connection.scaled_by.name(),
        unit: String::from(connection.target.unit().name()),
    }
}

// The connection `view` describes. Fails on a source or target with no such
// name.
fn connection_of_view(view: &ConnectionView) -> Result<Connection, String> {
    let source = source_named(&view.source)?;
    let scaled_by = source_named(&view.scaled_by)?;
    let target = match Target::from_name(&view.target) {
        Some(target) => target,
        None => return Err(format!("there is no target called {}", view.target)),
    };
    let connection = Connection {
        source,
        target,
        amount: view.amount,
        one_way: view.one_way,
        snap: view.snap,
        scaled_by,
    };
    Ok(connection)
}

fn source_named(name: &str) -> Result<Source, String> {
    match Source::from_name(name) {
        Some(source) => Ok(source),
        None => Err(format!("there is no source called {name}")),
    }
}

// How long the vibrato key takes to rise when held and fall when let go.
#[derive(Serialize)]
pub struct VibratoKeyView {
    rise_ms: u32,
    fall_ms: u32,
}

// The loudness envelope new notes start with.
#[derive(Serialize)]
pub struct LoudnessView {
    attack_ms: u32,
    decay_ms: u32,
    // In percent of the peak.
    sustain: u32,
    release_ms: u32,
}

fn loudness_view_of(sound_profile: &SoundProfile) -> LoudnessView {
    let envelope = sound_profile.loudness_envelope();
    LoudnessView {
        attack_ms: envelope.attack_ms,
        decay_ms: envelope.decay_ms,
        sustain: envelope.sustain,
        release_ms: envelope.release_ms,
    }
}

// The copies new notes are played by (docs/adr/0029-unison.md).
#[derive(Serialize)]
pub struct UnisonView {
    // From 1, which is off, to 7.
    copies: u32,
    // How far the highest copy is tuned above the note, in cents.
    detune_cents: u32,
    // How far apart they sit between the speakers, in percent.
    spread: u32,
}

fn unison_view_of(sound_profile: &SoundProfile) -> UnisonView {
    let unison = sound_profile.unison();
    UnisonView {
        copies: unison.copies,
        detune_cents: unison.detune_cents,
        spread: unison.spread,
    }
}

// Where the low-pass filter and its envelope are
// (docs/adr/0028-a-low-pass-filter.md).
#[derive(Serialize)]
pub struct FilterView {
    // Middle C's cutoff, in Hz. At 20000 the filter is open: off.
    cutoff_hz: f32,
    // In percent.
    resonance: u32,
    // How far the filter envelope opens the cutoff, in octaves.
    amount: f32,
    attack_ms: u32,
    decay_ms: u32,
    // In percent of the amount.
    sustain: u32,
    release_ms: u32,
    // How much each new note's cutoff follows middle C's, in percent.
    key_tracking: u32,
}

fn filter_view_of(sound_profile: &SoundProfile) -> FilterView {
    let filter = sound_profile.filter();
    let envelope = sound_profile.filter_envelope();
    FilterView {
        cutoff_hz: filter.cutoff_hz,
        resonance: filter.resonance,
        amount: envelope.amount,
        attack_ms: envelope.attack_ms,
        decay_ms: envelope.decay_ms,
        sustain: envelope.sustain,
        release_ms: envelope.release_ms,
        key_tracking: sound_profile.key_tracking(),
    }
}

// What the looper is doing.
#[derive(Serialize)]
pub struct LooperView {
    // "idle", "counting in", "recording", "looping" or "layering": playing
    // the loop and recording a layer over it.
    state: &'static str,
    // While counting in, the beat (1 to 4). While recording, the bar being
    // recorded, from 1. While looping or layering, how many bars the loop
    // lasts.
    number: u32,
    // True from the first press until the loop is cleared. The UI asks
    // again and again while it is, since the state changes on the beat.
    busy: bool,
    // How many beats into the count-in, the recording or the repeat playing
    // now, as of the engine's last chunk of sound. Each starts on beat 1 of
    // a bar. While looping or layering, it stays inside the loop. 0 when
    // idle.
    beats_in: f64,
    // The loop's layers while recording, looping or layering, oldest first,
    // for the lane under the keyboard: a box each
    // (docs/adr/0026-layers.md). Empty otherwise.
    layers: Vec<LayerView>,
    // While Esc waits for a layer's number, the layer it removes if none
    // comes: the newest, counted from 1. None (null for the UI) otherwise.
    removing: Option<u32>,
    // A message about the looper for the top bar, like "8 layers is the
    // most, Esc removes one". It comes in one answer only; None (null)
    // otherwise.
    message: Option<String>,
}

// One layer of the loop.
#[derive(Debug, PartialEq, Serialize)]
pub struct LayerView {
    // Its notes. While it is recorded, as played, in the order they
    // started. Once it plays, where each time around plays them, in the
    // order they start.
    notes: Vec<LoopNoteView>,
    // How loud it plays, in percent, from 0 to 100.
    volume: u32,
    // True while it is being recorded.
    recording: bool,
}

// One note of the loop, in beats from the loop's start.
#[derive(Debug, PartialEq, Serialize)]
pub struct LoopNoteView {
    note: u8,
    start: f64,
    // None (null for the UI) while its key is still held.
    end: Option<f64>,
}

// What the screen shows about the sound at `now`, with the song in
// `composer`. Esc's half second may be up, so that comes first.
fn sound_view_of(engine: &mut Engine, composer: &Composer, now: Instant) -> SoundView {
    engine.finish_esc_wait(now);

    let sound_profile = composer.sound_profile();
    let wave = sound_profile.wave();
    // The beat plays at the song's tempo. The loop's boxes are placed by the
    // beat's own, which is where the engine played the loop.
    let tempo = engine.tempo();
    let sample_rate = engine.sample_rate();
    let status = engine.looper_status();
    let busy = engine.is_looper_busy();
    let position = engine.looper_position();
    let mut layers = engine.loop_layers().to_vec();
    // The vibrato key's presses are kept with a layer's notes, but the lane
    // leaves them out until its design says how to draw them.
    for layer in &mut layers {
        layer.notes.retain(|note| note.note != VIBRATO_KEY_NOTE);
    }
    // Once a layer plays, the lane shows each note where the loop plays it.
    if is_loop_playing(status.state) {
        for layer in &mut layers {
            if !layer.recording {
                layer.notes = engine.plan_loop(&layer.notes, status.number);
            }
        }
    }

    let mut looper = looper_view_of(status, busy, position, &layers, tempo, sample_rate);
    looper.removing = engine.esc_removes(now);
    looper.message = engine.take_looper_message();

    let mut lfos = Vec::new();
    for lfo in sound_profile.lfos() {
        lfos.push(lfo_view_of(lfo));
    }
    let mut connections = Vec::new();
    for connection in sound_profile.connections() {
        connections.push(connection_view_of(connection));
    }
    let key_times = sound_profile.vibrato_key_times();

    SoundView {
        wave: wave.name(),
        tempo: composer.tempo(),
        snap: engine.snap(),
        looper,
        loudness: loudness_view_of(sound_profile),
        filter: filter_view_of(sound_profile),
        unison: unison_view_of(sound_profile),
        tuning_hz: sound_profile.tuning_hz(),
        lfos,
        connections,
        vibrato_key: VibratoKeyView {
            rise_ms: key_times.rise_ms,
            fall_ms: key_times.fall_ms,
        },
    }
}

// True while the loop plays: looping, or recording a layer over it.
fn is_loop_playing(state: LooperState) -> bool {
    match state {
        LooperState::Looping => true,
        LooperState::Layering => true,
        _ => false,
    }
}

// What the screen shows about the looper. `position` is how many samples
// into the count-in, the recording or the repeat playing now it is. Esc and
// messages are left for the caller to fill in.
fn looper_view_of(
    status: LooperStatus,
    busy: bool,
    position: u64,
    layers: &[LoopLayer],
    tempo: u32,
    sample_rate: u32,
) -> LooperView {
    let mut beats_in = beats_of(position, tempo, sample_rate);

    // Rust reads the state and the place one after the other, so right as
    // the loop closes it can see "looping" with the recording's place, past
    // the loop's end. The loop closes on a bar line, so wrapping the place
    // into the loop finds the same spot in the bar.
    if is_loop_playing(status.state) {
        let loop_beats = f64::from(status.number * BEATS_PER_BAR);
        if loop_beats > 0.0 {
            beats_in %= loop_beats;
        }
    }

    // Before recording starts there is nothing to show yet.
    let shows_layers = match status.state {
        LooperState::Recording => true,
        LooperState::Looping => true,
        LooperState::Layering => true,
        _ => false,
    };
    let mut layer_views = Vec::new();
    if shows_layers {
        for layer in layers {
            let view = LayerView {
                notes: note_views_of(&layer.notes, tempo, sample_rate),
                volume: layer.volume,
                recording: layer.recording,
            };
            layer_views.push(view);
        }
    }

    LooperView {
        state: status.state.name(),
        number: status.number,
        busy,
        beats_in,
        layers: layer_views,
        removing: None,
        message: None,
    }
}

// `notes`, in beats at `tempo`.
fn note_views_of(notes: &[LoopNote], tempo: u32, sample_rate: u32) -> Vec<LoopNoteView> {
    let mut views = Vec::new();
    for played in notes {
        let mut end = None;
        if let Some(end_sample) = played.end {
            end = Some(beats_of(end_sample, tempo, sample_rate));
        }
        views.push(LoopNoteView {
            note: played.note,
            start: beats_of(played.start, tempo, sample_rate),
            end,
        });
    }
    views
}

// How many beats `samples` samples last at `tempo` beats per minute.
fn beats_of(samples: u64, tempo: u32, sample_rate: u32) -> f64 {
    let samples_per_beat = f64::from(sample_rate) * 60.0 / f64::from(tempo);
    samples as f64 / samples_per_beat
}

// Plays `note` at `velocity` until `release_note` lets go of it. The first
// note opens the speakers, so it may start a little late.
#[tauri::command]
pub fn play_note(engine: State<'_, Mutex<Engine>>, note: u8, velocity: u8) -> Result<(), String> {
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    engine.note_on(note, velocity);
    Ok(())
}

// Lets go of `note`, so it fades out.
#[tauri::command]
pub fn release_note(engine: State<'_, Mutex<Engine>>, note: u8) {
    let mut engine = engine.lock().unwrap();
    // Before the speakers open, no note can be playing and nothing empties the
    // engine's mailbox. Sending anyway would let messages pile up there until
    // it is full, and then the first real note would be dropped.
    if engine.is_audio_running() {
        engine.note_off(note);
    }
}

// Lets go of every note and forgets every held key, for when the window loses
// focus and a held key's release would never arrive.
#[tauri::command]
pub fn release_all_notes(
    engine: State<'_, Mutex<Engine>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
) {
    let now = Instant::now();
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();

    all_keys_up(&mut key_map, &mut composer, now);

    // Same reason as in `release_note`.
    if engine.is_audio_running() {
        engine.all_notes_off();
    }
}

// Moves the song's notes played from now on to the next wave (sine,
// triangle, square, saw, then sine again), and returns what the screen shows
// about the sound. Notes already playing keep their wave. Like `play_note`,
// it opens the speakers first: before they open, nothing empties the
// engine's mailbox, so switches would pile up there.
//
// The commands that answer with the sound lock Compose before the engine,
// since the song holds its tempo and sound profile.
#[tauri::command]
pub fn switch_wave(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.sound_profile_mut().switch_wave(&mut engine);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Turns "snap to beat" off if it is on, and on if it is off, and returns
// what the screen shows about the sound. A loop already playing changes
// right away. Like `switch_wave`, it opens the speakers first.
#[tauri::command]
pub fn switch_snap(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    engine.switch_snap();
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets layer `layer`'s volume, counted from 1, to `volume` percent, from 0
// to 100 (docs/adr/0026-layers.md), and returns what the screen shows about
// the sound. The layer's notes already sounding change too. A layer's volume
// knob does this. Like `switch_wave`, it opens the speakers first.
#[tauri::command]
pub fn set_layer_volume(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    layer: u32,
    volume: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    engine.set_layer_volume(layer, volume)?;
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Moves the song's filter: middle C's cutoff to `cutoff_hz`, where 20000 is
// open, and the resonance to `resonance` percent
// (docs/adr/0028-a-low-pass-filter.md). Values outside their ranges are
// brought within them. Every note hears it, even those already sounding.
// Returns what the screen shows about the sound. The sound's dev panel in
// Compose sends it, naming the arguments in camelCase, like `cutoffHz`. Like
// `switch_wave`, it opens the speakers first.
#[tauri::command]
pub fn set_filter(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    cutoff_hz: f32,
    resonance: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let filter = Filter { cutoff_hz, resonance };
    composer.sound_profile_mut().set_filter(&mut engine, filter);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets the song's filter envelope for notes started from now on: how far it
// opens the cutoff, in octaves, its stages in ms, and its sustain in percent
// of the amount. Values outside their ranges are brought within them.
// Returns what the screen shows about the sound. Like `set_filter`, it opens
// the speakers first.
#[tauri::command]
pub fn set_filter_envelope(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    amount: f32,
    attack_ms: u32,
    decay_ms: u32,
    sustain: u32,
    release_ms: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let envelope = FilterEnvelope {
        amount,
        attack_ms,
        decay_ms,
        sustain,
        release_ms,
    };
    composer.sound_profile_mut().set_filter_envelope(&mut engine, envelope);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets the song's unison for notes started from now on
// (docs/adr/0029-unison.md): how many copies play each note, how far apart
// they are tuned in cents, and how far apart they sit between the speakers
// in percent. Values outside their ranges are brought within them. Returns
// what the screen shows about the sound. The sound's dev panel in Compose
// sends it, as `detuneCents`. Like `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_unison(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    copies: u32,
    detune_cents: u32,
    spread: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let unison = Unison {
        copies,
        detune_cents,
        spread,
    };
    composer.sound_profile_mut().set_unison(&mut engine, unison);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets the song's loudness envelope for notes started from now on: how long
// its attack, decay and release take in ms, from 1 to 10000, and its
// sustain in percent of the peak. Values outside their ranges are brought
// within them. Returns what the screen shows about the sound. The sound's
// dev panel in Compose sends it, as `attackMs`, `decayMs` and `releaseMs`.
// Like `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_loudness_envelope(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    attack_ms: u32,
    decay_ms: u32,
    sustain: u32,
    release_ms: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let envelope = LoudnessEnvelope {
        attack_ms,
        decay_ms,
        sustain,
        release_ms,
    };
    composer.sound_profile_mut().set_loudness_envelope(&mut engine, envelope);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets how much the cutoff of the song's notes started from now on follows
// middle C's, in percent: at 100 every note is equally bright, at 0 higher
// notes sound darker. Above 100 counts as 100. Returns what the screen shows
// about the sound. The sound's dev panel in Compose sends it, as
// `keyTracking`. Like `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_key_tracking(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    key_tracking: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.sound_profile_mut().set_key_tracking(&mut engine, key_tracking);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Tunes the song's notes started from now on from A4 at `a4_hz`, from 415 to
// 466, brought within that range. Returns what the screen shows about the
// sound. The sound's dev panel in Compose sends it, as `a4Hz`. Like
// `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_tuning(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    a4_hz: f32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.sound_profile_mut().set_tuning(&mut engine, a4_hz);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets the song's LFO `index`, from 0 to 7
// (docs/adr/0031-lfos-and-a-routing-list.md), as `lfo` describes it. Values
// outside their ranges are brought within them. Every note hears it from the
// next 32 samples on. Returns what the screen shows about the sound. Fails
// on an index past the LFOs, or a shape or speed with no such name. Like
// `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_lfo(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    index: usize,
    lfo: LfoView,
) -> Result<SoundView, String> {
    let now = Instant::now();
    if index >= LFO_COUNT {
        return Err(format!("there is no LFO {}", index + 1));
    }
    let lfo = lfo_of_view(&lfo)?;

    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.sound_profile_mut().set_lfo(&mut engine, index, lfo);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets connection `index` of the song's routing list, from 0 to 63, as
// `connection` describes it. Its amount is brought within its target's
// range. Returns what the screen shows about the sound. Fails on an index
// past the connections, or a source or target with no such name. Like
// `set_filter`, it opens the speakers first.
#[tauri::command]
pub fn set_connection(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    index: usize,
    connection: ConnectionView,
) -> Result<SoundView, String> {
    let now = Instant::now();
    if index >= CONNECTION_COUNT {
        return Err(format!("there is no connection {}", index + 1));
    }
    let connection = connection_of_view(&connection)?;

    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.sound_profile_mut().set_connection(&mut engine, index, connection);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Sets how long the song's vibrato key takes to rise when held and fall
// when let go, in ms, each brought within 0 to 10000
// (docs/adr/0032-the-vibrato-key.md). Returns what the screen shows about
// the sound. The UI sends them as `riseMs` and `fallMs`. Like `set_filter`,
// it opens the speakers first.
#[tauri::command]
pub fn set_vibrato_key_times(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    rise_ms: u32,
    fall_ms: u32,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let times = VibratoKeyTimes { rise_ms, fall_ms };
    composer.sound_profile_mut().set_vibrato_key_times(&mut engine, times);
    Ok(sound_view_of(&mut engine, &composer, now))
}

// Does what a laptop key does when pressed: plays its note or chord, holds
// the vibrato key, switches the wave, runs the looper or changes the tempo.
// `key` is the key's place on the keyboard, like "KeyQ". Returns what the
// screen shows about the sound, so the UI can show it.
//
// Commands read the clock before anything else, so waiting for a lock never
// counts toward how long a chord was held. Then they lock what they need in
// this order: the key map, Compose, the engine. So two of them can never end
// up waiting on each other.
#[tauri::command]
pub fn press_key(
    engine: State<'_, Mutex<Engine>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    key: String,
) -> Result<SoundView, String> {
    let now = Instant::now();
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();

    // A key that picks a layer for Esc plays nothing. Esc only waits once a
    // loop has layers, so the speakers are already open.
    if picks_layer(&mut engine, &key, now) {
        return Ok(sound_view_of(&mut engine, &composer, now));
    }

    // Like `switch_wave`, every key that does something opens the speakers
    // first: before they open, nothing empties the engine's mailbox.
    if let Some(action) = key_down(&mut key_map, &mut composer, &key, now) {
        engine.start_audio()?;
        take_key_action(&mut engine, &mut composer, action, now);
    }

    Ok(sound_view_of(&mut engine, &composer, now))
}

// While Esc waits, 1 to 8 pick a layer for it to remove instead of playing
// (docs/adr/0026-layers.md). Returns true if `key`, going down at `now`,
// picked one. Esc's half second may be up first, and then the key does what
// it always does.
fn picks_layer(engine: &mut Engine, key: &str, now: Instant) -> bool {
    engine.finish_esc_wait(now);
    let number = match layer_number_of(key) {
        Some(number) => number,
        None => return false,
    };
    engine.pick_layer(number, now)
}

// What a key pressed at `now` does to the engine. The wave key and the tempo
// keys change the song in `composer`, which passes the change on to the
// engine. Kept apart from opening the speakers, so the tests can check every
// key on an engine without them.
fn take_key_action(engine: &mut Engine, composer: &mut Composer, action: KeyAction, now: Instant) {
    match action {
        KeyAction::PlayNote(note) => {
            engine.note_on(note, LAPTOP_VELOCITY);
        }
        KeyAction::PlayChord(chord) => {
            for note in chord.notes() {
                engine.note_on(note, LAPTOP_VELOCITY);
            }
        }
        KeyAction::HoldVibratoKey => {
            engine.set_vibrato_key(true);
        }
        KeyAction::SwitchWave => {
            composer.sound_profile_mut().switch_wave(engine);
        }
        KeyAction::PressLooper => {
            engine.press_looper();
        }
        KeyAction::RemoveLayer => {
            engine.press_esc(now);
        }
        KeyAction::SlowDown => {
            composer.slow_down(engine);
        }
        KeyAction::SpeedUp => {
            composer.speed_up(engine);
        }
    }
}

// Lets go of the notes a laptop key started, except any that another held
// key is still playing, or of the vibrato key.
#[tauri::command]
pub fn release_key(
    engine: State<'_, Mutex<Engine>>,
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    key: String,
) {
    let now = Instant::now();
    let mut key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();

    // Forget the key first, whether or not the speakers are open.
    let lets_go_of_vibrato_key = key_map.holds_vibrato_key(&key);
    let notes = key_up(&mut key_map, &mut composer, &key, now);

    // Same reason as in `release_note`.
    if !engine.is_audio_running() {
        return;
    }
    if lets_go_of_vibrato_key {
        engine.set_vibrato_key(false);
    }
    for note in notes {
        engine.note_off(note);
    }
}

// The next three tell the key map and Compose about a key, at the time its
// command read the clock. They are kept apart from the engine, so the tests
// can play keys through the same steps without one.

// `key` went down at `now`. Returns what it does, or None for nothing.
pub(crate) fn key_down(key_map: &mut KeyMap, composer: &mut Composer, key: &str, now: Instant) -> Option<KeyAction> {
    let action = match key_map.press(key) {
        Some(action) => action,
        None => return None,
    };
    // Compose times every chord, so a chord added to the song keeps how long
    // it was held.
    if let KeyAction::PlayChord(chord) = action {
        composer.chord_pressed(key, chord, now);
    }
    Some(action)
}

// `key` came up at `now`, so a chord it is still holding ends. Returns the
// notes to stop.
pub(crate) fn key_up(key_map: &mut KeyMap, composer: &mut Composer, key: &str, now: Instant) -> Vec<u8> {
    let notes = key_map.release(key);
    composer.key_released(key, now);
    notes
}

// Every key came up at once at `now`.
pub(crate) fn all_keys_up(key_map: &mut KeyMap, composer: &mut Composer, now: Instant) {
    // Otherwise a key held when the window lost focus would still count as
    // held, and its next press would be ignored.
    key_map.release_all();
    // Every chord still held ends now.
    composer.all_released(now);
}

// What the screen shows about the sound, so the UI can show it when it
// opens.
#[tauri::command]
pub fn sound_view(composer: State<'_, Mutex<Composer>>, engine: State<'_, Mutex<Engine>>) -> SoundView {
    let now = Instant::now();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    sound_view_of(&mut engine, &composer, now)
}

// What every laptop key does, so the UI can draw each key's note or chord on
// it.
#[tauri::command]
pub fn key_layout(key_map: State<'_, Mutex<KeyMap>>) -> LayoutForUi {
    let key_map = key_map.lock().unwrap();
    key_map.layout_for_ui()
}

// Compose's commands. Each answers with everything Compose shows, so the UI
// can redraw it, including where the song is playing. Like the commands
// above, they read the clock first, then lock the key map, Compose and the
// engine, in that order.

// What Compose shows right now.
#[tauri::command]
pub fn compose_view(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> ComposeView {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    composer.view(&key_map, engine.song_place(), now)
}

// Sets the song's key: `tonic` is its home note, from 0 (C) to 11 (B), and
// `mode` is "major" or "minor".
#[tauri::command]
pub fn pick_key(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    tonic: u8,
    mode: Mode,
) -> Result<ComposeView, String> {
    if tonic > 11 {
        return Err(format!("no key has home note {tonic}: it goes from 0 (C) to 11 (B)"));
    }
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    composer.pick_key(Key { tonic, mode });
    Ok(composer.view(&key_map, engine.song_place(), now))
}

// Adds the chord played last to the end of the song, leaving out the chords
// tried before it. It keeps how long it was held, and if it still is, it
// keeps growing until it is let go. Compose's Return key does this.
#[tauri::command]
pub fn add_last_chord(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> ComposeView {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    composer.add_last_chord();
    composer.view(&key_map, engine.song_place(), now)
}

// Takes the last chord off the end of the song. Compose's Delete key does
// this.
#[tauri::command]
pub fn remove_last_chord(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> ComposeView {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let engine = engine.lock().unwrap();
    composer.remove_last_chord();
    composer.view(&key_map, engine.song_place(), now)
}

// Plays the chord at `place` in the song, from 0, for as long as it was
// held. Clicking a chord on the tape does this. Like `switch_wave`, it opens
// the speakers first.
#[tauri::command]
pub fn play_chord_in_song(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    place: usize,
) -> Result<ComposeView, String> {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.play_chord(place, &mut engine, now)?;
    Ok(composer.view(&key_map, engine.song_place(), now))
}

// Plays the whole song from its first chord. The tape's play button does
// this. Like `switch_wave`, it opens the speakers first.
#[tauri::command]
pub fn play_song(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> Result<ComposeView, String> {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    composer.play_song(&mut engine, now)?;
    Ok(composer.view(&key_map, engine.song_place(), now))
}

// Stops the song. Live notes and the loop play on.
#[tauri::command]
pub fn stop_song(
    key_map: State<'_, Mutex<KeyMap>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> ComposeView {
    let now = Instant::now();
    let key_map = key_map.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    // Same reason as in `release_note`: before the speakers open, no song
    // can be playing.
    if engine.is_audio_running() {
        engine.stop_song();
    }
    composer.view(&key_map, engine.song_place(), now)
}

// Renders a held middle C to Music/Jello/test-note.wav and returns where it
// went.
#[tauri::command]
pub fn render_test_note(app: AppHandle) -> Result<String, String> {
    // A second engine with no speakers, so the file never disturbs what is
    // playing live. It runs the same code the speakers hear.
    let mut engine = Engine::new()?;
    let sample_rate = engine.sample_rate();

    engine.note_on(MIDDLE_C, TEST_VELOCITY);
    let mut samples = engine.render(frames_in_ms(sample_rate, HOLD_MS));

    engine.note_off(MIDDLE_C);
    let tail = engine.render(frames_in_ms(sample_rate, TAIL_MS));
    samples.extend(tail);

    let path = jello_music_folder(&app)?.join("test-note.wav");
    write_wav(&path, &samples, sample_rate)?;

    Ok(path.display().to_string())
}

fn frames_in_ms(sample_rate: u32, ms: u32) -> usize {
    let frames = sample_rate * ms / 1000;
    frames as usize
}

// The Jello folder inside the user's Music folder, made if it is missing.
fn jello_music_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let music = app.path().audio_dir().map_err(|error| error.to_string())?;
    let folder = music.join("Jello");
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

// These tests look at what the commands answer and what each key does to
// the engine. They render without speakers.
#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use crate::engine::Wave;
    use crate::modulation::{NO_CONNECTION, START_LFO};

    // What key `key` does in the default layout.
    fn action_of(key: &str) -> KeyAction {
        let mut key_map = KeyMap::new();
        key_map.press(key).unwrap()
    }

    // Takes key `key`'s action now, on `engine` and the song in `composer`.
    fn press(engine: &mut Engine, composer: &mut Composer, key: &str) {
        take_key_action(engine, composer, action_of(key), Instant::now());
    }

    // What the screen shows about the sound at `now`, with a new song, for
    // the tests that look only at the looper.
    fn view_of(engine: &mut Engine, now: Instant) -> SoundView {
        sound_view_of(engine, &Composer::new(), now)
    }

    // Renders `frames` frames and returns the loudest sample.
    fn loudest(engine: &mut Engine, frames: usize) -> f32 {
        let samples = engine.render(frames);
        let mut loudest = 0.0;
        for sample in samples {
            if sample.abs() > loudest {
                loudest = sample.abs();
            }
        }
        loudest
    }

    #[test]
    fn note_and_chord_keys_play() {
        // M plays middle C, and C plays the C chord, whose three notes
        // together are louder than one.
        let mut composer = Composer::new();
        let mut note = Engine::new().unwrap();
        press(&mut note, &mut composer, "KeyM");
        let mut chord = Engine::new().unwrap();
        press(&mut chord, &mut composer, "KeyC");

        let note_loudness = loudest(&mut note, 4800);
        assert!(note_loudness > 0.0);
        assert!(loudest(&mut chord, 4800) > note_loudness);
    }

    #[test]
    fn backslash_switches_the_song_s_wave() {
        // A new song starts on the triangle, so the switch lands on the
        // square, and the engine plays it.
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        press(&mut engine, &mut composer, "Backslash");
        assert_eq!(composer.sound_profile().wave(), Wave::Square);

        let mut square = Engine::new().unwrap();
        square.set_wave(Wave::Square);
        let mut square_composer = Composer::new();
        press(&mut engine, &mut composer, "KeyM");
        press(&mut square, &mut square_composer, "KeyM");
        let switched = engine.render(4800);
        let expected = square.render(4800);
        assert!(switched == expected, "the engine does not play the song's new wave");
    }

    #[test]
    fn right_shift_runs_the_looper_and_esc_cancels_the_count_in_at_once() {
        // During the count-in there is only the one layer, so Esc has
        // nothing to wait for.
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        press(&mut engine, &mut composer, "ShiftRight");
        engine.render(10);
        assert_eq!(engine.looper_status().state, LooperState::CountingIn);

        press(&mut engine, &mut composer, "Escape");
        engine.render(10);
        assert_eq!(engine.looper_status().state, LooperState::Idle);
    }

    #[test]
    fn minus_and_equal_change_the_song_s_tempo_until_the_looper_starts() {
        // The engine plays each new tempo too.
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        press(&mut engine, &mut composer, "Equal");
        assert_eq!(composer.tempo(), 125);
        assert_eq!(engine.tempo(), 125);
        press(&mut engine, &mut composer, "Minus");
        press(&mut engine, &mut composer, "Minus");
        assert_eq!(composer.tempo(), 115);
        assert_eq!(engine.tempo(), 115);

        press(&mut engine, &mut composer, "ShiftRight");
        press(&mut engine, &mut composer, "Equal");
        assert_eq!(composer.tempo(), 115);
    }

    #[test]
    fn the_ui_reads_the_sound_by_these_names() {
        // ui/sound.ts reads these names, so renaming them here would leave
        // the wave, beat and looper keys blank.
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        composer.speed_up(&mut engine);
        engine.press_looper();
        engine.render(10);

        let json = serde_json::to_value(sound_view_of(&mut engine, &composer, Instant::now())).unwrap();
        assert_eq!(json["wave"], "triangle");
        assert_eq!(json["tempo"], 125);
        assert_eq!(json["snap"], true);
        assert_eq!(json["looper"]["state"], "counting in");
        assert_eq!(json["looper"]["number"], 1);
        assert_eq!(json["looper"]["busy"], true);
        assert!(json["looper"]["beats_in"].as_f64().unwrap() > 0.0);
        assert_eq!(json["looper"]["layers"], serde_json::json!([]));
        assert_eq!(json["looper"]["removing"], serde_json::Value::Null);
        assert_eq!(json["looper"]["message"], serde_json::Value::Null);

        // And the song's filter, by the names the UI will read. Every value
        // is different, so two names mixed up would show.
        let profile = composer.sound_profile_mut();
        profile.set_filter(
            &mut engine,
            Filter {
                cutoff_hz: 1000.0,
                resonance: 40,
            },
        );
        profile.set_filter_envelope(
            &mut engine,
            FilterEnvelope {
                amount: 2.5,
                attack_ms: 10,
                decay_ms: 200,
                sustain: 60,
                release_ms: 400,
            },
        );
        profile.set_key_tracking(&mut engine, 30);
        let json = serde_json::to_value(sound_view_of(&mut engine, &composer, Instant::now())).unwrap();
        assert_eq!(json["filter"]["cutoff_hz"], 1000.0);
        assert_eq!(json["filter"]["resonance"], 40);
        assert_eq!(json["filter"]["amount"], 2.5);
        assert_eq!(json["filter"]["attack_ms"], 10);
        assert_eq!(json["filter"]["decay_ms"], 200);
        assert_eq!(json["filter"]["sustain"], 60);
        assert_eq!(json["filter"]["release_ms"], 400);
        assert_eq!(json["filter"]["key_tracking"], 30);

        // And the song's unison, wave, loudness envelope and tuning, the
        // same way.
        let profile = composer.sound_profile_mut();
        profile.set_unison(
            &mut engine,
            Unison {
                copies: 5,
                detune_cents: 30,
                spread: 70,
            },
        );
        profile.switch_wave(&mut engine);
        profile.set_loudness_envelope(
            &mut engine,
            LoudnessEnvelope {
                attack_ms: 15,
                decay_ms: 250,
                sustain: 80,
                release_ms: 500,
            },
        );
        profile.set_tuning(&mut engine, 432.0);
        let json = serde_json::to_value(sound_view_of(&mut engine, &composer, Instant::now())).unwrap();
        assert_eq!(json["wave"], "square");
        assert_eq!(json["unison"]["copies"], 5);
        assert_eq!(json["unison"]["detune_cents"], 30);
        assert_eq!(json["unison"]["spread"], 70);
        assert_eq!(json["loudness"]["attack_ms"], 15);
        assert_eq!(json["loudness"]["decay_ms"], 250);
        assert_eq!(json["loudness"]["sustain"], 80);
        assert_eq!(json["loudness"]["release_ms"], 500);
        assert_eq!(json["tuning_hz"], 432.0);

        // And the loop's layers, by the names ui/sound.ts reads.
        let layers = [layer_of(&[LoopNote {
            note: A4,
            start: 12_000,
            end: None,
        }])];
        let mut view = looper_view_of(looping(1), true, 0, &layers, 120, SAMPLE_RATE);
        view.removing = Some(1);
        view.message = Some(String::from("There is no layer 7"));
        let json = serde_json::to_value(view).unwrap();
        let expected = serde_json::json!([{
            "notes": [{ "note": 69, "start": 0.5, "end": null }],
            "volume": 100,
            "recording": false,
        }]);
        assert_eq!(json["layers"], expected);
        assert_eq!(json["removing"], 1);
        assert_eq!(json["message"], "There is no layer 7");
    }

    #[test]
    fn left_shift_holds_the_vibrato_key_and_left_control_plays_f_sharp_minor() {
        // With a new song's vibrato, middle C played with left Shift held
        // swings, so it sounds unlike middle C alone.
        let mut composer = Composer::new();
        let mut held = Engine::new().unwrap();
        let mut up = Engine::new().unwrap();
        for engine in [&mut held, &mut up] {
            composer.send_all(engine);
        }
        press(&mut held, &mut composer, "ShiftLeft");
        press(&mut held, &mut composer, "KeyM");
        press(&mut up, &mut composer, "KeyM");
        let held_sound = held.render(48_000);
        let up_sound = up.render(48_000);
        assert!(held_sound != up_sound, "left Shift moved nothing");

        // Left Control plays F#m: F#3, A3 and C#4.
        let mut control = Engine::new().unwrap();
        press(&mut control, &mut composer, "ControlLeft");
        let mut by_hand = Engine::new().unwrap();
        for note in [54, 57, 61] {
            by_hand.note_on(note, LAPTOP_VELOCITY);
        }
        assert!(control.render(4800) == by_hand.render(4800), "left Control did not play F#m");
    }

    #[test]
    fn the_ui_reads_the_lfos_routing_list_and_key_times_by_these_names() {
        // Every value different, so two names mixed up would show.
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        let mut steps = [0.0; LFO_MOST_STEPS];
        steps[0] = -40.0;
        steps[1] = 70.0;
        let lfo = Lfo {
            shape: LfoShape::SawDown,
            step_count: 2,
            steps,
            per_note: false,
            speed: LfoSpeed::Presses,
            hz: 2.5,
            beats: 4.0,
            presses: 6,
            start: 20,
            spread: 30,
            delay_ms: 150,
            fade_ms: 250,
            delay_beats: 0.5,
            fade_beats: 1.5,
            smooth: 40,
            once: true,
        };
        let connection = Connection {
            source: Source::Lfo(2),
            target: Target::Cutoff,
            amount: -1.5,
            one_way: true,
            snap: true,
            scaled_by: Source::NoteHeight,
        };
        let profile = composer.sound_profile_mut();
        profile.set_lfo(&mut engine, 7, lfo);
        profile.set_connection(&mut engine, 63, connection);
        let times = VibratoKeyTimes {
            rise_ms: 120,
            fall_ms: 340,
        };
        profile.set_vibrato_key_times(&mut engine, times);

        let json = serde_json::to_value(sound_view_of(&mut engine, &composer, Instant::now())).unwrap();
        let lfos = json["lfos"].as_array().unwrap();
        assert_eq!(lfos.len(), LFO_COUNT);
        let expected_lfo = serde_json::json!({
            "shape": "saw down",
            "step_count": 2,
            "steps": steps.to_vec(),
            "per_note": false,
            "speed": "presses",
            "hz": 2.5,
            "beats": 4.0,
            "presses": 6,
            "start": 20,
            "spread": 30,
            "delay_ms": 150,
            "fade_ms": 250,
            "delay_beats": 0.5,
            "fade_beats": 1.5,
            "smooth": 40,
            "once": true,
        });
        assert_eq!(lfos[7], expected_lfo);
        assert_eq!(lfos[0]["shape"], "sine");

        let connections = json["connections"].as_array().unwrap();
        assert_eq!(connections.len(), CONNECTION_COUNT);
        let expected_connection = serde_json::json!({
            "source": "lfo 3",
            "target": "cutoff",
            "amount": -1.5,
            "one_way": true,
            "snap": true,
            "scaled_by": "note height",
            "unit": "octaves",
        });
        assert_eq!(connections[63], expected_connection);
        // A new song's vibrato, and an unused connection.
        assert_eq!(connections[0]["source"], "lfo 1");
        assert_eq!(connections[0]["target"], "pitch");
        assert_eq!(connections[0]["scaled_by"], "vibrato key");
        assert_eq!(connections[0]["unit"], "cents");
        assert_eq!(connections[1]["source"], "none");

        assert_eq!(json["vibrato_key"]["rise_ms"], 120);
        assert_eq!(json["vibrato_key"]["fall_ms"], 340);
    }

    #[test]
    fn an_lfo_and_a_connection_come_back_from_the_screen_as_they_went() {
        // Every shape and speed, and every source and target, by name.
        for shape in LfoShape::ALL {
            for speed in LfoSpeed::ALL {
                let lfo = Lfo {
                    shape,
                    speed,
                    ..START_LFO
                };
                assert_eq!(lfo_of_view(&lfo_view_of(&lfo)), Ok(lfo));
            }
        }
        let mut sources = vec![Source::None, Source::VibratoKey, Source::NoteHeight, Source::Random];
        sources.push(Source::LoudnessEnvelope);
        sources.push(Source::FilterEnvelope);
        let mut targets = Target::OTHERS.to_vec();
        for index in 0..LFO_COUNT {
            sources.push(Source::Lfo(index));
            targets.push(Target::LfoSpeed(index));
        }
        for source in &sources {
            for target in &targets {
                let connection = Connection {
                    source: *source,
                    target: *target,
                    amount: 0.5,
                    one_way: true,
                    snap: false,
                    scaled_by: *source,
                };
                assert_eq!(connection_of_view(&connection_view_of(&connection)), Ok(connection));
            }
        }

        // The screen may send more steps than an LFO has, or fewer.
        let mut view = lfo_view_of(&START_LFO);
        view.steps = vec![50.0; LFO_MOST_STEPS + 8];
        assert_eq!(lfo_of_view(&view).unwrap().steps, [50.0; LFO_MOST_STEPS]);
        view.steps = vec![50.0, -50.0];
        let mut expected = [0.0; LFO_MOST_STEPS];
        expected[0] = 50.0;
        expected[1] = -50.0;
        assert_eq!(lfo_of_view(&view).unwrap().steps, expected);

        // And need not send a connection's unit back.
        let sent = serde_json::json!({
            "source": "vibrato key",
            "target": "pan",
            "amount": 50.0,
            "one_way": false,
            "snap": false,
            "scaled_by": "none",
        });
        let view: ConnectionView = serde_json::from_value(sent).unwrap();
        let expected = Connection {
            source: Source::VibratoKey,
            target: Target::Pan,
            amount: 50.0,
            one_way: false,
            snap: false,
            scaled_by: Source::None,
        };
        assert_eq!(connection_of_view(&view), Ok(expected));
    }

    #[test]
    fn an_lfo_or_connection_with_an_odd_name_fails() {
        let mut view = lfo_view_of(&START_LFO);
        view.shape = String::from("wobble");
        assert_eq!(lfo_of_view(&view), Err(String::from("there is no LFO shape called wobble")));
        let mut view = lfo_view_of(&START_LFO);
        view.speed = String::from("bars");
        assert_eq!(lfo_of_view(&view), Err(String::from("there is no LFO speed called bars")));

        let view = connection_view_of(&NO_CONNECTION);
        let odd_source = ConnectionView {
            source: String::from("lfo 9"),
            ..view.clone()
        };
        assert_eq!(connection_of_view(&odd_source), Err(String::from("there is no source called lfo 9")));
        let odd_scale = ConnectionView {
            scaled_by: String::from("mood"),
            ..view.clone()
        };
        assert_eq!(connection_of_view(&odd_scale), Err(String::from("there is no source called mood")));
        let odd_target = ConnectionView {
            target: String::from("volume"),
            ..view
        };
        assert_eq!(connection_of_view(&odd_target), Err(String::from("there is no target called volume")));
    }

    #[test]
    fn the_lane_leaves_out_the_vibrato_key_s_presses() {
        // A4 recorded with the key held under it: the layer keeps both, and
        // the lane shows A4 alone.
        let mut engine = Engine::new().unwrap();
        engine.press_looper();
        engine.render(96_000);
        engine.note_on(A4, 100);
        engine.set_vibrato_key(true);
        engine.render(12_000);
        engine.set_vibrato_key(false);
        engine.note_off(A4);
        engine.render(12_000);

        let notes = engine.loop_layers()[0].notes.clone();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[1].note, VIBRATO_KEY_NOTE);
        let view = view_of(&mut engine, Instant::now());
        let a4 = LoopNoteView {
            note: A4,
            start: 0.0,
            end: Some(0.5),
        };
        assert_eq!(view.looper.layers[0].notes, vec![a4]);
    }

    const SAMPLE_RATE: u32 = 48_000;
    const A4: u8 = 69;
    const C5: u8 = 72;

    // At 120 a beat is 24,000 samples, and a bar 96,000.
    const BAR: usize = 96_000;

    fn status(state: LooperState, number: u32) -> LooperStatus {
        LooperStatus {
            state,
            number,
            waiting: false,
        }
    }

    fn looping(bars: u32) -> LooperStatus {
        status(LooperState::Looping, bars)
    }

    // A layer played back at full volume, with `notes`.
    fn layer_of(notes: &[LoopNote]) -> LoopLayer {
        LoopLayer {
            notes: notes.to_vec(),
            volume: 100,
            recording: false,
        }
    }

    #[test]
    fn the_looper_s_place_is_counted_in_beats_at_the_tempo() {
        // At 120 a beat is 24,000 samples, so 36,000 is a beat and a half.
        let counting_in = status(LooperState::CountingIn, 2);
        let view = looper_view_of(counting_in, true, 36_000, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 1.5);

        // At 125 a beat is 23,040 samples.
        let recording = status(LooperState::Recording, 1);
        let view = looper_view_of(recording, true, 34_560, &[], 125, SAMPLE_RATE);
        assert_eq!(view.beats_in, 1.5);

        // Recording counts on past the first bar: 9 beats is bar 3.
        let view = looper_view_of(recording, true, 9 * 24_000, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 9.0);

        let idle = status(LooperState::Idle, 0);
        let view = looper_view_of(idle, false, 0, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 0.0);
    }

    #[test]
    fn a_place_past_the_loop_s_end_wraps_into_the_loop() {
        // Read right as a 2-bar loop closed: the recording's place, 8 and a
        // half beats in, is half a beat into the loop.
        let view = looper_view_of(looping(2), true, 204_000, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 0.5);

        // Inside the loop it stays as it is, and so it does while a layer
        // is recorded.
        let view = looper_view_of(looping(2), true, 180_000, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 7.5);
        let layering = status(LooperState::Layering, 2);
        let view = looper_view_of(layering, true, 204_000, &[], 120, SAMPLE_RATE);
        assert_eq!(view.beats_in, 0.5);
    }

    #[test]
    fn the_loop_s_layers_are_in_beats_while_recording_looping_or_layering() {
        let notes = [
            LoopNote {
                note: A4,
                start: 12_000,
                end: Some(36_000),
            },
            LoopNote {
                note: C5,
                start: 48_000,
                end: None,
            },
        ];
        let expected = vec![
            LoopNoteView {
                note: A4,
                start: 0.5,
                end: Some(1.5),
            },
            LoopNoteView {
                note: C5,
                start: 2.0,
                end: None,
            },
        ];
        let mut quiet = layer_of(&notes[..1]);
        quiet.volume = 40;
        let mut recorded = layer_of(&notes);
        recorded.recording = true;
        let layers = [quiet, recorded];

        for state in [LooperState::Recording, LooperState::Looping, LooperState::Layering] {
            let view = looper_view_of(status(state, 1), true, 0, &layers, 120, SAMPLE_RATE);
            assert_eq!(view.layers.len(), 2);
            assert_eq!(view.layers[0].notes[..], expected[..1]);
            assert_eq!(view.layers[0].volume, 40);
            assert!(!view.layers[0].recording);
            assert_eq!(view.layers[1].notes, expected);
            assert_eq!(view.layers[1].volume, 100);
            assert!(view.layers[1].recording);
        }

        // At 60 a beat is twice as long, so the same samples are half the
        // beats.
        let view = looper_view_of(looping(1), true, 0, &layers, 60, SAMPLE_RATE);
        assert_eq!(view.layers[1].notes[0].start, 0.25);

        // Nothing is shown before recording starts, or once cleared.
        let counting_in = status(LooperState::CountingIn, 3);
        let view = looper_view_of(counting_in, true, 0, &layers, 120, SAMPLE_RATE);
        assert!(view.layers.is_empty());
        let idle = status(LooperState::Idle, 0);
        let view = looper_view_of(idle, false, 0, &layers, 120, SAMPLE_RATE);
        assert!(view.layers.is_empty());
    }

    #[test]
    fn the_sound_view_shows_the_loop_as_it_is_recorded() {
        // The count-in takes the first bar, 96,000 samples at 120. A4 goes
        // down on the sample recording starts and up half a beat later; C5
        // goes down a beat in and is still held.
        let mut engine = Engine::new().unwrap();
        engine.press_looper();
        engine.render(96_000);
        engine.note_on(A4, 100);
        engine.render(12_000);
        engine.note_off(A4);
        engine.render(12_000);
        engine.note_on(C5, 100);
        engine.render(6_000);

        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.state, "recording");
        assert_eq!(view.looper.beats_in, 1.25);
        let expected = vec![
            LoopNoteView {
                note: A4,
                start: 0.0,
                end: Some(0.5),
            },
            LoopNoteView {
                note: C5,
                start: 1.0,
                end: None,
            },
        ];
        assert_eq!(view.looper.layers.len(), 1);
        assert_eq!(view.looper.layers[0].notes, expected);
        assert!(view.looper.layers[0].recording);
    }

    #[test]
    fn the_sound_view_shows_the_loop_where_it_plays() {
        // A4 is played from 13,000 to 29,000 samples into the second bar
        // recorded, after the count-in's 96,000 and the first bar's. A beat
        // is 24,000 samples.
        let mut engine = Engine::new().unwrap();
        engine.press_looper();
        engine.render(96_000 + 96_000 + 13_000);
        engine.note_on(A4, 100);
        engine.render(16_000);
        engine.note_off(A4);
        engine.render(60_000);
        let as_played = vec![LoopNoteView {
            note: A4,
            start: 109_000.0 / 24_000.0,
            end: Some(125_000.0 / 24_000.0),
        }];

        // While recording it shows as played, even with snap on.
        let view = view_of(&mut engine, Instant::now());
        assert!(view.snap);
        assert_eq!(view.looper.state, "recording");
        assert_eq!(view.looper.layers[0].notes, as_played);

        // Pressed late in bar 2, the loop closes at its end, 2 bars long.
        // From then on it shows where the loop plays A4: on the nearest half
        // beats.
        engine.press_looper();
        engine.render(30_000);
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.state, "looping");
        let snapped = vec![LoopNoteView {
            note: A4,
            start: 4.5,
            end: Some(5.0),
        }];
        assert_eq!(view.looper.layers[0].notes, snapped);

        // With snap off, as played again.
        engine.switch_snap();
        let view = view_of(&mut engine, Instant::now());
        assert!(!view.snap);
        assert_eq!(view.looper.layers[0].notes, as_played);
    }

    // Plays A4 for half a beat, from now.
    fn play_a4(engine: &mut Engine) {
        engine.note_on(A4, 100);
        engine.render(12_000);
        engine.note_off(A4);
    }

    // An engine looping a 1-bar loop of `layers` layers, each holding one
    // A4, with the repeat after the last layer's just started.
    fn engine_with_layers(layers: usize) -> Engine {
        let mut engine = Engine::new().unwrap();
        // The count-in, then the first loop, closed at the end of its bar.
        engine.press_looper();
        engine.render(BAR);
        play_a4(&mut engine);
        engine.press_looper();
        engine.render(BAR - 12_000);

        // Each layer is recorded from beat 1 and stopped half a bar in.
        for _ in 1..layers {
            engine.press_looper();
            engine.render(10);
            play_a4(&mut engine);
            engine.render(BAR / 2 - 12_010);
            engine.press_looper();
            engine.render(BAR / 2);
        }
        engine.render(10);
        assert_eq!(engine.loop_layers().len(), layers);
        assert_eq!(engine.looper_status().state, LooperState::Looping);
        engine
    }

    // How many layers the engine has once it has acted on everything sent.
    fn layers_after_a_chunk(engine: &mut Engine) -> usize {
        engine.render(10);
        engine.loop_layers().len()
    }

    // Each layer's volume, oldest first.
    fn volumes(engine: &mut Engine) -> Vec<u32> {
        let mut volumes = Vec::new();
        for layer in engine.loop_layers() {
            volumes.push(layer.volume);
        }
        volumes
    }

    #[test]
    fn with_two_layers_esc_waits_half_a_second_then_removes_the_newest() {
        // Layer 1 is turned down, so we can tell which one goes.
        let mut engine = engine_with_layers(3);
        engine.set_layer_volume(1, 30).unwrap();
        let esc = Instant::now();
        let mut composer = Composer::new();
        take_key_action(&mut engine, &mut composer, KeyAction::RemoveLayer, esc);

        // Waiting: the screen shows which layer will go, and nothing has.
        let view = view_of(&mut engine, esc + Duration::from_millis(100));
        assert_eq!(view.looper.removing, Some(3));
        assert_eq!(layers_after_a_chunk(&mut engine), 3);
        let view = view_of(&mut engine, esc + Duration::from_millis(499));
        assert_eq!(view.looper.removing, Some(3));
        assert_eq!(layers_after_a_chunk(&mut engine), 3);

        // Half a second later the newest goes, the next time the screen
        // asks.
        let view = view_of(&mut engine, esc + Duration::from_millis(500));
        assert_eq!(view.looper.removing, None);
        assert_eq!(layers_after_a_chunk(&mut engine), 2);
        assert_eq!(volumes(&mut engine), vec![30, 100]);
        assert_eq!(engine.looper_status().state, LooperState::Looping);

        // A layer removed leaves no message.
        let view = view_of(&mut engine, esc + Duration::from_millis(600));
        assert_eq!(view.looper.message, None);
    }

    #[test]
    fn esc_twice_removes_the_newest_at_once() {
        let mut engine = engine_with_layers(3);
        let esc = Instant::now();
        engine.press_esc(esc);
        engine.press_esc(esc + Duration::from_millis(200));
        assert_eq!(layers_after_a_chunk(&mut engine), 2);
        assert_eq!(volumes(&mut engine), vec![100, 100]);
        assert_eq!(engine.loop_layers()[1].notes.len(), 1);

        // And nothing more goes once the half second is up.
        view_of(&mut engine, esc + Duration::from_millis(900));
        assert_eq!(layers_after_a_chunk(&mut engine), 2);
    }

    #[test]
    fn esc_then_a_number_removes_that_layer_instead() {
        // Layer 2's volume goes down first, so we can tell which one went.
        let mut engine = engine_with_layers(3);
        engine.set_layer_volume(2, 30).unwrap();
        let esc = Instant::now();
        engine.press_esc(esc);

        // Other keys play as usual while Esc waits.
        assert!(!picks_layer(&mut engine, "KeyM", esc + Duration::from_millis(100)));
        assert!(picks_layer(&mut engine, "Digit2", esc + Duration::from_millis(300)));
        assert_eq!(layers_after_a_chunk(&mut engine), 2);
        assert_eq!(volumes(&mut engine), vec![100, 100]);

        // Once picked, the half second is over, and numbers play again.
        view_of(&mut engine, esc + Duration::from_millis(900));
        assert_eq!(layers_after_a_chunk(&mut engine), 2);
        assert!(!picks_layer(&mut engine, "Digit1", esc + Duration::from_millis(950)));

        // The newest can be picked too.
        let again = esc + Duration::from_millis(1_000);
        engine.press_esc(again);
        assert!(picks_layer(&mut engine, "Digit2", again + Duration::from_millis(100)));
        assert_eq!(layers_after_a_chunk(&mut engine), 1);
    }

    #[test]
    fn a_number_after_the_half_second_plays_as_usual() {
        // The newest goes first, then 1 plays its chord.
        let mut engine = engine_with_layers(2);
        let esc = Instant::now();
        engine.press_esc(esc);
        let later = esc + Duration::from_millis(600);
        assert!(!picks_layer(&mut engine, "Digit1", later));
        assert_eq!(layers_after_a_chunk(&mut engine), 1);
    }

    #[test]
    fn esc_then_a_number_with_no_layer_says_so() {
        let mut engine = engine_with_layers(2);
        let esc = Instant::now();
        engine.press_esc(esc);
        assert!(engine.pick_layer(7, esc + Duration::from_millis(100)));
        assert_eq!(layers_after_a_chunk(&mut engine), 2);

        // The message comes once.
        let view = view_of(&mut engine, esc + Duration::from_millis(150));
        assert_eq!(view.looper.message, Some(String::from("There is no layer 7")));
        assert_eq!(view.looper.removing, None);
        let view = view_of(&mut engine, esc + Duration::from_millis(200));
        assert_eq!(view.looper.message, None);
    }

    #[test]
    fn esc_on_the_only_layer_clears_the_loop_at_once() {
        let mut engine = engine_with_layers(1);
        engine.press_esc(Instant::now());
        engine.render(10);
        assert_eq!(engine.looper_status().state, LooperState::Idle);
        assert!(engine.loop_layers().is_empty());
    }

    #[test]
    fn the_layer_keys_are_1_to_8() {
        assert_eq!(layer_number_of("Digit1"), Some(1));
        assert_eq!(layer_number_of("Digit8"), Some(8));
        assert_eq!(layer_number_of("Digit9"), None);
        assert_eq!(layer_number_of("Digit0"), None);
        assert_eq!(layer_number_of("KeyA"), None);
    }

    #[test]
    fn a_ninth_layer_is_refused_with_a_message() {
        let mut engine = engine_with_layers(8);
        engine.press_looper();
        engine.render(10);
        assert_eq!(engine.looper_status().state, LooperState::Looping);
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(
            view.looper.message,
            Some(String::from("8 layers is the most, Esc removes one"))
        );
        assert_eq!(view.looper.layers.len(), 8);
    }

    #[test]
    fn a_layer_stopped_with_nothing_played_is_dropped_with_a_message() {
        let mut engine = engine_with_layers(2);
        engine.press_looper();
        engine.render(10);
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.state, "layering");
        assert_eq!(view.looper.layers.len(), 3);
        assert!(view.looper.layers[2].recording);

        engine.press_looper();
        engine.render(10);
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.state, "looping");
        assert_eq!(view.looper.layers.len(), 2);
        assert_eq!(view.looper.message, Some(String::from("Nothing played, no layer added")));
    }

    #[test]
    fn a_layer_s_volume_shows_and_odd_volumes_fail() {
        let mut engine = engine_with_layers(2);
        engine.set_layer_volume(2, 55).unwrap();
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.layers[0].volume, 100);
        assert_eq!(view.looper.layers[1].volume, 55);

        assert!(engine.set_layer_volume(2, 101).is_err());
        assert!(engine.set_layer_volume(3, 50).is_err());
        assert!(engine.set_layer_volume(0, 50).is_err());
        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.layers[1].volume, 55);
    }

    #[test]
    fn the_layer_being_recorded_shows_as_played_and_the_others_where_they_play() {
        // The first loop's A4, played from beat 1 for half a beat, snaps to
        // 0 to 0.5. The layer's C5, played 13,000 samples into the repeat,
        // shows as played until the layer stops.
        let mut engine = engine_with_layers(1);
        engine.press_looper();
        engine.render(13_000);
        engine.note_on(C5, 100);
        engine.render(16_000);
        engine.note_off(C5);
        engine.render(10);

        let view = view_of(&mut engine, Instant::now());
        assert_eq!(view.looper.state, "layering");
        let a4 = LoopNoteView {
            note: A4,
            start: 0.0,
            end: Some(0.5),
        };
        assert_eq!(view.looper.layers[0].notes, vec![a4]);
        let c5 = LoopNoteView {
            note: C5,
            start: 13_010.0 / 24_000.0,
            end: Some(29_010.0 / 24_000.0),
        };
        assert_eq!(view.looper.layers[1].notes, vec![c5]);

        // Stopped, it shows where it plays: C5 snaps to 0.5 to 1.0.
        engine.press_looper();
        engine.render(10);
        let view = view_of(&mut engine, Instant::now());
        let snapped = LoopNoteView {
            note: C5,
            start: 0.5,
            end: Some(1.0),
        };
        assert_eq!(view.looper.layers[1].notes, vec![snapped]);
        assert!(!view.looper.layers[1].recording);
    }
}
