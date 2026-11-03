// The Rust core of Jello. It owns the C++ sound engine and gives the UI named
// commands to call.

mod agent;
mod arrange;
mod attachments;
mod automation;
mod audio_file;
mod cards;
mod guitar;
mod commands;
mod compose;
mod dictation;
mod drum_changes;
mod drums;
mod effects;
mod engine;
mod harmony;
mod hum;
mod instruments;
mod key_map;
mod key_mapping;
mod key_optimizer;
mod key_stats;
mod keys_tool;
mod layer_cycles;
mod layer_notes;
mod listening;
mod loudness;
mod measures;
mod memory;
mod mic;
mod mix;
mod mix_automation;
mod mix_commands;
mod mix_file;
mod mix_tool;
mod modulation;
mod phrases;
mod pitch;
mod practice;
mod practice_run;
mod presets;
mod scales;
mod scoring;
mod sections;
mod skills;
mod song_file;
mod song_session;
mod songs;
mod sound_changes;
mod sound_file;
mod sound_profile;
mod starters;
mod theory;
mod tool_server;
mod tools;
mod tracks;
mod undo;
mod wav;

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, RunEvent};

use compose::Composer;
use engine::Engine;
use key_map::KeyMap;
use song_session::{Folders, OpenSong, SAVE_CHECK};

