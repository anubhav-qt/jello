// The song's LFOs and routing list (docs/adr/0031-lfos-and-a-routing-list.md),
// as the Rust core keeps them: in the units the screen shows, with a plain
// name for every shape, source and target, so song files and the AI harness
// can write them later. src/engine.rs sends them on to the C++ engine.

use crate::engine::OSCILLATOR_COUNT;

// How many LFOs and connections a song has, and the most steps an LFO's
// steps shape can have (`JELLO_LFO_COUNT`, `JELLO_CONNECTION_COUNT` and
// `JELLO_LFO_MOST_STEPS` in engine/include/jello_engine.h).
pub const LFO_COUNT: usize = 8;
pub const CONNECTION_COUNT: usize = 64;
pub const LFO_MOST_STEPS: usize = 32;

// An LFO's ranges: its speed in Hz, in beats a cycle and in key presses a
// cycle, and its delay and fade-in in ms and in beats.
pub const LFO_SLOWEST_HZ: f64 = 0.01;
pub const LFO_FASTEST_HZ: f64 = 100.0;
pub const LFO_SHORTEST_BEATS: f64 = 1.0 / 16.0;
pub const LFO_LONGEST_BEATS: f64 = 128.0;
pub const LFO_MOST_PRESSES: u32 = 32;
pub const LFO_LONGEST_MS: u32 = 10000;
pub const LFO_LONGEST_DELAY_BEATS: f64 = 128.0;

// The longest the vibrato key takes to rise or fall, in ms.
pub const VIBRATO_KEY_LONGEST_MS: u32 = 10000;

// The shapes an LFO can have, each from -100% to 100%.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoShape {
    // Swings smoothly up, down, then back to the middle.
    Sine,
    // The same in straight lines.
    Triangle,
    // Up for the first half of each cycle, down for the second.
    Square,
    // Rises from the bottom to the top, then jumps back down.
    SawUp,
    // Falls from the top to the bottom, then jumps back up.
    SawDown,
    // A new random height each cycle, held.
    RandomSteps,
    // Glides from one random height to the next each cycle.
    SmoothRandom,
    // The heights you set, one after another, each held.
    Steps,
}

impl LfoShape {
    pub const ALL: [LfoShape; 8] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::Square,
        LfoShape::SawUp,
        LfoShape::SawDown,
        LfoShape::RandomSteps,
        LfoShape::SmoothRandom,
        LfoShape::Steps,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LfoShape::Sine => "sine",
            LfoShape::Triangle => "triangle",
            LfoShape::Square => "square",
            LfoShape::SawUp => "saw up",
            LfoShape::SawDown => "saw down",
            LfoShape::RandomSteps => "random steps",
            LfoShape::SmoothRandom => "smooth random",
            LfoShape::Steps => "steps",
        }
    }

    // The shape called `name`, or None.
    pub fn from_name(name: &str) -> Option<LfoShape> {
        for shape in LfoShape::ALL {
            if shape.name() == name {
                return Some(shape);
            }
        }
        None
    }

    // Its number in the C API (`JELLO_LFO_` shapes).
    pub(crate) fn number(self) -> i32 {
        match self {
            LfoShape::Sine => 0,
            LfoShape::Triangle => 1,
            LfoShape::Square => 2,
            LfoShape::SawUp => 3,
            LfoShape::SawDown => 4,
            LfoShape::RandomSteps => 5,
            LfoShape::SmoothRandom => 6,
            LfoShape::Steps => 7,
        }
    }
}

// What an LFO's speed is counted in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoSpeed {
    // Cycles a second.
    Hz,
    // How many beats one cycle lasts, at the song's tempo.
    Beats,
    // How many key presses one cycle takes. A chord is one press.
    Presses,
}

impl LfoSpeed {
    pub const ALL: [LfoSpeed; 3] = [LfoSpeed::Hz, LfoSpeed::Beats, LfoSpeed::Presses];

    pub fn name(self) -> &'static str {
        match self {
            LfoSpeed::Hz => "hz",
            LfoSpeed::Beats => "beats",
            LfoSpeed::Presses => "presses",
        }
    }

    // The speed called `name`, or None.
    pub fn from_name(name: &str) -> Option<LfoSpeed> {
        for speed in LfoSpeed::ALL {
            if speed.name() == name {
                return Some(speed);
            }
        }
        None
    }

    // Its number in the C API (`JELLO_LFO_HZ`, `_BEATS` and `_PRESSES`).
    pub(crate) fn number(self) -> i32 {
        match self {
            LfoSpeed::Hz => 0,
            LfoSpeed::Beats => 1,
            LfoSpeed::Presses => 2,
        }
    }
}

