// Hearing the pitch of one voice in a recording (docs/design.md#attachments),
// the YIN way: for each short stretch, find the smallest delay at which the
// sound repeats itself, and that delay is one cycle of the note. Runs of
// the same note make the melody. Attachments use it for the tune of an
// audio file; humming uses it for a hummed line.
//
// Samples are mono: one number a frame.

// The lowest and highest notes looked for, in Hz: a low man's voice to a
// high whistle.
pub const LOWEST_HZ: f32 = 80.0;
pub const HIGHEST_HZ: f32 = 1000.0;

// How many frames each look takes, and how far the next one moves on.
const FRAME: usize = 1024;
const HOP: usize = 512;
// How close to repeating itself the sound must be for a pitch to count:
// lower is stricter.
const THRESHOLD: f32 = 0.15;
// Stretches quieter than this, in dB below full scale, are silence.
const SILENCE_DB: f32 = -45.0;
// How many looks in a row a note must hold to count.
const SHORTEST_NOTE_LOOKS: usize = 4;
// How much of the sound that isn't silence must have a pitch for the
// notes to be worth giving.
const CLEAR_SHARE: f32 = 0.3;
// The most notes given.
pub const MOST_NOTES: usize = 64;

// One note heard: its MIDI number, when it starts and how long it holds,
// in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct HeardNote {
    pub note: u8,
    pub start_seconds: f32,
    pub seconds: f32,
}

// What one look found.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Look {
    Silent,
    Unpitched,
    Pitched(u8),
}

// The pitch of `frame`, in Hz, or None when it doesn't repeat itself
// clearly enough. `frame` must be longer than the longest cycle looked for.
pub fn hz_of(frame: &[f32], sample_rate: u32) -> Option<f32> {
    let shortest_cycle = (sample_rate as f32 / HIGHEST_HZ).floor() as usize;
    let longest_cycle = (sample_rate as f32 / LOWEST_HZ).ceil() as usize;
    if frame.len() <= longest_cycle + 1 || shortest_cycle < 2 {
        return None;
    }
    let width = frame.len() - longest_cycle - 1;

    // How different the sound is from itself `delay` frames later, for
    // each delay.
    let mut differences = vec![0.0_f32; longest_cycle + 2];
    for delay in 1..differences.len() {
        let mut total = 0.0;
        for place in 0..width {
            let step = frame[place] - frame[place + delay];
            total += step * step;
        }
        differences[delay] = total;
    }

    // Each difference against the mean of the ones before it, so short
    // delays don't win just for being short.
    let mut normalized = vec![1.0_f32; differences.len()];
    let mut running = 0.0;
    for delay in 1..differences.len() {
        running += differences[delay];
        if running > 0.0 {
            normalized[delay] = differences[delay] * delay as f32 / running;
        }
    }

    // The first delay under the threshold, walked down to the bottom of
    // its dip.
    let mut found = None;
    let mut delay = shortest_cycle;
    while delay <= longest_cycle {
        if normalized[delay] < THRESHOLD {
            while delay < longest_cycle && normalized[delay + 1] < normalized[delay] {
                delay += 1;
            }
            found = Some(delay);
            break;
        }
        delay += 1;
    }
    let delay = found?;

    // Between whole frames: fit a curve through the dip and its two
    // neighbours.
    let before = normalized[delay - 1];
    let at = normalized[delay];
    let after = normalized[delay + 1];
    let bend = before - 2.0 * at + after;
    let mut exact = delay as f32;
    if bend > 0.0 {
        exact += 0.5 * (before - after) / bend;
    }
    Some(sample_rate as f32 / exact)
}

// The MIDI note nearest `hz`.
pub fn note_of_hz(hz: f32) -> u8 {
    let note = 69.0 + 12.0 * (hz / 440.0).log2();
    note.round().clamp(0.0, 127.0) as u8
}

fn look_at(frame: &[f32], sample_rate: u32) -> Look {
    let mut power = 0.0;
    for sample in frame {
        power += sample * sample;
    }
    power /= frame.len() as f32;
    if power <= 0.0 || 10.0 * power.log10() < SILENCE_DB {
        return Look::Silent;
    }
    match hz_of(frame, sample_rate) {
        Some(hz) => Look::Pitched(note_of_hz(hz)),
        None => Look::Unpitched,
    }
}

