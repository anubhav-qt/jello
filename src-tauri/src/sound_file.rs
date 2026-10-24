// A sound written as text (docs/adr/0037-presets.md): what a preset file
// holds, and the sound part of a song file, under [sound]
// (docs/adr/0038-song-files.md). The text is TOML: names and values,
// with a heading for each group. Every choice is a word, like "saw" or
// "low-pass", never a number.
//
// Reading is forgiving, so a file keeps working as the synth grows and
// after it is edited by hand:
// - A value left out, or of the wrong sort (a word for a number), takes a
//   new song's.
// - A word Jello does not know, like kind = "banjo", takes a new song's too.
// - A name Jello does not know is skipped.
// - Values outside their ranges are left for the engine, which brings them
//   within them, as it does for the sound panel.
// Only the connections work differently: the file lists every one the
// sound uses, so a file with none has none. A connection whose source or
// target Jello does not know is left out. Text that is not TOML at all
// can't be read.

use toml::{Table, Value};

use crate::engine::{
    Filter, FilterEnvelope, FilterType, Kind, LoudnessEnvelope, OSCILLATOR_COUNT, Oscillator, Unison,
};
use crate::modulation::{
    CONNECTION_COUNT, Connection, LFO_COUNT, Lfo, LfoShape, LfoSpeed, NO_CONNECTION, Source,
    Target, VibratoKeyTimes,
};
use crate::sound_profile::SoundProfile;

// Every part of a sound profile, as plain values. A sound read from text
// may still have values outside their ranges until
// `SoundProfile::take_sound` sends it through the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub oscillators: [Oscillator; OSCILLATOR_COUNT],
    pub loudness_envelope: LoudnessEnvelope,
    pub filter: Filter,
    pub filter_envelope: FilterEnvelope,
    pub key_tracking: u32,
    pub unison: Unison,
    pub tuning_hz: f32,
    pub lfos: [Lfo; LFO_COUNT],
    pub connections: [Connection; CONNECTION_COUNT],
    pub vibrato_key_times: VibratoKeyTimes,
    pub glide_ms: u32,
    // The instrument each oscillator plays when its kind is the sample, by
    // its folder's name in the Instruments folder (docs/design.md#sampler).
    pub instruments: [Option<String>; OSCILLATOR_COUNT],
}

// The first line of every sound file, for whoever opens one.
const HEADING: &str = "# A Jello sound. Any value left out takes a new song's.";

// `sound` as text. Every value is written, except the connections not in
// use.
pub fn text_of(sound: &Sound) -> String {
    let mut text = String::new();
    text.push_str(HEADING);
    text.push('\n');
    write_sound(&mut text, sound, "");
    text
}

// Writes `sound` onto the end of `text`, every heading under `under`: ""
// for a sound file, or "sound" for a song file's [sound], where its
// oscillators come under [[sound.oscillators]].
pub fn write_sound(text: &mut String, sound: &Sound, under: &str) {
    // Single values come first: TOML wants them before any heading. In a
    // song file they come under [sound] itself.
    if under.is_empty() {
        text.push('\n');
    } else {
        start_group(text, &format!("[{under}]"));
    }
    write_number(text, "tuning_hz", sound.tuning_hz);
    write_number(text, "glide_ms", sound.glide_ms);

    for (oscillator, instrument) in sound.oscillators.iter().zip(&sound.instruments) {
        start_group(text, &heading(under, "[[oscillators]]"));
        write_word(text, "kind", oscillator.kind.name());
        if let Some(instrument) = instrument {
            write_name(text, "instrument", instrument);
        }
        write_number(text, "level", oscillator.level);
        write_number(text, "octaves", oscillator.octaves);
        write_number(text, "half_steps", oscillator.half_steps);
        write_number(text, "cents", oscillator.cents);
        write_number(text, "width", oscillator.width);
    }

    let loudness = sound.loudness_envelope;
    start_group(text, &heading(under, "[loudness]"));
    write_number(text, "attack_ms", loudness.attack_ms);
    write_number(text, "decay_ms", loudness.decay_ms);
    write_number(text, "sustain", loudness.sustain);
    write_number(text, "release_ms", loudness.release_ms);

    let filter = sound.filter;
    start_group(text, &heading(under, "[filter]"));
    write_word(text, "type", filter.filter_type.name());
    write_number(text, "cutoff_hz", filter.cutoff_hz);
    write_number(text, "resonance", filter.resonance);
    write_number(text, "key_tracking", sound.key_tracking);

    let envelope = sound.filter_envelope;
    start_group(text, &heading(under, "[filter_envelope]"));
    write_number(text, "amount", envelope.amount);
    write_number(text, "attack_ms", envelope.attack_ms);
    write_number(text, "decay_ms", envelope.decay_ms);
    write_number(text, "sustain", envelope.sustain);
    write_number(text, "release_ms", envelope.release_ms);

    let unison = sound.unison;
    start_group(text, &heading(under, "[unison]"));
    write_number(text, "copies", unison.copies);
    write_number(text, "detune_cents", unison.detune_cents);
    write_number(text, "spread", unison.spread);

    let key_times = sound.vibrato_key_times;
    start_group(text, &heading(under, "[vibrato_key]"));
    write_number(text, "rise_ms", key_times.rise_ms);
    write_number(text, "fall_ms", key_times.fall_ms);

    for lfo in &sound.lfos {
        write_lfo(text, lfo, under);
    }

    for connection in &sound.connections {
        if connection.source == Source::None {
            continue;
        }
        start_group(text, &heading(under, "[[connections]]"));
        write_word(text, "source", &connection.source.name());
        write_word(text, "target", &connection.target.name());
        write_number(text, "amount", connection.amount);
        write_flag(text, "one_way", connection.one_way);
        write_flag(text, "snap", connection.snap);
        write_word(text, "scaled_by", &connection.scaled_by.name());
    }
}