// One LFO: a slow wave that moves whatever the routing list connects it to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lfo {
    pub shape: LfoShape,
    // For the steps shape: how many steps, from 1 to LFO_MOST_STEPS, and
    // each one's height in percent, from -100 to 100. Only the first
    // `step_count` count.
    pub step_count: u32,
    pub steps: [f32; LFO_MOST_STEPS],
    // True: every note starts its own copy of it. False: one runs for every
    // note.
    pub per_note: bool,
    // What its speed is counted in, and its speed in each.
    pub speed: LfoSpeed,
    pub hz: f64,
    pub beats: f64,
    pub presses: u32,
    // Where in its cycle it starts, in percent, and for a per-note LFO how
    // far each note's start is moved at random on top, in percent of a
    // cycle.
    pub start: u32,
    pub spread: u32,
    // How long after each note starts it waits, then how long it takes to
    // grow to its full size: in ms, or in beats while its speed is in beats.
    pub delay_ms: u32,
    pub fade_ms: u32,
    pub delay_beats: f64,
    pub fade_beats: f64,
    // How much of the time between jumps is spent sliding instead, in
    // percent.
    pub smooth: u32,
    // For a per-note LFO: one cycle, then it holds where it ended.
    pub once: bool,
}

// Where every LFO of a new engine starts, and of a new song: a per-note sine
// at 5.5 Hz, starting anywhere in its cycle, with no delay, smoothed 10%.
pub const START_LFO: Lfo = Lfo {
    shape: LfoShape::Sine,
    step_count: 8,
    steps: [0.0; LFO_MOST_STEPS],
    per_note: true,
    speed: LfoSpeed::Hz,
    hz: 5.5,
    beats: 1.0,
    presses: 8,
    start: 0,
    spread: 100,
    delay_ms: 0,
    fade_ms: 0,
    delay_beats: 0.0,
    fade_beats: 0.0,
    smooth: 10,
    once: false,
};

// What moves a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    // An unused connection.
    None,
    // LFO number n, from 0. Called "lfo 1" to "lfo 8".
    Lfo(usize),
    // 0 while up, 100% while held, rising and falling in straight lines.
    VibratoKey,
    // The note's height: -100% five octaves below middle C, 100% five above.
    NoteHeight,
    // A random number each note starts with.
    Random,
    // Where the note's loudness envelope is, as a share of its peak.
    LoudnessEnvelope,
    // Where the note's filter envelope is.
    FilterEnvelope,
}

impl Source {
    // Every source but the LFOs, which come in LFO_COUNT.
    const OTHERS: [Source; 6] = [
        Source::None,
        Source::VibratoKey,
        Source::NoteHeight,
        Source::Random,
        Source::LoudnessEnvelope,
        Source::FilterEnvelope,
    ];

    pub fn name(self) -> String {
        match self {
            Source::None => String::from("none"),
            Source::Lfo(index) => format!("lfo {}", index + 1),
            Source::VibratoKey => String::from("vibrato key"),
            Source::NoteHeight => String::from("note height"),
            Source::Random => String::from("random"),
            Source::LoudnessEnvelope => String::from("loudness envelope"),
            Source::FilterEnvelope => String::from("filter envelope"),
        }
    }

    // The source called `name`, or None for no such name.
    pub fn from_name(name: &str) -> Option<Source> {
        for source in Source::OTHERS {
            if source.name() == name {
                return Some(source);
            }
        }
        for index in 0..LFO_COUNT {
            let lfo = Source::Lfo(index);
            if lfo.name() == name {
                return Some(lfo);
            }
        }
        None
    }

    // Its number in the C API (`JELLO_SOURCE_`).
    pub(crate) fn number(self) -> i32 {
        match self {
            Source::None => 0,
            Source::Lfo(index) => 1 + index as i32,
            Source::VibratoKey => 9,
            Source::NoteHeight => 10,
            Source::Random => 11,
            Source::LoudnessEnvelope => 12,
            Source::FilterEnvelope => 13,
        }
    }

    // The same source, with an LFO's number brought within LFO_COUNT.
    pub fn within_range(self) -> Source {
        match self {
            Source::Lfo(index) => Source::Lfo(index.min(LFO_COUNT - 1)),
            other => other,
        }
    }
}

