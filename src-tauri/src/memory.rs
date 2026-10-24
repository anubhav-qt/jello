// The agent's memory of the player (docs/design.md#chats-caching-and-memory):
// short notes it keeps across chats, each a habit, a taste, something the
// player knows, a goal or what to do next, with the chat it came from and
// its date. They are saved in Music/Jello/Memory/notes.json as they change.
// A new chat begins with them in its instructions, so a note saved now
// reaches the next chat and the chat open now keeps its cache.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State};

use crate::commands;
use crate::tools::{Studio, ToolAnswer};

pub const KINDS: [&str; 5] = ["habit", "taste", "knows", "goal", "next"];

// What each kind is, for the agent and the Memory page.
pub const KIND_WORDS: [(&str, &str); 5] = [
    ("habit", "Habits"),
    ("taste", "Tastes"),
    ("knows", "Knows"),
    ("goal", "Goals"),
    ("next", "Next"),
];

const MOST_NOTES: usize = 200;
const MOST_NOTE_CHARACTERS: usize = 300;
const FILE: &str = "notes.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u32,
    pub kind: String,
    pub text: String,
    // The file name of the chat the note came from, if it came from one.
    pub chat: Option<String>,
    // The day it was saved or last changed, like "2026-10-24".
    pub date: String,
}

// What notes.json holds.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    ask_first: bool,
    #[serde(default)]
    notes: Vec<Note>,
}

#[derive(Default)]
pub struct Memory {
    // Where notes.json is kept. Without it the notes live only until Jello
    // closes.
    folder: Option<PathBuf>,
    notes: Vec<Note>,
    // Whether the agent asks the player before saving a note.
    pub ask_first: bool,
    // The chat open now, by its file name.
    chat: Option<String>,
}

impl Memory {
    // The memory kept in `folder`, or an empty one if it has none yet or
    // its file can't be read.
    pub fn load(folder: Option<PathBuf>) -> Memory {
        let mut memory = Memory { folder, ..Memory::default() };
        let Some(folder) = &memory.folder else {
            return memory;
        };
        let Ok(text) = fs::read_to_string(folder.join(FILE)) else {
            return memory;
        };
        let Ok(saved) = serde_json::from_str::<Saved>(&text) else {
            return memory;
        };
        memory.notes = saved.notes;
        memory.ask_first = saved.ask_first;
        memory
    }

    #[cfg(test)]
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    // The chat open now, given by the host as the path of its file.
    pub fn set_chat(&mut self, path: Option<&str>) {
        let mut chat = None;
        if let Some(path) = path {
            let name = Path::new(path).file_name().map(|name| name.to_string_lossy().to_string());
            chat = name;
        }
        self.chat = chat;
    }

    // Keeps a new note of `kind` saying `text`, and returns it.
    pub fn remember(&mut self, kind: &str, text: &str) -> Result<Note, String> {
        check_kind(kind)?;
        let text = checked_text(text)?;
        if self.notes.len() >= MOST_NOTES {
            return Err(format!("memory holds at most {MOST_NOTES} notes: forget or change one first"));
        }
        let mut id = 1;
        for note in &self.notes {
            id = id.max(note.id + 1);
        }
        let note = Note {
            id,
            kind: kind.to_string(),
            text,
            chat: self.chat.clone(),
            date: today(),
        };
        self.notes.push(note.clone());
        self.save()?;
        Ok(note)
    }

    // Changes note `id` to say `text`, and to be of `kind` if one is given.
    pub fn change(&mut self, id: u32, text: &str, kind: Option<&str>) -> Result<Note, String> {
        if let Some(kind) = kind {
            check_kind(kind)?;
        }
        let text = checked_text(text)?;
        let chat = self.chat.clone();
        let Some(note) = self.notes.iter_mut().find(|note| note.id == id) else {
            return Err(format!("there is no note {id}"));
        };
        note.text = text;
        if let Some(kind) = kind {
            note.kind = kind.to_string();
        }
        if chat.is_some() {
            note.chat = chat;
        }
        note.date = today();
        let changed = note.clone();
        self.save()?;
        Ok(changed)
    }

    // Forgets note `id`, and returns what it said.
    pub fn forget(&mut self, id: u32) -> Result<Note, String> {
        let Some(place) = self.notes.iter().position(|note| note.id == id) else {
            return Err(format!("there is no note {id}"));
        };
        let note = self.notes.remove(place);
        self.save()?;
        Ok(note)
    }

    pub fn set_ask_first(&mut self, ask_first: bool) -> Result<(), String> {
        self.ask_first = ask_first;
        self.save()
    }

