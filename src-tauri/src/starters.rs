// The starter instruments (docs/design.md#sampler): free recordings Jello
// can fetch into the Instruments folder, each only once the player asks
// for it on the screen. Nothing comes with Jello itself.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands;

// How often the screen hears how far a download has got.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

// The folder in Instruments a download is unpacked in before its
// instrument is moved out. Starting with a dot keeps it off the list.
const UNPACKING: &str = ".unpacking";

// How deep in an unpacked download to look for its SFZ file.
const DEEPEST_LOOK: usize = 4;

// One starter instrument: what it is called once fetched, where it comes
// from, how big the download is, and whose it is.
pub struct Starter {
    pub name: &'static str,
    url: &'static str,
    bytes: u64,
    // Whether it plays notes or drums, for the screen.
    pub drums: bool,
    pub license: &'static str,
    pub credit: &'static str,
}

pub const STARTERS: [Starter; 2] = [
    Starter {
        name: "Salamander Grand Piano",
        url: "https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/SalamanderGrandPiano-SFZ+FLAC-V3+20200602.tar.gz",
        bytes: 741_757_374,
        drums: false,
        license: "CC BY 3.0",
        credit: "Alexander Holm",
    },
    Starter {
        name: "AVL Black Pearl",
        url: "https://www.bandshed.net/sounds/AVLDrumkits_SFZ/BLACK_PEARL_2023_repack.zip",
        bytes: 14_176_289,
        drums: true,
        license: "CC BY-SA 3.0",
        credit: "Glen MacArthur, AVL Drumkits",
    },
];

// The starter being fetched, if one is. One at a time.
#[derive(Default)]
pub struct Starters {
    downloading: Option<&'static str>,
}

// A starter as the screen shows it.
#[derive(Serialize)]
pub struct StarterView {
    name: &'static str,
    megabytes: u64,
    drums: bool,
    license: &'static str,
    credit: &'static str,
    // True once its folder is in Instruments.
    fetched: bool,
    downloading: bool,
}

pub fn starter_views(folder: Option<&Path>, starters: &Starters) -> Vec<StarterView> {
    let mut views = Vec::new();
    for starter in &STARTERS {
        let mut fetched = false;
        if let Some(folder) = folder {
            fetched = folder.join(starter.name).is_dir();
        }
        views.push(StarterView {
            name: starter.name,
            megabytes: starter.bytes / 1_000_000,
            drums: starter.drums,
            license: starter.license,
            credit: starter.credit,
            fetched,
            downloading: starters.downloading == Some(starter.name),
        });
    }
    views
}

fn starter_named(name: &str) -> Option<&'static Starter> {
    for starter in &STARTERS {
        if starter.name == name {
            return Some(starter);
        }
    }
    None
}

// Downloads starter `name` into the Instruments folder, telling the screen
// how far it has got ("starter-download"), then unpacks it there. Only the
// player's own click on the starter's Download button sends this.
#[tauri::command(async)]
pub fn download_starter(app: AppHandle, name: String) -> Result<(), String> {
    let Some(starter) = starter_named(&name) else {
        return Err(format!("there is no starter instrument called {name}"));
    };
    let folder = commands::instruments_folder(&app)?;
    if folder.join(starter.name).is_dir() {
        return Ok(());
    }
    {
        let starters = app.state::<Mutex<Starters>>();
        let mut starters = starters.lock().unwrap();
        if starters.downloading.is_some() {
            return Err(String::from("a starter instrument is already downloading"));
        }
        starters.downloading = Some(starter.name);
    }
    let result = fetch(&app, starter, &folder);
    let starters = app.state::<Mutex<Starters>>();
    starters.lock().unwrap().downloading = None;
    let _ = app.emit("instruments-changed", ());
    result
}

fn fetch(app: &AppHandle, starter: &Starter, folder: &Path) -> Result<(), String> {
    let unpacking = folder.join(UNPACKING);
    let _ = fs::remove_dir_all(&unpacking);
    fs::create_dir_all(&unpacking).map_err(|error| format!("the Instruments folder couldn't be made: {error}"))?;
    let archive = unpacking.join(archive_name(starter));

    let result = download(app, starter, &archive);
    let result = result.and_then(|_| unpack(&archive, &unpacking));
    let result = result.and_then(|_| keep_instrument(starter, &unpacking, folder));
    let _ = fs::remove_dir_all(&unpacking);
    result
}

// The download's file name: the last part of its address.
fn archive_name(starter: &Starter) -> String {
    let mut name = "download";
    if let Some(last) = starter.url.rsplit('/').next() {
        name = last;
    }
    String::from(name)
}

fn download(app: &AppHandle, starter: &Starter, archive: &Path) -> Result<(), String> {
    // curl comes with macOS and Windows.
    let mut curl = Command::new("curl")
        .args(["--location", "--fail", "--silent", "--show-error", "--output"])
        .arg(archive)
        .arg(starter.url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("the download couldn't start: {error}"))?;
    let mut told = Instant::now();
    let status = loop {
        if let Some(status) = curl.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if told.elapsed() >= PROGRESS_EVERY {
            tell_progress(app, starter, received_of(archive));
            told = Instant::now();
        }
        thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        let mut said = String::new();
        if let Some(mut errors) = curl.stderr.take() {
            let _ = errors.read_to_string(&mut said);
        }
        return Err(format!("{} couldn't download: {}", starter.name, said.trim()));
    }
    let received = received_of(archive);
    if received != starter.bytes {
        return Err(format!("{} didn't come whole, so it wasn't kept", starter.name));
    }
    tell_progress(app, starter, received);
    Ok(())
}