// The units a target is moved in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    // Hundredths of a half step.
    Cents,
    // Of the target's whole range: 100% of loudness is down to silence.
    Percent,
    // Each one doubles: the cutoff's pitch, an LFO's speed, or a time.
    Octaves,
    // Whole steps through the oscillators' kinds or the filter's types.
    Steps,
    // Whole copies.
    Copies,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Unit::Cents => "cents",
            Unit::Percent => "%",
            Unit::Octaves => "octaves",
            Unit::Steps => "steps",
            Unit::Copies => "copies",
        }
    }
}

// What a connection moves. Those before `OscillatorKind` move live, while a
// note sounds; the rest are read once as each note starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Pitch,
    Loudness,
    Pan,
    Cutoff,
    Resonance,
    KeyTracking,
    Detune,
    Spread,
    LoudnessSustain,
    FilterAmount,
    FilterSustain,
    // Whole steps through low-pass, high-pass, band-pass and notch, and
    // round again (docs/design.md#filter-types).
    FilterType,
    // LFO number n's speed, from 0. Called "lfo 1 speed" to "lfo 8 speed".
    LfoSpeed(usize),
    // Oscillator number n's level, pitch, width and kind, from 0
    // (docs/adr/0035-oscillators-with-kinds.md). Called "osc 1 level",
    // "osc 1 pitch", "osc 1 width" and "osc 1 kind", and so on to "osc 3
    // kind". Only a square hears its width (docs/design.md#pulse-width).
    OscillatorLevel(usize),
    OscillatorPitch(usize),
    OscillatorWidth(usize),
    OscillatorKind(usize),
    Copies,
    LoudnessAttack,
    LoudnessDecay,
    LoudnessRelease,
    FilterAttack,
    FilterDecay,
    FilterRelease,
    // How long each melody note glides (docs/adr/0036-glide.md).
    GlideTime,
}

impl Target {
    // Every target but the LFOs' speeds, which come in LFO_COUNT, and the
    // oscillators' targets, which come in OSCILLATOR_COUNT.
    pub(crate) const OTHERS: [Target; 20] = [
        Target::Pitch,
        Target::Loudness,
        Target::Pan,
        Target::Cutoff,
        Target::Resonance,
        Target::KeyTracking,
        Target::Detune,
        Target::Spread,
        Target::LoudnessSustain,
        Target::FilterAmount,
        Target::FilterSustain,
        Target::FilterType,
        Target::Copies,
        Target::LoudnessAttack,
        Target::LoudnessDecay,
        Target::LoudnessRelease,
        Target::FilterAttack,
        Target::FilterDecay,
        Target::FilterRelease,
        Target::GlideTime,
    ];

    pub fn name(self) -> String {
        match self {
            Target::Pitch => String::from("pitch"),
            Target::Loudness => String::from("loudness"),
            Target::Pan => String::from("pan"),
            Target::Cutoff => String::from("cutoff"),
            Target::Resonance => String::from("resonance"),
            Target::KeyTracking => String::from("key tracking"),
            Target::Detune => String::from("detune"),
            Target::Spread => String::from("spread"),
            Target::LoudnessSustain => String::from("loudness sustain"),
            Target::FilterAmount => String::from("filter amount"),
            Target::FilterSustain => String::from("filter sustain"),
            Target::FilterType => String::from("filter type"),
            Target::LfoSpeed(index) => format!("lfo {} speed", index + 1),
            Target::OscillatorLevel(index) => format!("osc {} level", index + 1),
            Target::OscillatorPitch(index) => format!("osc {} pitch", index + 1),
            Target::OscillatorWidth(index) => format!("osc {} width", index + 1),
            Target::OscillatorKind(index) => format!("osc {} kind", index + 1),
            Target::Copies => String::from("copies"),
            Target::LoudnessAttack => String::from("loudness attack"),
            Target::LoudnessDecay => String::from("loudness decay"),
            Target::LoudnessRelease => String::from("loudness release"),
            Target::FilterAttack => String::from("filter attack"),
            Target::FilterDecay => String::from("filter decay"),
            Target::FilterRelease => String::from("filter release"),
            Target::GlideTime => String::from("glide time"),
        }
    }

    // The target called `name`, or None for no such name.
    pub fn from_name(name: &str) -> Option<Target> {
        for target in Target::OTHERS {
            if target.name() == name {
                return Some(target);
            }
        }
        for index in 0..LFO_COUNT {
            let speed = Target::LfoSpeed(index);
            if speed.name() == name {
                return Some(speed);
            }
        }
        for index in 0..OSCILLATOR_COUNT {
            let oscillator_targets = [
                Target::OscillatorLevel(index),
                Target::OscillatorPitch(index),
                Target::OscillatorWidth(index),
                Target::OscillatorKind(index),
            ];
            for target in oscillator_targets {
                if target.name() == name {
                    return Some(target);
                }
            }
        }
        None
    }

