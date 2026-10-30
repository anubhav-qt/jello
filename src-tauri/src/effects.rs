// What each effect's values are (docs/design.md#mix-in-the-engine): their
// numbers in the C API, the words the song file and the agent call them,
// their ranges and where a new effect starts. The engine keeps the same
// ranges and starting values in each effect's own file (engine/src/eq.h and
// on); these follow them.
//
// The engine takes shares from 0 to 1. Jello writes them in percent, as the
// sound profile does, so a mix of 0.3 is written 30. Every choice is a word,
// like "bell" or "tape", and every switch is true or false.

use crate::engine::{
    BUS_COUNT, CHORUS_COPIES, CHORUS_CYCLE, CHORUS_DELAY, CHORUS_DEPTH, CHORUS_FEEDBACK, CHORUS_MIX,
    CHORUS_ON_BEAT, CHORUS_SPEED, CHORUS_SPREAD, COMPRESSOR_ATTACK, COMPRESSOR_KNEE, COMPRESSOR_LIMIT,
    COMPRESSOR_LISTENS_TO, COMPRESSOR_LOW_CUT, COMPRESSOR_MAKEUP, COMPRESSOR_MIX, COMPRESSOR_RATIO,
    COMPRESSOR_RELEASE, COMPRESSOR_THRESHOLD, DELAY_DUCK, DELAY_FEEDBACK, DELAY_GRIT, DELAY_HIGH_CUT,
    DELAY_LEFT_BEATS, DELAY_LEFT_MS, DELAY_LINKED, DELAY_LOW_CUT, DELAY_MIX, DELAY_ON_BEAT,
    DELAY_PING_PONG, DELAY_RIGHT_BEATS, DELAY_RIGHT_MS, DELAY_WOBBLE, DELAY_WOBBLE_SPEED,
    EFFECT_START_MIX, EQ_BAND_FREQUENCY, EQ_BAND_GAIN, EQ_BAND_ON, EQ_BAND_SHAPE, EQ_BAND_SLOPE,
    EQ_BAND_VALUES, EQ_BAND_WIDTH, EQ_BELL, EQ_OUTPUT, EffectKind, FILTER_EFFECT_CUTOFF,
    FILTER_EFFECT_DRIVE, FILTER_EFFECT_FOLLOW, FILTER_EFFECT_FOLLOW_ATTACK,
    FILTER_EFFECT_FOLLOW_RELEASE, FILTER_EFFECT_LISTENS_TO, FILTER_EFFECT_MIX,
    FILTER_EFFECT_RESONANCE, FILTER_EFFECT_SLOPE, FILTER_EFFECT_TYPE, FIRST_BUS_CHAIN, GATE_ATTACK,
    GATE_HOLD, GATE_LISTENS_TO, GATE_RANGE, GATE_RELEASE, GATE_SILENT_RANGE_DB, GATE_THRESHOLD,
    LISTEN_TO_OWN, REVERB_DAMPING, REVERB_DECAY, REVERB_DIFFUSION, REVERB_FREEZE, REVERB_LOW_CUT,
    REVERB_MIX, REVERB_MODULATION, REVERB_PRE_DELAY, REVERB_SIZE, REVERB_WIDTH, SATURATION_CURVE,
    SATURATION_DRIVE, SATURATION_MATCH, SATURATION_MIX, SATURATION_OUTPUT, SATURATION_TAPE,
    SATURATION_TONE, SHAPER_CYCLE, SHAPER_DEPTH, SHAPER_POINT_BEND, SHAPER_POINT_LEVEL,
    SHAPER_POINT_PLACE, SHAPER_POINT_VALUES, SHAPER_POINTS, TRACK_COUNT,
};

// How many bands an EQ has, and the most points a volume shaper has
// (`JELLO_EQ_BAND_COUNT`, `JELLO_SHAPER_MOST_POINTS`).
pub const EQ_BAND_COUNT: usize = 8;
pub const SHAPER_MOST_POINTS: usize = 16;

// Where each EQ band starts, in Hz, from the lowest band up (engine/src/eq.cpp).
const EQ_START_HZ: [f32; EQ_BAND_COUNT] = [40.0, 100.0, 250.0, 600.0, 1500.0, 3500.0, 8000.0, 16000.0];

