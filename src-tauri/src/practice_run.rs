// Plays practice tries (practice.rs), on a thread of its own. Each try
// counts in on the engine's click, plays Jello's parts on the live voices,
// reads the player's presses as Compose keeps them, and every 50 ms tells
// the Play tab what was hit, what was missed and which keys come next.
// Then it scores the try, keeps it, tells the agent, and, with a ladder or
// a loop, starts the next one.
//
// The loop pauses while practice plays, and plays again after. While a
// loop exists its tempo holds (the engine keeps the beat for it), so
// practice plays at the loop's tempo.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::compose::Composer;
use crate::engine::{Engine, LooperState};
use crate::key_map::{KeyMap, LAPTOP_VELOCITY};
use crate::practice::{self, BEATS_PER_BAR, MOST_PASSES, Piece, Plan, Practice, Settings, Sounding};
use crate::scoring::{self, Expected, Heard};
use crate::skills;

// How often the runner looks at the clock, and how often it tells the
// Play tab.
const TICK: Duration = Duration::from_millis(5);
const PROGRESS_EVERY_MS: u128 = 50;
// The quiet before a listen, and between tries.
const LEAD_MS: f64 = 600.0;
const BETWEEN_TRIES: Duration = Duration::from_millis(1200);
// The ear's answer ends this long after the last press, or after the
// longer wait if nothing is pressed.
const ANSWER_QUIET_MS: u128 = 3000;
const ANSWER_WAIT_MS: u128 = 10_000;
// How many keys after the next one are marked, numbered.
const LATER_MARKS: usize = 4;
// The most lines kept for the agent's next message.
const MOST_UNSENT: usize = 5;

// Starts a try of the piece set, on a thread of its own. Refused with no
// piece, while a try plays, or while the looper counts in or records.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let practice_state = app.state::<Mutex<Practice>>();
    let ladder = {
        let practice = practice_state.lock().unwrap();
        if practice.piece.is_none() {
            return Err(String::from("no piece is set"));
        }
        if practice.is_running() {
            return Err(String::from("a try is already playing"));
        }
        practice.settings.ladder
    };

    let engine_state = app.state::<Mutex<Engine>>();
    let loop_tempo = {
        let mut engine = engine_state.lock().unwrap();
        engine.start_audio()?;
        let status = engine.looper_status();
        let recording = match status.state {
            LooperState::CountingIn => true,
            LooperState::Recording => true,
            LooperState::Layering => true,
            _ => false,
        };
        if recording {
            return Err(String::from("the looper is recording: finish it first"));
        }
        let mut loop_tempo = None;
        if engine.is_looper_busy() {
            loop_tempo = Some(engine.tempo());
        }
        loop_tempo
    };
    if let Some(tempo) = loop_tempo {
        if ladder.is_some() {
            return Err(format!(
                "the loop holds the tempo at {tempo}, so the ladder can't speed up: turn the ladder off or remove the loop"
            ));
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut practice = practice_state.lock().unwrap();
        if practice.is_running() {
            return Err(String::from("a try is already playing"));
        }
        practice.running = Some(Arc::clone(&stop));
        practice.tempo_note = None;
        if let Some(tempo) = loop_tempo {
            if tempo != practice.settings.tempo {
                practice.tempo_note = Some(format!("the loop holds the tempo at {tempo}"));
            }
        }
        practice.phase = String::from("Starting");
    }
    show_view(app);
    let handle = app.clone();
    thread::spawn(move || run(handle, stop));
    Ok(())
}

