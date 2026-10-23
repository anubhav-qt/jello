// Song files (docs/adr/0038-song-files.md): each song is a text file in
// Music/Jello/Songs, like "Night drive.song", written as
// src-tauri/src/song_file.rs says. The song open saves itself a moment
// after each change. Its name is the file's name, and the settings file
// remembers which song was open last, so Jello opens it again.
//
// Everything here works on folders it is given, so the tests can use a
// folder of their own.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::presets::{name_problem, set_setting, setting};
use crate::song_file::{Song, song_of};
use crate::sound_profile::SoundProfile;

// What every song file's name ends with.
const EXTENSION: &str = "song";

// The setting that names the song open last.
const LAST_SONG: &str = "last_song";

// What a new song is called, before its number.
const NEW_SONG_NAME: &str = "Untitled";

// One song, as the Songs panel shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SongView {
    pub name: String,
    // False for a file that is not a song file: its tile is dimmed and
    // can't be opened.
    pub readable: bool,
}

// Every song in `folder`, sorted by name with upper and lower case
// together, each marked readable or not. A missing folder has none.
pub fn list(folder: &Path) -> Vec<SongView> {
    let mut songs = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return songs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = song_name_of(&path) else {
            continue;
        };
        let readable = read_song(&path, &SoundProfile::new()).is_ok();
        songs.push(SongView { name, readable });
    }
    songs.sort_by_key(|song| song.name.to_lowercase());
    songs
}

// The song name of the file at `path`: its name without ".song". None for
// any other file, and for hidden ones, whose names start with a dot, like
// a save on its way.
fn song_name_of(path: &Path) -> Option<String> {
    let extension = path.extension()?;
    if extension != EXTENSION {
        return None;
    }
    let name = path.file_stem()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    Some(String::from(name))
}

// The file song `name` is kept in, inside `folder`. Fails on a name that
// could reach outside the folder.
fn path_of(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let reaches_out = name.contains('/') || name.contains('\\') || name.starts_with('.');
    if reaches_out || name.is_empty() {
        return Err(format!("there is no song called {name}"));
    }
    Ok(folder.join(format!("{name}.{EXTENSION}")))
}

// Song `name` in `folder`, with `new_song`'s sound for whatever its sound
// leaves out.
pub fn read(folder: &Path, name: &str, new_song: &SoundProfile) -> Result<Song, String> {
    let path = path_of(folder, name)?;
    read_song(&path, new_song)
}

fn read_song(path: &Path, new_song: &SoundProfile) -> Result<Song, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => return Err(error.to_string()),
    };
    song_of(&text, new_song)
}