// How often the sections playing are kept ahead of the sound.
const ARRANGE_CHECK: Duration = Duration::from_millis(50);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The live engine, shared by every command. Its speakers open as the
    // song open last opens, in setup below, or the first time something
    // plays.
    let mut engine = Engine::new().expect("could not create the sound engine");
    // Its instruments are read on another thread, so picking one never
    // holds up the keys (docs/design.md#sampler).
    engine.load_in_background();

    // Compose's key, the chords added to the song so far, where the song
    // played last starts, and the song's tempo and sound profile. Those two
    // go to the engine before anything plays, so the first note and the
    // first beat already play them.
    let composer = Composer::new();
    composer.send_all(&mut engine);

    tauri::Builder::default()
        // The paperclip's file picker.
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(engine))
        .manage(agent::AgentHost::default())
        .manage(Mutex::new(tool_server::Questions::default()))
        .manage(phrases::Phrases::default())
        .manage(Mutex::new(undo::Replies::default()))
        .manage(Mutex::new(cards::Offers::default()))
        .manage(Mutex::new(practice::Practice::default()))
        .manage(Mutex::new(attachments::Attachments::default()))
        .manage(Mutex::new(mic::Mic::default()))
        .manage(Mutex::new(dictation::Dictation::default()))
        .manage(Mutex::new(hum::Hum::default()))
        .manage(Mutex::new(starters::Starters::default()))
        // What each laptop key does, and which note each held key started.
        .manage(Mutex::new(KeyMap::new()))
        // The player's own timing between keys, and the optimizer's offers
        // (docs/adr/0047-song-key-mappings.md).
        .manage(Mutex::new(key_stats::KeyStats::default()))
        .manage(Mutex::new(keys_tool::KeyOffers::default()))
        .manage(Mutex::new(composer))
        // Before the window opens, Jello opens the song open last, or makes
        // a new one on the starting sound (docs/adr/0038-song-files.md). The
        // folders are only known once the app is running. If one can't be
        // found, Jello still opens, with a new song it can't save. Then a
        // thread keeps saving the song a moment after each change.
        .setup(|app| {
            let folders = folders_of(app.handle());
            app.manage(Mutex::new(OpenSong::new(folders)));
            // Instruments come from Music/Jello/Instruments. Each one read
            // in the background starts playing once it is ready.
            if let Ok(folder) = commands::instruments_folder(app.handle()) {
                instruments::set_folder(folder);
            }
            // The guitars need no download, so their presets are always there.
            if let Ok(folder) = commands::sounds_folder(app.handle()) {
                let _ = guitar::save_presets(&folder);
            }
            let loaded = app.handle().clone();
            instruments::on_loaded(move || play_loaded_instruments(&loaded));
            // The agent's notes about the player, from Music/Jello/Memory.
            let memory = memory::Memory::load(memory::folder_of(app.handle()));
            app.manage(Mutex::new(memory));
            open_last_song(app.handle());
            keys_tool::start_keys(app.handle());
            // The agent's tools reach the open song, so their server starts
            // once it is kept. Without it Jello still opens, with no agent.
            let tool_server = tool_server::start(app.handle()).ok();
            app.manage(tool_server);
            let handle = app.handle().clone();
            thread::spawn(move || keep_saving(handle));
            let arranging = app.handle().clone();
            thread::spawn(move || keep_arranging(arranging));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            agent::agent_status,
            agent::agent_say,
            agent::stop_agent,
            agent::agent_new_chat,
            agent::agent_chats,
            agent::agent_open_chat,
            agent::agent_pick_model,
            agent::agent_sign_in,
            agent::agent_sign_in_answer,
            agent::agent_cancel_sign_in,
            tool_server::answer_agent,
            cards::play_agent_option,
            cards::use_agent_option,
            cards::practice_agent_option,
            practice::practice_view,
            practice::start_practice,
            practice::stop_practice,
            practice::practice_settings,
            practice::close_practice,
            memory::memory_view,
            memory::forget_memory,
            memory::set_memory_ask_first,
            attachments::agent_attach,
            attachments::agent_unattach,
            mic::mic_inputs,
            dictation::dictation_status,
            dictation::download_dictation_model,
            dictation::start_dictation,
            dictation::stop_dictation,
            hum::start_hum,
            hum::stop_hum,
            hum::cancel_hum,
            undo::agent_reply_starts,
            undo::undo_agent_reply,
            commands::play_note,
            commands::release_note,
            commands::release_all_notes,
            commands::switch_kind,
            commands::set_oscillator,
            commands::set_instrument,
            commands::set_drum_kit,
            commands::pick_track,
            commands::rename_track,
            commands::set_track_volume,
            commands::remove_track,
            commands::set_drum_pad,
            commands::list_instruments,
            commands::open_instruments_folder,
            starters::download_starter,
            commands::switch_snap,
            commands::set_layer_volume,
            commands::set_layer_bars,
            commands::set_tempo,
            commands::pause_loop,
            commands::play_loop,
            commands::set_layer_playing,
            commands::set_filter,
            commands::set_filter_envelope,
            commands::set_unison,
            commands::set_amp,
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
            keys_tool::keys_tab_changed,
            keys_tool::pick_play_scale,
            keys_tool::pick_song_scale,
            keys_tool::mapping_view,
            keys_tool::set_key_notes,
            keys_tool::set_key_locked,
            keys_tool::set_guitar_strings,
            keys_tool::reset_keys,
            keys_tool::sound_mapped_note,
            keys_tool::try_key_offer,
            keys_tool::stop_trying_keys,
            keys_tool::use_key_offer,
            keys_tool::coming_keys,
            commands::shift_octave,
            commands::edit_composition,
            commands::compose_view,
            commands::pick_key,
            commands::add_last_chord,
            commands::remove_last_chord,
            commands::play_chord_in_song,
            commands::play_song,
            commands::sections_view,
            commands::edit_sections,
            commands::render_wav,
            commands::stop_song,
            mix_commands::mix_view,
            mix_commands::mix_meters,
            mix_commands::set_mix_effect,
            mix_commands::move_mix_effect,
            mix_commands::set_mix_effect_on,
            mix_commands::set_mix_effect_value,
            mix_commands::set_mix_pan,
            mix_commands::set_mix_send,
            mix_commands::set_bus_volume,
            mix_commands::add_bus,
            mix_commands::remove_bus,
            mix_commands::rename_bus,
            mix_commands::set_limiter,
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

// Where songs, presets, settings and renders are kept, or None if one of
// them can't be found.
fn folders_of(app: &AppHandle) -> Option<Folders> {
    let songs = commands::songs_folder(app).ok()?;
    let sounds = commands::sounds_folder(app).ok()?;
    let settings = commands::settings_folder(app).ok()?;
    let renders = commands::renders_folder(app).ok()?;
    Some(Folders {
        songs,
        sounds,
        settings,
        renders,
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

// Plays each instrument asked for that has just been read, and tells the
// screen, which shows whether each is ready.
fn play_loaded_instruments(app: &AppHandle) {
    let engine = app.state::<Mutex<Engine>>();
    let mut engine = engine.lock().unwrap();
    engine.send_loaded_instruments();
    drop(engine);
    let _ = app.emit("instruments-changed", ());
}

// Looks for changes to the song a few times a second, for as long as Jello
// runs, and saves each once it has settled. It frees the instruments no
// note plays any more too.
fn keep_saving(app: AppHandle) {
    loop {
        thread::sleep(SAVE_CHECK);
        let now = Instant::now();
        let open = app.state::<Mutex<OpenSong>>();
        let composer = app.state::<Mutex<Composer>>();
        let engine = app.state::<Mutex<Engine>>();
        let mut open = open.lock().unwrap();
        let mut composer = composer.lock().unwrap();
        let mut engine = engine.lock().unwrap();
        // A layer removed with Esc leaves the sections before they are saved.
        sections::catch_up(&mut composer, &mut engine);
        song_session::save_if_settled(&mut open, &composer, &mut engine, now);
        keys_tool::save_key_stats(&app, &open);
        engine.tidy_sample_sets();
    }
}

// Keeps the sections playing (src/arrange.rs): every ARRANGE_CHECK, sends
// the sequencer what is due soon, tells the page each time another section
// starts, and once they have played.
fn keep_arranging(app: AppHandle) {
    let mut playing_before = None;
    loop {
        thread::sleep(ARRANGE_CHECK);
        let key_map = app.state::<Mutex<KeyMap>>();
        let composer = app.state::<Mutex<Composer>>();
        let engine = app.state::<Mutex<Engine>>();
        let mut key_map = key_map.lock().unwrap();
        let mut composer = composer.lock().unwrap();
        let mut engine = engine.lock().unwrap();
        let stopped = arrange::keep_playing(&mut composer, &mut engine);
        let playing = arrange::playing_now(&composer, &engine);
        // The section's keys take over at its boundary. Held notes keep
        // sounding, since the key map remembers what each key started.
        let mut section = None;
        if let Some(place) = playing {
            section = composer.arrangement().get(place).copied();
        }
        composer.set_section_playing(section);
        key_map.follow(composer.keys_now());
        drop(engine);
        drop(composer);
        drop(key_map);
        if stopped {
            let _ = app.emit("arrangement-stopped", ());
        } else if playing != playing_before && playing.is_some() {
            let _ = app.emit("section-playing", ());
        }
        playing_before = playing;
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
