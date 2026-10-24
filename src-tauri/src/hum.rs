// Humming a tune to the agent (docs/design.md#voice). After a count in on
// the click, the mic listens; once the player is done, the pitch finder
// turns the hum into notes, and the tempo and key are worked out. The notes,
// their times and the guesses go with the player's next message, never the
// sound, though the recording is kept beside the chat.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::attachments::{Attachment, Attachments};
use crate::audio_file;
use crate::engine::Engine;
use crate::harmony::key_name;
use crate::mic::{self, LISTENING_RATE, Mic};
use crate::pitch::{self, HeardNote};
use crate::song_file::note_name;

// Beats in a bar, as everywhere in Jello.
const BEATS_PER_BAR: u32 = 4;
// The longest count in, in bars.
const MOST_COUNT_IN_BARS: u32 = 2;
// The longest hum, in seconds.
const MOST_SECONDS: f64 = 60.0;
const NO_TUNE: &str = "No clear tune was heard. Hum a little louder, nearer the mic, one note at a time.";

// A hum while it is being heard.
#[derive(Default)]
pub struct Hum {
    listening: Option<Humming>,
}

struct Humming {
    // The tempo the player set, or None for a free tempo.
    tempo: Option<u32>,
    count_in_bars: u32,
    // When the count in ends: the hum's beat 0.
    starts: Instant,
    started: Instant,
}

// One note of the hum, timed from the end of the count in.
struct TimedNote {
    note: u8,
    seconds: f64,
    hold_seconds: f64,
}

// Starts a hum on mic `input` (None for the system's pick). With a set
// tempo, the click counts in `count_in_bars` bars at the song's tempo and
// keeps going if `click`. A free tempo has no count in and no click.
// Gives the tempo and how long the count in lasts, for the screen to count.
// Off the main thread: the first time, opening the mic waits for the
// system to ask the player.
#[tauri::command(async)]
pub fn start_hum(app: AppHandle, input: Option<usize>, count_in_bars: u32, click: bool, free: bool) -> Result<Value, String> {
    let hum = app.state::<Mutex<Hum>>();
    let mut hum = hum.lock().unwrap();
    if hum.listening.is_some() {
        return Err(String::from("a hum is already being heard"));
    }
    if count_in_bars > MOST_COUNT_IN_BARS {
        return Err(format!("the count in is at most {MOST_COUNT_IN_BARS} bars"));
    }

    let mut tempo = None;
    let mut count_in = 0;
    let mut clicks = false;
    if !free {
        let engine = app.state::<Mutex<Engine>>();
        tempo = Some(engine.lock().unwrap().tempo());
        count_in = count_in_bars;
        clicks = click;
    }

    {
        let engine = app.state::<Mutex<Engine>>();
        engine.lock().unwrap().start_audio()?;
    }
    {
        let mic = app.state::<Mutex<Mic>>();
        let mut mic = mic.lock().unwrap();
        mic.start(&app, input)?;
    }
    let started = Instant::now();
    let mut count_in_seconds = 0.0;
    if let Some(tempo) = tempo {
        count_in_seconds = f64::from(count_in * BEATS_PER_BAR) * 60.0 / f64::from(tempo);
        if count_in > 0 || clicks {
            let engine = app.state::<Mutex<Engine>>();
            engine.lock().unwrap().start_click();
        }
        // Without the click, it stops once the count in is over.
        if count_in > 0 && !clicks {
            let handle = app.clone();
            let wait = Duration::from_secs_f64(count_in_seconds);
            thread::spawn(move || {
                thread::sleep(wait);
                let hum = handle.state::<Mutex<Hum>>();
                let still = hum.lock().unwrap().listening.is_some();
                if still {
                    let engine = handle.state::<Mutex<Engine>>();
                    engine.lock().unwrap().stop_click();
                }
            });
        }
    }

    hum.listening = Some(Humming {
        tempo,
        count_in_bars: count_in,
        starts: started + Duration::from_secs_f64(count_in_seconds),
        started,
    });
    Ok(json!({
        "tempo": tempo,
        "count_in_beats": count_in * BEATS_PER_BAR,
        "count_in_seconds": count_in_seconds,
        "most_seconds": MOST_SECONDS,
    }))
}

