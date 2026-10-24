// The agent's tool server (docs/adr/0041-one-agent-for-every-mode.md,
// docs/design.md#the-agent-harness). It hands Jello's tools to the agent
// over MCP, the Model Context Protocol, as streamable HTTP: the agent posts
// one JSON-RPC message at a time to /mcp and gets the answer back as JSON.
//
// It listens on this machine only, on a free port, and answers only a
// request carrying the password made as Jello starts, which only the
// agent is told. A request from a web page is refused too: browsers say
// where a page came from in an Origin header, and the agent never sends
// one. It has no shell or file tools: only Jello's own.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::compose::Composer;
use crate::engine::Engine;
use crate::key_map::KeyMap;
use crate::memory::Memory;
use crate::cards::Offers;
use crate::phrases::{Phrase, Phrases};
use crate::practice::Practice;
use crate::practice_run;
use crate::song_session::OpenSong;
use crate::tools::{self, Screen, Studio, ToolAnswer};

// The MCP versions Jello speaks, newest first. An agent asking for another
// is offered the newest.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

// The longest message read. A tool call is a few kilobytes.
const MOST_BODY_BYTES: u64 = 1_000_000;

// What the agent is told about the tools as it connects.
const INSTRUCTIONS: &str = "Jello's tools: look at any part of what Jello is doing, change the \
                            sound, and more as they come. Look before changing, and say what you \
                            changed.";

// How long a tool waits for the player to answer its question. No answer
// counts as No.
const ANSWER_WAIT: Duration = Duration::from_secs(300);

// Where the agent reaches the server, and the password it sends.
pub struct ToolServer {
    pub url: String,
    pub password: String,
}

// A tool call: a tool's name and arguments in, its answer out.
type ToolCall = Arc<dyn Fn(&str, &Value) -> ToolAnswer + Send + Sync>;

// Starts the server for the app `app`, on a thread of its own. Its tools
// reach what the app keeps for its commands, so the open song must be kept
// already.
pub fn start(app: &AppHandle) -> Result<ToolServer, String> {
    let handle = app.clone();
    let call: ToolCall = Arc::new(move |name: &str, arguments: &Value| call_tool(&handle, name, arguments));
    start_with(call)
}

fn start_with(call: ToolCall) -> Result<ToolServer, String> {
    let server = match Server::http("127.0.0.1:0") {
        Ok(server) => server,
        Err(error) => return Err(format!("the tool server couldn't start: {error}")),
    };
    let Some(address) = server.server_addr().to_ip() else {
        return Err(String::from("the tool server has no port"));
    };
    let password = new_password()?;
    let url = format!("http://127.0.0.1:{}/mcp", address.port());

    let server_password = password.clone();
    thread::spawn(move || serve(server, server_password, call));
    Ok(ToolServer { url, password })
}

// A password of 32 random bytes, as 64 hex digits.
fn new_password() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    if let Err(error) = getrandom::fill(&mut bytes) {
        return Err(format!("no password could be made for the tool server: {error}"));
    }
    let mut password = String::new();
    for byte in bytes {
        password.push_str(&format!("{byte:02x}"));
    }
    Ok(password)
}

fn serve(server: Server, password: String, call: ToolCall) {
    for request in server.incoming_requests() {
        let password = password.clone();
        let call = Arc::clone(&call);
        // Each request gets its own thread, so a phrase playing for a
        // minute never holds up a look.
        thread::spawn(move || {
            let mut request = request;
            let response = answer_request(&mut request, &password, &call);
            let _ = request.respond(response);
        });
    }
}