fn write_lfo(text: &mut String, lfo: &Lfo, under: &str) {
    start_group(text, &heading(under, "[[lfos]]"));
    write_word(text, "shape", lfo.shape.name());
    write_flag(text, "per_note", lfo.per_note);
    write_word(text, "speed", lfo.speed.name());
    write_number(text, "hz", lfo.hz);
    write_number(text, "beats", lfo.beats);
    write_number(text, "presses", lfo.presses);
    write_number(text, "start", lfo.start);
    write_number(text, "spread", lfo.spread);
    write_number(text, "delay_ms", lfo.delay_ms);
    write_number(text, "fade_ms", lfo.fade_ms);
    write_number(text, "delay_beats", lfo.delay_beats);
    write_number(text, "fade_beats", lfo.fade_beats);
    write_number(text, "smooth", lfo.smooth);
    write_flag(text, "once", lfo.once);
    write_number(text, "step_count", lfo.step_count);

    // Every step is kept, even past the step count, so raising the count
    // again brings them back as they were.
    let mut heights = Vec::new();
    for height in &lfo.steps {
        heights.push(height.to_string());
    }
    text.push_str("steps = [");
    text.push_str(&heights.join(", "));
    text.push_str("]\n");
}

// `heading`, like "[[oscillators]]", under `under`: "[[sound.oscillators]]"
// under "sound", and as it is under "".
fn heading(under: &str, heading: &str) -> String {
    if under.is_empty() {
        return String::from(heading);
    }
    let name = heading.trim_matches(|letter| letter == '[' || letter == ']');
    if heading.starts_with("[[") {
        format!("[[{under}.{name}]]")
    } else {
        format!("[{under}.{name}]")
    }
}

// A heading, after an empty line.
pub fn start_group(text: &mut String, heading: &str) {
    text.push('\n');
    text.push_str(heading);
    text.push('\n');
}

// A number, like `level = 100` or `hz = 5.5`. Rust writes the shortest
// digits that read back as exactly the same number.
pub fn write_number(text: &mut String, name: &str, number: impl ToString) {
    text.push_str(name);
    text.push_str(" = ");
    text.push_str(&number.to_string());
    text.push('\n');
}

// A word, like `kind = "saw"`. Words come from Jello's own lists, so none
// has a quote or a backslash that would need escaping.
pub fn write_word(text: &mut String, name: &str, word: &str) {
    text.push_str(name);
    text.push_str(" = \"");
    text.push_str(word);
    text.push_str("\"\n");
}

// A name someone picked, like `instrument = "Grand piano"`, with any quote
// or backslash in it escaped as TOML wants.
pub fn write_name(text: &mut String, name: &str, picked: &str) {
    let quoted = Value::String(String::from(picked)).to_string();
    text.push_str(name);
    text.push_str(" = ");
    text.push_str(&quoted);
    text.push('\n');
}

pub fn write_flag(text: &mut String, name: &str, flag: bool) {
    write_number(text, name, flag);
}

