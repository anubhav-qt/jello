// The agent's tools (docs/adr/0041-one-agent-for-every-mode.md,
// docs/design.md#the-agent-harness). Each does what the player can do,
// through the same code the UI's commands call, and answers with the fresh
// state of its area as JSON. The tool server hands them to the agent.
//
// Tools lock what they need in the same order as the commands: the open
// song, the key map, Compose, the engine. So a tool and a command can
// never end up waiting on each other.

use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

use crate::cards::{self, Offers};
use crate::commands::sound_view_of;
use crate::compose::Composer;
use crate::engine::{Engine, FASTEST_TEMPO, FULL_VOLUME, SLOWEST_TEMPO};
use crate::harmony::{key_name, key_of_name};
use crate::key_map::KeyMap;
use crate::layer_notes;
use crate::listening;
use crate::memory::{self, Memory};
use crate::phrases::{self, Phrase, Phrases};
use crate::practice::{self, Practice};
use crate::presets;
use crate::skills;
use crate::song_file;
use crate::song_session::{self, OpenSong, song_now, songs_view_of};
use crate::sound_changes;
use crate::sound_profile::SoundProfile;
use crate::theory;

// Everything the tools reach: what the app keeps for its commands, the
// phrase playing, the agent's offers, and the screen.
pub struct Studio<'a> {
    pub open_song: &'a Mutex<OpenSong>,
    pub key_map: &'a Mutex<KeyMap>,
    pub composer: &'a Mutex<Composer>,
    pub engine: &'a Mutex<Engine>,
    pub phrases: &'a Phrases,
    pub offers: &'a Mutex<Offers>,
    pub practice: &'a Mutex<Practice>,
    pub memory: &'a Mutex<Memory>,
    pub screen: &'a dyn Screen,
}

// What the tools need from the screen.
pub trait Screen {
    // Asks the player a yes-or-no question in the agent's panel, and waits
    // for the answer.
    fn ask(&self, question: &str) -> bool;

    // Lights laptop key `key` on the keyboard while a phrase holds it down.
    fn show_key(&self, key: &str, down: bool);

    // Marks `keys` on the keyboard, each a laptop key and a label, as a keys
    // card shows them.
    fn show_marks(&self, _keys: &Value) {}

    // Says a phrase starts playing, so a lane card can move its playhead.
    fn show_phrase(&self, _phrase: &Phrase) {}

    // Shows practice's view on the Play tab (practice.rs).
    fn show_practice(&self, _view: &Value) {}

    // Starts a try of the piece set (practice_run.rs).
    fn start_practice(&self) -> Result<(), String> {
        Err(String::from("practice can't start here"))
    }
}

// What a tool answers: its text for the agent, a line for the panel for
// each thing it changed, like "Filter cutoff 1200 -> 800 Hz", and maybe a
// card for the panel to draw (cards.rs). A failed call's text says why, and
// it changed nothing.
#[derive(Debug, PartialEq)]
pub struct ToolAnswer {
    pub text: String,
    pub changes: Vec<String>,
    pub failed: bool,
    pub card: Option<Value>,
}

impl ToolAnswer {
    pub(crate) fn of(state: Value, changes: Vec<String>) -> ToolAnswer {
        ToolAnswer {
            text: state.to_string(),
            changes,
            failed: false,
            card: None,
        }
    }

    pub(crate) fn failed(why: String) -> ToolAnswer {
        ToolAnswer {
            text: why,
            changes: Vec::new(),
            failed: true,
            card: None,
        }
    }

    pub(crate) fn with_card(mut self, card: Value) -> ToolAnswer {
        self.card = Some(card);
        self
    }
}

// The parts jello_look can read.
const LOOK_PARTS: [&str; 10] = [
    "sound", "keys", "song", "looper", "compose", "presses", "presets", "songs", "skills", "memory",
];

