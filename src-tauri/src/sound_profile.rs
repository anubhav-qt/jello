// A song's sound profile (docs/adr/0030-a-sound-profile-per-song.md):
// everything about how its notes sound, which is its wave, its low-pass
// filter and filter envelope, and its unison. The song holds it. Every change
// to the sound goes through it to the engine, so the engine plays only what
// the song says.

use crate::engine::{
    Engine, Filter, FilterEnvelope, START_FILTER, START_FILTER_ENVELOPE, START_UNISON, START_WAVE, Unison, Wave,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundProfile {
    // The wave new notes play.
    wave: Wave,
    // Where the filter is. Every note hears it, even those already sounding.
    filter: Filter,
    // The filter envelope new notes start with.
    filter_envelope: FilterEnvelope,
    // The copies new notes are played by.
    unison: Unison,
}

impl SoundProfile {
    // A new song's sound profile: a triangle wave, the filter open, no
    // filter envelope and 1 copy. It is only where a new song starts, the
    // same place a new engine starts, and the song changes it from there.
    pub fn new() -> SoundProfile {
        SoundProfile {
            wave: START_WAVE,
            filter: START_FILTER,
            filter_envelope: START_FILTER_ENVELOPE,
            unison: START_UNISON,
        }
    }

    // Sends the whole profile to `engine`, so the notes started after this
    // play it. The app does this as it starts, before the first note.
    pub fn send_all(&self, engine: &mut Engine) {
        engine.set_wave(self.wave);
        engine.set_filter(self.filter);
        engine.set_filter_envelope(self.filter_envelope);
        engine.set_unison(self.unison);
    }

    // Moves new notes on to the next wave (sine, triangle, square, saw, then
    // sine again), on `engine` too. Notes already playing keep their wave.
    pub fn switch_wave(&mut self, engine: &mut Engine) {
        self.wave = self.wave.next();
        engine.set_wave(self.wave);
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

    // Sets unison for the notes started after this, on `engine` too, kept
    // as the engine sent it.
    pub fn set_unison(&mut self, engine: &mut Engine, unison: Unison) {
        self.unison = engine.set_unison(unison);
    }

    pub fn wave(&self) -> Wave {
        self.wave
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    pub fn filter_envelope(&self) -> FilterEnvelope {
        self.filter_envelope
    }

    pub fn unison(&self) -> Unison {
        self.unison
    }
}

// These tests check that a profile keeps what it was given, as the engine
// sent it, and that the engine plays exactly the profile. They render without
// speakers.
#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::{
        FILTER_LONGEST_MS, FILTER_MOST_OCTAVES, FILTER_OPEN_HZ, FULL_PERCENT, UNISON_MOST_COPIES,
        UNISON_MOST_DETUNE_CENTS,
    };

    const A4: u8 = 69;
    const VELOCITY: u8 = 100;

    // A profile unlike a new song's in every way: a saw, a closed filter
    // with resonance, a filter envelope and 5 spread copies.
    fn odd_profile(engine: &mut Engine) -> SoundProfile {
        let mut profile = SoundProfile::new();
        profile.switch_wave(engine);
        profile.switch_wave(engine);
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
        profile.set_unison(
            engine,
            Unison {
                copies: 5,
                detune_cents: 25,
                spread: 80,
            },
        );
        profile
    }

    // Plays A4 on `engine` and returns its first 300 ms, left and right.
    fn play_a4(engine: &mut Engine) -> Vec<f32> {
        engine.note_on(A4, VELOCITY);
        let frames = engine.sample_rate() * 300 / 1000;
        engine.render(frames as usize)
    }

    // Sends `profile` to a new engine part by part, leaving out `left_out`:
    // "wave", "filter", "filter envelope", "unison" or "nothing". Then
    // plays A4 on it.
    fn play_a4_without(profile: &SoundProfile, left_out: &str) -> Vec<f32> {
        let mut engine = Engine::new().unwrap();
        if left_out != "wave" {
            engine.set_wave(profile.wave());
        }
        if left_out != "filter" {
            engine.set_filter(profile.filter());
        }
        if left_out != "filter envelope" {
            engine.set_filter_envelope(profile.filter_envelope());
        }
        if left_out != "unison" {
            engine.set_unison(profile.unison());
        }
        play_a4(&mut engine)
    }

    #[test]
    fn a_new_song_starts_with_a_triangle_the_filter_open_and_one_copy() {
        let profile = SoundProfile::new();
        assert_eq!(profile.wave(), Wave::Triangle);
        assert_eq!(profile.filter(), START_FILTER);
        assert_eq!(profile.filter_envelope(), START_FILTER_ENVELOPE);
        assert_eq!(profile.unison(), START_UNISON);
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
        profile.set_unison(
            &mut engine,
            Unison {
                copies: 12,
                detune_cents: 80,
                spread: 150,
            },
        );

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
        assert_eq!(profile.filter(), filter);
        assert_eq!(profile.filter_envelope(), filter_envelope);
        assert_eq!(profile.unison(), unison);
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
        for part in ["wave", "filter", "filter envelope", "unison"] {
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
