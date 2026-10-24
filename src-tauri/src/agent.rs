// The agent host (docs/design.md#the-agent-mode): a small Node program,
// src-tauri/agent-host/host.mjs, that holds the agent's pi chat. Jello
// starts it the first time the panel asks for it and keeps it running.
//
// Jello writes it one line of JSON per message: send this, stop, start a
// new chat, pick a model, sign in. It writes back one line of JSON per
// thing that happens, and each goes to the panel as an "agent-host" event,
// as it arrives: thinking, each tool starting and ending, text, the end.
// Its tools are the tool server's (tool_server.rs), reached with the
// server's password, which only the host is told.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::attachments::Attachments;
use crate::cards::Offers;
use crate::commands;
use crate::memory::Memory;
use crate::phrases::Phrases;
use crate::practice::Practice;
use crate::song_session::OpenSong;
use crate::tool_server::{self, Questions, ToolServer};

// The longest message the player can send.
const MOST_MESSAGE_CHARACTERS: usize = 12000;

// How much of the host's error output is kept, to say why it ended.
const MOST_ERROR_BYTES: usize = 4000;

// The host while it runs, if it does.
#[derive(Default)]
pub struct AgentHost {
    running: Mutex<Option<Running>>,
}

struct Running {
    child: Child,
    input: ChildStdin,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// Where the host's program is. Only development builds have it so far:
// bundling Node and pi with Jello is a later step.
fn host_script() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent-host").join("host.mjs");
        return Ok(script);
    }
    Err(String::from("this build of Jello doesn't carry the agent yet"))
}

// The command that starts the host for `app`.
fn host_command(app: &AppHandle) -> Result<Command, String> {
    let server = app.state::<Option<ToolServer>>();
    let Some(server) = server.inner() else {
        return Err(String::from("the agent's tool server didn't start"));
    };
    let agent_folder = commands::settings_folder(app)?.join("agent");
    let mut node = String::from("node");
    if let Ok(chosen) = std::env::var("JELLO_NODE") {
        node = chosen;
    }
    let mut command = Command::new(node);
    command
        .arg(host_script()?)
        .env("JELLO_TOOLS_URL", &server.url)
        .env("JELLO_TOOLS_PASSWORD", &server.password)
        .env("JELLO_AGENT_DIR", agent_folder)
        .env("JELLO_CHATS_DIR", commands::chats_folder(app)?);
    Ok(command)
}

// Starts `command` as the host. Every line it writes goes to `tell`, as it
// comes; once it ends, `tell` hears "host_ended" with why.
fn start_host(mut command: Command, tell: Arc<dyn Fn(Value) + Send + Sync>) -> Result<Running, String> {
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Err(format!("the agent needs Node 22.19 or newer, and it couldn't start: {error}"));
        }
    };
    let input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();

    let error_reader = thread::spawn(move || last_bytes(errors));
    thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else {
                break;
            };
            tell(event_of(&line));
        }
        let why = error_reader.join().unwrap_or_default();
        tell(json!({"type": "host_ended", "why": why}));
    });
    Ok(Running { child, input })
}

// The event a line from the host says, or the line as a note if it isn't
// JSON, like a warning Node printed.
fn event_of(line: &str) -> Value {
    match serde_json::from_str::<Value>(line) {
        Ok(event) => event,
        Err(_) => json!({"type": "log", "text": line}),
    }
}

// The last MOST_ERROR_BYTES the host wrote as errors, as text.
fn last_bytes(mut pipe: impl Read) -> String {
    let mut kept: Vec<u8> = Vec::new();
    let mut chunk = [0; 4096];
    while let Ok(count) = pipe.read(&mut chunk) {
        if count == 0 {
            break;
        }
        kept.extend_from_slice(&chunk[..count]);
        if kept.len() > MOST_ERROR_BYTES {
            let extra = kept.len() - MOST_ERROR_BYTES;
            kept.drain(..extra);
        }
    }
    String::from_utf8_lossy(&kept).to_string()
}

