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
    fn jello_engine_set_wave(engine: *mut JelloEngine, wave: c_int);
    fn jello_engine_sample_rate(engine: *mut JelloEngine) -> c_int;
    fn jello_engine_render(engine: *mut JelloEngine, out: *mut f32, frames: c_int);
}

// Every frame of audio is one left sample and one right sample.
pub const CHANNELS: usize = 2;

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

pub struct Engine {
    raw: *mut JelloEngine,
    audio_running: bool,
    // The wave new notes play. The C++ engine starts on triangle too.
    wave: Wave,
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
}
