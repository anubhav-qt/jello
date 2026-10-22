// A song's sound profile (docs/adr/0030-a-sound-profile-per-song.md):
// everything about how its notes sound, which is its wave, its loudness
// envelope, its low-pass filter with its filter envelope and key tracking,
// its unison and its tuning. The song holds it. Every change to the sound
// goes through it to the engine, so the engine plays only what the song says.

use crate::engine::{
    Engine, Filter, FilterEnvelope, LoudnessEnvelope, START_FILTER, START_FILTER_ENVELOPE, START_KEY_TRACKING,
    START_LOUDNESS_ENVELOPE, START_TUNING_HZ, START_UNISON, START_WAVE, Unison, Wave,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundProfile {
    // The wave new notes play.
    wave: Wave,
    // The loudness envelope new notes start with.
    loudness_envelope: LoudnessEnvelope,
    // Where the filter is. Every note hears it, even those already sounding.
    filter: Filter,
    // The filter envelope new notes start with.
    filter_envelope: FilterEnvelope,
    // How much new notes' cutoffs follow middle C's, in percent.
    key_tracking: u32,
    // The copies new notes are played by.
    unison: Unison,
    // A4's pitch in Hz, which new notes are tuned from.
    tuning_hz: f32,
}

impl SoundProfile {
    // A new song's sound profile: a triangle wave, a 5 ms attack held at the
    // full peak with a 300 ms release, the filter open, no filter envelope,
    // full key tracking, 1 copy and A4 at 440 Hz. It is only where a new
    // song starts, the same place a new engine starts, and the song changes
    // it from there.
    pub fn new() -> SoundProfile {
        SoundProfile {
            wave: START_WAVE,
            loudness_envelope: START_LOUDNESS_ENVELOPE,
            filter: START_FILTER,
            filter_envelope: START_FILTER_ENVELOPE,
            key_tracking: START_KEY_TRACKING,
            unison: START_UNISON,
            tuning_hz: START_TUNING_HZ,
        }
    }

    // Sends the whole profile to `engine`, so the notes started after this
    // play it. The song does this as the app starts, before the first note.
    pub fn send_all(&self, engine: &mut Engine) {
        engine.set_wave(self.wave);
        engine.set_loudness_envelope(self.loudness_envelope);
        engine.set_filter(self.filter);
        engine.set_filter_envelope(self.filter_envelope);
        engine.set_key_tracking(self.key_tracking);
        engine.set_unison(self.unison);
        engine.set_tuning(self.tuning_hz);
    }

    // Moves new notes on to the next wave (sine, triangle, square, saw, then
    // sine again), on `engine` too. Notes already playing keep their wave.
    pub fn switch_wave(&mut self, engine: &mut Engine) {
        self.wave = self.wave.next();
        engine.set_wave(self.wave);
    }

    // Sets the loudness envelope for the notes started after this, on
    // `engine` too, kept as the engine sent it.
    pub fn set_loudness_envelope(&mut self, engine: &mut Engine, envelope: LoudnessEnvelope) {
        self.loudness_envelope = engine.set_loudness_envelope(envelope);
    }

    // Moves the filter, on `engine` too. Every note hears it, even those
    // already sounding. The profile keeps the filter as the engine sent it,
    // with values outside their ranges brought within them.
    pub fn set_filter(&mut self, engine: &mut Engine, filter: Filter) {
        self.filter = engine.set_filter(filter);
    }

    // Sets the filter envelope for the notes started after this, on
    // `engine` too, kept as the engine sent it.
    pub fn set_filter_envelope(&mut self, engine: &mut Engine, envelope: FilterEnvelope) {
        self.filter_envelope = engine.set_filter_envelope(envelope);
    }

    // Sets key tracking, in percent, for the notes started after this, on
    // `engine` too, kept as the engine sent it.
    pub fn set_key_tracking(&mut self, engine: &mut Engine, key_tracking: u32) {
        self.key_tracking = engine.set_key_tracking(key_tracking);
    }

    // Sets unison for the notes started after this, on `engine` too, kept
    // as the engine sent it.
    pub fn set_unison(&mut self, engine: &mut Engine, unison: Unison) {
        self.unison = engine.set_unison(unison);
    }

    // Tunes the notes started after this from A4 at `a4_hz`, on `engine`
    // too, kept as the engine sent it.
    pub fn set_tuning(&mut self, engine: &mut Engine, a4_hz: f32) {
        self.tuning_hz = engine.set_tuning(a4_hz);
    }

    pub fn wave(&self) -> Wave {
        self.wave
    }

    pub fn loudness_envelope(&self) -> LoudnessEnvelope {
        self.loudness_envelope
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    pub fn filter_envelope(&self) -> FilterEnvelope {
        self.filter_envelope
    }

    pub fn key_tracking(&self) -> u32 {
        self.key_tracking
    }

    pub fn unison(&self) -> Unison {
        self.unison
    }

    // A4's pitch in Hz.
    pub fn tuning_hz(&self) -> f32 {
        self.tuning_hz
    }
}

// These tests check that a profile keeps what it was given, as the engine
// sent it, and that the engine plays exactly the profile. They render without
// speakers.
#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::{
        FILTER_LONGEST_MS, FILTER_MOST_OCTAVES, FILTER_OPEN_HZ, FULL_PERCENT, LOUDNESS_LONGEST_MS,
        LOUDNESS_SHORTEST_MS, TUNING_HIGHEST_HZ, UNISON_MOST_COPIES, UNISON_MOST_DETUNE_CENTS,
    };

    const A4: u8 = 69;
    const VELOCITY: u8 = 100;

    // A profile unlike a new song's in every way: a saw, a slow attack that
    // falls to half the peak, a closed filter with resonance, a filter
    // envelope, half key tracking, 5 spread copies and A4 at 432 Hz.
    fn odd_profile(engine: &mut Engine) -> SoundProfile {
        let mut profile = SoundProfile::new();
        profile.switch_wave(engine);
        profile.switch_wave(engine);
        profile.set_loudness_envelope(
            engine,
            LoudnessEnvelope {
                attack_ms: 50,
                decay_ms: 100,
                sustain: 50,
                release_ms: 200,
            },
        );
        profile.set_filter(
            engine,
            Filter {
                cutoff_hz: 800.0,
                resonance: 50,
            },
        );
        profile.set_filter_envelope(
            engine,
            FilterEnvelope {
                amount: 3.0,
                attack_ms: 20,
                decay_ms: 150,
                sustain: 40,
                release_ms: 250,
            },
        );
        profile.set_key_tracking(engine, 50);
        profile.set_unison(
            engine,
            Unison {
                copies: 5,
                detune_cents: 25,
                spread: 80,
            },
        );
        profile.set_tuning(engine, 432.0);
        profile
    }

    // Plays A4 on `engine` and returns its first 300 ms, left and right.
    fn play_a4(engine: &mut Engine) -> Vec<f32> {
        engine.note_on(A4, VELOCITY);
        let frames = engine.sample_rate() * 300 / 1000;
        engine.render(frames as usize)
    }

    // Every part of a profile, as play_a4_without names them.
    const PARTS: [&str; 7] = [
        "wave",
        "loudness envelope",
        "filter",
        "filter envelope",
        "key tracking",
        "unison",
        "tuning",
    ];

    // Sends `profile` to a new engine part by part, leaving out `left_out`:
    // one of PARTS, or "nothing". Then plays A4 on it.
    fn play_a4_without(profile: &SoundProfile, left_out: &str) -> Vec<f32> {
        let mut engine = Engine::new().unwrap();
        if left_out != "wave" {
            engine.set_wave(profile.wave());
        }
        if left_out != "loudness envelope" {
            engine.set_loudness_envelope(profile.loudness_envelope());
        }
        if left_out != "filter" {
            engine.set_filter(profile.filter());
        }
        if left_out != "filter envelope" {
            engine.set_filter_envelope(profile.filter_envelope());
        }
        if left_out != "key tracking" {
            engine.set_key_tracking(profile.key_tracking());
        }
        if left_out != "unison" {
            engine.set_unison(profile.unison());
        }
        if left_out != "tuning" {
            engine.set_tuning(profile.tuning_hz());
        }
        play_a4(&mut engine)
    }

    #[test]
    fn a_new_song_starts_where_a_new_engine_does() {
        let profile = SoundProfile::new();
        assert_eq!(profile.wave(), Wave::Triangle);
        assert_eq!(profile.loudness_envelope(), START_LOUDNESS_ENVELOPE);
        assert_eq!(profile.filter(), START_FILTER);
        assert_eq!(profile.filter_envelope(), START_FILTER_ENVELOPE);
        assert_eq!(profile.key_tracking(), START_KEY_TRACKING);
        assert_eq!(profile.unison(), START_UNISON);
        assert_eq!(profile.tuning_hz(), START_TUNING_HZ);
    }

    #[test]
    fn switching_steps_through_the_waves_in_order() {
        // A new song starts on the triangle, so the first switch lands on
        // the square. After the saw, it wraps around to the sine.
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        let mut names = Vec::new();
        for _ in 0..5 {
            profile.switch_wave(&mut engine);
            names.push(profile.wave().name());
        }
        assert_eq!(names, ["square", "saw", "sine", "triangle", "square"]);

        // And the engine plays the wave it landed on.
        let mut square = Engine::new().unwrap();
        square.set_wave(Wave::Square);
        let switched = play_a4(&mut engine);
        let expected = play_a4(&mut square);
        assert!(switched == expected, "the engine does not play the profile's wave");
    }

    #[test]
    fn a_profile_keeps_its_values_as_the_engine_sent_them() {
        // Every value out of range, so each comes back at its edge.
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        profile.set_loudness_envelope(
            &mut engine,
            LoudnessEnvelope {
                attack_ms: 0,
                decay_ms: 20000,
                sustain: 150,
                release_ms: 0,
            },
        );
        profile.set_filter(
            &mut engine,
            Filter {
                cutoff_hz: 50000.0,
                resonance: 150,
            },
        );
        profile.set_filter_envelope(
            &mut engine,
            FilterEnvelope {
                amount: 9.5,
                attack_ms: 20000,
                decay_ms: 20000,
                sustain: 150,
                release_ms: 20000,
            },
        );
        profile.set_key_tracking(&mut engine, 150);
        profile.set_unison(
            &mut engine,
            Unison {
                copies: 12,
                detune_cents: 80,
                spread: 150,
            },
        );
        profile.set_tuning(&mut engine, 500.0);

        let loudness_envelope = LoudnessEnvelope {
            attack_ms: LOUDNESS_SHORTEST_MS,
            decay_ms: LOUDNESS_LONGEST_MS,
            sustain: FULL_PERCENT,
            release_ms: LOUDNESS_SHORTEST_MS,
        };
        let filter = Filter {
            cutoff_hz: FILTER_OPEN_HZ,
            resonance: FULL_PERCENT,
        };
        let filter_envelope = FilterEnvelope {
            amount: FILTER_MOST_OCTAVES,
            attack_ms: FILTER_LONGEST_MS,
            decay_ms: FILTER_LONGEST_MS,
            sustain: FULL_PERCENT,
            release_ms: FILTER_LONGEST_MS,
        };
        let unison = Unison {
            copies: UNISON_MOST_COPIES,
            detune_cents: UNISON_MOST_DETUNE_CENTS,
            spread: FULL_PERCENT,
        };
        assert_eq!(profile.loudness_envelope(), loudness_envelope);
        assert_eq!(profile.filter(), filter);
        assert_eq!(profile.filter_envelope(), filter_envelope);
        assert_eq!(profile.key_tracking(), FULL_PERCENT);
        assert_eq!(profile.unison(), unison);
        assert_eq!(profile.tuning_hz(), TUNING_HIGHEST_HZ);
    }

    #[test]
    fn every_change_reaches_the_engine() {
        // Changed one by one through the profile, or sent all at once to a
        // new engine, the profile sounds the same.
        let mut changed = Engine::new().unwrap();
        let profile = odd_profile(&mut changed);
        let mut sent = Engine::new().unwrap();
        profile.send_all(&mut sent);

        let changed_sound = play_a4(&mut changed);
        let sent_sound = play_a4(&mut sent);
        assert!(changed_sound == sent_sound, "a change did not reach the engine");
    }

    #[test]
    fn sending_a_profile_sends_every_part() {
        // Each part of the odd profile changes how it sounds, so a part left
        // out would show.
        let mut scratch = Engine::new().unwrap();
        let odd = odd_profile(&mut scratch);
        let mut sent = Engine::new().unwrap();
        odd.send_all(&mut sent);
        let sent_sound = play_a4(&mut sent);

        let whole = play_a4_without(&odd, "nothing");
        assert!(sent_sound == whole, "sending the profile did not play it");
        for part in PARTS {
            let without = play_a4_without(&odd, part);
            assert!(without != sent_sound, "the profile sounds the same without its {part}");
        }
    }

    #[test]
    fn a_new_song_s_profile_sounds_like_a_new_engine() {
        // So sending it as the app starts changes nothing.
        let mut sent = Engine::new().unwrap();
        SoundProfile::new().send_all(&mut sent);
        let mut plain = Engine::new().unwrap();
        let sent_sound = play_a4(&mut sent);
        let plain_sound = play_a4(&mut plain);
        assert!(sent_sound == plain_sound, "a new song's profile changed the sound");
    }
}