// Every tool, as MCP's tools/list names and describes them.
pub fn list() -> Value {
    json!([
        {
            "name": "jello_look",
            "description": "Reads any parts of what Jello is doing now. sound: every sound value, as \
                jello_sound takes them, and the preset it sounds exactly like. keys: what each laptop \
                key plays at the current octave. song: the open song's name and its whole song file. \
                looper: what the looper is doing, its layers and their notes in beats, the tempo and \
                snap. compose: Compose's key, chords, guidance and history. presses: the player's \
                recent presses. presets: every preset and the starting sound. songs: every song and \
                the one open. skills: each skill area's level out of 10 (the median of its last 5 \
                tries), trend and recent points, measured from every practice try. memory: your \
                notes about the player, and whether you ask before saving one. With no parts it \
                reads them all.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "parts": {
                        "type": "array",
                        "items": {"type": "string", "enum": LOOK_PARTS},
                    },
                },
            },
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "jello_sound",
            "description": "Changes the song's sound for notes played from now on, naming only what \
                changes, with the names jello_look's sound uses. Example: {\"changes\": {\"filter\": \
                {\"cutoff_hz\": 800}, \"oscillators\": [{\"number\": 2, \"kind\": \"saw\"}]}}. \
                Oscillators (1 to 3), LFOs (1 to 8) and connections (1 to 64) are named by their \
                number. A connection with source \"none\" is removed; an empty one needs a source \
                and a target. A value outside its range, or a name Jello doesn't know, is refused \
                with what is allowed, and then nothing changes. Answers with what changed and the \
                whole sound.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "changes": {"type": "object"},
                },
                "required": ["changes"],
            },
        },
        {
            "name": "jello_play",
            "description": "Plays a timed phrase of key presses through the same path as the \
                player's keys, so the keys light, the looper records and Compose follows. Each \
                press names a key (\"KeyC\", \"ShiftLeft\" for the vibrato key, \"AltLeft\" for \
                glide), a note (\"E4\", found on the melody keys at the current octave) or a chord \
                (\"Am\", found on the chord keys), with its start and hold in the phrase's unit: \
                beats at the song's tempo (the default) or ms. Example: {\"presses\": [{\"chord\": \
                \"Am\", \"start\": 0, \"hold\": 2}, {\"note\": \"E4\", \"start\": 0, \"hold\": \
                1}]}. A phrase lasts at most 60 s, one at a time; the player's Stop ends it. Answers \
                when it ends, with what was played and what the looper caught.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "unit": {"type": "string", "enum": ["beats", "ms"]},
                    "presses": presses_schema(),
                },
                "required": ["presses"],
            },
        },
        {
            "name": "jello_looper",
            "description": "Runs the looper, as the player does. press: what Right Shift does: \
                when idle it counts in a bar and records; while recording it closes the loop at the \
                nearest bar line; while looping it records a new layer over it, and pressed again \
                that layer starts playing. pause and play: the whole loop; play starts it from beat \
                1. snap: whether loops snap to the beat (on: true or false). tempo: the song's beats \
                per minute, 40 to 240, not while the looper is busy. layer_volume: a layer's volume \
                in percent (layer, volume). layer_playing: pauses or plays a layer (layer, playing). \
                remove_layer: removes a layer, or with no layer the newest. Layers count from 1, \
                oldest first. Answers with what the looper is doing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["press", "pause", "play", "snap", "tempo", "layer_volume", "layer_playing", "remove_layer"],
                    },
                    "on": {"type": "boolean"},
                    "tempo": {"type": "integer"},
                    "layer": {"type": "integer"},
                    "volume": {"type": "integer"},
                    "playing": {"type": "boolean"},
                },
                "required": ["action"],
            },
        },
        {
            "name": "jello_song",
            "description": "Changes the song, as Compose does. key: picks the key (key, like \
                \"A minor\" or \"Db major\"). add_last: adds the chord played last to the end \
                of the song, keeping how long it was held; play its key with jello_play first. \
                add_tried: adds the tried chord numbered chord from Compose's history. remove: \
                removes chord number chord. move: moves chord number chord one place (direction \
                left or right). play: plays the song from its first chord. play_chord: plays chord \
                number chord. stop: stops the song. open: opens the song called name, after saving \
                the one open. new: makes a new song on the starting sound. rename: renames the \
                open song to name. Chords count from 1. Answers with the song's name and Compose.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["key", "add_last", "add_tried", "remove", "move", "play", "play_chord", "stop", "open", "new", "rename"],
                    },
                    "key": {"type": "string"},
                    "chord": {"type": "integer"},
                    "direction": {"type": "string", "enum": ["left", "right"]},
                    "name": {"type": "string"},
                },
                "required": ["action"],
            },
        },
        {
            "name": "jello_presets",
            "description": "Works with presets, the sounds kept in Music/Jello/Sounds. list: every \
                preset and the starting sound. load: makes preset name the song's sound. save: \
                saves the song's sound as preset name; saving over one that exists asks the player \
                first. starting: marks preset name as the sound new songs start on, or with no name \
                unmarks it; this always asks the player first. A player's No changes nothing. \
                Answers with every preset and the one the song sounds like.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list", "load", "save", "starting"]},
                    "name": {"type": "string"},
                },
                "required": ["action"],
            },
        },
        {
            "name": "jello_listen",
            "description": "Renders silently on a second engine and measures the render, without \
                touching what plays live. what: loop (once through, from its start), chords (the \
                song's chords, each as long as it was held) or phrase (presses and unit as for \
                jello_play, one you are thinking of playing; the wave and looper keys do nothing \
                here). with_loop: true plays the loop under the chords or the phrase. Answers for \
                the whole render and beat by beat: loudness_db (RMS), peak_db, soft_clip_reached \
                (a sample past 0.8, where the soft clip bends them), brightness_hz (the spectral \
                centroid), lows, mids and highs percent of the power (below 250 Hz, to 4000 Hz, \
                above), width_percent (0 the same on both sides, 50 one side only, 100 opposite) \
                and balance_db (positive when the left is louder); silences of 0.2 s or more, in \
                beats from 1; and rings_on_ms, how long sound goes on after the end. A render \
                lasts at most 60 s. save_as also saves it, ringing included, as a WAV in \
                Music/Jello/Renders under a new name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "what": {"type": "string", "enum": ["loop", "chords", "phrase"]},
                    "with_loop": {"type": "boolean"},
                    "unit": {"type": "string", "enum": ["beats", "ms"]},
                    "presses": presses_schema(),
                    "save_as": {"type": "string"},
                },
                "required": ["what"],
            },
        },
        {
            "name": "jello_show",
            "description": "Puts a card in your reply, drawn by Jello: show instead of telling. \
                Every card has a kind and may have a title (80 characters). keys: marks keys on \
                the real keyboard, numbered or labelled (keys: [{key|note|chord, label}], label \
                at most 4 characters), cleared when the player next writes. lane: notes over \
                beats in 1 to 4 parts, like two hands, at any pitch (parts: [{name, notes: \
                [{note|chord, start, hold}]}], at most 512 notes and 256 beats); it scrolls as \
                jello_play plays it. progression: chords with their roman numerals in the key and \
                how each fits (chords: [\"Am\", \"F\"], key: optional, the song's by default). \
                compare: 2 to 4 takes or choices side by side (items: [{name, measures: [{name, \
                value}]}], up to 8 measures each). chart: a skill over time (unit, points: \
                [{label, value}], 2 to 64). sound: the song's sound drawn, or with changes (as \
                jello_sound takes them) the sound they would make over it; nothing changes. A \
                wrong field is refused with what is allowed. Answers only that it was shown.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "card": {
                        "type": "object",
                        "properties": {
                            "kind": {"type": "string", "enum": cards::CARD_KINDS},
                            "title": {"type": "string"},
                        },
                        "required": ["kind"],
                    },
                },
                "required": ["card"],
            },
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "jello_options",
            "description": "Offers the player 2 to 4 choices, each with Play and Use buttons, \
                instead of describing them. Every option has a name (40 characters). kind \
                chords: each option has chords: [\"Am\", {\"chord\": \"F\", \"hold\": 4}], \
                2 beats each by default; Use adds them to the end of the song. kind melody: each \
                option has presses as jello_play takes them, in beats, naming notes or chords; \
                Use writes it as a new loop layer. kind sound: each option has changes as \
                jello_sound takes them, and maybe presses to play it with; Use makes it the \
                song's sound. Every option must play on the keys at this octave. Nothing changes \
                until the player picks Use, and each Use can be undone. Answers with the offer's \
                number; a later message says if the player used one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": cards::OPTION_KINDS},
                    "title": {"type": "string"},
                    "options": {"type": "array", "items": {"type": "object"}},
                },
                "required": ["kind", "options"],
            },
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "jello_theory",
            "description": "Works out music theory exactly; ask it instead of counting half \
                steps. question chord: the chords notes make, with inversions (notes: [\"E4\", \
                \"G4\", \"C5\"]). numerals: chords as roman numerals in a key, the song's by \
                default (chords: [\"Am\", \"F\", \"G7\"], key). scale: a scale or mode's notes \
                spelled, and for seven notes the chord and seventh chord on each degree (tonic: \
                \"D\", scale: major, minor, dorian, phrygian, lydian, mixolydian, aeolian, \
                locrian, ionian, harmonic minor, melodic minor, major pentatonic, minor \
                pentatonic, blues, whole tone). interval: from a note to another (from, to), or \
                the note half_steps away (from, half_steps). voice_leading: how each note moves \
                to the nearest of the next chord (from, to: a chord's name or notes). keys: \
                which laptop key plays notes or a chord at this octave, and for a note out of \
                reach how many octaves to move (notes, or chord). Chord names: a root, then \
                nothing, m, dim, aug, sus2, sus4, 5, 6, m6, 7, maj7, m7, mMaj7, m7b5, dim7, 7sus4, \
                add9, madd9, 9, maj9 or m9.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": {"type": "string", "enum": theory::QUESTIONS},
                    "notes": {"type": "array", "items": {"type": "string"}},
                    "chords": {"type": "array", "items": {"type": "string"}},
                    "chord": {"type": "string"},
                    "key": {"type": "string"},
                    "tonic": {"type": "string"},
                    "scale": {"type": "string"},
                    "from": {},
                    "to": {},
                    "half_steps": {"type": "integer"},
                },
                "required": ["question"],
            },
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "jello_notes",
            "description": "Writes a loop layer's notes directly, at exact times and any pitch, \
                not only what the keys reach. notes: [{note|chord, start, hold}] in beats from \
                the layer's start, a chord giving its three notes, at most 2048. layer: the layer \
                to replace, from 1; without it a new layer goes on top (8 at most). bars: how \
                long the layer lasts, 1 to 64, by default what its notes need, or a replaced \
                layer's own. volume: percent. With no loop yet, the layer becomes the loop. \
                Refused while the looper records. Answers with the loop as written.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "layer": {"type": "integer"},
                    "bars": {"type": "integer"},
                    "volume": {"type": "integer"},
                    "notes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "note": {"type": "string"},
                                "chord": {"type": "string"},
                                "start": {"type": "number"},
                                "hold": {"type": "number"},
                            },
                            "required": ["start", "hold"],
                        },
                    },
                },
                "required": ["notes"],
            },
        },
        {
            "name": "jello_practice",
            "description": "Sets a piece for the player to practice on the Play tab, with its lane \
                and the next keys marked on the keyboard, and scores every try in Jello. action \
                set: piece {name, area (the skill its tries count toward: technique|rhythm| \
                harmony|ear|improvising|pieces; by default the mode's: ear, improvising, or \
                pieces), octave (the keys' octave at the start, -4 to 4), parts: 1 to 4 \
                of {name, hand: left|right|jello, notes: [{note | chord | notes, start, hold, \
                octave}]}, sections: [{name, from_bar, to_bar}]}, and settings. A note is a \
                melody note (\"E4\"), a chord key's major or minor chord (\"Am\", at the octave \
                in effect: C3 to B3 roots at octave 0), or notes played at once on the melody keys \
                ([\"C4\", \"E4\"]). start and hold are beats: numbers or fractions like \"1/3\" or \
                \"2 1/3\". octave on a note moves the keys there before it, for the rest of the \
                piece; the melody keys reach G#3 to G5 at octave 0, and a note out of reach is \
                refused. 4 beats a bar, at most 64 bars and 2048 notes. Jello plays the jello \
                parts, and the player's other hand when hands is left or right. action settings: \
                settings {tempo, swing 50 to 75 (50 straight), count_in 0 to 2 bars, click, wait \
                (no timing: each step waits for its notes), section (a name, or null), loop (try \
                after try until stopped), ladder {step, clean, top} or null (faster by step after \
                clean tries in a row, up to top), hands both|left|right, mode along|ear|improvise| \
                memory, coach (each try goes to you by itself), level beginner|novice| \
                intermediate|advanced|expert|master (the window: 150, 120, 90, 65, 50, 40 ms)}. \
                ear: Jello plays the player's parts, the player plays them back. improvise: Jello \
                plays its parts and the player's notes are measured against them. memory: one \
                listen, then the lane hides. action start, stop: a try. action results: every try. \
                action close: ends practice. Results come back in the player's next message as \
                (Jello: practice try N ended: ...).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["set", "settings", "start", "stop", "results", "close"]},
                    "piece": {"type": "object"},
                    "settings": {"type": "object"},
                },
                "required": ["action"],
            },
        },
        {
            "name": "jello_memory",
            "description": "Keeps short notes about the player across chats. action remember: kind \
                habit|taste|knows|goal|next (something they tend to do, something they like, \
                something they know, what they want to reach, what to do next time) and text, \
                one plain sentence. action change: id, text, and kind if it changes. action \
                forget: id. Notes reach the next chat's instructions; jello_look memory reads them \
                now. The player sees each note saved and can forget any of them. If they turned \
                on asking first, Jello asks them before saving.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["remember", "change", "forget"]},
                    "kind": {"type": "string", "enum": memory::KINDS},
                    "text": {"type": "string"},
                    "id": {"type": "integer"},
                },
                "required": ["action"],
            },
        },
    ])
}