// Writes song `name` in `folder`, made if it is missing, as `text`, a song
// file's text (song_file::text_of). The text goes to a hidden file first,
// which then takes the song file's place, so a crash halfway never leaves
// half a song.
pub fn save_text(folder: &Path, name: &str, text: &str) -> Result<(), String> {
    let path = path_of(folder, name)?;
    if let Err(error) = fs::create_dir_all(folder) {
        return Err(error.to_string());
    }
    let saving = folder.join(format!(".{name}.{EXTENSION}.saving"));
    if let Err(error) = fs::write(&saving, text) {
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(&saving, &path) {
        return Err(error.to_string());
    }
    Ok(())
}

// True if a song in `folder` is called `name`, with upper and lower case
// taken as the same: on a Mac both would be the same file.
fn is_taken(folder: &Path, name: &str) -> bool {
    let wanted = name.to_lowercase();
    for song in list(folder) {
        if song.name.to_lowercase() == wanted {
            return true;
        }
    }
    false
}

// A name for a new song in `folder`: "Untitled 1", or the lowest number no
// song has.
pub fn new_name(folder: &Path) -> String {
    let mut number = 1;
    loop {
        let name = format!("{NEW_SONG_NAME} {number}");
        if !is_taken(folder, &name) {
            return name;
        }
        number += 1;
    }
}

// Why song `from` in `folder` can't be renamed `to`, in words for the
// Songs panel, or None if it can. Spaces at either end don't count. The
// name rules are a preset's, and no other song can be called the same.
// Only changing the case of its own name is fine.
pub fn rename_problem(folder: &Path, from: &str, to: &str) -> Option<String> {
    if let Some(problem) = name_problem(to) {
        return Some(problem);
    }
    let to = to.trim();
    let is_own_name = to.to_lowercase() == from.to_lowercase();
    if is_own_name {
        return None;
    }
    if is_taken(folder, to) {
        return Some(format!("A song is already called {to}"));
    }
    None
}

// Renames song `from` in `folder` to `to`, with the spaces at its ends left
// off, and returns the new name. Fails for a name rename_problem refuses.
pub fn rename(folder: &Path, from: &str, to: &str) -> Result<String, String> {
    if let Some(problem) = rename_problem(folder, from, to) {
        return Err(problem);
    }
    let to = to.trim();
    let from_path = path_of(folder, from)?;
    let to_path = path_of(folder, to)?;
    if let Err(error) = fs::rename(from_path, to_path) {
        return Err(error.to_string());
    }
    Ok(String::from(to))
}

// The song open last, from the settings file in `settings_folder`, or None.
pub fn last_song(settings_folder: &Path) -> Option<String> {
    setting(settings_folder, LAST_SONG)
}

// Remembers song `name` as the one open last.
pub fn remember_last_song(settings_folder: &Path, name: &str) -> Result<(), String> {
    set_setting(settings_folder, LAST_SONG, Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::presets::{mark_starting, starting_name, test_folder};
    use crate::song_file::text_of;

    // A new song's text, at `tempo`.
    fn song_text(tempo: u32) -> String {
        let song = Song {
            key: None,
            tempo,
            snap: true,
            preset: None,
            chords: Vec::new(),
            looped: None,
            sound: SoundProfile::new().sound(),
        };
        text_of(&song)
    }

    fn names_in(songs: &[SongView]) -> Vec<&str> {
        let mut names = Vec::new();
        for song in songs {
            names.push(song.name.as_str());
        }
        names
    }

    fn files_in(folder: &Path) -> Vec<String> {
        let mut files = Vec::new();
        for entry in fs::read_dir(folder).unwrap() {
            let name = entry.unwrap().file_name();
            files.push(name.to_string_lossy().into_owned());
        }
        files.sort();
        files
    }

    #[test]
    fn a_saved_song_reads_back_and_is_listed() {
        let folder = test_folder("song-saved");
        save_text(&folder, "Night drive", &song_text(97)).unwrap();
        assert_eq!(files_in(&folder), vec!["Night drive.song"]);

        let song = read(&folder, "Night drive", &SoundProfile::new()).unwrap();
        assert_eq!(song.tempo, 97);

        let expected = SongView {
            name: String::from("Night drive"),
            readable: true,
        };
        assert_eq!(list(&folder), vec![expected]);
    }

    #[test]
    fn saving_again_replaces_the_song() {
        let folder = test_folder("song-again");
        save_text(&folder, "Loop", &song_text(97)).unwrap();
        save_text(&folder, "Loop", &song_text(130)).unwrap();
        assert_eq!(files_in(&folder), vec!["Loop.song"]);
        let song = read(&folder, "Loop", &SoundProfile::new()).unwrap();
        assert_eq!(song.tempo, 130);
    }

    #[test]
    fn songs_are_listed_by_name_and_files_that_are_not_songs_are_dimmed() {
        let folder = test_folder("song-list");
        save_text(&folder, "b", &song_text(97)).unwrap();
        save_text(&folder, "A", &song_text(97)).unwrap();
        fs::write(folder.join("c.song"), "tempo = [oops").unwrap();
        fs::write(folder.join("notes.txt"), "not a song").unwrap();
        fs::write(folder.join(".d.song.saving"), "half a song").unwrap();

        let songs = list(&folder);
        assert_eq!(names_in(&songs), vec!["A", "b", "c"]);
        assert!(songs[0].readable);
        assert!(songs[1].readable);
        assert!(!songs[2].readable);
        assert!(read(&folder, "c", &SoundProfile::new()).is_err());
    }

    #[test]
    fn a_missing_folder_has_no_songs() {
        let folder = test_folder("song-missing");
        assert_eq!(list(&folder), Vec::new());
        assert!(read(&folder, "Night drive", &SoundProfile::new()).is_err());
    }

    #[test]
    fn names_that_reach_outside_the_folder_are_refused() {
        let folder = test_folder("song-outside");
        for name in ["", "../escape", "a/b", "a\\b", ".hidden"] {
            assert!(save_text(&folder, name, &song_text(97)).is_err(), "{name:?}");
            assert!(read(&folder, name, &SoundProfile::new()).is_err(), "{name:?}");
        }
        assert!(!folder.exists());
    }

    #[test]
    fn new_names_count_up_from_the_lowest_free_number() {
        let folder = test_folder("song-new-names");
        assert_eq!(new_name(&folder), "Untitled 1");
        save_text(&folder, "untitled 1", &song_text(97)).unwrap();
        assert_eq!(new_name(&folder), "Untitled 2");
        save_text(&folder, "Untitled 3", &song_text(97)).unwrap();
        assert_eq!(new_name(&folder), "Untitled 2");
        save_text(&folder, "Untitled 2", &song_text(97)).unwrap();
        assert_eq!(new_name(&folder), "Untitled 4");
    }

    #[test]
    fn a_renamed_song_moves_to_its_new_name() {
        let folder = test_folder("song-rename");
        save_text(&folder, "Untitled 1", &song_text(97)).unwrap();
        let renamed = rename(&folder, "Untitled 1", "  Night drive ").unwrap();
        assert_eq!(renamed, "Night drive");
        assert_eq!(files_in(&folder), vec!["Night drive.song"]);
        let song = read(&folder, "Night drive", &SoundProfile::new()).unwrap();
        assert_eq!(song.tempo, 97);
    }

    #[test]
    fn a_song_can_t_take_another_song_s_name_in_any_case() {
        let folder = test_folder("song-taken");
        save_text(&folder, "Night drive", &song_text(97)).unwrap();
        save_text(&folder, "Untitled 1", &song_text(130)).unwrap();
        let problem = rename_problem(&folder, "Untitled 1", "night DRIVE");
        assert_eq!(problem, Some(String::from("A song is already called night DRIVE")));
        assert!(rename(&folder, "Untitled 1", "night DRIVE").is_err());
        let song = read(&folder, "Night drive", &SoundProfile::new()).unwrap();
        assert_eq!(song.tempo, 97);
    }

    #[test]
    fn a_song_can_change_the_case_of_its_own_name() {
        let folder = test_folder("song-own-case");
        save_text(&folder, "night drive", &song_text(97)).unwrap();
        assert_eq!(rename_problem(&folder, "night drive", "Night Drive"), None);
        rename(&folder, "night drive", "Night Drive").unwrap();
        assert_eq!(names_in(&list(&folder)), vec!["Night Drive"]);
    }

    #[test]
    fn names_a_preset_can_t_have_are_refused() {
        let folder = test_folder("song-bad-names");
        save_text(&folder, "Untitled 1", &song_text(97)).unwrap();
        for name in ["", "   ", "a/b", "..", "what?", "con"] {
            assert!(rename_problem(&folder, "Untitled 1", name).is_some(), "{name:?}");
            assert!(rename(&folder, "Untitled 1", name).is_err(), "{name:?}");
        }
        assert_eq!(files_in(&folder), vec!["Untitled 1.song"]);
    }

    #[test]
    fn the_last_song_is_remembered_beside_the_other_settings() {
        let settings = test_folder("song-last");
        assert_eq!(last_song(&settings), None);
        mark_starting(&settings, Some("Alto")).unwrap();
        remember_last_song(&settings, "Night drive").unwrap();
        assert_eq!(last_song(&settings), Some(String::from("Night drive")));
        assert_eq!(starting_name(&settings), Some(String::from("Alto")));
    }
}