// The words for the kinds of effect, in the order of `EffectKind`, none left
// out.
const KIND_WORDS: [(EffectKind, &str); 9] = [
    (EffectKind::Eq, "eq"),
    (EffectKind::Filter, "filter"),
    (EffectKind::Saturation, "saturation"),
    (EffectKind::Compressor, "compressor"),
    (EffectKind::Gate, "gate"),
    (EffectKind::VolumeShaper, "volume-shaper"),
    (EffectKind::Chorus, "chorus"),
    (EffectKind::Delay, "delay"),
    (EffectKind::Reverb, "reverb"),
];

// The words for an EQ band's shapes, a filter's types and saturation's
// curves, each numbered from 0 as the engine numbers them.
const EQ_SHAPES: &[&str] = &["low-cut", "low-shelf", "bell", "high-shelf", "high-cut", "notch"];
const FILTER_TYPES: &[&str] = &["low-pass", "high-pass", "band-pass", "notch"];
const SATURATION_CURVES: &[&str] = &["tape", "tube", "hard", "fold"];

// The slopes a cut can have, in dB an octave.
const EQ_SLOPES: &[f32] = &[12.0, 24.0, 48.0];
const FILTER_SLOPES: &[f32] = &[12.0, 24.0];

// The word for "listens to" its own input.
pub const OWN_WORD: &str = "own";

// The word for a compressor's ratio at its limit, where nothing passes the
// threshold.
pub const LIMIT_WORD: &str = "limit";

// What sort of number a value takes, and how it is written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sort {
    // Any number in its range, written as the engine takes it.
    Number,
    // A share, written in percent: the engine's 0 to 1 is 0 to 100.
    Percent,
    // A whole number in its range.
    Whole,
    // On or off, written true or false. The engine takes 1 or 0.
    Switch,
    // One of these words, which the engine takes numbered from 0.
    Choice(&'static [&'static str]),
    // Only one of these numbers, like a cut's slope.
    Steps(&'static [f32]),
    // A compressor's ratio: a number in its range, or the word "limit".
    Ratio,
    // "Listens to": its own input, a track or a bus.
    ListensTo,
}

// One value of an effect.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectValue {
    // Its number in the C API (`jello_engine_set_effect_value`).
    pub number: i32,
    // What the song file and the agent call it, like "cutoff_hz".
    pub word: String,
    pub sort: Sort,
    // Its range and where a new effect starts, as the engine takes them.
    pub lowest: f32,
    pub highest: f32,
    pub start: f32,
}

// A value as it is written: a number, a switch or a word.
#[derive(Clone, Debug, PartialEq)]
pub enum Written {
    Number(f64),
    Switch(bool),
    Word(String),
}

// The word for effect kind `kind`. None has none.
pub fn word_of_kind(kind: EffectKind) -> Option<&'static str> {
    for (each, word) in KIND_WORDS {
        if each == kind {
            return Some(word);
        }
    }
    None
}

// The kind of effect called `word`, or None.
pub fn kind_of_word(word: &str) -> Option<EffectKind> {
    for (kind, each) in KIND_WORDS {
        if each == word {
            return Some(kind);
        }
    }
    None
}

// Every kind's word, in order, to say what there is.
pub fn kind_words() -> Vec<&'static str> {
    let mut words = Vec::new();
    for (_, word) in KIND_WORDS {
        words.push(word);
    }
    words
}

// Every value of an effect of kind `kind`, in order. A chorus, delay or
// reverb starts fully wet on a bus (`on_bus`) and at EFFECT_START_MIX
// elsewhere. None has no values.
pub fn values_of(kind: EffectKind, on_bus: bool) -> Vec<EffectValue> {
    let mut start_mix = EFFECT_START_MIX;
    if on_bus {
        start_mix = 1.0;
    }
    match kind {
        EffectKind::None => Vec::new(),
        EffectKind::Eq => eq_values(),
        EffectKind::Filter => filter_values(),
        EffectKind::Saturation => saturation_values(),
        EffectKind::Compressor => compressor_values(),
        EffectKind::Gate => gate_values(),
        EffectKind::VolumeShaper => shaper_values(),
        EffectKind::Chorus => chorus_values(start_mix),
        EffectKind::Delay => delay_values(start_mix),
        EffectKind::Reverb => reverb_values(start_mix),
    }
}