// The sound `text` describes, starting from `new_song`'s and changing what
// the text says. Fails only on text that is not TOML.
pub fn sound_of(text: &str, new_song: &SoundProfile) -> Result<Sound, String> {
    let table = match text.parse::<Table>() {
        Ok(table) => table,
        Err(error) => return Err(format!("it is not a sound file: {}", error.message())),
    };
    Ok(sound_of_table(&table, new_song))
}

// The sound the names and values in `table` describe, starting from
// `new_song`'s, as for sound_of. A song file's [sound] is such a table.
pub fn sound_of_table(table: &Table, new_song: &SoundProfile) -> Sound {
    let mut sound = new_song.sound();
    sound.tuning_hz = read_f32(&table, "tuning_hz", sound.tuning_hz);
    sound.glide_ms = read_count(&table, "glide_ms", sound.glide_ms);

    let oscillators = groups(&table, "oscillators");
    for (index, group) in oscillators.into_iter().enumerate() {
        if index >= OSCILLATOR_COUNT {
            break;
        }
        sound.oscillators[index] = read_oscillator(group, sound.oscillators[index]);
        if let Some(instrument) = read_word(group, "instrument") {
            sound.instruments[index] = Some(String::from(instrument));
        }
    }

    if let Some(group) = group(&table, "loudness") {
        let loudness = &mut sound.loudness_envelope;
        loudness.attack_ms = read_count(group, "attack_ms", loudness.attack_ms);
        loudness.decay_ms = read_count(group, "decay_ms", loudness.decay_ms);
        loudness.sustain = read_count(group, "sustain", loudness.sustain);
        loudness.release_ms = read_count(group, "release_ms", loudness.release_ms);
    }

    if let Some(group) = group(&table, "filter") {
        let filter = &mut sound.filter;
        filter.filter_type = read_choice(group, "type", FilterType::from_name, filter.filter_type);
        filter.cutoff_hz = read_f32(group, "cutoff_hz", filter.cutoff_hz);
        filter.resonance = read_count(group, "resonance", filter.resonance);
        sound.key_tracking = read_count(group, "key_tracking", sound.key_tracking);
    }

    if let Some(group) = group(&table, "filter_envelope") {
        let envelope = &mut sound.filter_envelope;
        envelope.amount = read_f32(group, "amount", envelope.amount);
        envelope.attack_ms = read_count(group, "attack_ms", envelope.attack_ms);
        envelope.decay_ms = read_count(group, "decay_ms", envelope.decay_ms);
        envelope.sustain = read_count(group, "sustain", envelope.sustain);
        envelope.release_ms = read_count(group, "release_ms", envelope.release_ms);
    }

    if let Some(group) = group(&table, "unison") {
        let unison = &mut sound.unison;
        unison.copies = read_count(group, "copies", unison.copies);
        unison.detune_cents = read_count(group, "detune_cents", unison.detune_cents);
        unison.spread = read_count(group, "spread", unison.spread);
    }

    if let Some(group) = group(&table, "vibrato_key") {
        let key_times = &mut sound.vibrato_key_times;
        key_times.rise_ms = read_count(group, "rise_ms", key_times.rise_ms);
        key_times.fall_ms = read_count(group, "fall_ms", key_times.fall_ms);
    }

    let lfos = groups(&table, "lfos");
    for (lfo, group) in sound.lfos.iter_mut().zip(lfos) {
        *lfo = read_lfo(group, *lfo);
    }

    // The connections the file lists fill the routing list from the top, in
    // order. Any past the list's 64 are left out.
    sound.connections = [NO_CONNECTION; CONNECTION_COUNT];
    let mut filled = 0;
    for group in groups(&table, "connections") {
        if filled == CONNECTION_COUNT {
            break;
        }
        if let Some(connection) = read_connection(group) {
            sound.connections[filled] = connection;
            filled += 1;
        }
    }
    sound
}

// The oscillator `group` describes, with `missing`'s values for what it
// leaves out.
fn read_oscillator(group: &Table, missing: Oscillator) -> Oscillator {
    Oscillator {
        kind: read_choice(group, "kind", Kind::from_name, missing.kind),
        level: read_count(group, "level", missing.level),
        octaves: read_signed(group, "octaves", missing.octaves),
        half_steps: read_signed(group, "half_steps", missing.half_steps),
        cents: read_signed(group, "cents", missing.cents),
        width: read_count(group, "width", missing.width),
    }
}

