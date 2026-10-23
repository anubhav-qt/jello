// The song open now (docs/adr/0038-song-files.md): its name, saving it a
// moment after each change, and opening, making and renaming songs. Every
// song, even a new one, has a file in the Songs folder from the start.
//
// Saving works by looking, not by being told: a few times a second Jello
// writes the song out as text, and once that text has stayed the same for
// a second and differs from what was saved last, it is saved. So every
// change is caught, whichever command made it, and a chord or loop note
// still held is saved once it is let go.
//
// Commands lock what they need in this order: the open song, the key map,
// Compose, the engine. So two of them can never end up waiting on each
// other.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::commands::load_starting_sound;
use crate::compose::Composer;
use crate::engine::{Engine, LoopLayer, LoopNote, START_TEMPO};
use crate::key_map::LAPTOP_VELOCITY;
use crate::presets;
use crate::song_file::{BeatNote, Song, SongLayer, SongLoop, beats_of, samples_of, text_of};
use crate::songs::{self, SongView};
use crate::sound_profile::SoundProfile;

// How long a change has to stay before it is saved.
const SAVE_AFTER: Duration = Duration::from_secs(1);

// How often Jello looks for changes to save.
pub const SAVE_CHECK: Duration = Duration::from_millis(250);

// Where songs, presets and settings are kept.
#[derive(Clone, Debug)]
pub struct Folders {
    pub songs: PathBuf,
    pub sounds: PathBuf,
    pub settings: PathBuf,
}

pub struct OpenSong {
    // None when the folders couldn't be found: then nothing is saved, and
    // the song can't be renamed or switched.
    folders: Option<Folders>,
    // The song's name: its file's name. Empty until a song is opened or
    // made.
    name: String,
    // The text saved last, so a song that hasn't changed isn't written
    // again.
    saved_text: String,
    // The text seen last that differs from the saved one, and when it was
    // first seen.
    changed: Option<(String, Instant)>,
    // False if the engine refused the song's loop when it opened. Saving
    // would then leave the loop out of the file, so the file is kept as it
    // is until another song opens.
    saves: bool,
}

// What the Songs panel shows: every song, and the one open.
#[derive(Debug, PartialEq, Serialize)]
pub struct SongsView {
    songs: Vec<SongView>,
    open: String,
}

impl OpenSong {
    // No song open yet, with songs kept in `folders`.
    pub fn new(folders: Option<Folders>) -> OpenSong {
        OpenSong {
            folders,
            name: String::new(),
            saved_text: String::new(),
            changed: None,
            saves: true,
        }
    }

    #[cfg(test)]
    pub fn name(&self) -> &str {
        &self.name
    }

    // The folders, or an error saying they couldn't be found.
    fn folders(&self) -> Result<Folders, String> {
        match &self.folders {
            Some(folders) => Ok(folders.clone()),
            None => Err(String::from("the Songs folder couldn't be found")),
        }
    }

    // True while there is a song file to save to.
    fn can_save(&self) -> bool {
        if self.folders.is_none() {
            return false;
        }
        if self.name.is_empty() {
            return false;
        }
        self.saves
    }
}

// The song in `composer` and `engine` as it is now, as its file holds it.
pub fn song_now(composer: &Composer, engine: &mut Engine) -> Song {
    let mut preset = None;
    if let Some(name) = composer.lit_preset() {
        preset = Some(String::from(name));
    }
    Song {
        key: composer.key(),
        tempo: composer.tempo(),
        snap: engine.snap(),
        preset,
        chords: composer.held_chords(),
        looped: loop_now(engine),
        sound: composer.sound_profile().sound(),
    }
}

