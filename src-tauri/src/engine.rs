// A safe Rust wrapper around the C++ engine's C API
// (engine/include/jello_engine.h). The rest of the Rust core only uses this.

use std::ffi::c_int;
use std::time::{Duration, Instant};

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

unsafe extern "C" {
    fn jello_engine_create() -> *mut JelloEngine;
    fn jello_engine_destroy(engine: *mut JelloEngine);
    fn jello_engine_start_audio(engine: *mut JelloEngine) -> bool;
    fn jello_engine_note_on(engine: *mut JelloEngine, note: c_int, velocity: c_int);
    fn jello_engine_note_off(engine: *mut JelloEngine, note: c_int);
    fn jello_engine_all_notes_off(engine: *mut JelloEngine);
    fn jello_engine_set_wave(engine: *mut JelloEngine, wave: c_int);
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
// starts there too.
pub const START_TEMPO: u32 = 120;

// How much slowing down or speeding up changes the tempo.
const TEMPO_STEP: u32 = 5;

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

// A new engine's filter: open, with no resonance. The C++ engine starts
// there too.
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

// A new engine's filter envelope. The C++ engine starts there too.
pub const START_FILTER_ENVELOPE: FilterEnvelope = FilterEnvelope {
    amount: 0.0,
    attack_ms: 5,
    decay_ms: 300,
    sustain: 0,
    release_ms: 300,
};

// `value` brought within `lowest` to `highest`. NaN, not a number at all,
// counts as `lowest`; no slider sends it.
fn within(value: f32, lowest: f32, highest: f32) -> f32 {
    if value.is_nan() {
        return lowest;
    }
    value.clamp(lowest, highest)
}

// A filter envelope stage's length brought within its range, as a C int.
fn stage_ms(ms: u32) -> c_int {
    let ms = ms.min(FILTER_LONGEST_MS);
    ms as c_int
}

// The shapes a note can play (docs/adr/0009-how-notes-become-sound.md).
#[derive(Clone, Copy)]
pub enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
}

impl Wave {
    // The wave after this one: sine, triangle, square, saw, then sine again.
    pub fn next(self) -> Wave {
        match self {
            Wave::Sine => Wave::Triangle,
            Wave::Triangle => Wave::Square,
            Wave::Square => Wave::Saw,
            Wave::Saw => Wave::Sine,
        }
    }