// Stops listening and reads the hum. The reading waits to go with the
// player's next message, as an attached file. Gives the notes for the
// screen, the guesses and the file's card.
#[tauri::command(async)]
pub fn stop_hum(app: AppHandle) -> Result<Value, String> {
    let humming = {
        let hum = app.state::<Mutex<Hum>>();
        let mut hum = hum.lock().unwrap();
        hum.listening.take()
    };
    let Some(humming) = humming else {
        return Err(String::from("no hum is being heard"));
    };
    let heard = {
        let mic = app.state::<Mutex<Mic>>();
        let mut mic = mic.lock().unwrap();
        mic.stop(&app)
    };
    let rate = {
        let engine = app.state::<Mutex<Engine>>();
        let mut engine = engine.lock().unwrap();
        engine.stop_click();
        engine.sample_rate()
    };
    let heard = heard.unwrap_or_default();

    // Only what came after the count in.
    let count_in = humming.starts.duration_since(humming.started).as_secs_f64();
    let skip = ((count_in * f64::from(rate)) as usize).min(heard.len());
    let mut sound = mic::resampled(&heard[skip..], rate, LISTENING_RATE);
    sound.truncate((MOST_SECONDS * f64::from(LISTENING_RATE)) as usize);

    let path = keep_recording(&sound)?;
    let attachment = reading_of(&sound, &humming, &path)?;
    let card = attachment.card();
    let notes = json!(attachment_notes(&sound)?);
    let reading = attachment.text.clone();
    let attachments = app.state::<Mutex<Attachments>>();
    let mut attachments = attachments.lock().unwrap();
    attachments.add(attachment)?;
    Ok(json!({
        "card": card,
        "cards": attachments.cards(),
        "notes": notes,
        "reading": reading,
    }))
}

// Stops listening and forgets the hum.
#[tauri::command]
pub fn cancel_hum(app: AppHandle) {
    let humming = {
        let hum = app.state::<Mutex<Hum>>();
        let mut hum = hum.lock().unwrap();
        hum.listening.take()
    };
    if humming.is_none() {
        return;
    }
    {
        let mic = app.state::<Mutex<Mic>>();
        let mut mic = mic.lock().unwrap();
        mic.stop(&app);
    }
    let engine = app.state::<Mutex<Engine>>();
    engine.lock().unwrap().stop_click();
}

// The notes heard, for the screen to draw: MIDI number, start and hold in
// seconds.
fn attachment_notes(sound: &[f32]) -> Result<Vec<Value>, String> {
    let notes = timed_notes(sound)?;
    let mut drawn = Vec::new();
    for note in notes {
        drawn.push(json!({
            "note": note.note,
            "name": note_name(note.note),
            "start": round(note.seconds, 100.0),
            "hold": round(note.hold_seconds, 100.0),
        }));
    }
    Ok(drawn)
}

fn timed_notes(sound: &[f32]) -> Result<Vec<TimedNote>, String> {
    let Some(heard) = pitch::notes_of(sound, LISTENING_RATE) else {
        return Err(String::from(NO_TUNE));
    };
    if heard.is_empty() {
        return Err(String::from(NO_TUNE));
    }
    let mut notes = Vec::new();
    for HeardNote {
        note,
        start_seconds,
        seconds,
    } in heard
    {
        notes.push(TimedNote {
            note,
            seconds: f64::from(start_seconds),
            hold_seconds: f64::from(seconds),
        });
    }
    Ok(notes)
}

fn round(value: f64, by: f64) -> f64 {
    (value * by).round() / by
}

// Keeps the hum as a WAV file, to be copied beside the chat it goes with.
fn keep_recording(sound: &[f32]) -> Result<PathBuf, String> {
    let folder = std::env::temp_dir().join("jello-hums");
    fs::create_dir_all(&folder).map_err(|error| format!("the hum couldn't be kept: {error}"))?;
    let mut stamp = 0;
    if let Ok(since) = SystemTime::now().duration_since(UNIX_EPOCH) {
        stamp = since.as_millis();
    }
    let path = folder.join(format!("Hum {stamp}.wav"));
    write_wav(&path, sound)?;
    Ok(path)
}

fn write_wav(path: &Path, sound: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: LISTENING_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|error| format!("the hum couldn't be kept: {error}"))?;
    for sample in sound {
        let level = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        writer.write_sample(level).map_err(|error| format!("the hum couldn't be kept: {error}"))?;
    }
    writer.finalize().map_err(|error| format!("the hum couldn't be kept: {error}"))
}