fn run(app: AppHandle, stop: Arc<AtomicBool>) {
    let resume_loop = pause_loop(&app);
    let mut passes = 0;
    let mut listened = false;
    loop {
        let Some((piece, settings, tempo)) = snapshot(&app) else {
            break;
        };
        let tempo = set_engine_tempo(&app, tempo);
        let runner = Runner {
            app: &app,
            stop: &stop,
            settings: &settings,
            tempo,
        };
        let result = match settings.mode.as_str() {
            "ear" => runner.ear(&piece),
            "improvise" => runner.improvise(&piece),
            "memory" => {
                let heard_already = listened;
                listened = true;
                runner.along(&piece, !heard_already)
            }
            _ => {
                if settings.wait {
                    runner.wait(&piece)
                } else {
                    runner.along(&piece, false)
                }
            }
        };
        let Some(summary) = result else {
            break;
        };
        let done = keep_try(&app, summary, &settings, &piece, tempo);
        passes += 1;
        if done || passes >= MOST_PASSES {
            break;
        }
        let again = settings.loop_section || settings.ladder.is_some();
        if !again {
            break;
        }
        set_phase(&app, "Again");
        if !rest(&stop, BETWEEN_TRIES) {
            break;
        }
    }
    finish(&app, resume_loop);
}

// The piece, its settings and the tempo for the next try, or None if
// practice was closed.
fn snapshot(app: &AppHandle) -> Option<(Piece, Settings, u32)> {
    let practice = app.state::<Mutex<Practice>>();
    let practice = practice.lock().unwrap();
    let piece = practice.piece.clone()?;
    Some((piece, practice.settings.clone(), practice.tempo_now))
}

// Pauses the loop if it plays. Returns whether to play it again after.
fn pause_loop(app: &AppHandle) -> bool {
    let engine = app.state::<Mutex<Engine>>();
    let mut engine = engine.lock().unwrap();
    let playing = engine.looper_status().state == LooperState::Looping;
    if playing {
        engine.pause_loop();
    }
    playing
}

// Sets the engine to `tempo`, and returns the tempo it plays: a loop
// keeps its own.
fn set_engine_tempo(app: &AppHandle, tempo: u32) -> u32 {
    let engine = app.state::<Mutex<Engine>>();
    let mut engine = engine.lock().unwrap();
    engine.set_tempo(tempo)
}

// Practice ends: the click stops, the song's tempo comes back, the loop
// plays again if it played before, and the Play tab is told.
fn finish(app: &AppHandle, resume_loop: bool) {
    let tempo = app.state::<Mutex<Composer>>().lock().unwrap().tempo();
    {
        let engine = app.state::<Mutex<Engine>>();
        let mut engine = engine.lock().unwrap();
        engine.stop_click();
        engine.set_tempo(tempo);
        if resume_loop {
            engine.play_loop();
        }
    }
    {
        let practice = app.state::<Mutex<Practice>>();
        let mut practice = practice.lock().unwrap();
        practice.running = None;
        practice.phase = String::new();
    }
    let _ = app.emit("practice-marks", json!({"marks": []}));
    show_view(app);
}

fn show_view(app: &AppHandle) {
    let view = app.state::<Mutex<Practice>>().lock().unwrap().view();
    let _ = app.emit("practice", &view);
}

fn set_phase(app: &AppHandle, phase: &str) {
    app.state::<Mutex<Practice>>().lock().unwrap().phase = String::from(phase);
    let _ = app.emit("practice-phase", json!({"phase": phase}));
}

// Waits `length`, unless stopped first. Returns false if stopped.
fn rest(stop: &AtomicBool, length: Duration) -> bool {
    let until = Instant::now() + length;
    while Instant::now() < until {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        thread::sleep(TICK);
    }
    !stop.load(Ordering::Relaxed)
}

// Jello's notes in a try: each starts and ends on time on the live voices.
struct Backing {
    notes: Vec<Sounding>,
    started: Vec<bool>,
    ended: Vec<bool>,
}

impl Backing {
    fn new(notes: Vec<Sounding>) -> Backing {
        let count = notes.len();
        Backing {
            notes,
            started: vec![false; count],
            ended: vec![false; count],
        }
    }

