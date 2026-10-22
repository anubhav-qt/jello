// The Rust core of Jello. It owns the C++ sound engine and gives the UI named
// commands to call.

mod commands;
mod compose;
mod engine;
mod harmony;
mod key_map;
mod sound_profile;
mod wav;

use std::sync::Mutex;

use compose::Composer;
use engine::Engine;
use key_map::KeyMap;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The live engine, shared by every command. Its speakers open the first
    // time something plays.
    let mut engine = Engine::new().expect("could not create the sound engine");

    // Compose's key, the chords added to the song so far, where the song
    // played last starts, and the song's sound profile. The profile goes to
    // the engine before anything plays, so the first note already plays it.
    let composer = Composer::new();
    composer.sound_profile().send_all(&mut engine);

    tauri::Builder::default()
        .manage(Mutex::new(engine))
        // What each laptop key does, and which note each held key started.
        .manage(Mutex::new(KeyMap::new()))
        .manage(Mutex::new(composer))
        .invoke_handler(tauri::generate_handler![
            commands::play_note,
            commands::release_note,
            commands::release_all_notes,
            commands::switch_wave,
            commands::switch_snap,
            commands::set_layer_volume,
            commands::set_filter,
            commands::set_filter_envelope,
            commands::set_unison,
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