// What the agent reads about the hum: how long, the tempo it was hummed at
// or a guess, the key's guess, and each note's start and hold in beats and
// seconds from the first beat after the count in.
fn reading_of(sound: &[f32], humming: &Humming, path: &Path) -> Result<Attachment, String> {
    let notes = timed_notes(sound)?;
    let seconds = sound.len() as f64 / f64::from(LISTENING_RATE);

    let mut lines = Vec::new();
    let mut tempo = None;
    if let Some(set) = humming.tempo {
        tempo = Some(f64::from(set));
        let mut count_in = format!("after a count in of {} bars", humming.count_in_bars);
        if humming.count_in_bars == 1 {
            count_in = String::from("after a count in of 1 bar");
        }
        if humming.count_in_bars == 0 {
            count_in = String::from("with no count in");
        }
        lines.push(format!("A tune the player hummed, {seconds:.1} s, at {set} BPM on the click, {count_in}."));
    } else if let Some(guess) = audio_file::tempo_of(sound, LISTENING_RATE) {
        tempo = Some(f64::from(guess.round()));
        lines.push(format!(
            "A tune the player hummed, {seconds:.1} s, at a free tempo; it moves at about {} BPM (a guess).",
            guess.round()
        ));
    } else {
        lines.push(format!("A tune the player hummed, {seconds:.1} s, at a free tempo, too loose to guess."));
    }
    if let Some((key, sureness)) = audio_file::key_of(sound, LISTENING_RATE) {
        lines.push(format!("Key guess: {} (sureness {:.2}).", key_name(key), sureness));
    }

    // With a set tempo, time counts from the end of the count in, beat 0;
    // with a free one, from the first note.
    let mut origin = notes[0].seconds;
    if humming.tempo.is_some() {
        origin = 0.0;
    }
    let mut lowest = notes[0].note;
    let mut highest = notes[0].note;
    let mut heard = Vec::new();
    for note in &notes {
        lowest = lowest.min(note.note);
        highest = highest.max(note.note);
        let start = note.seconds - origin;
        let mut line = format!("{} at {:.2} s for {:.2} s", note_name(note.note), start, note.hold_seconds);
        if let Some(tempo) = tempo {
            let beats_per_second = tempo / 60.0;
            line = format!(
                "{line}: beat {:.2}, {:.2} beats",
                start * beats_per_second,
                note.hold_seconds * beats_per_second
            );
        }
        heard.push(line);
    }
    lines.push(format!(
        "{} notes, {} to {}, oldest first:",
        notes.len(),
        note_name(lowest),
        note_name(highest)
    ));
    lines.push(heard.join("\n"));

    Ok(Attachment {
        id: 0,
        name: String::from("Hummed tune"),
        path: path.to_path_buf(),
        kind: "hum",
        text: lines.join("\n"),
        pictures: Vec::new(),
        taken: format!("{} notes, {seconds:.0} s", notes.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn hummed(notes: &[(f32, f32)]) -> Vec<f32> {
        let mut sound = Vec::new();
        for (hz, seconds) in notes {
            let frames = (seconds * LISTENING_RATE as f32) as usize;
            for frame in 0..frames {
                let time = frame as f32 / LISTENING_RATE as f32;
                let wave = (2.0 * PI * hz * time).sin() + 0.3 * (4.0 * PI * hz * time).sin();
                sound.push(0.3 * wave);
            }
            sound.extend(vec![0.0; LISTENING_RATE as usize / 20]);
        }
        sound
    }

    #[test]
    fn a_hum_on_the_click_is_read_in_beats() {
        // C, E, G, half a second each at 120 BPM: a beat each.
        let sound = hummed(&[(261.63, 0.45), (329.63, 0.45), (392.0, 0.45)]);
        let now = Instant::now();
        let humming = Humming {
            tempo: Some(120),
            count_in_bars: 1,
            starts: now,
            started: now,
        };
        let attachment = reading_of(&sound, &humming, Path::new("Hum.wav")).unwrap();
        assert_eq!(attachment.kind, "hum");
        let text = &attachment.text;
        assert!(text.contains("at 120 BPM on the click, after a count in of 1 bar."), "{text}");
        assert!(text.contains("Key guess: C major"), "{text}");
        assert!(text.contains("3 notes, C4 to G4"), "{text}");
        // Heard in looks of 32 ms, so a little early.
        assert!(text.contains("E4 at 0.48 s for 0.48 s: beat 0.96, 0.96 beats"), "{text}");
        assert_eq!(attachment.taken, "3 notes, 2 s");
    }

    #[test]
    fn noise_is_not_a_tune() {
        let mut seed = 7_u32;
        let mut noise = Vec::new();
        for _ in 0..LISTENING_RATE {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            noise.push((seed >> 8) as f32 / (1 << 24) as f32 - 0.5);
        }
        let now = Instant::now();
        let humming = Humming {
            tempo: None,
            count_in_bars: 0,
            starts: now,
            started: now,
        };
        let refused = reading_of(&noise, &humming, Path::new("Hum.wav"));
        assert_eq!(refused.err(), Some(String::from(NO_TUNE)));
    }

    #[test]
    fn a_hum_is_kept_as_a_wav() {
        let sound = hummed(&[(440.0, 0.3)]);
        let path = keep_recording(&sound).unwrap();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, LISTENING_RATE);
        assert_eq!(reader.len() as usize, sound.len());
        fs::remove_file(path).unwrap();
    }
}