    // Starts and ends the notes due at `now_ms`.
    fn play_until(&mut self, engine: &mut Engine, now_ms: f64) {
        for (place, note) in self.notes.iter().enumerate() {
            if !self.started[place] && note.at_ms <= now_ms {
                engine.note_on(note.pitch, LAPTOP_VELOCITY);
                self.started[place] = true;
            }
            if self.started[place] && !self.ended[place] && note.end_ms <= now_ms {
                engine.note_off(note.pitch);
                self.ended[place] = true;
            }
        }
    }

    // Ends every note still sounding.
    fn silence(&mut self, engine: &mut Engine) {
        for (place, note) in self.notes.iter().enumerate() {
            if self.started[place] && !self.ended[place] {
                engine.note_off(note.pitch);
                self.ended[place] = true;
            }
        }
    }

    fn last_end_ms(&self) -> f64 {
        let mut last: f64 = 0.0;
        for note in &self.notes {
            last = last.max(note.end_ms);
        }
        last
    }
}

// A try's clock: ms from its first beat, which the count in comes before,
// and the presses since it began, on the same clock. Compose keeps only its
// last presses, so the clock keeps its own.
struct Clock {
    first_beat: Instant,
    first_beat_session_ms: f64,
    first_press: u64,
    kept: Vec<(u64, Heard)>,
}

impl Clock {
    // A clock whose first beat is `lead_ms` from now.
    fn starting_in(app: &AppHandle, lead_ms: f64) -> Clock {
        let composer = app.state::<Mutex<Composer>>();
        let composer = composer.lock().unwrap();
        let now = Instant::now();
        let lead = Duration::from_secs_f64(lead_ms / 1000.0);
        Clock {
            first_beat: now + lead,
            first_beat_session_ms: composer.session_ms(now) as f64 + lead_ms,
            first_press: composer.next_press_id(),
            kept: Vec::new(),
        }
    }

    // Ms since the first beat: negative before it.
    fn now_ms(&self) -> f64 {
        let now = Instant::now();
        if now >= self.first_beat {
            return now.duration_since(self.first_beat).as_secs_f64() * 1000.0;
        }
        -(self.first_beat.duration_since(now).as_secs_f64() * 1000.0)
    }

    // The presses since the clock started, timed from its first beat. Only
    // presses still held and new ones are read again.
    fn heard(&mut self, app: &AppHandle) -> Vec<Heard> {
        let mut from = self.first_press;
        if let Some((last, _)) = self.kept.last() {
            from = last + 1;
        }
        for (id, heard) in &self.kept {
            if heard.held_ms.is_none() {
                from = *id;
                break;
            }
        }
        let presses = app.state::<Mutex<Composer>>().lock().unwrap().presses_from(from);
        for press in presses {
            let heard = Heard {
                pitches: press.notes,
                at_ms: press.pressed_ms as f64 - self.first_beat_session_ms,
                held_ms: press.held_ms.map(|held| held as f64),
            };
            let known = self.kept.iter().position(|(id, _)| *id == press.id);
            match known {
                Some(place) => self.kept[place].1 = heard,
                None => self.kept.push((press.id, heard)),
            }
        }
        let mut heard = Vec::new();
        for (_, press) in &self.kept {
            heard.push(press.clone());
        }
        heard
    }
}

struct Runner<'a> {
    app: &'a AppHandle,
    stop: &'a AtomicBool,
    settings: &'a Settings,
    tempo: u32,
}

