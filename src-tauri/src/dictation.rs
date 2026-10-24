// Dictation (docs/design.md#voice): the player speaks and their words appear
// in the agent's field as they are heard. whisper.cpp, through whisper-rs,
// turns the speech into words on this computer; the sound is never sent.
// Its model downloads once, when the player asks for it.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::commands;
use crate::engine::Engine;
use crate::mic::{self, Heard, LISTENING_RATE, Mic};

// The speech model: whisper's English base model, about 148 MB, and what its
// bytes must hash to, from Hugging Face's own record of the file.
const MODEL_NAME: &str = "ggml-base.en.bin";
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";
const MODEL_BYTES: u64 = 147_964_211;
const MODEL_SHA256: &str = "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002";
// How often the download's progress is told.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
// How often the words so far are worked out again while listening.
const WORDS_EVERY: Duration = Duration::from_millis(1000);
// The longest dictation, in seconds; the mic keeps no more.
const MOST_SECONDS: u32 = 60;
// Quieter than this all the way through, and nothing was said.
const QUIETEST_SPEECH: f32 = 0.01;
// Words that help it hear music talk.
const HINT: &str = "Jello, chords, melody, bass line, verse, chorus, C major, A minor, tempo, BPM, bars, beat, synth, filter, LFO.";

// Where the model lives: the whisper folder in Jello's settings.
fn model_path(settings: &Path) -> PathBuf {
    settings.join("whisper").join(MODEL_NAME)
}

// The model once loaded, kept for every dictation after; the words being
// worked out while the player speaks; and whether a download is going.
#[derive(Default)]
pub struct Dictation {
    model: Option<Arc<WhisperContext>>,
    listening: Option<Speaking>,
    downloading: bool,
}

struct Speaking {
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
    model: Arc<WhisperContext>,
}

// Whether dictation is ready, or its model must download first.
#[tauri::command]
pub fn dictation_status(app: AppHandle, dictation: State<'_, Mutex<Dictation>>) -> Value {
    let dictation = dictation.lock().unwrap();
    let mut ready = false;
    if let Ok(settings) = commands::settings_folder(&app) {
        ready = model_path(&settings).is_file();
    }
    json!({
        "ready": ready,
        "downloading": dictation.downloading,
        "megabytes": MODEL_BYTES / 1_000_000,
    })
}

// Downloads the model, telling the screen how far it has got, and checks it
// is the very file Hugging Face lists before keeping it.
#[tauri::command(async)]
pub fn download_dictation_model(app: AppHandle) -> Result<(), String> {
    let settings = commands::settings_folder(&app)?;
    let path = model_path(&settings);
    if path.is_file() {
        return Ok(());
    }
    {
        let dictation = app.state::<Mutex<Dictation>>();
        let mut dictation = dictation.lock().unwrap();
        if dictation.downloading {
            return Err(String::from("the speech model is already downloading"));
        }
        dictation.downloading = true;
    }
    let result = download(&app, &path);
    let dictation = app.state::<Mutex<Dictation>>();
    dictation.lock().unwrap().downloading = false;
    result
}

fn download(app: &AppHandle, path: &Path) -> Result<(), String> {
    let folder = path.parent().unwrap();
    fs::create_dir_all(folder).map_err(|error| format!("the whisper folder couldn't be made: {error}"))?;
    let part = path.with_extension("part");

    // curl comes with macOS and Windows.
    let mut curl = Command::new("curl")
        .args(["--location", "--fail", "--silent", "--show-error", "--output"])
        .arg(&part)
        .arg(MODEL_URL)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("the download couldn't start: {error}"))?;
    let mut told = Instant::now();
    let status = loop {
        if let Some(status) = curl.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if told.elapsed() >= PROGRESS_EVERY {
            let received = fs::metadata(&part).map(|file| file.len()).unwrap_or(0);
            let _ = app.emit("dictation-download", json!({ "received": received, "total": MODEL_BYTES }));
            told = Instant::now();
        }
        thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        let mut said = String::new();
        if let Some(mut errors) = curl.stderr.take() {
            let _ = errors.read_to_string(&mut said);
        }
        let _ = fs::remove_file(&part);
        return Err(format!("the speech model couldn't download: {}", said.trim()));
    }

    let bytes = fs::read(&part).map_err(|error| format!("the speech model couldn't be read: {error}"))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    if hash != MODEL_SHA256 {
        let _ = fs::remove_file(&part);
        return Err(String::from("the speech model that came isn't the right file, so it wasn't kept"));
    }
    fs::rename(&part, path).map_err(|error| format!("the speech model couldn't be kept: {error}"))?;
    let _ = app.emit("dictation-download", json!({ "received": MODEL_BYTES, "total": MODEL_BYTES }));
    Ok(())
}