// The answer to one HTTP request.
fn answer_request(request: &mut Request, password: &str, call: &ToolCall) -> Response<Cursor<Vec<u8>>> {
    if request.url() != "/mcp" {
        return reply(404, String::new());
    }
    if header_of(request, "Origin").is_some() {
        return reply(403, String::new());
    }
    let wanted = format!("Bearer {password}");
    if header_of(request, "Authorization") != Some(wanted) {
        return reply(401, String::new());
    }
    // The agent only posts. MCP lets a server without messages of its own
    // to send refuse the rest.
    if *request.method() != Method::Post {
        let allow = Header::from_bytes("Allow", "POST").unwrap();
        return reply(405, String::new()).with_header(allow);
    }

    let mut body = String::new();
    let mut reader = request.as_reader().take(MOST_BODY_BYTES);
    if reader.read_to_string(&mut body).is_err() {
        return reply(400, String::new());
    }
    let message: Value = match serde_json::from_str(&body) {
        Ok(message) => message,
        Err(error) => {
            let answer = error_answer(Value::Null, -32700, format!("the message is not JSON: {error}"));
            return reply(400, answer.to_string());
        }
    };
    match answer_message(&message, call.as_ref()) {
        Some(answer) => reply(200, answer.to_string()),
        // A notification is taken without an answer.
        None => reply(202, String::new()),
    }
}

fn reply(status: u16, body: String) -> Response<Cursor<Vec<u8>>> {
    let json = Header::from_bytes("Content-Type", "application/json").unwrap();
    Response::from_string(body).with_status_code(status).with_header(json)
}

// The value of header `name`, in any case, or None.
fn header_of(request: &Request, name: &'static str) -> Option<String> {
    for header in request.headers() {
        if header.field.equiv(name) {
            return Some(header.value.to_string());
        }
    }
    None
}

// The JSON-RPC answer to `message`, calling tools through `call`, or None
// for a message that gets none: a notification, or an answer to the agent.
pub fn answer_message(message: &Value, call: &dyn Fn(&str, &Value) -> ToolAnswer) -> Option<Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id")?.clone();
    let params = &message["params"];
    let result = match method {
        "initialize" => initialized(params),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools::list()}),
        "tools/call" => tool_result(params, call),
        _ => {
            let why = format!("Jello's tool server has no method called {method}");
            return Some(error_answer(id, -32601, why));
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

// The answer to the agent's first message: the version both speak, that
// Jello has tools, and what they are for.
fn initialized(params: &Value) -> Value {
    let mut version = PROTOCOL_VERSIONS[0];
    if let Some(asked) = params["protocolVersion"].as_str() {
        if PROTOCOL_VERSIONS.contains(&asked) {
            version = asked;
        }
    }
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "jello", "version": env!("CARGO_PKG_VERSION")},
        "instructions": INSTRUCTIONS,
    })
}

fn tool_result(params: &Value, call: &dyn Fn(&str, &Value) -> ToolAnswer) -> Value {
    let name = params["name"].as_str().unwrap_or_default();
    let mut arguments = params["arguments"].clone();
    if arguments.is_null() {
        arguments = json!({});
    }
    let answer = call(name, &arguments);
    // The lines saying what changed, and the card, go along for the agent's
    // panel, under MCP's _meta, which the model isn't shown.
    let mut meta = json!({"jello/changes": answer.changes});
    if let Some(card) = answer.card {
        meta["jello/card"] = card;
    }
    json!({
        "content": [{"type": "text", "text": answer.text}],
        "isError": answer.failed,
        "_meta": meta,
    })
}

fn error_answer(id: Value, code: i64, message: String) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

// Calls tool `name` on what `app` keeps for its commands. Like the
// commands, a tool that can change something opens the speakers first:
// before they open, nothing empties the engine's mailbox. Once one has
// changed something, the UI is told, so it redraws and the agent's panel
// shows the change lines.
fn call_tool(app: &AppHandle, name: &str, arguments: &Value) -> ToolAnswer {
    let changes = tools::changes_something(name);
    if changes {
        let mut engine = app.state::<Mutex<Engine>>().inner().lock().unwrap();
        if let Err(why) = engine.start_audio() {
            return ToolAnswer::failed(why);
        }
    }

    let answer = with_studio(app, |studio| tools::call(studio, name, arguments));
    if changes && !answer.failed {
        let _ = app.emit("agent-acted", json!({"tool": name, "changes": answer.changes}));
    }
    answer
}