impl Runner<'_> {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn engine(&self) -> std::sync::MutexGuard<'_, Engine> {
        self.app.state::<Mutex<Engine>>().inner().lock().unwrap()
    }

    fn count_in_ms(&self, plan: &Plan) -> f64 {
        f64::from(self.settings.count_in) * BEATS_PER_BAR * plan.ms_per_beat
    }

    // Jello plays the player's parts once, for the ear and from memory.
    // Returns false if stopped.
    fn listen(&self, piece: &Piece) -> bool {
        let plan = practice::listen_plan(piece, self.settings, self.tempo);
        set_phase(self.app, "Listen");
        let clock = Clock::starting_in(self.app, LEAD_MS);
        let mut backing = Backing::new(plan.jello);
        let end_ms = backing.last_end_ms().max(plan.length_ms);
        let _ = self.app.emit(
            "practice-started",
            json!({"phase": "listen", "count_in_ms": LEAD_MS, "ms_per_beat": plan.ms_per_beat, "length_ms": end_ms, "hidden": false}),
        );
        loop {
            if self.stopped() {
                backing.silence(&mut self.engine());
                return false;
            }
            let now_ms = clock.now_ms();
            backing.play_until(&mut self.engine(), now_ms);
            if now_ms > end_ms {
                break;
            }
            thread::sleep(TICK);
        }
        backing.silence(&mut self.engine());
        true
    }

    // Playing along, scored on time. From memory, the first try listens
    // first, and the lane hides.
    fn along(&self, piece: &Piece, listen_first: bool) -> Option<Value> {
        let hidden = self.settings.mode == "memory";
        if listen_first {
            if !self.listen(piece) {
                return None;
            }
            if !rest(self.stop, Duration::from_millis(LEAD_MS as u64)) {
                return None;
            }
        }
        let plan = practice::plan(piece, self.settings, self.tempo);
        let window = self.settings.window_ms();
        let mut last_expected_end: f64 = plan.length_ms;
        for note in &plan.expected {
            last_expected_end = last_expected_end.max(note.at_ms + note.hold_ms);
        }
        let mut backing = Backing::new(plan.jello.clone());
        let end_ms = last_expected_end.max(backing.last_end_ms()) + window;
        let mut clock = self.count_in(&plan, end_ms, hidden);
        let mut counting = clock.now_ms() < 0.0;

        let mut last_progress = Instant::now();
        let mut heard_count = 0;
        loop {
            if self.stopped() {
                self.end_sound(&mut backing);
                return None;
            }
            let now_ms = clock.now_ms();
            if counting && now_ms >= 0.0 {
                counting = false;
                self.end_count_in();
            }
            backing.play_until(&mut self.engine(), now_ms);
            let heard = clock.heard(self.app);
            let new_press = heard.len() != heard_count;
            heard_count = heard.len();
            if new_press || last_progress.elapsed().as_millis() >= PROGRESS_EVERY_MS {
                self.show_progress(&plan.expected, &heard, now_ms, window);
                last_progress = Instant::now();
            }
            if now_ms > end_ms {
                break;
            }
            thread::sleep(TICK);
        }
        self.end_sound(&mut backing);
        let heard = clock.heard(self.app);
        let scored = scoring::score(&plan.expected, &heard, window);
        let mut summary = json!({
            "clean": scored.clean,
            "whole": scored.whole,
            "left": scored.left,
            "right": scored.right,
            "sections": sections_json(&scored.sections),
            "wrong": scored.wrong,
            "extras": scored.extras,
            "together_ms": scored.together_ms,
            "notes": scored.notes,
        });
        summary["hidden"] = json!(hidden);
        Some(summary)
    }

    // Starts the count in, and the click with it, and says the try started.
    // Returns the try's clock and whether the click is on.
    fn count_in(&self, plan: &Plan, end_ms: f64, hidden: bool) -> Clock {
        let count_in_ms = self.count_in_ms(plan);
        let clicking = self.settings.click || count_in_ms > 0.0;
        // The clock reads Compose's, which is locked before the engine.
        let clock = Clock::starting_in(self.app, count_in_ms);
        if clicking {
            self.engine().start_click();
        }
        if count_in_ms > 0.0 {
            set_phase(self.app, "Counting in");
        } else {
            set_phase(self.app, "Playing");
        }
        let _ = self.app.emit(
            "practice-started",
            json!({
                "phase": "play",
                "count_in_ms": count_in_ms,
                "ms_per_beat": plan.ms_per_beat,
                "length_ms": end_ms,
                "hidden": hidden,
                "tempo": self.tempo,
            }),
        );
        clock
    }

    // The count in is over: the click stops unless it was asked for all
    // through, and the try says it is playing.
    fn end_count_in(&self) {
        if !self.settings.click {
            self.engine().stop_click();
        }
        set_phase(self.app, "Playing");
    }

    fn end_sound(&self, backing: &mut Backing) {
        let mut engine = self.engine();
        engine.stop_click();
        backing.silence(&mut engine);
    }

    // The Play tab's progress: the notes hit and missed so far, and the
    // keys to press next.
    fn show_progress(&self, expected: &[Expected], heard: &[Heard], now_ms: f64, window: f64) {
        let mut due = Vec::new();
        for note in expected {
            if note.at_ms <= now_ms + window {
                due.push(note.clone());
            }
        }
        let scored = scoring::score(&due, heard, window);
        let mut hit = Vec::new();
        let mut missed = Vec::new();
        let mut done = vec![false; expected.len()];
        for (place, result) in scored.notes.iter().enumerate() {
            if result.hit {
                hit.push(result.index);
                done[place] = true;
            } else if due[place].at_ms + window < now_ms {
                missed.push(result.index);
                done[place] = true;
            }
        }
        let marks = self.marks(expected, &done);
        let _ = self.app.emit("practice-progress", json!({"at_ms": now_ms, "hit": hit, "missed": missed}));
        let _ = self.app.emit("practice-marks", json!({"marks": marks}));
    }

    // The keyboard's marks: the next notes outlined, the ones after them
    // numbered. `done` says which of `expected` are over.
    fn marks(&self, expected: &[Expected], done: &[bool]) -> Value {
        let key_map = self.app.state::<Mutex<KeyMap>>();
        let key_map = key_map.lock().unwrap();
        let mut marks = Vec::new();
        let mut group = 0;
        let mut group_at: Option<f64> = None;
        for (place, note) in expected.iter().enumerate() {
            if place < done.len() && done[place] {
                continue;
            }
            if let Some(at) = group_at {
                if (note.at_ms - at).abs() > 1.0 {
                    group += 1;
                }
            }
            group_at = Some(note.at_ms);
            if group > LATER_MARKS {
                break;
            }
            let Some(key) = practice::key_for(&key_map, &note.pitches) else {
                continue;
            };
            // A key played again later keeps its nearest mark.
            let marked = marks.iter().any(|mark: &Value| mark["key"] == key.as_str());
            if marked {
                continue;
            }
            let state = if group == 0 { "next" } else { "later" };
            marks.push(json!({"key": key, "label": (group + 1).to_string(), "state": state}));
        }
        json!(marks)
    }

    // Waiting for the right notes: no clock, each step waits until its
    // notes are pressed.
    fn wait(&self, piece: &Piece) -> Option<Value> {
        let plan = practice::plan(piece, self.settings, self.tempo);
        let steps = steps_of(&plan.expected);
        set_phase(self.app, "Your turn");
        let _ = self.app.emit(
            "practice-started",
            json!({"phase": "wait", "count_in_ms": 0, "ms_per_beat": plan.ms_per_beat, "length_ms": plan.length_ms, "hidden": false}),
        );
        let mut clock = Clock::starting_in(self.app, 0.0);
        let started = Instant::now();
        let mut step = 0;
        let mut waiting: Vec<Expected> = Vec::new();
        if let Some(first) = steps.first() {
            waiting = first.clone();
        }
        let mut read = 0;
        let mut wrong = 0;
        let mut done = vec![false; plan.expected.len()];
        let mut shown = false;
        while step < steps.len() {
            if self.stopped() {
                return None;
            }
            let heard = clock.heard(self.app);
            for press in &heard[read..] {
                if step >= steps.len() {
                    break;
                }
                let found = waiting.iter().position(|note| note.pitches == press.pitches);
                match found {
                    Some(place) => {
                        let note = waiting.remove(place);
                        mark_done(&plan.expected, &mut done, &note);
                    }
                    None => wrong += 1,
                }
                if waiting.is_empty() {
                    step += 1;
                    if step < steps.len() {
                        waiting = steps[step].clone();
                    }
                }
            }
            let changed = heard.len() != read;
            read = heard.len();
            if changed || !shown {
                let marks = self.marks(&plan.expected, &done);
                let _ = self.app.emit("practice-progress", json!({"step": step, "steps": steps.len(), "hit": done_indices(&plan.expected, &done), "missed": []}));
                let _ = self.app.emit("practice-marks", json!({"marks": marks}));
                shown = true;
            }
            thread::sleep(TICK);
        }
        let seconds = (started.elapsed().as_secs_f64() * 10.0).round() / 10.0;
        Some(json!({
            "clean": wrong == 0,
            "waited": {"steps": steps.len(), "wrong": wrong, "seconds": seconds},
        }))
    }

    // The ear: Jello plays the player's parts, then the player plays them
    // back, scored in order, without timing.
    fn ear(&self, piece: &Piece) -> Option<Value> {
        if !self.listen(piece) {
            return None;
        }
        let plan = practice::plan(piece, self.settings, self.tempo);
        let steps = steps_of(&plan.expected).len();
        set_phase(self.app, "Your turn");
        let _ = self.app.emit(
            "practice-started",
            json!({"phase": "answer", "count_in_ms": 0, "ms_per_beat": plan.ms_per_beat, "length_ms": 0, "hidden": true}),
        );
        let mut clock = Clock::starting_in(self.app, 0.0);
        let started = Instant::now();
        let mut last_press = None;
        let mut count = 0;
        loop {
            if self.stopped() {
                return None;
            }
            let heard = clock.heard(self.app);
            if heard.len() != count {
                count = heard.len();
                last_press = Some(Instant::now());
                let _ = self.app.emit("practice-progress", json!({"answered": count, "steps": steps}));
            }
            let all_let_go = heard.iter().all(|press| press.held_ms.is_some());
            let quiet = match last_press {
                Some(at) => at.elapsed().as_millis() >= ANSWER_QUIET_MS,
                None => started.elapsed().as_millis() >= ANSWER_WAIT_MS,
            };
            let answered = count >= steps && all_let_go;
            if quiet || answered {
                break;
            }
            thread::sleep(TICK);
        }
        let heard = clock.heard(self.app);
        let echoed = scoring::echo(&plan.expected, &heard);
        Some(json!({
            "clean": echoed.right == echoed.steps && echoed.extra_presses == 0,
            "echo": echoed,
        }))
    }

    // Improvising over Jello's parts, with the rules measured.
    fn improvise(&self, piece: &Piece) -> Option<Value> {
        let plan = practice::plan(piece, self.settings, self.tempo);
        let window = self.settings.window_ms();
        let mut backing = Backing::new(plan.jello.clone());
        let end_ms = plan.length_ms.max(backing.last_end_ms());
        let mut clock = self.count_in(&plan, end_ms, true);
        let mut counting = clock.now_ms() < 0.0;
        loop {
            if self.stopped() {
                self.end_sound(&mut backing);
                return None;
            }
            let now_ms = clock.now_ms();
            if counting && now_ms >= 0.0 {
                counting = false;
                self.end_count_in();
            }
            backing.play_until(&mut self.engine(), now_ms);
            if now_ms > end_ms {
                break;
            }
            thread::sleep(TICK);
        }
        self.end_sound(&mut backing);
        let mut heard = Vec::new();
        for press in clock.heard(self.app) {
            if press.at_ms >= -window {
                heard.push(press);
            }
        }
        let chords = chords_per_beat(&plan);
        let key = self.app.state::<Mutex<Composer>>().lock().unwrap().key();
        let measured = scoring::improvised(&heard, &chords, key, plan.ms_per_beat, window);
        let mut clean = false;
        if let (Some(in_key), Some(chord_tones)) = (measured.in_key_percent, measured.chord_tones_on_strong_beats_percent) {
            clean = in_key >= 80.0 && chord_tones >= 60.0;
        }
        Some(json!({"clean": clean, "improvised": measured}))
    }
}