fn load_model(path: &Path) -> Result<WhisperContext, String> {
    whisper_rs::install_logging_hooks();
    let Some(path) = path.to_str() else {
        return Err(String::from("the speech model's folder has a name it can't read"));
    };
    WhisperContext::new_with_params(path, WhisperContextParameters::default())
        .map_err(|error| format!("the speech model couldn't be loaded: {error}"))
}

// The words said in `speech`, mono at LISTENING_RATE, or nothing when it
// was all quiet.
pub fn words_of(model: &WhisperContext, speech: &[f32]) -> Result<String, String> {
    let mut loudest = 0.0_f32;
    for sample in speech {
        loudest = loudest.max(sample.abs());
    }
    if loudest < QUIETEST_SPEECH {
        return Ok(String::new());
    }
    // Whisper wants at least a second.
    let mut padded = speech.to_vec();
    if padded.len() < LISTENING_RATE as usize {
        padded.resize(LISTENING_RATE as usize + LISTENING_RATE as usize / 10, 0.0);
    }

    let mut state = model.create_state().map_err(|error| error.to_string())?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_initial_prompt(HINT);
    params.set_no_timestamps(true);
    params.set_suppress_blank(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let threads = thread::available_parallelism().map(|count| count.get()).unwrap_or(4);
    params.set_n_threads(threads.min(8) as i32);
    state.full(params, &padded).map_err(|error| format!("the words couldn't be heard: {error}"))?;

    let mut words = String::new();
    for segment in state.as_iter() {
        let Ok(text) = segment.to_str_lossy() else {
            continue;
        };
        let text = text.trim();
        // Whisper marks silence and noises in brackets, like [BLANK_AUDIO].
        if text.starts_with('[') || text.starts_with('(') {
            continue;
        }
        if !words.is_empty() {
            words.push(' ');
        }
        words.push_str(text);
    }
    Ok(words)
}

// Starts listening to mic `input` (None for the system's pick) and telling
// the screen the words heard so far, about once a second.
#[tauri::command(async)]
pub fn start_dictation(app: AppHandle, input: Option<usize>) -> Result<(), String> {
    let settings = commands::settings_folder(&app)?;
    let path = model_path(&settings);
    if !path.is_file() {
        return Err(String::from("dictation needs its speech model first"));
    }

    // The model loads the first time, without holding the others up.
    let dictation = app.state::<Mutex<Dictation>>();
    let kept = dictation.lock().unwrap().model.clone();
    let model = match kept {
        Some(model) => model,
        None => Arc::new(load_model(&path)?),
    };

    let mut dictation = dictation.lock().unwrap();
    dictation.model = Some(model.clone());
    if dictation.listening.is_some() {
        return Err(String::from("dictation is already listening"));
    }
    let heard = {
        let mic = app.state::<Mutex<Mic>>();
        let mut mic = mic.lock().unwrap();
        mic.start(&app, input)?
    };
    let rate = engine_rate(&app);

    let stop = Arc::new(AtomicBool::new(false));
    let handle = app.clone();
    let stopping = stop.clone();
    let working = model.clone();
    let worker = thread::spawn(move || tell_words(handle, stopping, heard, rate, working));
    dictation.listening = Some(Speaking { stop, worker, model });
    Ok(())
}

// Works the words out again from everything heard, about once a second,
// until told to stop.
fn tell_words(app: AppHandle, stop: Arc<AtomicBool>, heard: Heard, rate: u32, model: Arc<WhisperContext>) {
    let most = MOST_SECONDS as usize * rate as usize;
    let mut worked_on = 0;
    loop {
        let started = Instant::now();
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let sound = heard.lock().unwrap().clone();
        if sound.len() >= most {
            let _ = app.emit("dictation-full", json!({ "seconds": MOST_SECONDS }));
            return;
        }
        if sound.len() > worked_on {
            worked_on = sound.len();
            let speech = mic::resampled(&sound, rate, LISTENING_RATE);
            if let Ok(words) = words_of(&model, &speech) {
                if !stop.load(Ordering::SeqCst) {
                    let _ = app.emit("dictation-words", json!({ "words": words }));
                }
            }
        }
        let spent = started.elapsed();
        if spent < WORDS_EVERY {
            thread::sleep(WORDS_EVERY - spent);
        }
    }
}

// Stops listening and gives every word said, worked out once more from all
// of it.
#[tauri::command(async)]
pub fn stop_dictation(app: AppHandle) -> Result<String, String> {
    let speaking = {
        let dictation = app.state::<Mutex<Dictation>>();
        let mut dictation = dictation.lock().unwrap();
        dictation.listening.take()
    };
    let Some(speaking) = speaking else {
        return Ok(String::new());
    };
    speaking.stop.store(true, Ordering::SeqCst);
    let heard = {
        let mic = app.state::<Mutex<Mic>>();
        let mut mic = mic.lock().unwrap();
        mic.stop(&app)
    };
    let _ = speaking.worker.join();
    let Some(heard) = heard else {
        return Ok(String::new());
    };
    let speech = mic::resampled(&heard, engine_rate(&app), LISTENING_RATE);
    words_of(&speaking.model, &speech)
}

fn engine_rate(app: &AppHandle) -> u32 {
    let engine = app.state::<Mutex<Engine>>();
    let engine = engine.lock().unwrap();
    engine.sample_rate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_file;
    use crate::presets::test_folder;

    // Speaks `sentence` with macOS's own voice, at LISTENING_RATE, or None
    // where there is no such voice or no FFmpeg.
    fn spoken(sentence: &str) -> Option<Vec<f32>> {
        let folder = test_folder("dictation-speech");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("said.aiff");
        let said = Command::new("say").arg("-o").arg(&path).arg(sentence).status().ok()?;
        if !said.success() {
            return None;
        }
        audio_file::program("ffmpeg")?;
        let samples = audio_file::samples_of(&path).ok()?;
        let mono = audio_file::mono_of(&samples);
        Some(mic::resampled(&mono, audio_file::RATE, LISTENING_RATE))
    }

    fn installed_model() -> Option<WhisperContext> {
        let home = std::env::var("HOME").ok()?;
        let path = model_path(&PathBuf::from(home).join("Library/Application Support/com.jello.app"));
        if !path.is_file() {
            return None;
        }
        load_model(&path).ok()
    }

    #[test]
    fn spoken_words_are_heard() {
        let Some(model) = installed_model() else {
            return;
        };
        let Some(speech) = spoken("Make the chorus brighter and add a bass line.") else {
            return;
        };
        let words = words_of(&model, &speech).unwrap().to_lowercase();
        assert!(words.contains("chorus"), "{words}");
        assert!(words.contains("bass line"), "{words}");
    }

    #[test]
    fn silence_has_no_words() {
        let Some(model) = installed_model() else {
            return;
        };
        let silence = vec![0.0; LISTENING_RATE as usize * 2];
        assert_eq!(words_of(&model, &silence).unwrap(), "");
    }
}