// A phrase's presses, as jello_play and jello_listen take them.
fn presses_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "key": {"type": "string"},
                "note": {"type": "string"},
                "chord": {"type": "string"},
                "start": {"type": "number"},
                "hold": {"type": "number"},
            },
            "required": ["start", "hold"],
        },
    })
}

// True for the tools that can change something, so the UI redraws after
// them. jello_listen renders on an engine of its own, and cards and options
// change nothing until the player picks Use.
pub fn changes_something(name: &str) -> bool {
    let only_look = ["jello_look", "jello_listen", "jello_show", "jello_options", "jello_theory"];
    !only_look.contains(&name)
}

// Calls tool `name` with `arguments`.
pub fn call(studio: &Studio, name: &str, arguments: &Value) -> ToolAnswer {
    let answer = match name {
        "jello_look" => look(studio, arguments),
        "jello_sound" => change_sound(studio, arguments),
        "jello_play" => play_phrase(studio, arguments),
        "jello_looper" => run_looper(studio, arguments),
        "jello_song" => change_song(studio, arguments),
        "jello_presets" => use_presets(studio, arguments),
        "jello_listen" => listen(studio, arguments),
        "jello_show" => cards::show(studio, arguments),
        "jello_options" => cards::offer(studio, arguments),
        "jello_notes" => write_notes(studio, arguments),
        "jello_theory" => theory::answer(studio, arguments),
        "jello_practice" => practice::answer(studio, arguments),
        "jello_memory" => memory::answer(studio, arguments),
        _ => Err(format!("Jello has no tool called {name}")),
    };
    match answer {
        Ok(answer) => answer,
        Err(why) => ToolAnswer::failed(why),
    }
}

// jello_look: the parts asked for, or all of them.
fn look(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let parts = parts_asked(arguments)?;
    let now = Instant::now();
    let open_song = studio.open_song.lock().unwrap();
    let key_map = studio.key_map.lock().unwrap();
    let composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();

    let mut state = serde_json::Map::new();
    let mut card = None;
    for part in parts {
        let value = match part.as_str() {
            "sound" => json!({
                "preset": composer.lit_preset(),
                "values": sound_changes::json_of(&composer.sound_profile().sound()),
            }),
            "keys" => to_json(&key_map.layout_for_ui()),
            "song" => {
                let song = song_now(&composer, &mut engine);
                json!({
                    "name": open_song.name(),
                    "file": song_file::text_of(&song),
                })
            }
            "looper" => looper_json(&mut engine, &composer, now),
            // The recent presses are their own part.
            "compose" => compose_json(&composer, &key_map, &engine, now),
            "presses" => {
                let view = to_json(&composer.view(&key_map, engine.song_place(), now));
                // The panel draws them as a strip with their gaps.
                card = Some(cards::presses_card(&view["performance"], composer.key()));
                view["performance"].clone()
            }
            "presets" => match open_song.folders() {
                Ok(folders) => {
                    let starting = presets::starting_name(&folders.settings);
                    to_json(&presets::list(&folders.sounds, starting.as_deref()))
                }
                Err(why) => json!(why),
            },
            "songs" => to_json(&songs_view_of(&open_song)),
            // The memory is locked alone, after the rest.
            "skills" => {
                let memory = studio.memory.lock().unwrap();
                skills::skills_view(memory.folder())
            }
            "memory" => studio.memory.lock().unwrap().view(),
            _ => Value::Null,
        };
        state.insert(part, value);
    }
    let answer = ToolAnswer::of(Value::Object(state), Vec::new());
    match card {
        Some(card) => Ok(answer.with_card(card)),
        None => Ok(answer),
    }
}

// The parts jello_look's `arguments` ask for, or every part.
fn parts_asked(arguments: &Value) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let Some(asked) = arguments.get("parts") else {
        for part in LOOK_PARTS {
            parts.push(String::from(part));
        }
        return Ok(parts);
    };
    let all = LOOK_PARTS.join(", ");
    let Some(asked) = asked.as_array() else {
        return Err(format!("parts is a list of {all}"));
    };
    for part in asked {
        let Some(part) = part.as_str() else {
            return Err(format!("parts is a list of {all}"));
        };
        if !LOOK_PARTS.contains(&part) {
            return Err(format!("there is no part called {part}: there are {all}"));
        }
        parts.push(String::from(part));
    }
    if parts.is_empty() {
        for part in LOOK_PARTS {
            parts.push(String::from(part));
        }
    }
    Ok(parts)
}

// jello_sound: the changes in `arguments`, made to the song's sound.
fn change_sound(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let Some(changes) = arguments.get("changes") else {
        return Err(String::from("jello_sound needs changes, like {\"changes\": {\"filter\": {\"cutoff_hz\": 800}}}"));
    };
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();

    let before = composer.sound_profile().sound();
    let wanted = sound_changes::changed(&before, changes)?;
    sound_changes::take_changes(composer.sound_profile_mut(), &mut engine, &before, &wanted);

    // The lines say what the sound became, which is what the engine took.
    let after = composer.sound_profile().sound();
    let lines = sound_changes::change_lines(&before, &after);
    let state = json!({
        "changed": lines,
        "preset": composer.lit_preset(),
        "values": sound_changes::json_of(&after),
    });
    // The panel draws the change: the sound before, dashed, under the new.
    let card = cards::sound_change_card(&before, &after, &lines);
    Ok(ToolAnswer::of(state, lines).with_card(card))
}

// jello_play: the phrase in `arguments`, played to its end or until Stop.
fn play_phrase(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let key_map = studio.key_map.lock().unwrap();
    let composer = studio.composer.lock().unwrap();
    let phrase = phrases::phrase_of(arguments, &key_map, composer.tempo())?;
    drop(composer);
    drop(key_map);

    let played = phrases::play(studio, &phrase)?;
    let count = phrase.presses.len();
    let mut line = format!("Played {count} presses over {:.1} s", phrase.length_ms as f64 / 1000.0);
    if played.stopped {
        let seconds = played.after_ms as f64 / 1000.0;
        line = format!("Stopped the phrase after {seconds:.1} s, with {} of {count} presses played", played.started);
    }

    let now = Instant::now();
    let composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    let state = json!({
        "played": phrase.presses,
        "stopped": played.stopped,
        "looper": looper_json(&mut engine, &composer, now),
    });
    Ok(ToolAnswer::of(state, vec![line]))
}