// A value with its range and start.
fn value(number: i32, word: &str, sort: Sort, lowest: f32, highest: f32, start: f32) -> EffectValue {
    EffectValue {
        number,
        word: String::from(word),
        sort,
        lowest,
        highest,
        start,
    }
}

// A share from 0 to 1, written in percent.
fn percent(number: i32, word: &str, start: f32) -> EffectValue {
    value(number, word, Sort::Percent, 0.0, 1.0, start)
}

// A switch.
fn switch(number: i32, word: &str, start: bool) -> EffectValue {
    let mut start_number = 0.0;
    if start {
        start_number = 1.0;
    }
    value(number, word, Sort::Switch, 0.0, 1.0, start_number)
}

// One of `words`, starting at the one numbered `start`.
fn choice(number: i32, word: &str, words: &'static [&'static str], start: f32) -> EffectValue {
    let highest = (words.len() - 1) as f32;
    value(number, word, Sort::Choice(words), 0.0, highest, start)
}

// One of `steps`.
fn steps(number: i32, word: &str, steps: &'static [f32], start: f32) -> EffectValue {
    let lowest = steps[0];
    let highest = steps[steps.len() - 1];
    value(number, word, Sort::Steps(steps), lowest, highest, start)
}

// "Listens to", starting at its own input.
fn listens_to(number: i32) -> EffectValue {
    value(number, "listens_to", Sort::ListensTo, LISTEN_TO_OWN, LISTEN_TO_OWN, LISTEN_TO_OWN)
}

fn eq_values() -> Vec<EffectValue> {
    let mut values = Vec::new();
    for band in 0..EQ_BAND_COUNT {
        let first = band as i32 * EQ_BAND_VALUES;
        let name = format!("band_{}_", band + 1);
        values.push(switch(first + EQ_BAND_ON, &format!("{name}on"), false));
        values.push(choice(first + EQ_BAND_SHAPE, &format!("{name}shape"), EQ_SHAPES, EQ_BELL));
        let frequency = format!("{name}frequency_hz");
        values.push(value(first + EQ_BAND_FREQUENCY, &frequency, Sort::Number, 20.0, 20000.0, EQ_START_HZ[band]));
        values.push(value(first + EQ_BAND_GAIN, &format!("{name}gain_db"), Sort::Number, -24.0, 24.0, 0.0));
        values.push(value(first + EQ_BAND_WIDTH, &format!("{name}width"), Sort::Number, 0.1, 18.0, 1.0));
        values.push(steps(first + EQ_BAND_SLOPE, &format!("{name}slope"), EQ_SLOPES, 24.0));
    }
    values.push(value(EQ_OUTPUT, "output_db", Sort::Number, -24.0, 24.0, 0.0));
    values
}

fn filter_values() -> Vec<EffectValue> {
    vec![
        choice(FILTER_EFFECT_TYPE, "type", FILTER_TYPES, 0.0),
        steps(FILTER_EFFECT_SLOPE, "slope", FILTER_SLOPES, 12.0),
        value(FILTER_EFFECT_CUTOFF, "cutoff_hz", Sort::Number, 20.0, 20000.0, 20000.0),
        percent(FILTER_EFFECT_RESONANCE, "resonance", 0.0),
        percent(FILTER_EFFECT_DRIVE, "drive", 0.0),
        value(FILTER_EFFECT_FOLLOW, "follow_octaves", Sort::Number, -8.0, 8.0, 0.0),
        value(FILTER_EFFECT_FOLLOW_ATTACK, "follow_attack_ms", Sort::Number, 0.1, 100.0, 10.0),
        value(FILTER_EFFECT_FOLLOW_RELEASE, "follow_release_ms", Sort::Number, 5.0, 2000.0, 200.0),
        percent(FILTER_EFFECT_MIX, "mix", 1.0),
        listens_to(FILTER_EFFECT_LISTENS_TO),
    ]
}

fn saturation_values() -> Vec<EffectValue> {
    vec![
        value(SATURATION_DRIVE, "drive_db", Sort::Number, 0.0, 36.0, 0.0),
        choice(SATURATION_CURVE, "curve", SATURATION_CURVES, SATURATION_TAPE),
        value(SATURATION_TONE, "tone", Sort::Number, -1.0, 1.0, 0.0),
        value(SATURATION_OUTPUT, "output_db", Sort::Number, -24.0, 24.0, 0.0),
        switch(SATURATION_MATCH, "match", false),
        percent(SATURATION_MIX, "mix", 1.0),
    ]
}

