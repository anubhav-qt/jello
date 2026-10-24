// Reading a sound file the player attaches (docs/design.md#attachments):
// FFmpeg turns any audio or video into plain samples, then Jello measures
// it like its own renders (measures.rs) and guesses its tempo, its key and,
// when one voice is clear, its tune (pitch.rs).
//
// FFmpeg runs as its own program, never linked in; Jello looks for it in
// JELLO_FFMPEG, then where Homebrew puts it, then on the PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::engine::CHANNELS;
use crate::harmony::{Key, Mode, key_name};
use crate::measures::{self, Fft};
use crate::pitch;
use crate::song_file::note_name;

// The rate files are read at: plenty for measuring, and light.
pub const RATE: u32 = 22_050;
// The most of a file read, in seconds.
pub const MOST_SECONDS: u32 = 480;
// The most of a file the tune is looked for in, in seconds.
const MELODY_SECONDS: usize = 30;

// The tempo looked for, in beats a minute.
const SLOWEST_BPM: f32 = 60.0;
const FASTEST_BPM: f32 = 180.0;
// The look for the beat: how many frames each takes and how far apart.
const BEAT_FRAME: usize = 1024;
const BEAT_HOP: usize = 512;
// How onsets are blurred over the looks around them.
const BLUR: [f32; 5] = [1.0, 2.0, 3.0, 2.0, 1.0];
// The tempo most music sits near, and how far away, in octaves, a tempo
// still counts as near.
const USUAL_BPM: f32 = 120.0;
const LEANING_OCTAVES: f32 = 1.0;
// The look for the key: longer, so low notes come apart.
const KEY_FRAME: usize = 4096;
const KEY_LOWEST_HZ: f32 = 65.0;
const KEY_HIGHEST_HZ: f32 = 2000.0;

// How much each of the 12 notes belongs to a major and a minor key, from
// the home note up (Krumhansl and Kessler's listening tests).
const MAJOR_PROFILE: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

// Where Homebrew puts its programs on a Mac, which an app opened from the
// Finder doesn't have on its PATH.
const HOMEBREW_FOLDERS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

pub const NO_FFMPEG: &str = "Jello needs FFmpeg to read audio and video. Install it (on a Mac: brew install ffmpeg) or set JELLO_FFMPEG to where it is.";

