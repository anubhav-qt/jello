// A safe Rust wrapper around the C++ engine's C API
// (engine/include/jello_engine.h). The rest of the Rust core only uses this.

use std::ffi::c_int;
use std::time::{Duration, Instant};

use crate::modulation::{
    CONNECTION_COUNT, Connection, LFO_COUNT, LFO_FASTEST_HZ, LFO_LONGEST_BEATS, LFO_LONGEST_DELAY_BEATS,
    LFO_LONGEST_MS, LFO_MOST_PRESSES, LFO_MOST_STEPS, LFO_SHORTEST_BEATS, LFO_SLOWEST_HZ, Lfo, Unit,
    VIBRATO_KEY_LONGEST_MS, VibratoKeyTimes,
};

// The engine itself lives in C++. Rust only ever holds a pointer to it.
#[repr(C)]
struct JelloEngine {
    _private: [u8; 0],
}

// `JelloLooperStatus` in engine/include/jello_engine.h.
#[repr(C)]
struct RawLooperStatus {
    state: c_int,
    number: c_int,
    position: u64,
    waiting: bool,
}

// `JelloLoopChange` in engine/include/jello_engine.h.
#[repr(C)]
struct RawLoopChange {
    change: c_int,
    layer: c_int,
    index: c_int,
    note: c_int,
    sample: u64,
}

// The kinds of change, the `JELLO_LOOP_` numbers in
// engine/include/jello_engine.h.
const LOOP_FORGOT: c_int = 0;
const LOOP_STARTED: c_int = 1;
const LOOP_ENDED: c_int = 2;
const LOOP_KEPT: c_int = 3;
const LOOP_LAYER_STARTED: c_int = 4;
const LOOP_LAYER_STOPPED: c_int = 5;
const LOOP_LAYER_DROPPED: c_int = 6;
const LOOP_LAYER_REMOVED: c_int = 7;

// The number for the layer recorded last (`JELLO_NEWEST_LAYER` in
// engine/include/jello_engine.h).
const NEWEST_LAYER: c_int = -1;

// `JelloLoopNote` in engine/include/jello_engine.h.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawLoopNote {
    note: c_int,
    start: u64,
    end: u64,
}

// `JelloSongChord` in engine/include/jello_engine.h.
#[repr(C)]
struct RawSongChord {
    notes: [c_int; CHORD_MAX_NOTES],
    note_count: c_int,
    velocity: c_int,
    length: u64,
}

// `JelloLfo` in engine/include/jello_engine.h.
#[repr(C)]
struct RawLfo {
    shape: c_int,
    step_count: c_int,
    steps: [f32; LFO_MOST_STEPS],
    per_note: bool,
    speed: c_int,
    hz: f64,
    beats: f64,
    presses: c_int,
    start: f64,
    spread: f64,
    delay_ms: c_int,
    fade_ms: c_int,
    delay_beats: f64,
    fade_beats: f64,
    smooth: f64,
    once: bool,
}

// `JelloConnection` in engine/include/jello_engine.h.
#[repr(C)]
struct RawConnection {
    source: c_int,
    target: c_int,
    amount: f32,
    one_way: bool,
    snap: bool,
    scaled_by: c_int,
}

unsafe extern "C" {
    fn jello_engine_create() -> *mut JelloEngine;
    fn jello_engine_destroy(engine: *mut JelloEngine);
    fn jello_engine_start_audio(engine: *mut JelloEngine) -> bool;
    fn jello_engine_note_on(engine: *mut JelloEngine, note: c_int, velocity: c_int);
    fn jello_engine_note_off(engine: *mut JelloEngine, note: c_int);
    fn jello_engine_all_notes_off(engine: *mut JelloEngine);
    fn jello_engine_set_oscillator(
        engine: *mut JelloEngine,
        index: c_int,
        kind: c_int,
        level: f32,
        octaves: c_int,
        half_steps: c_int,
        cents: f32,
    );
    // Only the tests start and stop the click for now.
    #[cfg(test)]
    fn jello_engine_start_click(engine: *mut JelloEngine);
    #[cfg(test)]
    fn jello_engine_stop_click(engine: *mut JelloEngine);
    fn jello_engine_set_tempo(engine: *mut JelloEngine, tempo: c_int);
    fn jello_engine_press_looper(engine: *mut JelloEngine);
    // Only the tests clear the whole loop at once for now.
    #[cfg(test)]
    fn jello_engine_clear_loop(engine: *mut JelloEngine);
    fn jello_engine_remove_layer(engine: *mut JelloEngine, layer: c_int);
    fn jello_engine_set_layer_volume(engine: *mut JelloEngine, layer: c_int, volume: f32);
    fn jello_engine_set_snap(engine: *mut JelloEngine, snap: bool);
    fn jello_engine_plan_loop(
        engine: *mut JelloEngine,
        played: *const RawLoopNote,
        count: c_int,
        bars: c_int,
        tempo: c_int,
        snap: bool,
        planned: *mut RawLoopNote,
    ) -> bool;
    fn jello_engine_looper_status(engine: *mut JelloEngine) -> RawLooperStatus;
    fn jello_engine_next_loop_change(engine: *mut JelloEngine, change: *mut RawLoopChange) -> bool;
    fn jello_engine_play_song(engine: *mut JelloEngine, chords: *const RawSongChord, count: c_int) -> bool;
    fn jello_engine_stop_song(engine: *mut JelloEngine);
    fn jello_engine_set_filter(engine: *mut JelloEngine, cutoff_hz: f32, resonance: f32);
    fn jello_engine_set_filter_envelope(
        engine: *mut JelloEngine,
        amount: f32,
        attack_ms: c_int,
        decay_ms: c_int,
        sustain: f32,
        release_ms: c_int,
    );
    fn jello_engine_set_unison(engine: *mut JelloEngine, copies: c_int, detune_cents: f32, spread: f32);
    fn jello_engine_set_loudness_envelope(
        engine: *mut JelloEngine,
        attack_ms: c_int,
        decay_ms: c_int,
        sustain: f32,
        release_ms: c_int,
    );
    fn jello_engine_set_key_tracking(engine: *mut JelloEngine, key_tracking: f32);
    fn jello_engine_set_tuning(engine: *mut JelloEngine, a4_hz: f32);
    fn jello_engine_set_lfo(engine: *mut JelloEngine, index: c_int, lfo: *const RawLfo);
    fn jello_engine_set_connection(engine: *mut JelloEngine, index: c_int, connection: *const RawConnection);
    fn jello_engine_set_vibrato_key_times(engine: *mut JelloEngine, rise_ms: c_int, fall_ms: c_int);
    fn jello_engine_set_vibrato_key(engine: *mut JelloEngine, held: bool);
    fn jello_engine_song_position(engine: *mut JelloEngine) -> i64;
    fn jello_engine_sample_rate(engine: *mut JelloEngine) -> c_int;
    fn jello_engine_render(engine: *mut JelloEngine, out: *mut f32, frames: c_int);
}

// Every frame of audio is one left sample and one right sample.
pub const CHANNELS: usize = 2;

// The slowest and fastest tempos the engine plays, in beats per minute
// (`JELLO_SLOWEST_TEMPO` and `JELLO_FASTEST_TEMPO` in
// engine/include/jello_engine.h).
pub const SLOWEST_TEMPO: u32 = 40;
pub const FASTEST_TEMPO: u32 = 240;

// A new engine's tempo, the usual default in music software. The C++ engine
// starts there too, and so does a new song (src/compose.rs).
pub const START_TEMPO: u32 = 120;

// How many beats make a bar (`beats_per_bar` in engine/src/clock.h).
pub const BEATS_PER_BAR: u32 = 4;

// The most layers a loop holds (`JELLO_LOOP_MAX_LAYERS` in
// engine/include/jello_engine.h).
pub const MAX_LAYERS: usize = 8;

// A layer's volume, in percent, from silent to as played. Each layer starts
// at full.
pub const FULL_VOLUME: u32 = 100;

// How long Esc waits for a layer's number (docs/adr/0026-layers.md).
const ESC_WAIT: Duration = Duration::from_millis(500);

// The most chords a song plays, and the most notes in each chord
// (`JELLO_SONG_MAX_CHORDS` and `JELLO_CHORD_MAX_NOTES` in
// engine/include/jello_engine.h).
pub const SONG_MAX_CHORDS: usize = 512;
const CHORD_MAX_NOTES: usize = 4;

// A share in percent: 100 is all of it.
pub const FULL_PERCENT: u32 = 100;

// The low-pass filter's ranges (`JELLO_FILTER_` in
// engine/include/jello_engine.h): the lowest cutoff and the cutoff that
// means open, in Hz, the most octaves the filter envelope opens it, and the
// longest any of its stages takes.
pub const FILTER_LOWEST_HZ: f32 = 20.0;
pub const FILTER_OPEN_HZ: f32 = 20000.0;
pub const FILTER_MOST_OCTAVES: f32 = 8.0;
pub const FILTER_LONGEST_MS: u32 = 10000;

// Where the low-pass filter's controls are
// (docs/adr/0028-a-low-pass-filter.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter {
    // Middle C's cutoff, in Hz, from FILTER_LOWEST_HZ to FILTER_OPEN_HZ,
    // where the filter is open: off. Every other note's cutoff follows it,
    // an octave higher for each octave up.
    pub cutoff_hz: f32,
    // How much it boosts the sound right at the cutoff, in percent.
    pub resonance: u32,
}

// Where the C++ engine's filter starts: open, with no resonance. A new
// song's sound profile starts there too (src/sound_profile.rs).
pub const START_FILTER: Filter = Filter {
    cutoff_hz: FILTER_OPEN_HZ,
    resonance: 0,
};

// The filter envelope: how far it opens the cutoff when a note starts, in
// octaves, and its stages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterEnvelope {
    // From 0, which changes nothing, to FILTER_MOST_OCTAVES.
    pub amount: f32,
    pub attack_ms: u32,
    pub decay_ms: u32,
    // Where it holds while the key is held, in percent of the amount.
    pub sustain: u32,
    pub release_ms: u32,
}

// Where the C++ engine's filter envelope starts: at 0 octaves, so it changes
// nothing. A new song's sound profile starts there too.
pub const START_FILTER_ENVELOPE: FilterEnvelope = FilterEnvelope {
    amount: 0.0,
    attack_ms: 5,
    decay_ms: 300,
    sustain: 0,
    release_ms: 300,
};

// Unison's ranges (`JELLO_UNISON_` in engine/include/jello_engine.h): the
// most copies a note can have, and the most cents the highest is tuned
// above the note.
pub const UNISON_MOST_COPIES: u32 = 7;
pub const UNISON_MOST_DETUNE_CENTS: u32 = 50;

// Unison (docs/adr/0029-unison.md): how many copies of its wave play each
// note, how far apart they are tuned and how far apart they sit between the
// speakers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unison {
    // From 1, which is off, to UNISON_MOST_COPIES.
    pub copies: u32,
    // How far the highest copy is tuned above the note, and the lowest
    // below it, in cents: hundredths of a half step.
    pub detune_cents: u32,
    // In percent: at 0 every copy plays in the middle, at 100 the lowest
    // fully left and the highest fully right.
    pub spread: u32,
}

// Where the C++ engine's unison starts: 1 copy, off. A new song's sound
// profile starts there too.
pub const START_UNISON: Unison = Unison {
    copies: 1,
    detune_cents: 20,
    spread: 100,
};

// The shortest and longest any stage of the loudness envelope takes, in ms
// (`JELLO_LOUDNESS_` in engine/include/jello_engine.h).
pub const LOUDNESS_SHORTEST_MS: u32 = 1;
pub const LOUDNESS_LONGEST_MS: u32 = 10000;

// The loudness envelope: how a note's loudness rises when it starts, falls
// to where it holds while the key is held, and fades out once it is let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoudnessEnvelope {
    pub attack_ms: u32,
    pub decay_ms: u32,
    // Where it holds while the key is held, in percent of the peak.
    pub sustain: u32,
    pub release_ms: u32,
}

// Where the C++ engine's loudness envelope starts: a 5 ms attack, held at
// the full peak, and a 300 ms release. A new song's sound profile starts
// there too.
pub const START_LOUDNESS_ENVELOPE: LoudnessEnvelope = LoudnessEnvelope {
    attack_ms: 5,
    decay_ms: 300,
    sustain: 100,
    release_ms: 300,
};

// Where the C++ engine's key tracking starts, in percent: every note's
// cutoff follows middle C's all the way. A new song's sound profile starts
// there too.
pub const START_KEY_TRACKING: u32 = 100;

// The lowest and highest A4 notes can be tuned to, in Hz
// (`JELLO_TUNING_` in engine/include/jello_engine.h).
pub const TUNING_LOWEST_HZ: f32 = 415.0;
pub const TUNING_HIGHEST_HZ: f32 = 466.0;

// Where the C++ engine's tuning starts: A4 at 440 Hz. A new song's sound
// profile starts there too.
pub const START_TUNING_HZ: f32 = 440.0;

// The pitch the loop's notes have for presses of the vibrato key
// (`JELLO_VIBRATO_KEY_NOTE` in engine/include/jello_engine.h): one past the
// highest note.
pub const VIBRATO_KEY_NOTE: u8 = 128;

// `value` brought within `lowest` to `highest`. NaN, not a number at all,
// counts as `lowest`; no slider sends it.
fn within(value: f32, lowest: f32, highest: f32) -> f32 {
    if value.is_nan() {
        return lowest;
    }
    value.clamp(lowest, highest)
}

// The same for f64.
fn within_f64(value: f64, lowest: f64, highest: f64) -> f64 {
    if value.is_nan() {
        return lowest;
    }
    value.clamp(lowest, highest)
}

// `percent` as a share from 0 to 1, with anything above 100 counting as 100.
fn share_of(percent: u32) -> f64 {
    let percent = percent.min(FULL_PERCENT);
    f64::from(percent) / f64::from(FULL_PERCENT)
}

// `lfo` with every value brought within its range.
fn lfo_within_ranges(lfo: Lfo) -> Lfo {
    let step_count = lfo.step_count.clamp(1, LFO_MOST_STEPS as u32);
    let mut steps = lfo.steps;
    for step in &mut steps {
        *step = within(*step, -(FULL_PERCENT as f32), FULL_PERCENT as f32);
    }
    Lfo {
        shape: lfo.shape,
        step_count,
        steps,
        per_note: lfo.per_note,
        speed: lfo.speed,
        hz: within_f64(lfo.hz, LFO_SLOWEST_HZ, LFO_FASTEST_HZ),
        beats: within_f64(lfo.beats, LFO_SHORTEST_BEATS, LFO_LONGEST_BEATS),
        presses: lfo.presses.clamp(1, LFO_MOST_PRESSES),
        start: lfo.start.min(FULL_PERCENT),
        spread: lfo.spread.min(FULL_PERCENT),
        delay_ms: lfo.delay_ms.min(LFO_LONGEST_MS),
        fade_ms: lfo.fade_ms.min(LFO_LONGEST_MS),
        delay_beats: within_f64(lfo.delay_beats, 0.0, LFO_LONGEST_DELAY_BEATS),
        fade_beats: within_f64(lfo.fade_beats, 0.0, LFO_LONGEST_DELAY_BEATS),
        smooth: lfo.smooth.min(FULL_PERCENT),
        once: lfo.once,
    }
}

// `lfo`, already within its ranges, as the C API takes it: steps and
// percents as shares.
fn raw_lfo(lfo: &Lfo) -> RawLfo {
    let mut steps = [0.0; LFO_MOST_STEPS];
    for (raw, step) in steps.iter_mut().zip(lfo.steps) {
        *raw = step / FULL_PERCENT as f32;
    }
    RawLfo {
        shape: lfo.shape.number(),
        step_count: lfo.step_count as c_int,
        steps,
        per_note: lfo.per_note,
        speed: lfo.speed.number(),
        hz: lfo.hz,
        beats: lfo.beats,
        presses: lfo.presses as c_int,
        start: share_of(lfo.start),
        spread: share_of(lfo.spread),
        delay_ms: lfo.delay_ms as c_int,
        fade_ms: lfo.fade_ms as c_int,
        delay_beats: lfo.delay_beats,
        fade_beats: lfo.fade_beats,
        smooth: share_of(lfo.smooth),
        once: lfo.once,
    }
}

// `connection` with every value brought within its range.
fn connection_within_ranges(connection: Connection) -> Connection {
    let target = connection.target.within_range();
    let most = target.most_amount();
    Connection {
        source: connection.source.within_range(),
        target,
        amount: within(connection.amount, -most, most),
        one_way: connection.one_way,
        snap: connection.snap,
        scaled_by: connection.scaled_by.within_range(),
    }
}

// `connection`, already within its ranges, as the C API takes it: an amount
// in percent as a share.
fn raw_connection(connection: &Connection) -> RawConnection {
    let mut amount = connection.amount;
    if connection.target.unit() == Unit::Percent {
        amount = amount / FULL_PERCENT as f32;
    }
    RawConnection {
        source: connection.source.number(),
        target: connection.target.number(),
        amount,
        one_way: connection.one_way,
        snap: connection.snap,
        scaled_by: connection.scaled_by.number(),
    }
}

// A filter envelope stage's length brought within its range, as a C int.
fn stage_ms(ms: u32) -> c_int {
    let ms = ms.min(FILTER_LONGEST_MS);
    ms as c_int
}

// A loudness envelope stage's length brought within its range, as a C int.
fn loudness_ms(ms: u32) -> c_int {
    let ms = ms.clamp(LOUDNESS_SHORTEST_MS, LOUDNESS_LONGEST_MS);
    ms as c_int
}

// What an oscillator plays (docs/adr/0035-oscillators-with-kinds.md): the
// four waves (docs/adr/0009-how-notes-become-sound.md), and noise, a new
// random height every sample, which has no pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Sine,
    Triangle,
    Square,
    Saw,
    Noise,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Sine, Kind::Triangle, Kind::Square, Kind::Saw, Kind::Noise];

    // The kind after this one: sine, triangle, square, saw, noise, then sine
    // again.
    pub fn next(self) -> Kind {
        match self {
            Kind::Sine => Kind::Triangle,
            Kind::Triangle => Kind::Square,
            Kind::Square => Kind::Saw,
            Kind::Saw => Kind::Noise,
            Kind::Noise => Kind::Sine,
        }
    }

    // What to call the kind when showing it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Sine => "sine",
            Kind::Triangle => "triangle",
            Kind::Square => "square",
            Kind::Saw => "saw",
            Kind::Noise => "noise",
        }
    }

    // The kind called `name`, or None.
    pub fn from_name(name: &str) -> Option<Kind> {
        for kind in Kind::ALL {
            if kind.name() == name {
                return Some(kind);
            }
        }
        None
    }

    // The kind's number in the C API (`JELLO_KIND_` in
    // engine/include/jello_engine.h).
    fn number(self) -> c_int {
        match self {
            Kind::Sine => 0,
            Kind::Triangle => 1,
            Kind::Square => 2,
            Kind::Saw => 3,
            Kind::Noise => 4,
        }
    }
}

// How many oscillators play each note, and how far each can be tuned from
// the note, either way (`JELLO_OSCILLATOR_` in
// engine/include/jello_engine.h).
pub const OSCILLATOR_COUNT: usize = 3;
pub const OSCILLATOR_MOST_OCTAVES: i32 = 3;
pub const OSCILLATOR_MOST_HALF_STEPS: i32 = 12;
pub const OSCILLATOR_MOST_CENTS: i32 = 100;

// One oscillator (docs/adr/0035-oscillators-with-kinds.md): what it plays,
// how loud, and how far it is tuned from the note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oscillator {
    pub kind: Kind,
    // In percent: at 0 it is silent, and skipped.
    pub level: u32,
    // How far it is tuned from the note: whole octaves, whole half steps
    // and cents (hundredths of a half step), each from minus to plus its
    // most. Noise has no pitch, so they do nothing to it.
    pub octaves: i32,
    pub half_steps: i32,
    pub cents: i32,
}

// An oscillator that plays nothing: a triangle at level 0, right on the
// note.
const SILENT_OSCILLATOR: Oscillator = Oscillator {
    kind: Kind::Triangle,
    level: 0,
    octaves: 0,
    half_steps: 0,
    cents: 0,
};

// Where the C++ engine's oscillators start: the first a triangle at full
// level, the others silent, so a note sounds just as one wave did. A new
// song's sound profile starts there too.
pub const START_OSCILLATORS: [Oscillator; OSCILLATOR_COUNT] = [
    Oscillator {
        level: FULL_PERCENT,
        ..SILENT_OSCILLATOR
    },
    SILENT_OSCILLATOR,
    SILENT_OSCILLATOR,
];

// `oscillator` with every value brought within its range.
fn oscillator_within_ranges(oscillator: Oscillator) -> Oscillator {
    Oscillator {
        kind: oscillator.kind,
        level: oscillator.level.min(FULL_PERCENT),
        octaves: oscillator.octaves.clamp(-OSCILLATOR_MOST_OCTAVES, OSCILLATOR_MOST_OCTAVES),
        half_steps: oscillator.half_steps.clamp(-OSCILLATOR_MOST_HALF_STEPS, OSCILLATOR_MOST_HALF_STEPS),
        cents: oscillator.cents.clamp(-OSCILLATOR_MOST_CENTS, OSCILLATOR_MOST_CENTS),
    }
}

// What the looper is doing (docs/adr/0021-a-looper-on-a-beat.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LooperState {
    // No loop.
    Idle,
    // One bar of clicks before recording starts.
    CountingIn,
    // Recording, with the click on.
    Recording,
    // Playing the loop over and over, with the click off.
    Looping,
    // Playing the loop and recording a layer over it, with the click on
    // (docs/adr/0026-layers.md).
    Layering,
}

impl LooperState {
    // What to call it when showing it.
    pub fn name(self) -> &'static str {
        match self {
            LooperState::Idle => "idle",
            LooperState::CountingIn => "counting in",
            LooperState::Recording => "recording",
            LooperState::Looping => "looping",
            LooperState::Layering => "layering",
        }
    }

    // The state for its number in the C API (`JELLO_LOOPER_` in
    // engine/include/jello_engine.h).
    fn from_number(number: c_int) -> LooperState {
        match number {
            1 => LooperState::CountingIn,
            2 => LooperState::Recording,
            3 => LooperState::Looping,
            4 => LooperState::Layering,
            _ => LooperState::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LooperStatus {
    pub state: LooperState,
    // While counting in, the beat (1 to 4). While recording, the bar being
    // recorded, from 1. While looping or recording a layer, how many bars
    // the loop lasts. 0 when idle.
    pub number: u32,
    // True while a press, clear or removal is still on its way to the
    // engine's audio thread, so `state` doesn't show it yet.
    pub waiting: bool,
}

// One note of the loop (docs/adr/0024-the-looper-s-look.md). Samples count
// from the loop's start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopNote {
    pub note: u8,
    pub start: u64,
    // None while its key is still held, while recording.
    pub end: Option<u64>,
}

// One layer of the loop (docs/adr/0026-layers.md). The first loop is the
// first layer.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopLayer {
    // Its notes as played, in the order they started.
    pub notes: Vec<LoopNote>,
    // How loud it plays, in percent, from 0 to FULL_VOLUME.
    pub volume: u32,
    // True while it is being recorded.
    pub recording: bool,
}

// One chord of a song for the engine to play
// (docs/adr/0022-compose-as-a-tape.md).
#[derive(Clone, Debug, PartialEq)]
pub struct SongChord {
    // Its notes, at most 4.
    pub notes: Vec<u8>,
    // How hard they are hit, 1 to 127.
    pub velocity: u8,
    // How long it plays, in milliseconds.
    pub length_ms: u64,
}

// Where the song is playing: which of the chords sent last, and how far into
// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SongPlace {
    // The chord's place among them, from 0.
    pub chord: usize,
    // How far into it, in milliseconds.
    pub ms_in: u64,
}

// The engine keeps no sound of its own: the song's sound profile says how
// the notes sound, and sends it here
// (docs/adr/0030-a-sound-profile-per-song.md). The song sends its tempo too.
pub struct Engine {
    raw: *mut JelloEngine,
    audio_running: bool,
    // The beat's tempo in beats per minute. The C++ engine starts there too.
    tempo: u32,
    // Whether loops snap to the beat. The C++ engine starts with it on too.
    snap: bool,
    // How many samples each chord of the song sent last plays, so the
    // engine's position in the song can be turned into a chord.
    song_lengths: Vec<u64>,
    // Rust's copy of the loop's layers, oldest first, kept by the changes the
    // engine sends.
    layers: Vec<LoopLayer>,
    // A message about the looper for the top bar, kept until the screen
    // takes it.
    looper_message: Option<String>,
    // When Esc was pressed, while it waits for a layer's number.
    esc_pressed_at: Option<Instant>,
}

// The C++ engine can be used from any thread, one call at a time. The app
// keeps the engine in a Mutex, which takes care of "one call at a time".
unsafe impl Send for Engine {}

impl Engine {
    pub fn new() -> Result<Engine, String> {
        // SAFETY: create has no requirements.
        let raw = unsafe { jello_engine_create() };
        if raw.is_null() {
            return Err(String::from("the sound engine could not be created"));
        }

        let engine = Engine {
            raw,
            audio_running: false,
            tempo: START_TEMPO,
            snap: true,
            song_lengths: Vec::new(),
            layers: Vec::new(),
            looper_message: None,
            esc_pressed_at: None,
        };
        Ok(engine)
    }

    // Opens the default speakers. Does nothing if they are already open.
    pub fn start_audio(&mut self) -> Result<(), String> {
        // SAFETY: `raw` points to a live engine until Drop.
        let started = unsafe { jello_engine_start_audio(self.raw) };
        if !started {
            return Err(String::from("no audio output device could be opened"));
        }

        self.audio_running = true;
        Ok(())
    }

    // True once the speakers are open.
    pub fn is_audio_running(&self) -> bool {
        self.audio_running
    }

    // Starts `note` (0 to 127, middle C is 60) at `velocity`, how hard the key
    // was hit (1 to 127). The engine ignores anything outside those ranges.
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let note = c_int::from(note);
        let velocity = c_int::from(velocity);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_note_on(self.raw, note, velocity) };
    }

    // Lets go of `note`, so it fades out.
    pub fn note_off(&mut self, note: u8) {
        let note = c_int::from(note);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_note_off(self.raw, note) };
    }

    // Lets go of every note.
    pub fn all_notes_off(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_all_notes_off(self.raw) };
    }

    // Makes notes sent after this play oscillator `index`, from 0, as
    // `oscillator` says (`jello_engine_set_oscillator`). Notes sent before it
    // keep theirs, even while they ring on. Values outside their ranges are
    // brought within them, and an index past OSCILLATOR_COUNT is ignored.
    // Returns the oscillator as sent.
    pub fn set_oscillator(&mut self, index: usize, oscillator: Oscillator) -> Oscillator {
        let oscillator = oscillator_within_ranges(oscillator);
        if index >= OSCILLATOR_COUNT {
            return oscillator;
        }
        let level = share_of(oscillator.level) as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe {
            jello_engine_set_oscillator(
                self.raw,
                index as c_int,
                oscillator.kind.number(),
                level,
                oscillator.octaves as c_int,
                oscillator.half_steps as c_int,
                oscillator.cents as f32,
            )
        };
        oscillator
    }

    // Makes notes sent after this play `kind` on the first oscillator, at
    // full level and right on the note, as a new engine does with the
    // triangle. Only the tests use it, to play one kind on its own.
    #[cfg(test)]
    pub fn set_first_kind(&mut self, kind: Kind) {
        let oscillator = Oscillator {
            kind,
            ..START_OSCILLATORS[0]
        };
        self.set_oscillator(0, oscillator);
    }

    // The click on its own, without the looper. Only the tests use it for
    // now; a metronome key can later.

    // Starts the click on beat 1 of a bar. Does nothing if it is already on.
    #[cfg(test)]
    pub fn start_click(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_start_click(self.raw) };
    }

    // Stops the click. A tick already sounding rings out.
    #[cfg(test)]
    pub fn stop_click(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_stop_click(self.raw) };
    }

    // Right Shift's looper (`jello_engine_press_looper`): counts in, records,
    // closes the loop at the nearest bar line, then starts and stops layers.
    // A loop with MAX_LAYERS layers takes no more: the engine ignores the
    // press, and a message says why.
    pub fn press_looper(&mut self) {
        let status = self.looper_status();
        let full = self.loop_layers().len() == MAX_LAYERS;
        if status.state == LooperState::Looping {
            if full {
                self.looper_message = Some(format!("{MAX_LAYERS} layers is the most, Esc removes one"));
            }
        }
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_press_looper(self.raw) };
    }

    // Stops every layer's notes and forgets the loop, or cancels the
    // count-in or the recording. Live notes play on. Only the tests clear
    // the whole loop at once for now; Esc removes a layer at a time.
    #[cfg(test)]
    pub fn clear_loop(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_clear_loop(self.raw) };
    }

    // Stops the newest layer's notes and forgets it
    // (`jello_engine_remove_layer`). Removing the only layer clears the
    // loop, and so does removing any during the count-in or the first
    // recording.
    pub fn remove_newest_layer(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_remove_layer(self.raw, NEWEST_LAYER) };
    }

    // The same for layer `number`, counted from 1 in the order the layers
    // were recorded. The layers after it move up. A layer that doesn't
    // exist changes nothing.
    pub fn remove_layer(&mut self, number: u32) {
        if number == 0 {
            return;
        }
        // A number too big for a C int becomes -2, which the engine ignores.
        let layer = c_int::try_from(number - 1).unwrap_or(-2);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_remove_layer(self.raw, layer) };
    }

    // What Esc does at `now` (docs/adr/0026-layers.md). With two layers or
    // more, it waits half a second for a layer's number (`pick_layer`), and
    // if none comes the newest goes (`finish_esc_wait`). With fewer there is
    // nothing to pick from, so the newest goes at once: that cancels the
    // count-in or the recording, or clears a loop of one layer. Pressed
    // again while it waits, the newest goes at once.
    pub fn press_esc(&mut self, now: Instant) {
        if self.is_esc_waiting(now) {
            self.esc_pressed_at = None;
            self.remove_newest_layer();
            return;
        }
        if self.loop_layers().len() < 2 {
            self.remove_newest_layer();
            return;
        }
        self.esc_pressed_at = Some(now);
    }

    // While Esc waits, layer `number` (from 1) goes instead of the newest,
    // and this returns true: the key was used to pick it. A number with no
    // layer removes nothing and leaves a message saying so. Returns false
    // when Esc isn't waiting, so the key plays as usual.
    pub fn pick_layer(&mut self, number: u32, now: Instant) -> bool {
        if !self.is_esc_waiting(now) {
            return false;
        }
        self.esc_pressed_at = None;

        let count = self.loop_layers().len();
        if number as usize > count {
            self.looper_message = Some(format!("There is no layer {number}"));
            return true;
        }
        self.remove_layer(number);
        true
    }

    // Once Esc has waited half a second with no layer's number, the newest
    // layer goes. Every command that shows the looper calls this first, and
    // the screen asks every 50 ms while the looper is busy, so it goes at
    // most that much late.
    pub fn finish_esc_wait(&mut self, now: Instant) {
        let pressed_at = match self.esc_pressed_at {
            Some(pressed_at) => pressed_at,
            None => return,
        };
        if now.saturating_duration_since(pressed_at) < ESC_WAIT {
            return;
        }
        self.esc_pressed_at = None;
        self.remove_newest_layer();
    }

    // True while Esc waits for a layer's number at `now`.
    fn is_esc_waiting(&self, now: Instant) -> bool {
        let pressed_at = match self.esc_pressed_at {
            Some(pressed_at) => pressed_at,
            None => return false,
        };
        now.saturating_duration_since(pressed_at) < ESC_WAIT
    }

    // While Esc waits, the layer it removes if no number comes: the newest,
    // counted from 1. None otherwise.
    pub fn esc_removes(&mut self, now: Instant) -> Option<u32> {
        if !self.is_esc_waiting(now) {
            return None;
        }
        let newest = self.loop_layers().len();
        u32::try_from(newest).ok()
    }

    // Sets layer `number`'s volume (counted from 1) to `percent`, from 0 to
    // FULL_VOLUME, its notes already sounding too
    // (`jello_engine_set_layer_volume`). Fails for a layer that doesn't
    // exist or a volume above FULL_VOLUME.
    pub fn set_layer_volume(&mut self, number: u32, percent: u32) -> Result<(), String> {
        if percent > FULL_VOLUME {
            return Err(format!("a layer's volume goes from 0 to {FULL_VOLUME}, not {percent}"));
        }
        let count = self.loop_layers().len();
        if number == 0 {
            return Err(String::from("layers are counted from 1"));
        }
        if number as usize > count {
            return Err(format!("there is no layer {number}"));
        }

        let place = number as usize - 1;
        let layer = place as c_int;
        let volume = percent as f32 / FULL_VOLUME as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_layer_volume(self.raw, layer, volume) };
        self.layers[place].volume = percent;
        Ok(())
    }

    // The message about the looper waiting for the top bar, if there is
    // one. Taking it means it shows once.
    pub fn take_looper_message(&mut self) -> Option<String> {
        self.looper_message.take()
    }

    // Turns "snap to beat" on or off (`jello_engine_set_snap`). A loop
    // already playing changes right away.
    pub fn set_snap(&mut self, snap: bool) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_snap(self.raw, snap) };
        self.snap = snap;
    }

    // Turns "snap to beat" off if it is on, and on if it is off.
    pub fn switch_snap(&mut self) {
        let snap = !self.snap;
        self.set_snap(snap);
    }

    // True while loops snap to the beat.
    pub fn snap(&self) -> bool {
        self.snap
    }

    // Where a loop `bars` bars long plays one layer's `notes` each time
    // around, by the engine's own rule (`jello_engine_plan_loop`): moved to
    // the nearest half beat while snap is on, and as played while it is off.
    // They come in the order they start each time around. A note still held counts as
    // ending where it starts. If the engine can't work it out, the notes
    // come back as they are.
    pub fn plan_loop(&self, notes: &[LoopNote], bars: u32) -> Vec<LoopNote> {
        let mut played = Vec::new();
        for note in notes {
            let raw = RawLoopNote {
                note: c_int::from(note.note),
                start: note.start,
                end: note.end.unwrap_or(note.start),
            };
            played.push(raw);
        }
        // Room for as many notes as were played.
        let mut planned = played.clone();

        // Numbers too big for a C int become -1 or 0, which the engine
        // refuses.
        let count = c_int::try_from(played.len()).unwrap_or(-1);
        let bars = c_int::try_from(bars).unwrap_or(0);
        let tempo = c_int::try_from(self.tempo).unwrap_or(0);
        // SAFETY: `raw` points to a live engine until Drop, and `played` and
        // `planned` both hold exactly `count` notes.
        let worked_out = unsafe {
            jello_engine_plan_loop(
                self.raw,
                played.as_ptr(),
                count,
                bars,
                tempo,
                self.snap,
                planned.as_mut_ptr(),
            )
        };
        if !worked_out {
            return notes.to_vec();
        }

        let mut plan = Vec::new();
        for raw in planned {
            let note = LoopNote {
                note: u8::try_from(raw.note).unwrap_or(0),
                start: raw.start,
                end: Some(raw.end),
            };
            plan.push(note);
        }
        plan
    }

    // What the looper is doing, as of the engine's last chunk of sound.
    pub fn looper_status(&self) -> LooperStatus {
        // SAFETY: `raw` points to a live engine until Drop.
        let raw = unsafe { jello_engine_looper_status(self.raw) };
        let number = u32::try_from(raw.number).unwrap_or(0);
        LooperStatus {
            state: LooperState::from_number(raw.state),
            number,
            waiting: raw.waiting,
        }
    }

    // How many samples into the count-in, the recording or the repeat
    // playing now the looper is, as of the engine's last chunk of sound. 0
    // when idle. Each starts on beat 1 of a bar.
    pub fn looper_position(&self) -> u64 {
        // SAFETY: `raw` points to a live engine until Drop.
        let raw = unsafe { jello_engine_looper_status(self.raw) };
        raw.position
    }

    // The loop's layers, oldest first, as of the engine's last chunk of
    // sound. First takes in every change the engine has sent since the last
    // time.
    pub fn loop_layers(&mut self) -> &[LoopLayer] {
        loop {
            let mut change = RawLoopChange {
                change: LOOP_FORGOT,
                layer: 0,
                index: 0,
                note: 0,
                sample: 0,
            };
            // SAFETY: `raw` points to a live engine until Drop, and `change`
            // is ours to fill.
            let taken = unsafe { jello_engine_next_loop_change(self.raw, &mut change) };
            if !taken {
                break;
            }
            self.apply_loop_change(&change);
        }
        &self.layers
    }

    // Changes Rust's copy of the loop's layers the way the engine changed its
    // own.
    fn apply_loop_change(&mut self, change: &RawLoopChange) {
        let layer = usize::try_from(change.layer).unwrap_or(0);
        let index = usize::try_from(change.index).unwrap_or(0);
        if change.change == LOOP_FORGOT {
            self.layers.clear();
        } else if change.change == LOOP_LAYER_STARTED {
            let started = LoopLayer {
                notes: Vec::new(),
                volume: FULL_VOLUME,
                recording: true,
            };
            self.layers.push(started);
        } else if change.change == LOOP_STARTED {
            let note = LoopNote {
                note: u8::try_from(change.note).unwrap_or(0),
                start: change.sample,
                end: None,
            };
            if let Some(recording) = self.layers.get_mut(layer) {
                recording.notes.push(note);
            }
        } else if change.change == LOOP_ENDED {
            if let Some(ended) = self.loop_note_mut(layer, index) {
                ended.end = Some(change.sample);
            }
        } else if change.change == LOOP_KEPT {
            if let Some(closed) = self.layers.get_mut(layer) {
                closed.notes.truncate(index);
                closed.recording = false;
            }
        } else if change.change == LOOP_LAYER_STOPPED {
            if let Some(stopped) = self.layers.get_mut(layer) {
                stopped.recording = false;
            }
        } else if change.change == LOOP_LAYER_DROPPED {
            self.forget_layer(layer);
            self.looper_message = Some(String::from("Nothing played, no layer added"));
        } else if change.change == LOOP_LAYER_REMOVED {
            self.forget_layer(layer);
        }
    }

    // Note `index` of layer `layer` in Rust's copy, if both exist.
    fn loop_note_mut(&mut self, layer: usize, index: usize) -> Option<&mut LoopNote> {
        let holding = self.layers.get_mut(layer)?;
        holding.notes.get_mut(index)
    }

    // Takes layer `layer` out of Rust's copy, if it exists. The layers after
    // it move up.
    fn forget_layer(&mut self, layer: usize) {
        if layer < self.layers.len() {
            self.layers.remove(layer);
        }
    }

    // True from the looper's first press until it is cleared, counting a
    // press still on its way.
    pub fn is_looper_busy(&self) -> bool {
        let status = self.looper_status();
        if status.waiting {
            return true;
        }
        status.state != LooperState::Idle
    }

    // Plays `chords` one after another, each for its length, with no gap
    // between them, from the engine's next chunk of sound. Whatever the song
    // was playing stops first. Fails and plays nothing if there are more
    // than SONG_MAX_CHORDS chords, a chord has more than 4 notes, a note or
    // velocity is outside MIDI's range, or too many messages are waiting.
    // The engine checks the number of chords itself, so it isn't checked
    // twice here.
    pub fn play_song(&mut self, chords: &[SongChord]) -> Result<(), String> {
        let mut raw_chords = Vec::new();
        let mut lengths = Vec::new();
        for chord in chords {
            let raw = self.raw_song_chord(chord)?;
            lengths.push(raw.length);
            raw_chords.push(raw);
        }

        let too_many = format!("a song plays at most {SONG_MAX_CHORDS} chords");
        let count = match c_int::try_from(raw_chords.len()) {
            Ok(count) => count,
            Err(_) => return Err(too_many),
        };
        // SAFETY: `raw` points to a live engine until Drop, and `raw_chords`
        // holds exactly `count` chords.
        let played = unsafe { jello_engine_play_song(self.raw, raw_chords.as_ptr(), count) };
        if !played {
            return Err(format!(
                "the engine did not take the song: {too_many}, a note or velocity is outside MIDI's range, or too many messages are waiting",
            ));
        }

        self.song_lengths = lengths;
        Ok(())
    }

    // `chord` as the C API takes it, with its length turned into samples.
    fn raw_song_chord(&self, chord: &SongChord) -> Result<RawSongChord, String> {
        if chord.notes.len() > CHORD_MAX_NOTES {
            return Err(format!("a chord in a song has at most {CHORD_MAX_NOTES} notes"));
        }
        let mut notes = [0; CHORD_MAX_NOTES];
        for (place, note) in chord.notes.iter().enumerate() {
            notes[place] = c_int::from(*note);
        }

        // 48 samples a millisecond, so a chord held 2,000 ms plays 96,000.
        let length = chord.length_ms * u64::from(self.sample_rate()) / 1000;

        let raw = RawSongChord {
            notes,
            note_count: chord.notes.len() as c_int,
            velocity: c_int::from(chord.velocity),
            length,
        };
        Ok(raw)
    }

    // Lets go of what the song is playing and forgets it. Live notes and the
    // loop play on.
    pub fn stop_song(&mut self) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_stop_song(self.raw) };
    }

    // Where the song is, as of the engine's last chunk of sound, or None
    // when no song is playing. A song sent but not yet started counts as at
    // its very start.
    pub fn song_place(&self) -> Option<SongPlace> {
        // SAFETY: `raw` points to a live engine until Drop.
        let position = unsafe { jello_engine_song_position(self.raw) };

        // -1 means no song is playing.
        let mut samples_left = match u64::try_from(position) {
            Ok(position) => position,
            Err(_) => return None,
        };

        // Walk the chords until the position falls inside one. A chord 0
        // samples long is never playing.
        for (chord, length) in self.song_lengths.iter().enumerate() {
            if samples_left < *length {
                let ms_in = samples_left * 1000 / u64::from(self.sample_rate());
                return Some(SongPlace { chord, ms_in });
            }
            samples_left -= length;
        }

        // Played to its very end: its last chord lets go on the next sample.
        None
    }

    // Plays the beat at `tempo` beats per minute from the next beat on. The
    // engine ignores a tempo outside SLOWEST_TEMPO to FASTEST_TEMPO, and so
    // does the tempo kept here. While the looper is busy, the engine keeps
    // its beat, so this changes nothing. Returns the tempo the beat plays
    // now, which the song keeps (src/compose.rs).
    pub fn set_tempo(&mut self, tempo: u32) -> u32 {
        // Not sent at all, so the tempo kept here and the engine's always
        // agree. Sent anyway, it could go wrong: with Esc still on its way,
        // the engine would clear the loop first and then take the tempo.
        if self.is_looper_busy() {
            return self.tempo;
        }

        // A number too big for a C int becomes -1, which the engine ignores.
        let number = c_int::try_from(tempo).unwrap_or(-1);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tempo(self.raw, number) };

        if tempo < SLOWEST_TEMPO {
            return self.tempo;
        }
        if tempo > FASTEST_TEMPO {
            return self.tempo;
        }
        self.tempo = tempo;
        self.tempo
    }

    // The tempo the beat plays, in beats per minute.
    pub fn tempo(&self) -> u32 {
        self.tempo
    }

    // Moves the filter (`jello_engine_set_filter`). Values outside its
    // ranges are brought within them. Every note hears it, even those
    // already sounding. Returns the filter as sent.
    pub fn set_filter(&mut self, filter: Filter) -> Filter {
        let cutoff_hz = within(filter.cutoff_hz, FILTER_LOWEST_HZ, FILTER_OPEN_HZ);
        let resonance = filter.resonance.min(FULL_PERCENT);
        let share = resonance as f32 / FULL_PERCENT as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_filter(self.raw, cutoff_hz, share) };
        Filter { cutoff_hz, resonance }
    }

    // Sets the filter envelope (`jello_engine_set_filter_envelope`) for the
    // notes started after this. Values outside its ranges are brought
    // within them. Returns the envelope as sent.
    pub fn set_filter_envelope(&mut self, envelope: FilterEnvelope) -> FilterEnvelope {
        let amount = within(envelope.amount, 0.0, FILTER_MOST_OCTAVES);
        let sustain = envelope.sustain.min(FULL_PERCENT);
        let share = sustain as f32 / FULL_PERCENT as f32;
        let attack = stage_ms(envelope.attack_ms);
        let decay = stage_ms(envelope.decay_ms);
        let release = stage_ms(envelope.release_ms);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_filter_envelope(self.raw, amount, attack, decay, share, release) };
        FilterEnvelope {
            amount,
            attack_ms: attack as u32,
            decay_ms: decay as u32,
            sustain,
            release_ms: release as u32,
        }
    }

    // Sets unison (`jello_engine_set_unison`) for the notes started after
    // this. Values outside its ranges are brought within them. Returns
    // unison as sent.
    pub fn set_unison(&mut self, unison: Unison) -> Unison {
        let copies = unison.copies.clamp(1, UNISON_MOST_COPIES);
        let detune_cents = unison.detune_cents.min(UNISON_MOST_DETUNE_CENTS);
        let spread = unison.spread.min(FULL_PERCENT);
        let share = spread as f32 / FULL_PERCENT as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_unison(self.raw, copies as c_int, detune_cents as f32, share) };
        Unison {
            copies,
            detune_cents,
            spread,
        }
    }

    // Sets the loudness envelope (`jello_engine_set_loudness_envelope`) for
    // the notes started after this. Values outside its ranges are brought
    // within them. Returns the envelope as sent.
    pub fn set_loudness_envelope(&mut self, envelope: LoudnessEnvelope) -> LoudnessEnvelope {
        let attack = loudness_ms(envelope.attack_ms);
        let decay = loudness_ms(envelope.decay_ms);
        let sustain = envelope.sustain.min(FULL_PERCENT);
        let share = sustain as f32 / FULL_PERCENT as f32;
        let release = loudness_ms(envelope.release_ms);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_loudness_envelope(self.raw, attack, decay, share, release) };
        LoudnessEnvelope {
            attack_ms: attack as u32,
            decay_ms: decay as u32,
            sustain,
            release_ms: release as u32,
        }
    }

    // Sets key tracking (`jello_engine_set_key_tracking`), in percent, for
    // the notes started after this: how much each note's cutoff follows
    // middle C's. Above 100 counts as 100. Returns it as sent.
    pub fn set_key_tracking(&mut self, key_tracking: u32) -> u32 {
        let key_tracking = key_tracking.min(FULL_PERCENT);
        let share = key_tracking as f32 / FULL_PERCENT as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_key_tracking(self.raw, share) };
        key_tracking
    }

    // Tunes the notes started after this from A4 at `a4_hz`
    // (`jello_engine_set_tuning`), brought within TUNING_LOWEST_HZ to
    // TUNING_HIGHEST_HZ. Returns it as sent.
    pub fn set_tuning(&mut self, a4_hz: f32) -> f32 {
        let a4_hz = within(a4_hz, TUNING_LOWEST_HZ, TUNING_HIGHEST_HZ);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tuning(self.raw, a4_hz) };
        a4_hz
    }

    // Sets LFO `index`, from 0 (`jello_engine_set_lfo`). Values outside
    // their ranges are brought within them, and an index past LFO_COUNT is
    // ignored. Every note hears it from the next 32 samples on. Returns the
    // LFO as sent.
    pub fn set_lfo(&mut self, index: usize, lfo: Lfo) -> Lfo {
        let lfo = lfo_within_ranges(lfo);
        if index >= LFO_COUNT {
            return lfo;
        }
        let raw = raw_lfo(&lfo);
        // SAFETY: `raw` points to a live engine until Drop, and the LFO's
        // settings live until the call returns.
        unsafe { jello_engine_set_lfo(self.raw, index as c_int, &raw) };
        lfo
    }

    // Sets connection `index` of the routing list, from 0
    // (`jello_engine_set_connection`). Values outside their ranges are
    // brought within them, and an index past CONNECTION_COUNT is ignored.
    // Returns the connection as sent.
    pub fn set_connection(&mut self, index: usize, connection: Connection) -> Connection {
        let connection = connection_within_ranges(connection);
        if index >= CONNECTION_COUNT {
            return connection;
        }
        let raw = raw_connection(&connection);
        // SAFETY: `raw` points to a live engine until Drop, and the
        // connection lives until the call returns.
        unsafe { jello_engine_set_connection(self.raw, index as c_int, &raw) };
        connection
    }

    // Sets how long the vibrato key takes to rise and fall
    // (`jello_engine_set_vibrato_key_times`), each brought within
    // VIBRATO_KEY_LONGEST_MS. Returns them as sent.
    pub fn set_vibrato_key_times(&mut self, times: VibratoKeyTimes) -> VibratoKeyTimes {
        let rise_ms = times.rise_ms.min(VIBRATO_KEY_LONGEST_MS);
        let fall_ms = times.fall_ms.min(VIBRATO_KEY_LONGEST_MS);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_vibrato_key_times(self.raw, rise_ms as c_int, fall_ms as c_int) };
        VibratoKeyTimes { rise_ms, fall_ms }
    }

    // Holds or lets go of the live vibrato key
    // (`jello_engine_set_vibrato_key`, docs/adr/0032-the-vibrato-key.md).
    pub fn set_vibrato_key(&mut self, held: bool) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_vibrato_key(self.raw, held) };
    }

    pub fn sample_rate(&self) -> u32 {
        // SAFETY: `raw` points to a live engine until Drop.
        let rate = unsafe { jello_engine_sample_rate(self.raw) };
        rate as u32
    }

    // Makes `frames` frames of sound without speakers, for writing files.
    // Returns the samples as left, right, left, right, ... Notes sent before
    // this call take effect at its first frame.
    pub fn render(&mut self, frames: usize) -> Vec<f32> {
        // The speakers' audio thread would be rendering at the same time.
        assert!(
            !self.audio_running,
            "render can only be used on an engine whose audio is not running"
        );

        let frame_count = c_int::try_from(frames).expect("too many frames to render at once");
        let mut samples = vec![0.0; frames * CHANNELS];

        // SAFETY: `raw` points to a live engine until Drop, and `samples` has
        // room for exactly `frames` stereo frames.
        unsafe { jello_engine_render(self.raw, samples.as_mut_ptr(), frame_count) };

        samples
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: `raw` came from create and is never used after this.
        unsafe { jello_engine_destroy(self.raw) };
    }
}

// These tests play the real C++ engine through the wrapper above, rendering
// without speakers, and measure the samples it makes.
#[cfg(test)]
mod tests {
    use super::*;

    use std::f64::consts::PI;

    use crate::modulation::{
        LfoShape, LfoSpeed, NO_CONNECTION, START_LFO, START_VIBRATO_KEY_TIMES, Source, Target,
    };

    // The sound the C++ engine makes. When it is changed there on purpose,
    // change it here too.
    const VOICE_LEVEL: f32 = 0.35; // `voice_level` in engine/src/voice.cpp
    // A new engine's loudness envelope (`LoudnessEnvelopeSettings` in
    // engine/src/envelope.h, and START_LOUDNESS_ENVELOPE).
    const ATTACK_MS: u32 = 5;
    const DECAY_MS: u32 = 300;
    const SUSTAIN: f32 = 1.0;
    const RELEASE_MS: u32 = 300;
    const MAILBOX_SLOTS: usize = 1024; // `EventMailbox` in engine/src/mailbox.h
    const VOICE_COUNT: u8 = 32; // `voice_count` in engine/src/synth.cpp
    const CLIP_START: f32 = 0.8; // `clip_start` in engine/src/conductor.cpp

    // Notes by their MIDI numbers, and how many times a second their waves
    // repeat in standard tuning.
    const C3: u8 = 48;
    const C3_HZ: f64 = 130.8128;
    const MIDDLE_C: u8 = 60;
    const MIDDLE_C_HZ: f64 = 261.6256;
    const E4: u8 = 64;
    const E4_HZ: f64 = 329.6276;
    const G4: u8 = 67;
    const A4: u8 = 69;
    const A4_HZ: f64 = 440.0;
    const C5: u8 = 72;
    const C5_HZ: f64 = 523.2511;
    const A7: u8 = 105;
    const A7_HZ: f64 = 3520.0;

    // A firm key press, used unless a test says otherwise.
    const VELOCITY: u8 = 100;

    // A key press so soft it can't be heard, though the note still takes a
    // voice.
    const QUIET: u8 = 1;

    fn frames_in_ms(engine: &Engine, ms: u32) -> usize {
        let frames = engine.sample_rate() * ms / 1000;
        frames as usize
    }

    // Renders `ms` milliseconds and keeps only the left channel. The right
    // channel is the same unless unison spreads its copies apart.
    fn render_left(engine: &mut Engine, ms: u32) -> Vec<f32> {
        let frames = frames_in_ms(engine, ms);
        let samples = engine.render(frames);

        let mut left = Vec::new();
        for frame in samples.chunks(CHANNELS) {
            left.push(frame[0]);
        }
        left
    }

    // The loudest sample, ignoring whether it is above or below zero.
    fn peak(samples: &[f32]) -> f32 {
        let mut loudest = 0.0;
        for sample in samples {
            if sample.abs() > loudest {
                loudest = sample.abs();
            }
        }
        loudest
    }

    // How much a key press at `velocity` scales a note's loudness, from 0
    // to 1. Matches `velocity_to_peak` in engine/src/voice.cpp.
    fn velocity_gain(velocity: u8) -> f32 {
        let amount = velocity as f32 / 127.0;
        amount * amount
    }

    // How loud a note at VELOCITY gets at the top of its attack.
    fn full_level() -> f32 {
        VOICE_LEVEL * velocity_gain(VELOCITY)
    }

    // How loud a note at VELOCITY settles while its key is held.
    fn held_level() -> f32 {
        full_level() * SUSTAIN
    }

    // Presses `note`, waits out the attack and decay, then returns one second
    // of the note being held.
    fn hold(engine: &mut Engine, note: u8, velocity: u8) -> Vec<f32> {
        engine.note_on(note, velocity);
        render_left(engine, ATTACK_MS + DECAY_MS);
        render_left(engine, 1000)
    }

    // Checks that `actual` is within `percent` percent of `expected`. Samples
    // land at slightly different points on the wave each cycle, so measured
    // loudness is never exact.
    fn assert_within_percent(actual: f32, expected: f32, percent: f32) {
        let allowed = expected * percent / 100.0;
        let difference = (actual - expected).abs();
        assert!(
            difference <= allowed,
            "got {actual}, expected {expected} give or take {percent}%"
        );
    }

    // The pitch of `wave` in Hz, from how often it crosses zero on the way up.
    // Each crossing falls between two samples. Sine, triangle and saw waves
    // are close to straight lines there, so a straight line between the two
    // samples finds it almost exactly. A square jumps across zero instead,
    // so each of its crossings can be off by up to half a sample. Over a
    // whole second those misses barely count: for A4 they move the pitch by
    // less than 0.001 Hz.
    fn pitch(engine: &Engine, wave: &[f32]) -> f64 {
        let mut first_crossing = 0.0;
        let mut last_crossing = 0.0;
        let mut crossings = 0;

        for i in 1..wave.len() {
            let before = wave[i - 1] as f64;
            let after = wave[i] as f64;
            if before < 0.0 && after >= 0.0 {
                // How far from `before` to `after` the line reaches zero.
                let fraction = before / (before - after);
                let crossing = (i - 1) as f64 + fraction;
                if crossings == 0 {
                    first_crossing = crossing;
                }
                last_crossing = crossing;
                crossings += 1;
            }
        }

        let cycles = (crossings - 1) as f64;
        let seconds = (last_crossing - first_crossing) / engine.sample_rate() as f64;
        cycles / seconds
    }

    fn assert_pitch(engine: &Engine, wave: &[f32], expected_hz: f64) {
        let hz = pitch(engine, wave);
        let difference = (hz - expected_hz).abs();
        assert!(
            difference < 0.01,
            "played {hz} Hz, expected {expected_hz} Hz"
        );
    }

    // Checks which note `wave` is playing, to within 1 Hz. That still tells
    // apart any two notes these tests listen to, and suits a short, fading
    // wave, which can't be measured to 0.01 Hz like a long, steady one.
    fn assert_note(engine: &Engine, wave: &[f32], expected_hz: f64) {
        let hz = pitch(engine, wave);
        let difference = (hz - expected_hz).abs();
        assert!(
            difference < 1.0,
            "played {hz} Hz, expected {expected_hz} Hz"
        );
    }

    #[test]
    fn a_new_engine_is_silent() {
        let mut engine = Engine::new().unwrap();
        let wave = render_left(&mut engine, 1000);
        assert_eq!(peak(&wave), 0.0);
    }

    #[test]
    fn middle_c_plays_at_261_hz() {
        let mut engine = Engine::new().unwrap();
        let wave = hold(&mut engine, MIDDLE_C, VELOCITY);
        assert_pitch(&engine, &wave, MIDDLE_C_HZ);
    }

    #[test]
    fn a4_plays_at_440_hz() {
        let mut engine = Engine::new().unwrap();
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_pitch(&engine, &wave, A4_HZ);
    }

    #[test]
    fn a_note_starts_right_away_without_a_click() {
        let mut engine = Engine::new().unwrap();
        engine.note_on(MIDDLE_C, VELOCITY);
        let first_ms = render_left(&mut engine, 1);

        // It starts on the very first sample, from zero.
        assert_eq!(first_ms[0], 0.0);
        assert!(first_ms[1] > 0.0);

        // But gently: 1 ms into the 5 ms attack, it is at most a fifth of the
        // way up. Jumping straight to full loudness would click.
        let loudest_after_1_ms = full_level() / ATTACK_MS as f32;
        assert!(peak(&first_ms) <= loudest_after_1_ms);
    }

    #[test]
    fn a_held_note_settles_at_the_sustain_level() {
        let mut engine = Engine::new().unwrap();
        let wave = hold(&mut engine, MIDDLE_C, VELOCITY);
        assert_within_percent(peak(&wave), held_level(), 2.0);
    }

    #[test]
    fn a_note_never_goes_above_full_loudness() {
        let mut engine = Engine::new().unwrap();
        engine.note_on(MIDDLE_C, VELOCITY);

        // The attack, the decay and the start of the sustain.
        let wave = render_left(&mut engine, 1000);
        assert!(peak(&wave) <= full_level());
    }

    #[test]
    fn velocity_64_is_about_a_quarter_as_loud_as_127() {
        let mut soft = Engine::new().unwrap();
        let mut hard = Engine::new().unwrap();
        let soft_wave = hold(&mut soft, MIDDLE_C, 64);
        let hard_wave = hold(&mut hard, MIDDLE_C, 127);

        // Loudness follows the velocity squared, and (64 / 127) squared is
        // about 0.254.
        let ratio = peak(&soft_wave) / peak(&hard_wave);
        assert_within_percent(ratio, 0.254, 1.0);
    }

    // Checks that two sounds match, sample for sample. The engine adds in
    // floats, which can round the last digit a little differently, so a
    // millionth either way is allowed.
    fn assert_same_sound(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for i in 0..actual.len() {
            let difference = (actual[i] - expected[i]).abs();
            assert!(
                difference <= 0.000001,
                "sample {i} is {}, expected {}",
                actual[i],
                expected[i]
            );
        }
    }

    // What the soft clip does to one sample. Matches `soft_clip` in
    // engine/src/conductor.cpp.
    fn soft_clip(sample: f32) -> f32 {
        let size = sample.abs();
        if size <= CLIP_START {
            return sample;
        }

        let room = 1.0 - CLIP_START;
        let over = size - CLIP_START;
        let share = over / (over + room);
        let squeezed = CLIP_START + share * room;

        if sample < 0.0 {
            -squeezed
        } else {
            squeezed
        }
    }

    // Plays `notes` together, then each note alone on a new engine, and
    // checks that the chord is the lone notes added up, then put through the
    // soft clip. Below 0.8 the soft clip changes nothing, so a quiet chord
    // must be exactly its notes added up.
    fn assert_chord_adds_up(notes: &[u8], velocity: u8) {
        let mut engine = Engine::new().unwrap();
        for note in notes {
            engine.note_on(*note, velocity);
        }
        let chord = render_left(&mut engine, 1000);

        let mut added_up = vec![0.0; chord.len()];
        for note in notes {
            let mut alone = Engine::new().unwrap();
            alone.note_on(*note, velocity);
            let wave = render_left(&mut alone, 1000);
            for i in 0..added_up.len() {
                added_up[i] += wave[i];
            }
        }

        let mut expected = Vec::new();
        for sample in added_up {
            expected.push(soft_clip(sample));
        }
        assert_same_sound(&chord, &expected);
    }

    #[test]
    fn a_chord_is_its_notes_added_together() {
        // C major: middle C, E and G. Even if their peaks lined up, they
        // would add up to only 0.65.
        assert_chord_adds_up(&[MIDDLE_C, E4, G4], VELOCITY);
    }

    #[test]
    fn all_32_voices_play_at_once() {
        // 32 notes in a row, from C3 up. Softly, so that all together they
        // stay below 0.8: even if every peak lined up, they would add up to
        // only 0.62.
        let mut notes = Vec::new();
        for note in C3..C3 + VOICE_COUNT {
            notes.push(note);
        }
        assert_chord_adds_up(&notes, 30);
    }

    // Checks a release, from the moment the key is let go. Right after, the
    // note is still nearly as loud, so it fades instead of stopping dead
    // (which would click). It keeps fading for the whole release, and then
    // it is exact silence.
    fn assert_fades_out(engine: &Engine, fade: &[f32], after: &[f32]) {
        let ten_ms = frames_in_ms(engine, 10);

        let first_10_ms = &fade[..ten_ms];
        assert!(peak(first_10_ms) > held_level() * 0.9);

        let last_10_ms = &fade[fade.len() - ten_ms..];
        assert!(peak(last_10_ms) > 0.0);

        assert_eq!(peak(after), 0.0);
    }

    #[test]
    fn letting_go_fades_to_exact_silence() {
        let mut engine = Engine::new().unwrap();
        hold(&mut engine, MIDDLE_C, VELOCITY);

        engine.note_off(MIDDLE_C);
        let fade = render_left(&mut engine, RELEASE_MS);
        let after = render_left(&mut engine, 100);

        assert_fades_out(&engine, &fade, &after);
    }

    #[test]
    fn all_notes_off_fades_to_exact_silence() {
        // A chord, so every voice has to let go, not just one.
        let mut engine = Engine::new().unwrap();
        engine.note_on(MIDDLE_C, VELOCITY);
        engine.note_on(E4, VELOCITY);
        engine.note_on(G4, VELOCITY);
        render_left(&mut engine, 1000);

        engine.all_notes_off();
        let fade = render_left(&mut engine, RELEASE_MS);
        let after = render_left(&mut engine, 100);

        assert_fades_out(&engine, &fade, &after);
    }

    #[test]
    fn letting_go_of_another_note_changes_nothing() {
        let mut engine = Engine::new().unwrap();
        hold(&mut engine, MIDDLE_C, VELOCITY);

        // E4 is not playing, so middle C must keep sounding, even after a
        // release would have ended.
        engine.note_off(E4);
        render_left(&mut engine, RELEASE_MS);
        let wave = render_left(&mut engine, 50);

        assert_within_percent(peak(&wave), held_level(), 2.0);
    }

    #[test]
    fn letting_go_of_one_note_leaves_the_others_ringing() {
        let mut engine = Engine::new().unwrap();
        engine.note_on(MIDDLE_C, VELOCITY);
        engine.note_on(E4, VELOCITY);
        render_left(&mut engine, 500);

        // Once middle C has faded out, only E4 is left.
        engine.note_off(MIDDLE_C);
        render_left(&mut engine, RELEASE_MS);
        let wave = render_left(&mut engine, 1000);
        assert_pitch(&engine, &wave, E4_HZ);
    }

    // Checks that the sound never jumps suddenly from one sample to the next,
    // which is what a click is. A triangle wave climbs from -1 to 1 in half a
    // cycle, so each sample it moves at most 4 / (samples per cycle), times
    // its loudness. The fastest wave in `wave` sets that limit. The volume
    // moves at the same time, so half as much again is allowed on top.
    fn assert_no_click(engine: &Engine, wave: &[f32], fastest_hz: f64) {
        assert_no_click_steeper(engine, wave, fastest_hz, 1.0);
    }

    // As assert_no_click, for sound that can move up to `steeper` times as
    // fast as one note alone, like unison's copies added up.
    fn assert_no_click_steeper(engine: &Engine, wave: &[f32], fastest_hz: f64, steeper: f32) {
        let samples_per_cycle = engine.sample_rate() as f64 / fastest_hz;
        let wave_step = (4.0 / samples_per_cycle) as f32;
        let steepest = wave_step * full_level() * steeper;
        let allowed = steepest * 1.5;

        let mut biggest_jump = 0.0;
        for pair in wave.windows(2) {
            let jump = (pair[1] - pair[0]).abs();
            if jump > biggest_jump {
                biggest_jump = jump;
            }
        }

        assert!(
            biggest_jump <= allowed,
            "jumped by {biggest_jump}, but smooth sound moves at most {allowed}"
        );
    }

    // Presses the lowest `count` notes, from note 0 up, at QUIET. They fill
    // voices without covering up the notes a test listens to.
    fn press_quiet_notes(engine: &mut Engine, count: u8) {
        for note in 0..count {
            engine.note_on(note, QUIET);
        }
    }

    #[test]
    fn a_new_note_takes_over_the_oldest_held_one_without_a_click() {
        // Ten tries, each 1 ms later than the last. The new note arrives at
        // many points along middle C's wave, including near its top and
        // bottom, where a restart would jump the furthest.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();

            // Quiet notes, then middle C, so every voice is busy. Pressing
            // the quiet notes again makes them newer, so middle C becomes the
            // oldest held note, though it was not the first one pressed.
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            engine.note_on(MIDDLE_C, VELOCITY);
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            let mut wave = render_left(&mut engine, 500 + extra_ms);

            // An octave up. It takes over middle C's voice, so afterwards
            // only C5 can be heard.
            engine.note_on(C5, VELOCITY);
            let after = render_left(&mut engine, 1000);
            assert_pitch(&engine, &after, C5_HZ);

            wave.extend(after);
            assert_no_click(&engine, &wave, C5_HZ);
        }
    }

    #[test]
    fn a_new_note_takes_over_a_fading_one_without_a_click() {
        // Ten tries again, as above, while C5 fades out.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();

            // Quiet notes, then C5, so every voice is busy.
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            engine.note_on(C5, VELOCITY);
            let mut wave = render_left(&mut engine, 500);

            engine.note_off(C5);
            let fading = render_left(&mut engine, 100 + extra_ms);
            wave.extend(fading);

            // An octave below middle C. It takes over C5's fading voice, not
            // a held one, though the quiet notes are older. Afterwards only
            // C3 can be heard.
            engine.note_on(C3, VELOCITY);
            let after = render_left(&mut engine, 1000);
            assert_pitch(&engine, &after, C3_HZ);

            wave.extend(after);
            assert_no_click(&engine, &wave, C5_HZ);
        }
    }

    #[test]
    fn a_new_note_takes_over_the_quietest_fading_one() {
        let mut engine = Engine::new().unwrap();

        // Quiet notes, then middle C and C5, so every voice is busy.
        press_quiet_notes(&mut engine, VOICE_COUNT - 2);
        engine.note_on(MIDDLE_C, VELOCITY);
        engine.note_on(C5, VELOCITY);
        render_left(&mut engine, 500);

        // Both fade out, but C5 starts 100 ms sooner, so it is the quieter of
        // the two when a 33rd note arrives. That note is quiet too, so the
        // voice it takes over soon can't be heard.
        engine.note_off(C5);
        render_left(&mut engine, 100);
        engine.note_off(MIDDLE_C);
        engine.note_on(C3, QUIET);

        // After that voice's 5 ms attack, middle C is the only note left to
        // hear, so C5's voice was the one taken.
        render_left(&mut engine, 10);
        let wave = render_left(&mut engine, 100);
        assert_note(&engine, &wave, MIDDLE_C_HZ);
    }

    #[test]
    fn pressing_a_held_note_again_reuses_its_voice() {
        // Ten tries, each 1 ms later than the last, so the second press lands
        // at many points along A4's wave, as in the takeover tests above. The
        // first try lands after exactly 220 waves: a second voice for A4 would
        // start in step with the first, and the two would add up to more than
        // one note's full loudness.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.note_on(A4, VELOCITY);
            let mut wave = render_left(&mut engine, 500 + extra_ms);

            engine.note_on(A4, VELOCITY);
            let after = render_left(&mut engine, 500);
            assert!(peak(&after) <= full_level());

            // The one voice glides back up from where it was, without a click.
            wave.extend(after);
            assert_no_click(&engine, &wave, A4_HZ);
        }
    }

    #[test]
    fn pressing_a_fading_note_again_reuses_its_voice() {
        // Ten tries again. The first one presses A4 again 600 ms in, after
        // exactly 264 waves, so here too a second voice would line up with
        // the first.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.note_on(A4, VELOCITY);
            let mut wave = render_left(&mut engine, 500);

            engine.note_off(A4);
            let fading = render_left(&mut engine, 100 + extra_ms);
            wave.extend(fading);

            engine.note_on(A4, VELOCITY);
            let after = render_left(&mut engine, 500);
            assert!(peak(&after) <= full_level());

            wave.extend(after);
            assert_no_click(&engine, &wave, A4_HZ);
        }
    }

    #[test]
    fn nothing_ever_clips() {
        // Speakers can only play from -1.0 to 1.0. Anything past that clips,
        // which sounds harsh and broken. This is the loudest chord there is:
        // 32 notes at once, all pressed as hard as possible. Added up, their
        // waves go far past 1.0, so only the soft clip keeps them under it.
        let mut engine = Engine::new().unwrap();
        for note in C3..C3 + VOICE_COUNT {
            engine.note_on(note, 127);
        }

        // Both channels, since either one could clip.
        let frames = frames_in_ms(&engine, 1000);
        let samples = engine.render(frames);
        assert!(peak(&samples) < 1.0);
    }

    #[test]
    fn a_loud_chord_is_bent_by_the_soft_clip() {
        // The same loudest chord. Past 0.8, its added-up notes must be bent
        // exactly as the soft clip says: 1.0 into 0.9, 1.4 into 0.95. A hard
        // clamp just under 1.0 would also keep it from clipping, but it cuts
        // the tops of the waves flat, which sounds harsh too.
        let mut notes = Vec::new();
        for note in C3..C3 + VOICE_COUNT {
            notes.push(note);
        }
        assert_chord_adds_up(&notes, 127);
    }

    // Presses `note` at `velocity` on a new engine and returns how loud the
    // first 100 ms gets.
    fn loudness_after_pressing(note: u8, velocity: u8) -> f32 {
        let mut engine = Engine::new().unwrap();
        engine.note_on(note, velocity);
        let wave = render_left(&mut engine, 100);
        peak(&wave)
    }

    #[test]
    fn values_midi_never_sends_make_no_sound() {
        // Notes only go up to 127, and velocities from 1 to 127.
        assert_eq!(loudness_after_pressing(128, VELOCITY), 0.0);
        assert_eq!(loudness_after_pressing(255, VELOCITY), 0.0);
        assert_eq!(loudness_after_pressing(MIDDLE_C, 0), 0.0);
        assert_eq!(loudness_after_pressing(MIDDLE_C, 128), 0.0);
        assert_eq!(loudness_after_pressing(MIDDLE_C, 255), 0.0);

        // The edges of those ranges still play.
        assert!(loudness_after_pressing(0, 1) > 0.0);
        assert!(loudness_after_pressing(127, 127) > 0.0);
    }

    #[test]
    fn the_mailbox_holds_1024_messages() {
        let mut engine = Engine::new().unwrap();
        hold(&mut engine, MIDDLE_C, VELOCITY);

        // 1023 let-gos of E4, which is not playing, so they change nothing.
        // Then letting go of middle C, in the last free slot.
        for _ in 0..MAILBOX_SLOTS - 1 {
            engine.note_off(E4);
        }
        engine.note_off(MIDDLE_C);

        // It got in, so middle C fades out.
        let fade = render_left(&mut engine, RELEASE_MS);
        let after = render_left(&mut engine, 100);
        assert_fades_out(&engine, &fade, &after);
    }

    #[test]
    fn a_full_mailbox_drops_new_messages() {
        let mut engine = Engine::new().unwrap();
        hold(&mut engine, MIDDLE_C, VELOCITY);

        // Let-gos of E4 fill every slot, so letting go of middle C finds no
        // room and is dropped.
        for _ in 0..MAILBOX_SLOTS {
            engine.note_off(E4);
        }
        engine.note_off(MIDDLE_C);

        // Middle C keeps sounding, even after a release would have ended.
        render_left(&mut engine, RELEASE_MS);
        let wave = render_left(&mut engine, 50);
        assert_within_percent(peak(&wave), held_level(), 2.0);

        // Rendering emptied the mailbox, so now letting go gets in.
        engine.note_off(MIDDLE_C);
        let fade = render_left(&mut engine, RELEASE_MS);
        let after = render_left(&mut engine, 100);
        assert_fades_out(&engine, &fade, &after);
    }

    // Renders `ms` milliseconds in chunks of `chunk_ms`, the way the speakers
    // ask for sound a little at a time. Keeps only the left channel.
    fn render_in_chunks(engine: &mut Engine, ms: u32, chunk_ms: u32) -> Vec<f32> {
        let mut wave = Vec::new();
        let chunk_count = ms / chunk_ms;
        for _ in 0..chunk_count {
            let chunk = render_left(engine, chunk_ms);
            wave.extend(chunk);
        }
        wave
    }

    #[test]
    fn the_same_notes_always_make_the_same_sound() {
        // Design rule 6: renders are repeatable. Two engines get the same notes
        // at the same moments. One renders each part in one go, the other in
        // 10 ms chunks like the speakers. Every sample must match.
        let mut whole = Engine::new().unwrap();
        let mut chunked = Engine::new().unwrap();

        whole.note_on(MIDDLE_C, VELOCITY);
        chunked.note_on(MIDDLE_C, VELOCITY);
        let mut whole_wave = render_left(&mut whole, 1000);
        let mut chunked_wave = render_in_chunks(&mut chunked, 1000, 10);

        whole.note_off(MIDDLE_C);
        chunked.note_off(MIDDLE_C);
        let whole_tail = render_left(&mut whole, 400);
        let chunked_tail = render_in_chunks(&mut chunked, 400, 10);
        whole_wave.extend(whole_tail);
        chunked_wave.extend(chunked_tail);

        assert!(
            whole_wave == chunked_wave,
            "the two engines sounded different"
        );
    }

    // Every wave, in the order switching steps through them.
    const EVERY_WAVE: [Kind; 4] = [Kind::Sine, Kind::Triangle, Kind::Square, Kind::Saw];

    #[test]
    fn every_wave_plays_at_the_right_pitch() {
        for wave in EVERY_WAVE {
            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(wave);
            let held = hold(&mut engine, A4, VELOCITY);
            assert_pitch(&engine, &held, A4_HZ);
        }
    }

    // How big the tone at `hz` is inside `wave`: 1.0 for a sine at that pitch
    // that swings from -1 to 1, and 0.0 if that tone isn't there at all. It
    // works like a spectrum analyzer listening to one pitch: it multiplies
    // the samples by a sine at `hz` and adds them up, which only grows when
    // the two match. It does the same with a cosine (the sine a quarter cycle
    // later), so the tone is found wherever its cycles start. The answer is
    // exact when a whole number of the tone's cycles fits in `wave`, as every
    // harmonic of A4 does in one second.
    fn tone_size(engine: &Engine, wave: &[f32], hz: f64) -> f64 {
        let sample_rate = engine.sample_rate() as f64;
        let mut with_sine = 0.0;
        let mut with_cosine = 0.0;
        for i in 0..wave.len() {
            let angle = 2.0 * PI * hz * i as f64 / sample_rate;
            let sample = wave[i] as f64;
            with_sine += sample * angle.sin();
            with_cosine += sample * angle.cos();
        }
        let both = (with_sine * with_sine + with_cosine * with_cosine).sqrt();
        2.0 * both / wave.len() as f64
    }

    // How big harmonics 2 to 8 are next to the main tone (harmonic 1), for
    // each wave. A sine has none. A triangle has only the odd ones, harmonic
    // n at 1/n², and a square only the odd ones at 1/n. A saw has all of
    // them, at 1/n.
    const SINE_HARMONICS: [f64; 7] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    const TRIANGLE_HARMONICS: [f64; 7] = [0.0, 1.0 / 9.0, 0.0, 1.0 / 25.0, 0.0, 1.0 / 49.0, 0.0];
    const SQUARE_HARMONICS: [f64; 7] = [0.0, 1.0 / 3.0, 0.0, 1.0 / 5.0, 0.0, 1.0 / 7.0, 0.0];
    const SAW_HARMONICS: [f64; 7] = [
        1.0 / 2.0,
        1.0 / 3.0,
        1.0 / 4.0,
        1.0 / 5.0,
        1.0 / 6.0,
        1.0 / 7.0,
        1.0 / 8.0,
    ];

    // Checks the harmonics in `wave`, one second of a held A4: harmonics 2 to
    // 8 must have the `expected` sizes next to the main tone, give or take
    // 0.005.
    fn assert_harmonics(engine: &Engine, wave: &[f32], expected: [f64; 7]) {
        let main_tone = tone_size(engine, wave, A4_HZ);
        for i in 0..expected.len() {
            let harmonic = i + 2;
            let hz = A4_HZ * harmonic as f64;
            let size = tone_size(engine, wave, hz) / main_tone;
            let difference = (size - expected[i]).abs();
            assert!(
                difference <= 0.005,
                "harmonic {harmonic} is {size} of the main tone, expected {}",
                expected[i]
            );
        }
    }

    #[test]
    fn a_new_engine_plays_one_triangle_oscillator() {
        // As START_OSCILLATORS says, so a new song's sound profile, sent as
        // the app starts, changes nothing: the first oscillator a triangle
        // at full level, right on the note, and the others silent.
        let first = START_OSCILLATORS[0];
        assert_eq!(first.kind, Kind::Triangle);
        assert_eq!(first.level, FULL_PERCENT);
        assert_eq!((first.octaves, first.half_steps, first.cents), (0, 0, 0));
        for oscillator in &START_OSCILLATORS[1..] {
            assert_eq!(oscillator.level, 0);
        }
        let mut engine = Engine::new().unwrap();
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn a_sine_wave_has_no_harmonics() {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Sine);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SINE_HARMONICS);
    }

    #[test]
    fn a_triangle_wave_has_odd_harmonics_that_fade_fast() {
        // Away from the triangle and back again, so this checks that
        // switching to it works, not only that Jello starts on it.
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Saw);
        engine.set_first_kind(Kind::Triangle);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn a_square_wave_has_odd_harmonics_that_fade_slowly() {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Square);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SQUARE_HARMONICS);
    }

    #[test]
    fn a_saw_wave_has_every_harmonic() {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Saw);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SAW_HARMONICS);
    }

    #[test]
    fn high_notes_keep_stray_tones_quiet() {
        // Jumps and corners are made of harmonics that go on forever. Those
        // too high for 48000 samples a second fold back down as stray,
        // off-key tones, unless the jumps and corners are rounded off. A7,
        // near the top of a piano, is high enough that the triangle, square
        // and saw all fail here without their rounding. Its harmonics are
        // multiples of 3520 Hz, and folding moves them by multiples of
        // 48000 Hz, so the stray tones land on multiples of 320 Hz, the
        // biggest number that divides both. Below 5000 Hz, where they are
        // easiest to hear, each must be at least 1000 times quieter than the
        // main tone.
        for wave in EVERY_WAVE {
            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(wave);
            let held = hold(&mut engine, A7, VELOCITY);
            let main_tone = tone_size(&engine, &held, A7_HZ);

            let mut hz = 320.0;
            while hz < 5000.0 {
                let is_harmonic = hz % A7_HZ == 0.0;
                if !is_harmonic {
                    let size = tone_size(&engine, &held, hz) / main_tone;
                    assert!(
                        size <= 0.001,
                        "{}: the stray tone at {hz} Hz is {size} of the main tone",
                        wave.name()
                    );
                }
                hz += 320.0;
            }
        }
    }

    #[test]
    fn every_wave_starts_its_cycle_rising_from_zero() {
        // That is what lines up the main tones of all four. A note on a new
        // engine starts at the beginning of a cycle, and A4's cycles are
        // about 109 samples long. So the first half of the first cycle,
        // samples 1 to 54, must be above zero, and the second half, samples
        // 55 to 108, below. Sample 0 is where the cycle starts, so it is
        // skipped.
        for wave in EVERY_WAVE {
            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(wave);
            engine.note_on(A4, VELOCITY);
            let start = render_left(&mut engine, 3);

            for i in 1..=54 {
                assert!(
                    start[i] > 0.0,
                    "{}: sample {i} is not above zero",
                    wave.name()
                );
            }
            for i in 55..=108 {
                assert!(
                    start[i] < 0.0,
                    "{}: sample {i} is not below zero",
                    wave.name()
                );
            }
        }
    }

    // How loud `wave` is overall (its RMS): square every sample, average
    // them, then take the square root. Unlike the peak, it counts the whole
    // wave, so it follows how loud a wave sounds much better.
    fn rms(wave: &[f32]) -> f64 {
        let mut total = 0.0;
        for i in 0..wave.len() {
            let sample = wave[i] as f64;
            total += sample * sample;
        }
        let average = total / wave.len() as f64;
        average.sqrt()
    }

    #[test]
    fn every_wave_sounds_equally_loud() {
        // The trims bring every wave to the triangle's loudness. A held
        // triangle peaks at held_level(), and its RMS is that divided by the
        // square root of 3. Square and saw sounded louder by ear, so they sit
        // a further 2 dB lower (times 0.7943). At A4, the square and saw come
        // out about 1% under, because their rounded-off jumps take a little
        // off, so 2% either way is allowed.
        let triangle_loudness = held_level() as f64 / 3.0_f64.sqrt();
        for wave in EVERY_WAVE {
            let mut expected = triangle_loudness;
            if matches!(wave, Kind::Square | Kind::Saw) {
                expected = triangle_loudness * 0.7943;
            }

            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(wave);
            let held = hold(&mut engine, A4, VELOCITY);
            let loudness = rms(&held);
            let difference = (loudness - expected).abs();
            assert!(
                difference <= expected * 0.02,
                "{}: the RMS is {loudness}, expected {expected} give or take 2%",
                wave.name()
            );
        }
    }

    #[test]
    fn a_switch_leaves_ringing_notes_alone() {
        // A4 starts on the triangle, then the wave switches to the saw. Only
        // notes sent after a switch play the new wave, so A4 stays a triangle.
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, ATTACK_MS + DECAY_MS);

        engine.set_first_kind(Kind::Saw);
        let wave = render_left(&mut engine, 1000);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn a_note_pressed_again_after_a_switch_plays_the_new_wave() {
        // A4 starts on the triangle and is still ringing when the wave
        // switches to the saw. Pressing A4 again takes over its own voice, a
        // busy one, and that new press plays the saw.
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, 100);

        engine.set_first_kind(Kind::Saw);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SAW_HARMONICS);
    }

    #[test]
    fn a_switch_waits_its_turn_in_the_mailbox() {
        // A4 is sent just before the switch to the saw, so the audio thread
        // finds both waiting at the start of the same chunk. It acts on them
        // in the order they were sent, so A4 starts on the triangle.
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        engine.set_first_kind(Kind::Saw);
        render_left(&mut engine, ATTACK_MS + DECAY_MS);

        let wave = render_left(&mut engine, 1000);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    // One call to `jello_engine_set_oscillator`: its index, kind, level,
    // octaves, half steps and cents.
    type RawOscillator = (c_int, c_int, f32, c_int, c_int, f32);

    // A4 held on a new engine playing a sine, after `call` reached the C API.
    fn sine_after(call: RawOscillator) -> Vec<f32> {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Sine);
        let (index, kind, level, octaves, half_steps, cents) = call;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_oscillator(engine.raw, index, kind, level, octaves, half_steps, cents) };
        hold(&mut engine, A4, VELOCITY)
    }

    #[test]
    fn values_outside_the_oscillators_ranges_change_nothing() {
        // The C API takes any numbers, so it must ignore a call with any one
        // of them out of range. Rust brings its own within them, so this
        // calls the C API directly. Each call would otherwise play a saw an
        // octave up on top of the sine, so one that got through would show.
        let plain = sine_after((1, 0, 0.0, 0, 0, 0.0));
        let saw = Kind::Saw.number();
        let odd_calls: [(&str, RawOscillator); 14] = [
            ("index -1", (-1, saw, 1.0, 1, 0, 0.0)),
            ("index 3", (3, saw, 1.0, 1, 0, 0.0)),
            ("kind -1", (1, -1, 1.0, 1, 0, 0.0)),
            ("kind 5", (1, 5, 1.0, 1, 0, 0.0)),
            ("level -0.1", (1, saw, -0.1, 1, 0, 0.0)),
            ("level 1.1", (1, saw, 1.1, 1, 0, 0.0)),
            ("level NaN", (1, saw, f32::NAN, 1, 0, 0.0)),
            ("octaves 4", (1, saw, 1.0, 4, 0, 0.0)),
            ("octaves -4", (1, saw, 1.0, -4, 0, 0.0)),
            ("half steps 13", (1, saw, 1.0, 1, 13, 0.0)),
            ("half steps -13", (1, saw, 1.0, 1, -13, 0.0)),
            ("cents 100.5", (1, saw, 1.0, 1, 0, 100.5)),
            ("cents -101", (1, saw, 1.0, 1, 0, -101.0)),
            ("cents NaN", (1, saw, 1.0, 1, 0, f32::NAN)),
        ];
        for (what, call) in odd_calls {
            assert!(sine_after(call) == plain, "an oscillator with {what} changed the sound");
        }
    }

    #[test]
    fn the_engine_takes_every_oscillator_edge_rust_sends() {
        // A saw at each edge of a range, on the second oscillator, changes
        // the sine. The test above shows a hair further is ignored, so Rust
        // and the engine agree on every range.
        let plain = sine_after((1, 0, 0.0, 0, 0, 0.0));
        let saw_on_the_note = Oscillator {
            kind: Kind::Saw,
            level: FULL_PERCENT,
            octaves: 0,
            half_steps: 0,
            cents: 0,
        };
        let edges = [
            Oscillator { kind: Kind::Noise, ..saw_on_the_note },
            Oscillator { octaves: OSCILLATOR_MOST_OCTAVES, ..saw_on_the_note },
            Oscillator { octaves: -OSCILLATOR_MOST_OCTAVES, ..saw_on_the_note },
            Oscillator { half_steps: OSCILLATOR_MOST_HALF_STEPS, ..saw_on_the_note },
            Oscillator { half_steps: -OSCILLATOR_MOST_HALF_STEPS, ..saw_on_the_note },
            Oscillator { cents: OSCILLATOR_MOST_CENTS, ..saw_on_the_note },
            Oscillator { cents: -OSCILLATOR_MOST_CENTS, ..saw_on_the_note },
        ];
        for edge in edges {
            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(Kind::Sine);
            engine.set_oscillator(1, edge);
            let moved = hold(&mut engine, A4, VELOCITY);
            assert!(moved != plain, "the engine refused {edge:?}");
        }
    }

    // A new engine with its first oscillator silent and its second playing
    // `oscillator`.
    fn second_oscillator_engine(oscillator: Oscillator) -> Engine {
        let mut engine = Engine::new().unwrap();
        let silent = Oscillator {
            level: 0,
            ..START_OSCILLATORS[0]
        };
        engine.set_oscillator(0, silent);
        engine.set_oscillator(1, oscillator);
        engine
    }

    #[test]
    fn an_oscillator_plays_its_own_distance_from_the_note() {
        // Only the second oscillator sounds, a sine tuned away from A4. Each
        // octave doubles the pitch, each half step is 2 to the power of
        // 1/12, and 100 cents make a half step.
        let half_step = 2.0_f64.powf(1.0 / 12.0);
        let cases = [
            ((1, 0, 0), 2.0 * A4_HZ),
            ((-1, 0, 0), A4_HZ / 2.0),
            ((0, 7, 0), A4_HZ * half_step.powi(7)),
            ((0, 0, 50), A4_HZ * half_step.sqrt()),
            // An octave up, twelve half steps down, then a half step up.
            ((1, -12, 100), A4_HZ * half_step),
        ];
        for ((octaves, half_steps, cents), hz) in cases {
            let tuned = Oscillator {
                kind: Kind::Sine,
                level: FULL_PERCENT,
                octaves,
                half_steps,
                cents,
            };
            let mut engine = second_oscillator_engine(tuned);
            let held = hold(&mut engine, A4, VELOCITY);
            assert_pitch(&engine, &held, hz);
        }
    }

    #[test]
    fn oscillators_add_up_at_their_levels() {
        // Two halves of the same triangle make exactly one whole: a copy's
        // oscillators start their cycles together, so like waves at the same
        // pitch line up.
        let half = Oscillator {
            level: 50,
            ..START_OSCILLATORS[0]
        };
        let mut halves = Engine::new().unwrap();
        halves.set_oscillator(0, half);
        halves.set_oscillator(1, half);
        let mut whole = Engine::new().unwrap();
        let halves_sound = hold(&mut halves, A4, VELOCITY);
        let whole_sound = hold(&mut whole, A4, VELOCITY);
        assert_same_samples(&halves_sound, &whole_sound);

        // A sine with a sine an octave up at half its level: the octave's
        // tone is half the size of the note's.
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Sine);
        let octave_up = Oscillator {
            kind: Kind::Sine,
            level: 50,
            octaves: 1,
            half_steps: 0,
            cents: 0,
        };
        engine.set_oscillator(1, octave_up);
        let held = hold(&mut engine, A4, VELOCITY);
        let note = tone_size(&engine, &held, A4_HZ);
        let octave = tone_size(&engine, &held, 2.0 * A4_HZ);
        assert_within_percent((octave / note) as f32, 0.5, 0.5);
    }

    #[test]
    fn a_silent_oscillator_changes_not_a_single_sample() {
        // At level 0, however the rest of it is set, an oscillator is
        // skipped, so a new engine sounds exactly as one wave did.
        let mut plain = Engine::new().unwrap();
        let mut silent = Engine::new().unwrap();
        let saw_up = Oscillator {
            kind: Kind::Saw,
            level: 0,
            octaves: 2,
            half_steps: 5,
            cents: 30,
        };
        let noise = Oscillator {
            kind: Kind::Noise,
            ..saw_up
        };
        silent.set_oscillator(1, saw_up);
        silent.set_oscillator(2, noise);
        let silent_sound = hold(&mut silent, A4, VELOCITY);
        let plain_sound = hold(&mut plain, A4, VELOCITY);
        assert_same_samples(&silent_sound, &plain_sound);
    }

    // The first oscillator playing noise at full level.
    const NOISE: Oscillator = Oscillator {
        kind: Kind::Noise,
        level: FULL_PERCENT,
        octaves: 0,
        half_steps: 0,
        cents: 0,
    };

    // A new engine playing `oscillators`, one after another from the first.
    fn engine_playing(oscillators: &[Oscillator]) -> Engine {
        let mut engine = Engine::new().unwrap();
        for (index, oscillator) in oscillators.iter().enumerate() {
            engine.set_oscillator(index, *oscillator);
        }
        engine
    }

    #[test]
    fn noise_has_no_pitch_and_hisses_the_same_on_every_render() {
        // The first note on a new engine always starts its noise the same
        // way, on any note and however the noise is tuned, so A4 and a low,
        // retuned C2 hiss alike. Two renders of A4 do too.
        let mut a4 = engine_playing(&[NOISE]);
        let a4_sound = hold(&mut a4, A4, VELOCITY);
        assert!(rms(&a4_sound) > 0.0, "the noise is silent");
        let retuned = Oscillator {
            octaves: -3,
            half_steps: 5,
            cents: 40,
            ..NOISE
        };
        let mut c2 = engine_playing(&[retuned]);
        let c2_sound = hold(&mut c2, 36, VELOCITY);
        assert_same_samples(&c2_sound, &a4_sound);

        let mut again = engine_playing(&[NOISE]);
        let again_sound = hold(&mut again, A4, VELOCITY);
        assert_same_samples(&again_sound, &a4_sound);

        // It has no main tone: at A4 and its first harmonics, and at a
        // pitch nowhere near them, no tone is a twentieth the size of a
        // triangle's A4.
        let mut triangle = Engine::new().unwrap();
        let triangle_sound = hold(&mut triangle, A4, VELOCITY);
        let triangle_tone = tone_size(&triangle, &triangle_sound, A4_HZ);
        for hz in [A4_HZ, 2.0 * A4_HZ, 3.0 * A4_HZ, 1000.0] {
            let size = tone_size(&a4, &a4_sound, hz) / triangle_tone;
            assert!(size < 0.05, "the noise has a tone at {hz} Hz, {size} of a triangle's");
        }
    }

    #[test]
    fn noise_is_as_loud_as_the_triangle() {
        // Its heights spread evenly from -1 to 1, so its RMS is the
        // triangle's: held_level() divided by the square root of 3. A second
        // of it lands within 2% of that.
        let triangle_loudness = held_level() as f64 / 3.0_f64.sqrt();
        let mut engine = engine_playing(&[NOISE]);
        let held = hold(&mut engine, A4, VELOCITY);
        let loudness = rms(&held);
        assert_within_percent(loudness as f32, triangle_loudness as f32, 2.0);
    }

    #[test]
    fn no_two_noises_hiss_alike() {
        // Separate hisses add up by the square root of how many there are,
        // and the same hiss twice by how many. So two noise oscillators are
        // about 1.41 times as loud as one, not twice. And seven copies of a
        // noise in the middle, turned down by the square root of 7, stay as
        // loud as one copy, not 2.65 times.
        let mut one = engine_playing(&[NOISE]);
        let one_loudness = rms(&hold(&mut one, A4, VELOCITY)) as f32;

        let mut two = engine_playing(&[NOISE, NOISE]);
        let two_loudness = rms(&hold(&mut two, A4, VELOCITY)) as f32;
        assert_within_percent(two_loudness, one_loudness * 2.0_f32.sqrt(), 5.0);

        let mut copies = engine_playing(&[NOISE]);
        copies.set_unison(Unison {
            copies: UNISON_MOST_COPIES,
            detune_cents: 0,
            spread: 0,
        });
        let copies_loudness = rms(&hold(&mut copies, A4, VELOCITY)) as f32;
        assert_within_percent(copies_loudness, one_loudness, 5.0);
    }

    // Plays A4 on `wave` for `held_ms`, then lets go of it and renders
    // `fade_ms` of its fade. A fade shorter than RELEASE_MS leaves its voice
    // still sounding.
    fn play_and_let_go_of_a4(wave: Kind, held_ms: u32, fade_ms: u32) -> Engine {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(wave);
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, held_ms);
        engine.note_off(A4);
        render_left(&mut engine, fade_ms);
        engine
    }

    // Switches to `wave` and presses A4 again. If A4 is still sounding, the
    // press takes over its voice.
    fn press_a4_again_on(engine: &mut Engine, wave: Kind) {
        engine.set_first_kind(wave);
        engine.note_on(A4, VELOCITY);
    }

    #[test]
    fn a_note_pressed_again_on_a_new_wave_glides_across_without_a_click() {
        // A4 fades on one wave, the wave switches to the next one, and A4 is
        // pressed again, which takes over its own voice, still sounding the
        // old wave. Ten tries for each switch, each 1 ms later than the last,
        // so the press lands at many points along the wave.
        for old in EVERY_WAVE {
            let new = old.next();
            for extra_ms in 0..10 {
                let fade_ms = 100 + extra_ms;

                let mut engine = play_and_let_go_of_a4(old, 100, fade_ms);
                press_a4_again_on(&mut engine, new);
                let glide = render_left(&mut engine, 10);

                // Two sounds to hold it up against: A4 fading on as if it was
                // never pressed again, and a plain press, where A4 was on the
                // new wave all along, so there was no gap to glide across.
                let mut engine = play_and_let_go_of_a4(old, 100, fade_ms);
                let fade = render_left(&mut engine, 10);
                let mut engine = play_and_let_go_of_a4(new, 100, fade_ms);
                press_a4_again_on(&mut engine, new);
                let plain = render_left(&mut engine, 10);

                // It carries on exactly where the old wave was heading. The
                // envelope starts gliding to the new press's peak at the same
                // moment, which nudges the first sample by at most 1/240 of
                // full loudness, so 1/100 is allowed.
                let jump = (glide[0] - fade[0]).abs();
                assert!(
                    jump <= full_level() / 100.0,
                    "{} to {}: the press jumped by {jump}",
                    old.name(),
                    new.name()
                );

                // The gap is the difference from the plain press. It never
                // jumps, and after 5 ms it is gone, leaving the same sound.
                let mut gap = Vec::new();
                for i in 0..glide.len() {
                    gap.push(glide[i] - plain[i]);
                }
                assert_no_click(&engine, &gap, A4_HZ);
                let gone = frames_in_ms(&engine, 5);
                assert_same_sound(&glide[gone..], &plain[gone..]);
            }
        }
    }

    // Three oscillators, one way: a saw, a square an octave up at half level
    // and a triangle an octave down at 70%. The other way: a sine, noise at
    // 40% and a saw an octave down. Each oscillator is tuned the same both
    // ways, so its cycles run alike before the press, and a plain press
    // differs from a glide only by the gap.
    const THREE_ONE_WAY: [Oscillator; OSCILLATOR_COUNT] = [
        Oscillator {
            kind: Kind::Saw,
            level: FULL_PERCENT,
            octaves: 0,
            half_steps: 0,
            cents: 0,
        },
        Oscillator {
            kind: Kind::Square,
            level: 50,
            octaves: 1,
            half_steps: 0,
            cents: 0,
        },
        Oscillator {
            kind: Kind::Triangle,
            level: 70,
            octaves: -1,
            half_steps: 0,
            cents: 0,
        },
    ];
    const THREE_THE_OTHER_WAY: [Oscillator; OSCILLATOR_COUNT] = [
        Oscillator {
            kind: Kind::Sine,
            level: FULL_PERCENT,
            octaves: 0,
            half_steps: 0,
            cents: 0,
        },
        Oscillator {
            kind: Kind::Noise,
            level: 40,
            octaves: 1,
            half_steps: 0,
            cents: 0,
        },
        Oscillator {
            kind: Kind::Saw,
            level: FULL_PERCENT,
            octaves: -1,
            half_steps: 0,
            cents: 0,
        },
    ];

    // Plays A4 on `oscillators` for 100 ms, then lets go of it and renders
    // `fade_ms` of its fade.
    fn let_go_of_a4_on(oscillators: &[Oscillator], fade_ms: u32) -> Engine {
        let mut engine = engine_playing(oscillators);
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, 100);
        engine.note_off(A4);
        render_left(&mut engine, fade_ms);
        engine
    }

    // Sets `oscillators` and presses A4 again, taking over its voice.
    fn press_a4_again_with(engine: &mut Engine, oscillators: &[Oscillator]) {
        for (index, oscillator) in oscillators.iter().enumerate() {
            engine.set_oscillator(index, *oscillator);
        }
        engine.note_on(A4, VELOCITY);
    }

    #[test]
    fn three_oscillators_glide_across_to_three_others_without_a_click() {
        // As with one wave above, both ways round, the press landing at many
        // points along the waves. Each engine's second press starts the same
        // noise, so the gap is all that differs from a plain press.
        let ways = [(THREE_ONE_WAY, THREE_THE_OTHER_WAY), (THREE_THE_OTHER_WAY, THREE_ONE_WAY)];
        for (old, new) in ways {
            for extra_ms in 0..10 {
                let fade_ms = 100 + extra_ms;

                let mut engine = let_go_of_a4_on(&old, fade_ms);
                press_a4_again_with(&mut engine, &new);
                let glide = render_left(&mut engine, 10);

                let mut engine = let_go_of_a4_on(&old, fade_ms);
                let fade = render_left(&mut engine, 10);
                let mut engine = let_go_of_a4_on(&new, fade_ms);
                press_a4_again_with(&mut engine, &new);
                let plain = render_left(&mut engine, 10);

                let jump = (glide[0] - fade[0]).abs();
                assert!(jump <= full_level() / 100.0, "the press jumped by {jump}");

                let mut gap = Vec::new();
                for i in 0..glide.len() {
                    gap.push(glide[i] - plain[i]);
                }
                assert_no_click(&engine, &gap, A4_HZ);
                let gone = frames_in_ms(&engine, 5);
                assert_same_sound(&glide[gone..], &plain[gone..]);
            }
        }
    }

    #[test]
    fn a_second_press_mid_glide_picks_up_where_the_sound_was() {
        // As above, but 2 ms into the glide the wave switches again, to the
        // one after, and A4 is pressed a third time. By then the sound is
        // part new wave, part what is left of the first gap, and the new gap
        // must count both.
        for first in EVERY_WAVE {
            let second = first.next();
            let third = second.next();
            for extra_ms in 0..10 {
                let fade_ms = 100 + extra_ms;

                let mut engine = play_and_let_go_of_a4(first, 100, fade_ms);
                press_a4_again_on(&mut engine, second);
                render_left(&mut engine, 2);
                press_a4_again_on(&mut engine, third);
                let pressed_twice = render_left(&mut engine, 10);

                // The same, as if A4 was not pressed the third time.
                let mut engine = play_and_let_go_of_a4(first, 100, fade_ms);
                press_a4_again_on(&mut engine, second);
                render_left(&mut engine, 2);
                let pressed_once = render_left(&mut engine, 10);

                // The envelope nudges the first sample a little, as above.
                let jump = (pressed_twice[0] - pressed_once[0]).abs();
                assert!(
                    jump <= full_level() / 100.0,
                    "{} to {} to {}: the third press jumped by {jump}",
                    first.name(),
                    second.name(),
                    third.name()
                );
            }
        }
    }

    #[test]
    fn a_note_on_a_silent_voice_starts_without_a_gap() {
        // A4 fades out completely, the wave switches to the next one, and A4
        // is pressed again. It gets its old voice back, as the first free
        // one, but that voice was silent, so there is no sound to carry on
        // from. It must sound exactly like A4 played on the new wave both
        // times. Ten tries, each holding A4 1 ms longer, so the voice falls
        // silent at many points along the wave.
        for old in EVERY_WAVE {
            let new = old.next();
            for extra_ms in 0..10 {
                let held_ms = 100 + extra_ms;

                let mut engine = play_and_let_go_of_a4(old, held_ms, RELEASE_MS);
                press_a4_again_on(&mut engine, new);
                let after_silence = render_left(&mut engine, 10);

                let mut engine = play_and_let_go_of_a4(new, held_ms, RELEASE_MS);
                press_a4_again_on(&mut engine, new);
                let new_both_times = render_left(&mut engine, 10);

                assert_same_sound(&after_silence, &new_both_times);
            }
        }
    }

    // The click (docs/adr/0021-a-looper-on-a-beat.md). Its sound comes from
    // engine/src/click.cpp. When it is changed there on purpose, change it
    // here too.
    const HIGH_TICK_HZ: f64 = 1760.0; // `high_hz`
    const LOW_TICK_HZ: f64 = 880.0; // `low_hz`
    const TICK_MS: u32 = 40; // `tick_ms`
    const HALF_LIFE_MS: u32 = 4; // `half_life_ms`

    // How many samples a beat lasts at 120 beats per minute: half a second.
    const BEAT_AT_120: usize = 24_000;

    // How many samples the engine makes in a minute.
    const SAMPLES_PER_MINUTE: usize = 48_000 * 60;

    // Ticks are always far more than this many samples apart, so a run of
    // silence this long means the last tick is over.
    const SILENCE_BETWEEN_TICKS: usize = 1000;

    // The sample each tick in `wave` starts on. A tick's sine wave starts
    // from zero, so its first sample is silent and the one after it is the
    // first that sounds.
    fn tick_starts(wave: &[f32]) -> Vec<usize> {
        let mut starts = Vec::new();
        // How many silent samples came just before this one. The time before
        // the first sample counts as a long silence.
        let mut silent = SILENCE_BETWEEN_TICKS;
        for (i, sample) in wave.iter().enumerate() {
            if *sample != 0.0 {
                if silent >= SILENCE_BETWEEN_TICKS && i > 0 {
                    starts.push(i - 1);
                }
                silent = 0;
            } else {
                silent += 1;
            }
        }
        starts
    }

    // The samples one tick lasts, from `start` on.
    fn tick_at<'a>(engine: &Engine, wave: &'a [f32], start: usize) -> &'a [f32] {
        let length = frames_in_ms(engine, TICK_MS);
        &wave[start..start + length]
    }

    #[test]
    fn the_click_ticks_on_every_beat() {
        // At 120 beats per minute a beat is 24,000 samples, so the ticks start
        // on exactly these samples.
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        let wave = render_left(&mut engine, 2100);
        assert_eq!(tick_starts(&wave), vec![0, 24_000, 48_000, 72_000, 96_000]);
    }

    #[test]
    fn the_first_beat_of_each_bar_ticks_higher() {
        // Two bars of 4 beats, then the first beat of a third.
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        let wave = render_left(&mut engine, 4100);
        let starts = tick_starts(&wave);
        assert_eq!(starts.len(), 9);

        for (beat, start) in starts.iter().enumerate() {
            let tick = tick_at(&engine, &wave, *start);
            if beat % 4 == 0 {
                assert_note(&engine, tick, HIGH_TICK_HZ);
            } else {
                assert_note(&engine, tick, LOW_TICK_HZ);
            }
        }
    }

    #[test]
    fn a_tick_halves_every_4_ms_then_stops_after_40() {
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        let wave = render_left(&mut engine, 400);

        // Each 4 ms of the tick is about half as loud as the 4 ms before.
        let step = frames_in_ms(&engine, HALF_LIFE_MS);
        let tick = tick_at(&engine, &wave, 0);
        for part in 1..tick.len() / step {
            let before = peak(&tick[(part - 1) * step..part * step]);
            let now = peak(&tick[part * step..(part + 1) * step]);
            assert_within_percent(now, before / 2.0, 5.0);
        }

        // After 40 ms, exact silence until the next beat.
        let after = &wave[tick.len()..];
        assert_eq!(peak(after), 0.0);
    }

    #[test]
    fn the_beat_never_drifts() {
        // At 115 beats per minute a beat is 25,043.478... samples, so beats
        // can't all be the same whole number of samples long. Each one starts
        // on the sample nearest below where it belongs, worked out from the
        // start, so the rounding never adds up: 1,000 beats in, the next tick
        // starts on sample 1,000 x 2,880,000 / 115 = 25,043,478.26..., so
        // 25,043,478.
        let mut engine = Engine::new().unwrap();
        engine.set_tempo(115);
        engine.start_click();

        // Skip ahead to just before that tick, in 10-second chunks.
        let expected = 1000 * SAMPLES_PER_MINUTE / 115;
        let window_start = expected - 500;
        let mut rendered = 0;
        while rendered < window_start {
            let frames = usize::min(480_000, window_start - rendered);
            engine.render(frames);
            rendered += frames;
        }

        let window = render_left(&mut engine, 20);
        assert_eq!(tick_starts(&window), vec![500]);
    }

    #[test]
    fn ticks_land_on_the_same_samples_however_the_sound_is_asked_for() {
        // The speakers ask for sound a chunk at a time, and beats fall
        // anywhere inside a chunk. 7 ms chunks never line up with the beat,
        // yet every sample must match a render made in one go.
        let mut whole = Engine::new().unwrap();
        let mut chunked = Engine::new().unwrap();
        whole.start_click();
        chunked.start_click();

        let whole_wave = render_left(&mut whole, 2100);
        let chunked_wave = render_in_chunks(&mut chunked, 2100, 7);
        assert!(whole_wave == chunked_wave, "the two engines ticked differently");
    }

    #[test]
    fn a_new_tempo_starts_on_the_next_beat() {
        // Halfway through beat 1 at 120, the tempo drops to 60. Beat 1 keeps
        // its length, so beat 2 starts on time, and from there on each beat
        // lasts twice as long.
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        let mut wave = render_left(&mut engine, 250);
        engine.set_tempo(60);
        let rest = render_left(&mut engine, 2350);
        wave.extend(rest);

        let starts = tick_starts(&wave);
        assert_eq!(starts, vec![0, 24_000, 72_000, 120_000]);
    }

    #[test]
    fn the_slowest_and_fastest_tempos_play_and_others_are_ignored() {
        // A beat at 40 lasts 72,000 samples, and at 240 just 12,000.
        let mut slowest = Engine::new().unwrap();
        slowest.set_tempo(SLOWEST_TEMPO);
        slowest.start_click();
        let wave = render_left(&mut slowest, 3100);
        assert_eq!(tick_starts(&wave), vec![0, 72_000, 144_000]);

        let mut fastest = Engine::new().unwrap();
        fastest.set_tempo(FASTEST_TEMPO);
        fastest.start_click();
        let wave = render_left(&mut fastest, 600);
        assert_eq!(tick_starts(&wave), vec![0, 12_000, 24_000]);

        // Tempos past either end change nothing: the beat stays at 120.
        let mut engine = Engine::new().unwrap();
        engine.set_tempo(SLOWEST_TEMPO - 1);
        engine.set_tempo(FASTEST_TEMPO + 1);
        engine.set_tempo(0);
        engine.set_tempo(u32::MAX);
        engine.start_click();
        let wave = render_left(&mut engine, 1100);
        assert_eq!(tick_starts(&wave), vec![0, BEAT_AT_120, 2 * BEAT_AT_120]);
        assert_eq!(engine.tempo(), START_TEMPO);
    }

    #[test]
    fn stopping_the_click_lets_its_tick_ring_out() {
        // Stopped 10 ms into a tick, the tick still plays all of its 40 ms,
        // exactly like one left alone. Then no more ticks come.
        let mut stopped = Engine::new().unwrap();
        stopped.start_click();
        let mut wave = render_left(&mut stopped, 10);
        stopped.stop_click();
        let rest = render_left(&mut stopped, 1990);
        wave.extend(rest);

        let mut left_alone = Engine::new().unwrap();
        left_alone.start_click();
        let alone = render_left(&mut left_alone, 100);

        let tick = tick_at(&stopped, &wave, 0);
        let alone_tick = tick_at(&left_alone, &alone, 0);
        assert!(tick == alone_tick, "the tick was cut short");
        assert_eq!(peak(&wave[tick.len()..]), 0.0);
    }

    #[test]
    fn starting_the_click_again_begins_a_new_bar() {
        // Stopped after beat 2, then started again: the next tick is right
        // away, and it is the high one, as beat 1 of a new bar.
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        render_left(&mut engine, 600);
        engine.stop_click();
        render_left(&mut engine, 100);
        engine.start_click();
        let wave = render_left(&mut engine, 100);

        assert_eq!(tick_starts(&wave), vec![0]);
        assert_note(&engine, tick_at(&engine, &wave, 0), HIGH_TICK_HZ);
    }

    #[test]
    fn starting_the_click_while_it_is_on_changes_nothing() {
        // A second start 300 ms in doesn't jump back to beat 1, so beat 2
        // still comes at 24,000.
        let mut engine = Engine::new().unwrap();
        engine.start_click();
        let mut wave = render_left(&mut engine, 300);
        engine.start_click();
        let rest = render_left(&mut engine, 800);
        wave.extend(rest);

        assert_eq!(tick_starts(&wave), vec![0, BEAT_AT_120, 2 * BEAT_AT_120]);
        assert_note(&engine, tick_at(&engine, &wave, BEAT_AT_120), LOW_TICK_HZ);
    }

    #[test]
    fn the_click_takes_no_voice() {
        // All 32 voices are held. If a tick took one of them, a note would
        // stop. So once the tick is over, the notes sound exactly as they do
        // with no click at all.
        let mut clicking = Engine::new().unwrap();
        let mut quiet = Engine::new().unwrap();
        for note in 0..VOICE_COUNT {
            clicking.note_on(C3 + note, VELOCITY);
            quiet.note_on(C3 + note, VELOCITY);
        }
        clicking.start_click();

        render_left(&mut clicking, TICK_MS);
        render_left(&mut quiet, TICK_MS);
        let with_click = render_left(&mut clicking, 400);
        let without = render_left(&mut quiet, 400);
        assert!(with_click == without, "a note stopped when the click ticked");
    }

    #[test]
    fn the_click_and_notes_never_go_above_full_loudness() {
        // 32 notes as loud as they go, with a high tick on top, still stay
        // below 1.0.
        let mut engine = Engine::new().unwrap();
        for note in 0..VOICE_COUNT {
            engine.note_on(C3 + note, 127);
        }
        engine.start_click();
        let wave = render_left(&mut engine, 100);
        assert!(peak(&wave) < 1.0);
    }

    #[test]
    fn a_new_engine_beats_at_120_with_the_looper_idle() {
        let mut engine = Engine::new().unwrap();
        assert_eq!(engine.tempo(), START_TEMPO);
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, false));
        assert!(!engine.is_looper_busy());

        // Nothing ticks until the click or the looper starts.
        let wave = render_left(&mut engine, 1000);
        assert_eq!(peak(&wave), 0.0);
    }

    #[test]
    fn setting_the_tempo_answers_the_tempo_the_beat_plays() {
        let mut engine = Engine::new().unwrap();
        assert_eq!(engine.set_tempo(125), 125);

        // The engine plays it too: a beat at 125 is 23,040 samples.
        engine.start_click();
        let wave = render_left(&mut engine, 500);
        assert_eq!(tick_starts(&wave), vec![0, 23_040]);

        // A tempo it ignores answers the one it still plays.
        assert_eq!(engine.set_tempo(SLOWEST_TEMPO - 1), 125);
        assert_eq!(engine.set_tempo(FASTEST_TEMPO + 1), 125);
    }

    // The looper (docs/adr/0021-a-looper-on-a-beat.md). Most of these tests
    // play the same music twice: once through the looper, and once by hand,
    // with every loop note played live on the exact sample the loop should
    // play it, and the click stopped where the loop takes over. Every sample
    // must match. That shows the loop plays each note on exactly the right
    // sample, as loud and on the same wave as live, and lets go of it
    // exactly when it should. Many play notes between half beats, so they
    // turn "snap to beat" off, and the loop plays each note as played.

    // How many samples a bar of 4 beats lasts at 120 beats per minute: 2
    // seconds.
    const BAR_AT_120: usize = 4 * BEAT_AT_120;

    // At 120 the count-in takes the first bar, so recording starts on the
    // second.
    const RECORDING_STARTS: usize = BAR_AT_120;

    // An engine with "snap to beat" off, so its loops play every note
    // exactly as played.
    fn engine_without_snap() -> Engine {
        let mut engine = Engine::new().unwrap();
        engine.set_snap(false);
        engine
    }

    fn looper(state: LooperState, number: u32, waiting: bool) -> LooperStatus {
        LooperStatus {
            state,
            number,
            waiting,
        }
    }

    // One thing a test does, on the sample it is at.
    #[derive(Clone, Copy)]
    enum Step {
        NoteOn(u8),
        NoteOff(u8),
        AllNotesOff,
        PressLooper,
        ClearLoop,
        // Esc's two ways to remove a layer: the newest, or by its number.
        RemoveNewest,
        RemoveLayer(u32),
        // A layer's number and its new volume, in percent.
        Volume(u32, u32),
        StartClick,
        StopClick,
        SetFirstKind(Kind),
        PlaySong(&'static [TestChord]),
        StopSong,
        Snap(bool),
        HoldVibratoKey,
        LetGoOfVibratoKey,
    }

    use Step::*;

    fn take_step(engine: &mut Engine, step: Step) {
        match step {
            NoteOn(note) => engine.note_on(note, VELOCITY),
            NoteOff(note) => engine.note_off(note),
            AllNotesOff => engine.all_notes_off(),
            PressLooper => engine.press_looper(),
            ClearLoop => engine.clear_loop(),
            RemoveNewest => engine.remove_newest_layer(),
            RemoveLayer(number) => engine.remove_layer(number),
            Volume(number, percent) => engine.set_layer_volume(number, percent).unwrap(),
            StartClick => engine.start_click(),
            StopClick => engine.stop_click(),
            SetFirstKind(kind) => engine.set_first_kind(kind),
            PlaySong(song) => engine.play_song(&song_chords(song)).unwrap(),
            StopSong => engine.stop_song(),
            Snap(on) => engine.set_snap(on),
            HoldVibratoKey => engine.set_vibrato_key(true),
            LetGoOfVibratoKey => engine.set_vibrato_key(false),
        }
    }

    // `steps` with each one moved `by` samples later.
    fn moved(steps: &[(usize, Step)], by: usize) -> Vec<(usize, Step)> {
        let mut later = Vec::new();
        for (at, step) in steps {
            later.push((at + by, *step));
        }
        later
    }

    // Renders `frames` frames in chunks of at most `chunk`, and keeps only
    // the left channel.
    fn render_frames_left(engine: &mut Engine, frames: usize, chunk: usize) -> Vec<f32> {
        let mut left = Vec::new();
        while left.len() < frames {
            let size = usize::min(chunk, frames - left.len());
            let samples = engine.render(size);
            for frame in samples.chunks(CHANNELS) {
                left.push(frame[0]);
            }
        }
        left
    }

    // Renders `frames` frames, taking each step on the sample it is at, and
    // keeps only the left channel. Steps on the same sample are taken in the
    // order given. The sound between two steps renders in one go.
    fn play(engine: &mut Engine, steps: &[(usize, Step)], frames: usize) -> Vec<f32> {
        play_in_chunks(engine, steps, frames, usize::MAX)
    }

    // Like `play`, but renders in chunks of at most `chunk` frames, the way
    // the speakers ask for sound.
    fn play_in_chunks(engine: &mut Engine, steps: &[(usize, Step)], frames: usize, chunk: usize) -> Vec<f32> {
        // Sorting keeps steps on the same sample in their order.
        let mut in_order = steps.to_vec();
        in_order.sort_by_key(|(at, _)| *at);

        let mut wave = Vec::new();
        for (at, step) in in_order {
            let stretch = render_frames_left(engine, at - wave.len(), chunk);
            wave.extend(stretch);
            take_step(engine, step);
        }
        let rest = render_frames_left(engine, frames - wave.len(), chunk);
        wave.extend(rest);
        wave
    }

    // The first layer's notes in Rust's copy: the first loop's.
    fn first_layer_notes(engine: &mut Engine) -> Vec<LoopNote> {
        match engine.loop_layers().first() {
            Some(layer) => layer.notes.clone(),
            None => Vec::new(),
        }
    }

    // Checks that two renders are the same, sample for sample.
    fn assert_same_samples(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for i in 0..actual.len() {
            assert!(
                actual[i] == expected[i],
                "sample {i} is {}, expected {}",
                actual[i],
                expected[i]
            );
        }
    }

    #[test]
    fn the_first_press_counts_in_one_bar_then_records_from_beat_1() {
        let mut engine = Engine::new().unwrap();
        engine.press_looper();

        // On its way to the engine: already busy, though not counting in yet.
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, true));
        assert!(engine.is_looper_busy());

        // Each beat of the count-in, then each bar recorded.
        let mut wave = Vec::new();
        for beat in 1..=4 {
            let part = render_frames_left(&mut engine, BEAT_AT_120, usize::MAX);
            wave.extend(part);
            assert_eq!(engine.looper_status(), looper(LooperState::CountingIn, beat, false));
        }
        for bar in 1..=2 {
            let part = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
            wave.extend(part);
            assert_eq!(engine.looper_status(), looper(LooperState::Recording, bar, false));
        }

        // The click ticks on every beat throughout, higher on each beat 1.
        let mut expected = Vec::new();
        for beat in 0..12 {
            expected.push(beat * BEAT_AT_120);
        }
        assert_eq!(tick_starts(&wave), expected);
        for start in [0, BAR_AT_120, 2 * BAR_AT_120] {
            assert_note(&engine, tick_at(&engine, &wave, start), HIGH_TICK_HZ);
        }
    }

    #[test]
    fn the_looper_says_how_far_into_the_count_in_the_recording_and_each_repeat_it_is() {
        let mut engine = Engine::new().unwrap();
        assert_eq!(engine.looper_position(), 0);

        // The count-in counts from the press.
        engine.press_looper();
        render_frames_left(&mut engine, 30_000, usize::MAX);
        assert_eq!(engine.looper_position(), 30_000);

        // Recording counts from 0 again, from its first sample, and on past
        // its first bar.
        render_frames_left(&mut engine, RECORDING_STARTS - 30_000 + 10_000, usize::MAX);
        assert_eq!(engine.looper_status().state, LooperState::Recording);
        assert_eq!(engine.looper_position(), 10_000);
        render_frames_left(&mut engine, BAR_AT_120 + 70_000, usize::MAX);
        assert_eq!(engine.looper_position(), BAR_AT_120 as u64 + 80_000);

        // Pressed 80,000 samples into bar 2, nearer its end: the loop closes
        // there, 2 bars long, and each repeat counts from 0 again.
        engine.press_looper();
        render_frames_left(&mut engine, 16_000 + 5_000, usize::MAX);
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 2, false));
        assert_eq!(engine.looper_position(), 5_000);
        render_frames_left(&mut engine, 2 * BAR_AT_120 + 2_000, usize::MAX);
        assert_eq!(engine.looper_position(), 7_000);

        // Cleared, it is back to 0.
        engine.clear_loop();
        render_frames_left(&mut engine, 100, usize::MAX);
        assert_eq!(engine.looper_position(), 0);

        // A new count-in counts from its own press.
        engine.press_looper();
        render_frames_left(&mut engine, 12_000, usize::MAX);
        assert_eq!(engine.looper_position(), 12_000);
    }

    #[test]
    fn rust_keeps_a_copy_of_the_loop_s_notes() {
        // While recording: A4 played and let go, C5 still held.
        let start = RECORDING_STARTS;
        let steps = [
            (0, PressLooper),
            (start + 10_000, NoteOn(A4)),
            (start + 20_000, NoteOff(A4)),
            (start + 60_000, NoteOn(C5)),
        ];
        let mut engine = Engine::new().unwrap();
        play(&mut engine, &steps, start + 70_000);
        let a4 = LoopNote {
            note: A4,
            start: 10_000,
            end: Some(20_000),
        };
        let held_c5 = LoopNote {
            note: C5,
            start: 60_000,
            end: None,
        };
        assert_eq!(first_layer_notes(&mut engine), vec![a4, held_c5]);

        // Pressed in bar 1, the loop closes at its end, and C5, still held
        // there, ends there too. Letting go of it later changes nothing.
        engine.press_looper();
        render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        engine.note_off(C5);
        render_frames_left(&mut engine, 1_000, usize::MAX);
        let c5 = LoopNote {
            note: C5,
            start: 60_000,
            end: Some(BAR_AT_120 as u64),
        };
        assert_eq!(first_layer_notes(&mut engine), vec![a4, c5]);

        // Esc forgets them.
        engine.remove_newest_layer();
        render_frames_left(&mut engine, 1_000, usize::MAX);
        assert!(engine.loop_layers().is_empty());

        // A new recording starts with only its own notes, even when the
        // copy was last read before the old loop was cleared.
        let mut engine = Engine::new().unwrap();
        play(&mut engine, &steps, start + 70_000);
        engine.loop_layers();
        engine.clear_loop();
        let again = [(0, PressLooper), (start + 4_000, NoteOn(E4))];
        play(&mut engine, &again, start + 8_000);
        let e4 = LoopNote {
            note: E4,
            start: 4_000,
            end: None,
        };
        assert_eq!(first_layer_notes(&mut engine), vec![e4]);
    }

    #[test]
    fn a_loop_closed_at_the_bar_line_just_passed_leaves_out_the_notes_after_it() {
        // E4 starts in bar 1 and is still held. G4 starts just after bar 2
        // does.
        let start = RECORDING_STARTS;
        let steps = [
            (0, PressLooper),
            (start + 50_000, NoteOn(E4)),
            (start + BAR_AT_120 + 5_000, NoteOn(G4)),
        ];
        let mut engine = Engine::new().unwrap();
        play(&mut engine, &steps, start + BAR_AT_120 + 10_000);
        assert_eq!(first_layer_notes(&mut engine).len(), 2);

        // Pressed 20,000 samples into bar 2, nearer its start: the loop
        // closes where bar 2 started, 1 bar long. E4 ends on that bar line,
        // and G4, played after it, is left out.
        render_frames_left(&mut engine, 10_000, usize::MAX);
        engine.press_looper();
        render_frames_left(&mut engine, 10_000, usize::MAX);
        let e4 = LoopNote {
            note: E4,
            start: 50_000,
            end: Some(BAR_AT_120 as u64),
        };
        assert_eq!(first_layer_notes(&mut engine), vec![e4]);
    }

    #[test]
    fn a_loop_plays_its_notes_on_the_same_samples_every_time_around() {
        // Two notes in a 1-bar loop, counted from the loop's start. The
        // press late in bar 1 closes the loop at its end.
        let bar = [
            (10_000, NoteOn(A4)),
            (20_000, NoteOff(A4)),
            (40_000, NoteOn(C5)),
            (65_000, NoteOff(C5)),
        ];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        for time_around in 0..4 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 4 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(looper_engine.looper_status(), looper(LooperState::Looping, 1, false));
    }

    #[test]
    fn a_loop_is_at_least_one_bar() {
        // Pressed half a beat into bar 1, the nearest bar line is where
        // recording started. A loop can't be empty, so it closes at the end
        // of bar 1, and the A4 played after the press is in it.
        let bar = [(30_000, NoteOn(A4)), (40_000, NoteOff(A4))];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 12_000, PressLooper)];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(looper_engine.looper_status(), looper(LooperState::Looping, 1, false));
    }

    #[test]
    fn pressing_a_little_early_records_on_to_the_bar_line() {
        // Pressed 70,000 samples into bar 2, nearer its end than its start:
        // recording goes on to the bar line, so the A4 played after the press
        // is in the loop too. The loop is 2 bars.
        let bars = [
            (10_000, NoteOn(C5)),
            (20_000, NoteOff(C5)),
            (BAR_AT_120 + 75_000, NoteOn(A4)),
            (BAR_AT_120 + 90_000, NoteOff(A4)),
        ];
        let start = RECORDING_STARTS;
        let loop_length = 2 * BAR_AT_120;
        let mut looped = vec![(0, PressLooper), (start + BAR_AT_120 + 70_000, PressLooper)];
        looped.extend(moved(&bars, start));

        let mut live = vec![(0, StartClick), (start + loop_length, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bars, start + time_around * loop_length));
        }

        let frames = start + 3 * loop_length;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(looper_engine.looper_status(), looper(LooperState::Looping, 2, false));
    }

    #[test]
    fn recording_on_to_the_bar_line_still_shows_as_recording() {
        // Pressed a little before the end of bar 2, the looper records on
        // to the bar line, and the screen says recording, bar 2, until the
        // loop plays.
        let bar_line = RECORDING_STARTS + 2 * BAR_AT_120;
        let steps = [(0, PressLooper), (RECORDING_STARTS + BAR_AT_120 + 70_000, PressLooper)];
        let mut engine = Engine::new().unwrap();
        play(&mut engine, &steps, bar_line);
        assert_eq!(engine.looper_status(), looper(LooperState::Recording, 2, false));

        render_frames_left(&mut engine, 1, usize::MAX);
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 2, false));
    }

    // Two bars recorded, counted from the loop's start.
    const LATE_CLOSE_BARS: [(usize, Step); 7] = [
        // Over before the press, and pressed again while held, the way a
        // second chord key sharing it would.
        (1_000, NoteOn(C5)),
        (2_000, NoteOn(C5)),
        (3_000, NoteOff(C5)),
        // Held across the moment of the press.
        (5_000, NoteOn(E4)),
        (60_000, NoteOff(E4)),
        // In bar 2.
        (BAR_AT_120 + 30_000, NoteOn(A4)),
        (BAR_AT_120 + 50_000, NoteOff(A4)),
    ];

    // Where bar 3 starts, and the press 20,000 samples into it.
    const BAR_3: usize = RECORDING_STARTS + 2 * BAR_AT_120;
    const LATE_PRESS: usize = BAR_3 + 20_000;

    // Records LATE_CLOSE_BARS, plays G4 from bar 3's very first sample, then
    // presses late.
    fn late_close_steps() -> Vec<(usize, Step)> {
        let mut steps = vec![
            (0, PressLooper),
            (BAR_3, NoteOn(G4)),
            (BAR_3 + 10_000, NoteOff(G4)),
            (LATE_PRESS, PressLooper),
        ];
        steps.extend(moved(&LATE_CLOSE_BARS, RECORDING_STARTS));
        steps
    }

    #[test]
    fn pressing_a_little_late_closes_on_the_bar_line_just_passed() {
        // The press comes 20,000 samples into bar 3, nearer its start than
        // its end. The loop closes on that bar line, 2 bars long, so it has
        // already been playing for 20,000 samples: E4, which would be
        // sounding by now, starts right away, and C5, already over, waits
        // for the next time around. G4, played from bar 3's very first sample
        // until before the press, is not in the loop.
        let mut live = vec![
            (0, StartClick),
            (BAR_3, NoteOn(G4)),
            (BAR_3 + 10_000, NoteOff(G4)),
            (LATE_PRESS, StopClick),
        ];
        live.extend(moved(&LATE_CLOSE_BARS, RECORDING_STARTS));
        // The first time around, from 20,000 samples in.
        live.push((LATE_PRESS, NoteOn(E4)));
        live.push((BAR_3 + 60_000, NoteOff(E4)));
        live.extend(moved(&LATE_CLOSE_BARS[5..], BAR_3));
        // Then all of it, twice more.
        let loop_length = 2 * BAR_AT_120;
        live.extend(moved(&LATE_CLOSE_BARS, BAR_3 + loop_length));
        live.extend(moved(&LATE_CLOSE_BARS, BAR_3 + 2 * loop_length));

        let frames = BAR_3 + 3 * loop_length;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &late_close_steps(), frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(looper_engine.looper_status(), looper(LooperState::Looping, 2, false));
    }

    #[test]
    fn a_note_played_on_the_sample_recording_starts_is_recorded() {
        // With 10 ms chunks at 120 beats per minute, every bar line falls on
        // a chunk's first sample, so a note pressed just before beat 1 is
        // heard from the very sample recording starts. It is in the loop.
        let bar = [(0, NoteOn(A4)), (10_000, NoteOff(A4))];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn a_press_right_on_a_bar_line_closes_the_loop_there() {
        // Pressed on the very sample bar 3 starts, the nearest bar line is
        // that one. The loop is 2 bars long, and A4, its first note, starts
        // right away.
        let bars = [
            (0, NoteOn(A4)),
            (10_000, NoteOff(A4)),
            (BAR_AT_120 + 20_000, NoteOn(C5)),
            (BAR_AT_120 + 40_000, NoteOff(C5)),
        ];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (BAR_3, PressLooper)];
        looped.extend(moved(&bars, start));

        let loop_length = 2 * BAR_AT_120;
        let mut live = vec![(0, StartClick), (BAR_3, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bars, start + time_around * loop_length));
        }

        let frames = start + 3 * loop_length;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(looper_engine.looper_status(), looper(LooperState::Looping, 2, false));
    }

    #[test]
    fn loops_play_the_same_samples_however_the_sound_is_asked_for() {
        // The speakers ask for sound a chunk at a time, and loop notes and
        // bar lines fall anywhere inside a chunk. 7 ms chunks never line up
        // with them, yet every sample must match a render made in one go.
        let steps = late_close_steps();
        let frames = BAR_3 + 4 * BAR_AT_120;
        let mut whole = Engine::new().unwrap();
        let mut chunked = Engine::new().unwrap();
        let chunk = frames_in_ms(&chunked, 7);
        let whole_wave = play(&mut whole, &steps, frames);
        let chunked_wave = play_in_chunks(&mut chunked, &steps, frames, chunk);
        assert_same_samples(&chunked_wave, &whole_wave);
    }

    #[test]
    fn the_click_stops_once_the_loop_plays() {
        // An empty 1-bar loop: the count-in and the bar recorded tick, then
        // nothing does.
        let mut engine = Engine::new().unwrap();
        let steps = [(0, PressLooper), (RECORDING_STARTS + 80_000, PressLooper)];
        let wave = play(&mut engine, &steps, RECORDING_STARTS + 4 * BAR_AT_120);

        let mut expected = Vec::new();
        for beat in 0..8 {
            expected.push(beat * BEAT_AT_120);
        }
        assert_eq!(tick_starts(&wave), expected);
    }

    #[test]
    fn a_loop_never_drifts_from_the_beat() {
        // At 115 beats per minute a bar is 100,173.91... samples, so bars
        // can't all be the same whole number of samples long: most are
        // 100,174 and a few 100,173. Each time around starts on its bar's
        // first sample, worked out by the clock from the start, so after 60
        // times around the notes still land on the exact samples. E4, held
        // when the loop closes, is let go right on each bar line, even after
        // a bar a sample shorter than the loop.
        let tempo = 115;
        // The sample bar `n` starts on. The count-in is bar 0.
        let bar_line = |n: usize| n * 4 * SAMPLES_PER_MINUTE / tempo;
        let bar = [(1_000, NoteOn(A4)), (5_800, NoteOff(A4)), (80_000, NoteOn(E4))];
        let start = bar_line(1);
        let times_around = 60;

        let mut looped = vec![(0, PressLooper), (start + 90_000, PressLooper)];
        looped.extend(moved(&bar, start));
        // The live E4 is let go after the loop closes, so it isn't recorded.
        looped.push((bar_line(2) + 20_000, NoteOff(E4)));

        let mut live = vec![(0, StartClick)];
        live.extend(moved(&bar, start));
        live.push((bar_line(2), StopClick));
        live.push((bar_line(2) + 20_000, NoteOff(E4)));
        for n in 2..2 + times_around {
            live.extend(moved(&bar, bar_line(n)));
            live.push((bar_line(n + 1), NoteOff(E4)));
        }

        // Up to just before A4 would start again, so E4 is heard let go on
        // the last bar line too.
        let frames = bar_line(2 + times_around) + 1_000;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        looper_engine.set_tempo(115);
        live_engine.set_tempo(115);
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn a_note_held_when_the_loop_closes_is_let_go_at_its_end() {
        // A4 is still held when the loop closes, and let go 20,000 samples
        // later. The loop lets go of it right at its end each time around,
        // so it never keeps ringing.
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let looped = [
            (0, PressLooper),
            (start + 48_000, NoteOn(A4)),
            (start + 80_000, PressLooper),
            (first_time + 20_000, NoteOff(A4)),
        ];

        let mut live = vec![
            (0, StartClick),
            (start + 48_000, NoteOn(A4)),
            (first_time, StopClick),
            (first_time + 20_000, NoteOff(A4)),
        ];
        for time_around in 0..3 {
            let bar = first_time + time_around * BAR_AT_120;
            live.push((bar + 48_000, NoteOn(A4)));
            live.push((bar + BAR_AT_120, NoteOff(A4)));
        }

        let frames = first_time + 3 * BAR_AT_120 + 20_000;
        let mut looper_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn quick_and_repeated_notes_loop_exactly_as_played() {
        // A4 is let go on the very sample it starts. C5 is pressed again
        // while held, the way a second chord key sharing it would.
        let bar = [
            (10_000, NoteOn(A4)),
            (10_000, NoteOff(A4)),
            (20_000, NoteOn(C5)),
            (30_000, NoteOn(C5)),
            (40_000, NoteOff(C5)),
        ];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn letting_go_of_everything_ends_the_notes_being_recorded() {
        // When the window loses focus every note is let go, and the loop
        // lets go of them there too.
        let bar = [(10_000, NoteOn(A4)), (10_000, NoteOn(C5)), (30_000, AllNotesOff)];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        live.extend(moved(&bar, start));
        for time_around in 1..3 {
            let bar_start = start + time_around * BAR_AT_120;
            live.push((bar_start + 10_000, NoteOn(A4)));
            live.push((bar_start + 10_000, NoteOn(C5)));
            live.push((bar_start + 30_000, NoteOff(A4)));
            live.push((bar_start + 30_000, NoteOff(C5)));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn losing_focus_lets_go_of_live_notes_but_the_loop_plays_on() {
        // C5 is played live over the loop's A4, and the window loses focus
        // while both sound: C5 is let go, just as if its key came up, and
        // A4 plays on.
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let bar = [(10_000, NoteOn(A4)), (60_000, NoteOff(A4))];
        let mut steps = vec![
            (0, PressLooper),
            (start + 80_000, PressLooper),
            (first_time + 20_000, NoteOn(C5)),
        ];
        steps.extend(moved(&bar, start));
        let mut lost_focus = steps.clone();
        lost_focus.push((first_time + 40_000, AllNotesOff));
        let mut let_go = steps.clone();
        let_go.push((first_time + 40_000, NoteOff(C5)));

        let frames = first_time + 2 * BAR_AT_120;
        let mut lost_focus_engine = Engine::new().unwrap();
        let mut let_go_engine = Engine::new().unwrap();
        let lost_focus_wave = play(&mut lost_focus_engine, &lost_focus, frames);
        let let_go_wave = play(&mut let_go_engine, &let_go, frames);
        assert_same_samples(&lost_focus_wave, &let_go_wave);
    }

    #[test]
    fn notes_played_before_the_count_in_s_last_quarter_beat_are_not_recorded() {
        let start = RECORDING_STARTS;
        let notes = [(30_000, NoteOn(C5)), (40_000, NoteOff(C5))];
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(notes);
        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        live.extend(notes);

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn loop_notes_and_live_notes_never_cut_each_other_off() {
        // The loop plays A4 from 1,000 to 60,000 samples into each bar.
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let mut steps = vec![
            (0, PressLooper),
            (start + 1_000, NoteOn(A4)),
            (start + 60_000, NoteOff(A4)),
            (start + 80_000, PressLooper),
        ];
        // The first time around, A4 is played live as well: once while the
        // loop's A4 sounds, and once starting while it sounds and held after
        // the loop lets go of its own.
        steps.push((first_time + 20_000, NoteOn(A4)));
        steps.push((first_time + 30_000, NoteOff(A4)));
        steps.push((first_time + 50_000, NoteOn(A4)));
        steps.push((first_time + 90_000, NoteOff(A4)));

        let mut engine = Engine::new().unwrap();
        let wave = play(&mut engine, &steps, first_time + BAR_AT_120);

        // Once the first live A4 has faded out, the loop's A4 still sounds.
        let release = frames_in_ms(&engine, RELEASE_MS);
        let loop_alone = &wave[first_time + 30_000 + release + 100..first_time + 60_000];
        assert_within_percent(peak(loop_alone), held_level(), 2.0);

        // Once the loop's A4 has faded out, the held live A4 still sounds.
        let live_alone = &wave[first_time + 60_000 + release + 100..first_time + 90_000];
        assert_within_percent(peak(live_alone), held_level(), 2.0);
    }

    #[test]
    fn esc_on_the_only_layer_stops_the_loop_and_forgets_it() {
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let bar = [(10_000, NoteOn(A4)), (60_000, NoteOff(A4))];
        let mut looped = vec![
            (0, PressLooper),
            (start + 80_000, PressLooper),
            (first_time + 30_000, RemoveNewest),
        ];
        looped.extend(moved(&bar, start));

        // Esc lets go of the loop's A4, and nothing plays after it.
        let mut live = vec![
            (0, StartClick),
            (first_time, StopClick),
            (first_time + 10_000, NoteOn(A4)),
            (first_time + 30_000, NoteOff(A4)),
        ];
        live.extend(moved(&bar, start));

        let frames = first_time + 3 * BAR_AT_120;
        let mut engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, false));

        // The tempo is free again, and the next press counts in afresh at
        // it: a beat at 125 is 23,040 samples, so recording starts on the
        // fifth.
        assert_eq!(engine.set_tempo(125), 125);
        engine.press_looper();
        let wave = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        assert_eq!(tick_starts(&wave), vec![0, 23_040, 46_080, 69_120, 92_160]);
        assert_eq!(engine.looper_status(), looper(LooperState::Recording, 1, false));
    }

    #[test]
    fn after_esc_the_click_starts_on_a_new_bar() {
        // Esc stops the beat the looper started, so the click started
        // afterwards begins a new bar right away, high tick first.
        let mut engine = Engine::new().unwrap();
        engine.press_looper();
        render_left(&mut engine, 600);
        engine.remove_newest_layer();
        render_left(&mut engine, 100);
        engine.start_click();
        let wave = render_left(&mut engine, 100);

        assert_eq!(tick_starts(&wave), vec![0]);
        assert_note(&engine, tick_at(&engine, &wave, 0), HIGH_TICK_HZ);
    }

    #[test]
    fn a_press_during_the_count_in_cancels_it() {
        // Cancelled after beat 2 starts: no more ticks, and the tempo is
        // free again.
        let mut engine = Engine::new().unwrap();
        let steps = [(0, PressLooper), (30_000, PressLooper)];
        let wave = play(&mut engine, &steps, 2 * BAR_AT_120);
        assert_eq!(tick_starts(&wave), vec![0, BEAT_AT_120]);
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, false));

        assert_eq!(engine.set_tempo(125), 125);
    }

    #[test]
    fn a_press_while_closing_changes_nothing() {
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let bar = [(10_000, NoteOn(A4)), (60_000, NoteOff(A4))];
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));

        // Recording on to the bar line, a press is ignored.
        let mut pressed = looped.clone();
        pressed.push((start + 90_000, PressLooper));

        let frames = first_time + 3 * BAR_AT_120;
        let mut engine = Engine::new().unwrap();
        let mut pressed_engine = Engine::new().unwrap();
        let looped_wave = play(&mut engine, &looped, frames);
        let pressed_wave = play(&mut pressed_engine, &pressed, frames);
        assert_same_samples(&pressed_wave, &looped_wave);
        assert_eq!(pressed_engine.looper_status(), looper(LooperState::Looping, 1, false));
    }

    #[test]
    fn loop_notes_play_the_wave_chosen_when_they_start() {
        // Switched to square after A4 has played once around: from then on,
        // the loop's A4 plays square.
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let bar = [(10_000, NoteOn(A4)), (60_000, NoteOff(A4))];
        let switch = (first_time + 80_000, SetFirstKind(Kind::Square));
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper), switch];
        looped.extend(moved(&bar, start));

        let mut live = vec![(0, StartClick), (first_time, StopClick), switch];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    // How many notes a loop holds. When it is changed there on purpose,
    // change it here too.
    const LOOP_NOTES: usize = 4096; // `max_notes` in engine/src/looper.h

    #[test]
    fn a_full_loop_keeps_4096_notes_and_plays_the_rest_live() {
        // 4,096 short notes fill the loop, one every 7 samples. C5, the
        // note after them, still sounds live but isn't recorded.
        let mut bar = Vec::new();
        for i in 0..LOOP_NOTES {
            let note = C3 + (i % 24) as u8;
            bar.push((7 * i, NoteOn(note)));
            bar.push((7 * i + 3, NoteOff(note)));
        }
        let start = RECORDING_STARTS;
        let mut looped = vec![
            (0, PressLooper),
            (start + 40_000, NoteOn(C5)),
            (start + 45_000, NoteOff(C5)),
            (start + 80_000, PressLooper),
        ];
        looped.extend(moved(&bar, start));

        let mut live = vec![
            (0, StartClick),
            (start + 40_000, NoteOn(C5)),
            (start + 45_000, NoteOff(C5)),
            (start + BAR_AT_120, StopClick),
        ];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn the_tempo_is_locked_while_the_looper_is_busy() {
        let mut engine = Engine::new().unwrap();
        engine.press_looper();

        // Locked from the moment of the press, even before the engine has
        // acted on it.
        assert_eq!(engine.set_tempo(125), START_TEMPO);
        let count_in = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        assert_eq!(engine.set_tempo(115), START_TEMPO);

        // Asked directly, the engine keeps its beat as well.
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tempo(engine.raw, 60) };
        let recording = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        assert_eq!(tick_starts(&count_in), vec![0, 24_000, 48_000, 72_000]);
        assert_eq!(tick_starts(&recording), vec![0, 24_000, 48_000, 72_000]);

        // Esc unlocks it, once the engine has acted on it.
        engine.remove_newest_layer();
        assert_eq!(engine.set_tempo(125), START_TEMPO);
        render_left(&mut engine, 10);
        assert_eq!(engine.set_tempo(125), 125);
    }

    // Snap to beat (docs/adr/0021-a-looper-on-a-beat.md): each time around,
    // the loop plays its notes moved to the nearest half beat, an eighth
    // note. At 120 that is every 12,000 samples. The first time through
    // sounds as played, since live notes are never moved.

    // Records `played`, counted from the loop's start, into a 1-bar loop
    // closed by a press `close_at` samples in, and plays it 3 times around.
    // Checks every sample against playing it by hand: `played` the first
    // time through, then `each_time` every time around.
    fn assert_loop_plays(mut looper_engine: Engine, played: &[(usize, Step)], close_at: usize, each_time: &[(usize, Step)]) {
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + close_at, PressLooper)];
        looped.extend(moved(played, start));

        let mut live = vec![(0, StartClick)];
        live.extend(moved(played, start));
        live.push((start + BAR_AT_120, StopClick));
        for time_around in 1..4 {
            live.extend(moved(each_time, start + time_around * BAR_AT_120));
        }

        let frames = start + 4 * BAR_AT_120;
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn each_time_around_a_note_starts_and_ends_on_the_nearest_half_beat() {
        // A new engine snaps. A4 moves earlier at both ends, C5 starts
        // earlier and ends later, and E4 moves later at both ends.
        let played = [
            (13_000, NoteOn(A4)),
            (29_000, NoteOff(A4)),
            (41_000, NoteOn(C5)),
            (55_000, NoteOff(C5)),
            (70_000, NoteOn(E4)),
            (80_000, NoteOff(E4)),
        ];
        let each_time = [
            (12_000, NoteOn(A4)),
            (24_000, NoteOff(A4)),
            (36_000, NoteOn(C5)),
            (60_000, NoteOff(C5)),
            (72_000, NoteOn(E4)),
            (84_000, NoteOff(E4)),
        ];
        assert_loop_plays(Engine::new().unwrap(), &played, 85_000, &each_time);
    }

    #[test]
    fn a_snapped_note_lasts_at_least_half_a_beat() {
        // A quick tap would start and end on the same half beat, so it ends
        // on the next one.
        let played = [(25_000, NoteOn(A4)), (26_000, NoteOff(A4))];
        let each_time = [(24_000, NoteOn(A4)), (36_000, NoteOff(A4))];
        assert_loop_plays(Engine::new().unwrap(), &played, 85_000, &each_time);
    }

    #[test]
    fn a_note_within_a_quarter_beat_of_the_loop_s_end_moves_round_to_beat_1() {
        // At 40 beats per minute, the slowest, a bar is 288,000 samples, a
        // half beat 36,000 and a quarter beat 18,000. That leaves time for
        // a note played near the loop's end to fade out before the loop
        // plays it again on beat 1: by hand, the two would share one voice.
        //
        // The loop ends 288,000 samples in. A4, 17,000 before that, is
        // nearer the next time around's beat 1, so it plays there first
        // thing, for half a beat. E4, 19,000 before, is nearer the last half
        // beat, so it stays, and ends with the loop. Both are played after
        // the press, while the loop records on to the bar line.
        let bar = 288_000;
        let start = bar;
        let played = [
            (108_000, NoteOn(C5)),
            (144_000, NoteOff(C5)),
            (bar - 19_000, NoteOn(E4)),
            (bar - 17_500, NoteOff(E4)),
            (bar - 17_000, NoteOn(A4)),
            (bar - 16_000, NoteOff(A4)),
        ];
        let each_time = [
            (0, NoteOn(A4)),
            (36_000, NoteOff(A4)),
            (108_000, NoteOn(C5)),
            (144_000, NoteOff(C5)),
            (252_000, NoteOn(E4)),
            (bar, NoteOff(E4)),
        ];

        let mut looped = vec![(0, PressLooper), (start + 200_000, PressLooper)];
        looped.extend(moved(&played, start));
        let mut live = vec![(0, StartClick)];
        live.extend(moved(&played, start));
        live.push((start + bar, StopClick));
        for time_around in 1..4 {
            live.extend(moved(&each_time, start + time_around * bar));
        }

        let frames = start + 4 * bar;
        let mut looper_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        looper_engine.set_tempo(40);
        live_engine.set_tempo(40);
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn two_presses_of_one_key_never_cut_each_other_off() {
        // Both A4s snap to beat 1. The first, a quick tap, would last half a
        // beat, and its end would cut off the second: the synth plays a key
        // on one voice. So it ends where the second starts, and the second
        // plays on to its own end.
        let played = [
            (1_000, NoteOn(A4)),
            (2_000, NoteOff(A4)),
            (5_000, NoteOn(A4)),
            (50_000, NoteOff(A4)),
        ];
        let each_time = [
            (0, NoteOn(A4)),
            (0, NoteOff(A4)),
            (0, NoteOn(A4)),
            (48_000, NoteOff(A4)),
        ];
        assert_loop_plays(Engine::new().unwrap(), &played, 85_000, &each_time);
    }

    #[test]
    fn a_note_let_go_while_an_earlier_one_is_held_ends_first() {
        // C5 is held like a chord under A4, which starts after it and ends
        // before it.
        let played = [
            (1_000, NoteOn(C5)),
            (13_000, NoteOn(A4)),
            (23_000, NoteOff(A4)),
            (47_000, NoteOff(C5)),
        ];
        let each_time = [
            (0, NoteOn(C5)),
            (12_000, NoteOn(A4)),
            (24_000, NoteOff(A4)),
            (48_000, NoteOff(C5)),
        ];
        assert_loop_plays(Engine::new().unwrap(), &played, 85_000, &each_time);
    }

    #[test]
    fn a_note_played_a_little_before_recording_starts_counts_as_beat_1() {
        // In the count-in's last quarter beat, 6,000 samples at 120, notes
        // are recorded as played on beat 1 and keep their lengths: E4 even
        // lets go before recording starts. C5, a little earlier, is not
        // recorded. Snap is off, so the loop keeps the lengths as played.
        let start = RECORDING_STARTS;
        let played = [
            (start - 7_000, NoteOn(C5)),
            (start - 6_500, NoteOff(C5)),
            (start - 6_000, NoteOn(E4)),
            (start - 5_000, NoteOn(A4)),
            (start - 3_000, NoteOff(E4)),
            (start + 5_000, NoteOff(A4)),
        ];
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(played);

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        live.extend(played);
        for time_around in 1..4 {
            let bar = start + time_around * BAR_AT_120;
            live.push((bar, NoteOn(E4)));
            live.push((bar, NoteOn(A4)));
            live.push((bar + 3_000, NoteOff(E4)));
            live.push((bar + 10_000, NoteOff(A4)));
        }

        let frames = start + 4 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);

        // Rust's copy has them on beat 1 too.
        let e4 = LoopNote {
            note: E4,
            start: 0,
            end: Some(3_000),
        };
        let a4 = LoopNote {
            note: A4,
            start: 0,
            end: Some(10_000),
        };
        assert_eq!(first_layer_notes(&mut looper_engine), vec![e4, a4]);
    }

    #[test]
    fn an_early_note_and_the_same_key_again_never_cut_each_other_off() {
        // A4 is pressed 5,000 samples early, so it moves to beat 1 keeping
        // its length, and pressed again 2,000 samples in: the first would
        // now end at 7,000, after the second starts. So it ends where the
        // second starts, even with snap off.
        let start = RECORDING_STARTS;
        let played = [
            (start - 5_000, NoteOn(A4)),
            (start + 2_000, NoteOn(A4)),
            (start + 20_000, NoteOff(A4)),
        ];
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(played);

        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        live.extend(played);
        for time_around in 1..4 {
            let bar = start + time_around * BAR_AT_120;
            live.push((bar, NoteOn(A4)));
            live.push((bar + 2_000, NoteOff(A4)));
            live.push((bar + 2_000, NoteOn(A4)));
            live.push((bar + 20_000, NoteOff(A4)));
        }

        let frames = start + 4 * BAR_AT_120;
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn switching_snap_changes_a_loop_already_playing_right_away() {
        // Snapped, A4 plays from 12,000 to 24,000 and C5 from 36,000 to
        // 84,000.
        let played = [
            (13_000, NoteOn(A4)),
            (29_000, NoteOff(A4)),
            (31_000, NoteOn(C5)),
            (89_000, NoteOff(C5)),
        ];
        let snapped = [
            (12_000, NoteOn(A4)),
            (24_000, NoteOff(A4)),
            (36_000, NoteOn(C5)),
            (84_000, NoteOff(C5)),
        ];
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let second_time = first_time + BAR_AT_120;
        let third_time = second_time + BAR_AT_120;

        // Off 50,000 samples into the first time around, and on again
        // 86,000 into the second.
        let mut looped = vec![
            (0, PressLooper),
            (start + 85_000, PressLooper),
            (first_time + 50_000, Snap(false)),
            (second_time + 86_000, Snap(true)),
        ];
        looped.extend(moved(&played, start));

        // At the first switch C5 should be sounding either way, so it is let
        // go and started again, and ends where its timing as played ends it.
        // At the second, as played it sounds until 89,000, but snapped it
        // ended at 84,000, so it stops right away.
        let mut live = vec![(0, StartClick), (first_time, StopClick)];
        live.extend(moved(&played, start));
        live.extend(moved(&snapped[..3], first_time));
        live.push((first_time + 50_000, NoteOff(C5)));
        live.push((first_time + 50_000, NoteOn(C5)));
        live.push((first_time + 89_000, NoteOff(C5)));
        live.extend(moved(&played[..3], second_time));
        live.push((second_time + 86_000, NoteOff(C5)));
        live.extend(moved(&snapped, third_time));

        let frames = third_time + BAR_AT_120;
        let mut looper_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);
    }

    #[test]
    fn turning_snap_on_while_it_is_on_changes_nothing() {
        // C5 sounds from 36,000 to 84,000 samples in each time around. The
        // wave is switched while it sounds, and turning snap on again
        // neither stops nor restarts it: it keeps the wave it started with.
        let played = [(36_000, NoteOn(C5)), (84_000, NoteOff(C5))];
        let start = RECORDING_STARTS;
        let first_time = start + BAR_AT_120;
        let mut looped = vec![
            (0, PressLooper),
            (start + 85_000, PressLooper),
            (first_time + 40_000, SetFirstKind(Kind::Square)),
        ];
        looped.extend(moved(&played, start));
        let mut switched = looped.clone();
        switched.push((first_time + 50_000, Snap(true)));

        let frames = first_time + 2 * BAR_AT_120;
        let mut looper_engine = Engine::new().unwrap();
        let mut switched_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let switched_wave = play(&mut switched_engine, &switched, frames);
        assert_same_samples(&switched_wave, &looped_wave);
    }

    // A note of the loop that has ended, for Rust's copy.
    // Layers (docs/adr/0026-layers.md). Like the looper's other tests, most
    // of these play the same music twice: once through the looper, with
    // layers recorded over the loop, and once by hand. Each layer here
    // starts on a bar line, where a click started by hand falls in step with
    // the loop's beat. With snap off, the loop plays every note as played.

    // The first loop of these tests: A4 from 10,000 to 60,000 samples into
    // its one bar.
    const BASE: [(usize, Step); 2] = [(10_000, NoteOn(A4)), (60_000, NoteOff(A4))];

    // Where the loop first plays: the end of the bar recorded.
    const LOOP_STARTS: usize = RECORDING_STARTS + BAR_AT_120;

    // The sample time around `n` starts on, counting from 1.
    fn time_around(n: usize) -> usize {
        LOOP_STARTS + (n - 1) * BAR_AT_120
    }

    // The first loop through the looper: the second press, late in the bar,
    // closes it at the bar's end.
    fn base_looped() -> Vec<(usize, Step)> {
        let mut steps = vec![(0, PressLooper), (RECORDING_STARTS + 80_000, PressLooper)];
        steps.extend(moved(&BASE, RECORDING_STARTS));
        steps
    }

    // The same by hand for `times` times around: the click through the
    // count-in and the recording, and A4 played live every time.
    fn base_by_hand(times: usize) -> Vec<(usize, Step)> {
        let mut steps = vec![(0, StartClick), (LOOP_STARTS, StopClick)];
        steps.extend(moved(&BASE, RECORDING_STARTS));
        for n in 1..=times {
            steps.extend(moved(&BASE, time_around(n)));
        }
        steps
    }

    // Renders both and checks every sample matches.
    fn assert_layers_play(looped: &[(usize, Step)], live: &[(usize, Step)], frames: usize) -> Engine {
        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, looped, frames);
        let live_wave = play(&mut live_engine, live, frames);
        assert_same_samples(&looped_wave, &live_wave);
        looper_engine
    }

    #[test]
    fn a_layer_records_round_and_round_and_plays_from_the_next_time_around() {
        // The layer starts as the second time around starts, and records
        // through the third: C5 the first time, E4 the second, before C5.
        // Each plays from the time around after the one it was played in,
        // with the click on until the press that stops the layer.
        let second = time_around(2);
        let third = time_around(3);
        let c5 = [(20_000, NoteOn(C5)), (40_000, NoteOff(C5))];
        let e4 = [(5_000, NoteOn(E4)), (15_000, NoteOff(E4))];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&c5, second));
        looped.extend(moved(&e4, third));
        looped.push((third + 70_000, PressLooper));

        let mut live = base_by_hand(5);
        live.push((second, StartClick));
        live.push((third + 70_000, StopClick));
        for n in 2..=5 {
            live.extend(moved(&c5, time_around(n)));
        }
        for n in 3..=5 {
            live.extend(moved(&e4, time_around(n)));
        }

        let mut engine = assert_layers_play(&looped, &live, time_around(6));
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 1, false));

        // Rust's copy: the layer's notes in the order they were played,
        // counted from the loop's start.
        let layers = engine.loop_layers().to_vec();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[1].notes, vec![ended(C5, 20_000, 40_000), ended(E4, 5_000, 15_000)]);
        assert!(!layers[1].recording);
    }

    #[test]
    fn recording_a_layer_shows_with_the_loop_s_length_and_place() {
        // The first loop is turned all the way down, so only the click is
        // heard.
        let second = time_around(2);
        let mut steps = base_looped();
        steps.push((second, Volume(1, 0)));
        steps.push((second + 30_000, PressLooper));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, second + 50_000);
        assert_eq!(engine.looper_status(), looper(LooperState::Layering, 1, false));
        assert_eq!(engine.looper_position(), 50_000);
        assert!(engine.loop_layers()[1].recording);

        // The click comes back on the next beat, 72,000 samples into the
        // time around, and goes on every beat.
        let wave = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        assert_eq!(tick_starts(&wave), vec![22_000, 46_000, 70_000, 94_000]);
    }

    #[test]
    fn notes_held_when_a_layer_stops_record_until_let_go_but_never_past_the_loop_s_end() {
        // E4 goes down in the layer and is held into the next time around:
        // it is cut at the loop's end. G4 goes down before the press that
        // stops the layer and up after it, and keeps its whole length.
        let second = time_around(2);
        let third = time_around(3);
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.push((second + 50_000, NoteOn(E4)));
        looped.push((second + 60_000, NoteOn(G4)));
        looped.push((second + 70_000, PressLooper));
        looped.push((second + 90_000, NoteOff(G4)));
        looped.push((third + 30_000, NoteOff(E4)));

        let mut live = base_by_hand(4);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        live.push((second + 50_000, NoteOn(E4)));
        live.push((second + 60_000, NoteOn(G4)));
        live.push((second + 90_000, NoteOff(G4)));
        live.push((third + 30_000, NoteOff(E4)));
        for n in 3..=4 {
            let at = time_around(n);
            live.push((at + 50_000, NoteOn(E4)));
            live.push((at + BAR_AT_120, NoteOff(E4)));
            live.push((at + 60_000, NoteOn(G4)));
            live.push((at + 90_000, NoteOff(G4)));
        }

        let mut engine = assert_layers_play(&looped, &live, time_around(5));
        let layers = engine.loop_layers().to_vec();
        let loop_end = BAR_AT_120 as u64;
        assert_eq!(layers[1].notes, vec![ended(E4, 50_000, loop_end), ended(G4, 60_000, 90_000)]);
    }

    #[test]
    fn layers_play_the_same_key_on_their_own_voices() {
        // The first loop plays A4 from 10,000 to 60,000, and a layer plays it
        // from 28,800 to 76,800, while the first is still sounding. Each
        // keeps its own voice, so neither cuts the other off. By hand, the
        // second A4 is played by the song, which has voices of its own too.
        const SECOND_A4: &[TestChord] = &[chord(&[A4], 1000)];
        let second = time_around(2);
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.push((second + 28_800, NoteOn(A4)));
        looped.push((second + 76_800, NoteOff(A4)));
        looped.push((second + 90_000, PressLooper));

        let mut live = base_by_hand(4);
        live.push((second, StartClick));
        live.push((second + 90_000, StopClick));
        for n in 2..=4 {
            live.push((time_around(n) + 28_800, PlaySong(SECOND_A4)));
        }

        assert_layers_play(&looped, &live, time_around(5));
    }

    #[test]
    fn esc_removes_one_layer_and_the_others_play_on() {
        // A layer of C5 held under a quicker E4 over the first loop's A4.
        // Then layer 1, the first loop, is removed while its A4 sounds: A4
        // lets go at once and the layer plays on, E4 still letting go before
        // C5. Then the newest, now the only one, goes too, and the loop is
        // cleared.
        let second = time_around(2);
        let third = time_around(3);
        let fourth = time_around(4);
        let layer = [
            (20_000, NoteOn(C5)),
            (25_000, NoteOn(E4)),
            (35_000, NoteOff(E4)),
            (60_000, NoteOff(C5)),
        ];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&layer, second));
        looped.push((second + 70_000, PressLooper));
        looped.push((third + 30_000, RemoveLayer(1)));
        looped.push((fourth + 30_000, RemoveNewest));

        let mut live = base_by_hand(2);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        live.extend(moved(&layer, second));
        live.push((third + 10_000, NoteOn(A4)));
        live.push((third + 30_000, NoteOff(A4)));
        live.extend(moved(&layer, third));
        live.push((fourth + 20_000, NoteOn(C5)));
        live.push((fourth + 25_000, NoteOn(E4)));
        live.push((fourth + 30_000, NoteOff(C5)));
        live.push((fourth + 30_000, NoteOff(E4)));

        let mut engine = assert_layers_play(&looped, &live, time_around(6));
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, false));
        assert!(engine.loop_layers().is_empty());
        assert_eq!(engine.set_tempo(125), 125);
    }

    #[test]
    fn removing_a_layer_that_doesn_t_exist_changes_nothing() {
        // Rust only asks for layers it knows of, but its copy can be a chunk
        // behind the engine. Removing layer 3 of 2 while both sound leaves
        // them playing.
        let second = time_around(2);
        let third = time_around(3);
        let c5 = [(20_000, NoteOn(C5)), (40_000, NoteOff(C5))];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&c5, second));
        looped.push((second + 70_000, PressLooper));
        looped.push((third + 30_000, RemoveLayer(3)));

        let mut live = base_by_hand(4);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        for n in 2..=4 {
            live.extend(moved(&c5, time_around(n)));
        }

        let mut engine = assert_layers_play(&looped, &live, time_around(5));
        assert_eq!(engine.loop_layers().len(), 2);
    }

    #[test]
    fn after_a_removal_each_layer_keeps_its_own_voices_and_volume() {
        // A layer of C5 over the first loop's A4. The first loop is turned
        // all the way down once its A4 has faded, then removed. A new layer
        // then records E4 on the voices and volume the first loop had: it
        // plays as loud as played. Then layer 1, now C5's, is turned all the
        // way down: C5 goes quiet from its first sample, and E4 plays on.
        // The quiet C5 still takes a voice, so by hand E4 would land on
        // another voice and phase: the fifth time around is checked by
        // loudness instead of sample for sample.
        let second = time_around(2);
        let third = time_around(3);
        let fourth = time_around(4);
        let fifth = time_around(5);
        let c5 = [(20_000, NoteOn(C5)), (40_000, NoteOff(C5))];
        let e4 = [(50_000, NoteOn(E4)), (60_000, NoteOff(E4))];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&c5, second));
        looped.push((second + 70_000, PressLooper));
        looped.push((third + 80_000, Volume(1, 0)));
        looped.push((third + 90_000, RemoveLayer(1)));
        looped.push((fourth, PressLooper));
        looped.extend(moved(&e4, fourth));
        looped.push((fourth + 70_000, PressLooper));
        looped.push((fifth, Volume(1, 0)));

        let mut live = base_by_hand(3);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        live.push((fourth, StartClick));
        live.push((fourth + 70_000, StopClick));
        for n in 2..=4 {
            live.extend(moved(&c5, time_around(n)));
        }
        live.extend(moved(&e4, fourth));

        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, time_around(6));
        let live_wave = play(&mut live_engine, &live, fifth);
        assert_same_samples(&looped_wave[..fifth], &live_wave);

        // C5's place in the fifth time around is silent.
        let c5_place = &looped_wave[fifth + 20_000..fifth + 40_000];
        assert_eq!(peak(c5_place), 0.0);

        // E4 is as loud as played, give or take 2%.
        let e4_looped = peak(&looped_wave[fifth + 50_000..fifth + 60_000]);
        assert_within_percent(e4_looped, held_level(), 2.0);

        assert_eq!(volumes_of(&mut looper_engine), vec![0, FULL_VOLUME]);
    }

    #[test]
    fn live_notes_play_as_loud_as_played_whatever_a_layer_s_volume() {
        // The first loop is turned down to half before its A4 plays, and C5
        // is played live once A4 has faded, on the voice A4 had. By hand A4
        // plays at full volume, so the two only match once it has faded: C5
        // is as loud as played from its first sample.
        let first = LOOP_STARTS;
        let c5 = [(80_000, NoteOn(C5)), (90_000, NoteOff(C5))];
        let mut looped = base_looped();
        looped.push((first, Volume(1, 50)));
        looped.extend(moved(&c5, first));
        let mut live = base_by_hand(1);
        live.extend(moved(&c5, first));

        let mut looper_engine = engine_without_snap();
        let mut live_engine = Engine::new().unwrap();
        let looped_wave = play(&mut looper_engine, &looped, time_around(2));
        let live_wave = play(&mut live_engine, &live, time_around(2));

        // A4 at half, give or take 2%.
        let half = peak(&looped_wave[first + 20_000..first + 50_000]);
        let full = peak(&live_wave[first + 20_000..first + 50_000]);
        assert_within_percent(half, full / 2.0, 2.0);

        // A4 lets go 60,000 samples in and fades for 14,400.
        let faded = first + 75_000;
        assert_same_samples(&looped_wave[faded..], &live_wave[faded..]);
    }

    #[test]
    fn the_snap_switch_moves_every_layer_but_not_what_is_being_recorded() {
        // Layer 2 holds C5 from 41,000 to 50,000 samples in. Layer 3 is being
        // recorded, with E4 played from 38,000 to 42,000 this time around,
        // when snap goes on 44,000 samples in. Like closing the loop at a bar
        // line, the notes that should be sounding by then start again: A4,
        // snapped to 12,000 to 60,000, and C5, snapped to 36,000 to 48,000.
        // E4 only plays from the next time around, snapped to 36,000 to
        // 48,000 too.
        let second = time_around(2);
        let third = time_around(3);
        let c5 = [(41_000, NoteOn(C5)), (50_000, NoteOff(C5))];
        let e4 = [(38_000, NoteOn(E4)), (42_000, NoteOff(E4))];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&c5, second));
        looped.push((second + 70_000, PressLooper));
        looped.push((third, PressLooper));
        looped.extend(moved(&e4, third));
        looped.push((third + 44_000, Snap(true)));
        looped.push((third + 70_000, PressLooper));

        let mut live = base_by_hand(2);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        live.extend(moved(&c5, second));
        live.push((third, StartClick));
        live.push((third + 70_000, StopClick));
        live.push((third + 10_000, NoteOn(A4)));
        live.extend(moved(&e4, third));
        live.push((third + 41_000, NoteOn(C5)));
        live.push((third + 44_000, NoteOff(A4)));
        live.push((third + 44_000, NoteOff(C5)));
        live.push((third + 44_000, NoteOn(A4)));
        live.push((third + 44_000, NoteOn(C5)));
        live.push((third + 48_000, NoteOff(C5)));
        live.push((third + 60_000, NoteOff(A4)));
        let snapped = [
            (12_000, NoteOn(A4)),
            (36_000, NoteOn(C5)),
            (36_000, NoteOn(E4)),
            (48_000, NoteOff(C5)),
            (48_000, NoteOff(E4)),
            (60_000, NoteOff(A4)),
        ];
        for n in 4..=5 {
            live.extend(moved(&snapped, time_around(n)));
        }

        assert_layers_play(&looped, &live, time_around(6));
    }

    // Each layer's volume in Rust's copy, oldest first.
    fn volumes_of(engine: &mut Engine) -> Vec<u32> {
        let mut volumes = Vec::new();
        for layer in engine.loop_layers() {
            volumes.push(layer.volume);
        }
        volumes
    }

    #[test]
    fn removing_the_layer_being_recorded_stops_the_recording() {
        let second = time_around(2);
        let mut steps = base_looped();
        steps.push((second, PressLooper));
        steps.push((second + 20_000, NoteOn(C5)));
        steps.push((second + 30_000, RemoveNewest));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, second + 40_000);
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 1, false));
        assert_eq!(engine.loop_layers().len(), 1);

        // Nothing more is recorded.
        engine.note_on(E4, VELOCITY);
        render_frames_left(&mut engine, 10, usize::MAX);
        assert_eq!(engine.loop_layers()[0].notes, vec![ended(A4, 10_000, 60_000)]);

        // C5 is still held, but the layer it was recorded in is gone. So
        // once a new layer records G4, letting go of C5 leaves G4 held.
        engine.press_looper();
        render_frames_left(&mut engine, 10, usize::MAX);
        engine.note_on(G4, VELOCITY);
        render_frames_left(&mut engine, 10, usize::MAX);
        engine.note_off(C5);
        render_frames_left(&mut engine, 10, usize::MAX);
        let g4 = LoopNote {
            note: G4,
            start: 40_020,
            end: None,
        };
        assert_eq!(engine.loop_layers()[1].notes, vec![g4]);
    }

    #[test]
    fn a_note_held_while_an_earlier_layer_is_removed_keeps_its_whole_length() {
        // E4 goes down in the layer being recorded, and the first loop is
        // removed while it is held. The layer's notes move up into the gap,
        // and E4 still ends where its key is let go.
        let second = time_around(2);
        let e4 = [(20_000, NoteOn(E4)), (50_000, NoteOff(E4))];
        let mut looped = base_looped();
        looped.push((second, PressLooper));
        looped.extend(moved(&e4, second));
        looped.push((second + 30_000, RemoveLayer(1)));
        looped.push((second + 70_000, PressLooper));

        let mut live = base_by_hand(1);
        live.push((second, StartClick));
        live.push((second + 70_000, StopClick));
        live.push((second + 10_000, NoteOn(A4)));
        live.push((second + 30_000, NoteOff(A4)));
        for n in 2..=4 {
            live.extend(moved(&e4, time_around(n)));
        }

        let mut engine = assert_layers_play(&looped, &live, time_around(5));
        let layers = engine.loop_layers().to_vec();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].notes, vec![ended(E4, 20_000, 50_000)]);
    }

    #[test]
    fn a_note_still_held_as_the_next_layer_starts_ends_in_its_own_layer() {
        // G4 goes down in a layer, which is stopped, and the next one started,
        // while G4 is held. Letting go of it ends it in the layer it was
        // played in, and the new one stays empty.
        let second = time_around(2);
        let mut steps = base_looped();
        steps.push((second, PressLooper));
        steps.push((second + 60_000, NoteOn(G4)));
        steps.push((second + 70_000, PressLooper));
        steps.push((second + 75_000, PressLooper));
        steps.push((second + 90_000, NoteOff(G4)));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, second + 95_000);

        let layers = engine.loop_layers().to_vec();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[1].notes, vec![ended(G4, 60_000, 90_000)]);
        assert!(layers[2].notes.is_empty());
        assert!(layers[2].recording);
    }

    #[test]
    fn a_layer_stopped_with_no_notes_is_dropped_and_clears_the_loop_if_it_was_the_only_one() {
        let second = time_around(2);
        let mut steps = base_looped();
        // Started and stopped with nothing played: dropped.
        steps.push((second, PressLooper));
        steps.push((second + 10_000, PressLooper));
        // Started again, and the first loop removed while it records.
        steps.push((second + 20_000, PressLooper));
        steps.push((second + 30_000, RemoveLayer(1)));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, second + 40_000);
        assert_eq!(engine.looper_status(), looper(LooperState::Layering, 1, false));
        assert_eq!(engine.take_looper_message(), Some(String::from("Nothing played, no layer added")));
        assert_eq!(engine.loop_layers().len(), 1);
        assert!(engine.loop_layers()[0].recording);

        // Stopped with nothing played, it was the only layer: the loop is
        // cleared.
        engine.press_looper();
        render_frames_left(&mut engine, 10, usize::MAX);
        assert_eq!(engine.looper_status(), looper(LooperState::Idle, 0, false));
        assert!(engine.loop_layers().is_empty());
    }

    #[test]
    fn a_loop_holds_at_most_8_layers() {
        // Each layer records one C5 from the start of a time around and
        // stops half a bar in.
        let mut steps = base_looped();
        for n in 2..=8 {
            let at = time_around(n);
            steps.push((at, PressLooper));
            steps.push((at + 10_000, NoteOn(C5)));
            steps.push((at + 20_000, NoteOff(C5)));
            steps.push((at + 50_000, PressLooper));
        }
        let ninth = time_around(9);
        steps.push((ninth, PressLooper));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, ninth + 10_000);
        assert_eq!(engine.loop_layers().len(), 8);
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 1, false));
        assert_eq!(engine.take_looper_message(), Some(String::from("8 layers is the most, Esc removes one")));

        // Once one goes, a new one can start. The eighth is the C API's
        // highest number.
        engine.remove_layer(8);
        engine.press_looper();
        render_frames_left(&mut engine, 10, usize::MAX);
        assert_eq!(engine.looper_status(), looper(LooperState::Layering, 1, false));
        assert_eq!(engine.loop_layers().len(), 8);
    }

    #[test]
    fn rust_keeps_each_layer_s_notes_and_volume_as_layers_go() {
        // Three layers: the first loop's A4, then C5, then E4. Layer 2 is
        // turned down, then layer 1 removed: the others move up with their
        // notes and volumes.
        let second = time_around(2);
        let third = time_around(3);
        let mut steps = base_looped();
        steps.push((second, PressLooper));
        steps.push((second + 20_000, NoteOn(C5)));
        steps.push((second + 40_000, NoteOff(C5)));
        steps.push((second + 50_000, PressLooper));
        steps.push((third, PressLooper));
        steps.push((third + 50_000, NoteOn(E4)));
        steps.push((third + 60_000, NoteOff(E4)));
        steps.push((third + 70_000, PressLooper));
        steps.push((third + 80_000, Volume(2, 30)));
        steps.push((third + 90_000, RemoveLayer(1)));
        let mut engine = engine_without_snap();
        play(&mut engine, &steps, time_around(4));

        let c5_layer = LoopLayer {
            notes: vec![ended(C5, 20_000, 40_000)],
            volume: 30,
            recording: false,
        };
        let e4_layer = LoopLayer {
            notes: vec![ended(E4, 50_000, 60_000)],
            volume: FULL_VOLUME,
            recording: false,
        };
        assert_eq!(engine.loop_layers().to_vec(), vec![c5_layer, e4_layer]);
    }

    #[test]
    fn a_layer_s_volume_turns_its_notes_down_without_a_click() {
        // The first loop holds A4 from beat 1 to 90,000 samples in. The
        // first time around, it is turned down to half 40,000 samples in,
        // the next time around, to nothing, and the time after, back to
        // full.
        let loop_a4 = [(0, NoteOn(A4)), (90_000, NoteOff(A4))];
        let mut steps = vec![(0, PressLooper), (RECORDING_STARTS + 80_000, PressLooper)];
        steps.extend(moved(&loop_a4, RECORDING_STARTS));
        steps.push((LOOP_STARTS + 40_000, Volume(1, 50)));
        steps.push((time_around(2), Volume(1, 0)));
        steps.push((time_around(3), Volume(1, FULL_VOLUME)));
        let mut engine = engine_without_snap();
        let wave = play(&mut engine, &steps, time_around(4));

        // A wave's samples can fall a little either side of its peak, so
        // these are give or take 2%.
        let first = LOOP_STARTS;
        let full = peak(&wave[first + 20_000..first + 40_000]);
        let half = peak(&wave[first + 41_000..first + 80_000]);
        assert_within_percent(full, held_level(), 2.0);
        assert_within_percent(half, full / 2.0, 2.0);

        // It eases there over 10 ms, so no sample moves much further from
        // the one before than the wave itself does at full volume. Halved at
        // once, A4 would jump by half its height, 13 times as far.
        let steepest = |samples: &[f32]| {
            let mut most: f32 = 0.0;
            for pair in samples.windows(2) {
                most = most.max((pair[1] - pair[0]).abs());
            }
            most
        };
        let steady = steepest(&wave[first + 20_000..first + 40_000]);
        let turned = steepest(&wave[first + 39_000..first + 42_000]);
        assert!(turned <= steady * 1.5, "turned: {turned}, steady: {steady}");

        // Turned all the way down, A4 still plays, silently.
        let second = time_around(2);
        assert_eq!(peak(&wave[second + 1_000..second + 90_000]), 0.0);

        // And turned back up, as loud as played.
        let third = time_around(3);
        assert_within_percent(peak(&wave[third + 20_000..third + 40_000]), held_level(), 2.0);
    }

    fn ended(note: u8, start: u64, end: u64) -> LoopNote {
        LoopNote {
            note,
            start,
            end: Some(end),
        }
    }

    #[test]
    fn rust_asks_the_engine_where_the_loop_plays_its_notes() {
        let played = [
            ended(A4, 13_000, 29_000),
            ended(E4, 25_000, 26_000),
            ended(G4, 61_000, 70_000),
            ended(C5, 91_000, 94_000),
        ];

        // Snapped, C5 moves round to beat 1 and plays first, and E4 lasts
        // half a beat.
        let mut engine = Engine::new().unwrap();
        let snapped = vec![
            ended(C5, 0, 12_000),
            ended(A4, 12_000, 24_000),
            ended(E4, 24_000, 36_000),
            ended(G4, 60_000, 72_000),
        ];
        assert_eq!(engine.plan_loop(&played, 1), snapped);

        // A note still held counts as ending where it starts, so snapped it
        // lasts half a beat.
        let held = [LoopNote {
            note: A4,
            start: 13_000,
            end: None,
        }];
        assert_eq!(engine.plan_loop(&held, 1), vec![ended(A4, 12_000, 24_000)]);

        // A loop of no bars, a note outside MIDI's range or more notes than
        // a loop holds can't be worked out, so the notes come back as they
        // are.
        assert_eq!(engine.plan_loop(&played, 0), played.to_vec());
        let too_high = [ended(200, 13_000, 29_000)];
        assert_eq!(engine.plan_loop(&too_high, 1), too_high.to_vec());
        let mut too_many = Vec::new();
        for i in 0..LOOP_NOTES as u64 + 1 {
            too_many.push(ended(A4, 1_000 + i, 2_000 + i));
        }
        assert_eq!(engine.plan_loop(&too_many, 1), too_many);

        // With snap off, they play as played, and a note still held lasts
        // no time.
        engine.set_snap(false);
        assert_eq!(engine.plan_loop(&played, 1), played.to_vec());
        assert_eq!(engine.plan_loop(&held, 1), vec![ended(A4, 13_000, 13_000)]);

        // At 115 a half beat is 12,521.74 samples, and each one starts on
        // the whole sample before: half beat 1 on 12,521, and 2 on 25,043.
        let mut engine = Engine::new().unwrap();
        engine.set_tempo(115);
        assert_eq!(engine.plan_loop(&played[..1], 1), vec![ended(A4, 12_521, 25_043)]);
    }

    #[test]
    fn the_plan_never_depends_on_the_order_notes_were_played_in() {
        // A layer recorded round and round can hold notes that snap to the
        // same half beat, played in any order. Notes that start together go
        // from the one that ends first, then from the lowest. Of two A4s,
        // the shorter is cut off where the longer starts, so the longer is
        // heard.
        let long_a4 = ended(A4, 13_000, 40_000);
        let short_a4 = ended(A4, 11_000, 20_000);
        let e4 = ended(E4, 12_500, 20_000);
        let planned = vec![ended(E4, 12_000, 24_000), ended(A4, 12_000, 12_000), ended(A4, 12_000, 36_000)];

        let engine = Engine::new().unwrap();
        let orders = [
            [long_a4, short_a4, e4],
            [long_a4, e4, short_a4],
            [short_a4, long_a4, e4],
            [short_a4, e4, long_a4],
            [e4, long_a4, short_a4],
            [e4, short_a4, long_a4],
        ];
        for order in orders {
            assert_eq!(engine.plan_loop(&order, 1), planned);
        }
    }

    // Compose's song (docs/adr/0022-compose-as-a-tape.md). Like the looper's
    // tests, most of these play the same music twice: once as a song, and
    // once by hand, with every chord's notes played live on the exact
    // samples the song should play them. Every sample must match. That shows
    // each chord starts and lets go on exactly the right sample, as loud and
    // on the same wave as live, however the sound is asked for.

    // How many samples the engine makes in a millisecond.
    const SAMPLES_PER_MS: usize = 48;

    // How many samples a note takes to fade out once let go: 300 ms.
    const RELEASE_SAMPLES: usize = RELEASE_MS as usize * SAMPLES_PER_MS;

    const E3: u8 = 52;

    // Chords by their notes. Am shares C3 and E3 with C.
    const C_CHORD: &[u8] = &[C3, E3, 55]; // C3 E3 G3
    const AM_CHORD: &[u8] = &[45, C3, E3]; // A2 C3 E3
    const F_CHORD: &[u8] = &[41, 45, C3]; // F2 A2 C3
    const G_CHORD: &[u8] = &[43, 47, 50]; // G2 B2 D3

    // A chord of a song in these tests: its notes and how long it was held.
    #[derive(Clone, Copy)]
    struct TestChord {
        notes: &'static [u8],
        ms: u64,
    }

    const fn chord(notes: &'static [u8], ms: u64) -> TestChord {
        TestChord { notes, ms }
    }

    // Four chords whose ends fall partway into 10 ms chunks, the size the
    // speakers ask for.
    const SONG: &[TestChord] = &[
        chord(C_CHORD, 1001),
        chord(AM_CHORD, 753),
        chord(F_CHORD, 1499),
        chord(G_CHORD, 667),
    ];

    // A test song as the engine takes it, every chord as hard as VELOCITY.
    fn song_chords(song: &[TestChord]) -> Vec<SongChord> {
        let mut chords = Vec::new();
        for test_chord in song {
            chords.push(SongChord {
                notes: test_chord.notes.to_vec(),
                velocity: VELOCITY,
                length_ms: test_chord.ms,
            });
        }
        chords
    }

    // How many samples `song` lasts.
    fn song_samples(song: &[TestChord]) -> usize {
        let mut samples = 0;
        for test_chord in song {
            samples += test_chord.ms as usize * SAMPLES_PER_MS;
        }
        samples
    }

    // `song` played by hand from sample `at`: on the sample each chord
    // ends, its notes are let go, then the next chord's notes start.
    fn by_hand(song: &[TestChord], at: usize) -> Vec<(usize, Step)> {
        let mut steps = Vec::new();
        let mut start = at;
        for test_chord in song {
            for note in test_chord.notes {
                steps.push((start, NoteOn(*note)));
            }
            let end = start + test_chord.ms as usize * SAMPLES_PER_MS;
            for note in test_chord.notes {
                steps.push((end, NoteOff(*note)));
            }
            start = end;
        }
        steps
    }

    // `steps` without the ones on sample `at` or after it.
    fn before(steps: &[(usize, Step)], at: usize) -> Vec<(usize, Step)> {
        let mut kept = Vec::new();
        for (step_at, step) in steps {
            if *step_at < at {
                kept.push((*step_at, *step));
            }
        }
        kept
    }

    fn place(chord: usize, ms_in: u64) -> Option<SongPlace> {
        Some(SongPlace { chord, ms_in })
    }

    #[test]
    fn a_song_plays_each_chord_for_its_length_like_playing_it_by_hand() {
        // Started 1,000 samples in, and rendered in one go, in 10 ms chunks
        // and in odd-sized ones, with time left at the end for G to fade.
        let frames = 1000 + song_samples(SONG) + RELEASE_SAMPLES + 1000;
        let mut live_engine = Engine::new().unwrap();
        let live_wave = play(&mut live_engine, &by_hand(SONG, 1000), frames);
        assert!(peak(&live_wave) > 0.0);

        for chunk in [usize::MAX, 480, 337] {
            let mut song_engine = Engine::new().unwrap();
            let song_wave = play_in_chunks(&mut song_engine, &[(1000, PlaySong(SONG))], frames, chunk);
            assert_same_samples(&song_wave, &live_wave);
        }
    }

    #[test]
    fn a_chord_held_2000_ms_lets_go_on_sample_96000() {
        // 48 samples a millisecond.
        let mut song_engine = Engine::new().unwrap();
        song_engine.play_song(&song_chords(&[chord(C_CHORD, 2000)])).unwrap();
        let song_wave = play(&mut song_engine, &[], 120_000);

        let mut live = Vec::new();
        for note in C_CHORD {
            live.push((0, NoteOn(*note)));
            live.push((96_000, NoteOff(*note)));
        }
        let mut live_engine = Engine::new().unwrap();
        let live_wave = play(&mut live_engine, &live, 120_000);
        assert_same_samples(&song_wave, &live_wave);
    }

    #[test]
    fn chords_that_share_notes_play_them_again() {
        // C into Am into C: C3 and E3 let go and start again each time, the
        // way they do when one chord key is let go just as the next is
        // pressed.
        const SHARING: &[TestChord] = &[chord(C_CHORD, 500), chord(AM_CHORD, 500), chord(C_CHORD, 500)];
        let frames = song_samples(SHARING) + RELEASE_SAMPLES;
        let mut song_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let song_wave = play_in_chunks(&mut song_engine, &[(0, PlaySong(SHARING))], frames, 480);
        let live_wave = play(&mut live_engine, &by_hand(SHARING, 0), frames);
        assert_same_samples(&song_wave, &live_wave);
    }

    #[test]
    fn a_chord_0_ms_long_starts_and_lets_go_on_the_same_sample() {
        // A chord added the moment it was pressed has no length yet: first,
        // in the middle and last.
        const WITH_EMPTY: &[TestChord] = &[
            chord(G_CHORD, 0),
            chord(C_CHORD, 500),
            chord(F_CHORD, 0),
            chord(AM_CHORD, 500),
            chord(G_CHORD, 0),
        ];
        let frames = song_samples(WITH_EMPTY) + RELEASE_SAMPLES;
        let mut song_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let song_wave = play_in_chunks(&mut song_engine, &[(0, PlaySong(WITH_EMPTY))], frames, 480);
        let live_wave = play(&mut live_engine, &by_hand(WITH_EMPTY, 0), frames);
        assert_same_samples(&song_wave, &live_wave);
    }

    #[test]
    fn stopping_lets_go_of_the_chord_playing() {
        // Stopped 200 ms into Am: Am fades out the way it does when its key
        // is let go, and F never plays.
        let stop = 1000 + (1001 + 200) * SAMPLES_PER_MS;
        let mut live = before(&by_hand(SONG, 1000), stop);
        live.push((stop, AllNotesOff));
        let song = [(1000, PlaySong(SONG)), (stop, StopSong)];

        let frames = 1000 + song_samples(SONG) + RELEASE_SAMPLES;
        let mut song_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let song_wave = play_in_chunks(&mut song_engine, &song, frames, 480);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&song_wave, &live_wave);
        assert_eq!(song_engine.song_place(), None);
    }

    #[test]
    fn playing_again_starts_over() {
        // Played again 200 ms into Am: Am lets go, and C starts again right
        // there.
        let again = 1000 + (1001 + 200) * SAMPLES_PER_MS;
        let mut live = before(&by_hand(SONG, 1000), again);
        live.push((again, AllNotesOff));
        live.extend(by_hand(SONG, again));
        let song = [(1000, PlaySong(SONG)), (again, PlaySong(SONG))];

        let frames = again + song_samples(SONG) + RELEASE_SAMPLES;
        let mut song_engine = Engine::new().unwrap();
        let mut live_engine = Engine::new().unwrap();
        let song_wave = play_in_chunks(&mut song_engine, &song, frames, 480);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&song_wave, &live_wave);
    }

    #[test]
    fn live_notes_and_song_notes_never_cut_each_other_off() {
        const HELD_C: &[TestChord] = &[chord(C_CHORD, 2000)];

        // A live E3 played over the song's C chord, which has an E3 too, and
        // let go while the chord plays on. Once the live E3 has faded, the
        // song sounds exactly as if it had played alone.
        let mut alone_engine = Engine::new().unwrap();
        let alone = play(&mut alone_engine, &[(0, PlaySong(HELD_C))], 120_000);
        let with_live_steps = [(0, PlaySong(HELD_C)), (10_000, NoteOn(E3)), (30_000, NoteOff(E3))];
        let mut with_live_engine = Engine::new().unwrap();
        let with_live = play(&mut with_live_engine, &with_live_steps, 120_000);
        assert!(with_live[10_000..30_000] != alone[10_000..30_000]);
        let faded = 30_000 + RELEASE_SAMPLES;
        assert_same_samples(&with_live[faded..], &alone[faded..]);

        // The other way around: a live E3 held while the song's C chord
        // plays and lets go. Once the chord has faded, the live E3 sounds
        // exactly as if it had played alone.
        let mut live_alone_engine = Engine::new().unwrap();
        let live_alone = play(&mut live_alone_engine, &[(0, NoteOn(E3))], 200_000);
        let with_song_steps = [(0, NoteOn(E3)), (10_000, PlaySong(HELD_C))];
        let mut with_song_engine = Engine::new().unwrap();
        let with_song = play(&mut with_song_engine, &with_song_steps, 200_000);
        let song_faded = 10_000 + 96_000 + RELEASE_SAMPLES;
        assert_same_samples(&with_song[song_faded..], &live_alone[song_faded..]);
        assert!(peak(&with_song[song_faded..]) > 0.0);
    }

    #[test]
    fn the_looper_never_records_the_song() {
        // The song's C chord plays while the looper records its first bar.
        // The loop closes at the end of that bar, and comes out silent once
        // the chord has faded.
        const HELD_C: &[TestChord] = &[chord(C_CHORD, 2000)];
        let start = RECORDING_STARTS;
        let steps = [
            (0, PressLooper),
            (start + 1000, PlaySong(HELD_C)),
            (start + 80_000, PressLooper),
        ];
        let mut engine = Engine::new().unwrap();
        let wave = play(&mut engine, &steps, start + 3 * BAR_AT_120);
        assert_eq!(engine.looper_status(), looper(LooperState::Looping, 1, false));

        let faded = start + 1000 + 96_000 + RELEASE_SAMPLES;
        assert!(peak(&wave[start..faded]) > 0.0);
        assert_eq!(peak(&wave[faded..]), 0.0);
    }

    #[test]
    fn esc_and_letting_go_of_every_key_leave_the_song_playing() {
        // Esc clears the loop, and losing focus lets go of every live note,
        // but neither touches the song.
        let frames = song_samples(SONG) + RELEASE_SAMPLES;
        let mut alone_engine = Engine::new().unwrap();
        let alone = play(&mut alone_engine, &[(0, PlaySong(SONG))], frames);
        let steps = [(0, PlaySong(SONG)), (30_000, ClearLoop), (60_000, AllNotesOff)];
        let mut engine = Engine::new().unwrap();
        let wave = play(&mut engine, &steps, frames);
        assert_same_samples(&wave, &alone);
    }

    #[test]
    fn the_engine_says_where_the_song_is() {
        let mut engine = Engine::new().unwrap();
        assert_eq!(engine.song_place(), None);

        // On its way to the engine, the song counts as at its start. Then
        // it moves along with each 10 ms chunk.
        let song = [chord(C_CHORD, 1000), chord(G_CHORD, 0), chord(AM_CHORD, 500)];
        engine.play_song(&song_chords(&song)).unwrap();
        assert_eq!(engine.song_place(), place(0, 0));
        render_frames_left(&mut engine, 250 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), place(0, 250));

        // G is 0 ms long, so it never shows: C goes straight on to Am.
        render_frames_left(&mut engine, 750 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), place(2, 0));
        render_frames_left(&mut engine, 490 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), place(2, 490));

        // Played to its end, the song is over.
        render_frames_left(&mut engine, 10 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), None);

        // A stop still on its way already counts as stopped.
        engine.play_song(&song_chords(&song)).unwrap();
        render_frames_left(&mut engine, 100 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), place(0, 100));
        engine.stop_song();
        assert_eq!(engine.song_place(), None);
        render_frames_left(&mut engine, 10 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), None);
    }

    // The chords of a song as long as the engine plays, 1 ms each.
    fn longest_song() -> Vec<TestChord> {
        let chords = [C_CHORD, AM_CHORD, F_CHORD, G_CHORD];
        let mut song = Vec::new();
        for i in 0..SONG_MAX_CHORDS {
            song.push(chord(chords[i % 4], 1));
        }
        song
    }

    #[test]
    fn a_song_plays_512_chords_and_no_more() {
        let song = longest_song();
        let frames = song_samples(&song) + RELEASE_SAMPLES;
        let mut engine = Engine::new().unwrap();

        // A 513th is refused, with the mailbox empty, so the refusal is for
        // the number of chords and not for want of room.
        let mut too_long = song.clone();
        too_long.push(chord(C_CHORD, 1));
        assert!(engine.play_song(&song_chords(&too_long)).is_err());

        // Asked directly, the engine refuses it too.
        let mut raw_chords = Vec::new();
        for test_chord in song_chords(&too_long) {
            raw_chords.push(engine.raw_song_chord(&test_chord).unwrap());
        }
        // SAFETY: `raw` points to a live engine until Drop, and `raw_chords`
        // holds exactly 513 chords.
        let played = unsafe { jello_engine_play_song(engine.raw, raw_chords.as_ptr(), 513) };
        assert!(!played);

        // 512 play, the same as playing them by hand.
        engine.play_song(&song_chords(&song)).unwrap();
        let wave = play(&mut engine, &[], frames);
        let mut live_engine = Engine::new().unwrap();
        let live_wave = play(&mut live_engine, &by_hand(&song, 0), frames);
        assert_same_samples(&wave, &live_wave);
    }

    #[test]
    fn a_song_with_no_room_in_the_mailbox_is_refused_whole() {
        // A 512-chord song takes 514 of the mailbox's 1,024 slots: a stop,
        // its chords and a play. That leaves 510, so before the engine has
        // taken it, a song of 509 chords doesn't fit and none of it is sent,
        // but one of 508 fits exactly.
        let first = longest_song();
        let mut engine = Engine::new().unwrap();
        engine.play_song(&song_chords(&first)).unwrap();
        let too_big = song_chords(&[chord(F_CHORD, 1); 509]);
        assert!(engine.play_song(&too_big).is_err());
        let just_fits = vec![chord(G_CHORD, 1); 508];
        engine.play_song(&song_chords(&just_fits)).unwrap();

        // The engine takes them all at once, so the last song plays, and
        // the engine says where in it it is.
        let frames = song_samples(&just_fits) + RELEASE_SAMPLES;
        let first_part = play(&mut engine, &[], 100 * SAMPLES_PER_MS);
        assert_eq!(engine.song_place(), place(100, 0));
        let rest = play(&mut engine, &[], frames - first_part.len());
        let mut wave = first_part;
        wave.extend(rest);

        let mut live_engine = Engine::new().unwrap();
        let live_wave = play(&mut live_engine, &by_hand(&just_fits, 0), frames);
        assert_same_samples(&wave, &live_wave);

        // Once the engine has taken them, there is room again.
        assert!(engine.play_song(&too_big).is_ok());
    }

    #[test]
    fn a_refused_song_leaves_the_playhead_on_the_song_playing() {
        // The refused song's chords are 2 ms long and the playing song's
        // 1 ms, so measuring with the wrong lengths puts the playhead on
        // chord 50 instead of 100.
        let playing = longest_song();
        let mut engine = Engine::new().unwrap();
        engine.play_song(&song_chords(&playing)).unwrap();
        let refused = song_chords(&[chord(F_CHORD, 2); 509]);
        assert!(engine.play_song(&refused).is_err());

        render_frames_left(&mut engine, 100 * SAMPLES_PER_MS, 480);
        assert_eq!(engine.song_place(), place(100, 0));
    }

    #[test]
    fn a_song_with_an_odd_chord_plays_nothing() {
        let good = SongChord {
            notes: vec![MIDDLE_C],
            velocity: VELOCITY,
            length_ms: 100,
        };
        let odd_chords = [
            SongChord {
                notes: vec![128],
                ..good.clone()
            },
            SongChord {
                velocity: 0,
                ..good.clone()
            },
            SongChord {
                velocity: 128,
                ..good.clone()
            },
            SongChord {
                notes: vec![60, 64, 67, 70, 72],
                ..good.clone()
            },
        ];

        let mut engine = Engine::new().unwrap();
        for odd in odd_chords {
            let song = [good.clone(), odd];
            assert!(engine.play_song(&song).is_err());
        }

        // Asked directly, the engine refuses chords with more notes than it
        // has room for, or fewer than none.
        for note_count in [5, -1] {
            let mut raw = engine.raw_song_chord(&good).unwrap();
            raw.note_count = note_count;
            // SAFETY: `raw` points to a live engine until Drop, and `raw` is
            // one chord.
            let played = unsafe { jello_engine_play_song(engine.raw, &raw, 1) };
            assert!(!played, "a chord of {note_count} notes was played");
        }

        assert_eq!(engine.song_place(), None);
        let wave = render_left(&mut engine, 200);
        assert_eq!(peak(&wave), 0.0);
    }

    // The low-pass filter (docs/adr/0028-a-low-pass-filter.md). Every test
    // above plays with it open, as a new engine starts, and hears exactly
    // the sound the engine made before it had a filter.

    const GLIDE_MS: u32 = 20; // `glide_ms` in engine/src/filter.cpp
    const LOWEST_Q: f64 = 0.7071; // `lowest_q` in engine/src/filter.cpp
    const HIGHEST_Q: f64 = 10.0; // `highest_q` in engine/src/filter.cpp

    const A3: u8 = 57;
    const A5: u8 = 81;

    // A press soft enough that even a full resonance stays under the soft
    // clip, which would bend the sound being measured.
    const SOFT: u8 = 64;

    // The cutoff for middle C that puts A4's at 880 Hz, its second harmonic.
    // A4 is 9 half steps above middle C, and 880 Hz is 9 half steps above C5.
    const CUTOFF_FOR_A4_AT_880: f32 = C5_HZ as f32;

    fn filter(cutoff_hz: f32, resonance: u32) -> Filter {
        Filter { cutoff_hz, resonance }
    }

    fn filter_envelope(amount: f32, attack_ms: u32, decay_ms: u32, sustain: u32, release_ms: u32) -> FilterEnvelope {
        FilterEnvelope {
            amount,
            attack_ms,
            decay_ms,
            sustain,
            release_ms,
        }
    }

    // A new engine playing a saw, which has every harmonic, so it shows the
    // filter's whole shape.
    fn saw_engine() -> Engine {
        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Saw);
        engine
    }

    // The biggest difference between two sounds, sample by sample.
    fn biggest_difference(actual: &[f32], expected: &[f32]) -> f32 {
        assert_eq!(actual.len(), expected.len());
        let mut biggest = 0.0;
        for i in 0..actual.len() {
            let difference = (actual[i] - expected[i]).abs();
            if difference > biggest {
                biggest = difference;
            }
        }
        biggest
    }

    // Checks that two sounds differ by at most `allowed` at every sample.
    fn assert_nearly_same_sound(actual: &[f32], expected: &[f32], allowed: f32) {
        let difference = biggest_difference(actual, expected);
        assert!(
            difference <= allowed,
            "the sounds differ by {difference}, more than {allowed}"
        );
    }

    // Checks that every sample is a number. A filter whose math blows up
    // makes infinity or NaN, and peak() can't see NaN.
    fn assert_all_finite(samples: &[f32]) {
        for i in 0..samples.len() {
            assert!(samples[i].is_finite(), "sample {i} is {}", samples[i]);
        }
    }

    // How much a low-pass with its cutoff at `cutoff_hz` and sharpness `q`
    // lets through at `hz`: 1 is all of it. This is the analog filter's math,
    // with its pitches bent for samples the way the engine's filter bends
    // them, so the engine must match it almost exactly.
    fn low_pass_gain(engine: &Engine, hz: f64, cutoff_hz: f64, q: f64) -> f64 {
        let rate = engine.sample_rate() as f64;
        let ratio = (PI * hz / rate).tan() / (PI * cutoff_hz / rate).tan();
        let below = 1.0 - ratio * ratio;
        let damping = ratio / q;
        1.0 / (below * below + damping * damping).sqrt()
    }

    // How much of each harmonic of a held saw, from the first to `count`,
    // gets through `filter`: its size held through the filter, next to its
    // size held with the filter open. `hz` is the note's pitch.
    fn harmonics_let_through(note: u8, hz: f64, velocity: u8, filter: Filter, count: usize) -> Vec<f64> {
        let mut open = saw_engine();
        let dry = hold(&mut open, note, velocity);

        let mut filtered = saw_engine();
        filtered.set_filter(filter);
        let wet = hold(&mut filtered, note, velocity);
        // Past the soft clip, the measurement would be bent too.
        assert!(peak(&wet) < CLIP_START);

        let mut through = Vec::new();
        for harmonic in 1..=count {
            let harmonic_hz = hz * harmonic as f64;
            let size = tone_size(&open, &wet, harmonic_hz) / tone_size(&open, &dry, harmonic_hz);
            through.push(size);
        }
        through
    }

    #[test]
    fn an_open_filter_leaves_the_sound_exactly_as_it_was() {
        // A held saw: the filter closes partway, then opens again. Once the
        // glide is over, every sample is the same as on an engine whose
        // filter was never touched.
        let mut touched = saw_engine();
        let mut untouched = saw_engine();
        touched.note_on(A4, VELOCITY);
        untouched.note_on(A4, VELOCITY);
        render_left(&mut touched, 100);
        render_left(&mut untouched, 100);

        touched.set_filter(filter(1000.0, 50));
        let closed = render_left(&mut touched, 100);
        let plain = render_left(&mut untouched, 100);
        assert!(biggest_difference(&closed, &plain) > 0.01);

        touched.set_filter(START_FILTER);
        render_left(&mut touched, GLIDE_MS);
        render_left(&mut untouched, GLIDE_MS);
        let opened = render_left(&mut touched, 500);
        let plain = render_left(&mut untouched, 500);
        assert!(opened == plain, "the opened filter still changed the sound");
    }

    #[test]
    fn a_low_cutoff_turns_harmonics_down_12_db_an_octave() {
        // A4's cutoff at 880 Hz, its second harmonic. Each harmonic comes
        // through as much as the filter's math says, give or take 1%: the
        // first, an octave below the cutoff, nearly whole, the second at
        // 0.71, then less and less.
        let engine = Engine::new().unwrap();
        let through = harmonics_let_through(A4, A4_HZ, VELOCITY, filter(CUTOFF_FOR_A4_AT_880, 0), 8);
        for i in 0..through.len() {
            let harmonic = i + 1;
            let expected = low_pass_gain(&engine, A4_HZ * harmonic as f64, 880.0, LOWEST_Q);
            let difference = (through[i] - expected).abs();
            assert!(
                difference <= expected * 0.01,
                "harmonic {harmonic} came through at {}, expected {expected}",
                through[i]
            );
        }

        // Well above the cutoff, each octave up lets through about a quarter
        // as much: 12 dB quieter. Harmonic 8 is an octave above harmonic 4.
        let octave_up = through[7] / through[3];
        assert!(
            (octave_up - 0.25).abs() <= 0.01,
            "an octave up let through {octave_up} as much"
        );
    }

    #[test]
    fn every_note_is_equally_bright() {
        // The cutoff follows the note, an octave up for each octave up, so a
        // cutoff that keeps low chords warm doesn't mute the melody above
        // them. A3, A4 and A5 each get their cutoff on their second harmonic,
        // so each lets through 0.71 of it, give or take 1%, and the same share
        // of its fourth harmonic as A4, give or take 3%.
        let setting = filter(CUTOFF_FOR_A4_AT_880, 0);
        let a4 = harmonics_let_through(A4, A4_HZ, VELOCITY, setting, 4);
        for (note, hz) in [(A3, A4_HZ / 2.0), (A4, A4_HZ), (A5, A4_HZ * 2.0)] {
            let through = harmonics_let_through(note, hz, VELOCITY, setting, 4);
            assert!(
                (through[1] - LOWEST_Q).abs() <= LOWEST_Q * 0.01,
                "note {note} let through {} of its second harmonic",
                through[1]
            );
            assert!(
                (through[3] - a4[3]).abs() <= a4[3] * 0.03,
                "note {note} let through {} of its fourth harmonic, A4 {}",
                through[3],
                a4[3]
            );
        }
    }

    #[test]
    fn a_high_note_s_cutoff_lands_right_on_its_pitch() {
        // A filter made of samples crowds its pitches together toward the
        // top, so the engine bends every cutoff up to make up for it. A7
        // gets its cutoff on its second harmonic, 7040 Hz, and lets through
        // 0.71 of it, give or take 1%, like A3, A4 and A5 above. Unbent, the
        // cutoff would land about 6% low, near 6600 Hz, and let through 0.65.
        let setting = filter(CUTOFF_FOR_A4_AT_880, 0);
        let through = harmonics_let_through(A7, A7_HZ, VELOCITY, setting, 2);
        assert!(
            (through[1] - LOWEST_Q).abs() <= LOWEST_Q * 0.01,
            "A7 let through {} of its second harmonic",
            through[1]
        );
    }

    #[test]
    fn resonance_boosts_the_harmonic_at_the_cutoff() {
        // Right at the cutoff, the filter lets through its Q: 0.71 with no
        // resonance, and 10 times as much as an open filter at full. The
        // resonance is squared on the way, so halfway is a quarter of the way
        // up: 0.71 + (10 - 0.71) / 4, about 3.03.
        let halfway = LOWEST_Q + (HIGHEST_Q - LOWEST_Q) / 4.0;
        for (resonance, q) in [(0, LOWEST_Q), (50, halfway), (100, HIGHEST_Q)] {
            let through = harmonics_let_through(A4, A4_HZ, SOFT, filter(CUTOFF_FOR_A4_AT_880, resonance), 2);
            assert!(
                (through[1] - q).abs() <= q * 0.01,
                "at {resonance}% the cutoff's harmonic came through at {}, expected {q}",
                through[1]
            );
        }
    }

    #[test]
    fn a_moved_cutoff_glides_there_in_20_ms_on_notes_already_sounding() {
        // A held saw, then the cutoff moves from open. The note already
        // sounding hears it: next to a note that started with the cutoff
        // already there, it is still on its way 17 ms after the move, and
        // from 22 ms on it sounds the same: the glide has landed, and the
        // filter has settled.
        let mut moved = saw_engine();
        moved.note_on(A4, VELOCITY);
        render_left(&mut moved, 500);
        moved.set_filter(filter(CUTOFF_FOR_A4_AT_880, 0));
        let after_move = render_left(&mut moved, 40);

        let mut already = saw_engine();
        already.set_filter(filter(CUTOFF_FOR_A4_AT_880, 0));
        already.note_on(A4, VELOCITY);
        render_left(&mut already, 500);
        let steady = render_left(&mut already, 40);

        let at_17_ms = frames_in_ms(&moved, 17);
        let at_18_ms = frames_in_ms(&moved, 18);
        let on_its_way = biggest_difference(&after_move[at_17_ms..at_18_ms], &steady[at_17_ms..at_18_ms]);
        assert!(on_its_way > 0.001, "the glide was over too soon");

        let at_22_ms = frames_in_ms(&moved, 22);
        assert_nearly_same_sound(&after_move[at_22_ms..], &steady[at_22_ms..], 0.0001);
    }

    #[test]
    fn sweeping_the_cutoff_all_the_way_never_clicks() {
        // On a held triangle, the cutoff jumps from open to 20 Hz, where
        // little of A4 is left, then back to open. Each glides, so the sound
        // never jumps.
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        let mut wave = render_left(&mut engine, 100);

        engine.set_filter(filter(FILTER_LOWEST_HZ, 0));
        let closed = render_left(&mut engine, 100);
        let last_50_ms = &closed[closed.len() - frames_in_ms(&engine, 50)..];
        assert!(peak(last_50_ms) < held_level() / 50.0);
        wave.extend(closed);

        engine.set_filter(START_FILTER);
        wave.extend(render_left(&mut engine, 100));
        assert_no_click(&engine, &wave, A4_HZ);
    }

    #[test]
    fn full_resonance_rings_steady_and_never_runs_away() {
        // A held saw at full resonance, with A4's cutoff on its second
        // harmonic. Two seconds later it is exactly as loud, give or take 1%.
        let mut engine = saw_engine();
        engine.set_filter(filter(CUTOFF_FOR_A4_AT_880, 100));
        engine.note_on(A4, SOFT);
        render_left(&mut engine, ATTACK_MS + DECAY_MS);
        let early = render_left(&mut engine, 100);
        render_left(&mut engine, 2000);
        let late = render_left(&mut engine, 100);

        assert_all_finite(&late);
        assert_within_percent(peak(&late), peak(&early), 1.0);
    }

    #[test]
    fn nothing_clips_at_full_resonance() {
        // The loudest chord there is, 32 saws at the hardest press, through a
        // full resonance at a low cutoff.
        let mut engine = saw_engine();
        engine.set_filter(filter(200.0, 100));
        for note in C3..C3 + VOICE_COUNT {
            engine.note_on(note, 127);
        }

        let samples = engine.render(frames_in_ms(&engine, 1000));
        assert_all_finite(&samples);
        assert!(peak(&samples) < 1.0);
    }

    #[test]
    fn the_filter_s_edges_never_blow_up_and_letting_go_still_ends_in_silence() {
        // The lowest and highest notes, through the lowest cutoff and the one
        // just below open, with no resonance and full, while the filter
        // envelope opens all 8 octaves at once. The cutoffs reach from below
        // 1 Hz to past the highest the filter allows.
        for cutoff_hz in [FILTER_LOWEST_HZ, FILTER_OPEN_HZ - 1.0] {
            for resonance in [0, 100] {
                for note in [0, 127] {
                    let mut engine = saw_engine();
                    engine.set_filter(filter(cutoff_hz, resonance));
                    engine.set_filter_envelope(filter_envelope(FILTER_MOST_OCTAVES, 0, 100, 50, 100));
                    engine.note_on(note, 127);
                    let mut wave = render_left(&mut engine, 500);
                    engine.note_off(note);
                    wave.extend(render_left(&mut engine, RELEASE_MS));
                    let after = render_left(&mut engine, 100);

                    assert_all_finite(&wave);
                    assert!(peak(&wave) < 1.0);
                    assert_eq!(peak(&after), 0.0, "note {note} at {cutoff_hz} Hz never went silent");
                }
            }
        }
    }

    #[test]
    fn a_note_on_a_silent_voice_starts_right_at_its_own_cutoff() {
        // A5, with its cutoff on its second harmonic. Once its attack is
        // over, it already sounds exactly as it does half a second later:
        // it doesn't glide to its cutoff or fade into the filter. Half a
        // second is exactly 440 cycles of A5, so its wave lines up.
        let mut engine = saw_engine();
        engine.set_filter(filter(CUTOFF_FOR_A4_AT_880, 0));
        engine.note_on(A5, VELOCITY);
        let wave = render_left(&mut engine, 520);

        let start = between(&engine, &wave, ATTACK_MS + 1, ATTACK_MS + 13);
        let later = between(&engine, &wave, ATTACK_MS + 501, ATTACK_MS + 513);
        assert_nearly_same_sound(start, later, 0.0001);
    }

    #[test]
    fn a_new_note_takes_over_a_filtered_voice_without_a_click() {
        // As a_new_note_takes_over_the_oldest_held_one_without_a_click, with
        // the filter on and its envelope opening each note. The voice keeps
        // its filter's memory, and its cutoff glides from middle C's to C5's.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.set_filter(filter(1000.0, 0));
            engine.set_filter_envelope(filter_envelope(1.0, 5, 300, 50, 300));

            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            engine.note_on(MIDDLE_C, VELOCITY);
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            let mut wave = render_left(&mut engine, 500 + extra_ms);

            engine.note_on(C5, VELOCITY);
            let after = render_left(&mut engine, 1000);

            // While the cutoff moves, the filter holds the wave back by a
            // little more or less each moment, which shifts where it crosses
            // zero and reads as a pitch a few hundredths of a Hz off. By
            // 400 ms the envelope sits at its sustain and the cutoff is still.
            let settled = &after[frames_in_ms(&engine, 400)..];
            assert_pitch(&engine, settled, C5_HZ);

            wave.extend(after);
            assert_no_click(&engine, &wave, C5_HZ);
        }
    }

    #[test]
    fn the_filter_sounds_the_same_however_the_sound_is_asked_for() {
        // As the_same_notes_always_make_the_same_sound, with the filter, its
        // envelope and a move of the cutoff while the note is held.
        let mut whole = saw_engine();
        let mut chunked = saw_engine();
        for engine in [&mut whole, &mut chunked] {
            engine.set_filter(filter(500.0, 60));
            engine.set_filter_envelope(filter_envelope(3.0, 50, 200, 30, 400));
            engine.note_on(MIDDLE_C, VELOCITY);
        }
        let mut whole_wave = render_left(&mut whole, 300);
        let mut chunked_wave = render_in_chunks(&mut chunked, 300, 10);

        for engine in [&mut whole, &mut chunked] {
            engine.set_filter(filter(2000.0, 90));
        }
        whole_wave.extend(render_left(&mut whole, 300));
        chunked_wave.extend(render_in_chunks(&mut chunked, 300, 10));

        whole.note_off(MIDDLE_C);
        chunked.note_off(MIDDLE_C);
        whole_wave.extend(render_left(&mut whole, 400));
        chunked_wave.extend(render_in_chunks(&mut chunked, 400, 10));

        assert!(
            whole_wave == chunked_wave,
            "the two engines sounded different"
        );
    }

    #[test]
    fn a_filter_envelope_with_no_amount_changes_nothing() {
        // Its stages can be anything: at an amount of 0 it never moves the
        // cutoff, so every sample is the same as with the envelope untouched.
        let mut envelope = saw_engine();
        let mut plain = saw_engine();
        for engine in [&mut envelope, &mut plain] {
            engine.set_filter(filter(800.0, 40));
            engine.note_on(A4, VELOCITY);
        }
        envelope.set_filter_envelope(filter_envelope(0.0, 70, 900, 30, 2000));

        let mut envelope_wave = render_left(&mut envelope, 500);
        let mut plain_wave = render_left(&mut plain, 500);
        envelope.note_off(A4);
        plain.note_off(A4);
        envelope_wave.extend(render_left(&mut envelope, 400));
        plain_wave.extend(render_left(&mut plain, 400));
        assert!(envelope_wave == plain_wave, "an envelope of 0 changed the sound");
    }

    // A held A4 saw whose filter envelope opens its cutoff from 220 Hz, two
    // octaves below 880 Hz, by `envelope`.
    fn a4_opened_by(envelope: FilterEnvelope) -> Engine {
        let mut engine = saw_engine();
        engine.set_filter(filter(CUTOFF_FOR_A4_AT_880 / 4.0, 0));
        engine.set_filter_envelope(envelope);
        engine.note_on(A4, VELOCITY);
        engine
    }

    // A held A4 saw whose cutoff sits at `cutoff_for_a4` all along, with no
    // filter envelope.
    fn a4_through(cutoff_for_a4: f32) -> Engine {
        let mut engine = saw_engine();
        let middle_c_s = cutoff_for_a4 * CUTOFF_FOR_A4_AT_880 / 880.0;
        engine.set_filter(filter(middle_c_s, 0));
        engine.note_on(A4, VELOCITY);
        engine
    }

    // The part of `wave` from `from_ms` to `to_ms`.
    fn between<'a>(engine: &Engine, wave: &'a [f32], from_ms: u32, to_ms: u32) -> &'a [f32] {
        &wave[frames_in_ms(engine, from_ms)..frames_in_ms(engine, to_ms)]
    }

    #[test]
    fn the_filter_envelope_opens_the_cutoff_by_its_amount_over_its_attack() {
        // Two octaves over a 100 ms attack, then held there: a sustain of
        // 100%. Next to a note through 880 Hz all along, it is still opening
        // at 85 to 90 ms, and from 110 ms on it sounds the same.
        let mut opening = a4_opened_by(filter_envelope(2.0, 100, 300, 100, 300));
        let opened = render_left(&mut opening, 300);
        let mut open = a4_through(880.0);
        let all_along = render_left(&mut open, 300);

        let still_opening = biggest_difference(
            between(&open, &opened, 85, 90),
            between(&open, &all_along, 85, 90),
        );
        assert!(still_opening > 0.001, "it opened too soon");
        assert_nearly_same_sound(
            between(&open, &opened, 110, 300),
            between(&open, &all_along, 110, 300),
            0.0001,
        );
    }

    #[test]
    fn the_filter_envelope_falls_to_its_sustain_over_its_decay() {
        // Two octaves over a 50 ms attack, then down to a sustain of 50% over
        // a 200 ms decay: one octave up, at 440 Hz. It is still falling at
        // 200 to 230 ms, and from 270 ms on it sounds like 440 Hz all along.
        let mut falling = a4_opened_by(filter_envelope(2.0, 50, 200, 50, 300));
        let fell = render_left(&mut falling, 500);
        let mut sustain = a4_through(440.0);
        let all_along = render_left(&mut sustain, 500);

        let still_falling = biggest_difference(
            between(&sustain, &fell, 200, 230),
            between(&sustain, &all_along, 200, 230),
        );
        assert!(still_falling > 0.001, "it reached its sustain too soon");
        assert_nearly_same_sound(
            between(&sustain, &fell, 270, 500),
            between(&sustain, &all_along, 270, 500),
            0.0001,
        );
    }

    #[test]
    fn letting_go_closes_the_filter_envelope_over_its_release() {
        // Opened two octaves and held there, then let go with a 100 ms
        // release. It is still closing at 50 to 90 ms, and from 120 ms on it
        // sounds like the cutoff alone, though its loudness is still fading.
        let mut closing = a4_opened_by(filter_envelope(2.0, 5, 300, 100, 100));
        let mut cutoff_alone = a4_through(220.0);
        render_left(&mut closing, 500);
        render_left(&mut cutoff_alone, 500);
        closing.note_off(A4);
        cutoff_alone.note_off(A4);
        let closed = render_left(&mut closing, 250);
        let alone = render_left(&mut cutoff_alone, 250);

        let still_closing = biggest_difference(
            between(&closing, &closed, 50, 90),
            between(&closing, &alone, 50, 90),
        );
        assert!(still_closing > 0.001, "it closed too soon");
        assert_nearly_same_sound(
            between(&closing, &closed, 120, 250),
            between(&closing, &alone, 120, 250),
            0.0001,
        );
    }

    #[test]
    fn a_held_note_keeps_its_filter_envelope_and_the_next_note_gets_the_new_one() {
        // The envelope changes while A4 is held. A4 sounds exactly as if it
        // never changed. Once A4 has faded out, the next A4 sounds exactly as
        // if the new envelope had been there all along, though the old one,
        // with its 2 second release, was still letting go when A4 went
        // silent: a silent voice keeps nothing of its last note's filter.
        let old = filter_envelope(3.0, 5, 300, 50, 2000);
        let new = filter_envelope(1.5, 80, 100, 20, 300);

        let mut changed = saw_engine();
        let mut kept = saw_engine();
        let mut new_all_along = saw_engine();
        changed.set_filter_envelope(old);
        kept.set_filter_envelope(old);
        new_all_along.set_filter_envelope(new);
        for engine in [&mut changed, &mut kept, &mut new_all_along] {
            engine.set_filter(filter(300.0, 30));
            engine.note_on(A4, VELOCITY);
            render_left(engine, 100);
        }

        changed.set_filter_envelope(new);
        let changed_held = render_left(&mut changed, 500);
        let kept_held = render_left(&mut kept, 500);
        assert!(changed_held == kept_held, "the held note's envelope changed");

        render_left(&mut new_all_along, 500);
        for engine in [&mut changed, &mut new_all_along] {
            engine.note_off(A4);
            render_left(engine, RELEASE_MS + 100);
            engine.note_on(A4, VELOCITY);
        }
        let next = render_left(&mut changed, 500);
        let expected = render_left(&mut new_all_along, 500);
        assert!(next == expected, "the next note didn't get the new envelope");
    }

    // A4 on a saw through the filter at 1000 Hz and 50%, with its envelope
    // opening it 2 octaves, after `send` asks the engine for something
    // directly. The note is let go, so every stage is heard.
    fn filtered_a4_after(send: impl Fn(&Engine)) -> Vec<f32> {
        let mut engine = saw_engine();
        engine.set_filter(filter(1000.0, 50));
        engine.set_filter_envelope(filter_envelope(2.0, 5, 300, 50, 300));
        send(&engine);

        engine.note_on(A4, VELOCITY);
        let mut wave = render_left(&mut engine, 500);
        engine.note_off(A4);
        wave.extend(render_left(&mut engine, RELEASE_MS + 100));
        wave
    }

    #[test]
    fn odd_filter_values_change_nothing() {
        // Asked directly, the engine ignores values outside the filter's
        // ranges, and NaN. Each odd value goes to an engine of its own, so
        // one that got through can't hide behind the next.
        let plain = filtered_a4_after(|_| {});

        let odd_filters = [
            (f32::NAN, 0.5),
            (19.9, 0.5),
            (20000.1, 0.5),
            (1000.0, -0.1),
            (1000.0, 1.1),
            (1000.0, f32::NAN),
        ];
        for (cutoff_hz, resonance) in odd_filters {
            let odd = filtered_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_filter(engine.raw, cutoff_hz, resonance) };
            });
            assert!(odd == plain, "{cutoff_hz} Hz at {resonance} changed the sound");
        }
        let odd_envelopes = [
            (f32::NAN, 5, 300, 0.5, 300),
            (-0.1, 5, 300, 0.5, 300),
            (8.1, 5, 300, 0.5, 300),
            (2.0, -1, 300, 0.5, 300),
            (2.0, 10001, 300, 0.5, 300),
            (2.0, 5, -1, 0.5, 300),
            (2.0, 5, 10001, 0.5, 300),
            (2.0, 5, 300, -0.1, 300),
            (2.0, 5, 300, 1.1, 300),
            (2.0, 5, 300, f32::NAN, 300),
            (2.0, 5, 300, 0.5, -1),
            (2.0, 5, 300, 0.5, 10001),
        ];
        for (amount, attack, decay, sustain, release) in odd_envelopes {
            let odd = filtered_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_filter_envelope(engine.raw, amount, attack, decay, sustain, release) };
            });
            assert!(
                odd == plain,
                "the envelope {amount}, {attack}, {decay}, {sustain}, {release} changed the sound"
            );
        }
    }

    #[test]
    fn rust_brings_filter_values_within_their_ranges() {
        // A cutoff below the lowest is sent as the lowest, so it sounds the
        // same as the lowest. Everything else out of range stops at its edge.
        let mut below = saw_engine();
        let sent = below.set_filter(filter(5.0, 150));
        assert_eq!(sent, filter(FILTER_LOWEST_HZ, FULL_PERCENT));
        let mut lowest = saw_engine();
        lowest.set_filter(filter(FILTER_LOWEST_HZ, FULL_PERCENT));
        below.note_on(A4, VELOCITY);
        lowest.note_on(A4, VELOCITY);
        let below_wave = render_left(&mut below, 300);
        let lowest_wave = render_left(&mut lowest, 300);
        assert!(below_wave == lowest_wave, "the cutoff below the lowest was not sent as the lowest");

        let mut engine = Engine::new().unwrap();
        let sent = engine.set_filter(filter(50000.0, 0));
        assert_eq!(sent, filter(FILTER_OPEN_HZ, 0));
        let sent = engine.set_filter(filter(f32::NAN, 0));
        assert_eq!(sent, filter(FILTER_LOWEST_HZ, 0));

        let sent = engine.set_filter_envelope(filter_envelope(9.5, 20000, 20000, 150, 20000));
        let longest = FILTER_LONGEST_MS;
        let edges = filter_envelope(FILTER_MOST_OCTAVES, longest, longest, FULL_PERCENT, longest);
        assert_eq!(sent, edges);
        let sent = engine.set_filter_envelope(filter_envelope(-1.0, 0, 0, 0, 0));
        assert_eq!(sent, filter_envelope(0.0, 0, 0, 0, 0));
    }

    #[test]
    fn a_new_engine_starts_with_the_filter_open_and_no_envelope() {
        // As START_FILTER and START_FILTER_ENVELOPE say, so a new song's
        // sound profile, sent as the app starts, changes nothing.
        assert_eq!(START_FILTER, filter(FILTER_OPEN_HZ, 0));
        assert_eq!(START_FILTER_ENVELOPE, filter_envelope(0.0, 5, 300, 0, 300));

        let mut started = saw_engine();
        let mut sent = saw_engine();
        sent.set_filter(START_FILTER);
        sent.set_filter_envelope(START_FILTER_ENVELOPE);
        started.note_on(A4, VELOCITY);
        sent.note_on(A4, VELOCITY);
        let started_wave = render_left(&mut started, 300);
        let sent_wave = render_left(&mut sent, 300);
        assert!(started_wave == sent_wave, "a new engine's filter is not where the start values say");
    }

    // Unison (docs/adr/0029-unison.md). Every test above plays 1 copy, as a
    // new engine starts, and hears exactly the sound the engine made before
    // it had unison.

    fn unison(copies: u32, detune_cents: u32, spread: u32) -> Unison {
        Unison {
            copies,
            detune_cents,
            spread,
        }
    }

    // Renders `ms` milliseconds, both sides: left, right, left, ...
    fn render_stereo(engine: &mut Engine, ms: u32) -> Vec<f32> {
        let frames = frames_in_ms(engine, ms);
        engine.render(frames)
    }

    // Renders `ms` milliseconds and splits it into the left side and the
    // right.
    fn render_sides(engine: &mut Engine, ms: u32) -> (Vec<f32>, Vec<f32>) {
        let samples = render_stereo(engine, ms);
        let mut left = Vec::new();
        let mut right = Vec::new();
        for frame in samples.chunks(CHANNELS) {
            left.push(frame[0]);
            right.push(frame[1]);
        }
        (left, right)
    }

    // The pitch `cents` above `hz`, or below it when `cents` is below 0. A
    // cent is a hundredth of a half step, so 1200 make an octave.
    fn cents_above(hz: f64, cents: f64) -> f64 {
        hz * 2.0_f64.powf(cents / 1200.0)
    }

    // Where copy `index` of `copies` sits, from -1 (the lowest and furthest
    // left) to 1 (the highest and furthest right). Matches `place_among` in
    // engine/src/voice.cpp.
    fn place_among(index: u32, copies: u32) -> f64 {
        2.0 * index as f64 / (copies - 1) as f64 - 1.0
    }

    // How much of copy `index` of `copies` goes to the left speaker and to
    // the right, before the sum is turned down by the square root of the
    // copies. Each is a square root, so the two squared add up to 2.
    fn copy_sides(index: u32, copies: u32, spread: u32) -> (f64, f64) {
        let pan = place_among(index, copies) * spread as f64 / 100.0;
        ((1.0 - pan).sqrt(), (1.0 + pan).sqrt())
    }

    // The most times as fast as one note alone that `copies` copies spread
    // `spread` percent apart can move on one side: when they line up, each
    // moves by its share of that side. The two sides are mirror images, so
    // the left tells for both.
    fn steepest_side(copies: u32, spread: u32) -> f32 {
        let mut shares = 0.0;
        for index in 0..copies {
            let (left, _) = copy_sides(index, copies, spread);
            shares += left;
        }
        let steepest = shares / (copies as f64).sqrt();
        steepest as f32
    }

    // How much louder `louder` is than `quieter`, in dB.
    fn db_between(louder: f64, quieter: f64) -> f64 {
        20.0 * (louder / quieter).log10()
    }

    #[test]
    fn one_copy_ignores_detune_and_spread() {
        // A lone copy sits right on the note, in the middle, so detune and
        // spread change nothing: every sample, on both sides, is the same as
        // on an engine whose unison was never touched, and the two sides are
        // the same.
        let mut touched = saw_engine();
        let mut untouched = saw_engine();
        touched.set_unison(unison(1, UNISON_MOST_DETUNE_CENTS, FULL_PERCENT));
        for engine in [&mut touched, &mut untouched] {
            engine.note_on(MIDDLE_C, VELOCITY);
            engine.note_on(A4, VELOCITY);
        }
        let touched_sound = render_stereo(&mut touched, 500);
        let untouched_sound = render_stereo(&mut untouched, 500);
        assert!(touched_sound == untouched_sound, "detune or spread changed a lone copy");

        let (left, right) = render_sides(&mut touched, 300);
        assert!(left == right, "a lone copy's sides differ");
    }

    #[test]
    fn two_copies_fully_apart_play_the_detune_below_and_above() {
        // The lowest copy plays only on the left and the highest only on the
        // right, so each side's pitch is one copy's. A sine has no harmonics
        // to blur the measurement.
        for detune in [10, UNISON_MOST_DETUNE_CENTS] {
            let mut engine = Engine::new().unwrap();
            engine.set_first_kind(Kind::Sine);
            engine.set_unison(unison(2, detune, FULL_PERCENT));
            engine.note_on(A4, VELOCITY);
            render_sides(&mut engine, ATTACK_MS + DECAY_MS);
            let (left, right) = render_sides(&mut engine, 1000);
            assert_pitch(&engine, &left, cents_above(A4_HZ, -(detune as f64)));
            assert_pitch(&engine, &right, cents_above(A4_HZ, detune as f64));
        }
    }

    #[test]
    fn seven_copies_sit_evenly_apart_in_pitch_and_between_the_speakers() {
        // A held A5 sine in 7 copies, 50 cents and fully spread: copy n is
        // tuned and placed at its even step from the lowest to the highest,
        // and each side hears its share of it, over the square root of 7.
        // Next to a lone copy's size, every copy's tone on each side is
        // within 3% of what it should be. They are 8.5 Hz apart, so 4
        // seconds keep each one's measurement clear of its neighbors'.
        let copies = UNISON_MOST_COPIES;
        let detune = UNISON_MOST_DETUNE_CENTS;
        let mut lone = Engine::new().unwrap();
        lone.set_first_kind(Kind::Sine);
        lone.note_on(A5, VELOCITY);
        render_sides(&mut lone, ATTACK_MS + DECAY_MS);
        let (lone_left, _) = render_sides(&mut lone, 4000);
        let lone_size = tone_size(&lone, &lone_left, 2.0 * A4_HZ);

        let mut engine = Engine::new().unwrap();
        engine.set_first_kind(Kind::Sine);
        engine.set_unison(unison(copies, detune, FULL_PERCENT));
        engine.note_on(A5, VELOCITY);
        render_sides(&mut engine, ATTACK_MS + DECAY_MS);
        let (left, right) = render_sides(&mut engine, 4000);

        let scale = 1.0 / (copies as f64).sqrt();
        for index in 0..copies {
            let cents = place_among(index, copies) * detune as f64;
            let hz = cents_above(2.0 * A4_HZ, cents);
            let (left_share, right_share) = copy_sides(index, copies, FULL_PERCENT);
            let sides = [(&left, left_share, "left"), (&right, right_share, "right")];
            for (side, share, name) in sides {
                let size = tone_size(&engine, side, hz) / lone_size;
                let expected = share * scale;
                assert!(
                    (size - expected).abs() <= 0.03,
                    "copy {index} at {hz} Hz was {size} on the {name}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn with_no_spread_both_speakers_play_the_same() {
        // Every copy plays in the middle, so the two sides are the same, even
        // through the filter: each side's filter hears the same and
        // remembers the same.
        let mut engine = saw_engine();
        engine.set_filter(filter(800.0, 60));
        engine.set_unison(unison(UNISON_MOST_COPIES, 30, 0));
        engine.note_on(MIDDLE_C, VELOCITY);
        engine.note_on(E4, VELOCITY);
        let (left, right) = render_sides(&mut engine, 500);
        assert!(left == right, "the sides differ with no spread");

        // Spread, the next notes' sides differ.
        engine.set_unison(unison(UNISON_MOST_COPIES, 30, 50));
        engine.note_on(G4, VELOCITY);
        let (left, right) = render_sides(&mut engine, 500);
        assert!(biggest_difference(&left, &right) > 0.01, "the sides are the same with spread");
    }

    #[test]
    fn with_two_copies_fully_apart_each_side_is_a_lone_copy_through_the_filter() {
        // At 0 cents, the lowest copy plays the note, alone on the left: its
        // share of the side, the square root of 2, and the sum's, 1 over the
        // square root of 2, make 1, so the left is a lone copy through its
        // own filter, give or take the last digits of the math. The right is
        // the other copy, the same wave started elsewhere in its cycle, so
        // through its own filter it is just as loud.
        let mut lone = saw_engine();
        let mut pair = saw_engine();
        pair.set_unison(unison(2, 0, FULL_PERCENT));
        for engine in [&mut lone, &mut pair] {
            engine.set_filter(filter(1000.0, 70));
            engine.set_filter_envelope(filter_envelope(2.0, 5, 300, 30, 300));
            engine.note_on(A4, VELOCITY);
        }
        let (lone_left, _) = render_sides(&mut lone, 1000);
        let (left, right) = render_sides(&mut pair, 1000);
        assert_nearly_same_sound(&left, &lone_left, 0.00001);
        assert_within_percent(rms(&right) as f32, rms(&left) as f32, 1.0);
    }

    #[test]
    fn copies_never_start_lined_up() {
        // Each copy starts at its own point in its cycle, so a 7-copy saw's
        // first 20 ms never peak above 1.5 times a lone copy's. Lined up,
        // they would peak at about the square root of 7, 2.65 times.
        let mut lone = saw_engine();
        lone.note_on(A4, VELOCITY);
        let lone_start = render_left(&mut lone, 20);

        let mut engine = saw_engine();
        engine.set_unison(unison(UNISON_MOST_COPIES, 20, 0));
        engine.note_on(A4, VELOCITY);
        let start = render_left(&mut engine, 20);
        assert!(
            peak(&start) < peak(&lone_start) * 1.5,
            "the copies started at {}, a lone copy at {}",
            peak(&start),
            peak(&lone_start)
        );
    }

    #[test]
    fn seven_copies_are_as_loud_as_one_within_1_db() {
        // Copies drift in and out of step, so on average they add up to the
        // square root of how many there are, which the engine turns back
        // down. Spread, each copy is as loud wherever it sits. Over 3 seconds
        // of a held A4 saw, both sides together, 7 copies are within 1 dB of
        // a lone copy.
        let mut lone = saw_engine();
        lone.note_on(A4, VELOCITY);
        render_sides(&mut lone, ATTACK_MS + DECAY_MS);
        let lone_loudness = rms(&render_stereo(&mut lone, 3000));

        for (detune, spread) in [(20, 0), (UNISON_MOST_DETUNE_CENTS, FULL_PERCENT)] {
            let mut engine = saw_engine();
            engine.set_unison(unison(UNISON_MOST_COPIES, detune, spread));
            engine.note_on(A4, VELOCITY);
            render_sides(&mut engine, ATTACK_MS + DECAY_MS);
            let loudness = rms(&render_stereo(&mut engine, 3000));
            let db = db_between(loudness, lone_loudness);
            assert!(db.abs() < 1.0, "7 copies at {detune} cents and {spread}% were {db} dB off");
        }
    }

    #[test]
    fn a_new_note_takes_over_a_unison_voice_without_a_click() {
        // As a_new_note_takes_over_the_oldest_held_one_without_a_click, with
        // middle C in 7 copies spread fully apart, taken over by C5 in 3.
        // Each side has a gap of its own, so neither clicks. Copies added up
        // move faster than one note alone, so the limit is as much faster as
        // they can be, lined up.
        let steeper = steepest_side(UNISON_MOST_COPIES, FULL_PERCENT);
        let fastest_hz = cents_above(C5_HZ, 20.0);
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.set_unison(unison(UNISON_MOST_COPIES, 20, FULL_PERCENT));
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            engine.note_on(MIDDLE_C, VELOCITY);
            press_quiet_notes(&mut engine, VOICE_COUNT - 1);
            let (mut left, mut right) = render_sides(&mut engine, 500 + extra_ms);

            engine.set_unison(unison(3, 20, FULL_PERCENT));
            engine.note_on(C5, VELOCITY);
            let (left_after, right_after) = render_sides(&mut engine, 1000);
            left.extend(left_after);
            right.extend(right_after);
            assert_no_click_steeper(&engine, &left, fastest_hz, steeper);
            assert_no_click_steeper(&engine, &right, fastest_hz, steeper);
        }
    }

    #[test]
    fn the_filter_closes_on_both_sides_and_opened_leaves_both_as_they_were() {
        // As an_open_filter_leaves_the_sound_exactly_as_it_was, with 7
        // copies spread fully apart, so the two sides differ. Closed, the
        // filter changes both. Opened again, once the glide is over, both are
        // exactly as on an engine whose filter was never touched.
        let mut touched = saw_engine();
        let mut untouched = saw_engine();
        for engine in [&mut touched, &mut untouched] {
            engine.set_unison(unison(UNISON_MOST_COPIES, 20, FULL_PERCENT));
            engine.note_on(A4, VELOCITY);
            render_sides(engine, 100);
        }

        touched.set_filter(filter(1000.0, 50));
        let (closed_left, closed_right) = render_sides(&mut touched, 100);
        let (plain_left, plain_right) = render_sides(&mut untouched, 100);
        assert!(biggest_difference(&closed_left, &plain_left) > 0.01);
        assert!(biggest_difference(&closed_right, &plain_right) > 0.01);

        touched.set_filter(START_FILTER);
        render_sides(&mut touched, GLIDE_MS);
        render_sides(&mut untouched, GLIDE_MS);
        let opened = render_stereo(&mut touched, 500);
        let plain = render_stereo(&mut untouched, 500);
        assert!(opened == plain, "the opened filter still changed the sound");
    }

    #[test]
    fn a_held_note_keeps_its_copies_and_the_next_note_gets_the_new_ones() {
        // Like the wave, unison reaches the notes started after it. The held
        // A4 sounds exactly as on an engine whose unison never changed; E4,
        // pressed after, is played by the new copies, spread apart.
        let mut changed = saw_engine();
        let mut unchanged = saw_engine();
        for engine in [&mut changed, &mut unchanged] {
            engine.note_on(A4, VELOCITY);
            render_sides(engine, 100);
        }
        changed.set_unison(unison(UNISON_MOST_COPIES, 30, FULL_PERCENT));
        let held = render_stereo(&mut changed, 300);
        let plain = render_stereo(&mut unchanged, 300);
        assert!(held == plain, "the held note changed");

        for engine in [&mut changed, &mut unchanged] {
            engine.note_on(E4, VELOCITY);
        }
        let (left, right) = render_sides(&mut changed, 300);
        let (plain_left, _) = render_sides(&mut unchanged, 300);
        assert!(biggest_difference(&left, &plain_left) > 0.01, "E4 kept the old copies");
        assert!(biggest_difference(&left, &right) > 0.01, "E4's copies were not spread");
    }

    #[test]
    fn a_silent_voice_keeps_nothing_of_its_last_note_s_filter_on_either_side() {
        // A4 spread apart through a closed filter leaves each side's filter
        // remembering a different wave when it falls silent. The next A4 on
        // that voice has no spread, so both sides hear the same wave, and
        // with nothing left over they play exactly the same.
        let mut engine = saw_engine();
        engine.set_filter(filter(400.0, 60));
        engine.set_unison(unison(2, 20, FULL_PERCENT));
        engine.note_on(A4, VELOCITY);
        render_sides(&mut engine, 300);
        engine.note_off(A4);
        render_sides(&mut engine, RELEASE_MS + 100);

        engine.set_unison(unison(2, 20, 0));
        engine.note_on(A4, VELOCITY);
        let (left, right) = render_sides(&mut engine, 300);
        assert!(left == right, "a side kept its last note's filter");
    }

    #[test]
    fn a_layer_s_volume_turns_down_both_speakers() {
        // A loop of A4 by 2 copies fully apart, so each side plays a copy of
        // its own. Turned down to half, both sides play exactly half of what
        // they play at full volume.
        let loop_a4 = |percent: u32| {
            let notes = [(0, NoteOn(A4)), (90_000, NoteOff(A4))];
            let mut steps = vec![(0, PressLooper), (RECORDING_STARTS + 80_000, PressLooper)];
            steps.extend(moved(&notes, RECORDING_STARTS));
            steps.push((LOOP_STARTS, Volume(1, percent)));
            let mut engine = engine_without_snap();
            engine.set_unison(unison(2, 20, FULL_PERCENT));
            play(&mut engine, &steps, LOOP_STARTS + 20_000);
            render_sides(&mut engine, 300)
        };
        let (left, right) = loop_a4(FULL_VOLUME);
        let (half_left, half_right) = loop_a4(50);

        assert!(peak(&right) > 0.01, "the loop is silent");
        for i in 0..left.len() {
            assert_eq!(half_left[i], left[i] * 0.5, "left sample {i}");
            assert_eq!(half_right[i], right[i] * 0.5, "right sample {i}");
        }
    }

    #[test]
    fn unison_sounds_the_same_however_the_sound_is_asked_for() {
        // As the_same_notes_always_make_the_same_sound, both sides, with 7
        // copies spread apart through the filter, a note taken over by fewer
        // copies, and a let go.
        let mut whole = saw_engine();
        let mut chunked = saw_engine();
        for engine in [&mut whole, &mut chunked] {
            engine.set_filter(filter(700.0, 40));
            engine.set_unison(unison(UNISON_MOST_COPIES, 35, 80));
            engine.note_on(MIDDLE_C, VELOCITY);
        }
        let mut whole_sound = render_stereo(&mut whole, 300);
        let mut chunked_sound = Vec::new();
        for _ in 0..30 {
            chunked_sound.extend(render_stereo(&mut chunked, 10));
        }

        for engine in [&mut whole, &mut chunked] {
            engine.set_unison(unison(4, 10, 30));
            engine.note_on(MIDDLE_C, VELOCITY);
        }
        whole_sound.extend(render_stereo(&mut whole, 300));
        for _ in 0..30 {
            chunked_sound.extend(render_stereo(&mut chunked, 10));
        }

        whole.note_off(MIDDLE_C);
        chunked.note_off(MIDDLE_C);
        whole_sound.extend(render_stereo(&mut whole, 400));
        for _ in 0..40 {
            chunked_sound.extend(render_stereo(&mut chunked, 10));
        }

        assert!(whole_sound == chunked_sound, "the two engines sounded different");
    }

    // A held A4 saw in 3 copies, 25 cents apart and spread halfway, after
    // `send` asks the engine for something directly.
    fn unison_a4_after(send: impl Fn(&Engine)) -> Vec<f32> {
        let mut engine = saw_engine();
        engine.set_unison(unison(3, 25, 50));
        send(&engine);
        engine.note_on(A4, VELOCITY);
        render_stereo(&mut engine, 300)
    }

    #[test]
    fn odd_unison_values_change_nothing() {
        // Asked directly, the engine ignores values outside unison's ranges,
        // and NaN. Each odd value goes to an engine of its own, as in
        // odd_filter_values_change_nothing.
        let plain = unison_a4_after(|_| {});

        let odd_values = [
            (0, 25.0, 0.5),
            (-1, 25.0, 0.5),
            (8, 25.0, 0.5),
            (3, -0.1, 0.5),
            (3, 50.1, 0.5),
            (3, f32::NAN, 0.5),
            (3, 25.0, -0.1),
            (3, 25.0, 1.1),
            (3, 25.0, f32::NAN),
        ];
        for (copies, detune_cents, spread) in odd_values {
            let odd = unison_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_unison(engine.raw, copies, detune_cents, spread) };
            });
            assert!(
                odd == plain,
                "{copies} copies at {detune_cents} cents and {spread} changed the sound"
            );
        }
    }

    #[test]
    fn rust_brings_unison_values_within_their_ranges() {
        // Too many copies, too much detune and too much spread are sent as
        // the most of each, so they sound the same as the most. The engine
        // takes those edges: they don't sound like a lone copy.
        let mut over = saw_engine();
        let sent = over.set_unison(unison(12, 80, 150));
        let most = unison(UNISON_MOST_COPIES, UNISON_MOST_DETUNE_CENTS, FULL_PERCENT);
        assert_eq!(sent, most);
        let mut edges = saw_engine();
        edges.set_unison(most);
        let mut lone = saw_engine();
        for engine in [&mut over, &mut edges, &mut lone] {
            engine.note_on(A4, VELOCITY);
        }
        let over_sound = render_stereo(&mut over, 300);
        let edges_sound = render_stereo(&mut edges, 300);
        let lone_sound = render_stereo(&mut lone, 300);
        assert!(over_sound == edges_sound, "too much was not sent as the most");
        assert!(biggest_difference(&edges_sound, &lone_sound) > 0.01, "the engine refused the most");

        // No copies at all is sent as 1. The lowest detune and spread are
        // taken too: 7 copies at 0 cents and 0% don't sound like a lone copy.
        let mut engine = saw_engine();
        let sent = engine.set_unison(unison(0, 0, 0));
        assert_eq!(sent, unison(1, 0, 0));
        engine.set_unison(unison(UNISON_MOST_COPIES, 0, 0));
        let mut lone = saw_engine();
        engine.note_on(A4, VELOCITY);
        lone.note_on(A4, VELOCITY);
        let lowest_sound = render_stereo(&mut engine, 300);
        let lone_sound = render_stereo(&mut lone, 300);
        assert!(biggest_difference(&lowest_sound, &lone_sound) > 0.01, "the engine refused the lowest");
    }

    #[test]
    fn a_new_engine_starts_with_one_copy() {
        // As START_UNISON says, so a new song's sound profile, sent as the
        // app starts, changes nothing.
        assert_eq!(START_UNISON, unison(1, 20, 100));

        let mut started = saw_engine();
        let mut sent = saw_engine();
        sent.set_unison(START_UNISON);
        started.note_on(A4, VELOCITY);
        sent.note_on(A4, VELOCITY);
        let started_sound = render_stereo(&mut started, 300);
        let sent_sound = render_stereo(&mut sent, 300);
        assert!(started_sound == sent_sound, "a new engine's unison is not where START_UNISON says");
    }

    // The loudness envelope, key tracking and tuning, each the song's own
    // (docs/adr/0030-a-sound-profile-per-song.md). Every test above plays a
    // new engine's: a 5 ms attack, held at the full peak, a 300 ms release,
    // key tracking at 100% and A4 at 440 Hz.

    fn loudness(attack_ms: u32, decay_ms: u32, sustain: u32, release_ms: u32) -> LoudnessEnvelope {
        LoudnessEnvelope {
            attack_ms,
            decay_ms,
            sustain,
            release_ms,
        }
    }

    // How loud `envelope` has a note at sample `i`, as a share of its peak,
    // for a note started on sample 0 and let go on sample `let_go`, once it
    // holds at its sustain. Each stage is a straight line, ending on its
    // last sample.
    fn envelope_share(engine: &Engine, envelope: LoudnessEnvelope, i: usize, let_go: usize) -> f64 {
        let attack = frames_in_ms(engine, envelope.attack_ms);
        let decay = frames_in_ms(engine, envelope.decay_ms);
        let sustain = envelope.sustain as f64 / FULL_PERCENT as f64;
        let release = frames_in_ms(engine, envelope.release_ms);

        if i >= let_go {
            let into_release = i + 1 - let_go;
            if into_release >= release {
                return 0.0;
            }
            return sustain * (1.0 - into_release as f64 / release as f64);
        }
        if i < attack {
            return (i + 1) as f64 / attack as f64;
        }
        let into_decay = i + 1 - attack;
        if into_decay < decay {
            return 1.0 - (1.0 - sustain) * into_decay as f64 / decay as f64;
        }
        sustain
    }

    #[test]
    fn a_note_follows_its_loudness_envelope_through_every_stage() {
        // A 100 ms attack, a 200 ms decay to half the peak, held until 500
        // ms, then a 400 ms release. Next to a note held at the full peak,
        // every sample is exactly as loud as the envelope says, from the end
        // of that note's own 5 ms attack on. Then it is exact silence.
        let envelope = loudness(100, 200, 50, 400);
        let mut shaped = Engine::new().unwrap();
        shaped.set_loudness_envelope(envelope);
        let mut full = Engine::new().unwrap();
        shaped.note_on(A4, VELOCITY);
        full.note_on(A4, VELOCITY);

        let mut shaped_wave = render_left(&mut shaped, 500);
        let mut full_wave = render_left(&mut full, 500);
        let let_go = shaped_wave.len();
        shaped.note_off(A4);
        shaped_wave.extend(render_left(&mut shaped, 400));
        full_wave.extend(render_left(&mut full, 400));

        for i in frames_in_ms(&full, ATTACK_MS)..shaped_wave.len() {
            let share = envelope_share(&shaped, envelope, i, let_go);
            let expected = full_wave[i] as f64 * share;
            let difference = (shaped_wave[i] as f64 - expected).abs();
            assert!(
                difference < 0.000001,
                "sample {i} is {}, expected {expected}",
                shaped_wave[i]
            );
        }
        let after = render_left(&mut shaped, 100);
        assert_eq!(peak(&after), 0.0);
    }

    #[test]
    fn a_held_note_keeps_its_loudness_envelope_and_the_next_note_gets_the_new_one() {
        // The release changes from 300 ms to 1 second while A4 is held. A4
        // still fades out in 300 ms. The next A4 takes the whole second.
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, 100);
        engine.set_loudness_envelope(loudness(ATTACK_MS, DECAY_MS, 100, 1000));
        render_left(&mut engine, 100);
        engine.note_off(A4);
        render_left(&mut engine, RELEASE_MS);
        let after_old_release = render_left(&mut engine, 100);
        assert_eq!(peak(&after_old_release), 0.0, "the held note took the new release");

        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, 100);
        engine.note_off(A4);
        render_left(&mut engine, RELEASE_MS);
        let still_fading = render_left(&mut engine, 100);
        assert!(peak(&still_fading) > 0.0, "the next note didn't get the new release");
        render_left(&mut engine, 600);
        let after_new_release = render_left(&mut engine, 100);
        assert_eq!(peak(&after_new_release), 0.0);
    }

    // How much of A4's second harmonic, 880 Hz, held on a saw, gets through
    // the filter with middle C's cutoff at CUTOFF_FOR_A4_AT_880 and key
    // tracking at `key_tracking` percent.
    fn a4_second_harmonic_through(key_tracking: u32) -> f64 {
        let mut open = saw_engine();
        let dry = hold(&mut open, A4, VELOCITY);
        let mut filtered = saw_engine();
        filtered.set_filter(filter(CUTOFF_FOR_A4_AT_880, 0));
        filtered.set_key_tracking(key_tracking);
        let wet = hold(&mut filtered, A4, VELOCITY);
        tone_size(&open, &wet, 880.0) / tone_size(&open, &dry, 880.0)
    }

    #[test]
    fn key_tracking_sets_how_far_a_note_s_cutoff_follows_middle_c() {
        // A4 is three quarters of an octave above middle C. At 100% its
        // cutoff is that far above middle C's, right on its second harmonic.
        // At 50% it is half as far, and at 0% right at middle C's. Each lets
        // the harmonic through as much as the filter's math says, give or
        // take 1%.
        let engine = Engine::new().unwrap();
        for key_tracking in [100, 50, 0] {
            let share = key_tracking as f64 / FULL_PERCENT as f64;
            let octaves_up = 0.75 * share;
            let cutoff_hz = CUTOFF_FOR_A4_AT_880 as f64 * octaves_up.exp2();
            let expected = low_pass_gain(&engine, 880.0, cutoff_hz, LOWEST_Q);
            let through = a4_second_harmonic_through(key_tracking);
            assert!(
                (through - expected).abs() <= expected * 0.01,
                "at {key_tracking}% it let through {through}, expected {expected}"
            );
        }
    }

    #[test]
    fn tuning_moves_every_note_with_a4() {
        // Tuned to 432, A4 plays at 432 Hz and middle C keeps its distance
        // below it.
        let mut engine = Engine::new().unwrap();
        engine.set_tuning(432.0);
        let a4 = hold(&mut engine, A4, VELOCITY);
        assert_pitch(&engine, &a4, 432.0);

        let mut engine = Engine::new().unwrap();
        engine.set_tuning(432.0);
        let middle_c = hold(&mut engine, MIDDLE_C, VELOCITY);
        assert_pitch(&engine, &middle_c, MIDDLE_C_HZ * 432.0 / A4_HZ);
    }

    #[test]
    fn a_held_note_keeps_its_pitch_and_the_next_note_gets_the_new_tuning() {
        let mut engine = Engine::new().unwrap();
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, ATTACK_MS + DECAY_MS);
        engine.set_tuning(TUNING_HIGHEST_HZ);
        let held = render_left(&mut engine, 1000);
        assert_pitch(&engine, &held, A4_HZ);

        engine.note_off(A4);
        render_left(&mut engine, RELEASE_MS + 100);
        let next = hold(&mut engine, A4, VELOCITY);
        assert_pitch(&engine, &next, TUNING_HIGHEST_HZ as f64);
    }

    // A4 on a saw through the filter at 1000 Hz, with a loudness envelope,
    // key tracking and tuning all unlike a new engine's, after `send` asks
    // the engine for something directly. The note is let go, so every stage
    // is heard.
    fn shaped_a4_after(send: impl Fn(&Engine)) -> Vec<f32> {
        let mut engine = saw_engine();
        engine.set_filter(filter(1000.0, 0));
        engine.set_loudness_envelope(loudness(20, 100, 50, 200));
        engine.set_key_tracking(50);
        engine.set_tuning(450.0);
        send(&engine);

        engine.note_on(A4, VELOCITY);
        let mut wave = render_left(&mut engine, 300);
        engine.note_off(A4);
        wave.extend(render_left(&mut engine, 300));
        wave
    }

    #[test]
    fn odd_loudness_key_tracking_and_tuning_values_change_nothing() {
        // Asked directly, the engine ignores values outside their ranges,
        // and NaN. Each odd value goes to an engine of its own, so one that
        // got through can't hide behind the next.
        let plain = shaped_a4_after(|_| {});

        let odd_envelopes = [
            (0, 100, 0.5, 200),
            (10001, 100, 0.5, 200),
            (20, 0, 0.5, 200),
            (20, 10001, 0.5, 200),
            (20, 100, -0.1, 200),
            (20, 100, 1.1, 200),
            (20, 100, f32::NAN, 200),
            (20, 100, 0.5, 0),
            (20, 100, 0.5, 10001),
        ];
        for (attack, decay, sustain, release) in odd_envelopes {
            let odd = shaped_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_loudness_envelope(engine.raw, attack, decay, sustain, release) };
            });
            assert!(
                odd == plain,
                "the envelope {attack}, {decay}, {sustain}, {release} changed the sound"
            );
        }
        for key_tracking in [-0.1, 1.1, f32::NAN] {
            let odd = shaped_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_key_tracking(engine.raw, key_tracking) };
            });
            assert!(odd == plain, "key tracking at {key_tracking} changed the sound");
        }
        for a4_hz in [414.9, 466.1, f32::NAN] {
            let odd = shaped_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_tuning(engine.raw, a4_hz) };
            });
            assert!(odd == plain, "tuning to {a4_hz} changed the sound");
        }
    }

    #[test]
    fn rust_brings_loudness_key_tracking_and_tuning_within_their_ranges() {
        // Every value out of range comes back at its edge.
        let mut engine = Engine::new().unwrap();
        let sent = engine.set_loudness_envelope(loudness(0, 20000, 150, 0));
        let shortest = LOUDNESS_SHORTEST_MS;
        let longest = LOUDNESS_LONGEST_MS;
        assert_eq!(sent, loudness(shortest, longest, FULL_PERCENT, shortest));
        let sent = engine.set_loudness_envelope(loudness(20000, 0, 50, 20000));
        assert_eq!(sent, loudness(longest, shortest, 50, longest));
        assert_eq!(engine.set_key_tracking(150), FULL_PERCENT);
        assert_eq!(engine.set_tuning(400.0), TUNING_LOWEST_HZ);
        assert_eq!(engine.set_tuning(500.0), TUNING_HIGHEST_HZ);
        assert_eq!(engine.set_tuning(f32::NAN), TUNING_LOWEST_HZ);

        // The engine takes the edges. A 0 ms attack is sent as 1 ms, which
        // starts the note faster than a new engine's 5 ms.
        let mut fastest = Engine::new().unwrap();
        fastest.set_loudness_envelope(loudness(0, DECAY_MS, 100, RELEASE_MS));
        let mut plain = Engine::new().unwrap();
        fastest.note_on(A4, VELOCITY);
        plain.note_on(A4, VELOCITY);
        let fastest_start = render_left(&mut fastest, ATTACK_MS);
        let plain_start = render_left(&mut plain, ATTACK_MS);
        assert!(peak(&fastest_start) > peak(&plain_start), "the engine refused the shortest attack");

        // And a tuning below the lowest plays as the lowest.
        let mut below = Engine::new().unwrap();
        below.set_tuning(400.0);
        let wave = hold(&mut below, A4, VELOCITY);
        assert_pitch(&below, &wave, TUNING_LOWEST_HZ as f64);
    }

    #[test]
    fn a_new_engine_starts_with_a_new_song_s_loudness_key_tracking_and_tuning() {
        // As the START_ values say, so a new song's sound profile, sent as
        // the app starts, changes nothing.
        assert_eq!(START_LOUDNESS_ENVELOPE, loudness(ATTACK_MS, DECAY_MS, 100, RELEASE_MS));
        assert_eq!(START_KEY_TRACKING, 100);
        assert_eq!(START_TUNING_HZ, 440.0);

        let mut started = saw_engine();
        let mut sent = saw_engine();
        for engine in [&mut started, &mut sent] {
            engine.set_filter(filter(1000.0, 0));
        }
        sent.set_loudness_envelope(START_LOUDNESS_ENVELOPE);
        sent.set_key_tracking(START_KEY_TRACKING);
        sent.set_tuning(START_TUNING_HZ);
        started.note_on(A4, VELOCITY);
        sent.note_on(A4, VELOCITY);
        let started_wave = render_left(&mut started, 300);
        let sent_wave = render_left(&mut sent, 300);
        assert!(started_wave == sent_wave, "a new engine does not start where the START_ values say");
    }

    // LFOs and the routing list (docs/adr/0031-lfos-and-a-routing-list.md),
    // and the vibrato key (docs/adr/0032-the-vibrato-key.md). Every test
    // above plays a new engine's 8 LFOs and 64 connections, none of them
    // used, so nothing moves.
    //
    // The routing list is worked out every 32 samples, counted from the
    // engine's first, and each move goes in a straight line from one to the
    // next. The surest way to watch a move is the loudness: a note turned
    // down by the routing list is exactly the same note without it, times
    // its gain, on every sample.

    const CONTROL_SAMPLES: usize = 32; // `control_samples` in engine/src/routing.h
    const SHARED_GLIDE_SAMPLES: usize = 960; // `glide_ms` in engine/src/routing.cpp: 20 ms
    const STEP_NUDGE: f64 = 0.000000001; // `step_nudge` in engine/src/lfo.cpp
    const ONCE_END_NUDGE: f64 = 0.000000001; // `once_end_nudge` in engine/src/voice.cpp
    const SAMPLES_PER_SECOND: f64 = 48_000.0;
    const B4_HZ: f64 = 493.8833;

    // Every shape that is drawn, not random.
    const DRAWN_SHAPES: [LfoShape; 6] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::Square,
        LfoShape::SawUp,
        LfoShape::SawDown,
        LfoShape::Steps,
    ];

    // Five steps for the steps shape, in percent.
    const STEP_HEIGHTS: [f32; 5] = [-100.0, -35.0, 60.0, 100.0, 15.0];

    // An LFO like a new engine's, but with `shape` at `hz`, each note's copy
    // starting right at its start point, and no smoothing.
    fn lfo_at(shape: LfoShape, hz: f64) -> Lfo {
        Lfo {
            shape,
            hz,
            spread: 0,
            smooth: 0,
            ..START_LFO
        }
    }

    // `lfo` with `heights`, in percent, as its steps.
    fn with_steps(lfo: Lfo, heights: &[f32]) -> Lfo {
        let mut steps = [0.0; LFO_MOST_STEPS];
        steps[..heights.len()].copy_from_slice(heights);
        Lfo {
            step_count: heights.len() as u32,
            steps,
            ..lfo
        }
    }

    // `source` moving `target` by `amount`, in the target's own units, both
    // ways.
    fn connection(source: Source, target: Target, amount: f32) -> Connection {
        Connection {
            source,
            target,
            amount,
            ..NO_CONNECTION
        }
    }

    // The same, read one way.
    fn one_way_connection(source: Source, target: Target, amount: f32) -> Connection {
        Connection {
            one_way: true,
            ..connection(source, target, amount)
        }
    }

    // `source`, read one way, turning the loudness down: all the way to
    // silence at its top, and not at all at its bottom.
    fn dip_by(source: Source) -> Connection {
        one_way_connection(source, Target::Loudness, -100.0)
    }

    // The gain dip_by gives at a source's `height`, from -1 to 1.
    fn gain_at_height(height: f64) -> f64 {
        1.0 - (height + 1.0) / 2.0
    }

    // Every target, the LFOs' speeds and the oscillators' targets included.
    fn every_target() -> Vec<Target> {
        let mut targets = Target::OTHERS.to_vec();
        for index in 0..LFO_COUNT {
            targets.push(Target::LfoSpeed(index));
        }
        for index in 0..OSCILLATOR_COUNT {
            targets.push(Target::OscillatorLevel(index));
            targets.push(Target::OscillatorPitch(index));
            targets.push(Target::OscillatorKind(index));
        }
        targets
    }

    // A new engine with `lfos`, each at its index, and `connections` in the
    // routing list's first slots.
    fn routed_engine(lfos: &[(usize, Lfo)], connections: &[Connection]) -> Engine {
        let mut engine = Engine::new().unwrap();
        for (index, lfo) in lfos {
            engine.set_lfo(*index, *lfo);
        }
        for (index, connection) in connections.iter().enumerate() {
            engine.set_connection(index, *connection);
        }
        engine
    }

    // A4 played for `ms` from the first sample on a routed engine and on a
    // new one: the left sides, moved and plain.
    fn a4_routed(lfos: &[(usize, Lfo)], connections: &[Connection], ms: u32) -> (Vec<f32>, Vec<f32>) {
        let mut routed = routed_engine(lfos, connections);
        let mut plain = Engine::new().unwrap();
        routed.note_on(A4, VELOCITY);
        plain.note_on(A4, VELOCITY);
        let moved = render_left(&mut routed, ms);
        let plain_wave = render_left(&mut plain, ms);
        (moved, plain_wave)
    }

    // Where a move is on sample `i`, given where it is on the samples the
    // routing list is worked out on: `at(t)` for t = 0, 32, 64 and so on.
    // In between it goes in a straight line.
    fn on_the_line(at: &impl Fn(usize) -> f64, i: usize) -> f64 {
        let block = i - i % CONTROL_SAMPLES;
        let into = (i - block) as f64 / CONTROL_SAMPLES as f64;
        let from = at(block);
        let to = at(block + CONTROL_SAMPLES);
        from + (to - from) * into
    }

    // Checks that `moved` is `plain` times the gain `gain_at(t)` says on the
    // samples t the routing list is worked out on, and in straight lines in
    // between, on every sample. `what` names the case in a failure.
    fn assert_gain(what: &str, moved: &[f32], plain: &[f32], gain_at: impl Fn(usize) -> f64) {
        assert_eq!(moved.len(), plain.len());
        for i in 0..moved.len() {
            let gain = on_the_line(&gain_at, i);
            let expected = plain[i] as f64 * gain;
            let difference = (moved[i] as f64 - expected).abs();
            assert!(
                difference < 0.000001,
                "{what}: sample {i} is {}, expected {expected}, a gain of {gain}",
                moved[i]
            );
        }
    }

    // The gain `moved` has against `plain` from sample `from` to `to`, where
    // it holds still: read at the loudest sample there, the most exact.
    fn gain_between(moved: &[f32], plain: &[f32], from: usize, to: usize) -> f64 {
        let mut loudest = from;
        for i in from..to {
            if plain[i].abs() > plain[loudest].abs() {
                loudest = i;
            }
        }
        moved[loudest] as f64 / plain[loudest] as f64
    }

    // `percent` as a share from 0 to 1, as Rust sends it.
    fn share(percent: u32) -> f64 {
        percent as f64 / FULL_PERCENT as f64
    }

    // Step `index` of `lfo`, from -1 to 1, as Rust sends it.
    fn step_height(lfo: &Lfo, index: usize) -> f64 {
        let height = lfo.steps[index] / FULL_PERCENT as f32;
        height as f64
    }

    // How many cycles an LFO at `hz` goes in `samples` samples.
    fn cycles_after(hz: f64, samples: usize) -> f64 {
        hz * samples as f64 / SAMPLES_PER_SECOND
    }

    // How a shape jumps, smoothed: from `before` to `height`, sliding over
    // the first `smooth` of the stretch, `into` of the way through it
    // (`hold` in engine/src/lfo.cpp).
    fn slid(before: f64, height: f64, into: f64, smooth: f64) -> f64 {
        if into < smooth {
            return before + (height - before) * (into / smooth);
        }
        height
    }

    // How high `lfo` is `cycles` cycles into its run, from -1 to 1, drawn as
    // engine/src/lfo.h says. Only for the shapes that are not random.
    fn drawn_height(lfo: &Lfo, cycles: f64) -> f64 {
        let phase = cycles - cycles.floor();
        let smooth = share(lfo.smooth);
        match lfo.shape {
            LfoShape::Sine => (2.0 * PI * phase).sin(),
            LfoShape::Triangle => {
                if phase < 0.25 {
                    4.0 * phase
                } else if phase < 0.75 {
                    2.0 - 4.0 * phase
                } else {
                    4.0 * phase - 4.0
                }
            }
            LfoShape::Square => {
                if phase < 0.5 {
                    slid(-1.0, 1.0, phase / 0.5, smooth)
                } else {
                    slid(1.0, -1.0, (phase - 0.5) / 0.5, smooth)
                }
            }
            LfoShape::SawUp => {
                if phase < smooth {
                    slid(1.0, -1.0, phase, smooth)
                } else {
                    -1.0 + 2.0 * (phase - smooth) / (1.0 - smooth)
                }
            }
            LfoShape::SawDown => {
                if phase < smooth {
                    slid(-1.0, 1.0, phase, smooth)
                } else {
                    1.0 - 2.0 * (phase - smooth) / (1.0 - smooth)
                }
            }
            LfoShape::Steps => {
                let count = lfo.step_count as usize;
                let place = phase * count as f64;
                let mut step = (place + STEP_NUDGE).floor() as usize;
                if step > count - 1 {
                    step = count - 1;
                }
                let mut into = place - step as f64;
                if into < 0.0 {
                    into = 0.0;
                }
                // The step before the first is the last.
                let before = (step + count - 1) % count;
                slid(step_height(lfo, before), step_height(lfo, step), into, smooth)
            }
            _ => panic!("a random shape has no drawing"),
        }
    }

    #[test]
    fn each_of_the_8_lfos_moves_by_its_own_settings() {
        for index in 0..LFO_COUNT {
            let lfo = lfo_at(LfoShape::Sine, 1.31 + index as f64);
            let (moved, plain) = a4_routed(&[(index, lfo)], &[dip_by(Source::Lfo(index))], 500);
            assert_gain(&format!("lfo {}", index + 1), &moved, &plain, |t| {
                gain_at_height(drawn_height(&lfo, cycles_after(lfo.hz, t)))
            });
        }
    }

    #[test]
    fn every_lfo_shape_moves_a_note_as_drawn() {
        // Per note, at 3.31 Hz and starting 27% into its cycle, each shape
        // turns A4 down as far as it is high, on every sample. The odd
        // numbers keep every jump off the samples the routing list is worked
        // out on, where a hair either way would land on either side of it.
        for shape in DRAWN_SHAPES {
            let lfo = with_steps(
                Lfo {
                    start: 27,
                    ..lfo_at(shape, 3.31)
                },
                &STEP_HEIGHTS,
            );
            let (moved, plain) = a4_routed(&[(0, lfo)], &[dip_by(Source::Lfo(0))], 1000);
            assert_gain(shape.name(), &moved, &plain, |t| {
                let cycles = share(lfo.start) + cycles_after(lfo.hz, t);
                gain_at_height(drawn_height(&lfo, cycles))
            });
        }
    }

    #[test]
    fn smoothing_slides_each_jump_over_its_share() {
        // Smoothed 40%, the square and the steps slide to each new height
        // over the first 40% of its stretch, and the saws jump back over the
        // first 40% of each cycle. The sine and the triangle never jump, so
        // smoothing leaves them alone.
        for shape in DRAWN_SHAPES {
            let lfo = with_steps(
                Lfo {
                    start: 27,
                    smooth: 40,
                    ..lfo_at(shape, 3.31)
                },
                &STEP_HEIGHTS,
            );
            let (moved, plain) = a4_routed(&[(0, lfo)], &[dip_by(Source::Lfo(0))], 1000);
            assert_gain(shape.name(), &moved, &plain, |t| {
                let cycles = share(lfo.start) + cycles_after(lfo.hz, t);
                gain_at_height(drawn_height(&lfo, cycles))
            });
        }

        // Fully smoothed, a saw up is all jump back: a saw down.
        let up = Lfo {
            smooth: 100,
            ..lfo_at(LfoShape::SawUp, 3.31)
        };
        let down = lfo_at(LfoShape::SawDown, 3.31);
        let (up_wave, _) = a4_routed(&[(0, up)], &[dip_by(Source::Lfo(0))], 1000);
        let (down_wave, _) = a4_routed(&[(0, down)], &[dip_by(Source::Lfo(0))], 1000);
        assert_nearly_same_sound(&up_wave, &down_wave, 0.000001);
    }

    // The height a random steps LFO at `hz` holds in each of its cycles,
    // read off A4 dipped by it from the first sample: in each cycle, away
    // from its ends, where the gain holds still. The last may be cut short.
    fn held_heights(moved: &[f32], plain: &[f32], hz: f64) -> Vec<f64> {
        let cycle_samples = SAMPLES_PER_SECOND / hz;
        let mut heights = Vec::new();
        let mut cycle = 0;
        loop {
            // The gain holds still from the 32 samples after the cycle
            // starts until the 32 samples the next one starts in.
            let start = (cycle as f64 * cycle_samples) as usize;
            let from = start - start % CONTROL_SAMPLES + 2 * CONTROL_SAMPLES;
            let end = ((cycle + 1) as f64 * cycle_samples) as usize;
            let to = usize::min(end - end % CONTROL_SAMPLES, moved.len());
            if from + CONTROL_SAMPLES > to {
                return heights;
            }
            let gain = gain_between(moved, plain, from, to);
            heights.push(1.0 - 2.0 * gain);
            cycle += 1;
        }
    }

    #[test]
    fn random_steps_hold_a_new_height_each_cycle_and_smooth_random_glides_through_them() {
        // Random steps hold one height through each whole cycle.
        let hz = 10.3;
        let lfo = lfo_at(LfoShape::RandomSteps, hz);
        let (moved, plain) = a4_routed(&[(0, lfo)], &[dip_by(Source::Lfo(0))], 2000);
        let heights = held_heights(&moved, &plain, hz);
        assert_eq!(heights.len(), 21);
        assert_gain("random steps", &moved, &plain, |t| {
            let cycle = cycles_after(hz, t).floor() as usize;
            gain_at_height(heights[cycle])
        });

        // A new one each cycle, from anywhere between -1 and 1.
        for pair in heights.windows(2) {
            assert!(pair[0] != pair[1], "two cycles in a row held {}", pair[0]);
        }
        let mut lowest = 1.0;
        let mut highest = -1.0;
        for height in &heights {
            assert!(height.abs() <= 1.0, "a height of {height}");
            lowest = f64::min(lowest, *height);
            highest = f64::max(highest, *height);
        }
        assert!(lowest < -0.5, "the lowest height was {lowest}");
        assert!(highest > 0.5, "the highest height was {highest}");

        // Smooth random, at the same speed on the same note, starts each
        // cycle at the height random steps hold through it, then glides to
        // the next in half a cosine, leaving and landing gently.
        let lfo = lfo_at(LfoShape::SmoothRandom, hz);
        let (moved, plain) = a4_routed(&[(0, lfo)], &[dip_by(Source::Lfo(0))], 1900);
        assert_gain("smooth random", &moved, &plain, |t| {
            let cycles = cycles_after(hz, t);
            let cycle = cycles.floor();
            let done = (1.0 - (PI * (cycles - cycle)).cos()) / 2.0;
            let from = heights[cycle as usize];
            let to = heights[cycle as usize + 1];
            gain_at_height(from + (to - from) * done)
        });
    }

    // Checks that no height in `heights` is the one in `others` at its
    // place.
    fn assert_all_differ(heights: &[f64], others: &[f64]) {
        assert!(!heights.is_empty());
        for (height, other) in heights.iter().zip(others) {
            assert!(height != other, "both held {height}");
        }
    }

    #[test]
    fn a_random_lfo_moves_each_note_and_each_lfo_its_own_way_and_the_same_way_every_time() {
        let lfo = lfo_at(LfoShape::RandomSteps, 10.3);
        let dip = dip_by(Source::Lfo(0));

        // Two engines playing the same notes sound exactly the same.
        let (first, plain) = a4_routed(&[(0, lfo)], &[dip], 500);
        let (again, _) = a4_routed(&[(0, lfo)], &[dip], 500);
        assert_same_samples(&again, &first);
        let first_heights = held_heights(&first, &plain, lfo.hz);

        // LFO 2, set the same way, has heights of its own.
        let (lfo_2, _) = a4_routed(&[(1, lfo)], &[dip_by(Source::Lfo(1))], 500);
        assert_all_differ(&held_heights(&lfo_2, &plain, lfo.hz), &first_heights);

        // So does the next note pressed.
        let mut routed = routed_engine(&[(0, lfo)], &[dip]);
        let mut plain_engine = Engine::new().unwrap();
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut plain_engine] {
            engine.note_on(A4, VELOCITY);
            render_left(engine, 500);
            engine.note_off(A4);
            render_left(engine, 500);
            engine.note_on(A4, VELOCITY);
            waves.push(render_left(engine, 500));
        }
        assert_all_differ(&held_heights(&waves[0], &waves[1], lfo.hz), &first_heights);
    }

    #[test]
    fn a_per_note_lfo_starts_with_each_note_and_a_shared_one_runs_for_every_note() {
        let lfo = lfo_at(LfoShape::Sine, 3.31);
        let dip = dip_by(Source::Lfo(0));

        // Per note, A4 pressed 100 ms in moves exactly like A4 pressed at
        // once.
        let (at_once, _) = a4_routed(&[(0, lfo)], &[dip], 500);
        let mut later = routed_engine(&[(0, lfo)], &[dip]);
        render_left(&mut later, 100);
        later.note_on(A4, VELOCITY);
        let later_wave = render_left(&mut later, 500);
        assert_same_samples(&later_wave, &at_once);

        // Shared, one LFO runs from the engine's start for every note, so A4
        // pressed 100 ms in starts 100 ms into its cycle.
        let shared = Lfo { per_note: false, ..lfo };
        let mut routed = routed_engine(&[(0, shared)], &[dip]);
        let mut plain = Engine::new().unwrap();
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut plain] {
            render_left(engine, 100);
            engine.note_on(A4, VELOCITY);
            waves.push(render_left(engine, 500));
        }
        let lead = frames_in_ms(&plain, 100);
        assert_gain("shared", &waves[0], &waves[1], |t| {
            gain_at_height(drawn_height(&shared, cycles_after(shared.hz, lead + t)))
        });
    }

    #[test]
    fn spread_moves_each_note_s_start_on_at_random() {
        // A saw up 100 seconds a cycle barely moves in a note's first 10 ms,
        // so A4's gain there shows where in the cycle it started: p of the
        // way in is a height of 2p - 1, and so a gain of 1 - p. Twenty
        // presses of A4, 50 ms apart, each start somewhere from the start
        // point to the spread past it.
        for (start, spread) in [(0, 0), (0, 50), (25, 50), (0, 100)] {
            let lfo = Lfo {
                start,
                spread,
                ..lfo_at(LfoShape::SawUp, 0.01)
            };
            let mut routed = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]);
            let mut plain = Engine::new().unwrap();
            let first_10_ms = frames_in_ms(&plain, 10);
            let mut places = Vec::new();
            for _ in 0..20 {
                let mut waves = Vec::new();
                for engine in [&mut routed, &mut plain] {
                    engine.note_on(A4, VELOCITY);
                    waves.push(render_left(engine, 50));
                }
                let gain = gain_between(&waves[0], &waves[1], 0, first_10_ms);
                places.push(1.0 - gain);
            }

            let mut lowest = 1.0;
            let mut highest = 0.0;
            for place in &places {
                lowest = f64::min(lowest, *place);
                highest = f64::max(highest, *place);
            }
            let case = format!("start {start}%, spread {spread}%");
            assert!(lowest > share(start) - 0.0002, "{case}: one started at {lowest}");
            assert!(highest < share(start) + share(spread) + 0.0002, "{case}: one started at {highest}");
            // And they are spread over that, or all on the start point.
            if spread > 0 {
                assert!(highest - lowest > 0.6 * share(spread), "{case}: all started near {lowest}");
            } else {
                assert!(highest - lowest < 0.0002, "{case}: they started apart");
            }
        }
    }

    // How far an LFO has faded in at `age`, after waiting `delay` and then
    // growing over `fade`, all three in ms or all in beats (Routing::fade).
    fn faded_in(age: f64, delay: f64, fade: f64) -> f64 {
        if age < delay {
            return 0.0;
        }
        if fade <= 0.0 {
            return 1.0;
        }
        let grown = (age - delay) / fade;
        grown.min(1.0)
    }

    #[test]
    fn an_lfo_waits_out_its_delay_then_fades_in() {
        // In ms: nothing for 100 ms, then it grows to its full size in a
        // straight line over 200 ms.
        let lfo = Lfo {
            delay_ms: 100,
            fade_ms: 200,
            ..lfo_at(LfoShape::Sine, 3.31)
        };
        let dip = dip_by(Source::Lfo(0));
        let (moved, plain) = a4_routed(&[(0, lfo)], &[dip], 500);
        assert_gain("in ms", &moved, &plain, |t| {
            let age_ms = t as f64 * 1000.0 / SAMPLES_PER_SECOND;
            let grown = faded_in(age_ms, 100.0, 200.0);
            let height = drawn_height(&lfo, cycles_after(lfo.hz, t));
            1.0 - grown * (height + 1.0) / 2.0
        });
        // So for its first 100 ms, A4 is exactly as it was.
        let delay = frames_in_ms(&Engine::new().unwrap(), 100);
        assert_same_samples(&moved[..delay], &plain[..delay]);

        // Moving the pitch, which can go up as well as down, it leaves A4
        // alone for the whole delay too.
        let vibrato = connection(Source::Lfo(0), Target::Pitch, 100.0);
        let (moved, plain) = a4_routed(&[(0, lfo)], &[vibrato], 100);
        assert_same_samples(&moved, &plain);

        // In beats, it counts beats and leaves its ms out: at 120, half a
        // beat of delay is 250 ms, and a beat of fade-in 500 ms.
        let lfo = Lfo {
            speed: LfoSpeed::Beats,
            beats: 0.5,
            delay_beats: 0.5,
            fade_beats: 1.0,
            ..lfo
        };
        let (moved, plain) = a4_routed(&[(0, lfo)], &[dip], 1000);
        assert_gain("in beats", &moved, &plain, |t| {
            let beats = t as f64 / BEAT_AT_120 as f64;
            let grown = faded_in(beats, 0.5, 1.0);
            let height = drawn_height(&lfo, beats / lfo.beats);
            1.0 - grown * (height + 1.0) / 2.0
        });
    }

    #[test]
    fn once_runs_one_cycle_then_holds_where_it_ended() {
        // A saw up at 4 Hz, starting 27% in, rises, jumps back and rises
        // again to just short of where it started, then holds there.
        let lfo = Lfo {
            start: 27,
            once: true,
            ..lfo_at(LfoShape::SawUp, 4.0)
        };
        let dip = dip_by(Source::Lfo(0));
        let (moved, plain) = a4_routed(&[(0, lfo)], &[dip], 600);
        let end = share(lfo.start) + 1.0 - ONCE_END_NUDGE;
        assert_gain("per note", &moved, &plain, |t| {
            let cycles = share(lfo.start) + cycles_after(lfo.hz, t);
            gain_at_height(drawn_height(&lfo, f64::min(cycles, end)))
        });

        // A shared LFO has no note to start with, so it runs on.
        let shared = Lfo { per_note: false, ..lfo };
        let (moved, plain) = a4_routed(&[(0, shared)], &[dip], 600);
        assert_gain("shared", &moved, &plain, |t| {
            let cycles = share(shared.start) + cycles_after(shared.hz, t);
            gain_at_height(drawn_height(&shared, cycles))
        });
    }

    #[test]
    fn an_lfo_in_beats_runs_at_the_song_s_tempo() {
        // Two beats a cycle: a second at 120, and a second and a third at
        // 90. The beat need not be running.
        let lfo = Lfo {
            speed: LfoSpeed::Beats,
            beats: 2.0,
            ..lfo_at(LfoShape::Sine, 1.0)
        };
        for tempo in [120, 90] {
            let mut routed = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]);
            routed.set_tempo(tempo);
            let mut plain = Engine::new().unwrap();
            let mut waves = Vec::new();
            for engine in [&mut routed, &mut plain] {
                engine.note_on(A4, VELOCITY);
                waves.push(render_left(engine, 1000));
            }
            let beats_per_sample = tempo as f64 / (60.0 * SAMPLES_PER_SECOND);
            assert_gain(&format!("at {tempo}"), &waves[0], &waves[1], |t| {
                let beats = t as f64 * beats_per_sample;
                gain_at_height(drawn_height(&lfo, beats / lfo.beats))
            });
        }
    }

    // Plays A4 from the first sample, if `a4`, starts the click on sample
    // `beat_1`, and drops the tempo to 60 halfway through that beat. Returns
    // `frames` frames, left side.
    fn click_then_60(engine: &mut Engine, a4: bool, beat_1: usize, frames: usize) -> Vec<f32> {
        if a4 {
            engine.note_on(A4, VELOCITY);
        }
        let mut wave = render_frames_left(engine, beat_1, usize::MAX);
        engine.start_click();
        wave.extend(render_frames_left(engine, BEAT_AT_120 / 2, usize::MAX));
        engine.set_tempo(60);
        let rest = frames - wave.len();
        wave.extend(render_frames_left(engine, rest, usize::MAX));
        wave
    }

    // `wave` with `other` taken away, sample by sample.
    fn without(wave: &[f32], other: &[f32]) -> Vec<f32> {
        let mut left = Vec::new();
        for i in 0..wave.len() {
            left.push(wave[i] - other[i]);
        }
        left
    }

    #[test]
    fn a_shared_lfo_in_beats_starts_on_beat_1_and_lines_up_with_every_beat() {
        // Shared, a beat a cycle, it runs from the engine's start at the
        // tempo. When the click starts beat 1, 300 ms in, it starts its cycle
        // again, gliding there over 20 ms, so it never clicks. Halfway
        // through that beat the tempo drops to 60, and from the next beat on
        // each cycle lasts as long as that beat: 2 seconds.
        let lfo = Lfo {
            per_note: false,
            speed: LfoSpeed::Beats,
            beats: 1.0,
            ..lfo_at(LfoShape::Sine, 1.0)
        };
        let beat_1 = 14_400;
        let beat_2 = beat_1 + BEAT_AT_120;
        let beat_at_60 = 2 * BEAT_AT_120;
        let frames = beat_2 + beat_at_60;
        let routed = click_then_60(&mut routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]), true, beat_1, frames);
        let plain = click_then_60(&mut Engine::new().unwrap(), true, beat_1, frames);
        let click = click_then_60(&mut Engine::new().unwrap(), false, beat_1, frames);

        // What the routing list moved is A4, so the click is left out.
        let where_it_was = drawn_height(&lfo, beat_1 as f64 / BEAT_AT_120 as f64);
        let gap = where_it_was - drawn_height(&lfo, 0.0);
        assert_gain("shared in beats", &without(&routed, &click), &without(&plain, &click), |t| {
            if t <= beat_1 {
                return gain_at_height(drawn_height(&lfo, t as f64 / BEAT_AT_120 as f64));
            }
            if t <= beat_2 {
                let since = t - beat_1;
                let mut glide = 0.0;
                if since < SHARED_GLIDE_SAMPLES {
                    glide = gap * (SHARED_GLIDE_SAMPLES - since) as f64 / SHARED_GLIDE_SAMPLES as f64;
                }
                let height = drawn_height(&lfo, since as f64 / BEAT_AT_120 as f64);
                return gain_at_height(height + glide);
            }
            let since = t - beat_2;
            gain_at_height(drawn_height(&lfo, 1.0 + since as f64 / beat_at_60 as f64))
        });

        // A beat that lands partway through a block of 32 samples counts
        // from where it really was: 10 samples later, once the glide is
        // over, the LFO is where that beat puts it, until beat 2.
        let beat_1 = 14_410;
        let beat_2 = beat_1 + BEAT_AT_120;
        let routed = click_then_60(&mut routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]), true, beat_1, frames);
        let plain = click_then_60(&mut Engine::new().unwrap(), true, beat_1, frames);
        let click = click_then_60(&mut Engine::new().unwrap(), false, beat_1, frames);
        let glide_over = beat_1 + SHARED_GLIDE_SAMPLES;
        let from = glide_over - glide_over % CONTROL_SAMPLES + CONTROL_SAMPLES;
        let to = beat_2 - beat_2 % CONTROL_SAMPLES;
        let moved = without(&routed, &click);
        let plain = without(&plain, &click);
        assert_gain("beat partway through a block", &moved[from..to], &plain[from..to], |t| {
            let since = from + t - beat_1;
            gain_at_height(drawn_height(&lfo, since as f64 / BEAT_AT_120 as f64))
        });
    }

    #[test]
    fn an_lfo_in_presses_moves_on_a_step_each_key_press_and_a_chord_is_one_press() {
        // Per note, four steps and four presses a cycle, so each press holds
        // the next step for as long as its notes sound: A4, the C chord, A4
        // and A4 again, then round to the first step.
        let lfo = with_steps(
            Lfo {
                speed: LfoSpeed::Presses,
                presses: 4,
                ..lfo_at(LfoShape::Steps, 1.0)
            },
            &[-100.0, -50.0, 50.0, 100.0],
        );
        let mut routed = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]);
        let mut plain = Engine::new().unwrap();
        let presses: [&[u8]; 5] = [&[A4], C_CHORD, &[A4], &[A4], &[A4]];
        let heights = [-1.0, -0.5, 0.5, 1.0, -1.0];
        for (notes, height) in presses.iter().zip(heights) {
            let mut waves = Vec::new();
            for engine in [&mut routed, &mut plain] {
                for note in *notes {
                    engine.note_on(*note, VELOCITY);
                }
                let mut wave = render_left(engine, 100);
                for note in *notes {
                    engine.note_off(*note);
                }
                wave.extend(render_left(engine, RELEASE_MS + 100));
                waves.push(wave);
            }
            assert_gain(&format!("at {height}"), &waves[0], &waves[1], |_| gain_at_height(height));
        }
    }

    #[test]
    fn a_shared_lfo_in_presses_slides_every_held_note_to_each_press_s_step() {
        // Shared, a press moves every note its player holds to the press's
        // step, sliding there over the smoothing's share of a beat: at 50%
        // and 120, 250 ms. A4 is held throughout, C5 pressed 500 ms in, and
        // the C chord, one press, at a second. Softly, so five notes stay
        // below the soft clip.
        let lfo = with_steps(
            Lfo {
                per_note: false,
                speed: LfoSpeed::Presses,
                presses: 4,
                smooth: 50,
                ..lfo_at(LfoShape::Steps, 1.0)
            },
            &[-100.0, -50.0, 50.0, 100.0],
        );
        let mut routed = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]);
        let mut plain = Engine::new().unwrap();
        let presses: [&[u8]; 3] = [&[A4], &[C5], C_CHORD];
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut plain] {
            let mut wave = Vec::new();
            for notes in presses {
                for note in notes {
                    engine.note_on(*note, SOFT);
                }
                wave.extend(render_left(engine, 500));
            }
            waves.push(wave);
        }
        let slide = (BEAT_AT_120 / 2) as f64;
        assert_gain("shared presses", &waves[0], &waves[1], |t| {
            let mut height = -1.0;
            if t >= 24_000 {
                height = slid(-1.0, -0.5, (t - 24_000) as f64 / slide, 1.0);
            }
            if t >= 48_000 {
                height = slid(-0.5, 0.5, (t - 48_000) as f64 / slide, 1.0);
            }
            gain_at_height(height)
        });
    }

    #[test]
    fn the_song_counts_its_own_presses_from_its_start_each_time_it_plays() {
        // Per note, eight steps and eight presses a cycle, so each of the
        // song's four chords holds a step of its own. Notes fade out in 1
        // ms, so soon after each chord starts it sounds alone.
        let lfo = with_steps(
            Lfo {
                speed: LfoSpeed::Presses,
                presses: 8,
                ..lfo_at(LfoShape::Steps, 1.0)
            },
            &[-100.0, -75.0, -50.0, -25.0, 25.0, 50.0, 75.0, 100.0],
        );
        let quick = loudness(ATTACK_MS, DECAY_MS, 100, LOUDNESS_SHORTEST_MS);
        let mut routed = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0))]);
        let mut plain = Engine::new().unwrap();
        let length = song_samples(SONG);
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut plain] {
            // The song, a live A4 after it, then the song again.
            engine.set_loudness_envelope(quick);
            engine.play_song(&song_chords(SONG)).unwrap();
            let mut wave = render_frames_left(engine, length + 4800, usize::MAX);
            engine.note_on(A4, VELOCITY);
            wave.extend(render_left(engine, 100));
            engine.note_off(A4);
            wave.extend(render_left(engine, 100));
            engine.play_song(&song_chords(SONG)).unwrap();
            wave.extend(render_frames_left(engine, length, usize::MAX));
            waves.push(wave);
        }

        // Both times, chord n holds step n: the live A4 counts apart, and
        // the song counts from its start again.
        let second = length + 4800 + 9600;
        for play_start in [0, second] {
            let mut chord_start = play_start;
            for (index, test_chord) in SONG.iter().enumerate() {
                let chord_end = chord_start + test_chord.ms as usize * SAMPLES_PER_MS;
                let from = chord_start + 4 * CONTROL_SAMPLES;
                let height = step_height(&lfo, index);
                let moved = &waves[0][from..chord_end];
                let plain_wave = &waves[1][from..chord_end];
                assert_gain(&format!("chord {index} from {play_start}"), moved, plain_wave, |_| {
                    gain_at_height(height)
                });
                chord_start = chord_end;
            }
        }
    }

    // A4 for `ms` on an engine set up by `set_up`, pressed 1 ms in, then let
    // go for 400 ms more: the left side and the right.
    fn a4_after(set_up: impl Fn(&mut Engine), ms: u32) -> (Vec<f32>, Vec<f32>) {
        let mut engine = Engine::new().unwrap();
        set_up(&mut engine);
        render_stereo(&mut engine, 1);
        engine.note_on(A4, VELOCITY);
        let (mut left, mut right) = render_sides(&mut engine, ms);
        engine.note_off(A4);
        let (left_after, right_after) = render_sides(&mut engine, 400);
        left.extend(left_after);
        right.extend(right_after);
        (left, right)
    }

    // Connects the live vibrato key to `target` by `amount`, read one way,
    // in the routing list's last slot, and holds it. With no rise, from the
    // first 32 samples on it moves the target by the whole amount.
    fn hold_key_moving(engine: &mut Engine, target: Target, amount: f32) {
        let key = one_way_connection(Source::VibratoKey, target, amount);
        engine.set_connection(CONNECTION_COUNT - 1, key);
        engine.set_vibrato_key(true);
    }

    // The left sides of A4 played after 300 ms, for listening to its pitch.
    fn a4_pitch_after(set_up: impl Fn(&mut Engine)) -> Vec<f32> {
        let (left, _) = a4_after(set_up, 1300);
        let from = 300 * SAMPLES_PER_MS;
        left[from..from + 1000 * SAMPLES_PER_MS].to_vec()
    }

    #[test]
    fn snap_rounds_an_oscillator_s_pitch_to_half_steps() {
        // 130 cents up, snapped, is one half step: just like tuning the
        // oscillator a half step up.
        let (left, right) = a4_after(
            |engine| {
                let snapped = Connection {
                    snap: true,
                    ..one_way_connection(Source::VibratoKey, Target::OscillatorPitch(0), 130.0)
                };
                engine.set_connection(CONNECTION_COUNT - 1, snapped);
                engine.set_vibrato_key(true);
            },
            500,
        );
        let (expected_left, expected_right) = a4_after(
            |engine| {
                let half_step_up = Oscillator {
                    half_steps: 1,
                    ..START_OSCILLATORS[0]
                };
                engine.set_oscillator(0, half_step_up);
            },
            500,
        );
        assert_nearly_same_sound(&left, &expected_left, 0.00001);
        assert_nearly_same_sound(&right, &expected_right, 0.00001);
    }

    #[test]
    fn the_pitch_moves_by_cents_and_the_loudness_and_pan_by_shares() {
        // 1200 cents is an octave up, and -1200 one down.
        let engine = Engine::new().unwrap();
        for (cents, hz) in [(1200.0, 880.0), (-1200.0, 220.0), (200.0, B4_HZ)] {
            let wave = a4_pitch_after(|engine| hold_key_moving(engine, Target::Pitch, cents));
            assert_pitch(&engine, &wave, hz);
        }

        // -50% of the loudness is half as loud.
        let (plain_left, plain_right) = a4_after(|_| {}, 300);
        let (left, right) = a4_after(|engine| hold_key_moving(engine, Target::Loudness, -50.0), 300);
        assert_gain("left", &left, &plain_left, |_| 0.5);
        assert_gain("right", &right, &plain_right, |_| 0.5);

        // Turning it up is as loud as it gets already.
        let (left, _) = a4_after(|engine| hold_key_moving(engine, Target::Loudness, 50.0), 300);
        assert_same_samples(&left, &plain_left);

        // 50% of the pan is halfway to the right. Each side gets the square
        // root of its share, so A4 is as loud wherever it sits.
        let (left, right) = a4_after(|engine| hold_key_moving(engine, Target::Pan, 50.0), 300);
        assert_gain("left", &left, &plain_left, |_| 0.5_f64.sqrt());
        assert_gain("right", &right, &plain_right, |_| 1.5_f64.sqrt());

        // And -100% is all the way left.
        let (left, right) = a4_after(|engine| hold_key_moving(engine, Target::Pan, -100.0), 300);
        assert_gain("left", &left, &plain_left, |_| 2.0_f64.sqrt());
        assert_eq!(peak(&right), 0.0);
    }

    // The second oscillator, playing `kind` at half level, right on the note.
    fn half_level(kind: Kind) -> Oscillator {
        Oscillator {
            kind,
            level: 50,
            octaves: 0,
            half_steps: 0,
            cents: 0,
        }
    }

    // A saw with a triangle at half level as its second oscillator, through
    // the filter at 1000 Hz with a little resonance, a filter envelope, a
    // loudness envelope that falls to 80%, and 3 copies.
    fn set_up_everything(engine: &mut Engine) {
        engine.set_first_kind(Kind::Saw);
        engine.set_oscillator(1, half_level(Kind::Triangle));
        engine.set_filter(filter(1000.0, 20));
        engine.set_filter_envelope(filter_envelope(2.0, 20, 100, 50, 200));
        engine.set_loudness_envelope(loudness(10, 100, 80, 200));
        engine.set_unison(unison(3, 10, 30));
    }

    #[test]
    fn moving_a_live_setting_sounds_like_setting_it_there() {
        // Each target moved by the key sounds just like an engine with that
        // setting moved there, within rounding.
        let cases: [(Target, f32, fn(&mut Engine)); 13] = [
            // Each oscillator's level and pitch: the saw's down by 40%, the
            // triangle's up by 30% and the empty third's up from silent, and
            // the saw 7 half steps up.
            (Target::OscillatorLevel(0), -40.0, |engine| {
                let quieter = Oscillator {
                    level: 60,
                    ..half_level(Kind::Saw)
                };
                engine.set_oscillator(0, quieter);
            }),
            (Target::OscillatorLevel(1), 30.0, |engine| {
                let louder = Oscillator {
                    level: 80,
                    ..half_level(Kind::Triangle)
                };
                engine.set_oscillator(1, louder);
            }),
            (Target::OscillatorLevel(2), 25.0, |engine| {
                let quiet = Oscillator {
                    level: 25,
                    ..START_OSCILLATORS[0]
                };
                engine.set_oscillator(2, quiet);
            }),
            (Target::OscillatorPitch(0), 700.0, |engine| {
                let fifth_up = Oscillator {
                    level: FULL_PERCENT,
                    half_steps: 7,
                    ..half_level(Kind::Saw)
                };
                engine.set_oscillator(0, fifth_up);
            }),
            (Target::Cutoff, 1.0, |engine| {
                engine.set_filter(filter(2000.0, 20));
            }),
            (Target::Resonance, 50.0, |engine| {
                engine.set_filter(filter(1000.0, 70));
            }),
            (Target::KeyTracking, -50.0, |engine| {
                engine.set_key_tracking(50);
            }),
            (Target::FilterAmount, 1.5, |engine| {
                engine.set_filter_envelope(filter_envelope(3.5, 20, 100, 50, 200));
            }),
            (Target::FilterSustain, -30.0, |engine| {
                engine.set_filter_envelope(filter_envelope(2.0, 20, 100, 20, 200));
            }),
            (Target::LoudnessSustain, -40.0, |engine| {
                engine.set_loudness_envelope(loudness(10, 100, 40, 200));
            }),
            (Target::Detune, 20.0, |engine| {
                engine.set_unison(unison(3, 30, 30));
            }),
            (Target::Spread, 50.0, |engine| {
                engine.set_unison(unison(3, 10, 80));
            }),
            // Past its edge, a setting stops there.
            (Target::Spread, 100.0, |engine| {
                engine.set_unison(unison(3, 10, 100));
            }),
        ];
        for (target, amount, set_there) in cases {
            let (left, right) = a4_after(
                |engine| {
                    set_up_everything(engine);
                    hold_key_moving(engine, target, amount);
                },
                500,
            );
            let (expected_left, expected_right) = a4_after(
                |engine| {
                    set_up_everything(engine);
                    set_there(engine);
                },
                500,
            );
            let difference = f32::max(
                biggest_difference(&left, &expected_left),
                biggest_difference(&right, &expected_right),
            );
            assert!(
                difference < 0.00001,
                "{} moved by {amount} differs by {difference}",
                target.name()
            );
        }

        // Moving the cutoff brings the filter in even while it is open, so
        // 6 octaves down from 20,000 Hz is a filter at 312.5 Hz.
        let (left, _) = a4_after(
            |engine| {
                engine.set_first_kind(Kind::Saw);
                hold_key_moving(engine, Target::Cutoff, -6.0);
            },
            500,
        );
        let (expected, _) = a4_after(
            |engine| {
                engine.set_first_kind(Kind::Saw);
                engine.set_filter(filter(312.5, 0));
            },
            500,
        );
        assert_nearly_same_sound(&left, &expected, 0.00001);
    }

    #[test]
    fn moving_a_setting_read_as_a_note_starts_sounds_exactly_like_setting_it_there() {
        let cases: [(Target, f32, fn(&mut Engine)); 14] = [
            // From the saw, a step on is the noise, two go round to the sine,
            // and two back land on the triangle.
            (Target::OscillatorKind(0), 1.0, |engine| {
                engine.set_first_kind(Kind::Noise);
            }),
            (Target::OscillatorKind(0), 2.0, |engine| {
                engine.set_first_kind(Kind::Sine);
            }),
            (Target::OscillatorKind(0), -2.0, |engine| {
                engine.set_first_kind(Kind::Triangle);
            }),
            // The second oscillator's kind, from its triangle, at half level.
            (Target::OscillatorKind(1), 1.0, |engine| {
                engine.set_oscillator(1, half_level(Kind::Square));
            }),
            (Target::Copies, 2.0, |engine| {
                engine.set_unison(unison(5, 10, 30));
            }),
            // Copies stop at 7.
            (Target::Copies, 6.0, |engine| {
                engine.set_unison(unison(7, 10, 30));
            }),
            // Each octave of time doubles it, and halves it below 0.
            (Target::LoudnessAttack, 1.0, |engine| {
                engine.set_loudness_envelope(loudness(20, 100, 80, 200));
            }),
            (Target::LoudnessDecay, -1.0, |engine| {
                engine.set_loudness_envelope(loudness(10, 50, 80, 200));
            }),
            (Target::LoudnessRelease, 1.0, |engine| {
                engine.set_loudness_envelope(loudness(10, 100, 80, 400));
            }),
            // A time stops at its shortest.
            (Target::LoudnessAttack, -14.0, |engine| {
                engine.set_loudness_envelope(loudness(LOUDNESS_SHORTEST_MS, 100, 80, 200));
            }),
            (Target::FilterAttack, 2.0, |engine| {
                engine.set_filter_envelope(filter_envelope(2.0, 80, 100, 50, 200));
            }),
            (Target::FilterDecay, 1.0, |engine| {
                engine.set_filter_envelope(filter_envelope(2.0, 20, 200, 50, 200));
            }),
            (Target::FilterRelease, -1.0, |engine| {
                engine.set_filter_envelope(filter_envelope(2.0, 20, 100, 50, 100));
            }),
            (Target::FilterAttack, -14.0, |engine| {
                engine.set_filter_envelope(filter_envelope(2.0, 0, 100, 50, 200));
            }),
        ];
        for (target, amount, set_there) in cases {
            let (left, right) = a4_after(
                |engine| {
                    set_up_everything(engine);
                    hold_key_moving(engine, target, amount);
                },
                300,
            );
            let (expected_left, expected_right) = a4_after(
                |engine| {
                    set_up_everything(engine);
                    set_there(engine);
                },
                300,
            );
            let case = format!("{} moved by {amount}", target.name());
            assert!(left == expected_left, "{case} sounds different on the left");
            assert!(right == expected_right, "{case} sounds different on the right");
        }

        // A note already sounding keeps them: held after A4 starts, the key
        // leaves its wave alone.
        let mut moved = Engine::new().unwrap();
        let mut plain = Engine::new().unwrap();
        let mut waves = Vec::new();
        for engine in [&mut moved, &mut plain] {
            engine.note_on(A4, VELOCITY);
            render_left(engine, 100);
        }
        hold_key_moving(&mut moved, Target::OscillatorKind(0), 1.0);
        for engine in [&mut moved, &mut plain] {
            waves.push(render_left(engine, 300));
        }
        assert_same_samples(&waves[0], &waves[1]);
    }

    #[test]
    fn a_setting_read_as_a_note_starts_takes_the_whole_move_with_no_fade_in() {
        // LFO 1 holds at its top but fades in over a second. The copies are
        // read as a note starts, so they take its whole move all the same:
        // 2 more copies sound just like 5 set there.
        let lfo = Lfo {
            fade_ms: 1000,
            ..with_steps(lfo_at(LfoShape::Steps, 1.0), &[100.0])
        };
        let (left, right) = a4_after(
            |engine| {
                set_up_everything(engine);
                engine.set_lfo(0, lfo);
                engine.set_connection(0, connection(Source::Lfo(0), Target::Copies, 2.0));
            },
            300,
        );
        let (expected_left, expected_right) = a4_after(
            |engine| {
                set_up_everything(engine);
                engine.set_unison(unison(5, 10, 30));
            },
            300,
        );
        assert!(left == expected_left, "the copies sound different on the left");
        assert!(right == expected_right, "the copies sound different on the right");
    }

    #[test]
    fn moving_an_lfo_s_speed_an_octave_up_doubles_it() {
        // Per note, at 2 Hz, LFO 2 moved up an octave runs at 4, in Hz or
        // in beats.
        let dip = dip_by(Source::Lfo(1));
        let in_beats = Lfo {
            speed: LfoSpeed::Beats,
            beats: 1.0,
            ..lfo_at(LfoShape::Sine, 1.0)
        };
        let cases = [
            (lfo_at(LfoShape::Sine, 2.0), lfo_at(LfoShape::Sine, 4.0)),
            (in_beats, Lfo { beats: 0.5, ..in_beats }),
        ];
        for (lfo, twice_as_fast) in cases {
            let (moved, _) = a4_after(
                |engine| {
                    engine.set_lfo(1, lfo);
                    engine.set_connection(0, dip);
                    hold_key_moving(engine, Target::LfoSpeed(1), 1.0);
                },
                1000,
            );
            let (expected, _) = a4_after(
                |engine| {
                    engine.set_lfo(1, twice_as_fast);
                    engine.set_connection(0, dip);
                },
                1000,
            );
            assert_same_samples(&moved, &expected);
        }

        // A shared LFO's speed is moved only by shared LFOs, since every
        // note shares it, and with no fade-in. LFO 3, a square so slow it
        // stays at its top, moves LFO 2 an octave up from its first 32
        // samples on.
        let shared = Lfo {
            per_note: false,
            ..lfo_at(LfoShape::Sine, 2.0)
        };
        let at_the_top = Lfo {
            per_note: false,
            ..lfo_at(LfoShape::Square, 0.01)
        };
        let speeding = connection(Source::Lfo(2), Target::LfoSpeed(1), 1.0);
        let (moved, plain) = a4_routed(&[(1, shared), (2, at_the_top)], &[dip, speeding], 1000);
        assert_gain("shared", &moved, &plain, |t| {
            let mut cycles = cycles_after(2.0, t);
            if t > CONTROL_SAMPLES {
                cycles = cycles_after(2.0, CONTROL_SAMPLES) + cycles_after(4.0, t - CONTROL_SAMPLES);
            }
            gain_at_height(drawn_height(&shared, cycles))
        });

        // The vibrato key belongs to each player, so it moves no shared
        // LFO's speed.
        let key_speeding = one_way_connection(Source::VibratoKey, Target::LfoSpeed(1), 1.0);
        let mut routed = routed_engine(&[(1, shared)], &[dip, key_speeding]);
        routed.set_vibrato_key(true);
        let mut plain = Engine::new().unwrap();
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut plain] {
            engine.note_on(A4, VELOCITY);
            waves.push(render_left(engine, 1000));
        }
        assert_gain("shared, by the key", &waves[0], &waves[1], |t| {
            gain_at_height(drawn_height(&shared, cycles_after(2.0, t)))
        });
    }

    // How far a note's envelope is after `samples` samples, as a share of
    // its peak, while it is held: it rises over `attack` samples, then
    // falls over `decay` to `sustain`.
    fn envelope_after(samples: usize, attack: usize, decay: usize, sustain: f64) -> f64 {
        if samples < attack {
            return samples as f64 / attack as f64;
        }
        let into_decay = samples - attack;
        if into_decay < decay {
            return 1.0 - (1.0 - sustain) * into_decay as f64 / decay as f64;
        }
        sustain
    }

    #[test]
    fn every_other_source_reads_from_minus_1_to_1() {
        // The note's height: 0 at middle C, -1 five octaves below and 1 five
        // above, and no further.
        let notes = [(MIDDLE_C, 0.0), (A4, 0.15), (C3, -0.2), (0, -1.0), (120, 1.0), (127, 1.0)];
        for (note, height) in notes {
            let mut routed = routed_engine(&[], &[dip_by(Source::NoteHeight)]);
            let mut plain = Engine::new().unwrap();
            let mut waves = Vec::new();
            for engine in [&mut routed, &mut plain] {
                engine.note_on(note, VELOCITY);
                waves.push(render_left(engine, 200));
            }
            assert_gain(&format!("note {note}"), &waves[0], &waves[1], |_| gain_at_height(height));
        }

        // A random number each note starts with: the same on every engine,
        // and another for the next note.
        let mut routed = routed_engine(&[], &[dip_by(Source::Random)]);
        let mut again = routed_engine(&[], &[dip_by(Source::Random)]);
        let mut plain = Engine::new().unwrap();
        let mut waves = Vec::new();
        for engine in [&mut routed, &mut again, &mut plain] {
            let mut wave = Vec::new();
            for _ in 0..2 {
                engine.note_on(A4, VELOCITY);
                wave.extend(render_left(engine, 200));
                engine.note_off(A4);
                wave.extend(render_left(engine, RELEASE_MS + 100));
            }
            waves.push(wave);
        }
        assert_same_samples(&waves[1], &waves[0]);
        let note_samples = (200 + RELEASE_MS as usize + 100) * SAMPLES_PER_MS;
        let first = gain_between(&waves[0], &waves[2], 0, note_samples);
        let second = gain_between(&waves[0], &waves[2], note_samples, 2 * note_samples);
        assert_gain("first note", &waves[0][..note_samples], &waves[2][..note_samples], |_| first);
        assert_gain("second note", &waves[0][note_samples..], &waves[2][note_samples..], |_| second);
        assert!(first != second, "both notes got {first}");
        for gain in [first, second] {
            assert!(gain >= 0.0, "a random number of {}", 1.0 - 2.0 * gain);
            assert!(gain <= 1.0, "a random number of {}", 1.0 - 2.0 * gain);
        }

        // Where the note's loudness envelope is, as a share of its peak, and
        // where its filter envelope is. Each is read before the 32 samples
        // its move heads into, so the move comes 32 samples after it.
        let attack = 100 * SAMPLES_PER_MS;
        let decay = 100 * SAMPLES_PER_MS;
        for source in [Source::LoudnessEnvelope, Source::FilterEnvelope] {
            let mut routed = routed_engine(&[], &[dip_by(source)]);
            let mut plain = Engine::new().unwrap();
            let mut waves = Vec::new();
            for engine in [&mut routed, &mut plain] {
                engine.set_loudness_envelope(loudness(100, 100, 50, 200));
                engine.set_filter_envelope(filter_envelope(0.0, 100, 100, 50, 200));
                engine.note_on(A4, VELOCITY);
                waves.push(render_left(engine, 400));
            }
            assert_gain(&source.name(), &waves[0], &waves[1], |t| {
                if t < CONTROL_SAMPLES {
                    return 1.0;
                }
                let share = envelope_after(t - CONTROL_SAMPLES, attack, decay, 0.5);
                gain_at_height(2.0 * share - 1.0)
            });
        }
    }

    // Where the vibrato key is on sample `t`, from 0 (up) to 1 (held): held
    // on sample `held` and let go on `let_go`, rising over `rise` samples
    // and falling over `fall` from wherever it got to. It moves on every 32
    // samples, so these tests press it on one of those.
    fn key_at(t: usize, held: usize, let_go: usize, rise: usize, fall: usize) -> f64 {
        if t <= held {
            return 0.0;
        }
        let held_for = usize::min(t, let_go) - held;
        let risen = f64::min(held_for as f64 / rise as f64, 1.0);
        if t <= let_go {
            return risen;
        }
        let fallen = (t - let_go) as f64 / fall as f64;
        f64::max(risen - fallen, 0.0)
    }

    // A4 played from the first sample with `steps` on a routed engine, and
    // A4 alone on a new one: the left sides, moved and plain.
    fn a4_with_steps(routed: &mut Engine, steps: &[(usize, Step)], frames: usize) -> (Vec<f32>, Vec<f32>) {
        let mut all_steps = vec![(0, NoteOn(A4))];
        all_steps.extend_from_slice(steps);
        let moved = play(routed, &all_steps, frames);
        let plain = play(&mut Engine::new().unwrap(), &[(0, NoteOn(A4))], frames);
        (moved, plain)
    }

    #[test]
    fn the_vibrato_key_rises_and_falls_in_straight_lines() {
        // Rising over 100 ms and falling over 200, the key turns A4 down all
        // the way while held. Held 96 ms in and let go 496 ms in, it is all
        // the way down by 196 ms and back up by 696. Let go halfway up, it
        // falls from there.
        let rise = 100 * SAMPLES_PER_MS;
        let fall = 200 * SAMPLES_PER_MS;
        let times = VibratoKeyTimes {
            rise_ms: 100,
            fall_ms: 200,
        };
        for (held, let_go) in [(4_608, 23_808), (4_608, 4_608 + rise / 2)] {
            let mut routed = routed_engine(&[], &[dip_by(Source::VibratoKey)]);
            routed.set_vibrato_key_times(times);
            let steps = [(held, HoldVibratoKey), (let_go, LetGoOfVibratoKey)];
            let (moved, plain) = a4_with_steps(&mut routed, &steps, 40_000);
            assert_gain(&format!("held at {held}, let go at {let_go}"), &moved, &plain, |t| {
                1.0 - key_at(t, held, let_go, rise, fall)
            });
        }
    }

    #[test]
    fn a_connection_scaled_by_another_source_moves_by_that_share_of_its_amount() {
        // LFO 1 turns A4 down, scaled by the vibrato key: not at all until
        // the key is held, 96 ms in, then more and more as it rises over 100
        // ms, and fully from then on. This is how the vibrato key brings in
        // a vibrato.
        let lfo = lfo_at(LfoShape::Sine, 3.31);
        let by_the_key = Connection {
            scaled_by: Source::VibratoKey,
            ..dip_by(Source::Lfo(0))
        };
        let held = 4_608;
        let rise = 100 * SAMPLES_PER_MS;
        let mut routed = routed_engine(&[(0, lfo)], &[by_the_key]);
        routed.set_vibrato_key_times(VibratoKeyTimes {
            rise_ms: 100,
            fall_ms: 0,
        });
        let (moved, plain) = a4_with_steps(&mut routed, &[(held, HoldVibratoKey)], 24_000);
        assert_gain("by the key", &moved, &plain, |t| {
            let key = key_at(t, held, usize::MAX, rise, 1);
            let height = drawn_height(&lfo, cycles_after(lfo.hz, t));
            1.0 - key * (height + 1.0) / 2.0
        });

        // Scaled by LFO 2, a square so slow it stays at its top, which waits
        // 100 ms and then fades in over 200: the dip fades in with it.
        let at_the_top = Lfo {
            delay_ms: 100,
            fade_ms: 200,
            ..lfo_at(LfoShape::Square, 0.01)
        };
        let by_lfo_2 = Connection {
            scaled_by: Source::Lfo(1),
            ..dip_by(Source::Lfo(0))
        };
        let (moved, plain) = a4_routed(&[(0, lfo), (1, at_the_top)], &[by_lfo_2], 500);
        assert_gain("by lfo 2", &moved, &plain, |t| {
            let age_ms = t as f64 * 1000.0 / SAMPLES_PER_SECOND;
            let grown = faded_in(age_ms, 100.0, 200.0);
            let height = drawn_height(&lfo, cycles_after(lfo.hz, t));
            1.0 - grown * (height + 1.0) / 2.0
        });
    }

    #[test]
    fn moves_on_one_target_add_up_and_the_pitch_stops_4_octaves_away() {
        // Two connections turning A4 down 30% each, in the routing list's
        // first slot and its last, turn it down 60%.
        let (plain, _) = a4_after(|_| {}, 300);
        let (moved, _) = a4_after(
            |engine| {
                engine.set_connection(0, one_way_connection(Source::VibratoKey, Target::Loudness, -30.0));
                hold_key_moving(engine, Target::Loudness, -30.0);
            },
            300,
        );
        assert_gain("two dips", &moved, &plain, |_| 0.4);

        // Two connections each moving a sine 3000 cents up add up to 6000,
        // which stops at 4800: 4 octaves above A4. And the same down.
        let engine = Engine::new().unwrap();
        for (cents, hz) in [(3000.0, 7040.0), (-3000.0, 27.5)] {
            let wave = a4_pitch_after(|engine| {
                engine.set_first_kind(Kind::Sine);
                engine.set_connection(0, one_way_connection(Source::VibratoKey, Target::Pitch, cents));
                hold_key_moving(engine, Target::Pitch, cents);
            });
            assert_pitch(&engine, &wave, hz);
        }
    }

    // The pitch `half_steps` half steps from A4, in Hz.
    fn half_steps_from_a4(half_steps: f64) -> f64 {
        440.0 * 2.0_f64.powf(half_steps / 12.0)
    }

    #[test]
    fn snap_rounds_a_pitch_move_to_whole_half_steps() {
        // 140 cents snaps to 100, a half step up from A4, 160 to 200, two
        // half steps, and -140 to a half step down.
        let engine = Engine::new().unwrap();
        for (cents, half_steps) in [(140.0, 1.0), (160.0, 2.0), (-140.0, -1.0)] {
            let wave = a4_pitch_after(|engine| {
                let snapped = Connection {
                    snap: true,
                    ..one_way_connection(Source::VibratoKey, Target::Pitch, cents)
                };
                engine.set_connection(0, snapped);
                engine.set_vibrato_key(true);
            });
            assert_pitch(&engine, &wave, half_steps_from_a4(half_steps));
        }

        // Only the pitch snaps: half the loudness stays half, though -0.5
        // rounded to whole half steps would be 0.
        let (plain, _) = a4_after(|_| {}, 300);
        let (moved, _) = a4_after(
            |engine| {
                let snapped = Connection {
                    snap: true,
                    ..one_way_connection(Source::VibratoKey, Target::Loudness, -50.0)
                };
                engine.set_connection(0, snapped);
                engine.set_vibrato_key(true);
            },
            300,
        );
        assert_gain("snapped loudness", &moved, &plain, |_| 0.5);
    }

    // The vibrato key turning notes down by half, read one way.
    fn key_halving() -> Connection {
        one_way_connection(Source::VibratoKey, Target::Loudness, -50.0)
    }

    #[test]
    fn the_live_vibrato_key_never_moves_the_song_s_notes() {
        let frames = song_samples(SONG) + RELEASE_MS as usize * SAMPLES_PER_MS;
        let mut held = routed_engine(&[], &[key_halving()]);
        held.set_vibrato_key(true);
        let mut up = routed_engine(&[], &[key_halving()]);
        let held_wave = play(&mut held, &[(0, PlaySong(SONG))], frames);
        let up_wave = play(&mut up, &[(0, PlaySong(SONG))], frames);
        assert_same_samples(&held_wave, &up_wave);
    }

    #[test]
    fn letting_go_of_everything_lets_go_of_the_vibrato_key() {
        // When the window loses focus, the key goes up with A4, just as if
        // both had been let go, so C5, played after, is not turned down.
        let held = [(0, HoldVibratoKey), (0, NoteOn(A4)), (19_200, NoteOn(C5))];
        let mut lost_focus = held.to_vec();
        lost_focus.push((9_600, AllNotesOff));
        let mut let_go = held.to_vec();
        let_go.push((9_600, NoteOff(A4)));
        let_go.push((9_600, LetGoOfVibratoKey));
        let frames = 40_000;
        let lost_focus_wave = play(&mut routed_engine(&[], &[key_halving()]), &lost_focus, frames);
        let let_go_wave = play(&mut routed_engine(&[], &[key_halving()]), &let_go, frames);
        assert_same_samples(&lost_focus_wave, &let_go_wave);

        // Kept held, C5 would have been turned down.
        let mut kept = held.to_vec();
        kept.push((9_600, NoteOff(A4)));
        let kept_wave = play(&mut routed_engine(&[], &[key_halving()]), &kept, frames);
        assert!(kept_wave != let_go_wave, "the key never moved C5");

        // While recording, the loop's press of the key ends there too.
        let start = RECORDING_STARTS;
        let steps = [
            (0, PressLooper),
            (start + 10_000, HoldVibratoKey),
            (start + 20_000, AllNotesOff),
        ];
        let mut engine = Engine::new().unwrap();
        play(&mut engine, &steps, start + 30_000);
        let key_press = LoopNote {
            note: VIBRATO_KEY_NOTE,
            start: 10_000,
            end: Some(20_000),
        };
        assert_eq!(first_layer_notes(&mut engine), vec![key_press]);
    }

    #[test]
    fn a_loop_plays_its_vibrato_key_back_on_its_own_notes() {
        // Recorded with A4, the key plays back each time around exactly as
        // it was played, rising over 50 ms and falling over 100 as it did:
        // every sample matches A4 and the key played live each time.
        let times = VibratoKeyTimes {
            rise_ms: 50,
            fall_ms: 100,
        };
        let bar = [
            (10_000, NoteOn(A4)),
            (20_000, HoldVibratoKey),
            (50_000, LetGoOfVibratoKey),
            (70_000, NoteOff(A4)),
        ];
        let start = RECORDING_STARTS;
        let mut looped = vec![(0, PressLooper), (start + 80_000, PressLooper)];
        looped.extend(moved(&bar, start));
        let mut live = vec![(0, StartClick), (start + BAR_AT_120, StopClick)];
        for time_around in 0..3 {
            live.extend(moved(&bar, start + time_around * BAR_AT_120));
        }

        let frames = start + 3 * BAR_AT_120;
        let mut looper_engine = routed_engine(&[], &[key_halving()]);
        looper_engine.set_snap(false);
        looper_engine.set_vibrato_key_times(times);
        let mut live_engine = routed_engine(&[], &[key_halving()]);
        live_engine.set_vibrato_key_times(times);
        let looped_wave = play(&mut looper_engine, &looped, frames);
        let live_wave = play(&mut live_engine, &live, frames);
        assert_same_samples(&looped_wave, &live_wave);

        // Rust's copy keeps the key's press as a note on a pitch of its own.
        let a4 = LoopNote {
            note: A4,
            start: 10_000,
            end: Some(70_000),
        };
        let key_press = LoopNote {
            note: VIBRATO_KEY_NOTE,
            start: 20_000,
            end: Some(50_000),
        };
        assert_eq!(first_layer_notes(&mut looper_engine), vec![a4, key_press]);

        // The live key, held while the loop plays, moves none of its notes.
        let first_time = start + BAR_AT_120;
        let mut with_live_key = looped.clone();
        with_live_key.push((first_time + 5_000, HoldVibratoKey));
        with_live_key.push((first_time + 75_000, LetGoOfVibratoKey));
        let mut engine = routed_engine(&[], &[key_halving()]);
        engine.set_snap(false);
        engine.set_vibrato_key_times(times);
        let with_live_key_wave = play(&mut engine, &with_live_key, frames);
        assert_same_samples(&with_live_key_wave, &looped_wave);
    }

    // A4 from the first sample on `routed` and on a new engine, with
    // `change` made to `routed` `at` samples in: the left sides, moved and
    // plain, `frames` long.
    fn a4_changed_at(
        routed: &mut Engine,
        change: impl Fn(&mut Engine),
        at: usize,
        frames: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut plain = Engine::new().unwrap();
        routed.note_on(A4, VELOCITY);
        plain.note_on(A4, VELOCITY);
        let mut moved = render_frames_left(routed, at, usize::MAX);
        change(routed);
        moved.extend(render_frames_left(routed, frames - at, usize::MAX));
        let plain_wave = render_frames_left(&mut plain, frames, usize::MAX);
        (moved, plain_wave)
    }

    #[test]
    fn a_changed_lfo_moves_notes_already_sounding_and_a_shared_one_glides_there() {
        // Per note: 200 ms into A4, its sine at 3.31 Hz becomes a triangle
        // at 5 Hz, which goes on from the same place in its cycle.
        let lfo = lfo_at(LfoShape::Sine, 3.31);
        let changed = lfo_at(LfoShape::Triangle, 5.0);
        let change_at = 9_600;
        let dip = dip_by(Source::Lfo(0));
        let mut routed = routed_engine(&[(0, lfo)], &[dip]);
        let change = |engine: &mut Engine| {
            engine.set_lfo(0, changed);
        };
        let (moved, plain) = a4_changed_at(&mut routed, change, change_at, 24_000);
        assert_gain("per note", &moved, &plain, |t| {
            if t <= change_at {
                return gain_at_height(drawn_height(&lfo, cycles_after(lfo.hz, t)));
            }
            let cycles = cycles_after(lfo.hz, change_at) + cycles_after(changed.hz, t - change_at);
            gain_at_height(drawn_height(&changed, cycles))
        });

        // Shared: started half a cycle further on, it glides there from
        // where it was over 20 ms, so it never clicks.
        let shared = Lfo { per_note: false, ..lfo };
        let moved_on = Lfo { start: 50, ..shared };
        let mut routed = routed_engine(&[(0, shared)], &[dip]);
        let change = |engine: &mut Engine| {
            engine.set_lfo(0, moved_on);
        };
        let (moved, plain) = a4_changed_at(&mut routed, change, change_at, 24_000);
        let before = drawn_height(&shared, cycles_after(shared.hz, change_at));
        let after = drawn_height(&moved_on, share(moved_on.start) + cycles_after(shared.hz, change_at));
        let gap = before - after;
        assert_gain("shared", &moved, &plain, |t| {
            let cycles = cycles_after(shared.hz, t);
            if t <= change_at {
                return gain_at_height(drawn_height(&shared, cycles));
            }
            let since = t - change_at;
            let mut glide = 0.0;
            if since < SHARED_GLIDE_SAMPLES {
                glide = gap * (SHARED_GLIDE_SAMPLES - since) as f64 / SHARED_GLIDE_SAMPLES as f64;
            }
            gain_at_height(drawn_height(&moved_on, share(moved_on.start) + cycles) + glide)
        });
    }

    #[test]
    fn a_move_of_nothing_changes_not_a_single_sample() {
        // Every target, moved by 0 on a held key, or by its most from the
        // key while it is up, read one way. The filter is in already, since
        // moving the cutoff brings it in.
        let plain = a4_after(set_up_everything, 300);
        for target in every_target() {
            let most = target.most_amount();
            for (amount, held) in [(0.0, true), (most, false), (-most, false)] {
                let moved = a4_after(
                    |engine| {
                        set_up_everything(engine);
                        engine.set_connection(0, one_way_connection(Source::VibratoKey, target, amount));
                        engine.set_vibrato_key(held);
                    },
                    300,
                );
                assert!(
                    moved == plain,
                    "{} by {amount}, the key held {held}, changed the sound",
                    target.name()
                );
            }
        }
    }

    // Every source but none.
    fn every_source() -> Vec<Source> {
        let mut sources = Vec::new();
        for index in 0..LFO_COUNT {
            sources.push(Source::Lfo(index));
        }
        sources.push(Source::VibratoKey);
        sources.push(Source::NoteHeight);
        sources.push(Source::Random);
        sources.push(Source::LoudnessEnvelope);
        sources.push(Source::FilterEnvelope);
        sources
    }

    #[test]
    fn the_busiest_routing_list_never_blows_up() {
        // Every LFO shape, at its fastest, half of them shared, in Hz, beats
        // and presses. All 64 connections in use, from every source to every
        // target by its most either way, some scaled by another source,
        // some one way, some snapped. The key held and the beat running,
        // and 32 saws at the hardest press, the lowest and highest notes
        // among them, through full resonance.
        let mut engine = saw_engine();
        engine.set_filter(filter(200.0, 100));
        for index in 0..LFO_COUNT {
            let lfo = Lfo {
                shape: LfoShape::ALL[index],
                per_note: index % 2 == 0,
                speed: LfoSpeed::ALL[index % 3],
                hz: LFO_FASTEST_HZ,
                beats: LFO_SHORTEST_BEATS,
                presses: 1,
                smooth: 0,
                ..START_LFO
            };
            engine.set_lfo(index, with_steps(lfo, &STEP_HEIGHTS));
        }
        let sources = every_source();
        let targets = every_target();
        for index in 0..CONNECTION_COUNT {
            let target = targets[index % targets.len()];
            let mut amount = target.most_amount();
            if index % 2 == 1 {
                amount = -amount;
            }
            let mut scaled_by = Source::None;
            if index % 5 == 0 {
                scaled_by = sources[(index / 5) % sources.len()];
            }
            let busy = Connection {
                source: sources[index % sources.len()],
                target,
                amount,
                one_way: index % 3 == 0,
                snap: index % 4 == 0,
                scaled_by,
            };
            engine.set_connection(index, busy);
        }
        engine.set_vibrato_key(true);
        engine.start_click();
        engine.note_on(0, 127);
        engine.note_on(127, 127);
        for note in C3..C3 + VOICE_COUNT - 2 {
            engine.note_on(note, 127);
        }

        let samples = engine.render(frames_in_ms(&engine, 1000));
        assert_all_finite(&samples);
        assert!(peak(&samples) < 1.0);
    }

    #[test]
    fn the_routing_list_moves_the_same_samples_however_the_sound_is_asked_for() {
        // Per-note and shared LFOs in Hz, beats and presses, smoothed, the
        // key rising and falling, and notes, key presses and the beat all
        // starting between the samples the routing list is worked out on.
        // In chunks of 7 ms, or of 5 samples, every sample matches a render
        // made in one go.
        let lfos = [
            (0, Lfo { smooth: 30, ..START_LFO }),
            (
                1,
                Lfo {
                    per_note: false,
                    speed: LfoSpeed::Beats,
                    beats: 0.5,
                    ..lfo_at(LfoShape::Triangle, 1.0)
                },
            ),
            (2, Lfo { smooth: 30, ..lfo_at(LfoShape::RandomSteps, 10.3) }),
            (
                3,
                with_steps(
                    Lfo {
                        per_note: false,
                        speed: LfoSpeed::Presses,
                        presses: 5,
                        smooth: 50,
                        ..lfo_at(LfoShape::Steps, 1.0)
                    },
                    &STEP_HEIGHTS,
                ),
            ),
        ];
        let connections = [
            connection(Source::Lfo(0), Target::Pitch, 50.0),
            one_way_connection(Source::Lfo(1), Target::Loudness, -50.0),
            connection(Source::Lfo(2), Target::Cutoff, 2.0),
            connection(Source::Lfo(3), Target::Pan, 50.0),
            one_way_connection(Source::VibratoKey, Target::LfoSpeed(0), 1.0),
            Connection {
                scaled_by: Source::Lfo(2),
                ..one_way_connection(Source::VibratoKey, Target::Pitch, 25.0)
            },
        ];
        let new_engine = || {
            let mut engine = routed_engine(&lfos, &connections);
            engine.set_filter(filter(2000.0, 30));
            engine.set_vibrato_key_times(VibratoKeyTimes {
                rise_ms: 50,
                fall_ms: 80,
            });
            engine
        };
        let steps = [
            (1_001, NoteOn(A4)),
            (3_003, StartClick),
            (7_007, HoldVibratoKey),
            (11_011, NoteOn(C5)),
            (20_000, NoteOn(E4)),
            (25_013, LetGoOfVibratoKey),
            (30_017, NoteOff(A4)),
            (40_009, NoteOn(G4)),
        ];
        let frames = 60_000;
        let whole_wave = play(&mut new_engine(), &steps, frames);
        for chunk in [7 * SAMPLES_PER_MS, 5] {
            let chunked_wave = play_in_chunks(&mut new_engine(), &steps, frames, chunk);
            assert_same_samples(&chunked_wave, &whole_wave);
        }
    }

    // A4 dipped by LFO 1, a sine at 3.31 Hz, with its pitch moved 100 cents
    // by the vibrato key, which rises and falls over 100 ms and is let go
    // halfway, after `send` asks the engine for something directly.
    fn routed_a4_after(send: impl Fn(&Engine)) -> Vec<f32> {
        let lfo = lfo_at(LfoShape::Sine, 3.31);
        let key = one_way_connection(Source::VibratoKey, Target::Pitch, 100.0);
        let mut engine = routed_engine(&[(0, lfo)], &[dip_by(Source::Lfo(0)), key]);
        engine.set_vibrato_key_times(VibratoKeyTimes {
            rise_ms: 100,
            fall_ms: 100,
        });
        send(&engine);

        engine.set_vibrato_key(true);
        engine.note_on(A4, VELOCITY);
        let mut wave = render_left(&mut engine, 150);
        engine.set_vibrato_key(false);
        wave.extend(render_left(&mut engine, 150));
        wave
    }

    // A triangle at 5 Hz as the C API takes it, with `change` made to it.
    fn raw_triangle(change: fn(&mut RawLfo)) -> RawLfo {
        let mut raw = raw_lfo(&lfo_at(LfoShape::Triangle, 5.0));
        change(&mut raw);
        raw
    }

    #[test]
    fn odd_lfo_connection_and_vibrato_key_values_change_nothing() {
        // Asked directly, the engine ignores values outside their ranges,
        // and NaN, each on an engine of its own. Each odd LFO is otherwise a
        // triangle at 5 Hz, so one that got through would be heard.
        let plain = routed_a4_after(|_| {});

        let odd_lfos: [(&str, fn(&mut RawLfo)); 36] = [
            ("shape -1", |raw| raw.shape = -1),
            ("shape 8", |raw| raw.shape = 8),
            ("0 steps", |raw| raw.step_count = 0),
            ("33 steps", |raw| raw.step_count = 33),
            ("a step at 1.1", |raw| raw.steps[0] = 1.1),
            ("a step at -1.1", |raw| raw.steps[7] = -1.1),
            ("a step at NaN", |raw| raw.steps[3] = f32::NAN),
            ("speed -1", |raw| raw.speed = -1),
            ("speed 3", |raw| raw.speed = 3),
            ("0.009 Hz", |raw| raw.hz = 0.009),
            ("100.1 Hz", |raw| raw.hz = 100.1),
            ("NaN Hz", |raw| raw.hz = f64::NAN),
            ("1/17 beat", |raw| raw.beats = 1.0 / 17.0),
            ("128.1 beats", |raw| raw.beats = 128.1),
            ("NaN beats", |raw| raw.beats = f64::NAN),
            ("0 presses", |raw| raw.presses = 0),
            ("33 presses", |raw| raw.presses = 33),
            ("start -0.1", |raw| raw.start = -0.1),
            ("start 1.1", |raw| raw.start = 1.1),
            ("start NaN", |raw| raw.start = f64::NAN),
            ("spread -0.1", |raw| raw.spread = -0.1),
            ("spread 1.1", |raw| raw.spread = 1.1),
            ("spread NaN", |raw| raw.spread = f64::NAN),
            ("delay -1 ms", |raw| raw.delay_ms = -1),
            ("delay 10001 ms", |raw| raw.delay_ms = 10_001),
            ("fade -1 ms", |raw| raw.fade_ms = -1),
            ("fade 10001 ms", |raw| raw.fade_ms = 10_001),
            ("delay -0.1 beats", |raw| raw.delay_beats = -0.1),
            ("delay 128.1 beats", |raw| raw.delay_beats = 128.1),
            ("delay NaN beats", |raw| raw.delay_beats = f64::NAN),
            ("fade -0.1 beats", |raw| raw.fade_beats = -0.1),
            ("fade 128.1 beats", |raw| raw.fade_beats = 128.1),
            ("fade NaN beats", |raw| raw.fade_beats = f64::NAN),
            ("smooth -0.1", |raw| raw.smooth = -0.1),
            ("smooth 1.1", |raw| raw.smooth = 1.1),
            ("smooth NaN", |raw| raw.smooth = f64::NAN),
        ];
        for (what, change) in odd_lfos {
            let odd = routed_a4_after(|engine| {
                let raw = raw_triangle(change);
                // SAFETY: `raw` points to a live engine until Drop, and the
                // LFO lives until the call returns.
                unsafe { jello_engine_set_lfo(engine.raw, 0, &raw) };
            });
            assert!(odd == plain, "an LFO with {what} changed the sound");
        }
        for index in [-1, LFO_COUNT as c_int] {
            let odd = routed_a4_after(|engine| {
                let raw = raw_triangle(|_| {});
                // SAFETY: as above.
                unsafe { jello_engine_set_lfo(engine.raw, index, &raw) };
            });
            assert!(odd == plain, "LFO {index} changed the sound");
        }

        // Each odd connection is otherwise LFO 1 dipping A4 by half, or
        // nothing at all in the first slot, so one that got through would
        // be heard.
        let half_dip = raw_connection(&one_way_connection(Source::Lfo(0), Target::Loudness, -50.0));
        let odd_connections = [
            ("source -1", RawConnection { source: -1, ..half_dip }),
            ("source 14", RawConnection { source: 14, ..half_dip }),
            ("scaled by -1", RawConnection { scaled_by: -1, ..half_dip }),
            ("scaled by 14", RawConnection { scaled_by: 14, ..half_dip }),
            ("target -1", RawConnection { target: -1, ..half_dip }),
            ("target 35", RawConnection { target: 35, ..half_dip }),
            ("amount NaN", RawConnection { amount: f32::NAN, ..half_dip }),
        ];
        for (what, raw) in odd_connections {
            let odd = routed_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop, and the
                // connection lives until the call returns.
                unsafe { jello_engine_set_connection(engine.raw, 0, &raw) };
            });
            assert!(odd == plain, "a connection with {what} changed the sound");
        }
        for target in every_target() {
            for most in [target.most_amount(), -target.most_amount()] {
                let unused = Connection {
                    amount: most,
                    ..connection(Source::None, target, 0.0)
                };
                let mut raw = raw_connection(&unused);
                raw.amount = raw.amount * 1.001;
                let odd = routed_a4_after(|engine| {
                    // SAFETY: as above.
                    unsafe { jello_engine_set_connection(engine.raw, 0, &raw) };
                });
                assert!(odd == plain, "{} past {most} changed the sound", target.name());
            }
        }
        for index in [-1, CONNECTION_COUNT as c_int] {
            let odd = routed_a4_after(|engine| {
                let raw = raw_connection(&dip_by(Source::Lfo(0)));
                // SAFETY: as above.
                unsafe { jello_engine_set_connection(engine.raw, index, &raw) };
            });
            assert!(odd == plain, "connection {index} changed the sound");
        }

        for (rise_ms, fall_ms) in [(-1, 0), (10_001, 0), (0, -1), (0, 10_001)] {
            let odd = routed_a4_after(|engine| {
                // SAFETY: `raw` points to a live engine until Drop.
                unsafe { jello_engine_set_vibrato_key_times(engine.raw, rise_ms, fall_ms) };
            });
            assert!(odd == plain, "key times of {rise_ms} and {fall_ms} ms changed the sound");
        }
    }

    #[test]
    fn rust_brings_lfos_connections_and_vibrato_key_times_within_their_ranges() {
        // Every value out of range comes back at its edge, and NaN at its
        // lowest.
        let mut engine = Engine::new().unwrap();
        let mut steps = [0.0; LFO_MOST_STEPS];
        steps[0] = 150.0;
        steps[1] = -150.0;
        steps[2] = f32::NAN;
        let mut steps_within = [0.0; LFO_MOST_STEPS];
        steps_within[0] = 100.0;
        steps_within[1] = -100.0;
        steps_within[2] = -100.0;
        let too_much = Lfo {
            step_count: 40,
            steps,
            hz: 200.0,
            beats: 1000.0,
            presses: 100,
            start: 150,
            spread: 150,
            delay_ms: 20_000,
            fade_ms: 20_000,
            delay_beats: 200.0,
            fade_beats: 200.0,
            smooth: 150,
            ..START_LFO
        };
        let at_the_top = Lfo {
            step_count: LFO_MOST_STEPS as u32,
            steps: steps_within,
            hz: LFO_FASTEST_HZ,
            beats: LFO_LONGEST_BEATS,
            presses: LFO_MOST_PRESSES,
            start: FULL_PERCENT,
            spread: FULL_PERCENT,
            delay_ms: LFO_LONGEST_MS,
            fade_ms: LFO_LONGEST_MS,
            delay_beats: LFO_LONGEST_DELAY_BEATS,
            fade_beats: LFO_LONGEST_DELAY_BEATS,
            smooth: FULL_PERCENT,
            ..START_LFO
        };
        assert_eq!(engine.set_lfo(0, too_much), at_the_top);
        let too_little = Lfo {
            step_count: 0,
            hz: 0.0,
            beats: 0.0,
            presses: 0,
            delay_beats: -1.0,
            fade_beats: f64::NAN,
            ..START_LFO
        };
        let at_the_bottom = Lfo {
            step_count: 1,
            hz: LFO_SLOWEST_HZ,
            beats: LFO_SHORTEST_BEATS,
            presses: 1,
            delay_beats: 0.0,
            fade_beats: 0.0,
            ..START_LFO
        };
        assert_eq!(engine.set_lfo(0, too_little), at_the_bottom);
        assert_eq!(engine.set_lfo(0, Lfo { hz: f64::NAN, ..START_LFO }).hz, LFO_SLOWEST_HZ);

        // An LFO, source or target past the 8th is the 8th.
        let past_the_8th = Connection {
            source: Source::Lfo(9),
            target: Target::LfoSpeed(12),
            amount: 1.0,
            one_way: true,
            snap: true,
            scaled_by: Source::Lfo(20),
        };
        let the_8th = Connection {
            source: Source::Lfo(7),
            target: Target::LfoSpeed(7),
            scaled_by: Source::Lfo(7),
            ..past_the_8th
        };
        assert_eq!(engine.set_connection(0, past_the_8th), the_8th);
        for target in every_target() {
            let most = target.most_amount();
            let sent = engine.set_connection(0, connection(Source::VibratoKey, target, 2.0 * most));
            assert_eq!(sent.amount, most, "{}", target.name());
            let sent = engine.set_connection(0, connection(Source::VibratoKey, target, -2.0 * most));
            assert_eq!(sent.amount, -most, "{}", target.name());
            let sent = engine.set_connection(0, connection(Source::VibratoKey, target, f32::NAN));
            assert_eq!(sent.amount, -most, "{}", target.name());
        }

        let too_long = VibratoKeyTimes {
            rise_ms: 20_000,
            fall_ms: VIBRATO_KEY_LONGEST_MS + 1,
        };
        let longest = VibratoKeyTimes {
            rise_ms: VIBRATO_KEY_LONGEST_MS,
            fall_ms: VIBRATO_KEY_LONGEST_MS,
        };
        assert_eq!(engine.set_vibrato_key_times(too_long), longest);

        // An LFO or connection past the last is sent nowhere.
        assert_eq!(engine.set_lfo(LFO_COUNT, START_LFO), START_LFO);
        assert_eq!(engine.set_connection(CONNECTION_COUNT, NO_CONNECTION), NO_CONNECTION);
    }

    #[test]
    fn the_engine_takes_every_edge_rust_sends() {
        // Each LFO at an edge of one of its ranges, otherwise a triangle at
        // 5 Hz, replaces routed_a4_after's sine, so the sound changes. The
        // odd values test shows a hair further is ignored, so Rust and the
        // engine agree on every range.
        let plain = routed_a4_after(|_| {});
        let triangle = lfo_at(LfoShape::Triangle, 5.0);
        let edges = [
            Lfo { step_count: 1, ..triangle },
            with_steps(triangle, &[100.0; LFO_MOST_STEPS]),
            with_steps(triangle, &[-100.0; LFO_MOST_STEPS]),
            Lfo { hz: LFO_SLOWEST_HZ, ..triangle },
            Lfo { hz: LFO_FASTEST_HZ, ..triangle },
            Lfo { beats: LFO_SHORTEST_BEATS, ..triangle },
            Lfo { beats: LFO_LONGEST_BEATS, ..triangle },
            Lfo { presses: 1, ..triangle },
            Lfo { presses: LFO_MOST_PRESSES, ..triangle },
            Lfo { start: FULL_PERCENT, ..triangle },
            Lfo { spread: FULL_PERCENT, ..triangle },
            Lfo { smooth: FULL_PERCENT, ..triangle },
            Lfo { delay_ms: LFO_LONGEST_MS, ..triangle },
            Lfo { fade_ms: LFO_LONGEST_MS, ..triangle },
            Lfo { delay_beats: LFO_LONGEST_DELAY_BEATS, ..triangle },
            Lfo { fade_beats: LFO_LONGEST_DELAY_BEATS, ..triangle },
        ];
        for edge in edges {
            let moved = routed_a4_after(|engine| {
                let raw = raw_lfo(&edge);
                // SAFETY: `raw` points to a live engine until Drop, and the
                // LFO lives until the call returns.
                unsafe { jello_engine_set_lfo(engine.raw, 0, &raw) };
            });
            assert!(moved != plain, "the engine refused {edge:?}");
        }

        // Each target moved by its most either way, by an unused connection
        // in LFO 1's slot: taken, it leaves A4 undipped.
        for target in every_target() {
            for most in [target.most_amount(), -target.most_amount()] {
                let unused = Connection {
                    amount: most,
                    ..connection(Source::None, target, 0.0)
                };
                let moved = routed_a4_after(|engine| {
                    let raw = raw_connection(&unused);
                    // SAFETY: `raw` points to a live engine until Drop, and
                    // the connection lives until the call returns.
                    unsafe { jello_engine_set_connection(engine.raw, 0, &raw) };
                });
                assert!(moved != plain, "the engine refused {} by {most}", target.name());
            }
        }

        // And the longest key times.
        let moved = routed_a4_after(|engine| {
            let longest = VIBRATO_KEY_LONGEST_MS as c_int;
            // SAFETY: `raw` points to a live engine until Drop.
            unsafe { jello_engine_set_vibrato_key_times(engine.raw, longest, longest) };
        });
        assert!(moved != plain, "the engine refused the longest key times");
    }

    #[test]
    fn a_new_engine_s_lfos_connections_and_vibrato_key_start_where_rust_says() {
        // As START_LFO, NO_CONNECTION and START_VIBRATO_KEY_TIMES say, so
        // sending them changes nothing. Each LFO turns A4 down a little, and
        // the key moves its pitch, held and then let go.
        let mut started = Engine::new().unwrap();
        let mut sent = Engine::new().unwrap();
        for engine in [&mut started, &mut sent] {
            for index in 0..LFO_COUNT {
                engine.set_connection(index, one_way_connection(Source::Lfo(index), Target::Loudness, -10.0));
            }
            hold_key_moving(engine, Target::Pitch, 100.0);
        }
        for index in 0..LFO_COUNT {
            sent.set_lfo(index, START_LFO);
        }
        for index in LFO_COUNT..CONNECTION_COUNT - 1 {
            sent.set_connection(index, NO_CONNECTION);
        }
        sent.set_vibrato_key_times(START_VIBRATO_KEY_TIMES);

        let mut waves = Vec::new();
        for engine in [&mut started, &mut sent] {
            engine.note_on(A4, VELOCITY);
            let mut wave = render_left(engine, 150);
            engine.set_vibrato_key(false);
            wave.extend(render_left(engine, 150));
            waves.push(wave);
        }
        assert!(waves[0] == waves[1], "a new engine does not start where the START_ values say");
    }
}
