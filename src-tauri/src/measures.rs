// Measuring rendered sound (docs/design.md#the-agent-harness), so the agent
// can check what it made without ears: how loud it is, whether it reaches
// the soft clip, how bright it is, how its sound splits between lows, mids
// and highs, how wide it is, and where it falls silent.
//
// Samples come as the engine renders them: left, right, left, right, ...

use std::f32::consts::PI;

use serde_json::{Value, json};

use crate::engine::CHANNELS;

// Where the engine's soft clip starts bending samples (`clip_start` in
// engine/src/conductor.cpp).
const CLIP_START: f32 = 0.8;

// A stretch whose loudest sample is quieter than this, -60 dB, is silent.
pub const SILENCE_LEVEL: f32 = 0.001;

// How long silence has to last before it counts, and the steps it is looked
// for in.
const SHORTEST_SILENCE_MS: u32 = 200;
const SILENCE_STEP_MS: u32 = 10;

// How many frames each spectrum looks at: a power of two, about 43 ms at
// 48,000 frames a second.
const WINDOW: usize = 2048;

// Where the lows end and the highs start, in Hz. The mids are between.
const LOWS_END_HZ: f32 = 250.0;
const HIGHS_START_HZ: f32 = 4000.0;

// What one stretch of sound measures.
#[derive(Debug, PartialEq)]
pub struct Measures {
    // How loud it is on average (its RMS), in dB below full scale. None
    // for silence.
    pub loudness_db: Option<f32>,
    // Its loudest sample, in dB below full scale. None for silence.
    pub peak_db: Option<f32>,
    // Whether a sample went past CLIP_START, where the soft clip bends them.
    pub soft_clip_reached: bool,
    // The middle of its spectrum, weighted by strength (its spectral
    // centroid), in Hz: higher sounds brighter. None for silence.
    pub brightness_hz: Option<f32>,
    // How its power splits between the lows, the mids and the highs, in
    // percent. All 0 for silence.
    pub lows_percent: f32,
    pub mids_percent: f32,
    pub highs_percent: f32,
    // How much of its power differs between left and right: 0 for the same
    // sound on both sides, 50 for sound on one side only, 100 for the two
    // sides opposite.
    pub width_percent: f32,
    // How much louder the left is than the right, in dB. Negative when the
    // right is louder.
    pub balance_db: f32,
}

// Measures `samples`, rendered at `sample_rate` frames a second.
pub fn measure(samples: &[f32], sample_rate: u32) -> Measures {
    let mut sum_of_squares = 0.0_f64;
    let mut peak = 0.0_f32;
    let mut left_power = 0.0_f64;
    let mut right_power = 0.0_f64;
    let mut middle_power = 0.0_f64;
    let mut side_power = 0.0_f64;
    for frame in samples.chunks_exact(CHANNELS) {
        let left = frame[0];
        let right = frame[1];
        sum_of_squares += f64::from(left * left + right * right);
        peak = peak.max(left.abs());
        peak = peak.max(right.abs());
        left_power += f64::from(left * left);
        right_power += f64::from(right * right);
        let middle = (left + right) / 2.0;
        let side = (left - right) / 2.0;
        middle_power += f64::from(middle * middle);
        side_power += f64::from(side * side);
    }

    let mut loudness_db = None;
    let mut peak_db = None;
    if peak > 0.0 {
        let mean_square = sum_of_squares / samples.len() as f64;
        loudness_db = Some(power_db(mean_square));
        peak_db = Some(20.0 * peak.log10());
    }

    let mut width_percent = 0.0;
    let both_powers = middle_power + side_power;
    if both_powers > 0.0 {
        width_percent = (side_power / both_powers * 100.0) as f32;
    }

    let spectrum = spectrum_of(samples);
    let bands = bands_of(&spectrum, sample_rate);
    Measures {
        loudness_db,
        peak_db,
        soft_clip_reached: peak > CLIP_START,
        brightness_hz: brightness_of(&spectrum, sample_rate),
        lows_percent: bands[0],
        mids_percent: bands[1],
        highs_percent: bands[2],
        width_percent,
        balance_db: balance_of(left_power, right_power),
    }
}

// `power` in dB, where 1 is full scale.
fn power_db(power: f64) -> f32 {
    (10.0 * power.log10()) as f32
}