// Runs `work` on the studio made of what `app` keeps for its commands, with
// the app's screen.
pub fn with_studio<T>(app: &AppHandle, work: impl FnOnce(&Studio) -> T) -> T {
    let open_song = app.state::<Mutex<OpenSong>>();
    let key_map = app.state::<Mutex<KeyMap>>();
    let composer = app.state::<Mutex<Composer>>();
    let engine = app.state::<Mutex<Engine>>();
    let phrases = app.state::<Phrases>();
    let offers = app.state::<Mutex<Offers>>();
    let practice = app.state::<Mutex<Practice>>();
    let memory = app.state::<Mutex<Memory>>();
    let screen = AppScreen { app };
    let studio = Studio {
        open_song: open_song.inner(),
        key_map: key_map.inner(),
        composer: composer.inner(),
        engine: engine.inner(),
        phrases: phrases.inner(),
        offers: offers.inner(),
        practice: practice.inner(),
        memory: memory.inner(),
        screen: &screen,
    };
    work(&studio)
}

// The app's screen: the UI, told through events.
struct AppScreen<'a> {
    app: &'a AppHandle,
}

impl Screen for AppScreen<'_> {
    fn ask(&self, question: &str) -> bool {
        ask_player(self.app, question)
    }

    fn show_key(&self, key: &str, down: bool) {
        let _ = self.app.emit("agent-key", json!({"key": key, "down": down}));
    }

    fn show_marks(&self, keys: &Value) {
        let _ = self.app.emit("agent-marks", json!({"keys": keys}));
    }

    // The panel's lane card follows the phrase's presses, in ms.
    fn show_phrase(&self, phrase: &Phrase) {
        let mut downs = Vec::new();
        for event in &phrase.events {
            if event.down {
                downs.push(json!({"key": event.key, "at_ms": event.at_ms}));
            }
        }
        let _ = self.app.emit("agent-phrase", json!({"length_ms": phrase.length_ms, "downs": downs}));
    }

    fn show_practice(&self, view: &Value) {
        let _ = self.app.emit("practice", view);
    }

    fn start_practice(&self) -> Result<(), String> {
        practice_run::start(self.app)
    }
}

// The questions tools are waiting on, each to be answered Yes or No in the
// agent's panel, by number.
#[derive(Default)]
pub struct Questions {
    asked: u64,
    waiting: HashMap<u64, Sender<bool>>,
}

// Asks the player `question` in the agent's panel and waits for the
// answer, for up to ANSWER_WAIT.
fn ask_player(app: &AppHandle, question: &str) -> bool {
    let questions = app.state::<Mutex<Questions>>();
    let (sender, receiver) = mpsc::channel();
    let mut waiting = questions.lock().unwrap();
    waiting.asked += 1;
    let number = waiting.asked;
    waiting.waiting.insert(number, sender);
    // Unlocked while it waits, so the answer can come in.
    drop(waiting);

    let _ = app.emit("agent-asks", json!({"number": number, "question": question}));
    let answer = receiver.recv_timeout(ANSWER_WAIT).unwrap_or(false);
    questions.lock().unwrap().waiting.remove(&number);
    answer
}

// The player's answer to question `number`: the panel's Yes or No.
#[tauri::command]
pub fn answer_agent(questions: State<'_, Mutex<Questions>>, number: u64, yes: bool) {
    let questions = questions.lock().unwrap();
    if let Some(sender) = questions.waiting.get(&number) {
        let _ = sender.send(yes);
    }
}