fn read_lfo(group: &Table, missing: Lfo) -> Lfo {
    let mut steps = missing.steps;
    if let Some(Value::Array(heights)) = group.get("steps") {
        for (step, height) in steps.iter_mut().zip(heights) {
            if let Some(number) = number_in(height) {
                *step = number as f32;
            }
        }
    }

    Lfo {
        shape: read_choice(group, "shape", LfoShape::from_name, missing.shape),
        step_count: read_count(group, "step_count", missing.step_count),
        steps,
        per_note: read_flag(group, "per_note", missing.per_note),
        speed: read_choice(group, "speed", LfoSpeed::from_name, missing.speed),
        hz: read_f64(group, "hz", missing.hz),
        beats: read_f64(group, "beats", missing.beats),
        presses: read_count(group, "presses", missing.presses),
        start: read_count(group, "start", missing.start),
        spread: read_count(group, "spread", missing.spread),
        delay_ms: read_count(group, "delay_ms", missing.delay_ms),
        fade_ms: read_count(group, "fade_ms", missing.fade_ms),
        delay_beats: read_f64(group, "delay_beats", missing.delay_beats),
        fade_beats: read_f64(group, "fade_beats", missing.fade_beats),
        smooth: read_count(group, "smooth", missing.smooth),
        once: read_flag(group, "once", missing.once),
    }
}

// The connection `group` describes, or None if Jello does not know its
// source or its target, or its source is "none". What else it leaves out
// is an unused connection's: no move, both ways, no snap, scaled by
// nothing.
fn read_connection(group: &Table) -> Option<Connection> {
    let source = read_choice(group, "source", Source::from_name, Source::None);
    if source == Source::None {
        return None;
    }
    let target_word = read_word(group, "target")?;
    let target = Target::from_name(target_word)?;

    let connection = Connection {
        source,
        target,
        amount: read_f32(group, "amount", NO_CONNECTION.amount),
        one_way: read_flag(group, "one_way", NO_CONNECTION.one_way),
        snap: read_flag(group, "snap", NO_CONNECTION.snap),
        scaled_by: read_choice(group, "scaled_by", Source::from_name, NO_CONNECTION.scaled_by),
    };
    Some(connection)
}

// The group under heading `name`, like [filter], or None.
pub fn group<'a>(table: &'a Table, name: &str) -> Option<&'a Table> {
    match table.get(name) {
        Some(Value::Table(group)) => Some(group),
        _ => None,
    }
}

// The groups under each heading `name`, like [[oscillators]], in order.
pub fn groups<'a>(table: &'a Table, name: &str) -> Vec<&'a Table> {
    let mut found = Vec::new();
    if let Some(Value::Array(items)) = table.get(name) {
        for item in items {
            if let Value::Table(group) = item {
                found.push(group);
            }
        }
    }
    found
}

// A whole number or a fraction as a number, or None for anything else.
pub fn number_in(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(number) => Some(*number as f64),
        Value::Float(number) => Some(*number),
        _ => None,
    }
}

pub fn read_f64(group: &Table, name: &str, missing: f64) -> f64 {
    let Some(value) = group.get(name) else {
        return missing;
    };
    match number_in(value) {
        Some(number) => number,
        None => missing,
    }
}

fn read_f32(group: &Table, name: &str, missing: f32) -> f32 {
    let number = read_f64(group, name, missing as f64);
    number as f32
}

// A count can't go below 0, so a number below 0 reads as 0. A fraction is
// rounded to the nearest whole number. Anything too big reads as the
// biggest count, which the engine brings within range.
pub fn read_count(group: &Table, name: &str, missing: u32) -> u32 {
    let number = read_f64(group, name, missing as f64);
    if number.is_nan() {
        return missing;
    }
    let rounded = number.round();
    if rounded < 0.0 {
        return 0;
    }
    if rounded > u32::MAX as f64 {
        return u32::MAX;
    }
    rounded as u32
}

// Like a count, but it can go below 0.
pub fn read_signed(group: &Table, name: &str, missing: i32) -> i32 {
    let number = read_f64(group, name, missing as f64);
    if number.is_nan() {
        return missing;
    }
    let rounded = number.round();
    if rounded < i32::MIN as f64 {
        return i32::MIN;
    }
    if rounded > i32::MAX as f64 {
        return i32::MAX;
    }
    rounded as i32
}

pub fn read_flag(group: &Table, name: &str, missing: bool) -> bool {
    match group.get(name) {
        Some(Value::Boolean(flag)) => *flag,
        _ => missing,
    }
}

pub fn read_word<'a>(group: &'a Table, name: &str) -> Option<&'a str> {
    match group.get(name) {
        Some(Value::String(word)) => Some(word),
        _ => None,
    }
}