// The notes sung, hummed or played in `mono`, oldest first, or None when
// too little of it has a clear pitch (a full mix, drums, speech).
pub fn notes_of(mono: &[f32], sample_rate: u32) -> Option<Vec<HeardNote>> {
    let mut looks = Vec::new();
    let mut start = 0;
    while start + FRAME <= mono.len() {
        looks.push(look_at(&mono[start..start + FRAME], sample_rate));
        start += HOP;
    }

    let mut sounding = 0;
    let mut pitched = 0;
    for look in &looks {
        if *look != Look::Silent {
            sounding += 1;
        }
        if let Look::Pitched(_) = look {
            pitched += 1;
        }
    }
    if sounding < SHORTEST_NOTE_LOOKS {
        return None;
    }
    if (pitched as f32) < CLEAR_SHARE * sounding as f32 {
        return None;
    }

    let seconds_per_look = HOP as f32 / sample_rate as f32;
    let mut notes = Vec::new();
    let mut place = 0;
    while place < looks.len() && notes.len() < MOST_NOTES {
        let Look::Pitched(note) = looks[place] else {
            place += 1;
            continue;
        };
        let mut end = place + 1;
        while end < looks.len() && looks[end] == Look::Pitched(note) {
            end += 1;
        }
        let held = end - place;
        if held >= SHORTEST_NOTE_LOOKS {
            notes.push(HeardNote {
                note,
                start_seconds: place as f32 * seconds_per_look,
                seconds: held as f32 * seconds_per_look,
            });
        }
        place = end;
    }
    Some(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const RATE: u32 = 22_050;

    fn tone(hz: f32, seconds: f32) -> Vec<f32> {
        let frames = (seconds * RATE as f32) as usize;
        let mut samples = Vec::new();
        for frame in 0..frames {
            let time = frame as f32 / RATE as f32;
            // A few overtones, like a voice, so the lowest one must win.
            let wave = (2.0 * PI * hz * time).sin() + 0.5 * (4.0 * PI * hz * time).sin() + 0.25 * (6.0 * PI * hz * time).sin();
            samples.push(0.3 * wave);
        }
        samples
    }

    #[test]
    fn a_tone_is_heard_at_its_pitch() {
        let a = tone(220.0, 0.1);
        let hz = hz_of(&a[..FRAME], RATE).unwrap();
        assert!((hz - 220.0).abs() < 1.0, "{hz}");
        let high = tone(880.0, 0.1);
        assert_eq!(note_of_hz(hz_of(&high[..FRAME], RATE).unwrap()), 81);
        assert_eq!(note_of_hz(261.63), 60);
    }

    #[test]
    fn a_hummed_line_becomes_notes() {
        let mut line = tone(261.63, 0.5);
        line.extend(vec![0.0; RATE as usize / 5]);
        line.extend(tone(329.63, 0.5));
        line.extend(tone(392.0, 0.5));
        let notes = notes_of(&line, RATE).unwrap();
        let pitches: Vec<u8> = notes.iter().map(|heard| heard.note).collect();
        assert_eq!(pitches, vec![60, 64, 67]);
        assert!(notes[0].start_seconds < 0.05);
        assert!((notes[0].seconds - 0.5).abs() < 0.1, "{:?}", notes[0]);
        assert!((notes[1].start_seconds - 0.7).abs() < 0.1, "{:?}", notes[1]);
    }

    #[test]
    fn noise_and_silence_give_no_notes() {
        let mut seed = 1_u32;
        let mut noise = Vec::new();
        for _ in 0..RATE {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            noise.push((seed >> 8) as f32 / (1 << 24) as f32 - 0.5);
        }
        assert_eq!(notes_of(&noise, RATE), None);
        assert_eq!(notes_of(&vec![0.0; RATE as usize], RATE), None);
        assert_eq!(hz_of(&[0.1; 100], RATE), None);
    }
}