    // The notes as the agent reads them at the start of a chat: one line
    // each, by kind, with its number to change or forget it. Empty when
    // there are none.
    pub fn prompt_text(&self) -> String {
        let mut lines = Vec::new();
        for (kind, words) in KIND_WORDS {
            let mut kept = Vec::new();
            for note in &self.notes {
                if note.kind == kind {
                    kept.push(format!("- [{}] {} ({})", note.id, note.text, note.date));
                }
            }
            if kept.is_empty() {
                continue;
            }
            lines.push(format!("{words}:"));
            lines.extend(kept);
        }
        lines.join("\n")
    }

    // The notes and the switch, for jello_look and the Memory page.
    pub fn view(&self) -> Value {
        json!({"ask_first": self.ask_first, "notes": self.notes})
    }

    fn save(&self) -> Result<(), String> {
        let Some(folder) = &self.folder else {
            return Ok(());
        };
        let saved = Saved { ask_first: self.ask_first, notes: self.notes.clone() };
        let text = match serde_json::to_string_pretty(&saved) {
            Ok(text) => text,
            Err(error) => return Err(error.to_string()),
        };
        if let Err(error) = fs::create_dir_all(folder) {
            return Err(error.to_string());
        }
        // The text goes to a hidden file first, which then takes the real
        // one's place, so a crash halfway never leaves half the notes.
        let saving = folder.join(format!(".{FILE}.saving"));
        if let Err(error) = fs::write(&saving, text) {
            return Err(error.to_string());
        }
        if let Err(error) = fs::rename(&saving, folder.join(FILE)) {
            return Err(error.to_string());
        }
        Ok(())
    }
}

fn check_kind(kind: &str) -> Result<(), String> {
    if KINDS.contains(&kind) {
        return Ok(());
    }
    Err(format!("a note's kind is one of {}, not {kind}", KINDS.join(", ")))
}

fn checked_text(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(String::from("a note needs text"));
    }
    let count = text.chars().count();
    if count > MOST_NOTE_CHARACTERS {
        return Err(format!("a note is at most {MOST_NOTE_CHARACTERS} characters, not {count}"));
    }
    Ok(text.to_string())
}

// Today's date in UTC, like "2026-10-24".
fn today() -> String {
    let mut seconds = 0;
    if let Ok(since) = SystemTime::now().duration_since(UNIX_EPOCH) {
        seconds = since.as_secs();
    }
    date_of(seconds)
}

// The UTC date `seconds` after 1970 began, by the days-to-civil method
// (howardhinnant.github.io/date_algorithms.html).
fn date_of(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let day_of_era = days - era * 146097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_place = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_place + 2) / 5 + 1;
    let mut month = month_place + 3;
    if month > 12 {
        month -= 12;
    }
    let mut year = year_of_era + era * 400;
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}")
}

// jello_memory: remembers, changes or forgets a note. With the Memory
// page's switch on, the player is asked first. The memory is locked
// alone, and never while the player is asked.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let Some(action) = arguments["action"].as_str() else {
        return Err(String::from("action is needed: remember, change or forget"));
    };
    let ask_first = studio.memory.lock().unwrap().ask_first;
    match action {
        "remember" => {
            let kind = arguments["kind"].as_str().unwrap_or("");
            let text = arguments["text"].as_str().unwrap_or("");
            check_kind(kind)?;
            let text = checked_text(text)?;
            if ask_first && !studio.screen.ask(&format!("Remember this? {text}")) {
                return Err(String::from("the player said no: the note wasn't saved"));
            }
            let note = studio.memory.lock().unwrap().remember(kind, &text)?;
            Ok(note_answer("remember", note, "Remembered a note"))
        }
        "change" => {
            let id = id_of(arguments)?;
            let text = arguments["text"].as_str().unwrap_or("");
            let kind = arguments["kind"].as_str();
            let text = checked_text(text)?;
            if ask_first && !studio.screen.ask(&format!("Change note {id} to this? {text}")) {
                return Err(String::from("the player said no: the note wasn't changed"));
            }
            let note = studio.memory.lock().unwrap().change(id, &text, kind)?;
            Ok(note_answer("change", note, "Changed a note"))
        }
        "forget" => {
            let id = id_of(arguments)?;
            let note = studio.memory.lock().unwrap().forget(id)?;
            Ok(note_answer("forget", note, "Forgot a note"))
        }
        _ => Err(format!("memory has no action called {action}: remember, change or forget")),
    }
}

fn id_of(arguments: &Value) -> Result<u32, String> {
    let Some(id) = arguments["id"].as_u64() else {
        return Err(String::from("id is the note's number, like 3"));
    };
    match u32::try_from(id) {
        Ok(id) => Ok(id),
        Err(_) => Err(format!("there is no note {id}")),
    }
}