// The loop as its file holds it, in beats at the tempo it plays at. A note
// still held is left out until it is let go, and so is a layer left with
// no notes.
fn loop_now(engine: &mut Engine) -> Option<SongLoop> {
    let (bars, layers) = engine.loop_to_keep()?;
    let tempo = engine.tempo();
    let sample_rate = engine.sample_rate();

    let mut song_layers = Vec::new();
    for layer in layers {
        let mut notes = Vec::new();
        for played in &layer.notes {
            let Some(end) = played.end else {
                continue;
            };
            notes.push(BeatNote {
                note: played.note,
                start: beats_of(played.start, tempo, sample_rate),
                end: beats_of(end, tempo, sample_rate),
            });
        }
        if notes.is_empty() {
            continue;
        }
        song_layers.push(SongLayer {
            volume: layer.volume,
            playing: layer.playing,
            notes,
        });
    }

    if song_layers.is_empty() {
        return None;
    }
    Some(SongLoop {
        bars,
        layers: song_layers,
    })
}

// Opens `song`'s loop on `engine`, paused, at its tempo, or with no loop,
// forgets the one there is. The beats become samples at the song's tempo.
fn open_loop_of(engine: &mut Engine, song: &Song) -> Result<(), String> {
    let Some(looped) = &song.looped else {
        return engine.open_loop(song.tempo, 0, &[], LAPTOP_VELOCITY);
    };

    let sample_rate = engine.sample_rate();
    let mut layers = Vec::new();
    for layer in &looped.layers {
        let mut notes = Vec::new();
        for note in &layer.notes {
            notes.push(LoopNote {
                note: note.note,
                start: samples_of(note.start, song.tempo, sample_rate),
                end: Some(samples_of(note.end, song.tempo, sample_rate)),
            });
        }
        layers.push(LoopLayer {
            notes,
            volume: layer.volume,
            playing: layer.playing,
            recording: false,
        });
    }
    engine.open_loop(song.tempo, looped.bars, &layers, LAPTOP_VELOCITY)
}

// Saves the open song now if it changed since it was saved last.
pub fn save_now(open: &mut OpenSong, composer: &Composer, engine: &mut Engine) -> Result<(), String> {
    if !open.can_save() {
        return Ok(());
    }
    let text = text_of(&song_now(composer, engine));
    if text == open.saved_text {
        return Ok(());
    }
    let folders = open.folders()?;
    songs::save_text(&folders.songs, &open.name, &text)?;
    open.saved_text = text;
    open.changed = None;
    Ok(())
}

// Looks for a change at `now`, and saves the song once a change has stayed
// for SAVE_AFTER. A save that fails is tried again next time.
pub fn save_if_settled(open: &mut OpenSong, composer: &Composer, engine: &mut Engine, now: Instant) {
    if !open.can_save() {
        return;
    }
    let text = text_of(&song_now(composer, engine));
    if text == open.saved_text {
        open.changed = None;
        return;
    }

    let mut settled = false;
    if let Some((seen, seen_at)) = &open.changed {
        if *seen == text {
            settled = now.saturating_duration_since(*seen_at) >= SAVE_AFTER;
        } else {
            open.changed = Some((text.clone(), now));
        }
    } else {
        open.changed = Some((text.clone(), now));
    }
    if !settled {
        return;
    }

    let Ok(folders) = open.folders() else {
        return;
    };
    if songs::save_text(&folders.songs, &open.name, &text).is_ok() {
        open.saved_text = text;
        open.changed = None;
    }
}

// Opens song `name` at `now`, after saving the one open. Fails, changing
// nothing, if the open song can't be saved or the file can't be read. The
// engine needs its speakers running, so the loop it is sent is taken in.
pub fn open_song_named(
    open: &mut OpenSong,
    composer: &mut Composer,
    engine: &mut Engine,
    name: &str,
    now: Instant,
) -> Result<(), String> {
    let folders = open.folders()?;
    save_now(open, composer, engine)?;
    let song = songs::read(&folders.songs, name, &SoundProfile::new())?;

    engine.stop_song();
    let loop_opened = open_loop_of(engine, &song);
    engine.set_snap(song.snap);
    *composer = Composer::opened(engine, &song, now);
    if let Some(preset) = &song.preset {
        if let Ok(sound) = presets::read(&folders.sounds, preset, &SoundProfile::new()) {
            composer.remember_preset_if_same(preset, &sound);
        }
    }

    open.name = String::from(name);
    open.saved_text = text_of(&song_now(composer, engine));
    open.changed = None;
    open.saves = loop_opened.is_ok();
    let _ = songs::remember_last_song(&folders.settings, name);
    loop_opened
}

