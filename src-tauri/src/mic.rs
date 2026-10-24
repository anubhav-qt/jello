// Listening to the mic (docs/design.md#voice), for dictation and humming.
// The engine keeps what the mic hears in its ring until it is read; a
// thread here takes it out every 50 ms, keeps it, and tells the screen how
// loud it is. The sound stays on this computer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine::Engine;

// How often what was heard is taken out of the engine.
const DRAIN_EVERY: Duration = Duration::from_millis(50);
// The longest listen kept, in seconds. Anything after is let go.
pub const MOST_SECONDS: u32 = 120;
// The rate speech models and the pitch finder listen at.
pub const LISTENING_RATE: u32 = 16_000;

// What is heard so far, at the engine's rate, shared with whoever turns it
// into words or notes as it comes.
pub type Heard = Arc<Mutex<Vec<f32>>>;

// The mic, while something listens.
#[derive(Default)]
pub struct Mic {
    listening: Option<Listening>,
}

struct Listening {
    stop: Arc<AtomicBool>,
    heard: Heard,
    drainer: JoinHandle<()>,
}

impl Mic {
    pub fn is_listening(&self) -> bool {
        self.listening.is_some()
    }

    // Opens mic `input` (from mic_inputs, or None for the system's pick) and
    // starts keeping what it hears. Gives what is heard so far, as it grows.
    pub fn start(&mut self, app: &AppHandle, input: Option<usize>) -> Result<Heard, String> {
        if self.listening.is_some() {
            return Err(String::from("the mic is already listening"));
        }
        let rate = {
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            engine.start_capture(input)?;
            engine.sample_rate()
        };

        let stop = Arc::new(AtomicBool::new(false));
        let heard: Heard = Arc::new(Mutex::new(Vec::new()));
        let handle = app.clone();
        let stopping = stop.clone();
        let keeping = heard.clone();
        let drainer = thread::spawn(move || drain(handle, stopping, keeping, rate));
        self.listening = Some(Listening {
            stop,
            heard: heard.clone(),
            drainer,
        });
        Ok(heard)
    }

    // Closes the mic and gives everything heard, at the engine's rate, or
    // None when nothing was listening.
    pub fn stop(&mut self, app: &AppHandle) -> Option<Vec<f32>> {
        let listening = self.listening.take()?;
        {
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            engine.stop_capture();
        }
        // The drainer takes the last of it out once the mic is closed.
        listening.stop.store(true, Ordering::SeqCst);
        let _ = listening.drainer.join();
        let heard = listening.heard.lock().unwrap();
        Some(heard.clone())
    }
}

// Takes what the mic heard out of the engine until told to stop, keeping at
// most MOST_SECONDS of it, and tells the screen the level each time.
fn drain(app: AppHandle, stop: Arc<AtomicBool>, heard: Heard, rate: u32) {
    let most = MOST_SECONDS as usize * rate as usize;
    loop {
        let stopping = stop.load(Ordering::SeqCst);
        let (taken, peak, dropped) = {
            let engine = app.state::<Mutex<Engine>>();
            let mut engine = engine.lock().unwrap();
            (engine.read_capture(), engine.capture_peak(), engine.capture_dropped())
        };
        {
            let mut kept = heard.lock().unwrap();
            let room = most.saturating_sub(kept.len());
            let keeping = taken.len().min(room);
            kept.extend_from_slice(&taken[..keeping]);
            let seconds = kept.len() as f32 / rate as f32;
            let level = json!({ "peak": peak, "seconds": seconds, "dropped": dropped });
            let _ = app.emit("mic-level", level);
        }
        if stopping {
            return;
        }
        thread::sleep(DRAIN_EVERY);
    }
}

// `samples` at `from` a second, at `to` a second instead. Each new sample
// is the average of the old ones it stands for, so nothing too high to
// hear at the new rate folds back down.
pub fn resampled(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return samples.to_vec();
    }
    let step = from as f64 / to as f64;
    let count = (samples.len() as f64 / step).floor() as usize;
    let mut out = Vec::with_capacity(count);
    for place in 0..count {
        let start = (place as f64 * step).floor() as usize;
        let mut end = ((place + 1) as f64 * step).floor() as usize;
        end = end.clamp(start + 1, samples.len());
        let mut total = 0.0;
        for sample in &samples[start..end] {
            total += sample;
        }
        out.push(total / (end - start) as f32);
    }
    out
}

// The mics there are, by name, numbered from 0 in this order.
#[tauri::command]
pub fn mic_inputs(engine: State<'_, Mutex<Engine>>, mic: State<'_, Mutex<Mic>>) -> Vec<String> {
    // Asking while listening would keep the old list anyway.
    let listening = mic.lock().unwrap().is_listening();
    let mut engine = engine.lock().unwrap();
    if listening {
        return Vec::new();
    }
    engine.list_inputs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_the_tone_and_the_length() {
        let mut samples = Vec::new();
        for place in 0..48_000 {
            let time = place as f32 / 48_000.0;
            samples.push((2.0 * std::f32::consts::PI * 220.0 * time).sin());
        }
        let out = resampled(&samples, 48_000, LISTENING_RATE);
        assert_eq!(out.len(), 16_000);
        let hz = crate::pitch::hz_of(&out[..1024], LISTENING_RATE).unwrap();
        assert!((hz - 220.0).abs() < 1.0, "{hz}");
        assert_eq!(resampled(&[0.5, 0.25], 16_000, 16_000), vec![0.5, 0.25]);
        assert_eq!(resampled(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 48_000, 16_000), vec![2.0, 5.0]);
    }
}
