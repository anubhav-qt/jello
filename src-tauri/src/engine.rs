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
    const VOICE_COUNT: u8 = 32; // `voice_count` in engine/src/synth.cpp
    const CLIP_START: f32 = 0.8; // `clip_start` in engine/src/synth.cpp

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
    // engine/src/synth.cpp.
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
}
