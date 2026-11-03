// The learning path (docs/design.md#ai-and-learning-rethought-2026-11-03):
// each song walks nine stages, from its first idea to its render, and a map
// of topics shows what the player has covered across all songs.
//
// A song keeps whether Learn is on and which stages are done, in its file.
// The current stage is the first one not done. Singing waits for vocals, so
// it is never current. Covered topics belong to the player, not a song, and
// are kept in Music/Jello/Memory/topics.json.

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::song_file::Song;

// Every stage, as the song file names it and as the tape shows it.
pub const STAGES: [(&str, &str); 9] = [
    ("idea", "Idea"),
    ("motif", "Motif"),
    ("chords", "Chords"),
    ("form", "Form"),
    ("arrangement", "Arrangement"),
    ("lyrics", "Lyrics"),
    ("singing", "Singing"),
    ("mix", "Mix"),
    ("render", "Render"),
];

// The stage that waits for vocals.
const WAITS_FOR_VOCALS: &str = "singing";

// The topic map's areas, each with its topics and the stage that teaches
// each one.
pub const AREAS: [(&str, &[(&str, &str)]); 4] = [
    (
        "Theory",
        &[
            ("Scales", "idea"),
            ("Rhythm", "idea"),
            ("Chords", "chords"),
            ("Functions", "chords"),
            ("Cadences", "chords"),
            ("Modes", "chords"),
            ("Voice leading", "arrangement"),
        ],
    ),
    (
        "Composition",
        &[
            ("Motifs", "motif"),
            ("Melody", "motif"),
            ("Song form", "form"),
            ("Tension and release", "form"),
            ("Arranging", "arrangement"),
        ],
    ),
    (
        "Lyrics",
        &[
            ("Theme and story", "lyrics"),
            ("Rhyme", "lyrics"),
            ("Syllable stress", "lyrics"),
            ("Fitting words", "lyrics"),
            ("Singing", "singing"),
        ],
    ),
    (
        "Finishing",
        &[
            ("Sound design", "arrangement"),
            ("Mixing", "mix"),
            ("Loudness", "mix"),
            ("Render", "render"),
        ],
    ),
];

// The file covered topics are kept in, in the memory folder.
const TOPICS_FILE: &str = "topics.json";

// A song's place on the path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SongPath {
    // Whether Learn is on: the AI answers with hints, checks and lessons.
    pub learn: bool,
    // The stages done, by name, in the order of STAGES.
    pub done: Vec<String>,
}

impl SongPath {
    // Marks `stage` done or not done. Fails on a name that is no stage.
    pub fn set_done(&mut self, stage: &str, done: bool) -> Result<(), String> {
        if !is_stage(stage) {
            return Err(format!("there is no stage called {stage}: {}", stage_names()));
        }
        let mut kept = Vec::new();
        for (name, _) in STAGES {
            let was_done = self.done.iter().any(|old| old == name);
            let mut is_done = was_done;
            if name == stage {
                is_done = done;
            }
            if is_done {
                kept.push(String::from(name));
            }
        }
        self.done = kept;
        Ok(())
    }

    pub fn is_done(&self, stage: &str) -> bool {
        self.done.iter().any(|name| name == stage)
    }

    // The first stage not done, skipping singing, or None once all are.
    pub fn current(&self) -> Option<&'static str> {
        for (name, _) in STAGES {
            if name == WAITS_FOR_VOCALS {
                continue;
            }
            if !self.is_done(name) {
                return Some(name);
            }
        }
        None
    }
}

pub fn is_stage(name: &str) -> bool {
    STAGES.iter().any(|(stage, _)| *stage == name)
}

// The stages' names, for an answer that names a wrong one.
fn stage_names() -> String {
    let mut names = Vec::new();
    for (name, _) in STAGES {
        names.push(name);
    }
    names.join(", ")
}