// The player's notes in steps: those starting together are one.
fn steps_of(expected: &[Expected]) -> Vec<Vec<Expected>> {
    let mut steps: Vec<Vec<Expected>> = Vec::new();
    for note in expected {
        if let Some(step) = steps.last_mut() {
            if (step[0].at_ms - note.at_ms).abs() <= 1.0 {
                step.push(note.clone());
                continue;
            }
        }
        steps.push(vec![note.clone()]);
    }
    steps
}

fn mark_done(expected: &[Expected], done: &mut [bool], note: &Expected) {
    for (place, other) in expected.iter().enumerate() {
        if !done[place] && other == note {
            done[place] = true;
            return;
        }
    }
}

fn done_indices(expected: &[Expected], done: &[bool]) -> Vec<usize> {
    let mut indices = Vec::new();
    for (place, note) in expected.iter().enumerate() {
        if done[place] && !indices.contains(&note.index) {
            indices.push(note.index);
        }
    }
    indices
}

// The pitches Jello sounds on each beat, or None for none.
fn chords_per_beat(plan: &Plan) -> Vec<Option<Vec<u8>>> {
    let beats = (plan.length_ms / plan.ms_per_beat).round() as usize;
    let mut chords = Vec::new();
    for beat in 0..beats {
        let at = beat as f64 * plan.ms_per_beat + 1.0;
        let mut pitches = Vec::new();
        for note in &plan.jello {
            if note.at_ms <= at && at < note.end_ms {
                pitches.push(note.pitch);
            }
        }
        pitches.sort();
        pitches.dedup();
        if pitches.is_empty() {
            chords.push(None);
        } else {
            chords.push(Some(pitches));
        }
    }
    chords
}

