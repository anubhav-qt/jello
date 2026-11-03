// The learning path's tool and commands (src/path.rs): jello_path for the
// agent, and what the tape's stage strip, the topic map and the Learn
// switch ask Rust for.

use std::sync::Mutex;

use serde_json::{Value, json};
use tauri::State;

use crate::compose::Composer;
use crate::engine::Engine;
use crate::memory::Memory;
use crate::path::{self, PathView};
use crate::song_session::{OpenSong, song_now};
use crate::tools::{Studio, ToolAnswer, text_argument};

// The actions jello_path takes.
pub const ACTIONS: [&str; 4] = ["look", "learn", "stage", "topic"];

// The tool as MCP's tools/list describes it.
pub fn description() -> Value {
    json!({
        "name": "jello_path",
        "description": "The learning path of the song open now. Each song walks nine stages: idea, \
            motif, chords, form, arrangement, lyrics, singing, mix, render. The current stage is the \
            first not done; singing waits for vocals and is never current. Learn is a switch per \
            song: on, answer with hints, checks and short lessons on the player's own song, and make \
            a take only when told to do it; off, make takes as usual. The topic map (Theory, \
            Composition, Lyrics, Finishing) shows the topics the player has covered across all \
            songs, each taught at one stage. look: Learn, every stage with its state and what the \
            song has for it, and every topic with its state. learn: with on, turns Learn on or off. \
            stage: with stage (a name) and done, marks it done or not, once the player has passed \
            its task. topic: with topic (a name, like \"Functions\") and covered, marks it covered \
            or not. Answers with the whole path.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ACTIONS},
                "on": {"type": "boolean"},
                "stage": {"type": "string", "enum": stage_ids()},
                "done": {"type": "boolean"},
                "topic": {"type": "string"},
                "covered": {"type": "boolean"},
            },
            "required": ["action"],
        },
    })
}

fn stage_ids() -> Vec<&'static str> {
    let mut ids = Vec::new();
    for (id, _) in path::STAGES {
        ids.push(id);
    }
    ids
}

// A flag the action needs, like `on` for learn.
fn needed_flag(arguments: &Value, name: &str) -> Result<bool, String> {
    match arguments.get(name).and_then(Value::as_bool) {
        Some(flag) => Ok(flag),
        None => Err(format!("{name} is needed, as true or false")),
    }
}

// jello_path: one change, or none for look, then the whole path.
pub fn answer(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    let change = match action.as_str() {
        "look" => None,
        "learn" => {
            let on = needed_flag(arguments, "on")?;
            studio.composer.lock().unwrap().path_mut().learn = on;
            Some(learn_words(on))
        }
        "stage" => {
            let stage = text_argument(arguments, "stage")?;
            let done = needed_flag(arguments, "done")?;
            studio.composer.lock().unwrap().path_mut().set_done(&stage, done)?;
            Some(stage_words(&stage, done))
        }
        "topic" => {
            let topic = text_argument(arguments, "topic")?;
            let covered = needed_flag(arguments, "covered")?;
            let memory = studio.memory.lock().unwrap();
            let Some(folder) = memory.folder() else {
                return Err(String::from("there is no memory folder to keep topics in"));
            };
            path::set_covered(folder, &topic, covered)?;
            Some(topic_words(&topic, covered))
        }
        _ => return Err(format!("jello_path's actions are {}", ACTIONS.join(", "))),
    };

    let view = view_now(studio.open_song, studio.composer, studio.engine, studio.memory);
    let state = serde_json::to_value(&view).unwrap_or(Value::Null);
    let mut changes = Vec::new();
    if let Some(change) = change {
        changes.push(change);
    }
    Ok(ToolAnswer::of(state, changes))
}

fn learn_words(on: bool) -> String {
    if on {
        return String::from("Learn on");
    }
    String::from("Learn off")
}

fn stage_words(stage: &str, done: bool) -> String {
    if done {
        return format!("Stage {stage} done");
    }
    format!("Stage {stage} not done")
}

fn topic_words(topic: &str, covered: bool) -> String {
    if covered {
        return format!("Topic {topic} covered");
    }
    format!("Topic {topic} not covered")
}

// The path as it is now. Locks the open song, Compose and the engine in
// order, then the memory alone.
fn view_now(
    open_song: &Mutex<OpenSong>,
    composer: &Mutex<Composer>,
    engine: &Mutex<Engine>,
    memory: &Mutex<Memory>,
) -> PathView {
    let name;
    let song;
    let song_path;
    {
        let open_song = open_song.lock().unwrap();
        let composer = composer.lock().unwrap();
        let mut engine = engine.lock().unwrap();
        name = String::from(open_song.name());
        song = song_now(&composer, &mut engine);
        song_path = composer.path().clone();
    }
    let covered = covered_now(memory);
    path::view_of(&name, &song, &song_path, &covered)
}

fn covered_now(memory: &Mutex<Memory>) -> Vec<String> {
    let memory = memory.lock().unwrap();
    match memory.folder() {
        Some(folder) => path::covered_in(folder),
        None => Vec::new(),
    }
}

// What the stage strip and the topic map show.
#[tauri::command]
pub fn path_view(
    open_song: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    memory: State<'_, Mutex<Memory>>,
) -> PathView {
    view_now(&open_song, &composer, &engine, &memory)
}

// The Learn switch in the rail.
#[tauri::command]
pub fn set_learn(
    open_song: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    memory: State<'_, Mutex<Memory>>,
    on: bool,
) -> PathView {
    composer.lock().unwrap().path_mut().learn = on;
    view_now(&open_song, &composer, &engine, &memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_say_what_changed() {
        assert_eq!(learn_words(true), "Learn on");
        assert_eq!(learn_words(false), "Learn off");
        assert_eq!(stage_words("chords", true), "Stage chords done");
        assert_eq!(stage_words("chords", false), "Stage chords not done");
        assert_eq!(topic_words("Rhyme", true), "Topic Rhyme covered");
    }

    #[test]
    fn the_schema_lists_every_stage() {
        let stages = description()["inputSchema"]["properties"]["stage"]["enum"].clone();
        assert_eq!(stages.as_array().unwrap().len(), path::STAGES.len());
    }

    #[test]
    fn a_flag_must_be_true_or_false() {
        assert!(needed_flag(&json!({"on": true}), "on").unwrap());
        assert!(needed_flag(&json!({"on": "yes"}), "on").unwrap_err().contains("true or false"));
        assert!(needed_flag(&json!({}), "on").is_err());
    }
}