// jello_looper: the action in `arguments`, done to the looper.
fn run_looper(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    let now = Instant::now();
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();

    let mut lines = Vec::new();
    match action.as_str() {
        "press" => {
            let was = engine.looper_status().state.name();
            engine.press_looper();
            lines.push(format!("Looper pressed while {was}"));
        }
        "pause" => {
            engine.pause_loop();
            lines.push(String::from("Loop paused"));
        }
        "play" => {
            engine.play_loop();
            lines.push(String::from("Loop playing from beat 1"));
        }
        "snap" => {
            let on = flag_argument(arguments, "on")?;
            engine.set_snap(on);
            if on {
                lines.push(String::from("Snap to beat on"));
            } else {
                lines.push(String::from("Snap to beat off"));
            }
        }
        "tempo" => {
            let tempo = whole_argument(arguments, "tempo")?;
            if tempo < i64::from(SLOWEST_TEMPO) || tempo > i64::from(FASTEST_TEMPO) {
                return Err(format!("tempo is {SLOWEST_TEMPO} to {FASTEST_TEMPO} beats per minute, not {tempo}"));
            }
            if engine.is_looper_busy() {
                return Err(String::from("the tempo can't change while the looper is busy"));
            }
            let was = composer.tempo();
            composer.set_tempo(&mut engine, tempo as u32);
            lines.push(format!("Tempo {was} -> {} bpm", composer.tempo()));
        }
        "layer_volume" => {
            let layer = layer_argument(arguments)?;
            let volume = whole_argument(arguments, "volume")?;
            if volume < 0 || volume > i64::from(FULL_VOLUME) {
                return Err(format!("volume is 0 to {FULL_VOLUME}%, not {volume}"));
            }
            engine.set_layer_volume(layer, volume as u32)?;
            lines.push(format!("Layer {layer} volume {volume}%"));
        }
        "layer_playing" => {
            let layer = layer_argument(arguments)?;
            let playing = flag_argument(arguments, "playing")?;
            engine.set_layer_playing(layer, playing)?;
            if playing {
                lines.push(format!("Layer {layer} playing"));
            } else {
                lines.push(format!("Layer {layer} paused"));
            }
        }
        "remove_layer" => {
            let count = engine.loop_layers().len() as u32;
            if count == 0 {
                return Err(String::from("the loop has no layers"));
            }
            if arguments.get("layer").is_some() {
                let layer = layer_argument(arguments)?;
                if layer > count {
                    return Err(format!("the loop has layers 1 to {count}, not {layer}"));
                }
                engine.remove_layer(layer);
                lines.push(format!("Layer {layer} removed"));
            } else {
                engine.remove_newest_layer();
                lines.push(format!("Layer {count} removed"));
            }
        }
        _ => {
            let actions = "press, pause, play, snap, tempo, layer_volume, layer_playing and remove_layer";
            return Err(format!("jello_looper has no action {action}: it has {actions}"));
        }
    }

    let state = looper_json(&mut engine, &composer, now);
    Ok(ToolAnswer::of(state, lines))
}

// jello_notes: writes a loop layer's notes, as `arguments` give them.
fn write_notes(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let notes = layer_notes::notes_of(arguments)?;
    let layer = optional_count_argument(arguments, "layer")?;
    let bars = optional_count_argument(arguments, "bars")?;
    let volume = optional_count_argument(arguments, "volume")?;
    let mut engine = studio.engine.lock().unwrap();
    let written = layer_notes::write_layer(&mut engine, layer, bars, volume, &notes)?;
    // The loop as written, which the engine's audio thread takes in a moment.
    let mut layers = Vec::new();
    let mut loop_bars = 0;
    if let Some((bars, kept)) = engine.loop_to_keep() {
        loop_bars = bars;
        for (place, kept_layer) in kept.iter().enumerate() {
            let mut layer_bars = kept_layer.bars;
            if layer_bars == 0 {
                layer_bars = bars;
            }
            layers.push(json!({
                "layer": place + 1,
                "bars": layer_bars,
                "notes": kept_layer.notes.len(),
                "volume": kept_layer.volume,
                "playing": kept_layer.playing,
            }));
        }
    }
    let state = json!({
        "written": written.line,
        "loop": {"bars": loop_bars, "layers": layers},
        "tempo": engine.tempo(),
    });
    Ok(ToolAnswer::of(state, vec![written.line]))
}

// Argument `name` as a whole number from 0, or None if it is left out.
fn optional_count_argument(arguments: &Value, name: &str) -> Result<Option<u32>, String> {
    if arguments[name].is_null() {
        return Ok(None);
    }
    let number = whole_argument(arguments, name)?;
    if number < 0 || number > i64::from(u32::MAX) {
        return Err(format!("{name} is a whole number from 0, not {number}"));
    }
    Ok(Some(number as u32))
}

// What the looper is doing, its tempo and snap, as jello_look reads them.
fn looper_json(engine: &mut Engine, composer: &Composer, now: Instant) -> Value {
    let view = to_json(&sound_view_of(engine, composer, now));
    json!({
        "looper": view["looper"],
        "tempo": view["tempo"],
        "snap": view["snap"],
    })
}

// The layer `arguments` name, from 1.
fn layer_argument(arguments: &Value) -> Result<u32, String> {
    let layer = whole_argument(arguments, "layer")?;
    if layer < 1 || layer > i64::from(u32::MAX) {
        return Err(format!("layers count from 1, so there is no layer {layer}"));
    }
    Ok(layer as u32)
}

// jello_song: the action in `arguments`, done to the song.
fn change_song(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    let now = Instant::now();
    let mut open_song = studio.open_song.lock().unwrap();
    let key_map = studio.key_map.lock().unwrap();
    let mut composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();

    let chords_before = chord_names(&composer, &key_map, &engine, now);
    let mut lines = Vec::new();
    match action.as_str() {
        "key" => {
            let name = text_argument(arguments, "key")?;
            let Some(key) = key_of_name(&name) else {
                return Err(format!("{name} is not a key: a key is named like \"A minor\" or \"Db major\""));
            };
            composer.pick_key(key);
            lines.push(format!("Key {}", key_name(key)));
        }
        "add_last" => {
            composer.add_last_chord();
            let chords = chord_names(&composer, &key_map, &engine, now);
            if chords.len() == chords_before.len() {
                return Err(String::from("no chord has been played yet: play one with jello_play first"));
            }
            lines.push(format!("Added {}", chords[chords.len() - 1]));
        }
        "add_tried" => {
            let number = whole_argument(arguments, "chord")?;
            let view = to_json(&composer.view(&key_map, engine.song_place(), now));
            let tried = view["history"].as_array().map_or(0, Vec::len);
            if number < 1 || number > tried as i64 {
                return Err(format!("Compose's history has tried chords 1 to {tried}, not {number}"));
            }
            let place = number as usize - 1;
            composer.edit("add_history", place)?;
            engine.stop_song();
            lines.push(format!("Added {}", text_of_json(&view["history"][place]["name"])));
        }
        "remove" => {
            let place = chord_place(arguments, &chords_before)?;
            composer.edit("remove", place)?;
            engine.stop_song();
            lines.push(format!("Removed chord {} ({})", place + 1, chords_before[place]));
        }
        "move" => {
            let place = chord_place(arguments, &chords_before)?;
            let direction = text_argument(arguments, "direction")?;
            let lands = moved_place(place, &direction, chords_before.len())?;
            composer.edit(&direction, place)?;
            engine.stop_song();
            lines.push(format!("Moved {} from chord {} to {}", chords_before[place], place + 1, lands + 1));
        }
        "play" => {
            composer.play_song(&mut engine, now)?;
            lines.push(String::from("Playing the song"));
        }
        "play_chord" => {
            let place = chord_place(arguments, &chords_before)?;
            composer.play_chord(place, &mut engine, now)?;
            lines.push(format!("Playing chord {} ({})", place + 1, chords_before[place]));
        }
        "stop" => {
            engine.stop_song();
            lines.push(String::from("Song stopped"));
        }
        "open" => {
            let name = text_argument(arguments, "name")?;
            song_session::open_song_named(&mut open_song, &mut composer, &mut engine, &name, now)?;
            lines.push(format!("Opened {name}"));
        }
        "new" => {
            song_session::new_song(&mut open_song, &mut composer, &mut engine)?;
            lines.push(format!("Made {}", open_song.name()));
        }
        "rename" => {
            let name = text_argument(arguments, "name")?;
            let was = String::from(open_song.name());
            song_session::rename_open_song(&mut open_song, &composer, &mut engine, &name)?;
            lines.push(format!("Renamed {was} to {}", open_song.name()));
        }
        _ => {
            let actions = "key, add_last, add_tried, remove, move, play, play_chord, stop, open, new and rename";
            return Err(format!("jello_song has no action {action}: it has {actions}"));
        }
    }

    let state = json!({
        "name": open_song.name(),
        "compose": compose_json(&composer, &key_map, &engine, now),
    });
    Ok(ToolAnswer::of(state, lines))
}

// What Compose shows, as jello_look reads it, without the recent presses.
fn compose_json(composer: &Composer, key_map: &KeyMap, engine: &Engine, now: Instant) -> Value {
    let mut view = to_json(&composer.view(key_map, engine.song_place(), now));
    if let Some(view) = view.as_object_mut() {
        view.remove("performance");
    }
    view
}