// Makes a new song at `now`, after saving the one open: "Untitled 1" or
// the next free number, with no key, no chords and no loop, at a new
// song's tempo, on the starting sound. Its file is written at once.
pub fn new_song(open: &mut OpenSong, composer: &mut Composer, engine: &mut Engine) -> Result<(), String> {
    let folders = open.folders()?;
    save_now(open, composer, engine)?;
    let name = songs::new_name(&folders.songs);

    engine.stop_song();
    engine.open_loop(START_TEMPO, 0, &[], LAPTOP_VELOCITY)?;
    engine.set_snap(true);
    *composer = Composer::new();
    composer.send_all(engine);
    load_starting_sound(&folders.sounds, &folders.settings, composer, engine);

    open.name = name;
    open.saved_text = String::new();
    open.changed = None;
    open.saves = true;
    save_now(open, composer, engine)?;
    let _ = songs::remember_last_song(&folders.settings, &open.name);
    Ok(())
}

// Opens the song open last, or if it is gone or can't be read, makes a new
// one. Jello does this as it starts.
pub fn open_last_song(open: &mut OpenSong, composer: &mut Composer, engine: &mut Engine, now: Instant) {
    let Ok(folders) = open.folders() else {
        return;
    };
    if let Some(name) = songs::last_song(&folders.settings) {
        if open_song_named(open, composer, engine, &name, now).is_ok() {
            return;
        }
    }
    let _ = new_song(open, composer, engine);
}

// Renames the open song to `to`, after saving it. Fails for a name
// songs::rename_problem refuses.
pub fn rename_open_song(
    open: &mut OpenSong,
    composer: &Composer,
    engine: &mut Engine,
    to: &str,
) -> Result<(), String> {
    let folders = open.folders()?;
    save_now(open, composer, engine)?;
    let renamed = songs::rename(&folders.songs, &open.name, to)?;
    open.name = renamed;
    let _ = songs::remember_last_song(&folders.settings, &open.name);
    Ok(())
}

// Every song and the one open.
fn songs_view_of(open: &OpenSong) -> SongsView {
    let mut songs = Vec::new();
    if let Some(folders) = &open.folders {
        songs = songs::list(&folders.songs);
    }
    SongsView {
        songs,
        open: open.name.clone(),
    }
}

// Every song in the Songs folder, sorted by name, and the one open. The
// Songs panel asks each time it opens, so songs added, renamed or removed
// in the folder show up.
#[tauri::command]
pub fn song_list(open: State<'_, Mutex<OpenSong>>) -> SongsView {
    let open = open.lock().unwrap();
    songs_view_of(&open)
}

// Opens song `name`, after saving the one open. The loop comes in paused.
#[tauri::command]
pub fn open_song(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    name: String,
) -> Result<SongsView, String> {
    let now = Instant::now();
    let mut open = open.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    open_song_named(&mut open, &mut composer, &mut engine, &name, now)?;
    Ok(songs_view_of(&open))
}

// Makes a new song, after saving the one open.
#[tauri::command]
pub fn make_new_song(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
) -> Result<SongsView, String> {
    let mut open = open.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    new_song(&mut open, &mut composer, &mut engine)?;
    Ok(songs_view_of(&open))
}

// Renames the open song to `name`.
#[tauri::command]
pub fn rename_song(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    name: String,
) -> Result<SongsView, String> {
    let mut open = open.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    rename_open_song(&mut open, &composer, &mut engine, &name)?;
    Ok(songs_view_of(&open))
}

