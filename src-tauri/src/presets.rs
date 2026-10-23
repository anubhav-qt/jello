// Presets (docs/adr/0037-presets.md): sounds saved by name, each a text file
// in Music/Jello/Sounds, like "Glass lead.sound", written as
// src-tauri/src/sound_file.rs says. Loading one copies it into the song,
// which then changes its copy as it likes; the file stays as it was. One
// preset can be marked as the starting sound, which every new song starts
// with. The mark is kept in Jello's settings file.
//
// Everything here works on folders it is given, so the tests can use a
// folder of their own.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use toml::{Table, Value};

use crate::sound_file::{Sound, sound_of, text_of};
use crate::sound_profile::SoundProfile;

// What every preset file's name ends with.
const EXTENSION: &str = "sound";

// The longest a preset's name can be, in letters.
pub const NAME_LONGEST: usize = 40;

// Names Windows keeps for its devices: a file can't be called any of them,
// in upper or lower case.
const WINDOWS_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

// Jello's settings file, in the app's own settings folder.
const SETTINGS_FILE: &str = "settings.toml";

// The setting that names the starting sound.
const STARTING_SOUND: &str = "starting_sound";

// One preset, as the Presets page shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PresetView {
    pub name: String,
    // False for a file that is not a sound file: its tile is dimmed and
    // can't be loaded.
    pub readable: bool,
    // Whether new songs start with it.
    pub starting: bool,
}

// Why `name` can't name a preset, in words for the Presets page, or None if
// it can. Spaces at either end don't count: saving leaves them off. Only
// letters, numbers, spaces, "-" and "_" are allowed, so a name never has a
// character Windows or macOS refuses in file names, or one that could
// reach outside the Sounds folder.
pub fn name_problem(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return Some(String::from("Type a name"));
    }
    if name.chars().count() > NAME_LONGEST {
        return Some(format!("Up to {NAME_LONGEST} letters"));
    }
    for letter in name.chars() {
        let allowed = letter.is_alphanumeric() || letter == ' ' || letter == '-' || letter == '_';
        if !allowed {
            return Some(String::from("Only letters, numbers, spaces, - and _"));
        }
    }
    for windows_name in WINDOWS_NAMES {
        if name.eq_ignore_ascii_case(windows_name) {
            return Some(String::from("Windows keeps this name for itself"));
        }
    }
    None
}

// Every preset in `folder`, sorted by name with upper and lower case
// together, each marked readable or not, and starting if it is called
// `starting`. A missing folder has none.
pub fn list(folder: &Path, starting: Option<&str>) -> Vec<PresetView> {
    let mut presets = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return presets;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = preset_name_of(&path) else {
            continue;
        };
        let readable = read_sound(&path, &SoundProfile::new()).is_ok();
        let is_starting = Some(name.as_str()) == starting;
        presets.push(PresetView {
            name,
            readable,
            starting: is_starting,
        });
    }
    presets.sort_by_key(|preset| preset.name.to_lowercase());
    presets
}

// The preset name of the file at `path`: its name without ".sound". None
// for any other file, and for hidden ones, whose names start with a dot.
fn preset_name_of(path: &Path) -> Option<String> {
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

// The file preset `name` is kept in, inside `folder`. Fails on a name that
// could reach outside the folder.
fn path_of(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let reaches_out = name.contains('/') || name.contains('\\') || name.starts_with('.');
    if reaches_out || name.is_empty() {
        return Err(format!("there is no preset called {name}"));
    }
    Ok(folder.join(format!("{name}.{EXTENSION}")))
}

// Preset `name` in `folder`, as a sound that takes `new_song`'s values for
// whatever the file leaves out.
pub fn read(folder: &Path, name: &str, new_song: &SoundProfile) -> Result<Sound, String> {
    let path = path_of(folder, name)?;
    read_sound(&path, new_song)
}

fn read_sound(path: &Path, new_song: &SoundProfile) -> Result<Sound, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => return Err(error.to_string()),
    };
    sound_of(&text, new_song)
}

// Saves `sound` as preset `name` in `folder`, made if it is missing, and
// returns the name it was saved as. Spaces at either end of `name` are left
// off. A preset already called that is replaced, and so is one whose name
// differs only in upper and lower case, which keeps its own name: on a Mac
// both would be the same file. Fails on a name `name_problem` refuses.
pub fn save(folder: &Path, name: &str, sound: &Sound) -> Result<String, String> {
    if let Some(problem) = name_problem(name) {
        return Err(problem);
    }
    let mut name = String::from(name.trim());
    for preset in list(folder, None) {
        if preset.name.to_lowercase() == name.to_lowercase() {
            name = preset.name;
        }
    }

    if let Err(error) = fs::create_dir_all(folder) {
        return Err(error.to_string());
    }
    let path = path_of(folder, &name)?;
    if let Err(error) = fs::write(&path, text_of(sound)) {
        return Err(error.to_string());
    }
    Ok(name)
}