// How much louder the left is than the right, in dB. A silent side counts
// as SILENCE_LEVEL, so one side alone measures a number, not infinity.
fn balance_of(left_power: f64, right_power: f64) -> f32 {
    if left_power == right_power {
        return 0.0;
    }
    let floor = f64::from(SILENCE_LEVEL * SILENCE_LEVEL);
    let left = left_power.max(floor);
    let right = right_power.max(floor);
    power_db(left / right)
}

// The silent stretches in `samples`, each at least SHORTEST_SILENCE_MS
// long, as the frame it starts on and the frame after it ends.
pub fn silences(samples: &[f32], sample_rate: u32) -> Vec<(usize, usize)> {
    let step = (sample_rate * SILENCE_STEP_MS / 1000) as usize;
    let shortest = (sample_rate * SHORTEST_SILENCE_MS / 1000) as usize;
    let frames = samples.len() / CHANNELS;

    let mut stretches = Vec::new();
    let mut silent_from = None;
    let mut start = 0;
    while start < frames {
        let end = (start + step).min(frames);
        let quiet = peak_of(&samples[start * CHANNELS..end * CHANNELS]) < SILENCE_LEVEL;
        if quiet && silent_from.is_none() {
            silent_from = Some(start);
        }
        if !quiet {
            if let Some(from) = silent_from {
                stretches.push((from, start));
            }
            silent_from = None;
        }
        start = end;
    }
    if let Some(from) = silent_from {
        stretches.push((from, frames));
    }

    stretches.retain(|(from, to)| to - from >= shortest);
    stretches
}

// The loudest sample in `samples`, from 0 up.
pub fn peak_of(samples: &[f32]) -> f32 {
    let mut peak = 0.0_f32;
    for sample in samples {
        peak = peak.max(sample.abs());
    }
    peak
}

// The strength of each frequency in `samples`, left and right together,
// from 0 Hz up in steps of the sample rate over WINDOW, as magnitudes added
// up over every WINDOW frames. The windows follow each other, and the last
// one ends with the samples, so every frame is looked at. Fewer frames
// than a window are padded with silence.
fn spectrum_of(samples: &[f32]) -> Vec<f32> {
    let mut middle = Vec::new();
    for frame in samples.chunks_exact(CHANNELS) {
        middle.push((frame[0] + frame[1]) / 2.0);
    }

    let mut starts = Vec::new();
    let mut start = 0;
    while start + WINDOW <= middle.len() {
        starts.push(start);
        start += WINDOW;
    }
    if start < middle.len() {
        starts.push(middle.len().saturating_sub(WINDOW));
    }

    let fft = Fft::new(WINDOW);
    let mut spectrum = vec![0.0; WINDOW / 2];
    for start in starts {
        let end = (start + WINDOW).min(middle.len());
        let strengths = fft.strengths(&middle[start..end]);
        for (bin, strength) in strengths.iter().enumerate() {
            spectrum[bin] += strength;
        }
    }
    spectrum
}

// The frequency of `bin` in a spectrum, in Hz.
fn hz_of(bin: usize, sample_rate: u32) -> f32 {
    bin as f32 * sample_rate as f32 / WINDOW as f32
}

// The middle of `spectrum`, weighted by strength, or None for silence. The
// first bin, 0 Hz, is left out: it is an offset, not a sound.
fn brightness_of(spectrum: &[f32], sample_rate: u32) -> Option<f32> {
    let mut weighted = 0.0_f64;
    let mut total = 0.0_f64;
    for (bin, strength) in spectrum.iter().enumerate().skip(1) {
        weighted += f64::from(hz_of(bin, sample_rate) * strength);
        total += f64::from(*strength);
    }
    if total <= 0.0 {
        return None;
    }
    Some((weighted / total) as f32)
}

// How the power in `spectrum` splits between lows, mids and highs, in
// percent. All 0 for silence. 0 Hz is left out, as for brightness.
fn bands_of(spectrum: &[f32], sample_rate: u32) -> [f32; 3] {
    let mut powers = [0.0_f64; 3];
    for (bin, strength) in spectrum.iter().enumerate().skip(1) {
        let hz = hz_of(bin, sample_rate);
        let power = f64::from(strength * strength);
        if hz < LOWS_END_HZ {
            powers[0] += power;
        } else if hz < HIGHS_START_HZ {
            powers[1] += power;
        } else {
            powers[2] += power;
        }
    }

    let total = powers[0] + powers[1] + powers[2];
    let mut percents = [0.0; 3];
    if total > 0.0 {
        for band in 0..3 {
            percents[band] = (powers[band] / total * 100.0) as f32;
        }
    }
    percents
}