fn compressor_values() -> Vec<EffectValue> {
    vec![
        value(COMPRESSOR_THRESHOLD, "threshold_db", Sort::Number, -60.0, 0.0, 0.0),
        value(COMPRESSOR_RATIO, "ratio", Sort::Ratio, 1.0, 20.0, 4.0),
        value(COMPRESSOR_ATTACK, "attack_ms", Sort::Number, 0.1, 100.0, 10.0),
        value(COMPRESSOR_RELEASE, "release_ms", Sort::Number, 5.0, 2000.0, 100.0),
        value(COMPRESSOR_KNEE, "knee_db", Sort::Number, 0.0, 24.0, 0.0),
        value(COMPRESSOR_MAKEUP, "makeup_db", Sort::Number, 0.0, 24.0, 0.0),
        percent(COMPRESSOR_MIX, "mix", 1.0),
        listens_to(COMPRESSOR_LISTENS_TO),
        value(COMPRESSOR_LOW_CUT, "low_cut_hz", Sort::Number, 20.0, 500.0, 20.0),
    ]
}

fn gate_values() -> Vec<EffectValue> {
    let silent = GATE_SILENT_RANGE_DB;
    vec![
        value(GATE_THRESHOLD, "threshold_db", Sort::Number, -80.0, 0.0, -80.0),
        value(GATE_RANGE, "range_db", Sort::Number, silent, 0.0, silent),
        value(GATE_ATTACK, "attack_ms", Sort::Number, 0.1, 50.0, 1.0),
        value(GATE_HOLD, "hold_ms", Sort::Number, 0.0, 500.0, 20.0),
        value(GATE_RELEASE, "release_ms", Sort::Number, 5.0, 2000.0, 100.0),
        listens_to(GATE_LISTENS_TO),
    ]
}

// A new volume shaper is the classic pump: down at the start of each beat,
// back up by half way. Points past the third sit at the end, full.
fn shaper_values() -> Vec<EffectValue> {
    let mut values = Vec::new();
    for point in 0..SHAPER_MOST_POINTS {
        let first = point as i32 * SHAPER_POINT_VALUES;
        let name = format!("point_{}_", point + 1);
        let mut place = 1.0;
        let mut level = 1.0;
        let mut bend = 0.0;
        if point == 0 {
            place = 0.0;
            level = 0.0;
            bend = 0.5;
        } else if point == 1 {
            place = 0.5;
        }
        values.push(percent(first + SHAPER_POINT_PLACE, &format!("{name}place"), place));
        values.push(percent(first + SHAPER_POINT_LEVEL, &format!("{name}level"), level));
        let bend_word = format!("{name}bend");
        values.push(value(first + SHAPER_POINT_BEND, &bend_word, Sort::Number, -1.0, 1.0, bend));
    }
    values.push(value(SHAPER_CYCLE, "cycle_beats", Sort::Number, 1.0 / 16.0, 16.0, 1.0));
    values.push(percent(SHAPER_DEPTH, "depth", 0.0));
    let most_points = SHAPER_MOST_POINTS as f32;
    values.push(value(SHAPER_POINTS, "points", Sort::Whole, 1.0, most_points, 3.0));
    values
}

fn chorus_values(start_mix: f32) -> Vec<EffectValue> {
    vec![
        value(CHORUS_COPIES, "copies", Sort::Whole, 1.0, 4.0, 2.0),
        value(CHORUS_SPEED, "speed_hz", Sort::Number, 0.01, 10.0, 0.8),
        value(CHORUS_CYCLE, "cycle_beats", Sort::Number, 1.0 / 16.0, 32.0, 1.0),
        switch(CHORUS_ON_BEAT, "on_beat", false),
        percent(CHORUS_DEPTH, "depth", 0.25),
        value(CHORUS_DELAY, "delay_ms", Sort::Number, 1.0, 40.0, 7.0),
        value(CHORUS_FEEDBACK, "feedback", Sort::Percent, 0.0, 0.95, 0.0),
        percent(CHORUS_SPREAD, "spread", 1.0),
        percent(CHORUS_MIX, "mix", start_mix),
    ]
}

