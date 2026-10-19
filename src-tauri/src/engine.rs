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
    fn jello_engine_note_on(engine: *mut JelloEngine, note: c_int, velocity: c_int);
    fn jello_engine_note_off(engine: *mut JelloEngine, note: c_int);
    fn jello_engine_all_notes_off(engine: *mut JelloEngine);
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

    // The sound the C++ engine makes. When it is changed there on purpose,
    // change it here too.
    const VOICE_LEVEL: f32 = 0.35; // `voice_level` in engine/src/voice.cpp
    const ATTACK_MS: u32 = 5; // `attack_ms` in engine/src/envelope.cpp
    const DECAY_MS: u32 = 300; // `decay_ms` in engine/src/envelope.cpp
    const SUSTAIN: f32 = 0.7; // `sustain_level` in engine/src/envelope.cpp
    const RELEASE_MS: u32 = 300; // `release_ms` in engine/src/envelope.cpp
    const MAILBOX_SLOTS: usize = 1024; // `slot_count` in engine/src/mailbox.h

    // Notes by their MIDI numbers, and how many times a second their waves
    // repeat in standard tuning.
    const C3: u8 = 48;
    const MIDDLE_C: u8 = 60;
    const MIDDLE_C_HZ: f64 = 261.6256;
    const E4: u8 = 64;
    const A4: u8 = 69;
    const A4_HZ: f64 = 440.0;
    const C5: u8 = 72;
    const C5_HZ: f64 = 523.2511;

    // A firm key press, used unless a test says otherwise.
    const VELOCITY: u8 = 100;

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
    // Each crossing falls between two samples. A triangle wave is a straight
    // line there, so a straight line between the two samples finds it almost
    // exactly.
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
        let mut engine = Engine::new().unwrap();
        hold(&mut engine, MIDDLE_C, VELOCITY);

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

    #[test]
    fn a_new_note_takes_over_a_held_one_without_a_click() {
        // Ten tries, each 1 ms later than the last. The new note arrives at
        // many points along middle C's wave, including near its top and
        // bottom, where a restart would jump the furthest.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.note_on(MIDDLE_C, VELOCITY);
            let mut wave = render_left(&mut engine, 500 + extra_ms);

            // An octave up, while middle C is still held.
            engine.note_on(C5, VELOCITY);
            let after = render_left(&mut engine, 100);
            wave.extend(after);

            assert_no_click(&engine, &wave, C5_HZ);
        }
    }

    #[test]
    fn a_new_note_takes_over_a_fading_one_without_a_click() {
        // Ten tries again, as above, while C5 fades out.
        for extra_ms in 0..10 {
            let mut engine = Engine::new().unwrap();
            engine.note_on(C5, VELOCITY);
            let mut wave = render_left(&mut engine, 500);

            engine.note_off(C5);
            let fading = render_left(&mut engine, 100 + extra_ms);
            wave.extend(fading);

            // An octave below middle C, while C5 is still fading.
            engine.note_on(C3, VELOCITY);
            let after = render_left(&mut engine, 100);
            wave.extend(after);

            assert_no_click(&engine, &wave, C5_HZ);
        }
    }

    #[test]
    fn nothing_ever_clips() {
        // Speakers can only play from -1.0 to 1.0. Anything past that clips,
        // which sounds harsh and broken. With one voice, the hardest key
        // press is the loudest sound there is.
        let mut engine = Engine::new().unwrap();
        engine.note_on(MIDDLE_C, 127);
        let wave = render_left(&mut engine, 1000);
        assert!(peak(&wave) <= 1.0);
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

        // 1023 middle Cs, then C5 in the last free slot. All of them get in,
        // and the last one sent wins.
        for _ in 0..MAILBOX_SLOTS - 1 {
            engine.note_on(MIDDLE_C, VELOCITY);
        }
        engine.note_on(C5, VELOCITY);

        let wave = render_left(&mut engine, 1000);
        assert_pitch(&engine, &wave, C5_HZ);
    }

    #[test]
    fn a_full_mailbox_drops_new_messages() {
        let mut engine = Engine::new().unwrap();

        // Every slot holds middle C, so C5 finds no room and is dropped.
        for _ in 0..MAILBOX_SLOTS {
            engine.note_on(MIDDLE_C, VELOCITY);
        }
        engine.note_on(C5, VELOCITY);

        let wave = render_left(&mut engine, 1000);
        assert_pitch(&engine, &wave, MIDDLE_C_HZ);

        // Rendering emptied the mailbox, so now C5 gets in.
        engine.note_on(C5, VELOCITY);
        let wave = render_left(&mut engine, 1000);
        assert_pitch(&engine, &wave, C5_HZ);
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
}
