// The jam partner (docs/design.md#ai-and-learning-rethought-2026-11-03):
// call and answer with the AI. The player plays a 2 bar call on the click;
// the call goes to the model in words (agent-host/host.mjs answerCall), and
// its answer, a note per line, plays on the picked track's live voices.
// A model takes seconds, so at first the jam takes turns: the click stops
// while the AI works, then counts in a bar and the answer plays. After 8
// bars of call and answer the click keeps going instead, and the answer
// comes in on the next phrase line. Looping everything so far while the AI
// works, and the groove that follows, come next (docs/tasks.md).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent;
use crate::compose::{Composer, HeardPress};
use crate::engine::Engine;
use crate::harmony::key_name;
use crate::key_map::LAPTOP_VELOCITY;
use crate::song_file::{note_name, note_of_name};

const BEATS_PER_BAR: f64 = 4.0;
// How long a call and an answer last, in bars.
const CALL_BARS: f64 = 2.0;
// Bars of call and answer before the click keeps going while the AI works.
const BARS_UNTIL_LOOPING: f64 = 8.0;
// How long Jello waits for an answer.
const ANSWER_WAIT: Duration = Duration::from_secs(30);
// Calls in a row with no press before the partner stops.
const MOST_SILENT_CALLS: u32 = 2;
// The most notes an answer plays.
const MOST_ANSWER_NOTES: usize = 64;
// A press this much of a beat before the call's first beat still counts.
const EARLY_BEATS: f64 = 0.25;
// In looping, an answer ready this close to a phrase line waits for the next.
const LINE_MARGIN: Duration = Duration::from_millis(100);
const TICK: Duration = Duration::from_millis(5);

// A note of the answer, in beats from its start.
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerNote {
    pub note: u8,
    pub start: f64,
    pub hold: f64,
}

#[derive(Default)]
pub struct Partner {
    running: Arc<AtomicBool>,
    round: u64,
    // The round the answer is for, and its notes or why there are none.
    answer: Option<(u64, Result<Vec<AnswerNote>, String>)>,
}

// The call's presses in words, one per line, like "C4 E4 G4 at 0.5 for 1
// beats", timed in beats from `first_beat_ms` on the composer's clock.
// Presses outside the call's `beats` are left out; one still held is held
// to the call's end.
pub fn call_lines(presses: &[HeardPress], first_beat_ms: f64, ms_per_beat: f64, beats: f64) -> Vec<String> {
    let mut lines = Vec::new();
    for press in presses {
        if press.notes.is_empty() {
            continue;
        }
        let start = (press.pressed_ms as f64 - first_beat_ms) / ms_per_beat;
        if start < -EARLY_BEATS || start >= beats {
            continue;
        }
        let start = start.max(0.0);
        let mut hold = beats - start;
        if let Some(held_ms) = press.held_ms {
            hold = hold.min(held_ms as f64 / ms_per_beat);
        }
        let mut names = Vec::new();
        for note in &press.notes {
            names.push(note_name(*note));
        }
        lines.push(format!("{} at {} for {} beats", names.join(" "), quarter(start), quarter(hold)));
    }
    lines
}

// `beats` to the nearest quarter beat, without a trailing ".0".
fn quarter(beats: f64) -> String {
    let rounded = (beats * 4.0).round() / 4.0;
    if rounded.fract() == 0.0 {
        return format!("{}", rounded as i64);
    }
    format!("{rounded}")
}

// What the model is asked: the call, its length, tempo and key.
pub fn call_message(lines: &[String], tempo: u32, key: &str) -> String {
    let mut message = format!(
        "Call, {} bars of {} beats at {tempo} bpm, in {key}. Answer with {} beats:\n",
        CALL_BARS, BEATS_PER_BAR, CALL_BARS * BEATS_PER_BAR
    );
    message.push_str(&lines.join("\n"));
    message
}