fn sections_json(sections: &[(String, scoring::Tally)]) -> Value {
    let mut listed = Vec::new();
    for (name, tally) in sections {
        listed.push(json!({"name": name, "tally": tally}));
    }
    json!(listed)
}

// Keeps a finished try: in this practice's tries, in practice.jsonl, for
// the agent's next message, and on the Play tab. Returns whether the
// ladder is done.
fn keep_try(app: &AppHandle, results: Value, settings: &Settings, piece: &Piece, tempo: u32) -> bool {
    let clean = results["clean"].as_bool().unwrap_or(false);
    let practice_state = app.state::<Mutex<Practice>>();
    let mut practice = practice_state.lock().unwrap();
    let number = practice.tries.len() + 1;
    let summary = summary_of(number, results, settings, piece, tempo);
    let coach = coach_of(&summary);
    practice.tries.push(summary.clone());
    practice.tell_agent(format!("practice try {number} ended: {coach}"));
    keep_last_unsent(&mut practice);
    let done = practice::climb(&mut practice, clean);
    drop(practice);

    let _ = append_to_file(app, &summary);
    let _ = app.emit("practice-try", json!({"number": number, "summary": summary, "coach": settings.coach}));
    show_view(app);
    done
}

fn keep_last_unsent(practice: &mut Practice) {
    let mut lines = practice.take_unsent();
    if lines.len() > MOST_UNSENT {
        lines.drain(..lines.len() - MOST_UNSENT);
    }
    for line in lines {
        practice.tell_agent(line);
    }
}