    // What to call the wave when showing it.
    pub fn name(self) -> &'static str {
        match self {
            Wave::Sine => "sine",
            Wave::Triangle => "triangle",
            Wave::Square => "square",
            Wave::Saw => "saw",
        }
    }

    // The wave's number in the C API (`JELLO_WAVE_` in
    // engine/include/jello_engine.h).
    fn number(self) -> c_int {
        match self {
            Wave::Sine => 0,
            Wave::Triangle => 1,
            Wave::Square => 2,
            Wave::Saw => 3,
        }
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

pub struct Engine {
    raw: *mut JelloEngine,
    audio_running: bool,
    // The wave new notes play. The C++ engine starts on triangle too.
    wave: Wave,
    // The beat's tempo in beats per minute. The C++ engine starts there too.
    tempo: u32,
    // Whether loops snap to the beat. The C++ engine starts with it on too.
    snap: bool,
    // Where the filter and its envelope are.
    filter: Filter,
    filter_envelope: FilterEnvelope,
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
            wave: Wave::Triangle,
            tempo: START_TEMPO,
            snap: true,
            filter: START_FILTER,
            filter_envelope: START_FILTER_ENVELOPE,
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

    // Makes notes sent after this play `wave`. Notes sent before it keep
    // theirs, even while they ring on.
    pub fn set_wave(&mut self, wave: Wave) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_wave(self.raw, wave.number()) };
        self.wave = wave;
    }

    // Moves new notes on to the next wave (sine, triangle, square, saw, then
    // sine again) and returns it.
    pub fn switch_wave(&mut self) -> Wave {
        let next = self.wave.next();
        self.set_wave(next);
        next
    }

    // The wave new notes play.
    pub fn wave(&self) -> Wave {
        self.wave
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
    // its beat, so this changes nothing.
    pub fn set_tempo(&mut self, tempo: u32) {
        // Not sent at all, so the tempo kept here and the engine's always
        // agree. Sent anyway, it could go wrong: with Esc still on its way,
        // the engine would clear the loop first and then take the tempo.
        if self.is_looper_busy() {
            return;
        }

        // A number too big for a C int becomes -1, which the engine ignores.
        let number = c_int::try_from(tempo).unwrap_or(-1);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tempo(self.raw, number) };

        if tempo < SLOWEST_TEMPO {
            return;
        }
        if tempo > FASTEST_TEMPO {
            return;
        }
        self.tempo = tempo;
    }

    // Slows the beat down by 5 beats per minute, but never below
    // SLOWEST_TEMPO.
    pub fn slow_down(&mut self) {
        let slower = self.tempo - TEMPO_STEP;
        if slower >= SLOWEST_TEMPO {
            self.set_tempo(slower);
        }
    }

    // Speeds the beat up by 5 beats per minute, but never above
    // FASTEST_TEMPO.
    pub fn speed_up(&mut self) {
        let faster = self.tempo + TEMPO_STEP;
        if faster <= FASTEST_TEMPO {
            self.set_tempo(faster);
        }
    }

    // The beat's tempo, in beats per minute.
    pub fn tempo(&self) -> u32 {
        self.tempo
    }

    // Moves the filter (`jello_engine_set_filter`). Values outside its
    // ranges are brought within them. Every note hears it, even those
    // already sounding.
    pub fn set_filter(&mut self, filter: Filter) {
        let cutoff_hz = within(filter.cutoff_hz, FILTER_LOWEST_HZ, FILTER_OPEN_HZ);
        let resonance = filter.resonance.min(FULL_PERCENT);
        let share = resonance as f32 / FULL_PERCENT as f32;
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_filter(self.raw, cutoff_hz, share) };
        self.filter = Filter { cutoff_hz, resonance };
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    // Sets the filter envelope (`jello_engine_set_filter_envelope`) for the
    // notes started after this. Values outside its ranges are brought
    // within them.
    pub fn set_filter_envelope(&mut self, envelope: FilterEnvelope) {
        let amount = within(envelope.amount, 0.0, FILTER_MOST_OCTAVES);
        let sustain = envelope.sustain.min(FULL_PERCENT);
        let share = sustain as f32 / FULL_PERCENT as f32;
        let attack = stage_ms(envelope.attack_ms);
        let decay = stage_ms(envelope.decay_ms);
        let release = stage_ms(envelope.release_ms);
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_filter_envelope(self.raw, amount, attack, decay, share, release) };
        self.filter_envelope = FilterEnvelope {
            amount,
            attack_ms: attack as u32,
            decay_ms: decay as u32,
            sustain,
            release_ms: release as u32,
        };
    }

    pub fn filter_envelope(&self) -> FilterEnvelope {
        self.filter_envelope
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

    // The sound the C++ engine makes. When it is changed there on purpose,
    // change it here too.
    const VOICE_LEVEL: f32 = 0.35; // `voice_level` in engine/src/voice.cpp
    const ATTACK_MS: u32 = 5; // `attack_ms` in engine/src/envelope.cpp
    const DECAY_MS: u32 = 300; // `decay_ms` in engine/src/envelope.cpp
    const SUSTAIN: f32 = 1.0; // `sustain_level` in engine/src/envelope.cpp
    const RELEASE_MS: u32 = 300; // `release_ms` in engine/src/envelope.cpp
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
    // channel is the same for now.
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
        let samples_per_cycle = engine.sample_rate() as f64 / fastest_hz;
        let wave_step = (4.0 / samples_per_cycle) as f32;
        let steepest = wave_step * full_level();
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
    const EVERY_WAVE: [Wave; 4] = [Wave::Sine, Wave::Triangle, Wave::Square, Wave::Saw];

    #[test]
    fn every_wave_plays_at_the_right_pitch() {
        for wave in EVERY_WAVE {
            let mut engine = Engine::new().unwrap();
            engine.set_wave(wave);
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
    fn a_new_engine_plays_a_triangle_wave() {
        let mut engine = Engine::new().unwrap();
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn a_sine_wave_has_no_harmonics() {
        let mut engine = Engine::new().unwrap();
        engine.set_wave(Wave::Sine);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SINE_HARMONICS);
    }

    #[test]
    fn a_triangle_wave_has_odd_harmonics_that_fade_fast() {
        // Away from the triangle and back again, so this checks that
        // switching to it works, not only that Jello starts on it.
        let mut engine = Engine::new().unwrap();
        engine.set_wave(Wave::Saw);
        engine.set_wave(Wave::Triangle);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn a_square_wave_has_odd_harmonics_that_fade_slowly() {
        let mut engine = Engine::new().unwrap();
        engine.set_wave(Wave::Square);
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SQUARE_HARMONICS);
    }

    #[test]
    fn a_saw_wave_has_every_harmonic() {
        let mut engine = Engine::new().unwrap();
        engine.set_wave(Wave::Saw);
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
            engine.set_wave(wave);
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
            engine.set_wave(wave);
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
            if matches!(wave, Wave::Square | Wave::Saw) {
                expected = triangle_loudness * 0.7943;
            }

            let mut engine = Engine::new().unwrap();
            engine.set_wave(wave);
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

        engine.set_wave(Wave::Saw);
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

        engine.set_wave(Wave::Saw);
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
        engine.set_wave(Wave::Saw);
        render_left(&mut engine, ATTACK_MS + DECAY_MS);

        let wave = render_left(&mut engine, 1000);
        assert_harmonics(&engine, &wave, TRIANGLE_HARMONICS);
    }

    #[test]
    fn numbers_that_are_not_waves_change_nothing() {
        // The C API takes any number as a wave, so it must ignore all but the
        // four it knows. Rust's Wave can only send those four, so this calls
        // the C API directly.
        let mut engine = Engine::new().unwrap();
        engine.set_wave(Wave::Sine);
        for number in [-1, 4, 255] {
            // SAFETY: `raw` points to a live engine until Drop.
            unsafe { jello_engine_set_wave(engine.raw, number) };
        }

        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SINE_HARMONICS);
    }

    #[test]
    fn switching_steps_through_the_waves_in_order() {
        // A new engine starts on the triangle, so the first switch lands on
        // the square. After the saw, it wraps around to the sine.
        let mut engine = Engine::new().unwrap();
        let mut names = Vec::new();
        for _ in 0..5 {
            let next = engine.switch_wave();
            names.push(next.name());
        }
        assert_eq!(names, ["square", "saw", "sine", "triangle", "square"]);

        // And the engine really plays the wave it landed on.
        let wave = hold(&mut engine, A4, VELOCITY);
        assert_harmonics(&engine, &wave, SQUARE_HARMONICS);
    }

    #[test]
    fn the_engine_says_which_wave_new_notes_play() {
        let mut engine = Engine::new().unwrap();
        assert_eq!(engine.wave().name(), "triangle");

        engine.switch_wave();
        assert_eq!(engine.wave().name(), "square");

        engine.set_wave(Wave::Sine);
        assert_eq!(engine.wave().name(), "sine");
    }

    // Plays A4 on `wave` for `held_ms`, then lets go of it and renders
    // `fade_ms` of its fade. A fade shorter than RELEASE_MS leaves its voice
    // still sounding.
    fn play_and_let_go_of_a4(wave: Wave, held_ms: u32, fade_ms: u32) -> Engine {
        let mut engine = Engine::new().unwrap();
        engine.set_wave(wave);
        engine.note_on(A4, VELOCITY);
        render_left(&mut engine, held_ms);
        engine.note_off(A4);
        render_left(&mut engine, fade_ms);
        engine
    }

    // Switches to `wave` and presses A4 again. If A4 is still sounding, the
    // press takes over its voice.
    fn press_a4_again_on(engine: &mut Engine, wave: Wave) {
        engine.set_wave(wave);
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
    fn slowing_down_and_speeding_up_go_by_5_and_stop_at_the_ends() {
        let mut engine = Engine::new().unwrap();
        engine.speed_up();
        assert_eq!(engine.tempo(), 125);

        // The engine plays it too: a beat at 125 is 23,040 samples.
        engine.start_click();
        let wave = render_left(&mut engine, 500);
        assert_eq!(tick_starts(&wave), vec![0, 23_040]);

        engine.slow_down();
        engine.slow_down();
        assert_eq!(engine.tempo(), 115);

        // From 120 it takes 16 steps to reach 40, and then it stays there.
        let mut engine = Engine::new().unwrap();
        for _ in 0..20 {
            engine.slow_down();
        }
        assert_eq!(engine.tempo(), SLOWEST_TEMPO);

        // And 24 steps to reach 240.
        for _ in 0..50 {
            engine.speed_up();
        }
        assert_eq!(engine.tempo(), FASTEST_TEMPO);
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
        SetWave(Wave),
        PlaySong(&'static [TestChord]),
        StopSong,
        Snap(bool),
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
            SetWave(wave) => engine.set_wave(wave),
            PlaySong(song) => engine.play_song(&song_chords(song)).unwrap(),
            StopSong => engine.stop_song(),
            Snap(on) => engine.set_snap(on),
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
        engine.speed_up();
        assert_eq!(engine.tempo(), 125);
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

        engine.speed_up();
        assert_eq!(engine.tempo(), 125);
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
        let switch = (first_time + 80_000, SetWave(Wave::Square));
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
        engine.speed_up();
        assert_eq!(engine.tempo(), START_TEMPO);
        let count_in = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        engine.slow_down();
        assert_eq!(engine.tempo(), START_TEMPO);

        // Asked directly, the engine keeps its beat as well.
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tempo(engine.raw, 60) };
        let recording = render_frames_left(&mut engine, BAR_AT_120, usize::MAX);
        assert_eq!(tick_starts(&count_in), vec![0, 24_000, 48_000, 72_000]);
        assert_eq!(tick_starts(&recording), vec![0, 24_000, 48_000, 72_000]);

        // Esc unlocks it, once the engine has acted on it.
        engine.remove_newest_layer();
        engine.speed_up();
        assert_eq!(engine.tempo(), START_TEMPO);
        render_left(&mut engine, 10);
        engine.speed_up();
        assert_eq!(engine.tempo(), 125);
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
            (first_time + 40_000, SetWave(Wave::Square)),
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
        engine.speed_up();
        assert_eq!(engine.tempo(), 125);
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
        engine.set_wave(Wave::Saw);
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
        below.set_filter(filter(5.0, 150));
        assert_eq!(below.filter(), filter(FILTER_LOWEST_HZ, FULL_PERCENT));
        let mut lowest = saw_engine();
        lowest.set_filter(filter(FILTER_LOWEST_HZ, FULL_PERCENT));
        below.note_on(A4, VELOCITY);
        lowest.note_on(A4, VELOCITY);
        let below_wave = render_left(&mut below, 300);
        let lowest_wave = render_left(&mut lowest, 300);
        assert!(below_wave == lowest_wave, "the cutoff below the lowest was not sent as the lowest");

        let mut engine = Engine::new().unwrap();
        engine.set_filter(filter(50000.0, 0));
        assert_eq!(engine.filter(), START_FILTER);
        engine.set_filter(filter(f32::NAN, 0));
        assert_eq!(engine.filter(), filter(FILTER_LOWEST_HZ, 0));

        engine.set_filter_envelope(filter_envelope(9.5, 20000, 20000, 150, 20000));
        let longest = FILTER_LONGEST_MS;
        let edges = filter_envelope(FILTER_MOST_OCTAVES, longest, longest, FULL_PERCENT, longest);
        assert_eq!(engine.filter_envelope(), edges);
        engine.set_filter_envelope(filter_envelope(-1.0, 0, 0, 0, 0));
        assert_eq!(engine.filter_envelope(), filter_envelope(0.0, 0, 0, 0, 0));
    }

    #[test]
    fn a_new_engine_starts_with_the_filter_open_and_no_envelope() {
        let engine = Engine::new().unwrap();
        assert_eq!(engine.filter(), filter(FILTER_OPEN_HZ, 0));
        assert_eq!(engine.filter_envelope(), filter_envelope(0.0, 5, 300, 0, 300));
    }
}