// Writes `message` to the host as one line.
fn write_line(running: &mut Running, message: &Value) -> Result<(), String> {
    let line = format!("{message}\n");
    match running.input.write_all(line.as_bytes()) {
        Ok(()) => Ok(()),
        Err(error) => Err(format!("the agent couldn't be reached: {error}")),
    }
}

// Whether the host has ended since it started.
fn has_ended(running: &mut Running) -> bool {
    match running.child.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(_) => true,
    }
}

// Sends `message` to the host, starting it first if it isn't running.
fn send(app: &AppHandle, message: Value) -> Result<(), String> {
    let host = app.state::<AgentHost>();
    let mut running = host.running.lock().unwrap();
    let mut must_start = running.is_none();
    if let Some(started) = running.as_mut() {
        if has_ended(started) {
            must_start = true;
        }
    }
    if must_start {
        let handle = app.clone();
        let tell = Arc::new(move |event: Value| {
            note_chat(&handle, &event);
            let _ = handle.emit("agent-host", event);
        });
        *running = Some(start_host(host_command(app)?, tell)?);
    }
    write_line(running.as_mut().unwrap(), &message)
}

// Keeps which chat is open, so a note saved in it says where it came from.
fn note_chat(app: &AppHandle, event: &Value) {
    let kind = event["type"].as_str().unwrap_or("");
    let memory = app.state::<Mutex<Memory>>();
    if kind == "chat" || kind == "chat_opened" {
        memory.lock().unwrap().set_chat(event["path"].as_str());
    }
    if kind == "new_chat" || kind == "host_ended" {
        memory.lock().unwrap().set_chat(None);
    }
}

// Sends `message` to the host only if it is running already.
fn send_if_running(app: &AppHandle, message: Value) {
    let host = app.state::<AgentHost>();
    let mut running = host.running.lock().unwrap();
    if let Some(started) = running.as_mut() {
        let _ = write_line(started, &message);
    }
}

// The panel opened: the host starts if it hasn't, and says the models it
// can use, the one picked and the providers to sign in to.
#[tauri::command]
pub fn agent_status(app: AppHandle) -> Result<(), String> {
    send(&app, json!({"type": "status"}))
}

// The player sends `text` to the agent, doing `job`: "tutor", "composer"
// or "producer". A new chat is noted as begun on the open song.
#[tauri::command]
pub fn agent_say(app: AppHandle, text: String, job: String) -> Result<(), String> {
    let text = text.trim();
    let nothing_attached = app.state::<Mutex<Attachments>>().lock().unwrap().is_empty();
    if text.is_empty() && nothing_attached {
        return Err(String::from("there is nothing to send"));
    }
    if text.chars().count() > MOST_MESSAGE_CHARACTERS {
        return Err(format!("a message is at most {MOST_MESSAGE_CHARACTERS} characters"));
    }
    if !["tutor", "composer", "producer"].contains(&job.as_str()) {
        return Err(format!("the agent has no job called {job}"));
    }
    let song = app.state::<Mutex<OpenSong>>().lock().unwrap().name().to_string();
    // The options the player used since, so the agent knows.
    let mut used = app.state::<Mutex<Offers>>().lock().unwrap().take_used();
    // And what happened in practice.
    let practiced = app.state::<Mutex<Practice>>().lock().unwrap().take_unsent();
    used.extend(practiced);
    // The notes about the player, which a new chat begins with.
    let memory = app.state::<Mutex<Memory>>().lock().unwrap().prompt_text();
    // And the files attached, which go with this message only.
    let mut attached = Vec::new();
    for attachment in app.state::<Mutex<Attachments>>().lock().unwrap().take() {
        attached.push(attachment.message());
    }
    let message = json!({
        "type": "send",
        "text": text,
        "job": job,
        "song": song,
        "used": used,
        "memory": memory,
        "attachments": attached,
    });
    send(&app, message)
}

// The saved chats, for the Chats page, told as "chats".
#[tauri::command]
pub fn agent_chats(app: AppHandle) -> Result<(), String> {
    send(&app, json!({"type": "chats"}))
}

