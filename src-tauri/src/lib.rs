// The Rust core of Jello. It owns the C++ sound engine and gives the UI named
// commands to call.

mod commands;
mod compose;
mod engine;
mod harmony;
mod key_map;
mod modulation;
mod presets;
mod song_file;
mod song_session;
mod songs;
mod sound_file;
mod sound_profile;
mod wav;

use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use tauri::{AppHandle, Manager, RunEvent};

use compose::Composer;
use engine::Engine;
use key_map::KeyMap;
use song_session::{Folders, OpenSong, SAVE_CHECK};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The live engine, shared by every command. Its speakers open as the
    // song open last opens, in setup below, or the first time something
    // plays.
    let mut engine = Engine::new().expect("could not create the sound engine");

    // Compose's key, the chords added to the song so far, where the song
    // played last starts, and the song's tempo and sound profile. Those two
    // go to the engine before anything plays, so the first note and the
    // first beat already play them.
    let composer = Composer::new();
    composer.send_all(&mut engine);

    tauri::Builder::default()
        .manage(Mutex::new(engine))
        // What each laptop key does, and which note each held key started.
        .manage(Mutex::new(KeyMap::new()))
        .manage(Mutex::new(composer))
        // Before the window opens, Jello opens the song open last, or makes
        // a new one on the starting sound (docs/adr/0038-song-files.md). The
        // folders are only known once the app is running. If one can't be
        // found, Jello still opens, with a new song it can't save. Then a
        // thread keeps saving the song a moment after each change.
        .setup(|app| {
            let folders = folders_of(app.handle());
            app.manage(Mutex::new(OpenSong::new(folders)));
            open_last_song(app.handle());
            let handle = app.handle().clone();
            thread::spawn(move || keep_saving(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::play_note,
            commands::release_note,
            commands::release_all_notes,
            commands::switch_kind,
            commands::set_oscillator,
            commands::switch_snap,
            commands::set_layer_volume,
            commands::pause_loop,
            commands::play_loop,
            commands::set_layer_playing,
            commands::set_filter,
            commands::set_filter_envelope,
            commands::set_unison,
            commands::set_loudness_envelope,
            commands::set_key_tracking,
            commands::set_tuning,
            commands::set_lfo,
            commands::set_connection,
            commands::set_vibrato_key_times,
            commands::set_glide_time,
            commands::preset_list,
            commands::load_preset,
            commands::save_preset,
            commands::preset_name_problem,
            commands::mark_starting_preset,
            song_session::song_list,
            song_session::open_song,
            song_session::make_new_song,
            song_session::rename_song,
            song_session::song_name_problem,
            commands::press_key,
            commands::release_key,
            commands::sound_view,
            commands::key_layout,
            commands::compose_view,
            commands::pick_key,
            commands::add_last_chord,
            commands::remove_last_chord,
            commands::play_chord_in_song,
            commands::play_song,
            commands::stop_song,
            commands::render_test_note,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // The song is saved one last time as Jello quits.
            if let RunEvent::Exit = event {
                save_now(app);
            }
        });
}

// Where songs, presets and settings are kept, or None if one of them can't
// be found.
fn folders_of(app: &AppHandle) -> Option<Folders> {
    let songs = commands::songs_folder(app).ok()?;
    let sounds = commands::sounds_folder(app).ok()?;
    let settings = commands::settings_folder(app).ok()?;
    Some(Folders {
        songs,
        sounds,
        settings,
    })
}

// Opens the song open last, or makes a new one. The speakers open first, so
// the song's loop reaches the engine at once and waits there, paused.
fn open_last_song(app: &AppHandle) {
    let now = Instant::now();
    let open = app.state::<Mutex<OpenSong>>();
    let composer = app.state::<Mutex<Composer>>();
    let engine = app.state::<Mutex<Engine>>();
    let mut open = open.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    let _ = engine.start_audio();
    song_session::open_last_song(&mut open, &mut composer, &mut engine, now);
}

// Looks for changes to the song a few times a second, for as long as Jello
// runs, and saves each once it has settled.
fn keep_saving(app: AppHandle) {
    loop {
        thread::sleep(SAVE_CHECK);
        let now = Instant::now();
        let open = app.state::<Mutex<OpenSong>>();
        let composer = app.state::<Mutex<Composer>>();
        let engine = app.state::<Mutex<Engine>>();
        let mut open = open.lock().unwrap();
        let composer = composer.lock().unwrap();
        let mut engine = engine.lock().unwrap();
        song_session::save_if_settled(&mut open, &composer, &mut engine, now);
    }
}

// Saves the song now if it changed since it was saved last.
fn save_now(app: &AppHandle) {
    let open = app.state::<Mutex<OpenSong>>();
    let composer = app.state::<Mutex<Composer>>();
    let engine = app.state::<Mutex<Engine>>();
    let mut open = open.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    let _ = song_session::save_now(&mut open, &composer, &mut engine);
}