fn delay_values(start_mix: f32) -> Vec<EffectValue> {
    vec![
        value(DELAY_LEFT_MS, "left_ms", Sort::Number, 1.0, 8000.0, 250.0),
        value(DELAY_RIGHT_MS, "right_ms", Sort::Number, 1.0, 8000.0, 250.0),
        value(DELAY_LEFT_BEATS, "left_beats", Sort::Number, 1.0 / 16.0, 16.0, 0.5),
        value(DELAY_RIGHT_BEATS, "right_beats", Sort::Number, 1.0 / 16.0, 16.0, 0.5),
        switch(DELAY_ON_BEAT, "on_beat", true),
        switch(DELAY_LINKED, "linked", true),
        switch(DELAY_PING_PONG, "ping_pong", false),
        percent(DELAY_FEEDBACK, "feedback", 0.35),
        value(DELAY_LOW_CUT, "low_cut_hz", Sort::Number, 20.0, 20000.0, 20.0),
        value(DELAY_HIGH_CUT, "high_cut_hz", Sort::Number, 20.0, 20000.0, 20000.0),
        percent(DELAY_WOBBLE, "wobble", 0.0),
        value(DELAY_WOBBLE_SPEED, "wobble_hz", Sort::Number, 0.1, 10.0, 0.5),
        percent(DELAY_GRIT, "grit", 0.0),
        percent(DELAY_DUCK, "duck", 0.0),
        percent(DELAY_MIX, "mix", start_mix),
    ]
}

fn reverb_values(start_mix: f32) -> Vec<EffectValue> {
    vec![
        percent(REVERB_SIZE, "size", 0.5),
        value(REVERB_DECAY, "decay_s", Sort::Number, 0.1, 30.0, 1.5),
        value(REVERB_PRE_DELAY, "pre_delay_ms", Sort::Number, 0.0, 500.0, 0.0),
        value(REVERB_DAMPING, "damping_hz", Sort::Number, 1000.0, 20000.0, 8000.0),
        value(REVERB_LOW_CUT, "low_cut_hz", Sort::Number, 20.0, 1000.0, 20.0),
        percent(REVERB_DIFFUSION, "diffusion", 0.7),
        percent(REVERB_MODULATION, "modulation", 0.3),
        percent(REVERB_WIDTH, "width", 1.0),
        switch(REVERB_FREEZE, "freeze", false),
        percent(REVERB_MIX, "mix", start_mix),
    ]
}

// `number`, in the engine's units, kept within `value`'s range: a number
// past either end counts as that end, a whole number or a choice is
// rounded, and a step is the nearest one. "Listens to" and a compressor at
// its limit are kept as they are; the mix checks them. Not a number at all
// counts as the start.
pub fn kept_within(value: &EffectValue, number: f32) -> f32 {
    if number.is_nan() {
        return value.start;
    }
    if value.sort == Sort::ListensTo {
        return number;
    }
    if value.sort == Sort::Ratio {
        if number == COMPRESSOR_LIMIT {
            return number;
        }
    }
    if let Sort::Steps(steps) = value.sort {
        return nearest_step(steps, number);
    }
    let kept = number.clamp(value.lowest, value.highest);
    let rounds = matches!(value.sort, Sort::Whole | Sort::Switch | Sort::Choice(_));
    if rounds {
        return kept.round();
    }
    kept
}

// The step in `steps` nearest `number`, the lower one when halfway.
fn nearest_step(steps: &[f32], number: f32) -> f32 {
    let mut nearest = steps[0];
    for step in steps {
        if (step - number).abs() < (nearest - number).abs() {
            nearest = *step;
        }
    }
    nearest
}

// `number`, in the engine's units, as `value` is written.
pub fn written_of(value: &EffectValue, number: f32) -> Written {
    match value.sort {
        Sort::Number | Sort::Whole | Sort::Steps(_) => Written::Number(tidy(number)),
        Sort::Percent => Written::Number(tidy_percent(number)),
        Sort::Switch => Written::Switch(number == 1.0),
        Sort::Choice(words) => Written::Word(String::from(words[number as usize])),
        Sort::Ratio => written_ratio(number),
        Sort::ListensTo => Written::Word(listens_to_word(number)),
    }
}

