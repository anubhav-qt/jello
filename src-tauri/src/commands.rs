// Named commands the UI can call (docs/adr/0003-every-action-is-a-command.md).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::engine::Engine;
use crate::wav::write_wav;

// The test file holds middle C at velocity 100 for 2 seconds, then lets go.
// It keeps going for 400 ms after that: the 300 ms fade-out plus a little
// silence, so it ends cleanly.
const MIDDLE_C: u8 = 60;
const TEST_VELOCITY: u8 = 100;
const HOLD_MS: u32 = 2000;
const TAIL_MS: u32 = 400;

// Plays `note` at `velocity` until `release_note` lets go of it. The first
// note opens the speakers, so it may start a little late.
#[tauri::command]
pub fn play_note(engine: State<'_, Mutex<Engine>>, note: u8, velocity: u8) -> Result<(), String> {
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    engine.note_on(note, velocity);
    Ok(())
}

// Lets go of `note`, so it fades out.
#[tauri::command]
pub fn release_note(engine: State<'_, Mutex<Engine>>, note: u8) {
    let mut engine = engine.lock().unwrap();
    // Before the speakers open, no note can be playing and nothing empties the
    // engine's mailbox. Sending anyway would let messages pile up there until
    // it is full, and then the first real note would be dropped.
    if engine.is_audio_running() {
        engine.note_off(note);
    }
}

// Lets go of every note, for when the window loses focus and a held key's
// release would never arrive.
#[tauri::command]
pub fn release_all_notes(engine: State<'_, Mutex<Engine>>) {
    let mut engine = engine.lock().unwrap();
    // Same reason as in `release_note`.
    if engine.is_audio_running() {
        engine.all_notes_off();
    }
}

// Moves notes played from now on to the next wave (sine, triangle, square,
// saw, then sine again) and returns its name. Notes already playing keep
// their wave. Like `play_note`, it opens the speakers first: before they
// open, nothing empties the engine's mailbox, so switches would pile up there.
#[tauri::command]
pub fn switch_wave(engine: State<'_, Mutex<Engine>>) -> Result<String, String> {
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let wave = engine.switch_wave();
    Ok(String::from(wave.name()))
}

// Renders a held middle C to Music/Jello/test-note.wav and returns where it
// went.
#[tauri::command]
pub fn render_test_note(app: AppHandle) -> Result<String, String> {
    // A second engine with no speakers, so the file never disturbs what is
    // playing live. It runs the same code the speakers hear.
    let mut engine = Engine::new()?;
    let sample_rate = engine.sample_rate();

    engine.note_on(MIDDLE_C, TEST_VELOCITY);
    let mut samples = engine.render(frames_in_ms(sample_rate, HOLD_MS));

    engine.note_off(MIDDLE_C);
    let tail = engine.render(frames_in_ms(sample_rate, TAIL_MS));
    samples.extend(tail);

    let path = jello_music_folder(&app)?.join("test-note.wav");
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
