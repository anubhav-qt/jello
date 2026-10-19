// The Rust core of Jello. It owns the C++ sound engine and gives the UI named
// commands to call.

mod commands;
mod engine;
mod wav;

use std::sync::Mutex;

use engine::Engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The live engine, shared by every command. Its speakers open the first
    // time something plays.
    let engine = Engine::new().expect("could not create the sound engine");

    tauri::Builder::default()
        .manage(Mutex::new(engine))
        .invoke_handler(tauri::generate_handler![
            commands::play_note,
            commands::release_note,
            commands::release_all_notes,
            commands::switch_wave,
            commands::render_test_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
