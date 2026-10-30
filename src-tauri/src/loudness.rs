// Loudness as streaming services measure it (ITU-R BS.1770-4, EBU R 128),
// and the true peak, for listening (docs/design.md#mix): the agent checks a
// mix is as loud as a release should be, and that it never clips once it is
// turned into an MP3 or played back.
//
// Loudness, in LUFS:
// 1. Each side goes through K-weighting: a shelf that lifts the highs by
//    4 dB, as a head does, then a high-pass that lets go of the deepest
//    bass, which ears hardly hear.
// 2. The weighted sound is cut into blocks of 400 ms, a new one every
//    100 ms, and each block's power is added up over both sides.
// 3. Blocks quieter than -70 LUFS are left out (silence), and then blocks
//    more than 10 LU quieter than the rest (quiet passages), so a song is
//    measured by what is playing.
//
// The true peak, in dB: the loudest point between the samples as well as
// on them, found by reading the sound at 4 times its sample rate. A
// converter draws the smooth wave through the samples, and its peaks can
// rise above every sample.
//
// Samples come as the engine renders them: left, right, left, right, ...

use std::f64::consts::PI;

use crate::engine::CHANNELS;

// The blocks loudness is measured in, and how often a new one starts.
const BLOCK_MS: f64 = 400.0;
const STEP_MS: f64 = 100.0;

// The gates: blocks below ABSOLUTE_GATE_LUFS are silence, and blocks more
// than RELATIVE_GATE_LU below the loudness of the rest are left out too.
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
const RELATIVE_GATE_LU: f64 = 10.0;

// BS.1770's offset, so a 1 kHz sine at full scale on one side reads
// -3.01 LUFS.
const LOUDNESS_OFFSET: f64 = -0.691;

// The K-weighting filters, as BS.1770 gives them, worked out for any
// sample rate: the shelf's centre, gain and width, and the high-pass's
// corner and width.
const SHELF_HZ: f64 = 1681.974450955533;
const SHELF_GAIN_DB: f64 = 3.999843853973347;
const SHELF_Q: f64 = 0.7071752369554196;
const SHELF_BAND_POWER: f64 = 0.4996667741545416;
const HIGH_PASS_HZ: f64 = 38.13547087602444;
const HIGH_PASS_Q: f64 = 0.5003270373238773;

// How many times the sample rate the true peak is read at, and how many
// samples on each side of a point the reading uses.
const OVERSAMPLING: usize = 4;
const HALF_TAPS: i64 = 6;

// A filter of two poles and two zeros, run one sample at a time.
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    // The two samples in and out before this one.
    in_1: f64,
    in_2: f64,
    out_1: f64,
    out_2: f64,
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Biquad {
        Biquad {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
            in_1: 0.0,
            in_2: 0.0,
            out_1: 0.0,
            out_2: 0.0,
        }
    }

    fn next(&mut self, sample: f64) -> f64 {
        let mut out = self.b0 * sample + self.b1 * self.in_1 + self.b2 * self.in_2;
        out -= self.a1 * self.out_1 + self.a2 * self.out_2;
        self.in_2 = self.in_1;
        self.in_1 = sample;
        self.out_2 = self.out_1;
        self.out_1 = out;
        out
    }
}

// The first K-weighting stage: the high shelf.
fn shelf(sample_rate: u32) -> Biquad {
    let k = (PI * SHELF_HZ / f64::from(sample_rate)).tan();
    let high = 10.0_f64.powf(SHELF_GAIN_DB / 20.0);
    let band = high.powf(SHELF_BAND_POWER);
    let b = [high + band * k / SHELF_Q + k * k, 2.0 * (k * k - high), high - band * k / SHELF_Q + k * k];
    let a = [1.0 + k / SHELF_Q + k * k, 2.0 * (k * k - 1.0), 1.0 - k / SHELF_Q + k * k];
    Biquad::new(b, a)
}

// The second K-weighting stage: the high-pass.
fn high_pass(sample_rate: u32) -> Biquad {
    let k = (PI * HIGH_PASS_HZ / f64::from(sample_rate)).tan();
    let b = [1.0, -2.0, 1.0];
    let a = [1.0 + k / HIGH_PASS_Q + k * k, 2.0 * (k * k - 1.0), 1.0 - k / HIGH_PASS_Q + k * k];
    // The high-pass passes the mids at full level, so its zeros need no
    // scaling: BS.1770 gives them as 1, -2, 1.
    let scale = a[0];
    Biquad::new([b[0] * scale, b[1] * scale, b[2] * scale], a)
}