// A try's results with what was practiced.
pub fn summary_of(number: usize, results: Value, settings: &Settings, piece: &Piece, tempo: u32) -> Value {
    let mut at = 0;
    if let Ok(since) = SystemTime::now().duration_since(UNIX_EPOCH) {
        at = since.as_secs();
    }
    let mut summary = json!({
        "try": number,
        "at": at,
        "piece": piece.name,
        "area": skills::area_for(piece.area.as_deref(), &settings.mode),
        "mode": settings.mode,
        "wait": settings.wait,
        "tempo": tempo,
        "level": settings.level,
        "window_ms": settings.window_ms(),
        "section": settings.section,
        "hands": settings.hands,
        "swing": settings.swing,
    });
    if let (Some(fields), Some(results)) = (summary.as_object_mut(), results.as_object()) {
        for (name, value) in results {
            fields.insert(name.clone(), value.clone());
        }
    }
    summary
}

// The try as the agent reads it: the per-note results become the notes
// missed and the three furthest off.
pub fn coach_of(summary: &Value) -> Value {
    let mut coach = summary.clone();
    let Some(fields) = coach.as_object_mut() else {
        return coach;
    };
    let Some(notes) = fields.remove("notes") else {
        return coach;
    };
    let mut missed = Vec::new();
    let mut offsets = Vec::new();
    for note in notes.as_array().cloned().unwrap_or_default() {
        let index = note["index"].as_u64().unwrap_or(0) + 1;
        if note["hit"] == json!(false) {
            if !missed.contains(&index) {
                missed.push(index);
            }
            continue;
        }
        if let Some(offset) = note["offset_ms"].as_f64() {
            offsets.push((index, offset));
        }
    }
    offsets.sort_by(|first, second| second.1.abs().total_cmp(&first.1.abs()));
    offsets.truncate(3);
    let mut furthest = Vec::new();
    for (index, offset) in offsets {
        furthest.push(json!({"note": index, "offset_ms": offset.round()}));
    }
    fields.insert(String::from("missed_notes"), json!(missed));
    fields.insert(String::from("furthest_off"), json!(furthest));
    coach
}