// The names of the song's chords, in order.
fn chord_names(composer: &Composer, key_map: &KeyMap, engine: &Engine, now: Instant) -> Vec<String> {
    let view = to_json(&composer.view(key_map, engine.song_place(), now));
    let mut names = Vec::new();
    if let Some(chords) = view["progression"].as_array() {
        for chord in chords {
            names.push(text_of_json(&chord["name"]));
        }
    }
    names
}

// The place, from 0, of the chord `arguments` number from 1, in a song of
// `chords`.
fn chord_place(arguments: &Value, chords: &[String]) -> Result<usize, String> {
    let number = whole_argument(arguments, "chord")?;
    if chords.is_empty() {
        return Err(String::from("the song has no chords yet"));
    }
    if number < 1 || number > chords.len() as i64 {
        return Err(format!("the song has chords 1 to {}, not {number}", chords.len()));
    }
    Ok(number as usize - 1)
}

// Where the chord at `place` lands moved one place `direction`, in a song
// of `count` chords, or why it can't move.
fn moved_place(place: usize, direction: &str, count: usize) -> Result<usize, String> {
    if direction == "left" {
        if place == 0 {
            return Err(String::from("the first chord can't move further left"));
        }
        return Ok(place - 1);
    }
    if direction == "right" {
        if place + 1 == count {
            return Err(String::from("the last chord can't move further right"));
        }
        return Ok(place + 1);
    }
    Err(format!("direction is left or right, not {direction}"))
}

// jello_presets: the action in `arguments`. Asking the player happens with
// nothing locked, so Jello keeps playing while the question waits.
fn use_presets(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let action = text_argument(arguments, "action")?;
    let folders = studio.open_song.lock().unwrap().folders()?;

    let mut lines = Vec::new();
    match action.as_str() {
        "list" => {}
        "load" => {
            let name = text_argument(arguments, "name")?;
            let sound = presets::read(&folders.sounds, &name, &SoundProfile::new())?;
            let mut composer = studio.composer.lock().unwrap();
            let mut engine = studio.engine.lock().unwrap();
            let before = composer.sound_profile().sound();
            composer.take_preset(&mut engine, &name, &sound);
            let after = composer.sound_profile().sound();
            lines.push(format!("Loaded preset {name}"));
            lines.extend(sound_changes::change_lines(&before, &after));
        }
        "save" => {
            let name = text_argument(arguments, "name")?;
            if let Some(problem) = presets::name_problem(&name) {
                return Err(problem);
            }
            let starting = presets::starting_name(&folders.settings);
            let existing = presets::list(&folders.sounds, starting.as_deref());
            let mut taken = None;
            for preset in &existing {
                if preset.name.to_lowercase() == name.to_lowercase() {
                    taken = Some(preset.name.clone());
                }
            }
            if let Some(taken) = taken {
                if !studio.screen.ask(&format!("Save the song's sound over preset {taken}?")) {
                    return Err(format!("the player said no, so preset {taken} stays as it was"));
                }
            }
            let mut composer = studio.composer.lock().unwrap();
            let sound = composer.sound_profile().sound();
            let saved = presets::save(&folders.sounds, &name, &sound)?;
            composer.saved_as_preset(&saved);
            lines.push(format!("Saved preset {saved}"));
        }
        "starting" => {
            let name = optional_text_argument(arguments, "name")?;
            let question = match &name {
                Some(name) => format!("Make preset {name} the sound every new song starts on?"),
                None => String::from("Let new songs start on Jello's own sound again?"),
            };
            if !studio.screen.ask(&question) {
                return Err(String::from("the player said no, so the starting sound stays"));
            }
            presets::mark_starting(&folders.settings, name.as_deref())?;
            match &name {
                Some(name) => lines.push(format!("Starting sound {name}")),
                None => lines.push(String::from("No starting sound")),
            }
        }
        _ => return Err(format!("jello_presets has no action {action}: it has list, load, save and starting")),
    }

    let starting = presets::starting_name(&folders.settings);
    let composer = studio.composer.lock().unwrap();
    let state = json!({
        "presets": to_json(&presets::list(&folders.sounds, starting.as_deref())),
        "sounds_like": composer.lit_preset(),
    });
    Ok(ToolAnswer::of(state, lines))
}

// jello_listen: renders and measures, changing nothing live.
fn listen(studio: &Studio, arguments: &Value) -> Result<ToolAnswer, String> {
    let measured = listening::listen(studio, arguments)?;
    let card = cards::listen_card(&measured);
    Ok(ToolAnswer::of(measured, Vec::new()).with_card(card))
}

// Argument `name` as text.
pub(crate) fn text_argument(arguments: &Value, name: &str) -> Result<String, String> {
    match arguments[name].as_str() {
        Some(text) => Ok(String::from(text)),
        None => Err(format!("{name} is needed, as text")),
    }
}

// The same, or None if it is left out or null.
pub(crate) fn optional_text_argument(arguments: &Value, name: &str) -> Result<Option<String>, String> {
    if arguments[name].is_null() {
        return Ok(None);
    }
    let text = text_argument(arguments, name)?;
    Ok(Some(text))
}

// Argument `name` as a whole number.
fn whole_argument(arguments: &Value, name: &str) -> Result<i64, String> {
    let Some(number) = arguments[name].as_f64() else {
        return Err(format!("{name} is needed, as a whole number"));
    };
    if number.fract() != 0.0 {
        return Err(format!("{name} is a whole number, not {number}"));
    }
    Ok(number as i64)
}

// Argument `name` as true or false.
fn flag_argument(arguments: &Value, name: &str) -> Result<bool, String> {
    match arguments[name].as_bool() {
        Some(flag) => Ok(flag),
        None => Err(format!("{name} is needed, as true or false")),
    }
}

// The same, or false if it is left out or null.
pub(crate) fn optional_flag_argument(arguments: &Value, name: &str) -> Result<bool, String> {
    if arguments[name].is_null() {
        return Ok(false);
    }
    flag_argument(arguments, name)
}

// A JSON word without its quotes.
fn text_of_json(value: &Value) -> String {
    match value.as_str() {
        Some(text) => String::from(text),
        None => value.to_string(),
    }
}