// The stages' file names from a song file's list, skipping any Jello
// doesn't know, in the order of STAGES.
pub fn done_of_names(names: &[String]) -> Vec<String> {
    let mut done = Vec::new();
    for (stage, _) in STAGES {
        if names.iter().any(|name| name == stage) {
            done.push(String::from(stage));
        }
    }
    done
}

// One stage as the tape's strip shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StageView {
    pub id: String,
    pub name: String,
    // "done", "current", "open" or "later" (waiting for vocals).
    pub state: String,
    // What the song has for it, like "4 chords", shown on hover, or "".
    pub note: String,
}

// One topic as the map shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicView {
    pub name: String,
    pub stage: String,
    // "done", "current" or "open".
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AreaView {
    pub name: String,
    pub topics: Vec<TopicView>,
}

// What the stage strip and the topic map show.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PathView {
    pub learn: bool,
    pub song: String,
    pub stages: Vec<StageView>,
    pub current: Option<String>,
    pub areas: Vec<AreaView>,
}

// The path of song `song`, named `name`, with the topics the player has
// covered.
pub fn view_of(name: &str, song: &Song, path: &SongPath, covered: &[String]) -> PathView {
    let current = path.current();
    let mut stages = Vec::new();
    for (id, stage_name) in STAGES {
        let state = stage_state(path, current, id);
        let note = stage_note(song, id, &state);
        stages.push(StageView {
            id: String::from(id),
            name: String::from(stage_name),
            state,
            note,
        });
    }
    PathView {
        learn: path.learn,
        song: String::from(name),
        stages,
        current: current.map(String::from),
        areas: areas_view(current, covered),
    }
}

fn stage_state(path: &SongPath, current: Option<&str>, stage: &str) -> String {
    if stage == WAITS_FOR_VOCALS {
        return String::from("later");
    }
    if path.is_done(stage) {
        return String::from("done");
    }
    if current == Some(stage) {
        return String::from("current");
    }
    String::from("open")
}

// What the song has for a stage, measured, for its hover words.
fn stage_note(song: &Song, stage: &str, state: &str) -> String {
    if stage == WAITS_FOR_VOCALS {
        return String::from("after vocals");
    }
    if state == "current" {
        return String::from("learning");
    }
    match stage {
        "idea" => counted(loop_note_count(song), "note", "notes"),
        "chords" => counted(song.chords.len(), "chord", "chords"),
        "form" => counted(song.sections.len(), "section", "sections"),
        "arrangement" => counted(song.tracks.len(), "track", "tracks"),
        _ => String::new(),
    }
}

fn loop_note_count(song: &Song) -> usize {
    let Some(looped) = &song.looped else {
        return 0;
    };
    let mut count = 0;
    for layer in &looped.layers {
        count += layer.notes.len();
    }
    count
}

// "1 chord", "4 chords", or "" for none.
fn counted(count: usize, one: &str, many: &str) -> String {
    if count == 0 {
        return String::new();
    }
    if count == 1 {
        return format!("1 {one}");
    }
    format!("{count} {many}")
}

// The map: a topic covered is done, and the first not covered in the
// current stage is the one being learned now.
fn areas_view(current: Option<&str>, covered: &[String]) -> Vec<AreaView> {
    let now = topic_now(current, covered);
    let mut areas = Vec::new();
    for (area, topics) in AREAS {
        let mut views = Vec::new();
        for (topic, stage) in topics.iter() {
            let mut state = String::from("open");
            if covered.iter().any(|name| name == topic) {
                state = String::from("done");
            } else if now == Some(*topic) {
                state = String::from("current");
            }
            views.push(TopicView {
                name: String::from(*topic),
                stage: String::from(*stage),
                state,
            });
        }
        areas.push(AreaView {
            name: String::from(area),
            topics: views,
        });
    }
    areas
}