// The integrated loudness of `samples`, rendered at `sample_rate` frames a
// second, in LUFS. None for less than one 400 ms block, or for silence.
pub fn integrated_lufs(samples: &[f32], sample_rate: u32) -> Option<f64> {
    let powers = block_powers(samples, sample_rate);

    let mut loud = Vec::new();
    for power in powers {
        if lufs_of(power) > ABSOLUTE_GATE_LUFS {
            loud.push(power);
        }
    }
    if loud.is_empty() {
        return None;
    }
    let gate = lufs_of(mean(&loud)) - RELATIVE_GATE_LU;

    let mut kept = Vec::new();
    for power in loud {
        if lufs_of(power) > gate {
            kept.push(power);
        }
    }
    Some(lufs_of(mean(&kept)))
}

// Each 400 ms block's K-weighted power, both sides added up.
fn block_powers(samples: &[f32], sample_rate: u32) -> Vec<f64> {
    let frames = samples.len() / CHANNELS;
    let mut weighted = vec![0.0; frames];
    for side in 0..CHANNELS {
        let mut first = shelf(sample_rate);
        let mut second = high_pass(sample_rate);
        for frame in 0..frames {
            let sample = f64::from(samples[frame * CHANNELS + side]);
            let filtered = second.next(first.next(sample));
            weighted[frame] += filtered * filtered;
        }
    }

    let block = (BLOCK_MS * f64::from(sample_rate) / 1000.0).round() as usize;
    let step = (STEP_MS * f64::from(sample_rate) / 1000.0).round() as usize;
    let mut powers = Vec::new();
    let mut start = 0;
    while start + block <= frames {
        let sum: f64 = weighted[start..start + block].iter().sum();
        powers.push(sum / block as f64);
        start += step;
    }
    powers
}

fn lufs_of(power: f64) -> f64 {
    LOUDNESS_OFFSET + 10.0 * power.log10()
}

fn mean(numbers: &[f64]) -> f64 {
    let sum: f64 = numbers.iter().sum();
    sum / numbers.len() as f64
}

// The true peak of `samples`, in dB below full scale: the loudest point on
// either side, read at OVERSAMPLING times the sample rate. None for
// silence.
pub fn true_peak_db(samples: &[f32]) -> Option<f64> {
    let frames = samples.len() / CHANNELS;
    let phases = phase_taps();
    let mut peak: f64 = 0.0;
    for side in 0..CHANNELS {
        let at = |frame: i64| -> f64 {
            if frame < 0 || frame >= frames as i64 {
                return 0.0;
            }
            f64::from(samples[frame as usize * CHANNELS + side])
        };
        for frame in 0..frames as i64 {
            for taps in &phases {
                let mut point = 0.0;
                for (place, tap) in taps.iter().enumerate() {
                    point += at(frame + place as i64 - HALF_TAPS + 1) * tap;
                }
                peak = peak.max(point.abs());
            }
        }
    }
    if peak == 0.0 {
        return None;
    }
    Some(20.0 * peak.log10())
}

// For each point between two samples, OVERSAMPLING of them from the sample
// itself, the weights of the samples around it: a sinc, faded out at the
// edges so it ends, each phase adding up to 1 so a steady level stays put.
fn phase_taps() -> Vec<Vec<f64>> {
    let mut phases = Vec::new();
    for phase in 0..OVERSAMPLING {
        let offset = phase as f64 / OVERSAMPLING as f64;
        let mut taps = Vec::new();
        for place in 0..2 * HALF_TAPS {
            // How far the sample at `place` is from the point.
            let distance = offset - (place - HALF_TAPS + 1) as f64;
            taps.push(sinc(distance) * fade(distance));
        }
        let sum: f64 = taps.iter().sum();
        for tap in &mut taps {
            *tap /= sum;
        }
        phases.push(taps);
    }
    phases
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        return 1.0;
    }
    (PI * x).sin() / (PI * x)
}