// Opens the saved chat at `path` to carry on with it. The host checks it is
// a chat in the chats folder.
#[tauri::command]
pub fn agent_open_chat(app: AppHandle, path: String) -> Result<(), String> {
    send(&app, json!({"type": "open_chat", "path": path}))
}

// The panel's Stop, and Escape while the agent works: ends the phrase
// playing, answers No to every question waiting, and stops the chat's run.
#[tauri::command]
pub fn stop_agent(app: AppHandle, phrases: State<'_, Phrases>, questions: State<'_, Mutex<Questions>>) {
    tool_server::stop_tools(&phrases, &questions);
    send_if_running(&app, json!({"type": "stop"}));
}

#[tauri::command]
pub fn agent_new_chat(app: AppHandle) -> Result<(), String> {
    send(&app, json!({"type": "new"}))
}

#[tauri::command]
pub fn agent_pick_model(app: AppHandle, provider: String, model: String) -> Result<(), String> {
    send(&app, json!({"type": "pick", "provider": provider, "model": model}))
}

// Signs in to `provider` by "oauth" (in the browser) or "api_key".
#[tauri::command]
pub fn agent_sign_in(app: AppHandle, provider: String, method: String) -> Result<(), String> {
    if method != "oauth" && method != "api_key" {
        return Err(format!("there is no way to sign in called {method}"));
    }
    send(&app, json!({"type": "sign_in", "provider": provider, "method": method}))
}

// The player's answer to what signing in asked, like a code or an API key.
// It goes straight to the host, which hands it to pi; Jello keeps no copy.
#[tauri::command]
pub fn agent_sign_in_answer(app: AppHandle, text: String) {
    send_if_running(&app, json!({"type": "answer", "text": text}));
}

#[tauri::command]
pub fn agent_cancel_sign_in(app: AppHandle) {
    send_if_running(&app, json!({"type": "cancel_sign_in"}));
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    use std::sync::mpsc;
    use std::time::Duration;

    // Starts `script` in sh as the host, with what it tells kept in order.
    fn fake_host(script: &str) -> (Running, mpsc::Receiver<Value>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let tell = Arc::new(move |event: Value| {
            let _ = sender.lock().unwrap().send(event);
        });
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        (start_host(command, tell).unwrap(), receiver)
    }

    fn next(receiver: &mpsc::Receiver<Value>) -> Value {
        receiver.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn lines_go_both_ways_and_each_answer_is_told_as_it_comes() {
        let (mut running, told) = fake_host("while read line; do echo \"$line\"; done");
        write_line(&mut running, &json!({"type": "status"})).unwrap();
        assert_eq!(next(&told), json!({"type": "status"}));
        write_line(&mut running, &json!({"type": "send", "text": "hi"})).unwrap();
        assert_eq!(next(&told)["text"], "hi");
        assert!(!has_ended(&mut running));
    }

    #[test]
    fn a_line_that_is_not_json_is_told_as_a_note() {
        let (_running, told) = fake_host("echo 'Warning: something'; sleep 5");
        assert_eq!(next(&told), json!({"type": "log", "text": "Warning: something"}));
    }

    #[test]
    fn a_host_that_ends_says_why() {
        let (mut running, told) = fake_host("echo 'Cannot find module' >&2; exit 1");
        let ended = next(&told);
        assert_eq!(ended["type"], "host_ended");
        assert_eq!(ended["why"], "Cannot find module\n");
        thread::sleep(Duration::from_millis(100));
        assert!(has_ended(&mut running));
    }

    #[test]
    fn only_the_last_error_bytes_are_kept() {
        let long = "x".repeat(MOST_ERROR_BYTES * 3) + "end";
        let kept = last_bytes(long.as_bytes());
        assert_eq!(kept.len(), MOST_ERROR_BYTES);
        assert!(kept.ends_with("end"));
    }

    #[test]
    fn a_missing_node_is_explained() {
        let tell = Arc::new(|_: Value| {});
        let result = start_host(Command::new("/no/such/node"), tell);
        let why = result.err().unwrap();
        assert!(why.starts_with("the agent needs Node 22.19 or newer"));
    }
}