fn written_ratio(number: f32) -> Written {
    if number == COMPRESSOR_LIMIT {
        return Written::Word(String::from(LIMIT_WORD));
    }
    Written::Number(tidy(number))
}

// `number` as a f64 without the float's noise: 0.3 stays 0.3, not
// 0.30000001192092896. Four decimal places is finer than any slider and
// keeps a 1/16 beat whole.
fn tidy(number: f32) -> f64 {
    let text = format!("{number:.4}");
    text.parse().unwrap_or(f64::from(number))
}

// A share from 0 to 1 in percent, without the float's noise.
fn tidy_percent(share: f32) -> f64 {
    let percent = f64::from(share) * 100.0;
    let text = format!("{percent:.4}");
    text.parse().unwrap_or(percent)
}

// `written` as `value` takes it, in the engine's units, kept within its
// range. An error says what `value` takes when `written` is the wrong
// sort or an unknown word.
pub fn number_of(value: &EffectValue, written: &Written) -> Result<f32, String> {
    let wrong = || format!("{} takes {}", value.word, what_it_takes(value));
    match (value.sort, written) {
        (Sort::Switch, Written::Switch(on)) => {
            if *on {
                return Ok(1.0);
            }
            Ok(0.0)
        }
        (Sort::Choice(words), Written::Word(word)) => {
            let Some(place) = words.iter().position(|each| each == word) else {
                return Err(wrong());
            };
            Ok(place as f32)
        }
        (Sort::Ratio, Written::Word(word)) => {
            if word == LIMIT_WORD {
                return Ok(COMPRESSOR_LIMIT);
            }
            Err(wrong())
        }
        (Sort::ListensTo, Written::Word(word)) => {
            let Some(chain) = chain_of_listens_to(word) else {
                return Err(wrong());
            };
            Ok(chain)
        }
        (Sort::Percent, Written::Number(number)) => Ok(kept_within(value, (*number / 100.0) as f32)),
        (Sort::Ratio, Written::Number(number)) => {
            // Only the word is the limit: a number is a ratio, even 0.
            let ratio = *number as f32;
            if ratio.is_nan() {
                return Ok(value.start);
            }
            Ok(ratio.clamp(value.lowest, value.highest))
        }
        (Sort::Number | Sort::Whole | Sort::Steps(_), Written::Number(number)) => {
            Ok(kept_within(value, *number as f32))
        }
        _ => Err(wrong()),
    }
}

// Whether `written` is a number within `value`'s range, or for a
// whole number or a step, exactly one. Words and switches are checked by
// number_of instead, so they count as in range here.
pub fn is_in_range(value: &EffectValue, written: &Written) -> bool {
    let Written::Number(number) = written else {
        return true;
    };
    let mut number = *number;
    if value.sort == Sort::Percent {
        number /= 100.0;
    }
    if number.is_nan() {
        return false;
    }
    let number = number as f32;
    if let Sort::Steps(steps) = value.sort {
        return steps.contains(&number);
    }
    let is_number = matches!(value.sort, Sort::Number | Sort::Percent | Sort::Ratio | Sort::Whole);
    if !is_number {
        return true;
    }
    if value.sort == Sort::Whole {
        if number.fract() != 0.0 {
            return false;
        }
    }
    if number < value.lowest {
        return false;
    }
    number <= value.highest
}

// What `value` takes, in words, for an error.
pub fn what_it_takes(value: &EffectValue) -> String {
    match value.sort {
        Sort::Number | Sort::Whole => format!("a number from {} to {}", tidy(value.lowest), tidy(value.highest)),
        Sort::Percent => format!("a percent from 0 to {}", tidy_percent(value.highest)),
        Sort::Switch => String::from("true or false"),
        Sort::Choice(words) => format!("one of {}", words.join(", ")),
        Sort::Steps(steps) => {
            let mut numbers = Vec::new();
            for step in steps {
                numbers.push(tidy(*step).to_string());
            }
            format!("one of {}", numbers.join(", "))
        }
        Sort::Ratio => format!("a number from 1 to {}, or {LIMIT_WORD}", tidy(value.highest)),
        Sort::ListensTo => format!("{OWN_WORD}, a track like \"track 2\" or a bus like \"bus 1\""),
    }
}