fn topic_now(current: Option<&str>, covered: &[String]) -> Option<&'static str> {
    let stage = current?;
    for (_, topics) in AREAS {
        for (topic, topic_stage) in topics.iter() {
            if *topic_stage != stage {
                continue;
            }
            if !covered.iter().any(|name| name == topic) {
                return Some(topic);
            }
        }
    }
    None
}

pub fn is_topic(name: &str) -> bool {
    for (_, topics) in AREAS {
        if topics.iter().any(|(topic, _)| *topic == name) {
            return true;
        }
    }
    false
}

fn topic_names() -> String {
    let mut names = Vec::new();
    for (_, topics) in AREAS {
        for (topic, _) in topics.iter() {
            names.push(*topic);
        }
    }
    names.join(", ")
}

// The topics covered, kept in `folder`. A missing or broken file covers
// none, and names Jello doesn't know are left out.
pub fn covered_in(folder: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(folder.join(TOPICS_FILE)) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(list) = value.get("covered").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut covered = Vec::new();
    for item in list {
        let Some(name) = item.as_str() else {
            continue;
        };
        if is_topic(name) && !covered.iter().any(|old: &String| old == name) {
            covered.push(String::from(name));
        }
    }
    covered
}

// Marks `topic` covered or not, in `folder`. Returns the topics covered
// after.
pub fn set_covered(folder: &Path, topic: &str, covered: bool) -> Result<Vec<String>, String> {
    if !is_topic(topic) {
        return Err(format!("there is no topic called {topic}: {}", topic_names()));
    }
    let mut list = covered_in(folder);
    list.retain(|name| name != topic);
    if covered {
        list.push(String::from(topic));
    }
    fs::create_dir_all(folder).map_err(|error| format!("could not make the memory folder: {error}"))?;
    let text = json!({ "covered": list }).to_string();
    fs::write(folder.join(TOPICS_FILE), text).map_err(|error| format!("could not save the topics: {error}"))?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_profile::SoundProfile;

    fn path_with(done: &[&str]) -> SongPath {
        let mut path = SongPath::default();
        for stage in done {
            path.set_done(stage, true).unwrap();
        }
        path
    }

    fn states(view: &PathView) -> Vec<String> {
        let mut states = Vec::new();
        for stage in &view.stages {
            states.push(stage.state.clone());
        }
        states
    }

    #[test]
    fn a_new_song_starts_at_the_idea() {
        let path = SongPath::default();
        assert_eq!(path.current(), Some("idea"));
        let view = view_of("Night Drive", &Song::new(&SoundProfile::new()), &path, &[]);
        assert_eq!(view.stages.len(), 9);
        assert_eq!(view.stages[0].state, "current");
        assert_eq!(view.stages[0].note, "learning");
        assert_eq!(view.stages[6].state, "later");
        assert_eq!(view.stages[6].note, "after vocals");
        assert!(!view.learn);
    }

    #[test]
    fn the_current_stage_is_the_first_not_done() {
        let path = path_with(&["idea", "motif"]);
        assert_eq!(path.current(), Some("chords"));
        let view = view_of("", &Song::new(&SoundProfile::new()), &path, &[]);
        let expected = ["done", "done", "current", "open", "open", "open", "later", "open", "open"];
        assert_eq!(states(&view), expected);
    }

    #[test]
    fn a_stage_can_be_done_out_of_order_and_undone() {
        let mut path = path_with(&["mix", "idea"]);
        assert_eq!(path.done, ["idea", "mix"]);
        assert_eq!(path.current(), Some("motif"));
        path.set_done("idea", false).unwrap();
        assert_eq!(path.done, ["mix"]);
        assert_eq!(path.current(), Some("idea"));
    }

    #[test]
    fn marking_twice_keeps_one() {
        let path = path_with(&["form", "form"]);
        assert_eq!(path.done, ["form"]);
    }

    #[test]
    fn singing_is_skipped_until_vocals() {
        let path = path_with(&["idea", "motif", "chords", "form", "arrangement", "lyrics"]);
        assert_eq!(path.current(), Some("mix"));
        let path = path_with(&["idea", "motif", "chords", "form", "arrangement", "lyrics", "mix", "render"]);
        assert_eq!(path.current(), None);
    }

    #[test]
    fn a_wrong_stage_changes_nothing() {
        let mut path = path_with(&["idea"]);
        let why = path.set_done("bridge", true).unwrap_err();
        assert!(why.contains("bridge"));
        assert!(why.contains("render"));
        assert_eq!(path.done, ["idea"]);
    }

    #[test]
    fn file_names_jello_does_not_know_are_skipped() {
        let names = vec![String::from("render"), String::from("nope"), String::from("idea")];
        assert_eq!(done_of_names(&names), ["idea", "render"]);
    }

    #[test]
    fn notes_count_what_the_song_has() {
        let mut song = Song::new(&SoundProfile::new());
        let path = path_with(&["idea", "motif", "chords", "form", "arrangement"]);
        let view = view_of("", &song, &path, &[]);
        assert_eq!(view.stages[2].note, "");
        assert_eq!(view.stages[4].note, "1 track");

        let chord = song_chord();
        song.chords = vec![chord.clone(), chord.clone(), chord];
        let view = view_of("", &song, &path, &[]);
        assert_eq!(view.stages[2].note, "3 chords");
    }

    fn song_chord() -> crate::song_file::HeldChord {
        crate::song_file::HeldChord {
            chord: crate::harmony::chord_of_name("Am").unwrap(),
            held_ms: 500,
            notes: None,
        }
    }

    #[test]
    fn the_map_marks_covered_and_the_topic_now() {
        let path = path_with(&["idea", "motif"]);
        let covered = vec![String::from("Chords"), String::from("Scales")];
        let view = view_of("", &Song::new(&SoundProfile::new()), &path, &covered);
        let theory = &view.areas[0];
        assert_eq!(theory.name, "Theory");
        assert_eq!(theory.topics[0].state, "done");
        assert_eq!(theory.topics[2].state, "done");
        assert_eq!(theory.topics[3].name, "Functions");
        assert_eq!(theory.topics[3].state, "current");
        assert_eq!(theory.topics[4].state, "open");
        let mut current = 0;
        for area in &view.areas {
            for topic in &area.topics {
                if topic.state == "current" {
                    current += 1;
                }
            }
        }
        assert_eq!(current, 1);
    }

    #[test]
    fn every_topic_belongs_to_a_stage() {
        for (_, topics) in AREAS {
            for (_, stage) in topics.iter() {
                assert!(is_stage(stage));
            }
        }
    }

    fn scratch_folder(label: &str) -> std::path::PathBuf {
        let folder = std::env::temp_dir().join(format!("jello-path-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn covered_topics_are_kept_in_the_memory_folder() {
        let folder = scratch_folder("kept");
        assert!(covered_in(&folder).is_empty());
        set_covered(&folder, "Rhyme", true).unwrap();
        set_covered(&folder, "Modes", true).unwrap();
        set_covered(&folder, "Rhyme", true).unwrap();
        assert_eq!(covered_in(&folder), ["Modes", "Rhyme"]);
        set_covered(&folder, "Modes", false).unwrap();
        assert_eq!(covered_in(&folder), ["Rhyme"]);
        let _ = fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_wrong_topic_or_a_broken_file_covers_nothing() {
        let folder = scratch_folder("broken");
        assert!(set_covered(&folder, "Juggling", true).unwrap_err().contains("Juggling"));
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(TOPICS_FILE), "not json").unwrap();
        assert!(covered_in(&folder).is_empty());
        fs::write(folder.join(TOPICS_FILE), r#"{"covered": ["Rhyme", 4, "Juggling", "Rhyme"]}"#).unwrap();
        assert_eq!(covered_in(&folder), ["Rhyme"]);
        let _ = fs::remove_dir_all(&folder);
    }
}