// Why the open song can't be renamed `name`, for the hint beside the name
// field, or None (null for the UI) if it can.
#[tauri::command]
pub fn song_name_problem(open: State<'_, Mutex<OpenSong>>, name: String) -> Option<String> {
    let open = open.lock().unwrap();
    let Ok(folders) = open.folders() else {
        return Some(String::from("The Songs folder couldn't be found"));
    };
    songs::rename_problem(&folders.songs, &open.name, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::{FULL_VOLUME, GLIDE_KEY_NOTE, Kind, LooperState, VIBRATO_KEY_NOTE};
    use crate::harmony::{Chord, Key, Mode, Quality};
    use crate::presets::{mark_starting, test_folder};
    use crate::song_file::HeldChord;
    use crate::sound_file::Sound;

    // A4 and C5 by their MIDI numbers.
    const A4: u8 = 69;
    const C5: u8 = 72;

    // Folders of their own for one test.
    fn folders_for(label: &str) -> Folders {
        Folders {
            songs: test_folder(&format!("{label}-songs")),
            sounds: test_folder(&format!("{label}-sounds")),
            settings: test_folder(&format!("{label}-settings")),
        }
    }

    // No song open yet, a new song's Compose and an engine, with songs kept
    // in folders of their own.
    fn starting(label: &str) -> (OpenSong, Composer, Engine) {
        let open = OpenSong::new(Some(folders_for(label)));
        (open, Composer::new(), Engine::new().unwrap())
    }

    fn a_minor() -> Key {
        Key {
            tonic: 9,
            mode: Mode::Minor,
        }
    }

    fn am() -> Chord {
        Chord {
            root: 57,
            quality: Quality::Minor,
        }
    }

    // A new song's sound with its first oscillator on a saw.
    fn saw_sound() -> Sound {
        let mut sound = SoundProfile::new().sound();
        sound.oscillators[0].kind = Kind::Saw;
        sound
    }

    fn note(note: u8, start: f64, end: f64) -> BeatNote {
        BeatNote { note, start, end }
    }

    // A song with every part moved away from a new song's, at 100 beats per
    // minute, where a beat is 28,800 samples, so its beats are whole
    // samples.
    fn night_drive() -> Song {
        let mut sound = saw_sound();
        sound.filter.cutoff_hz = 1234.5;
        let first = SongLayer {
            volume: 80,
            playing: true,
            notes: vec![note(A4, 0.0, 0.5), note(VIBRATO_KEY_NOTE, 1.0, 2.0)],
        };
        let second = SongLayer {
            volume: 40,
            playing: false,
            notes: vec![note(C5, 4.25, 7.5), note(GLIDE_KEY_NOTE, 6.0, 6.5)],
        };
        Song {
            key: Some(a_minor()),
            tempo: 100,
            snap: false,
            preset: None,
            chords: vec![
                HeldChord {
                    chord: am(),
                    held_ms: 1250,
                },
                HeldChord {
                    chord: Chord {
                        root: 53,
                        quality: Quality::Major,
                    },
                    held_ms: 400,
                },
            ],
            looped: Some(SongLoop {
                bars: 2,
                layers: vec![first, second],
            }),
            sound,
        }
    }

    // Writes `song` into the Songs folder as `name`.
    fn put(open: &OpenSong, name: &str, song: &Song) {
        let folders = open.folders().unwrap();
        songs::save_text(&folders.songs, name, &text_of(song)).unwrap();
    }

    // Song `name` as its file holds it now.
    fn file_of(open: &OpenSong, name: &str) -> Song {
        let folders = open.folders().unwrap();
        songs::read(&folders.songs, name, &SoundProfile::new()).unwrap()
    }

    fn last_song_of(open: &OpenSong) -> Option<String> {
        songs::last_song(&open.folders().unwrap().settings)
    }

    #[test]
    fn a_new_song_is_saved_at_once_as_the_next_untitled() {
        let (mut open, mut composer, mut engine) = starting("session-new");
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        assert_eq!(open.name(), "Untitled 1");
        let song = file_of(&open, "Untitled 1");
        assert_eq!(song.tempo, START_TEMPO);
        assert_eq!(song.key, None);
        assert!(song.chords.is_empty());
        assert!(song.looped.is_none());
        assert!(song.snap);
        assert_eq!(last_song_of(&open), Some(String::from("Untitled 1")));

        new_song(&mut open, &mut composer, &mut engine).unwrap();
        assert_eq!(open.name(), "Untitled 2");
        assert_eq!(last_song_of(&open), Some(String::from("Untitled 2")));
    }

    #[test]
    fn a_new_song_starts_on_the_starting_sound() {
        let (mut open, mut composer, mut engine) = starting("session-starting");
        let folders = open.folders().unwrap();
        presets::save(&folders.sounds, "Saw", &saw_sound()).unwrap();
        mark_starting(&folders.settings, Some("Saw")).unwrap();

        new_song(&mut open, &mut composer, &mut engine).unwrap();
        assert_eq!(composer.sound_profile().sound(), saw_sound());
        assert_eq!(composer.lit_preset(), Some("Saw"));
        let song = file_of(&open, "Untitled 1");
        assert_eq!(song.sound, saw_sound());
        assert_eq!(song.preset, Some(String::from("Saw")));
    }

    #[test]
    fn a_change_is_saved_once_it_has_stayed_for_a_second() {
        let (mut open, mut composer, mut engine) = starting("session-settle");
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        let start = Instant::now();

        composer.pick_key(a_minor());
        save_if_settled(&mut open, &composer, &mut engine, start);
        let half_a_second = start + Duration::from_millis(500);
        save_if_settled(&mut open, &composer, &mut engine, half_a_second);
        assert_eq!(file_of(&open, "Untitled 1").key, None);

        let a_second = start + SAVE_AFTER;
        save_if_settled(&mut open, &composer, &mut engine, a_second);
        assert_eq!(file_of(&open, "Untitled 1").key, Some(a_minor()));
    }

    #[test]
    fn another_change_waits_a_second_again() {
        let (mut open, mut composer, mut engine) = starting("session-again");
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        // The engine takes in the new song, so the tempo can change.
        engine.render(64);
        let start = Instant::now();

        composer.pick_key(a_minor());
        save_if_settled(&mut open, &composer, &mut engine, start);
        composer.speed_up(&mut engine);
        let later = start + Duration::from_millis(800);
        save_if_settled(&mut open, &composer, &mut engine, later);
        let a_second_from_start = start + SAVE_AFTER;
        save_if_settled(&mut open, &composer, &mut engine, a_second_from_start);
        assert_eq!(file_of(&open, "Untitled 1").key, None);

        save_if_settled(&mut open, &composer, &mut engine, later + SAVE_AFTER);
        let song = file_of(&open, "Untitled 1");
        assert_eq!(song.key, Some(a_minor()));
        assert_eq!(song.tempo, START_TEMPO + 5);
    }

    #[test]
    fn an_opened_song_is_exactly_as_it_was_left() {
        let (mut open, mut composer, mut engine) = starting("session-reopen");
        let song = night_drive();
        put(&open, "Night drive", &song);

        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", Instant::now()).unwrap();
        assert_eq!(open.name(), "Night drive");
        // The loop counts as opened before the engine takes it in, and after.
        assert_eq!(song_now(&composer, &mut engine), song);
        engine.render(64);
        assert_eq!(engine.looper_status().state, crate::engine::LooperState::Paused);
        assert_eq!(engine.tempo(), 100);
        assert!(!engine.snap());
        assert_eq!(song_now(&composer, &mut engine), song);
        assert_eq!(last_song_of(&open), Some(String::from("Night drive")));

        // Nothing changed, so nothing is written again.
        let path = open.folders().unwrap().songs.join("Night drive.song");
        std::fs::write(&path, "left alone").unwrap();
        save_now(&mut open, &composer, &mut engine).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "left alone");
    }

    #[test]
    fn opening_a_song_saves_the_open_one_first() {
        let (mut open, mut composer, mut engine) = starting("session-switch");
        put(&open, "Night drive", &night_drive());
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        composer.pick_key(a_minor());

        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", Instant::now()).unwrap();
        assert_eq!(file_of(&open, "Untitled 1").key, Some(a_minor()));
    }

    #[test]
    fn a_song_that_can_t_be_read_changes_nothing() {
        let (mut open, mut composer, mut engine) = starting("session-unreadable");
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        let songs_folder = open.folders().unwrap().songs;
        std::fs::write(songs_folder.join("Broken.song"), "tempo = [oops").unwrap();
        composer.pick_key(a_minor());

        let now = Instant::now();
        assert!(open_song_named(&mut open, &mut composer, &mut engine, "Broken", now).is_err());
        assert!(open_song_named(&mut open, &mut composer, &mut engine, "Missing", now).is_err());
        assert_eq!(open.name(), "Untitled 1");
        assert_eq!(composer.key(), Some(a_minor()));
        assert_eq!(last_song_of(&open), Some(String::from("Untitled 1")));
    }

    #[test]
    fn a_new_song_forgets_the_open_song_s_loop_and_snap() {
        let (mut open, mut composer, mut engine) = starting("session-new-forgets");
        put(&open, "Night drive", &night_drive());
        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", Instant::now()).unwrap();
        engine.render(64);

        new_song(&mut open, &mut composer, &mut engine).unwrap();
        engine.render(64);
        assert!(engine.snap());
        assert_eq!(engine.looper_status().state, LooperState::Idle);
        assert!(song_now(&composer, &mut engine).looped.is_none());
    }

    // A song with one layer of `count` short notes.
    fn song_with_notes(count: usize) -> Song {
        let mut notes = Vec::new();
        for i in 0..count {
            let start = i as f64 * 0.002;
            notes.push(note(A4, start, start + 0.001));
        }
        let layer = SongLayer {
            volume: FULL_VOLUME,
            playing: true,
            notes,
        };
        let mut song = night_drive();
        song.looped = Some(SongLoop {
            bars: 2,
            layers: vec![layer],
        });
        song
    }

    #[test]
    fn a_song_whose_loop_the_engine_refuses_is_never_saved_over() {
        // The first loop's 3,000 notes still wait for the engine, so the
        // second's 3,000 don't fit.
        let (mut open, mut composer, mut engine) = starting("session-refused");
        put(&open, "Big one", &song_with_notes(3000));
        put(&open, "Big two", &song_with_notes(3000));
        let big_two = open.folders().unwrap().songs.join("Big two.song");
        let text = std::fs::read_to_string(&big_two).unwrap();
        let now = Instant::now();
        open_song_named(&mut open, &mut composer, &mut engine, "Big one", now).unwrap();
        assert!(open_song_named(&mut open, &mut composer, &mut engine, "Big two", now).is_err());

        composer.pick_key(Key {
            tonic: 0,
            mode: Mode::Major,
        });
        save_now(&mut open, &composer, &mut engine).unwrap();
        save_if_settled(&mut open, &composer, &mut engine, now + Duration::from_secs(5));
        assert_eq!(std::fs::read_to_string(&big_two).unwrap(), text);
    }

    #[test]
    fn launch_reopens_the_last_song() {
        let (mut open, mut composer, mut engine) = starting("session-launch");
        put(&open, "Night drive", &night_drive());
        songs::remember_last_song(&open.folders().unwrap().settings, "Night drive").unwrap();

        open_last_song(&mut open, &mut composer, &mut engine, Instant::now());
        assert_eq!(open.name(), "Night drive");
        assert_eq!(composer.key(), Some(a_minor()));
    }

    #[test]
    fn launch_makes_a_new_song_when_the_last_one_is_gone() {
        let (mut open, mut composer, mut engine) = starting("session-launch-new");
        open_last_song(&mut open, &mut composer, &mut engine, Instant::now());
        assert_eq!(open.name(), "Untitled 1");

        let (mut open, mut composer, mut engine) = starting("session-launch-gone");
        songs::remember_last_song(&open.folders().unwrap().settings, "Night drive").unwrap();
        open_last_song(&mut open, &mut composer, &mut engine, Instant::now());
        assert_eq!(open.name(), "Untitled 1");
        assert_eq!(last_song_of(&open), Some(String::from("Untitled 1")));
    }

    #[test]
    fn a_renamed_song_saves_under_its_new_name() {
        let (mut open, mut composer, mut engine) = starting("session-rename");
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        // The engine takes in the new song, so the tempo can change.
        engine.render(64);
        composer.pick_key(a_minor());

        rename_open_song(&mut open, &composer, &mut engine, " Night drive ").unwrap();
        assert_eq!(open.name(), "Night drive");
        assert_eq!(file_of(&open, "Night drive").key, Some(a_minor()));
        let folders = open.folders().unwrap();
        assert_eq!(songs::list(&folders.songs).len(), 1);
        assert_eq!(last_song_of(&open), Some(String::from("Night drive")));

        composer.speed_up(&mut engine);
        save_now(&mut open, &composer, &mut engine).unwrap();
        assert_eq!(file_of(&open, "Night drive").tempo, START_TEMPO + 5);
        assert_eq!(songs::list(&folders.songs).len(), 1);
    }

    #[test]
    fn a_preset_lights_again_only_if_the_sound_still_matches_it() {
        let (mut open, mut composer, mut engine) = starting("session-preset");
        let folders = open.folders().unwrap();
        let mut song = night_drive();
        song.preset = Some(String::from("Lead"));
        put(&open, "Night drive", &song);
        let now = Instant::now();

        presets::save(&folders.sounds, "Lead", &song.sound).unwrap();
        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", now).unwrap();
        assert_eq!(composer.lit_preset(), Some("Lead"));

        presets::save(&folders.sounds, "Lead", &saw_sound()).unwrap();
        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", now).unwrap();
        assert_eq!(composer.lit_preset(), None);
        // Saved without it, since the sound no longer matches it.
        assert_eq!(song_now(&composer, &mut engine).preset, None);
    }

    #[test]
    fn a_held_chord_waits_until_it_is_let_go() {
        let mut composer = Composer::new();
        let mut engine = Engine::new().unwrap();
        let start = Instant::now();
        composer.chord_pressed("a", am(), start);
        composer.add_last_chord();
        assert!(song_now(&composer, &mut engine).chords.is_empty());

        composer.key_released("a", start + Duration::from_millis(700));
        let expected = HeldChord {
            chord: am(),
            held_ms: 700,
        };
        assert_eq!(song_now(&composer, &mut engine).chords, vec![expected]);
    }

    #[test]
    fn a_layer_still_recording_is_left_out() {
        let (mut open, mut composer, mut engine) = starting("session-recording");
        let song = night_drive();
        put(&open, "Night drive", &song);
        open_song_named(&mut open, &mut composer, &mut engine, "Night drive", Instant::now()).unwrap();
        engine.render(64);

        // Right Shift plays the loop, and again records a layer over it.
        engine.press_looper();
        engine.render(64);
        engine.press_looper();
        engine.note_on(A4, LAPTOP_VELOCITY);
        engine.render(64);
        engine.note_off(A4);
        engine.render(64);
        assert_eq!(engine.loop_layers().len(), 3);
        assert!(engine.loop_layers()[2].recording);

        let kept = song_now(&composer, &mut engine).looped.unwrap();
        assert_eq!(kept, song.looped.unwrap());
    }

    #[test]
    fn the_ui_reads_the_songs_by_these_names() {
        let view = SongsView {
            songs: vec![SongView {
                name: String::from("Night drive"),
                readable: false,
            }],
            open: String::from("Night drive"),
        };
        let json = serde_json::to_value(view).unwrap();
        let expected = serde_json::json!({
            "songs": [{ "name": "Night drive", "readable": false }],
            "open": "Night drive",
        });
        assert_eq!(json, expected);
    }
}