fn received_of(archive: &Path) -> u64 {
    match fs::metadata(archive) {
        Ok(file) => file.len(),
        Err(_) => 0,
    }
}

fn tell_progress(app: &AppHandle, starter: &Starter, received: u64) {
    let progress = json!({ "name": starter.name, "received": received, "total": starter.bytes });
    let _ = app.emit("starter-download", progress);
}

// Unpacks `archive` into `into` with tar, which comes with macOS and
// Windows and reads both .zip and .tar.gz. It refuses any file that would
// land outside `into`.
fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("the download couldn't be unpacked: {error}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!("the download couldn't be unpacked: {}", said.trim()));
    }
    let _ = fs::remove_file(archive);
    Ok(())
}

// Moves the folder holding the unpacked SFZ file into Instruments, named
// for the starter, with any license or readme from the folders above it.
fn keep_instrument(starter: &Starter, unpacking: &Path, folder: &Path) -> Result<(), String> {
    let Some(found) = folder_with_sfz(unpacking, 0) else {
        return Err(format!("{} came without an SFZ file Jello can read", starter.name));
    };
    let mut above = found.parent();
    while let Some(parent) = above {
        if !parent.starts_with(unpacking) {
            break;
        }
        copy_notes(parent, &found);
        above = parent.parent();
    }
    let kept = folder.join(starter.name);
    fs::rename(&found, &kept).map_err(|error| format!("{} couldn't be kept: {error}", starter.name))?;
    Ok(())
}

// The first folder, looking level by level from `folder` down, that holds
// an SFZ file, or None.
fn folder_with_sfz(folder: &Path, depth: usize) -> Option<PathBuf> {
    let mut inside = Vec::new();
    for entry in fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            inside.push(path);
            continue;
        }
        let extension = path.extension().map(|text| text.to_string_lossy().to_lowercase());
        if extension.as_deref() == Some("sfz") {
            return Some(folder.to_path_buf());
        }
    }
    if depth >= DEEPEST_LOOK {
        return None;
    }
    inside.sort();
    for path in inside {
        if let Some(found) = folder_with_sfz(&path, depth + 1) {
            return Some(found);
        }
    }
    None
}

// Copies each license or readme file in `from` into `into`, so the
// instrument keeps who made it and on what terms.
fn copy_notes(from: &Path, into: &Path) {
    let Ok(entries) = fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let is_note = name.contains("licen") || name.contains("readme") || name.contains("copying");
        if is_note {
            let _ = fs::copy(&path, into.join(entry.file_name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A folder of its own in the temp folder, emptied first.
    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("jello-starters-{name}"));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn the_instrument_is_the_folder_its_sfz_is_in_with_its_license() {
        let unpacking = scratch("found");
        let inner = unpacking.join("Kit 2023").join("SFZ");
        fs::create_dir_all(&inner).unwrap();
        fs::write(unpacking.join("Kit 2023").join("LICENSE.txt"), "CC BY-SA 3.0").unwrap();
        fs::write(inner.join("kit.sfz"), "<region> sample=kick.wav").unwrap();
        let instruments = scratch("kept");

        keep_instrument(&STARTERS[1], &unpacking, &instruments).unwrap();
        let kept = instruments.join(STARTERS[1].name);
        assert!(kept.join("kit.sfz").is_file());
        assert!(kept.join("LICENSE.txt").is_file());
    }

    #[test]
    fn a_download_without_an_sfz_is_not_kept() {
        let unpacking = scratch("empty");
        fs::write(unpacking.join("readme.txt"), "nothing here").unwrap();
        let instruments = scratch("none kept");
        let result = keep_instrument(&STARTERS[0], &unpacking, &instruments);
        assert!(result.is_err());
        assert!(!instruments.join(STARTERS[0].name).exists());
    }

    #[test]
    fn a_zip_unpacks_with_tar() {
        // tar comes with macOS and Windows, and Jello unpacks every starter
        // with it.
        let folder = scratch("zip");
        let source = folder.join("Drums");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("drums.sfz"), "<region> sample=kick.wav").unwrap();
        let archive = folder.join("drums.zip");
        let made = Command::new("tar")
            .arg("-a")
            .arg("-cf")
            .arg(&archive)
            .arg("-C")
            .arg(&folder)
            .arg("Drums")
            .status()
            .unwrap();
        assert!(made.success());
        let into = folder.join("into");
        fs::create_dir_all(&into).unwrap();

        unpack(&archive, &into).unwrap();
        assert!(into.join("Drums").join("drums.sfz").is_file());
        assert!(!archive.exists());
    }

    #[test]
    fn every_starter_has_its_own_name_and_a_download_name() {
        assert_ne!(STARTERS[0].name, STARTERS[1].name);
        assert_eq!(archive_name(&STARTERS[1]), "BLACK_PEARL_2023_repack.zip");
        assert!(starter_named("Salamander Grand Piano").is_some());
        assert!(starter_named("Cowbell").is_none());
    }
}