// The answer to a note saved, changed or forgotten: the note, drawn as a
// card in the reply.
fn note_answer(action: &str, note: Note, change: &str) -> ToolAnswer {
    let state = json!({"action": action, "note": note});
    let card = json!({"kind": "memory", "title": "", "action": action, "note": note});
    ToolAnswer::of(state, vec![String::from(change)]).with_card(card)
}

// Where `app` keeps the memory: Music/Jello/Memory.
pub fn folder_of(app: &AppHandle) -> Option<PathBuf> {
    commands::memory_folder(app).ok()
}

// The Memory page asks for the notes, the switch and the skills.
#[tauri::command]
pub fn memory_view(app: AppHandle, memory: State<'_, Mutex<Memory>>) -> Value {
    let mut view = memory.lock().unwrap().view();
    view["skills"] = crate::skills::skills_view(folder_of(&app).as_deref());
    view
}

// The player forgets note `id` on the Memory page.
#[tauri::command]
pub fn forget_memory(app: AppHandle, memory: State<'_, Mutex<Memory>>, id: u32) -> Result<(), String> {
    memory.lock().unwrap().forget(id)?;
    let _ = app.emit("memory-changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_memory_ask_first(app: AppHandle, memory: State<'_, Mutex<Memory>>, ask_first: bool) -> Result<(), String> {
    memory.lock().unwrap().set_ask_first(ask_first)?;
    let _ = app.emit("memory-changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::test_folder;

    #[test]
    fn notes_are_saved_and_load_again() {
        let folder = test_folder("memory-save");
        let mut memory = Memory::load(Some(folder.clone()));
        memory.set_chat(Some("/Music/Jello/Chats/2026-10-24_abc.jsonl"));
        let note = memory.remember("habit", "  Rushes the left hand into bar 3 ").unwrap();
        assert_eq!(note.id, 1);
        assert_eq!(note.text, "Rushes the left hand into bar 3");
        assert_eq!(note.chat.as_deref(), Some("2026-10-24_abc.jsonl"));
        memory.remember("goal", "Play the Steps piece at 90").unwrap();
        memory.set_ask_first(true).unwrap();

        let loaded = Memory::load(Some(folder));
        assert_eq!(loaded.notes().len(), 2);
        assert!(loaded.ask_first);
        assert_eq!(loaded.notes()[1].kind, "goal");
    }

    #[test]
    fn notes_change_and_are_forgotten_by_number() {
        let mut memory = Memory::default();
        memory.remember("taste", "Likes dark pads").unwrap();
        memory.remember("knows", "The C major scale").unwrap();
        let changed = memory.change(1, "Likes dark, slow pads", Some("taste")).unwrap();
        assert_eq!(changed.text, "Likes dark, slow pads");
        let forgotten = memory.forget(2).unwrap();
        assert_eq!(forgotten.text, "The C major scale");
        assert_eq!(memory.notes().len(), 1);
        assert!(memory.forget(2).is_err());
        assert!(memory.change(9, "x", None).is_err());
        // A forgotten number is not given again while a higher one is kept.
        let note = memory.remember("next", "Try wait mode").unwrap();
        assert_eq!(note.id, 2);
    }

    #[test]
    fn wrong_notes_are_refused() {
        let mut memory = Memory::default();
        assert!(memory.remember("mood", "Happy").is_err());
        assert!(memory.remember("habit", "   ").is_err());
        let long = "x".repeat(MOST_NOTE_CHARACTERS + 1);
        assert!(memory.remember("habit", &long).is_err());
        memory.remember("habit", "Holds notes too short").unwrap();
        assert!(memory.change(1, "Fine", Some("mood")).is_err());
        assert_eq!(memory.notes()[0].text, "Holds notes too short");
    }

    #[test]
    fn the_prompt_lists_notes_by_kind() {
        let mut memory = Memory::default();
        assert_eq!(memory.prompt_text(), "");
        memory.remember("goal", "Play hands together").unwrap();
        memory.remember("habit", "Rushes").unwrap();
        let text = memory.prompt_text();
        let date = today();
        assert_eq!(text, format!("Habits:\n- [2] Rushes ({date})\nGoals:\n- [1] Play hands together ({date})"));
    }

    #[test]
    fn a_broken_file_loads_as_empty() {
        let folder = test_folder("memory-broken");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(FILE), "{not json").unwrap();
        let memory = Memory::load(Some(folder));
        assert!(memory.notes().is_empty());
    }

    #[test]
    fn dates_are_counted_from_1970() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(951782400), "2000-02-29");
        assert_eq!(date_of(1792800000), "2026-10-24");
    }
}
