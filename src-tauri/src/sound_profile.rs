// A song's sound profile (docs/adr/0030-a-sound-profile-per-song.md):
// everything about how its notes sound, which is its oscillators
// (docs/adr/0035-oscillators-with-kinds.md), its loudness envelope, its low-pass filter with its filter envelope and key tracking,
// its unison, its tuning, and its LFOs, routing list and vibrato key times
// (docs/adr/0031-lfos-and-a-routing-list.md). The song holds it. Every change
// to the sound goes through it to the engine, so the engine plays only what
// the song says.

use crate::engine::{
    Engine, Filter, FilterEnvelope, LoudnessEnvelope, OSCILLATOR_COUNT, Oscillator, START_FILTER,
    START_FILTER_ENVELOPE, START_KEY_TRACKING, START_LOUDNESS_ENVELOPE, START_OSCILLATORS, START_TUNING_HZ,
    START_UNISON, Unison,
};
use crate::modulation::{
    CONNECTION_COUNT, Connection, LFO_COUNT, Lfo, NO_CONNECTION, START_LFO, Source, Target, VibratoKeyTimes,
};

// A new song comes with vibrato ready on the vibrato key
// (docs/adr/0032-the-vibrato-key.md): its first connection swings the pitch
// 25 cents either way with LFO 1, a per-note sine at 5.5 Hz, as far as the
// key is held. Up, it moves nothing, so the song sounds just as it would
// without it. Only a starting point; each song changes it as it likes.
const START_VIBRATO: Connection = Connection {
    source: Source::Lfo(0),
    target: Target::Pitch,
    amount: 25.0,
    one_way: false,
    snap: false,
    scaled_by: Source::VibratoKey,
};