// A Hann window across HALF_TAPS samples each way: 1 at the middle, 0 at
// the edges.
fn fade(distance: f64) -> f64 {
    let share = distance / HALF_TAPS as f64;
    0.5 * (1.0 + (PI * share).cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    // `seconds` of a 1 kHz sine whose peak is `dbfs`, on both sides, as the
    // EBU's loudness tests (Tech 3341) use.
    fn tone(dbfs: f64, seconds: f64) -> Vec<f32> {
        let level = 10.0_f64.powf(dbfs / 20.0);
        let frames = (f64::from(RATE) * seconds) as usize;
        let mut samples = Vec::new();
        for n in 0..frames {
            let sample = level * (2.0 * PI * 1000.0 * n as f64 / f64::from(RATE)).sin();
            samples.push(sample as f32);
            samples.push(sample as f32);
        }
        samples
    }

    fn assert_near(measured: Option<f64>, expected: f64, within: f64) {
        let measured = measured.unwrap();
        assert!((measured - expected).abs() <= within, "{measured} is not {expected}");
    }

    #[test]
    fn a_steady_tone_measures_as_the_ebu_says() {
        // EBU Tech 3341, cases 1 and 2.
        assert_near(integrated_lufs(&tone(-23.0, 20.0), RATE), -23.0, 0.1);
        assert_near(integrated_lufs(&tone(-33.0, 20.0), RATE), -33.0, 0.1);
    }

    #[test]
    fn the_same_tone_measures_the_same_at_other_sample_rates() {
        let rate = 44_100;
        let level = 10.0_f64.powf(-23.0 / 20.0);
        let mut samples = Vec::new();
        for n in 0..rate * 10 {
            let sample = level * (2.0 * PI * 1000.0 * f64::from(n) / f64::from(rate)).sin();
            samples.push(sample as f32);
            samples.push(sample as f32);
        }
        assert_near(integrated_lufs(&samples, rate), -23.0, 0.1);
    }

    #[test]
    fn quiet_passages_and_silence_are_gated_out() {
        // EBU Tech 3341, cases 3 and 4.
        let mut three = tone(-36.0, 10.0);
        three.extend(tone(-23.0, 60.0));
        three.extend(tone(-36.0, 10.0));
        assert_near(integrated_lufs(&three, RATE), -23.0, 0.1);

        let mut four = tone(-72.0, 10.0);
        four.extend(tone(-36.0, 10.0));
        four.extend(tone(-23.0, 60.0));
        four.extend(tone(-36.0, 10.0));
        four.extend(tone(-72.0, 10.0));
        assert_near(integrated_lufs(&four, RATE), -23.0, 0.1);
    }

    #[test]
    fn deep_bass_counts_for_less_than_the_mids() {
        let level = 10.0_f64.powf(-23.0 / 20.0);
        let mut bass = Vec::new();
        for n in 0..RATE * 5 {
            let sample = level * (2.0 * PI * 30.0 * f64::from(n) / f64::from(RATE)).sin();
            bass.push(sample as f32);
            bass.push(sample as f32);
        }
        let bass_lufs = integrated_lufs(&bass, RATE).unwrap();
        assert!(bass_lufs < -24.0, "{bass_lufs}");
    }

    #[test]
    fn silence_and_too_little_sound_have_no_loudness() {
        assert_eq!(integrated_lufs(&vec![0.0; 2 * RATE as usize * 2], RATE), None);
        assert_eq!(integrated_lufs(&tone(-23.0, 0.3), RATE), None);
        // Quieter than -70 LUFS throughout is silence too.
        assert_eq!(integrated_lufs(&tone(-75.0, 5.0), RATE), None);
        assert_eq!(integrated_lufs(&[], RATE), None);
        assert_eq!(true_peak_db(&vec![0.0; 1000]), None);
        assert_eq!(true_peak_db(&[]), None);
    }

    #[test]
    fn the_true_peak_finds_peaks_between_the_samples() {
        // A sine at a quarter of the sample rate, its samples landing half
        // way up each side: every sample is at -3 dB, but the wave peaks at
        // 0 dB between them.
        let mut samples = Vec::new();
        for n in 0..4000 {
            let sample = (PI / 2.0 * f64::from(n) + PI / 4.0).sin();
            samples.push(sample as f32);
            samples.push(sample as f32);
        }
        let sample_peak = samples.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        assert!(20.0 * f64::from(sample_peak).log10() < -2.9);
        assert_near(true_peak_db(&samples), 0.0, 0.3);
    }

    #[test]
    fn the_true_peak_of_a_slow_wave_is_its_sample_peak() {
        assert_near(true_peak_db(&tone(-6.0, 1.0)), -6.0, 0.05);
        let mut one_side = tone(-12.0, 1.0);
        for frame in 0..one_side.len() / 2 {
            one_side[frame * 2] = 0.0;
        }
        assert_near(true_peak_db(&one_side), -12.0, 0.05);
    }
}