// FFmpeg's `name` program (ffmpeg or ffprobe), or None if it isn't found.
pub fn program(name: &str) -> Option<PathBuf> {
    if name == "ffmpeg" {
        if let Ok(path) = std::env::var("JELLO_FFMPEG") {
            return Some(PathBuf::from(path));
        }
    }
    if name == "ffprobe" {
        if let Ok(path) = std::env::var("JELLO_FFPROBE") {
            return Some(PathBuf::from(path));
        }
    }
    for folder in HOMEBREW_FOLDERS {
        let path = Path::new(folder).join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    let Ok(output) = Command::new(name).arg("-version").output() else {
        return None;
    };
    if !output.status.success() {
        return None;
    }
    Some(PathBuf::from(name))
}

// The first MOST_SECONDS of `path`'s sound, left and right, at RATE.
pub fn samples_of(path: &Path) -> Result<Vec<f32>, String> {
    let Some(ffmpeg) = program("ffmpeg") else {
        return Err(String::from(NO_FFMPEG));
    };
    let output = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-t", &MOST_SECONDS.to_string(), "-vn", "-ac", "2", "-ar", &RATE.to_string(), "-f", "f32le", "-"])
        .output()
        .map_err(|error| format!("FFmpeg didn't start: {error}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg couldn't read the sound: {}", said.trim()));
    }
    let mut samples = Vec::new();
    for bytes in output.stdout.chunks_exact(4) {
        samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
    Ok(samples)
}

// Left and right as one.
pub fn mono_of(samples: &[f32]) -> Vec<f32> {
    let mut mono = Vec::new();
    for frame in samples.chunks_exact(CHANNELS) {
        mono.push((frame[0] + frame[1]) / 2.0);
    }
    mono
}

// The tempo of `mono`, in beats a minute, or None when nothing pulses. It
// finds when the sound gets suddenly stronger (each onset), then the gap
// at which those onsets most often repeat.
pub fn tempo_of(mono: &[f32], sample_rate: u32) -> Option<f32> {
    let fft = Fft::new(BEAT_FRAME);
    let mut previous: Vec<f32> = Vec::new();
    let mut rises = Vec::new();
    let mut start = 0;
    while start + BEAT_FRAME <= mono.len() {
        let strengths = fft.strengths(&mono[start..start + BEAT_FRAME]);
        let mut levels = Vec::new();
        for strength in strengths {
            levels.push((1.0 + 100.0 * strength).ln());
        }
        let mut rise = 0.0;
        if !previous.is_empty() {
            for bin in 0..levels.len() {
                let step = levels[bin] - previous[bin];
                if step > 0.0 {
                    rise += step;
                }
            }
        }
        rises.push(rise);
        previous = levels;
        start += BEAT_HOP;
    }

    let looks_per_second = sample_rate as f32 / BEAT_HOP as f32;
    let shortest_gap = (60.0 * looks_per_second / FASTEST_BPM).floor() as usize;
    let longest_gap = (60.0 * looks_per_second / SLOWEST_BPM).ceil() as usize;
    if rises.len() < 2 * longest_gap + 2 {
        return None;
    }

    // Blurred a little, so a beat that falls between two looks still
    // lines up with itself.
    let mut blurred = Vec::new();
    for place in 0..rises.len() {
        let mut total = 0.0;
        for (step, weight) in BLUR.iter().enumerate() {
            let near = place as i64 + step as i64 - 2;
            if near >= 0 && (near as usize) < rises.len() {
                total += weight * rises[near as usize];
            }
        }
        blurred.push(total);
    }
    let mean = blurred.iter().sum::<f32>() / blurred.len() as f32;
    let mut centered = Vec::new();
    for rise in &blurred {
        centered.push(rise - mean);
    }

    // How alike the onsets are with themselves `gap` looks later, leaning
    // toward tempos near the usual one: a beat also repeats at twice its
    // gap, and the leaning keeps that from winning.
    let mut alike = vec![0.0_f32; longest_gap + 2];
    for gap in shortest_gap.saturating_sub(1)..alike.len() {
        let mut total = 0.0;
        for place in 0..centered.len() - gap {
            total += centered[place] * centered[place + gap];
        }
        let bpm = 60.0 * looks_per_second / gap as f32;
        let octaves_away = (bpm / USUAL_BPM).log2() / LEANING_OCTAVES;
        let leaning = (-0.5 * octaves_away * octaves_away).exp();
        alike[gap] = leaning * total / (centered.len() - gap) as f32;
    }

    let mut best = shortest_gap;
    for gap in shortest_gap..=longest_gap {
        if alike[gap] > alike[best] {
            best = gap;
        }
    }
    if alike[best] <= 0.0 {
        return None;
    }
    let before = alike[best - 1];
    let at = alike[best];
    let after = alike[best + 1];
    let bend = before - 2.0 * at + after;
    let mut exact = best as f32;
    if bend < 0.0 {
        exact += 0.5 * (before - after) / bend;
    }
    Some(60.0 * looks_per_second / exact)
}

// How strong each of the 12 notes is in `mono`, from C up.
fn chroma_of(mono: &[f32], sample_rate: u32) -> [f32; 12] {
    let fft = Fft::new(KEY_FRAME);
    let mut chroma = [0.0_f32; 12];
    let mut start = 0;
    while start + KEY_FRAME <= mono.len() {
        let strengths = fft.strengths(&mono[start..start + KEY_FRAME]);
        for (bin, strength) in strengths.iter().enumerate().skip(1) {
            let hz = bin as f32 * sample_rate as f32 / KEY_FRAME as f32;
            if hz < KEY_LOWEST_HZ || hz > KEY_HIGHEST_HZ {
                continue;
            }
            let note = pitch::note_of_hz(hz);
            chroma[usize::from(note % 12)] += strength;
        }
        start += KEY_FRAME;
    }
    chroma
}

// How alike two rows of numbers rise and fall, from -1 to 1 (Pearson).
fn alike(first: &[f32; 12], second: &[f32; 12]) -> f32 {
    let first_mean = first.iter().sum::<f32>() / 12.0;
    let second_mean = second.iter().sum::<f32>() / 12.0;
    let mut together = 0.0;
    let mut first_spread = 0.0;
    let mut second_spread = 0.0;
    for note in 0..12 {
        let first_step = first[note] - first_mean;
        let second_step = second[note] - second_mean;
        together += first_step * second_step;
        first_spread += first_step * first_step;
        second_spread += second_step * second_step;
    }
    if first_spread <= 0.0 || second_spread <= 0.0 {
        return 0.0;
    }
    together / (first_spread * second_spread).sqrt()
}

// The key `mono` sounds most like, and how sure that is (from -1 to 1),
// or None for silence.
pub fn key_of(mono: &[f32], sample_rate: u32) -> Option<(Key, f32)> {
    let chroma = chroma_of(mono, sample_rate);
    let mut best: Option<(Key, f32)> = None;
    for tonic in 0..12 {
        for (mode, profile) in [(Mode::Major, MAJOR_PROFILE), (Mode::Minor, MINOR_PROFILE)] {
            let mut turned = [0.0_f32; 12];
            for note in 0..12 {
                turned[(note + tonic) % 12] = profile[note];
            }
            let score = alike(&chroma, &turned);
            let key = Key {
                tonic: tonic as u8,
                mode,
            };
            let mut better = true;
            if let Some((_, best_score)) = best {
                better = score > best_score;
            }
            if better {
                best = Some((key, score));
            }
        }
    }
    let (key, score) = best?;
    if score <= 0.0 {
        return None;
    }
    Some((key, score))
}

// What the agent reads about a sound: how long it is, its measures, its
// tempo and key as guesses, and its tune when one voice is clear.
pub fn reading_of(samples: &[f32], sample_rate: u32) -> Value {
    let mono = mono_of(samples);
    let seconds = mono.len() as f32 / sample_rate as f32;
    let measured = measures::measure(samples, sample_rate);

    let mut tempo = Value::Null;
    if let Some(bpm) = tempo_of(&mono, sample_rate) {
        tempo = json!(bpm.round());
    }
    let mut key = Value::Null;
    let mut key_sureness = Value::Null;
    if let Some((found, score)) = key_of(&mono, sample_rate) {
        key = json!(key_name(found));
        key_sureness = json!((f64::from(score) * 100.0).round() / 100.0);
    }

    let melody_end = mono.len().min(MELODY_SECONDS * sample_rate as usize);
    let mut tune = Value::Null;
    if let Some(notes) = pitch::notes_of(&mono[..melody_end], sample_rate) {
        let mut heard = Vec::new();
        for note in notes {
            heard.push(json!({
                "note": note_name(note.note),
                "at_seconds": (f64::from(note.start_seconds) * 100.0).round() / 100.0,
                "seconds": (f64::from(note.seconds) * 100.0).round() / 100.0,
            }));
        }
        tune = json!(heard);
    }

    json!({
        "seconds": (f64::from(seconds) * 10.0).round() / 10.0,
        "measures": measures::json_of(&measured),
        "tempo_bpm_guess": tempo,
        "key_guess": key,
        "key_sureness": key_sureness,
        "tune_first_30_seconds": tune,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // Stereo clicks at `bpm` over `seconds`, on a quiet hum.
    fn clicks(bpm: f32, seconds: f32) -> Vec<f32> {
        let frames = (seconds * RATE as f32) as usize;
        let gap = (60.0 / bpm * RATE as f32) as usize;
        let mut samples = Vec::new();
        for frame in 0..frames {
            let since = frame % gap;
            let mut sample = 0.01 * (2.0 * PI * 110.0 * frame as f32 / RATE as f32).sin();
            if since < 200 {
                let fade = 1.0 - since as f32 / 200.0;
                sample += 0.5 * fade * (2.0 * PI * 2000.0 * frame as f32 / RATE as f32).sin();
            }
            samples.push(sample);
            samples.push(sample);
        }
        samples
    }

    // C, F, G and C chords with their home notes low, for `seconds` each,
    // in stereo.
    fn chords(seconds: f32) -> Vec<f32> {
        let groups: [&[f32]; 4] = [
            &[130.81, 261.63, 329.63, 392.0],
            &[174.61, 261.63, 349.23, 440.0],
            &[196.0, 246.94, 293.66, 392.0],
            &[130.81, 261.63, 329.63, 392.0],
        ];
        let frames = (seconds * RATE as f32) as usize;
        let mut samples = Vec::new();
        for group in groups {
            for frame in 0..frames {
                let time = frame as f32 / RATE as f32;
                let mut sample = 0.0;
                for hz in group {
                    sample += 0.15 * (2.0 * PI * hz * time).sin();
                }
                samples.push(sample);
                samples.push(sample);
            }
        }
        samples
    }

    #[test]
    fn clicks_give_their_tempo() {
        let samples = clicks(120.0, 10.0);
        let bpm = tempo_of(&mono_of(&samples), RATE).unwrap();
        assert!((bpm - 120.0).abs() < 3.0, "{bpm}");
        let slow = clicks(75.0, 12.0);
        let bpm = tempo_of(&mono_of(&slow), RATE).unwrap();
        assert!((bpm - 75.0).abs() < 3.0, "{bpm}");
        assert_eq!(tempo_of(&[0.0; 1000], RATE), None);
    }

    #[test]
    fn chords_give_their_key() {
        let samples = chords(2.0);
        let (key, score) = key_of(&mono_of(&samples), RATE).unwrap();
        assert_eq!(key_name(key), "C major");
        assert!(score > 0.5, "{score}");
        assert_eq!(key_of(&vec![0.0; 20_000], RATE), None);
    }

    #[test]
    fn a_reading_has_everything() {
        let reading = reading_of(&chords(1.0), RATE);
        assert_eq!(reading["seconds"], 4.0);
        assert_eq!(reading["key_guess"], "C major");
        assert!(reading["measures"]["loudness_db"].is_number());
        // Three notes at once are no one voice.
        assert!(reading.get("tune_first_30_seconds").is_some());
    }

    #[test]
    fn ffmpeg_reads_a_file_when_it_is_there() {
        if program("ffmpeg").is_none() {
            return;
        }
        let folder = crate::presets::test_folder("audio-file");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("tone.wav");
        let status = Command::new(program("ffmpeg").unwrap())
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let samples = samples_of(&path).unwrap();
        assert!((samples.len() as i64 - 2 * RATE as i64).abs() < 2000, "{}", samples.len());
        let reading = reading_of(&samples, RATE);
        assert_eq!(reading["tune_first_30_seconds"][0]["note"], "A4");
        let missing = samples_of(&folder.join("missing.wav"));
        assert!(missing.unwrap_err().starts_with("FFmpeg couldn't read"));
    }
}