// The fast Fourier transform for one window length, with what it needs
// worked out once: the turns it multiplies by, and the Hann window that
// fades each stretch in and out so its edges don't add frequencies of
// their own.
pub(crate) struct Fft {
    length: usize,
    cosines: Vec<f32>,
    sines: Vec<f32>,
    hann: Vec<f32>,
}

impl Fft {
    // `length` must be a power of two.
    pub(crate) fn new(length: usize) -> Fft {
        let mut cosines = Vec::new();
        let mut sines = Vec::new();
        for k in 0..length / 2 {
            let angle = -2.0 * PI * k as f32 / length as f32;
            cosines.push(angle.cos());
            sines.push(angle.sin());
        }
        let mut hann = Vec::new();
        for n in 0..length {
            let turn = 2.0 * PI * n as f32 / length as f32;
            hann.push(0.5 - 0.5 * turn.cos());
        }
        Fft {
            length,
            cosines,
            sines,
            hann,
        }
    }

    // The magnitude of each frequency in `samples`, up to `length` of them,
    // from 0 Hz to just under half the sample rate.
    pub(crate) fn strengths(&self, samples: &[f32]) -> Vec<f32> {
        let mut real = vec![0.0; self.length];
        let mut imaginary = vec![0.0; self.length];
        for (n, sample) in samples.iter().enumerate() {
            real[n] = sample * self.hann[n];
        }
        self.transform(&mut real, &mut imaginary);

        let mut strengths = Vec::new();
        for bin in 0..self.length / 2 {
            let magnitude = (real[bin] * real[bin] + imaginary[bin] * imaginary[bin]).sqrt();
            strengths.push(magnitude);
        }
        strengths
    }

    // Turns `real` and `imaginary`, a stretch of sound, into its
    // frequencies, in place (the Cooley-Tukey way): first the numbers go in
    // bit-reversed order, then ever longer stretches are put together from
    // two halves each.
    fn transform(&self, real: &mut [f32], imaginary: &mut [f32]) {
        let length = self.length;
        let mut reversed = 0;
        for place in 1..length {
            let mut bit = length / 2;
            while reversed & bit != 0 {
                reversed ^= bit;
                bit /= 2;
            }
            reversed |= bit;
            if place < reversed {
                real.swap(place, reversed);
                imaginary.swap(place, reversed);
            }
        }

        let mut stretch = 2;
        while stretch <= length {
            let half = stretch / 2;
            let turn_step = length / stretch;
            for start in (0..length).step_by(stretch) {
                for k in 0..half {
                    let cosine = self.cosines[k * turn_step];
                    let sine = self.sines[k * turn_step];
                    let first = start + k;
                    let second = first + half;
                    let turned_real = real[second] * cosine - imaginary[second] * sine;
                    let turned_imaginary = real[second] * sine + imaginary[second] * cosine;
                    real[second] = real[first] - turned_real;
                    imaginary[second] = imaginary[first] - turned_imaginary;
                    real[first] += turned_real;
                    imaginary[first] += turned_imaginary;
                }
            }
            stretch *= 2;
        }
    }
}

// `measures` as the agent reads them, rounded: dB and percents to a tenth,
// Hz to whole numbers.
pub fn json_of(measures: &Measures) -> Value {
    json!({
        "loudness_db": rounded(measures.loudness_db, 10.0),
        "peak_db": rounded(measures.peak_db, 10.0),
        "soft_clip_reached": measures.soft_clip_reached,
        "brightness_hz": rounded(measures.brightness_hz, 1.0),
        "lows_percent": tenths(measures.lows_percent),
        "mids_percent": tenths(measures.mids_percent),
        "highs_percent": tenths(measures.highs_percent),
        "width_percent": tenths(measures.width_percent),
        "balance_db": tenths(measures.balance_db),
    })
}

// `number` rounded to 1 / `steps`, or null for None. Rounded as a 64-bit
// number, so -9.1 reaches the agent as -9.1, not -9.100000381469727.
fn rounded(number: Option<f32>, steps: f64) -> Value {
    match number {
        Some(number) => json!((f64::from(number) * steps).round() / steps),
        None => Value::Null,
    }
}