// The word for "listens to" chain `chain`: "own", "track 2" or "bus 1",
// counted from 1.
pub fn listens_to_word(chain: f32) -> String {
    if chain == LISTEN_TO_OWN {
        return String::from(OWN_WORD);
    }
    let chain = chain as usize;
    if chain >= FIRST_BUS_CHAIN {
        return format!("bus {}", chain - FIRST_BUS_CHAIN + 1);
    }
    format!("track {}", chain + 1)
}

// The chain "listens to" `word` names, as the engine numbers it, or None
// for a word that names none. Whether the song has it is the mix's to say.
pub fn chain_of_listens_to(word: &str) -> Option<f32> {
    if word == OWN_WORD {
        return Some(LISTEN_TO_OWN);
    }
    if let Some(number) = word.strip_prefix("track ") {
        let number: usize = number.trim().parse().ok()?;
        if number == 0 || number > TRACK_COUNT {
            return None;
        }
        return Some((number - 1) as f32);
    }
    if let Some(number) = word.strip_prefix("bus ") {
        let number: usize = number.trim().parse().ok()?;
        if number == 0 || number > BUS_COUNT {
            return None;
        }
        return Some((FIRST_BUS_CHAIN + number - 1) as f32);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EFFECT_MOST_VALUES, EQ_HIGH_CUT};

    const ALL_KINDS: [EffectKind; 9] = [
        EffectKind::Eq,
        EffectKind::Filter,
        EffectKind::Saturation,
        EffectKind::Compressor,
        EffectKind::Gate,
        EffectKind::VolumeShaper,
        EffectKind::Chorus,
        EffectKind::Delay,
        EffectKind::Reverb,
    ];

    #[test]
    fn every_kind_has_a_word_that_names_it_back() {
        for kind in ALL_KINDS {
            let word = word_of_kind(kind).unwrap();
            assert_eq!(kind_of_word(word), Some(kind));
        }
        assert_eq!(word_of_kind(EffectKind::None), None);
        assert_eq!(kind_of_word("banjo"), None);
        assert_eq!(kind_words().len(), ALL_KINDS.len());
    }

    #[test]
    fn every_value_has_its_own_word_and_number_and_starts_in_its_range() {
        for kind in ALL_KINDS {
            let values = values_of(kind, false);
            assert!(!values.is_empty());
            for (place, each) in values.iter().enumerate() {
                assert!(each.number >= 0 && (each.number as usize) < EFFECT_MOST_VALUES, "{}", each.word);
                assert_eq!(kept_within(each, each.start), each.start, "{} starts in range", each.word);
                for other in &values[place + 1..] {
                    assert_ne!(each.word, other.word);
                    assert_ne!(each.number, other.number);
                }
            }
        }
        assert!(values_of(EffectKind::None, false).is_empty());
    }

    #[test]
    fn echoing_effects_start_fully_wet_only_on_a_bus() {
        for kind in [EffectKind::Chorus, EffectKind::Delay, EffectKind::Reverb] {
            let mix = |on_bus| {
                let values = values_of(kind, on_bus);
                values.iter().find(|each| each.word == "mix").unwrap().start
            };
            assert_eq!(mix(false), EFFECT_START_MIX);
            assert_eq!(mix(true), 1.0);
        }
        let eq_mix = values_of(EffectKind::Saturation, true);
        assert_eq!(eq_mix.iter().find(|each| each.word == "mix").unwrap().start, 1.0);
    }

    #[test]
    fn a_new_eq_has_every_band_off_on_a_bell_at_its_own_frequency() {
        let values = values_of(EffectKind::Eq, false);
        assert_eq!(values.len(), EQ_BAND_COUNT * EQ_BAND_VALUES as usize + 1);
        let start = |word: &str| values.iter().find(|each| each.word == word).unwrap().start;
        assert_eq!(start("band_1_on"), 0.0);
        assert_eq!(start("band_1_shape"), EQ_BELL);
        assert_eq!(start("band_1_frequency_hz"), 40.0);
        assert_eq!(start("band_8_frequency_hz"), 16000.0);
        let band_3_frequency = values.iter().find(|each| each.word == "band_3_frequency_hz").unwrap();
        assert_eq!(band_3_frequency.number, 2 * EQ_BAND_VALUES + EQ_BAND_FREQUENCY);
    }

    #[test]
    fn numbers_are_kept_within_their_ranges() {
        let eq = values_of(EffectKind::Eq, false);
        let frequency = eq.iter().find(|each| each.word == "band_1_frequency_hz").unwrap();
        assert_eq!(kept_within(frequency, 5.0), 20.0);
        assert_eq!(kept_within(frequency, 99999.0), 20000.0);
        assert_eq!(kept_within(frequency, f32::NAN), 40.0);
        let slope = eq.iter().find(|each| each.word == "band_1_slope").unwrap();
        assert_eq!(kept_within(slope, 30.0), 24.0);
        assert_eq!(kept_within(slope, 100.0), 48.0);
        let shape = eq.iter().find(|each| each.word == "band_1_shape").unwrap();
        assert_eq!(kept_within(shape, 3.6), EQ_HIGH_CUT);
        let chorus = values_of(EffectKind::Chorus, false);
        let feedback = chorus.iter().find(|each| each.word == "feedback").unwrap();
        assert_eq!(kept_within(feedback, 1.0), 0.95);
    }

    #[test]
    fn values_are_written_in_words_and_percent_and_read_back() {
        let compressor = values_of(EffectKind::Compressor, false);
        let find = |word: &str| compressor.iter().find(|each| each.word == word).unwrap().clone();
        let mix = find("mix");
        assert_eq!(written_of(&mix, 0.3), Written::Number(30.0));
        assert_eq!(number_of(&mix, &Written::Number(30.0)), Ok(0.3));
        assert_eq!(number_of(&mix, &Written::Number(250.0)), Ok(1.0));
        let ratio = find("ratio");
        assert_eq!(written_of(&ratio, COMPRESSOR_LIMIT), Written::Word(String::from("limit")));
        assert_eq!(number_of(&ratio, &Written::Word(String::from("limit"))), Ok(COMPRESSOR_LIMIT));
        assert_eq!(number_of(&ratio, &Written::Number(0.5)), Ok(1.0));
        assert_eq!(number_of(&ratio, &Written::Number(0.0)), Ok(1.0));
        let listens = find("listens_to");
        assert_eq!(written_of(&listens, LISTEN_TO_OWN), Written::Word(String::from("own")));
        assert_eq!(written_of(&listens, 2.0), Written::Word(String::from("track 3")));
        assert_eq!(written_of(&listens, 10.0), Written::Word(String::from("bus 2")));
        assert_eq!(number_of(&listens, &Written::Word(String::from("bus 2"))), Ok(10.0));

        let saturation = values_of(EffectKind::Saturation, false);
        let curve = saturation.iter().find(|each| each.word == "curve").unwrap();
        assert_eq!(written_of(curve, 3.0), Written::Word(String::from("fold")));
        assert_eq!(number_of(curve, &Written::Word(String::from("tube"))), Ok(1.0));
        let matched = saturation.iter().find(|each| each.word == "match").unwrap();
        assert_eq!(written_of(matched, 1.0), Written::Switch(true));
        assert_eq!(number_of(matched, &Written::Switch(false)), Ok(0.0));
    }

    #[test]
    fn the_wrong_sort_or_an_unknown_word_says_what_the_value_takes() {
        let saturation = values_of(EffectKind::Saturation, false);
        let curve = saturation.iter().find(|each| each.word == "curve").unwrap();
        let error = number_of(curve, &Written::Word(String::from("banjo"))).unwrap_err();
        assert_eq!(error, "curve takes one of tape, tube, hard, fold");
        let drive = saturation.iter().find(|each| each.word == "drive_db").unwrap();
        let error = number_of(drive, &Written::Switch(true)).unwrap_err();
        assert_eq!(error, "drive_db takes a number from 0 to 36");
        let compressor = values_of(EffectKind::Compressor, false);
        let listens = compressor.iter().find(|each| each.word == "listens_to").unwrap();
        assert!(number_of(listens, &Written::Word(String::from("track 10"))).is_err());
        assert!(number_of(listens, &Written::Word(String::from("bus 0"))).is_err());
        assert!(number_of(listens, &Written::Word(String::from("master"))).is_err());
    }
}