    // Its number in the C API (`JELLO_TARGET_`).
    pub(crate) fn number(self) -> i32 {
        match self {
            Target::Pitch => 0,
            Target::Loudness => 1,
            Target::Pan => 2,
            Target::Cutoff => 3,
            Target::Resonance => 4,
            Target::KeyTracking => 5,
            Target::Detune => 6,
            Target::Spread => 7,
            Target::LoudnessSustain => 8,
            Target::FilterAmount => 9,
            Target::FilterSustain => 10,
            Target::FilterType => 11,
            Target::LfoSpeed(index) => 12 + index as i32,
            Target::OscillatorLevel(index) => 20 + index as i32,
            Target::OscillatorPitch(index) => 23 + index as i32,
            Target::OscillatorWidth(index) => 26 + index as i32,
            Target::OscillatorKind(index) => 29 + index as i32,
            Target::Copies => 32,
            Target::LoudnessAttack => 33,
            Target::LoudnessDecay => 34,
            Target::LoudnessRelease => 35,
            Target::FilterAttack => 36,
            Target::FilterDecay => 37,
            Target::FilterRelease => 38,
            Target::GlideTime => 39,
        }
    }

    // The same target, with an LFO's number brought within LFO_COUNT and
    // an oscillator's within OSCILLATOR_COUNT.
    pub fn within_range(self) -> Target {
        let last_oscillator = OSCILLATOR_COUNT - 1;
        match self {
            Target::LfoSpeed(index) => Target::LfoSpeed(index.min(LFO_COUNT - 1)),
            Target::OscillatorLevel(index) => Target::OscillatorLevel(index.min(last_oscillator)),
            Target::OscillatorPitch(index) => Target::OscillatorPitch(index.min(last_oscillator)),
            Target::OscillatorWidth(index) => Target::OscillatorWidth(index.min(last_oscillator)),
            Target::OscillatorKind(index) => Target::OscillatorKind(index.min(last_oscillator)),
            other => other,
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Target::Pitch => Unit::Cents,
            Target::OscillatorPitch(_) => Unit::Cents,
            Target::Detune => Unit::Cents,
            Target::Cutoff => Unit::Octaves,
            Target::FilterAmount => Unit::Octaves,
            Target::LfoSpeed(_) => Unit::Octaves,
            Target::LoudnessAttack => Unit::Octaves,
            Target::LoudnessDecay => Unit::Octaves,
            Target::LoudnessRelease => Unit::Octaves,
            Target::FilterAttack => Unit::Octaves,
            Target::FilterDecay => Unit::Octaves,
            Target::FilterRelease => Unit::Octaves,
            Target::GlideTime => Unit::Octaves,
            Target::OscillatorKind(_) => Unit::Steps,
            Target::FilterType => Unit::Steps,
            Target::Copies => Unit::Copies,
            _ => Unit::Percent,
        }
    }

    // The most a connection can move it either way, in its unit.
    pub fn most_amount(self) -> f32 {
        match self {
            Target::Pitch => 4800.0,
            Target::OscillatorPitch(_) => 4800.0,
            Target::Detune => 50.0,
            Target::Cutoff => 10.0,
            Target::FilterAmount => 8.0,
            // Round through all five kinds, or all four types.
            Target::OscillatorKind(_) => 4.0,
            Target::FilterType => 3.0,
            // From the narrowest pulse, 5% of the cycle, to the widest, 95%.
            Target::OscillatorWidth(_) => 90.0,
            Target::Copies => 6.0,
            _ => {
                // An LFO's speed or an envelope's or a glide's time, all in
                // octaves, run over about 13 octaves. Everything else is in
                // percent.
                if self.unit() == Unit::Octaves {
                    return 14.0;
                }
                100.0
            }
        }
    }
}

// One connection of the routing list: `source` moves `target` by `amount`,
// in the target's unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Connection {
    pub source: Source,
    pub target: Target,
    // From minus to plus the target's most amount.
    pub amount: f32,
    // Both ways, the target moves from -amount to amount; one way, from 0
    // to amount.
    pub one_way: bool,
    // For the pitch targets only, the note's and each oscillator's: the
    // move is rounded to whole half steps.
    pub snap: bool,
    // Another source the amount is multiplied by, read one way, from 0 (no
    // move) to 100% (the full amount). None leaves it full.
    pub scaled_by: Source,
}