// Adds the try to Music/Jello/Memory/practice.jsonl, one line each, for
// skills measured over time.
fn append_to_file(app: &AppHandle, summary: &Value) -> Result<(), String> {
    let folder = commands::memory_folder(app)?;
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join("practice.jsonl"))
        .map_err(|error| error.to_string())?;
    let line = coach_of(summary).to_string();
    writeln!(file, "{line}").map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::Hand;

    fn expected(index: usize, pitch: u8, at_ms: f64) -> Expected {
        Expected {
            index,
            hand: Hand::Right,
            pitches: vec![pitch],
            at_ms,
            hold_ms: 400.0,
            section: None,
        }
    }

    #[test]
    fn notes_starting_together_are_one_step() {
        let notes = vec![expected(0, 60, 0.0), expected(1, 64, 0.0), expected(2, 67, 500.0)];
        let steps = steps_of(&notes);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].len(), 2);
    }

    #[test]
    fn chords_per_beat_follow_jello_s_notes() {
        let plan = Plan {
            expected: Vec::new(),
            jello: vec![
                Sounding { pitch: 48, at_ms: 0.0, end_ms: 1000.0 },
                Sounding { pitch: 52, at_ms: 0.0, end_ms: 1000.0 },
                Sounding { pitch: 53, at_ms: 1500.0, end_ms: 2000.0 },
            ],
            length_ms: 2000.0,
            ms_per_beat: 500.0,
            from_beat: 0.0,
        };
        let chords = chords_per_beat(&plan);
        assert_eq!(chords, vec![Some(vec![48, 52]), Some(vec![48, 52]), None, Some(vec![53])]);
    }

    #[test]
    fn the_coach_reads_misses_and_the_furthest_off() {
        let summary = json!({
            "try": 1,
            "notes": [
                {"index": 0, "hit": true, "offset_ms": 10.0},
                {"index": 1, "hit": false},
                {"index": 2, "hit": true, "offset_ms": -80.0},
                {"index": 3, "hit": true, "offset_ms": 30.0},
                {"index": 4, "hit": true, "offset_ms": 5.0},
            ],
        });
        let coach = coach_of(&summary);
        assert!(coach.get("notes").is_none());
        assert_eq!(coach["missed_notes"], json!([2]));
        assert_eq!(coach["furthest_off"][0], json!({"note": 3, "offset_ms": -80.0}));
        assert_eq!(coach["furthest_off"].as_array().unwrap().len(), 3);
    }
}