fn to_json(value: &impl serde::Serialize) -> Value {
    match serde_json::to_value(value) {
        Ok(json) => json,
        Err(error) => json!(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::Duration;

    use crate::commands::{key_down, key_up};
    use crate::presets::test_folder;
    use crate::song_session::Folders;

    // A studio on its own, like the app's but with no speakers. With no
    // folders nothing is saved. As its own screen, it answers every
    // question with `says_yes`, and keeps the questions in `asked` and the
    // keys lit and let go in `shown`.
    struct TestStudio {
        open_song: Mutex<OpenSong>,
        key_map: Mutex<KeyMap>,
        composer: Mutex<Composer>,
        engine: Mutex<Engine>,
        phrases: Phrases,
        offers: Mutex<Offers>,
        practice: Mutex<Practice>,
        memory: Mutex<Memory>,
        says_yes: bool,
        asked: Mutex<Vec<String>>,
        shown: Mutex<Vec<(String, bool)>>,
    }

    impl Screen for TestStudio {
        fn ask(&self, question: &str) -> bool {
            self.asked.lock().unwrap().push(String::from(question));
            self.says_yes
        }

        fn show_key(&self, key: &str, down: bool) {
            self.shown.lock().unwrap().push((String::from(key), down));
        }
    }

    impl TestStudio {
        fn new() -> TestStudio {
            TestStudio::with_folders(None)
        }

        fn with_folders(folders: Option<Folders>) -> TestStudio {
            let mut engine = Engine::new().unwrap();
            let composer = Composer::new();
            composer.send_all(&mut engine);
            TestStudio {
                open_song: Mutex::new(OpenSong::new(folders)),
                key_map: Mutex::new(KeyMap::new()),
                composer: Mutex::new(composer),
                engine: Mutex::new(engine),
                phrases: Phrases::default(),
                offers: Mutex::new(Offers::default()),
                practice: Mutex::new(Practice::default()),
                memory: Mutex::new(Memory::default()),
                says_yes: true,
                asked: Mutex::new(Vec::new()),
                shown: Mutex::new(Vec::new()),
            }
        }

        fn call(&self, name: &str, arguments: Value) -> ToolAnswer {
            let studio = Studio {
                open_song: &self.open_song,
                key_map: &self.key_map,
                composer: &self.composer,
                engine: &self.engine,
                phrases: &self.phrases,
                offers: &self.offers,
                practice: &self.practice,
                memory: &self.memory,
                screen: self,
            };
            call(&studio, name, &arguments)
        }

        // Taps laptop key `key`, as the player would, through the same
        // steps as press_key and release_key.
        fn tap(&self, key: &str) {
            let now = Instant::now();
            let mut key_map = self.key_map.lock().unwrap();
            let mut composer = self.composer.lock().unwrap();
            key_down(&mut key_map, &mut composer, key, now);
            key_up(&mut key_map, &mut composer, key, now);
        }
    }

    fn folders_for(label: &str) -> Folders {
        Folders {
            songs: test_folder(&format!("{label}-songs")),
            sounds: test_folder(&format!("{label}-sounds")),
            settings: test_folder(&format!("{label}-settings")),
            renders: test_folder(&format!("{label}-renders")),
        }
    }

    fn json_text(answer: &ToolAnswer) -> Value {
        serde_json::from_str(&answer.text).unwrap()
    }

    #[test]
    fn every_tool_listed_is_one_jello_has() {
        let test = TestStudio::new();
        for tool in list().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let answer = test.call(name, json!({}));
            assert_ne!(answer.text, format!("Jello has no tool called {name}"));
        }
        let answer = test.call("jello_dance", json!({}));
        assert!(answer.failed);
    }

    #[test]
    fn looking_reads_the_parts_asked_for() {
        let test = TestStudio::new();
        let answer = test.call("jello_look", json!({"parts": ["sound", "keys"]}));
        assert!(!answer.failed);
        let state = json_text(&answer);
        assert_eq!(state.as_object().unwrap().len(), 2);
        assert_eq!(state["sound"]["values"]["filter"]["type"], "low-pass");
        // KeyM plays middle C at the starting octave.
        assert_eq!(state["keys"]["notes"]["KeyM"], 60);
    }

    #[test]
    fn looking_with_no_parts_reads_them_all() {
        let test = TestStudio::new();
        let state = json_text(&test.call("jello_look", json!({})));
        for part in LOOK_PARTS {
            assert!(state.get(part).is_some(), "{part} is missing");
        }
        // With no Songs folder, the presets say why there are none.
        assert_eq!(state["presets"], "the Songs folder couldn't be found");
    }

    #[test]
    fn looking_for_a_part_jello_has_not_says_which_there_are() {
        let test = TestStudio::new();
        let answer = test.call("jello_look", json!({"parts": ["mixer"]}));
        assert!(answer.failed);
        assert!(answer.text.starts_with("there is no part called mixer: there are sound, keys"));
    }

    #[test]
    fn changing_the_sound_changes_the_song_and_says_what_changed() {
        let test = TestStudio::new();
        let answer = test.call("jello_sound", json!({"changes": {"filter": {"cutoff_hz": 800}}}));
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(answer.changes, vec![String::from("Filter cutoff 20000 -> 800 Hz")]);

        let composer = test.composer.lock().unwrap();
        assert_eq!(composer.sound_profile().filter().cutoff_hz, 800.0);
        let state = json_text(&answer);
        assert_eq!(state["values"]["filter"]["cutoff_hz"], 800.0);
    }

    #[test]
    fn a_refused_sound_change_changes_nothing() {
        let test = TestStudio::new();
        let before = test.composer.lock().unwrap().sound_profile().sound();
        let changes = json!({"changes": {"filter": {"resonance": 50, "cutoff_hz": -5}}});
        let answer = test.call("jello_sound", changes);
        assert!(answer.failed);
        assert_eq!(answer.text, "filter cutoff_hz is 20 to 20000 Hz, not -5");
        assert!(answer.changes.is_empty());
        // Not even the resonance, which came first.
        let after = test.composer.lock().unwrap().sound_profile().sound();
        assert_eq!(after, before);
    }

    #[test]
    fn the_looper_s_tempo_changes_within_its_range() {
        let test = TestStudio::new();
        let answer = test.call("jello_looper", json!({"action": "tempo", "tempo": 90}));
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(answer.changes, vec![String::from("Tempo 120 -> 90 bpm")]);
        assert_eq!(json_text(&answer)["tempo"], 90);
        assert_eq!(test.engine.lock().unwrap().tempo(), 90);

        let answer = test.call("jello_looper", json!({"action": "tempo", "tempo": 300}));
        assert_eq!(answer.text, "tempo is 40 to 240 beats per minute, not 300");
        assert_eq!(test.composer.lock().unwrap().tempo(), 90);
    }

    #[test]
    fn the_looper_switches_snap_and_refuses_layers_it_has_not() {
        let test = TestStudio::new();
        let answer = test.call("jello_looper", json!({"action": "snap", "on": false}));
        assert_eq!(answer.changes, vec![String::from("Snap to beat off")]);
        assert!(!test.engine.lock().unwrap().snap());

        let answer = test.call("jello_looper", json!({"action": "remove_layer"}));
        assert_eq!(answer.text, "the loop has no layers");
        let answer = test.call("jello_looper", json!({"action": "layer_volume", "layer": 1, "volume": 101}));
        assert_eq!(answer.text, "volume is 0 to 100%, not 101");
        let answer = test.call("jello_looper", json!({"action": "dance"}));
        assert!(answer.text.starts_with("jello_looper has no action dance"));
    }

    #[test]
    fn the_song_takes_a_key_by_name() {
        let test = TestStudio::new();
        let answer = test.call("jello_song", json!({"action": "key", "key": "A minor"}));
        assert_eq!(answer.changes, vec![String::from("Key A minor")]);
        assert_eq!(json_text(&answer)["compose"]["key"]["name"], "A minor");

        let answer = test.call("jello_song", json!({"action": "key", "key": "H major"}));
        assert!(answer.failed);
        assert!(answer.text.starts_with("H major is not a key"));
    }

    #[test]
    fn chords_played_are_added_moved_and_removed() {
        let test = TestStudio::new();
        let answer = test.call("jello_song", json!({"action": "add_last"}));
        assert_eq!(answer.text, "no chord has been played yet: play one with jello_play first");

        // KeyX plays Cm and KeyC plays C.
        test.tap("KeyX");
        let answer = test.call("jello_song", json!({"action": "add_last"}));
        assert_eq!(answer.changes, vec![String::from("Added Cm")]);
        test.tap("KeyC");
        test.call("jello_song", json!({"action": "add_last"}));

        let answer = test.call("jello_song", json!({"action": "move", "chord": 2, "direction": "left"}));
        assert_eq!(answer.changes, vec![String::from("Moved C from chord 2 to 1")]);
        let chords = &json_text(&answer)["compose"]["progression"];
        assert_eq!(chords[0]["name"], "C");
        assert_eq!(chords[1]["name"], "Cm");

        let answer = test.call("jello_song", json!({"action": "move", "chord": 1, "direction": "left"}));
        assert_eq!(answer.text, "the first chord can't move further left");
        let answer = test.call("jello_song", json!({"action": "remove", "chord": 3}));
        assert_eq!(answer.text, "the song has chords 1 to 2, not 3");

        let answer = test.call("jello_song", json!({"action": "remove", "chord": 1}));
        assert_eq!(answer.changes, vec![String::from("Removed chord 1 (C)")]);
        let chords = &json_text(&answer)["compose"]["progression"];
        assert_eq!(chords.as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_new_preset_is_saved_without_asking() {
        let test = TestStudio::with_folders(Some(folders_for("tools-save")));
        let answer = test.call("jello_presets", json!({"action": "save", "name": "Warm"}));
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(answer.changes, vec![String::from("Saved preset Warm")]);
        assert!(test.asked.lock().unwrap().is_empty());
        assert_eq!(json_text(&answer)["sounds_like"], "Warm");
    }

    #[test]
    fn saving_over_a_preset_asks_and_a_no_keeps_it() {
        let mut test = TestStudio::with_folders(Some(folders_for("tools-over")));
        test.call("jello_presets", json!({"action": "save", "name": "Warm"}));
        test.call("jello_sound", json!({"changes": {"filter": {"cutoff_hz": 500}}}));

        test.says_yes = false;
        let answer = test.call("jello_presets", json!({"action": "save", "name": "warm"}));
        assert_eq!(answer.text, "the player said no, so preset Warm stays as it was");
        let asked = test.asked.lock().unwrap().clone();
        assert_eq!(asked, vec![String::from("Save the song's sound over preset Warm?")]);

        // Loading it back brings the old cutoff back.
        let answer = test.call("jello_presets", json!({"action": "load", "name": "Warm"}));
        assert_eq!(answer.changes[0], "Loaded preset Warm");
        assert_eq!(answer.changes[1], "Filter cutoff 500 -> 20000 Hz");
    }

    #[test]
    fn the_starting_sound_changes_only_on_a_yes() {
        let mut test = TestStudio::with_folders(Some(folders_for("tools-starting")));
        test.call("jello_presets", json!({"action": "save", "name": "Warm"}));

        test.says_yes = false;
        let answer = test.call("jello_presets", json!({"action": "starting", "name": "Warm"}));
        assert!(answer.failed);
        let state = json_text(&test.call("jello_presets", json!({"action": "list"})));
        assert_eq!(state["presets"][0]["starting"], false);

        test.says_yes = true;
        let answer = test.call("jello_presets", json!({"action": "starting", "name": "Warm"}));
        assert_eq!(answer.changes, vec![String::from("Starting sound Warm")]);
        assert_eq!(json_text(&answer)["presets"][0]["starting"], true);
        assert_eq!(test.asked.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_phrase_presses_and_lets_go_on_time_like_the_player() {
        let test = TestStudio::new();
        let phrase = json!({"unit": "ms", "presses": [
            {"note": "C4", "start": 0, "hold": 20},
            {"chord": "Am", "start": 10, "hold": 20},
        ]});
        let answer = test.call("jello_play", phrase);
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(answer.changes, vec![String::from("Played 2 presses over 0.0 s")]);

        let shown = test.shown.lock().unwrap().clone();
        let expected = vec![
            (String::from("KeyM"), true),
            (String::from("KeyQ"), true),
            (String::from("KeyM"), false),
            (String::from("KeyQ"), false),
        ];
        assert_eq!(shown, expected);

        // Compose heard the chord, so it can be added to the song.
        let answer = test.call("jello_song", json!({"action": "add_last"}));
        assert_eq!(answer.changes, vec![String::from("Added Am")]);
        let looked = test.call("jello_look", json!({"parts": ["presses"]}));
        let presses = json_text(&looked);
        assert_eq!(presses["presses"].as_array().unwrap().len(), 2);
        // The strip names a chord key's press by its chord.
        let strip = looked.card.unwrap();
        assert_eq!(strip["presses"][0]["label"], "C4");
        assert_eq!(strip["presses"][1]["label"], "Am");
        assert!(strip["presses"][1]["at_ms"].as_u64().unwrap() >= 10);
    }

    #[test]
    fn stop_ends_a_phrase_and_lets_go_of_what_it_holds() {
        let test = TestStudio::new();
        let long = json!({"unit": "ms", "presses": [{"note": "C4", "start": 0, "hold": 20000}]});
        thread::scope(|scope| {
            let playing = scope.spawn(|| test.call("jello_play", long));
            // A second phrase waits its turn.
            thread::sleep(Duration::from_millis(50));
            let second = json!({"unit": "ms", "presses": [{"note": "D4", "start": 0, "hold": 10}]});
            let answer = test.call("jello_play", second);
            assert_eq!(answer.text, "a phrase is playing already: wait for it to end, or stop it");

            test.phrases.stop();
            let answer = playing.join().unwrap();
            assert!(answer.changes[0].starts_with("Stopped the phrase after 0."));
            assert!(answer.changes[0].ends_with("with 1 of 1 presses played"));
        });
        // KeyM went down and came back up.
        let shown = test.shown.lock().unwrap().clone();
        assert_eq!(shown, vec![(String::from("KeyM"), true), (String::from("KeyM"), false)]);
        // Nothing is left held, so the player's next press of KeyM plays.
        let mut key_map = test.key_map.lock().unwrap();
        assert!(key_map.press("KeyM").is_some());
    }

    #[test]
    fn listening_renders_a_phrase_and_saves_it_under_a_new_name_only() {
        let test = TestStudio::with_folders(Some(folders_for("tools-listen")));
        let phrase = json!({"what": "phrase", "presses": [{"chord": "Am", "start": 0, "hold": 1}], "save_as": "Try 1"});
        let answer = test.call("jello_listen", phrase.clone());
        assert!(!answer.failed, "{}", answer.text);
        assert!(answer.changes.is_empty());
        let measured = json_text(&answer);
        assert_eq!(measured["rendered"], json!("phrase"));
        assert_eq!(measured["beats"].as_array().unwrap().len(), 1);
        assert!(measured["whole"]["loudness_db"].as_f64().unwrap() > -30.0);
        let meters = answer.card.as_ref().unwrap();
        assert_eq!(meters["kind"], "listen");
        assert_eq!(meters["loudness"].as_array().unwrap().len(), 1);
        let saved = measured["saved"].as_str().unwrap();
        assert!(saved.ends_with("Try 1.wav"));
        assert!(std::path::Path::new(saved).exists());

        let again = test.call("jello_listen", phrase);
        assert_eq!(again.text, "a render called \"Try 1\" is saved already: pick another name");
        let bad = test.call("jello_listen", json!({"what": "chords", "save_as": "a/b"}));
        assert_eq!(bad.text, "save_as can't be \"a/b\": Only letters, numbers, spaces, - and _");
        let nothing = test.call("jello_listen", json!({"what": "chords"}));
        assert_eq!(nothing.text, "the song has no chords yet");
        let wrong = test.call("jello_listen", json!({"what": "mix"}));
        assert_eq!(wrong.text, "jello_listen renders the loop, the chords or a phrase, not mix");
    }

    #[test]
    fn cards_are_checked_and_handed_to_the_panel() {
        let test = TestStudio::new();
        test.call("jello_song", json!({"action": "key", "key": "A minor"}));
        let card = json!({"card": {"kind": "progression", "title": "The loop", "chords": ["Am", "F", "C", "G"]}});
        let answer = test.call("jello_show", card);
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(json_text(&answer), json!({"shown": "progression"}));
        let card = answer.card.unwrap();
        assert_eq!(card["key"], "A minor");
        let numerals: Vec<&str> = card["chords"].as_array().unwrap().iter().map(|chord| chord["numeral"].as_str().unwrap()).collect();
        assert_eq!(numerals, vec!["i", "bVI", "bIII", "bVII"]);

        let lane = json!({"card": {"kind": "lane", "parts": [{"name": "Right hand", "notes": [
            {"note": "E4", "start": 0, "hold": 1},
            {"chord": "Am", "start": 4, "hold": 1.5},
        ]}]}});
        let card = test.call("jello_show", lane).card.unwrap();
        assert_eq!(card["beats"], 8.0);
        assert_eq!(card["parts"][0]["notes"][1]["pitches"], json!([57, 60, 64]));

        let wrong = test.call("jello_show", json!({"card": {"kind": "lane", "parts": []}}));
        assert_eq!(wrong.text, "a lane card has 1 to 4 parts, not 0");
        assert!(wrong.card.is_none());
        let unknown = test.call("jello_show", json!({"card": {"kind": "mixer"}}));
        assert!(unknown.text.starts_with("there is no card called mixer: there are keys, lane"));
        assert!(!changes_something("jello_show"));
    }

    #[test]
    fn a_keys_card_marks_the_real_keys() {
        let test = TestStudio::new();
        let card = json!({"card": {"kind": "keys", "keys": [{"note": "C4"}, {"chord": "Am", "label": "R"}]}});
        let card = test.call("jello_show", card).card.unwrap();
        assert_eq!(card["keys"][0], json!({"key": "KeyM", "what": "C4", "label": "1"}));
        assert_eq!(card["keys"][1]["label"], "R");
        let long = json!({"card": {"kind": "keys", "keys": [{"note": "C4", "label": "Tonic"}]}});
        assert_eq!(test.call("jello_show", long).text, "key 1's label is at most 4 characters, like \"1\" or \"R\"");
    }

    #[test]
    fn a_sound_card_draws_changes_without_making_them() {
        let test = TestStudio::new();
        let card = json!({"card": {"kind": "sound", "changes": {"filter": {"cutoff_hz": 800}}}});
        let card = test.call("jello_show", card).card.unwrap();
        assert_eq!(card["after"]["filter"]["cutoff_hz"], 800.0);
        assert_eq!(card["before"]["filter"]["cutoff_hz"], 20000.0);
        assert_eq!(card["lines"], json!(["Filter cutoff 20000 -> 800 Hz"]));
        assert_eq!(test.composer.lock().unwrap().sound_profile().filter().cutoff_hz, 20000.0);

        // A real change draws itself the same way.
        let answer = test.call("jello_sound", json!({"changes": {"filter": {"cutoff_hz": 800}}}));
        assert_eq!(answer.card.unwrap()["kind"], "sound");
    }

    #[test]
    fn options_are_kept_by_number_and_checked_first() {
        let test = TestStudio::new();
        let options = json!({"kind": "chords", "options": [
            {"name": "Darker", "chords": ["Am", {"chord": "F", "hold": 4}]},
            {"name": "Brighter", "chords": ["C", "G"]},
        ]});
        let answer = test.call("jello_options", options);
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(json_text(&answer), json!({"offered": 2, "offer": 1}));
        let card = answer.card.unwrap();
        assert_eq!(card["kind"], "options");
        assert_eq!(card["options"][0]["chords"][1], json!({"name": "F", "hold": 4.0}));

        let one = json!({"kind": "chords", "options": [{"name": "Only", "chords": ["C"]}]});
        assert_eq!(test.call("jello_options", one).text, "jello_options has 2 to 4 options, not 1");
        let bad = json!({"kind": "sound", "options": [
            {"name": "A", "changes": {"filter": {"cutoff_hz": 800}}},
            {"name": "B", "changes": {"filter": {"cutoff_hz": -1}}},
        ]});
        assert_eq!(test.call("jello_options", bad).text, "option 2: filter cutoff_hz is 20 to 20000 Hz, not -1");
        // Out of reach of the keys at this octave.
        let high = json!({"kind": "melody", "options": [
            {"name": "A", "presses": [{"note": "C4", "start": 0, "hold": 1}]},
            {"name": "B", "presses": [{"note": "C8", "start": 0, "hold": 1}]},
        ]});
        assert!(test.call("jello_options", high).failed);
        // Only the first offer was kept.
        let offers = test.offers.lock().unwrap();
        assert!(offers.has(1));
        assert!(!offers.has(2));
    }

    #[test]
    fn notes_written_become_a_layer() {
        let test = TestStudio::new();
        let notes = json!({"notes": [
            {"note": "E4", "start": 0, "hold": 1},
            {"chord": "Am", "start": 4, "hold": 4},
        ]});
        let answer = test.call("jello_notes", notes);
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(answer.changes, vec![String::from("New layer 1: 4 notes over 2 bars")]);
        let state = json_text(&answer);
        assert_eq!(state["loop"]["bars"], 2);
        assert_eq!(state["loop"]["layers"][0]["notes"], 4);

        let late = json!({"layer": 1, "notes": [{"note": "E4", "start": 8, "hold": 1}]});
        assert_eq!(test.call("jello_notes", late).text, "E4 ends on beat 9, after the layer's 2 bars (8 beats)");
        let missing = json!({"layer": 3, "notes": [{"note": "E4", "start": 0, "hold": 1}]});
        assert_eq!(test.call("jello_notes", missing).text, "there is no layer 3: the loop has 1");
    }

    #[test]
    fn listening_leaves_the_live_keys_as_they_are() {
        let test = TestStudio::new();
        let phrase = json!({"what": "phrase", "presses": [{"key": "KeyC", "start": 0, "hold": 1}]});
        test.call("jello_listen", phrase);
        let compose = json_text(&test.call("jello_look", json!({"parts": ["presses"]})));
        assert_eq!(compose["presses"], json!([]));
        assert!(test.shown.lock().unwrap().is_empty());
        assert!(!changes_something("jello_listen"));
    }

    #[test]
    fn practice_is_set_checked_and_reported() {
        let test = TestStudio::new();
        let piece = json!({
            "name": "Five notes",
            "parts": [{"name": "Right hand", "hand": "right", "notes": [
                {"note": "C4", "start": 0, "hold": 1},
                {"note": "D4", "start": 1, "hold": 1},
            ]}],
        });
        let answer = test.call("jello_practice", json!({"action": "set", "piece": piece, "settings": {"level": "novice"}}));
        assert!(!answer.failed, "{}", answer.text);
        let view: Value = serde_json::from_str(&answer.text).unwrap();
        assert_eq!(view["piece"]["bars"], 1);
        assert_eq!(view["window_ms"], 120.0);
        assert!(view.get("lane").is_none());
        assert_eq!(answer.card.unwrap()["kind"], "practice");

        let wrong = test.call("jello_practice", json!({"action": "settings", "settings": {"hands": "left"}}));
        assert!(wrong.failed);
        assert!(wrong.text.contains("no part"), "{}", wrong.text);
        let changed = test.call("jello_practice", json!({"action": "settings", "settings": {"tempo": 70}}));
        assert_eq!(changed.changes, vec!["Practice tempo: 70"]);
        assert_eq!(test.practice.lock().unwrap().tempo_now, 70);

        // The test screen can't play a try.
        assert!(test.call("jello_practice", json!({"action": "start"})).failed);
        assert!(test.call("jello_practice", json!({"action": "stop"})).failed);
        let closed = test.call("jello_practice", json!({"action": "close"}));
        assert!(!closed.failed);
        assert!(test.practice.lock().unwrap().piece.is_none());
    }

    #[test]
    fn memory_notes_are_kept_shown_and_asked_about_when_the_switch_is_on() {
        let test = TestStudio::new();
        let saved = test.call("jello_memory", json!({"action": "remember", "kind": "habit", "text": "Rushes bar 3"}));
        assert!(!saved.failed, "{}", saved.text);
        assert_eq!(saved.changes, vec!["Remembered a note"]);
        let card = saved.card.unwrap();
        assert_eq!(card["kind"], "memory");
        assert_eq!(card["note"]["id"], 1);
        assert!(test.asked.lock().unwrap().is_empty());

        let state = json_text(&test.call("jello_look", json!({"parts": ["memory", "skills"]})));
        assert_eq!(state["memory"]["notes"][0]["text"], "Rushes bar 3");
        assert_eq!(state["skills"][6]["area"], "writing");

        test.memory.lock().unwrap().ask_first = true;
        let changed = test.call("jello_memory", json!({"action": "change", "id": 1, "text": "Rushes bar 4"}));
        assert!(!changed.failed, "{}", changed.text);
        assert_eq!(test.asked.lock().unwrap().len(), 1);
        let forgot = test.call("jello_memory", json!({"action": "forget", "id": 1}));
        assert_eq!(forgot.changes, vec!["Forgot a note"]);
        assert!(test.call("jello_memory", json!({"action": "forget", "id": 1})).failed);
        assert!(test.call("jello_memory", json!({"action": "remember", "kind": "mood", "text": "x"})).failed);
        assert!(test.call("jello_memory", json!({"action": "shout"})).failed);
    }

    #[test]
    fn a_note_the_player_says_no_to_is_not_saved() {
        let mut test = TestStudio::new();
        test.says_yes = false;
        test.memory.lock().unwrap().ask_first = true;
        let answer = test.call("jello_memory", json!({"action": "remember", "kind": "goal", "text": "Play at 90"}));
        assert!(answer.failed);
        assert!(answer.text.contains("said no"), "{}", answer.text);
        assert!(test.memory.lock().unwrap().notes().is_empty());
    }

    #[test]
    fn a_piece_names_the_skill_its_tries_count_toward() {
        let test = TestStudio::new();
        let notes = json!([{"note": "C4", "start": 0, "hold": 1}]);
        let piece = json!({"name": "Swing", "area": "rhythm", "parts": [{"name": "Right", "hand": "right", "notes": notes}]});
        let answer = test.call("jello_practice", json!({"action": "set", "piece": piece}));
        assert!(!answer.failed, "{}", answer.text);
        assert_eq!(test.practice.lock().unwrap().piece.as_ref().unwrap().area.as_deref(), Some("rhythm"));
        let wrong = json!({"name": "Swing", "area": "dancing", "parts": [{"name": "Right", "hand": "right", "notes": notes}]});
        let refused = test.call("jello_practice", json!({"action": "set", "piece": wrong}));
        assert!(refused.failed);
        assert!(refused.text.contains("area is one of"), "{}", refused.text);
    }
}