// The choice named by word `name`, like the kind in kind = "saw", found
// with `from_name`. A word left out, or one `from_name` does not know,
// gives `missing`.
fn read_choice<T>(group: &Table, name: &str, from_name: fn(&str) -> Option<T>, missing: T) -> T {
    let Some(word) = read_word(group, name) else {
        return missing;
    };
    match from_name(word) {
        Some(choice) => choice,
        None => missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::{Engine, TUNING_HIGHEST_HZ, UNISON_MOST_COPIES};

    // A new song's sound with every connection empty: what a file with no
    // values and no connections reads as.
    fn new_song_without_connections() -> Sound {
        let mut sound = SoundProfile::new().sound();
        sound.connections = [NO_CONNECTION; CONNECTION_COUNT];
        sound
    }

    // A sound with every value moved away from a new song's.
    fn changed_sound() -> Sound {
        let mut sound = SoundProfile::new().sound();
        sound.tuning_hz = 432.5;
        sound.glide_ms = 340;

        sound.oscillators[0].kind = Kind::Square;
        sound.oscillators[0].width = 20;
        sound.oscillators[1].kind = Kind::Saw;
        sound.oscillators[1].level = 60;
        sound.oscillators[1].octaves = -1;
        sound.oscillators[1].half_steps = 7;
        sound.oscillators[1].cents = -12;
        sound.oscillators[2].kind = Kind::Noise;
        sound.oscillators[2].level = 5;

        sound.loudness_envelope.attack_ms = 40;
        sound.loudness_envelope.decay_ms = 700;
        sound.loudness_envelope.sustain = 55;
        sound.loudness_envelope.release_ms = 1200;

        sound.filter.filter_type = FilterType::BandPass;
        sound.filter.cutoff_hz = 1234.5;
        sound.filter.resonance = 35;
        sound.key_tracking = 40;

        sound.filter_envelope.amount = 2.25;
        sound.filter_envelope.attack_ms = 15;
        sound.filter_envelope.decay_ms = 450;
        sound.filter_envelope.sustain = 30;
        sound.filter_envelope.release_ms = 800;

        sound.unison.copies = 5;
        sound.unison.detune_cents = 18;
        sound.unison.spread = 70;

        sound.vibrato_key_times.rise_ms = 120;
        sound.vibrato_key_times.fall_ms = 90;

        let lfo = &mut sound.lfos[3];
        lfo.shape = LfoShape::ALL[7];
        lfo.step_count = 5;
        lfo.steps[0] = 100.0;
        lfo.steps[1] = -37.5;
        lfo.steps[4] = 12.25;
        lfo.per_note = false;
        lfo.speed = LfoSpeed::Beats;
        lfo.hz = 0.75;
        lfo.beats = 0.5;
        lfo.presses = 3;
        lfo.start = 25;
        lfo.spread = 10;
        lfo.delay_ms = 200;
        lfo.fade_ms = 300;
        lfo.delay_beats = 2.0;
        lfo.fade_beats = 0.25;
        lfo.smooth = 80;
        lfo.once = true;

        sound.connections[1] = Connection {
            source: Source::NoteHeight,
            target: Target::Cutoff,
            amount: 1.5,
            one_way: true,
            snap: false,
            scaled_by: Source::None,
        };
        sound.connections[2] = Connection {
            source: Source::Lfo(3),
            target: Target::OscillatorWidth(0),
            amount: -40.0,
            one_way: false,
            snap: true,
            scaled_by: Source::Random,
        };
        sound
    }

    #[test]
    fn a_sound_reads_back_exactly_as_it_was_written() {
        let sound = changed_sound();
        let text = text_of(&sound);
        let read = sound_of(&text, &SoundProfile::new()).unwrap();
        assert_eq!(read, sound);

        // And a new song's sound too.
        let new_song = SoundProfile::new().sound();
        let read = sound_of(&text_of(&new_song), &SoundProfile::new()).unwrap();
        assert_eq!(read, new_song);
    }

    #[test]
    fn taking_a_sound_makes_it_the_whole_profile() {
        // Every value of the changed sound is within range, so the song
        // keeps each one as it is, and none is left at a new song's.
        let sound = changed_sound();
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        profile.take_sound(&mut engine, &sound);
        assert_eq!(profile.sound(), sound);
    }

    #[test]
    fn choices_are_written_as_words() {
        let text = text_of(&changed_sound());
        assert!(text.starts_with(HEADING), "{text}");
        assert!(text.contains("kind = \"square\""), "{text}");
        assert!(text.contains("type = \"band-pass\""), "{text}");
        assert!(text.contains("source = \"note height\""), "{text}");
        assert!(text.contains("target = \"osc 1 width\""), "{text}");
        assert!(text.contains("scaled_by = \"random\""), "{text}");
    }

    #[test]
    fn only_connections_in_use_are_written() {
        let text = text_of(&changed_sound());
        let count = text.matches("[[connections]]").count();
        assert_eq!(count, 3, "{text}");
    }

    #[test]
    fn missing_values_take_a_new_song_s() {
        let read = sound_of("", &SoundProfile::new()).unwrap();
        assert_eq!(read, new_song_without_connections());

        // A value left out of a group, and a whole oscillator left out.
        let text = "[filter]\ncutoff_hz = 500\n\n[[oscillators]]\nlevel = 30\n";
        let read = sound_of(text, &SoundProfile::new()).unwrap();
        let mut expected = new_song_without_connections();
        expected.filter.cutoff_hz = 500.0;
        expected.oscillators[0].level = 30;
        assert_eq!(read, expected);
    }

    #[test]
    fn values_of_the_wrong_sort_take_a_new_song_s() {
        let text = "tuning_hz = \"high\"\nglide_ms = true\n\n[filter]\ntype = 3\nresonance = [1, 2]\n\n[loudness]\nsustain = -20\n";
        let read = sound_of(text, &SoundProfile::new()).unwrap();
        let mut expected = new_song_without_connections();
        // A count below 0 is a count, read as 0.
        expected.loudness_envelope.sustain = 0;
        assert_eq!(read, expected);
    }

    #[test]
    fn unknown_words_take_a_new_song_s_and_unknown_names_are_skipped() {
        let text = "wobble = 3\n\n[[oscillators]]\nkind = \"banjo\"\nlevel = 70\n\n[filter]\ntype = \"comb\"\n\n[elbow]\nbend = 1\n";
        let read = sound_of(text, &SoundProfile::new()).unwrap();
        let mut expected = new_song_without_connections();
        expected.oscillators[0].level = 70;
        assert_eq!(read, expected);
    }

    #[test]
    fn connections_jello_does_not_know_are_left_out_and_the_rest_move_up() {
        let text = "\
[[connections]]
source = \"lfo 99\"
target = \"pitch\"

[[connections]]
source = \"random\"
target = \"elbow\"

[[connections]]
source = \"none\"
target = \"pitch\"

[[connections]]
target = \"pitch\"

[[connections]]
source = \"random\"
target = \"pan\"
amount = 50
";
        let read = sound_of(text, &SoundProfile::new()).unwrap();
        let mut expected = new_song_without_connections();
        expected.connections[0] = Connection {
            source: Source::Random,
            target: Target::Pan,
            amount: 50.0,
            ..NO_CONNECTION
        };
        assert_eq!(read, expected);
    }

    #[test]
    fn more_than_64_connections_keep_the_first_64() {
        let mut text = String::new();
        for amount in 0..70 {
            text.push_str(&format!("[[connections]]\nsource = \"random\"\ntarget = \"pan\"\namount = {amount}\n\n"));
        }
        let read = sound_of(&text, &SoundProfile::new()).unwrap();
        assert_eq!(read.connections[CONNECTION_COUNT - 1].amount, 63.0);
    }

    #[test]
    fn values_out_of_range_are_brought_in_by_the_engine() {
        let text = "tuning_hz = 9000\n\n[unison]\ncopies = 500\n\n[[oscillators]]\nlevel = 4000000000\n";
        let read = sound_of(text, &SoundProfile::new()).unwrap();
        // Read as they are...
        assert_eq!(read.tuning_hz, 9000.0);
        assert_eq!(read.unison.copies, 500);

        // ...and brought within range once the song takes them.
        let mut engine = Engine::new().unwrap();
        let mut profile = SoundProfile::new();
        profile.take_sound(&mut engine, &read);
        assert_eq!(profile.tuning_hz(), TUNING_HIGHEST_HZ);
        assert_eq!(profile.unison().copies, UNISON_MOST_COPIES);
        assert_eq!(profile.oscillators()[0].level, 100);
    }

    #[test]
    fn text_that_is_not_toml_can_t_be_read() {
        let result = sound_of("this is [not a sound", &SoundProfile::new());
        let message = result.unwrap_err();
        assert!(message.starts_with("it is not a sound file"), "{message}");
    }
}