fn tenths(number: f32) -> Value {
    rounded(Some(number), 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    // `seconds` of a sine wave at `hz`, `level` loud, the same on both
    // sides.
    fn sine(hz: f32, level: f32, seconds: f32) -> Vec<f32> {
        let frames = (RATE as f32 * seconds) as usize;
        let mut samples = Vec::new();
        for n in 0..frames {
            let sample = level * (2.0 * PI * hz * n as f32 / RATE as f32).sin();
            samples.push(sample);
            samples.push(sample);
        }
        samples
    }

    fn near(measured: f32, expected: f32, within: f32) {
        assert!(
            (measured - expected).abs() <= within,
            "measured {measured}, expected {expected} within {within}"
        );
    }

    #[test]
    fn a_half_loud_sine_peaks_at_minus_6_db_and_averages_minus_9() {
        let measures = measure(&sine(1000.0, 0.5, 1.0), RATE);
        near(measures.peak_db.unwrap(), -6.02, 0.05);
        near(measures.loudness_db.unwrap(), -9.03, 0.05);
        assert!(!measures.soft_clip_reached);
    }

    #[test]
    fn a_sine_is_as_bright_as_its_frequency() {
        for hz in [110.0, 1000.0, 6000.0] {
            let measures = measure(&sine(hz, 0.5, 0.5), RATE);
            near(measures.brightness_hz.unwrap(), hz, hz * 0.05);
        }
    }

    #[test]
    fn each_sine_lands_in_its_own_band() {
        let low = measure(&sine(100.0, 0.5, 0.5), RATE);
        near(low.lows_percent, 100.0, 1.0);
        let middle = measure(&sine(1000.0, 0.5, 0.5), RATE);
        near(middle.mids_percent, 100.0, 1.0);
        let high = measure(&sine(8000.0, 0.5, 0.5), RATE);
        near(high.highs_percent, 100.0, 1.0);
    }

    #[test]
    fn past_0_8_the_soft_clip_is_reached() {
        let measures = measure(&sine(440.0, 0.9, 0.2), RATE);
        assert!(measures.soft_clip_reached);
    }

    #[test]
    fn the_same_on_both_sides_is_narrow_and_one_side_is_half_wide() {
        let both = measure(&sine(440.0, 0.5, 0.2), RATE);
        near(both.width_percent, 0.0, 0.01);
        near(both.balance_db, 0.0, 0.01);

        let mut left_only = sine(440.0, 0.5, 0.2);
        for frame in left_only.chunks_exact_mut(CHANNELS) {
            frame[1] = 0.0;
        }
        let measures = measure(&left_only, RATE);
        near(measures.width_percent, 50.0, 0.01);
        assert!(measures.balance_db > 40.0);
    }

    #[test]
    fn silence_measures_nothing() {
        let measures = measure(&vec![0.0; 9600], RATE);
        assert_eq!(measures.loudness_db, None);
        assert_eq!(measures.peak_db, None);
        assert_eq!(measures.brightness_hz, None);
        assert_eq!(measures.lows_percent, 0.0);
        assert_eq!(json_of(&measures)["loudness_db"], Value::Null);
    }

    #[test]
    fn silences_are_found_where_they_last_long_enough() {
        // Half a second of sound, a quarter of silence, a tenth of sound,
        // a tenth of silence (too short to count), and the rest silent.
        let mut samples = sine(440.0, 0.5, 0.5);
        samples.extend(vec![0.0; 2 * 12_000]);
        samples.extend(sine(440.0, 0.5, 0.1));
        samples.extend(vec![0.0; 2 * 4_800]);
        samples.extend(sine(440.0, 0.5, 0.1));
        samples.extend(vec![0.0; 2 * 24_000]);

        let found = silences(&samples, RATE);
        assert_eq!(found, vec![(24_000, 36_000), (50_400, 74_400)]);
    }

    #[test]
    fn measures_round_for_the_agent() {
        let measures = measure(&sine(1000.0, 0.5, 0.5), RATE);
        let json = json_of(&measures);
        assert_eq!(json["peak_db"], json!(-6.0));
        assert_eq!(json["mids_percent"], json!(100.0));
    }
}