// Part of the agent panel's Stop (agent.rs): ends the phrase playing, and
// answers No to every question waiting.
pub fn stop_tools(phrases: &Phrases, questions: &Mutex<Questions>) {
    phrases.stop();
    let questions = questions.lock().unwrap();
    for sender in questions.waiting.values() {
        let _ = sender.send(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::net::TcpStream;

    // A tool call that answers with the tool's name, and fails for "bad".
    fn echo(name: &str, arguments: &Value) -> ToolAnswer {
        ToolAnswer {
            text: format!("{name} {arguments}"),
            changes: vec![format!("Called {name}")],
            failed: name == "bad",
            card: None,
        }
    }

    fn message(method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params})
    }

    #[test]
    fn it_speaks_the_version_asked_for_or_its_newest() {
        let asked = message("initialize", json!({"protocolVersion": "2025-06-18"}));
        let answer = answer_message(&asked, &echo).unwrap();
        assert_eq!(answer["id"], 7);
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(answer["result"]["serverInfo"]["name"], "jello");

        let unknown = message("initialize", json!({"protocolVersion": "2031-01-01"}));
        let answer = answer_message(&unknown, &echo).unwrap();
        assert_eq!(answer["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_answer() {
        let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert_eq!(answer_message(&notification, &echo), None);
    }

    #[test]
    fn it_lists_the_tools_and_calls_them() {
        let answer = answer_message(&message("tools/list", json!({})), &echo).unwrap();
        let tools = answer["result"]["tools"].as_array().unwrap();
        assert_eq!(tools[0]["name"], "jello_look");

        let call = message("tools/call", json!({"name": "jello_look", "arguments": {"parts": ["keys"]}}));
        let answer = answer_message(&call, &echo).unwrap();
        assert_eq!(answer["result"]["content"][0]["text"], "jello_look {\"parts\":[\"keys\"]}");
        assert_eq!(answer["result"]["isError"], false);
        assert_eq!(answer["result"]["_meta"]["jello/changes"], json!(["Called jello_look"]));

        // Arguments left out are none.
        let call = message("tools/call", json!({"name": "bad"}));
        let answer = answer_message(&call, &echo).unwrap();
        assert_eq!(answer["result"]["content"][0]["text"], "bad {}");
        assert_eq!(answer["result"]["isError"], true);
    }

    #[test]
    fn an_unknown_method_is_an_error() {
        let answer = answer_message(&message("resources/list", json!({})), &echo).unwrap();
        assert_eq!(answer["error"]["code"], -32601);
    }

    // Sends `request` to `server` as raw HTTP and returns the status and
    // body of its answer.
    fn send(server: &ToolServer, request: &str) -> (u16, String) {
        let address = server.url.trim_start_matches("http://").trim_end_matches("/mcp");
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        let status = answer[9..12].parse().unwrap();
        let body = answer.split("\r\n\r\n").nth(1).unwrap_or_default();
        (status, String::from(body))
    }

    fn post(password: &str, extra_header: &str, body: &str) -> String {
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {password}\r\n{extra_header}\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn it_answers_only_the_agent_with_the_password() {
        let server = start_with(Arc::new(echo)).unwrap();
        let ping = message("ping", json!({})).to_string();

        let (status, body) = send(&server, &post(&server.password, "", &ping));
        assert_eq!(status, 200);
        assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["result"], json!({}));

        let (status, _) = send(&server, &post("guess", "", &ping));
        assert_eq!(status, 401);

        // Not even with the password, from a web page.
        let origin = "Origin: http://example.com\r\n";
        let (status, _) = send(&server, &post(&server.password, origin, &ping));
        assert_eq!(status, 403);
    }

    #[test]
    fn it_takes_notifications_and_refuses_other_methods_and_paths() {
        let server = start_with(Arc::new(echo)).unwrap();
        let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
        let (status, _) = send(&server, &post(&server.password, "", &notification));
        assert_eq!(status, 202);

        let password = &server.password;
        let get = format!("GET /mcp HTTP/1.1\r\nAuthorization: Bearer {password}\r\nConnection: close\r\n\r\n");
        let (status, _) = send(&server, &get);
        assert_eq!(status, 405);

        let elsewhere = "GET /files HTTP/1.1\r\nConnection: close\r\n\r\n";
        let (status, _) = send(&server, elsewhere);
        assert_eq!(status, 404);
    }

    #[test]
    fn every_launch_has_its_own_password() {
        let first = new_password().unwrap();
        let second = new_password().unwrap();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }
}
