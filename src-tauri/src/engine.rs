// A safe Rust wrapper around the C++ engine's C API
// (engine/include/jello_engine.h). The rest of the Rust core only uses this.

use std::ffi::c_int;

// The engine itself lives in C++. Rust only ever holds a pointer to it.
#[repr(C)]
struct JelloEngine {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn jello_engine_create() -> *mut JelloEngine;
    fn jello_engine_destroy(engine: *mut JelloEngine);
    fn jello_engine_start_audio(engine: *mut JelloEngine) -> bool;
    fn jello_engine_set_tone(engine: *mut JelloEngine, on: bool);
    fn jello_engine_sample_rate(engine: *mut JelloEngine) -> c_int;
    fn jello_engine_render(engine: *mut JelloEngine, out: *mut f32, frames: c_int);
}

// Every frame of audio is one left sample and one right sample.
pub const CHANNELS: usize = 2;

pub struct Engine {
    raw: *mut JelloEngine,
    audio_running: bool,
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

    pub fn set_tone(&mut self, on: bool) {
        // SAFETY: `raw` points to a live engine until Drop.
        unsafe { jello_engine_set_tone(self.raw, on) };
    }

    pub fn sample_rate(&self) -> u32 {
        // SAFETY: `raw` points to a live engine until Drop.
        let rate = unsafe { jello_engine_sample_rate(self.raw) };
        rate as u32
    }

    // Makes `frames` frames of sound without speakers, for writing files.
    // Returns the samples as left, right, left, right, ...
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

#[cfg(test)]
mod tests {
    use super::*;

    // Matches `level` in engine/src/tone.cpp.
    const TONE_LEVEL: f32 = 0.25;

    fn frames_in_ms(engine: &Engine, ms: u32) -> usize {
        let frames = engine.sample_rate() * ms / 1000;
        frames as usize
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

    // Turns the tone on and renders one second of it.
    fn render_one_second_of_tone(engine: &mut Engine) -> Vec<f32> {
        engine.set_tone(true);
        let one_second = frames_in_ms(engine, 1000);
        engine.render(one_second)
    }

    #[test]
    fn tone_starts_silent() {
        let mut engine = Engine::new().unwrap();
        let samples = render_one_second_of_tone(&mut engine);

        // After 1 ms the 10 ms fade-in is only a tenth of the way up.
        let first_ms = frames_in_ms(&engine, 1) * CHANNELS;
        assert!(peak(&samples[..first_ms]) <= TONE_LEVEL * 0.1);
    }

    #[test]
    fn tone_reaches_the_right_loudness() {
        let mut engine = Engine::new().unwrap();
        let samples = render_one_second_of_tone(&mut engine);

        // Skip the first 100 ms so the fade-in is long done.
        let after_fade = frames_in_ms(&engine, 100) * CHANNELS;
        let difference = peak(&samples[after_fade..]) - TONE_LEVEL;
        assert!(difference.abs() < 0.001);
    }

    #[test]
    fn tone_never_clips() {
        let mut engine = Engine::new().unwrap();
        let samples = render_one_second_of_tone(&mut engine);

        // Speakers can only play from -1.0 to 1.0. Anything past that clips,
        // which sounds harsh and broken.
        assert!(peak(&samples) <= 1.0);
    }

    #[test]
    fn tone_fades_back_to_silence() {
        let mut engine = Engine::new().unwrap();
        render_one_second_of_tone(&mut engine);

        engine.set_tone(false);
        let tail = engine.render(frames_in_ms(&engine, 50));

        // The fade-out takes 10 ms, so the last 10 ms must be exact silence.
        let last_10_ms = frames_in_ms(&engine, 10) * CHANNELS;
        let start = tail.len() - last_10_ms;
        assert_eq!(peak(&tail[start..]), 0.0);
    }
}
