// The Rust core of Jello. For now it only opens the window; commands for the
// engine get added here once the C++ engine design is agreed.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