// The vibrato key rises and falls over a quarter of a second in a new song.
const START_SONG_KEY_TIMES: VibratoKeyTimes = VibratoKeyTimes {
    rise_ms: 250,
    fall_ms: 250,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundProfile {
    // The oscillators new notes play.
    oscillators: [Oscillator; OSCILLATOR_COUNT],
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
    // The LFOs, the routing list's connections and how long the vibrato key
    // takes to rise and fall.
    lfos: [Lfo; LFO_COUNT],
    connections: [Connection; CONNECTION_COUNT],
    vibrato_key_times: VibratoKeyTimes,
}

impl SoundProfile {
    // A new song's sound profile: one triangle oscillator at full level and
    // two silent ones, a 5 ms attack held at the
    // full peak with a 300 ms release, the filter open, no filter envelope,
    // full key tracking, 1 copy and A4 at 440 Hz, the same place a new
    // engine starts, plus vibrato ready on the vibrato key. It is only where
    // a new song starts, and the song changes it from there.
    pub fn new() -> SoundProfile {
        let mut connections = [NO_CONNECTION; CONNECTION_COUNT];
        connections[0] = START_VIBRATO;
        SoundProfile {
            oscillators: START_OSCILLATORS,
            loudness_envelope: START_LOUDNESS_ENVELOPE,
            filter: START_FILTER,
            filter_envelope: START_FILTER_ENVELOPE,
            key_tracking: START_KEY_TRACKING,
            unison: START_UNISON,
            tuning_hz: START_TUNING_HZ,
            lfos: [START_LFO; LFO_COUNT],
            connections,
            vibrato_key_times: START_SONG_KEY_TIMES,
        }
    }

    // Sends the whole profile to `engine`, so the notes started after this
    // play it. The song does this as the app starts, before the first note.
    pub fn send_all(&self, engine: &mut Engine) {
        for (index, oscillator) in self.oscillators.iter().enumerate() {
            engine.set_oscillator(index, *oscillator);
        }
        engine.set_loudness_envelope(self.loudness_envelope);
        engine.set_filter(self.filter);
        engine.set_filter_envelope(self.filter_envelope);
        engine.set_key_tracking(self.key_tracking);
        engine.set_unison(self.unison);
        engine.set_tuning(self.tuning_hz);
        for (index, lfo) in self.lfos.iter().enumerate() {
            engine.set_lfo(index, *lfo);
        }
        for (index, connection) in self.connections.iter().enumerate() {
            engine.set_connection(index, *connection);
        }
        engine.set_vibrato_key_times(self.vibrato_key_times);
    }

    // Moves new notes' first oscillator on to the next kind (sine,
    // triangle, square, saw, noise, then sine again), on `engine` too. Notes
    // already playing keep theirs.
    pub fn switch_first_kind(&mut self, engine: &mut Engine) {
        let mut first = self.oscillators[0];
        first.kind = first.kind.next();
        self.set_oscillator(engine, 0, first);
    }

    // Sets oscillator `index`, from 0, for the notes started after this, on
    // `engine` too, kept as the engine sent it. An index past the
    // oscillators changes nothing.
    pub fn set_oscillator(&mut self, engine: &mut Engine, index: usize, oscillator: Oscillator) {
        if index >= OSCILLATOR_COUNT {
            return;
        }
        self.oscillators[index] = engine.set_oscillator(index, oscillator);
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

    // Sets LFO `index`, from 0, on `engine` too, kept as the engine sent
    // it. An index past the LFOs changes nothing.
    pub fn set_lfo(&mut self, engine: &mut Engine, index: usize, lfo: Lfo) {
        if index >= LFO_COUNT {
            return;
        }
        self.lfos[index] = engine.set_lfo(index, lfo);
    }

    // Sets connection `index` of the routing list, from 0, on `engine` too,
    // kept as the engine sent it. An index past the connections changes
    // nothing.
    pub fn set_connection(&mut self, engine: &mut Engine, index: usize, connection: Connection) {
        if index >= CONNECTION_COUNT {
            return;
        }
        self.connections[index] = engine.set_connection(index, connection);
    }

    // Sets how long the vibrato key takes to rise and fall, on `engine` too,
    // kept as the engine sent it.
    pub fn set_vibrato_key_times(&mut self, engine: &mut Engine, times: VibratoKeyTimes) {
        self.vibrato_key_times = engine.set_vibrato_key_times(times);
    }

    pub fn oscillators(&self) -> &[Oscillator; OSCILLATOR_COUNT] {
        &self.oscillators
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

    pub fn lfos(&self) -> &[Lfo; LFO_COUNT] {
        &self.lfos
    }

    pub fn connections(&self) -> &[Connection; CONNECTION_COUNT] {
        &self.connections
    }

    pub fn vibrato_key_times(&self) -> VibratoKeyTimes {
        self.vibrato_key_times
    }
}

// These tests check that a profile keeps what it was given, as the engine
// sent it, and that the engine plays exactly the profile. They render without
// speakers.
#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::{
        FILTER_LONGEST_MS, FILTER_MOST_OCTAVES, FILTER_OPEN_HZ, FULL_PERCENT, Kind, LOUDNESS_LONGEST_MS,
        LOUDNESS_SHORTEST_MS, OSCILLATOR_MOST_CENTS, OSCILLATOR_MOST_HALF_STEPS, OSCILLATOR_MOST_OCTAVES,
        TUNING_HIGHEST_HZ, UNISON_MOST_COPIES, UNISON_MOST_DETUNE_CENTS,
    };
    use crate::modulation::{LFO_FASTEST_HZ, LfoShape, VIBRATO_KEY_LONGEST_MS};

    const A4: u8 = 69;
    const VELOCITY: u8 = 100;

    // A profile unlike a new song's in every way: a saw, with a square an
    // octave and a fifth down at 40%, a slow attack that
    // falls to half the peak, a closed filter with resonance, a filter
    // envelope, half key tracking, 5 spread copies, A4 at 432 Hz, LFO 1 a
    // triangle at 7 Hz, LFO 2 moving the cutoff, and a vibrato key that
    // rises over 100 ms.
    fn odd_profile(engine: &mut Engine) -> SoundProfile {
        // The app sends a new song's profile as it starts, then changes it.
        let mut profile = SoundProfile::new();
        profile.send_all(engine);
        profile.switch_first_kind(engine);
        profile.switch_first_kind(engine);
        let square_below = Oscillator {
            kind: Kind::Square,
            level: 40,
            octaves: -1,
            half_steps: -5,
            cents: 3,
        };
        profile.set_oscillator(engine, 1, square_below);
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
        let triangle = Lfo {
            shape: LfoShape::Triangle,
            hz: 7.0,
            ..START_LFO
        };
        profile.set_lfo(engine, 0, triangle);
        let cutoff_moving = Connection {
            source: Source::Lfo(1),
            target: Target::Cutoff,
            amount: 1.0,
            one_way: false,
            snap: false,
            scaled_by: Source::None,
        };
        profile.set_connection(engine, 1, cutoff_moving);
        let times = VibratoKeyTimes {
            rise_ms: 100,
            fall_ms: 50,
        };
        profile.set_vibrato_key_times(engine, times);
        profile
    }

    // Plays A4 on `engine` and returns its first 300 ms, left and right.
    fn play_a4(engine: &mut Engine) -> Vec<f32> {
        engine.note_on(A4, VELOCITY);
        let frames = engine.sample_rate() * 300 / 1000;
        engine.render(frames as usize)
    }

    // The same with the vibrato key held, so whatever it moves is heard.
    fn play_a4_with_the_key(engine: &mut Engine) -> Vec<f32> {
        engine.set_vibrato_key(true);
        play_a4(engine)
    }

    // Every part of a profile, as play_a4_without names them.
    const PARTS: [&str; 10] = [
        "oscillators",
        "loudness envelope",
        "filter",
        "filter envelope",
        "key tracking",
        "unison",
        "tuning",
        "lfos",
        "connections",
        "vibrato key times",
    ];

    // Sends `profile` to a new engine part by part, leaving out `left_out`:
    // one of PARTS, or "nothing". Then plays A4 on it.
    fn play_a4_without(profile: &SoundProfile, left_out: &str) -> Vec<f32> {
        let mut engine = Engine::new().unwrap();
        if left_out != "oscillators" {
            for (index, oscillator) in profile.oscillators().iter().enumerate() {
                engine.set_oscillator(index, *oscillator);
            }
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
        if left_out != "lfos" {
            for (index, lfo) in profile.lfos().iter().enumerate() {
                engine.set_lfo(index, *lfo);
            }
        }
        if left_out != "connections" {
            for (index, connection) in profile.connections().iter().enumerate() {
                engine.set_connection(index, *connection);
            }
        }
        if left_out != "vibrato key times" {
            engine.set_vibrato_key_times(profile.vibrato_key_times());
        }
        play_a4_with_the_key(&mut engine)
    }

    #[test]
    fn a_new_song_starts_where_a_new_engine_does() {
        let profile = SoundProfile::new();
        assert_eq!(profile.oscillators(), &START_OSCILLATORS);
        assert_eq!(profile.loudness_envelope(), START_LOUDNESS_ENVELOPE);
        assert_eq!(profile.filter(), START_FILTER);
        assert_eq!(profile.filter_envelope(), START_FILTER_ENVELOPE);
        assert_eq!(profile.key_tracking(), START_KEY_TRACKING);
        assert_eq!(profile.unison(), START_UNISON);
        assert_eq!(profile.tuning_hz(), START_TUNING_HZ);
        assert_eq!(profile.lfos(), &[START_LFO; LFO_COUNT]);
    }

    #[test]
    fn a_new_song_has_vibrato_ready_on_the_vibrato_key() {
        // Its first connection swings the pitch 25 cents either way with LFO
        // 1, scaled by the key, which rises and falls over 250 ms. No other
        // connection is used.
        let profile = SoundProfile::new();
        let vibrato = Connection {
            source: Source::Lfo(0),
            target: Target::Pitch,
            amount: 25.0,
            one_way: false,
            snap: false,
            scaled_by: Source::VibratoKey,
        };
        assert_eq!(profile.connections()[0], vibrato);
        for connection in &profile.connections()[1..] {
            assert_eq!(*connection, NO_CONNECTION);
        }
        let times = VibratoKeyTimes {
            rise_ms: 250,
            fall_ms: 250,
        };
        assert_eq!(profile.vibrato_key_times(), times);

        // Held from the start, A4 swings 25 cents either way of 440 Hz, about
        // 6.4 Hz, five and a half times a second, once the key has risen.
        let mut engine = Engine::new().unwrap();
        profile.send_all(&mut engine);
        engine.set_vibrato_key(true);
        engine.note_on(A4, VELOCITY);
        let rate = engine.sample_rate();
        let wave = left_side(&engine.render(rate as usize * 2));
        let risen = cycle_pitches(&wave[rate as usize / 2..], rate);
        let swing = 440.0 * 2.0_f64.powf(25.0 / 1200.0) - 440.0;
        assert!((highest(&risen) - 440.0 - swing).abs() < 0.5, "it swung up to {}", highest(&risen));
        assert!((440.0 - lowest(&risen) - swing).abs() < 0.5, "it swung down to {}", lowest(&risen));
        let swings = rises_through(&risen, 440.0);
        assert!(swings == 8 || swings == 9, "it swung {swings} times in 1.5 seconds");

        // In its first 40 ms, the key has risen less than a sixth of the way.
        let rising = cycle_pitches(&wave[..rate as usize / 25], rate);
        assert!(highest(&rising) - 440.0 < swing / 6.0, "it rose to {} at once", highest(&rising));
        assert!(440.0 - lowest(&rising) < swing / 6.0, "it fell to {} at once", lowest(&rising));
    }

    // The left side of `samples`, left and right.
    fn left_side(samples: &[f32]) -> Vec<f32> {
        let mut left = Vec::new();
        for frame in samples.chunks(2) {
            left.push(frame[0]);
        }
        left
    }

    // The pitch of each cycle of `wave`, in Hz, from one crossing of zero on
    // the way up to the next. Each crossing falls between two samples, found
    // by a straight line between them.
    fn cycle_pitches(wave: &[f32], rate: u32) -> Vec<f64> {
        let mut crossings = Vec::new();
        for i in 1..wave.len() {
            let before = wave[i - 1] as f64;
            let after = wave[i] as f64;
            if before < 0.0 && after >= 0.0 {
                let fraction = before / (before - after);
                crossings.push((i - 1) as f64 + fraction);
            }
        }
        let mut pitches = Vec::new();
        for pair in crossings.windows(2) {
            pitches.push(rate as f64 / (pair[1] - pair[0]));
        }
        pitches
    }

    fn highest(values: &[f64]) -> f64 {
        let mut highest = values[0];
        for value in values {
            highest = f64::max(highest, *value);
        }
        highest
    }

    fn lowest(values: &[f64]) -> f64 {
        let mut lowest = values[0];
        for value in values {
            lowest = f64::min(lowest, *value);
        }
        lowest
    }

    // How many times `values` go from below `level` to above it.
    fn rises_through(values: &[f64], level: f64) -> usize {
        let mut rises = 0;
        for pair in values.windows(2) {
            if pair[0] < level && pair[1] >= level {
                rises += 1;
            }
        }
        rises
    }

    #[test]
    fn switching_steps_the_first_oscillator_through_the_kinds_in_order() {
        // A new song starts on the triangle, so the first switch lands on
        // the square. After the noise, it wraps around to the sine. The
        // other oscillators stay as they are.
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        let mut names = Vec::new();
        for _ in 0..6 {
            profile.switch_first_kind(&mut engine);
            names.push(profile.oscillators()[0].kind.name());
        }
        assert_eq!(names, ["square", "saw", "noise", "sine", "triangle", "square"]);
        assert_eq!(profile.oscillators()[1..], START_OSCILLATORS[1..]);

        // And the engine plays the kind it landed on.
        let mut square = Engine::new().unwrap();
        square.set_first_kind(Kind::Square);
        let switched = play_a4(&mut engine);
        let expected = play_a4(&mut square);
        assert!(switched == expected, "the engine does not play the profile's kind");
    }

    #[test]
    fn setting_an_oscillator_plays_it() {
        // The sound panel sets each oscillator's values, not only its kind.
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        let fifth_up = Oscillator {
            kind: Kind::Saw,
            level: 50,
            octaves: 0,
            half_steps: 7,
            cents: -4,
        };
        profile.set_oscillator(&mut engine, 2, fifth_up);
        assert_eq!(profile.oscillators()[2], fifth_up);

        let mut expected_engine = Engine::new().unwrap();
        expected_engine.set_oscillator(2, fifth_up);
        let set = play_a4(&mut engine);
        let expected = play_a4(&mut expected_engine);
        assert!(set == expected, "the engine does not play the oscillator the profile set");
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
        let too_far_off = Oscillator {
            kind: Kind::Noise,
            level: 150,
            octaves: 5,
            half_steps: -20,
            cents: 300,
        };
        profile.set_oscillator(&mut engine, 1, too_far_off);
        let too_fast = Lfo {
            hz: 200.0,
            start: 150,
            ..START_LFO
        };
        profile.set_lfo(&mut engine, 3, too_fast);
        let too_far = Connection {
            source: Source::Lfo(9),
            target: Target::Pitch,
            amount: 6000.0,
            one_way: false,
            snap: true,
            scaled_by: Source::None,
        };
        profile.set_connection(&mut engine, 5, too_far);
        let too_long = VibratoKeyTimes {
            rise_ms: 20000,
            fall_ms: 20000,
        };
        profile.set_vibrato_key_times(&mut engine, too_long);

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
        let at_the_edges = Oscillator {
            kind: Kind::Noise,
            level: FULL_PERCENT,
            octaves: OSCILLATOR_MOST_OCTAVES,
            half_steps: -OSCILLATOR_MOST_HALF_STEPS,
            cents: OSCILLATOR_MOST_CENTS,
        };
        assert_eq!(profile.oscillators()[1], at_the_edges);
        let at_the_top = Lfo {
            hz: LFO_FASTEST_HZ,
            start: FULL_PERCENT,
            ..START_LFO
        };
        assert_eq!(profile.lfos()[3], at_the_top);
        let as_far_as_it_goes = Connection {
            source: Source::Lfo(LFO_COUNT - 1),
            amount: 4800.0,
            ..too_far
        };
        assert_eq!(profile.connections()[5], as_far_as_it_goes);
        let longest = VibratoKeyTimes {
            rise_ms: VIBRATO_KEY_LONGEST_MS,
            fall_ms: VIBRATO_KEY_LONGEST_MS,
        };
        assert_eq!(profile.vibrato_key_times(), longest);

        // An oscillator, LFO or connection past the last changes nothing.
        let before = profile;
        profile.set_oscillator(&mut engine, OSCILLATOR_COUNT, too_far_off);
        profile.set_lfo(&mut engine, LFO_COUNT, too_fast);
        profile.set_connection(&mut engine, CONNECTION_COUNT, too_far);
        assert_eq!(profile, before);
    }

    #[test]
    fn every_change_reaches_the_engine() {
        // Changed one by one through the profile, or sent all at once to a
        // new engine, the profile sounds the same.
        let mut changed = Engine::new().unwrap();
        let profile = odd_profile(&mut changed);
        let mut sent = Engine::new().unwrap();
        profile.send_all(&mut sent);

        let changed_sound = play_a4_with_the_key(&mut changed);
        let sent_sound = play_a4_with_the_key(&mut sent);
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
        let sent_sound = play_a4_with_the_key(&mut sent);

        let whole = play_a4_without(&odd, "nothing");
        assert!(sent_sound == whole, "sending the profile did not play it");
        for part in PARTS {
            let without = play_a4_without(&odd, part);
            assert!(without != sent_sound, "the profile sounds the same without its {part}");
        }
    }

    #[test]
    fn a_new_song_s_profile_sounds_like_a_new_engine() {
        // So sending it as the app starts changes nothing, with the vibrato
        // key up.
        let mut sent = Engine::new().unwrap();
        SoundProfile::new().send_all(&mut sent);
        let mut plain = Engine::new().unwrap();
        let sent_sound = play_a4(&mut sent);
        let plain_sound = play_a4(&mut plain);
        assert!(sent_sound == plain_sound, "a new song's profile changed the sound");
    }
}