// A connection with no source, which moves nothing. Every connection of a
// new engine is one.
pub const NO_CONNECTION: Connection = Connection {
    source: Source::None,
    target: Target::Pitch,
    amount: 0.0,
    one_way: false,
    snap: false,
    scaled_by: Source::None,
};

// How long the vibrato key takes to rise from up to held, and to fall back,
// in ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VibratoKeyTimes {
    pub rise_ms: u32,
    pub fall_ms: u32,
}

// Where a new engine's vibrato key times start: at once. Only the tests use
// it; a new song sets its own.
#[cfg(test)]
pub const START_VIBRATO_KEY_TIMES: VibratoKeyTimes = VibratoKeyTimes {
    rise_ms: 0,
    fall_ms: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    // Every source, the LFOs included, in the C API's order.
    fn every_source() -> Vec<Source> {
        let mut sources = vec![Source::None];
        for index in 0..LFO_COUNT {
            sources.push(Source::Lfo(index));
        }
        for source in &Source::OTHERS[1..] {
            sources.push(*source);
        }
        sources
    }

    // Every target, the LFOs' speeds and the oscillators' targets included,
    // in the C API's order.
    fn every_target() -> Vec<Target> {
        let mut targets = Vec::new();
        for target in Target::OTHERS {
            if target == Target::Copies {
                for index in 0..LFO_COUNT {
                    targets.push(Target::LfoSpeed(index));
                }
                for index in 0..OSCILLATOR_COUNT {
                    targets.push(Target::OscillatorLevel(index));
                }
                for index in 0..OSCILLATOR_COUNT {
                    targets.push(Target::OscillatorPitch(index));
                }
                for index in 0..OSCILLATOR_COUNT {
                    targets.push(Target::OscillatorWidth(index));
                }
                for index in 0..OSCILLATOR_COUNT {
                    targets.push(Target::OscillatorKind(index));
                }
            }
            targets.push(target);
        }
        targets
    }

    #[test]
    fn every_name_finds_its_own_shape_speed_source_and_target() {
        for shape in LfoShape::ALL {
            assert_eq!(LfoShape::from_name(shape.name()), Some(shape));
        }
        for speed in LfoSpeed::ALL {
            assert_eq!(LfoSpeed::from_name(speed.name()), Some(speed));
        }
        for source in every_source() {
            assert_eq!(Source::from_name(&source.name()), Some(source));
        }
        for target in every_target() {
            assert_eq!(Target::from_name(&target.name()), Some(target));
        }
        assert_eq!(Source::from_name("lfo 9"), None);
        assert_eq!(Target::from_name("lfo 0 speed"), None);
        assert_eq!(Target::from_name("osc 4 level"), None);
        assert_eq!(Target::from_name("wave"), None);
    }

    #[test]
    fn the_c_api_numbers_count_up_from_0_in_order() {
        // As engine/include/jello_engine.h numbers them.
        for (number, shape) in LfoShape::ALL.iter().enumerate() {
            assert_eq!(shape.number(), number as i32);
        }
        for (number, speed) in LfoSpeed::ALL.iter().enumerate() {
            assert_eq!(speed.number(), number as i32);
        }
        let sources = every_source();
        assert_eq!(sources.len(), 14);
        for (number, source) in sources.iter().enumerate() {
            assert_eq!(source.number(), number as i32, "{}", source.name());
        }
        let targets = every_target();
        assert_eq!(targets.len(), 40);
        for (number, target) in targets.iter().enumerate() {
            assert_eq!(target.number(), number as i32, "{}", target.name());
        }
    }

    #[test]
    fn an_lfo_past_the_8th_is_the_8th() {
        assert_eq!(Source::Lfo(8).within_range(), Source::Lfo(7));
        assert_eq!(Source::Lfo(3).within_range(), Source::Lfo(3));
        assert_eq!(Source::Random.within_range(), Source::Random);
        assert_eq!(Target::LfoSpeed(20).within_range(), Target::LfoSpeed(7));
        assert_eq!(Target::OscillatorKind(5).within_range(), Target::OscillatorKind(2));
        assert_eq!(Target::OscillatorWidth(3).within_range(), Target::OscillatorWidth(2));
        assert_eq!(Target::OscillatorLevel(1).within_range(), Target::OscillatorLevel(1));
        assert_eq!(Target::Pan.within_range(), Target::Pan);
    }
}