// The name of the starting sound, from the settings file in
// `settings_folder`, or None if no preset is marked or the file can't be
// read.
pub fn starting_name(settings_folder: &Path) -> Option<String> {
    setting(settings_folder, STARTING_SOUND)
}

// The word setting `setting` holds in the settings file in
// `settings_folder`, or None if it holds none or the file can't be read.
pub fn setting(settings_folder: &Path, setting: &str) -> Option<String> {
    let settings = read_settings(settings_folder);
    match settings.get(setting) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

// Marks preset `name` as the starting sound, or with None, unmarks it, in
// the settings file in `settings_folder`, made if it is missing. Any other
// settings in the file stay as they were.
pub fn mark_starting(settings_folder: &Path, name: Option<&str>) -> Result<(), String> {
    set_setting(settings_folder, STARTING_SOUND, name)
}

// Sets setting `setting` to the word `value`, or with None, takes it out, in
// the settings file in `settings_folder`, made if it is missing. Any other
// settings in the file stay as they were.
pub fn set_setting(settings_folder: &Path, setting: &str, value: Option<&str>) -> Result<(), String> {
    let mut settings = read_settings(settings_folder);
    match value {
        Some(value) => {
            settings.insert(String::from(setting), Value::String(String::from(value)));
        }
        None => {
            settings.remove(setting);
        }
    }

    if let Err(error) = fs::create_dir_all(settings_folder) {
        return Err(error.to_string());
    }
    let path = settings_folder.join(SETTINGS_FILE);
    if let Err(error) = fs::write(path, settings.to_string()) {
        return Err(error.to_string());
    }
    Ok(())
}

// The settings in `settings_folder`, or none if the file is missing or is
// not TOML.
fn read_settings(settings_folder: &Path) -> Table {
    let path = settings_folder.join(SETTINGS_FILE);
    let Ok(text) = fs::read_to_string(path) else {
        return Table::new();
    };
    match text.parse::<Table>() {
        Ok(settings) => settings,
        Err(_) => Table::new(),
    }
}

// A fresh, empty folder for one test, not made yet: `label` keeps tests
// that run at once apart.
#[cfg(test)]
pub fn test_folder(label: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("jello-test-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    folder
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::Kind;

    // A new song's sound with its first oscillator on `kind`.
    fn sound_on(kind: Kind) -> Sound {
        let mut sound = SoundProfile::new().sound();
        sound.oscillators[0].kind = kind;
        sound
    }

    fn names_in(presets: &[PresetView]) -> Vec<&str> {
        let mut names = Vec::new();
        for preset in presets {
            names.push(preset.name.as_str());
        }
        names
    }

    #[test]
    fn names_with_letters_numbers_spaces_dashes_and_underscores_are_fine() {
        assert_eq!(name_problem("Glass lead"), None);
        assert_eq!(name_problem("bass-2_old"), None);
        assert_eq!(name_problem("  Pad  "), None);
        assert_eq!(name_problem("Élan"), None);
        let longest = "a".repeat(NAME_LONGEST);
        assert_eq!(name_problem(&longest), None);
    }

    #[test]
    fn other_names_are_refused_with_a_reason() {
        let refused = [
            "",
            "   ",
            "a/b",
            "a\\b",
            "..",
            ".hidden",
            "what?",
            "a:b",
            "x*",
            "<lead>",
            "\"quoted\"",
            "pipe|d",
            "con",
            "Lpt1",
            "NUL",
        ];
        for name in refused {
            assert!(name_problem(name).is_some(), "{name:?} was allowed");
        }
        let too_long = "a".repeat(NAME_LONGEST + 1);
        assert_eq!(name_problem(&too_long), Some(String::from("Up to 40 letters")));
    }

    #[test]
    fn a_saved_preset_reads_back_and_is_listed() {
        let folder = test_folder("saved");
        let saved = save(&folder, "  Glass lead ", &sound_on(Kind::Saw)).unwrap();
        assert_eq!(saved, "Glass lead");
        assert!(folder.join("Glass lead.sound").exists());

        let read = read(&folder, "Glass lead", &SoundProfile::new()).unwrap();
        assert_eq!(read.oscillators[0].kind, Kind::Saw);

        let presets = list(&folder, None);
        let expected = PresetView {
            name: String::from("Glass lead"),
            readable: true,
            starting: false,
        };
        assert_eq!(presets, vec![expected]);
    }

    #[test]
    fn a_refused_name_saves_nothing() {
        let folder = test_folder("refused");
        let result = save(&folder, "a/b", &sound_on(Kind::Saw));
        assert!(result.is_err());
        assert!(!folder.exists());
    }

    #[test]
    fn presets_are_listed_by_name_with_upper_and_lower_case_together() {
        let folder = test_folder("sorted");
        // Sorted by their letters alone, "Choir" would come first.
        for name in ["bass", "Choir", "alto"] {
            save(&folder, name, &sound_on(Kind::Sine)).unwrap();
        }
        // Other files, and hidden ones, are not presets.
        fs::write(folder.join("notes.txt"), "hello").unwrap();
        fs::write(folder.join(".secret.sound"), "").unwrap();
        let presets = list(&folder, None);
        assert_eq!(names_in(&presets), vec!["alto", "bass", "Choir"]);
    }

    #[test]
    fn a_missing_folder_has_no_presets() {
        let folder = test_folder("missing");
        assert_eq!(list(&folder, None), Vec::new());
    }

    #[test]
    fn saving_under_a_name_in_other_case_replaces_it_and_keeps_its_name() {
        let folder = test_folder("replace");
        save(&folder, "Glass Lead", &sound_on(Kind::Saw)).unwrap();
        let saved = save(&folder, "glass lead", &sound_on(Kind::Square)).unwrap();
        assert_eq!(saved, "Glass Lead");

        let presets = list(&folder, None);
        assert_eq!(names_in(&presets), vec!["Glass Lead"]);
        let read = read(&folder, "Glass Lead", &SoundProfile::new()).unwrap();
        assert_eq!(read.oscillators[0].kind, Kind::Square);
    }

    #[test]
    fn a_file_that_is_not_a_sound_is_listed_dimmed_and_can_t_load() {
        let folder = test_folder("broken");
        save(&folder, "Fine", &sound_on(Kind::Saw)).unwrap();
        fs::write(folder.join("Broken.sound"), "this is [not a sound").unwrap();

        let presets = list(&folder, None);
        assert_eq!(names_in(&presets), vec!["Broken", "Fine"]);
        assert!(!presets[0].readable);
        assert!(presets[1].readable);
        assert!(read(&folder, "Broken", &SoundProfile::new()).is_err());
    }

    #[test]
    fn a_missing_preset_or_one_outside_the_folder_can_t_be_read() {
        let outside = test_folder("outside");
        let folder = outside.join("Sounds");
        save(&folder, "Fine", &sound_on(Kind::Saw)).unwrap();
        // A sound file just outside the Sounds folder.
        fs::write(outside.join("Secret.sound"), text_of(&sound_on(Kind::Saw))).unwrap();

        assert!(read(&folder, "Fine", &SoundProfile::new()).is_ok());
        assert!(read(&folder, "Gone", &SoundProfile::new()).is_err());
        assert!(read(&folder, "../Secret", &SoundProfile::new()).is_err());
        assert!(read(&folder, "", &SoundProfile::new()).is_err());
    }

    #[test]
    fn one_preset_at_a_time_is_the_starting_sound() {
        let sounds = test_folder("starting-sounds");
        let settings = test_folder("starting-settings");
        for name in ["Alto", "Bass"] {
            save(&sounds, name, &sound_on(Kind::Saw)).unwrap();
        }
        assert_eq!(starting_name(&settings), None);

        mark_starting(&settings, Some("Alto")).unwrap();
        assert_eq!(starting_name(&settings), Some(String::from("Alto")));
        let presets = list(&sounds, starting_name(&settings).as_deref());
        assert!(presets[0].starting);
        assert!(!presets[1].starting);

        mark_starting(&settings, Some("Bass")).unwrap();
        let presets = list(&sounds, starting_name(&settings).as_deref());
        assert!(!presets[0].starting);
        assert!(presets[1].starting);

        mark_starting(&settings, None).unwrap();
        assert_eq!(starting_name(&settings), None);
    }

    #[test]
    fn marking_keeps_the_other_settings() {
        let settings = test_folder("other-settings");
        fs::create_dir_all(&settings).unwrap();
        fs::write(settings.join(SETTINGS_FILE), "theme = \"dark\"\n").unwrap();
        mark_starting(&settings, Some("Alto")).unwrap();
        let text = fs::read_to_string(settings.join(SETTINGS_FILE)).unwrap();
        assert!(text.contains("theme = \"dark\""), "{text}");
        assert!(text.contains("starting_sound = \"Alto\""), "{text}");
    }

    #[test]
    fn a_broken_settings_file_marks_nothing() {
        let settings = test_folder("broken-settings");
        fs::create_dir_all(&settings).unwrap();
        fs::write(settings.join(SETTINGS_FILE), "starting_sound = [oops").unwrap();
        assert_eq!(starting_name(&settings), None);
        fs::write(settings.join(SETTINGS_FILE), "starting_sound = 7").unwrap();
        assert_eq!(starting_name(&settings), None);
    }
}
