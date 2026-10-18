// Named commands the UI can call (docs/adr/0003-every-action-is-a-command.md).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::engine::Engine;
use crate::wav::write_wav;

// The test tone file plays for 2 seconds, then keeps a short quiet tail so
// the fade-out is in the file too and it ends without a click.
const TEST_TONE_MS: u32 = 2000;
const QUIET_TAIL_MS: u32 = 50;

#[tauri::command]
pub fn start_tone(engine: State<'_, Mutex<Engine>>) -> Result<(), String> {
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    engine.set_tone(true);
    Ok(())
}

#[tauri::command]
pub fn stop_tone(engine: State<'_, Mutex<Engine>>) {
    let mut engine = engine.lock().unwrap();
    engine.set_tone(false);
}

// Renders the test tone to Music/Jello/test-tone.wav and returns where it went.
#[tauri::command]
pub fn render_test_tone(app: AppHandle) -> Result<String, String> {
    // A second engine with no speakers, so the file never disturbs what is
    // playing live. It runs the same code the speakers hear.
    let mut engine = Engine::new()?;
    let sample_rate = engine.sample_rate();

    engine.set_tone(true);
    let mut samples = engine.render(frames_in_ms(sample_rate, TEST_TONE_MS));

    engine.set_tone(false);
    let tail = engine.render(frames_in_ms(sample_rate, QUIET_TAIL_MS));
    samples.extend(tail);

    let path = jello_music_folder(&app)?.join("test-tone.wav");
    write_wav(&path, &samples, sample_rate)?;

    Ok(path.display().to_string())
}

fn frames_in_ms(sample_rate: u32, ms: u32) -> usize {
    let frames = sample_rate * ms / 1000;
    frames as usize
}

// The Jello folder inside the user's Music folder, made if it is missing.
fn jello_music_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let music = app.path().audio_dir().map_err(|error| error.to_string())?;
    let folder = music.join("Jello");
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}