// The notes in the model's answer `text`: lines like "E4 1 0.5" (note,
// start beat, beats held). Lines that aren't notes are left out, notes
// starting outside `beats` too, and holds end by `beats`. Fails when no
// note is left.
pub fn answer_notes(text: &str, beats: f64) -> Result<Vec<AnswerNote>, String> {
    let mut notes = Vec::new();
    for line in text.lines() {
        if notes.len() == MOST_ANSWER_NOTES {
            break;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() != 3 {
            continue;
        }
        let Some(note) = note_of_name(words[0]) else {
            continue;
        };
        let Ok(start) = words[1].parse::<f64>() else {
            continue;
        };
        let Ok(hold) = words[2].parse::<f64>() else {
            continue;
        };
        if !start.is_finite() || !hold.is_finite() || hold <= 0.0 {
            continue;
        }
        if start < 0.0 || start >= beats {
            continue;
        }
        let hold = hold.min(beats - start);
        notes.push(AnswerNote { note, start, hold });
    }
    if notes.is_empty() {
        return Err(String::from("the answer had no notes to play"));
    }
    Ok(notes)
}

// The host's answer to round `round`, from agent.rs.
pub fn hear_host(app: &AppHandle, event: &Value) {
    if event["type"].as_str() != Some("partner_answer") {
        return;
    }
    let Some(round) = event["round"].as_u64() else {
        return;
    };
    let answer = match event["text"].as_str() {
        Some(text) => answer_notes(text, CALL_BARS * BEATS_PER_BAR),
        None => Err(event["why"].as_str().unwrap_or("no answer").to_string()),
    };
    let partner = app.state::<Mutex<Partner>>();
    let mut partner = partner.lock().unwrap();
    if partner.round == round {
        partner.answer = Some((round, answer));
    }
}

// Tells the rail what the partner is doing: "count_in", "call",
// "thinking", "answer", or "off" with what ended it.
fn say(app: &AppHandle, state: &str, said: &str, looping: bool) {
    let _ = app.emit("partner", json!({"state": state, "said": said, "looping": looping}));
}

// Waits until `due`. False if the partner was stopped first.
fn wait_until(due: Instant, running: &AtomicBool) -> bool {
    loop {
        if !running.load(Ordering::SeqCst) {
            return false;
        }
        let now = Instant::now();
        if now >= due {
            return true;
        }
        thread::sleep((due - now).min(TICK));
    }
}

// Plays `notes` from `at`, `ms_per_beat` a beat. False if stopped first;
// every note still sounding is let go either way.
fn play_answer(app: &AppHandle, notes: &[AnswerNote], at: Instant, ms_per_beat: f64, running: &AtomicBool) -> bool {
    let mut events: Vec<(f64, bool, u8)> = Vec::new();
    for note in notes {
        events.push((note.start * ms_per_beat, true, note.note));
        events.push(((note.start + note.hold) * ms_per_beat, false, note.note));
    }
    // A note let go comes before one pressed at the same moment.
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut sounding: Vec<u8> = Vec::new();
    let mut finished = true;
    for (at_ms, down, note) in events {
        let due = at + Duration::from_secs_f64(at_ms / 1000.0);
        if !wait_until(due, running) {
            finished = false;
            break;
        }
        let engine = app.state::<Mutex<Engine>>();
        let mut engine = engine.lock().unwrap();
        if down {
            engine.note_on(note, LAPTOP_VELOCITY);
            sounding.push(note);
        } else {
            engine.note_off(note);
            sounding.retain(|held| *held != note);
        }
    }
    let engine = app.state::<Mutex<Engine>>();
    let mut engine = engine.lock().unwrap();
    for note in sounding {
        engine.note_off(note);
    }
    finished
}

fn start_click(app: &AppHandle) -> Instant {
    app.state::<Mutex<Engine>>().lock().unwrap().start_click();
    Instant::now()
}

fn stop_click(app: &AppHandle) {
    app.state::<Mutex<Engine>>().lock().unwrap().stop_click();
}

// Waits for round `round`'s answer. Err says why there is none.
fn wait_for_answer(app: &AppHandle, round: u64, running: &AtomicBool) -> Result<Vec<AnswerNote>, String> {
    let asked = Instant::now();
    loop {
        if !running.load(Ordering::SeqCst) {
            return Err(String::new());
        }
        {
            let partner = app.state::<Mutex<Partner>>();
            let mut partner = partner.lock().unwrap();
            let ready = match &partner.answer {
                Some((answered, _)) => *answered == round,
                None => false,
            };
            if ready {
                let (_, answer) = partner.answer.take().unwrap();
                return answer;
            }
        }
        if asked.elapsed() >= ANSWER_WAIT {
            return Err(String::from("the AI took too long to answer"));
        }
        thread::sleep(TICK);
    }
}

// The first phrase line, `length` apart from `from`, at least LINE_MARGIN
// after `now`, so an answer ready just before a line waits for the next.
fn next_line(from: Instant, length: Duration, now: Instant) -> Instant {
    let mut line = from;
    while line < now + LINE_MARGIN {
        line += length;
    }
    line
}

// Rounds of call and answer until Stop or a problem. Returns what ended it,
// or "" for Stop.
fn run(app: &AppHandle, running: &AtomicBool) -> String {
    let tempo = app.state::<Mutex<Engine>>().lock().unwrap().tempo();
    let ms_per_beat = 60_000.0 / f64::from(tempo);
    let bar = Duration::from_secs_f64(ms_per_beat * BEATS_PER_BAR / 1000.0);
    let call_beats = CALL_BARS * BEATS_PER_BAR;
    let call_length = Duration::from_secs_f64(ms_per_beat * call_beats / 1000.0);
    let mut bars_played = 0.0;
    let mut silent = 0;
    let mut looping = false;

    say(app, "count_in", "", looping);
    let mut call_at = start_click(app) + bar;
    loop {
        let first_press = app.state::<Mutex<Composer>>().lock().unwrap().next_press_id();
        if !wait_until(call_at, running) {
            return String::new();
        }
        say(app, "call", "", looping);
        let call_end = call_at + call_length;
        if !wait_until(call_end, running) {
            return String::new();
        }
        let (lines, key) = {
            let composer = app.state::<Mutex<Composer>>();
            let composer = composer.lock().unwrap();
            let first_beat_ms = composer.session_ms(call_at) as f64;
            let presses = composer.presses_from(first_press);
            let lines = call_lines(&presses, first_beat_ms, ms_per_beat, call_beats);
            let key = match composer.key() {
                Some(key) => key_name(key),
                None => String::from("no set key"),
            };
            (lines, key)
        };
        if lines.is_empty() {
            silent += 1;
            if silent >= MOST_SILENT_CALLS {
                return String::from("No call heard, so the partner stopped.");
            }
            call_at = call_end;
            continue;
        }
        silent = 0;

        if !looping {
            stop_click(app);
        }
        say(app, "thinking", "", looping);
        let round = {
            let partner = app.state::<Mutex<Partner>>();
            let mut partner = partner.lock().unwrap();
            partner.round += 1;
            partner.answer = None;
            partner.round
        };
        let message = json!({"type": "partner_call", "round": round, "call": call_message(&lines, tempo, &key)});
        if !agent::tell_host_if_running(app, message) {
            return String::from("The agent isn't running: open the rail and sign in.");
        }
        let notes = match wait_for_answer(app, round, running) {
            Ok(notes) => notes,
            Err(why) => return why,
        };

        let answer_at = if looping {
            next_line(call_end, call_length, Instant::now())
        } else {
            say(app, "count_in", "", looping);
            start_click(app) + bar
        };
        if !wait_until(answer_at, running) {
            return String::new();
        }
        say(app, "answer", "", looping);
        if !play_answer(app, &notes, answer_at, ms_per_beat, running) {
            return String::new();
        }
        bars_played += CALL_BARS * 2.0;
        if bars_played >= BARS_UNTIL_LOOPING {
            looping = true;
        }
        call_at = answer_at + call_length;
    }
}

// The partner starts: a bar of count in, then the player's first call.
#[tauri::command]
pub fn start_partner(app: AppHandle, partner: State<'_, Mutex<Partner>>) -> Result<(), String> {
    let running = {
        let partner = partner.lock().unwrap();
        if partner.running.swap(true, Ordering::SeqCst) {
            return Err(String::from("the partner is playing already"));
        }
        partner.running.clone()
    };
    if let Err(problem) = app.state::<Mutex<Engine>>().lock().unwrap().start_audio() {
        running.store(false, Ordering::SeqCst);
        return Err(problem);
    }
    let app = app.clone();
    thread::spawn(move || {
        let ended = run(&app, &running);
        stop_click(&app);
        running.store(false, Ordering::SeqCst);
        say(&app, "off", &ended, false);
    });
    Ok(())
}

#[tauri::command]
pub fn stop_partner(partner: State<'_, Mutex<Partner>>) {
    partner.lock().unwrap().running.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(notes: &[u8], pressed_ms: u64, held_ms: Option<u64>) -> HeardPress {
        HeardPress { id: 0, notes: notes.to_vec(), pressed_ms, held_ms }
    }

    #[test]
    fn a_call_is_its_presses_in_beats_from_its_first_beat() {
        // 120 bpm: 500 ms a beat, the call's first beat at 10 s.
        let presses = [
            press(&[60], 9_950, Some(500)),
            press(&[60, 64, 67], 10_750, Some(1000)),
            press(&[72], 13_500, None),
            press(&[74], 14_000, Some(100)),
            press(&[50], 9_000, Some(100)),
        ];
        let lines = call_lines(&presses, 10_000.0, 500.0, 8.0);
        assert_eq!(lines, vec!["C4 at 0 for 1 beats", "C4 E4 G4 at 1.5 for 2 beats", "C5 at 7 for 1 beats"]);
        let off_the_beat = [press(&[60], 10_125, Some(375))];
        assert_eq!(call_lines(&off_the_beat, 10_000.0, 500.0, 8.0), vec!["C4 at 0.25 for 0.75 beats"]);
        let past_the_end = [press(&[72], 13_000, Some(3000))];
        assert_eq!(call_lines(&past_the_end, 10_000.0, 500.0, 8.0), vec!["C5 at 6 for 2 beats"]);
    }

    #[test]
    fn the_model_is_asked_with_the_call_s_length_tempo_and_key() {
        let message = call_message(&[String::from("C4 at 0 for 1 beats")], 96, "C major");
        assert_eq!(message, "Call, 2 bars of 4 beats at 96 bpm, in C major. Answer with 8 beats:\nC4 at 0 for 1 beats");
    }

    #[test]
    fn an_answer_keeps_only_notes_inside_the_call() {
        let text = "Here you go:\nE4 0 1\nG4 1 0.5\nC5 6 4\nX9 2 1\nA4 9 1\nB4 -1 1\nD4 2 0\nF4 2.5 1 extra";
        let notes = answer_notes(text, 8.0).unwrap();
        assert_eq!(
            notes,
            vec![
                AnswerNote { note: 64, start: 0.0, hold: 1.0 },
                AnswerNote { note: 67, start: 1.0, hold: 0.5 },
                AnswerNote { note: 72, start: 6.0, hold: 2.0 },
            ]
        );
        assert!(answer_notes("I can't play that.", 8.0).is_err());
    }

    #[test]
    fn a_looping_answer_comes_in_on_the_next_phrase_line() {
        let start = Instant::now();
        let length = Duration::from_secs(4);
        // Ready 1 s after the call ended: the next line.
        assert_eq!(next_line(start, length, start + Duration::from_secs(1)), start + length);
        // Ready just before a line: too close, so the next.
        let close = start + length - Duration::from_millis(50);
        assert_eq!(next_line(start, length, close), start + length * 2);
        // Ready before the call even ended: the call's end.
        assert_eq!(next_line(start, length, start - Duration::from_secs(1)), start);
    }

    #[test]
    fn an_answer_plays_at_most_its_most_notes() {
        let mut text = String::new();
        for place in 0..100 {
            text.push_str(&format!("C4 {} 0.05\n", f64::from(place) * 0.07));
        }
        assert_eq!(answer_notes(&text, 8.0).unwrap().len(), MOST_ANSWER_NOTES);
    }
}
