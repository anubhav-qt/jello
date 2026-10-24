// jello_listen (docs/design.md#the-agent-harness): renders the song on a
// second engine with no speakers, measures the render (src/measures.rs),
// and can save it as a WAV in Music/Jello/Renders. What plays live is never
// touched: the second engine gets a copy of the song's sound, chords and
// loop, and of the keys' layout at the current octave.
//
// It renders one of three things:
//   - "loop": the loop once through, from its start.
//   - "chords": the song's chords, each for as long as it was held.
//   - "phrase": a phrase the agent is thinking of playing, written as for
//     jello_play.
// The chords and a phrase can play over the loop, with "with_loop": true.
// After that the loop pauses, everything lets go, and the render goes on
// until the sound has died away, to measure how long notes ring on.

use std::fs;
use std::time::Instant;

use serde_json::{Value, json};

use crate::compose::Composer;
use crate::engine::{BEATS_PER_BAR, CHANNELS, Engine, SONG_MAX_CHORDS};
use crate::key_map::{KeyAction, KeyMap, LAPTOP_VELOCITY};
use crate::measures::{SILENCE_LEVEL, json_of, measure, peak_of, silences};
use crate::phrases::{self, Phrase};
use crate::presets::name_problem;
use crate::song_file::Song;
use crate::song_session::{open_loop_of, song_now};
use crate::tools::{Studio, optional_flag_argument, optional_text_argument, text_argument};
use crate::wav::write_wav;

// The longest stretch rendered before everything lets go.
const RENDER_LONGEST_MS: u64 = 60_000;

// How many frames the second engine renders at a time, about what the
// speakers ask for.
const RENDER_CHUNK: usize = 512;

// After the end, the render goes on in steps this long until one is silent,
// for at most RING_LONGEST_MS.
const RING_STEP_MS: u64 = 50;
const RING_LONGEST_MS: u64 = 10_000;

// A finished render.
struct Render {
    samples: Vec<f32>,
    sample_rate: u32,
    // How many frames play before everything lets go. The rest is the
    // sound ringing on.
    played_frames: usize,
    // How long the chords or the loop last, if they had to be cut at
    // RENDER_LONGEST_MS.
    cut_from_ms: Option<u64>,
    // How long sound went on after the end, and whether it still had not
    // died away after RING_LONGEST_MS.
    rings_on_ms: u64,
    still_ringing: bool,
}

// jello_listen with `arguments`: renders, measures, and saves the WAV if
// asked. Answers with the measures.
pub fn listen(studio: &Studio, arguments: &Value) -> Result<Value, String> {
    let what = text_argument(arguments, "what")?;
    if what != "loop" && what != "chords" && what != "phrase" {
        return Err(format!("jello_listen renders the loop, the chords or a phrase, not {what}"));
    }
    let with_loop = optional_flag_argument(arguments, "with_loop")?;
    let mut wav_path = None;
    if let Some(name) = optional_text_argument(arguments, "save_as")? {
        wav_path = Some(wav_path_for(studio, &name)?);
    }

    let (song, key_map, phrase) = copies_of(studio, &what, arguments)?;
    let render = render(&song, key_map, &what, with_loop, phrase.as_ref())?;

    let mut answer = answer_of(&render, song.tempo);
    answer["rendered"] = json!(what);
    answer["with_loop"] = json!(with_loop);
    if let Some(path) = wav_path {
        write_wav(&path, &render.samples, render.sample_rate)?;
        answer["saved"] = json!(path.display().to_string());
    }
    Ok(answer)
}

// Where the render saved as `name` goes, after checking the name. Refuses
// a name a render already has, so no render is saved over.
fn wav_path_for(studio: &Studio, name: &str) -> Result<std::path::PathBuf, String> {
    if let Some(problem) = name_problem(name) {
        return Err(format!("save_as can't be \"{name}\": {problem}"));
    }
    let open = studio.open_song.lock().unwrap();
    let folder = open.folders()?.renders;
    drop(open);

    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = folder.join(format!("{}.wav", name.trim()));
    if path.exists() {
        return Err(format!("a render called \"{}\" is saved already: pick another name", name.trim()));
    }
    Ok(path)
}

// What the render needs from the studio, copied so nothing stays locked
// while it renders: the song as it is now, the keys' layout with no key
// held, and the phrase if one is asked for, read at the song's tempo.
fn copies_of(studio: &Studio, what: &str, arguments: &Value) -> Result<(Song, KeyMap, Option<Phrase>), String> {
    let key_map = studio.key_map.lock().unwrap();
    let composer = studio.composer.lock().unwrap();
    let mut engine = studio.engine.lock().unwrap();
    let song = song_now(&composer, &mut engine);

    let mut phrase = None;
    if what == "phrase" {
        let read = phrases::phrase_of(arguments, &key_map, composer.tempo())?;
        phrase = Some(read);
    }
    let mut keys = key_map.clone();
    keys.release_all();
    Ok((song, keys, phrase))
}

// Renders `what` from `song` on a new engine with no speakers.
fn render(song: &Song, mut keys: KeyMap, what: &str, with_loop: bool, phrase: Option<&Phrase>) -> Result<Render, String> {
    let plays_loop = what == "loop" || with_loop;
    let mut copy = song.clone();
    if plays_loop {
        if song.looped.is_none() {
            return Err(String::from("the song has no loop yet"));
        }
    } else {
        copy.looped = None;
    }
    if what == "chords" && song.chords.is_empty() {
        return Err(String::from("the song has no chords yet"));
    }

    // The loop opens first: it sets the tempo the song plays at.
    let now = Instant::now();
    let mut engine = Engine::new()?;
    let sample_rate = engine.sample_rate();
    open_loop_of(&mut engine, &copy)?;
    let mut composer = Composer::opened(&mut engine, &copy, now);
    if plays_loop {
        engine.play_loop();
    }

    let mut length = match what {
        "loop" => loop_frames(&copy, sample_rate),
        "chords" => chord_frames(&copy, sample_rate),
        _ => frames_of(phrase_length(phrase), sample_rate),
    };
    let longest = frames_of(RENDER_LONGEST_MS, sample_rate);
    let mut cut_from_ms = None;
    if length > longest {
        cut_from_ms = Some(length as u64 * 1000 / u64::from(sample_rate));
        length = longest;
    }

    let mut samples = Vec::new();
    if what == "chords" {
        composer.play_song(&mut engine, now)?;
    }
    if let Some(phrase) = phrase {
        for event in &phrase.events {
            render_until(&mut engine, &mut samples, frames_of(event.at_ms, sample_rate));
            if event.down {
                press(&mut keys, &mut engine, &event.key);
            } else {
                release(&mut keys, &mut engine, &event.key);
            }
        }
    }
    render_until(&mut engine, &mut samples, length);

    if plays_loop {
        engine.pause_loop();
    }
    engine.stop_song();
    let (rings_on_ms, still_ringing) = ring_out(&mut engine, &mut samples, sample_rate);

    Ok(Render {
        samples,
        sample_rate,
        played_frames: length,
        cut_from_ms,
        rings_on_ms,
        still_ringing,
    })
}

// How many frames `ms` milliseconds last.
fn frames_of(ms: u64, sample_rate: u32) -> usize {
    let frames = ms * u64::from(sample_rate) / 1000;
    frames as usize
}

// How many frames the loop lasts, worked out as the engine does.
fn loop_frames(song: &Song, sample_rate: u32) -> usize {
    let Some(looped) = &song.looped else {
        return 0;
    };
    let samples_per_minute = u64::from(sample_rate) * 60;
    let beats = u64::from(looped.bars) * u64::from(BEATS_PER_BAR);
    let frames = beats * samples_per_minute / u64::from(song.tempo);
    frames as usize
}

// How many frames the song's chords last, each as long as it was held, as
// the engine plays them.
fn chord_frames(song: &Song, sample_rate: u32) -> usize {
    let mut frames = 0;
    for held in song.chords.iter().take(SONG_MAX_CHORDS) {
        frames += frames_of(held.held_ms, sample_rate);
    }
    frames
}

fn phrase_length(phrase: Option<&Phrase>) -> u64 {
    match phrase {
        Some(phrase) => phrase.length_ms,
        None => 0,
    }
}

// Renders onto `samples` until they hold `frames` frames.
fn render_until(engine: &mut Engine, samples: &mut Vec<f32>, frames: usize) {
    loop {
        let rendered = samples.len() / CHANNELS;
        if rendered >= frames {
            return;
        }
        let chunk = (frames - rendered).min(RENDER_CHUNK);
        samples.extend(engine.render(chunk));
    }
}

// Renders on in steps of RING_STEP_MS until one is silent. Returns how long
// the sound rang on, and whether it still rang after RING_LONGEST_MS.
fn ring_out(engine: &mut Engine, samples: &mut Vec<f32>, sample_rate: u32) -> (u64, bool) {
    let step = frames_of(RING_STEP_MS, sample_rate);
    let mut rings_on_ms = 0;
    loop {
        let chunk = engine.render(step);
        let silent = peak_of(&chunk) < SILENCE_LEVEL;
        samples.extend(chunk);
        if silent {
            return (rings_on_ms, false);
        }
        rings_on_ms += RING_STEP_MS;
        if rings_on_ms >= RING_LONGEST_MS {
            return (rings_on_ms, true);
        }
    }
}

// A phrase's key going down on the render's copy of the keys. Notes and
// chords play, and the vibrato and glide keys hold. The wave, looper and
// tempo keys do nothing in a render.
fn press(keys: &mut KeyMap, engine: &mut Engine, key: &str) {
    let Some(action) = keys.press(key) else {
        return;
    };
    match action {
        KeyAction::PlayNote(note) => {
            engine.note_on(note, LAPTOP_VELOCITY);
        }
        KeyAction::PlayChord(chord) => {
            for note in chord.notes() {
                engine.note_on(note, LAPTOP_VELOCITY);
            }
        }
        KeyAction::HoldVibratoKey => {
            engine.set_vibrato_key(true);
        }
        KeyAction::HoldGlideKey => {
            engine.set_glide_key(true);
        }
        _ => {}
    }
}

// A phrase's key coming up on the render's copy of the keys, letting go of
// what it holds.
fn release(keys: &mut KeyMap, engine: &mut Engine, key: &str) {
    let lets_go_of_vibrato_key = keys.holds_vibrato_key(key);
    let lets_go_of_glide_key = keys.holds_glide_key(key);
    let notes = keys.release(key);
    if lets_go_of_vibrato_key {
        engine.set_vibrato_key(false);
    }
    if lets_go_of_glide_key {
        engine.set_glide_key(false);
    }
    for note in notes {
        engine.note_off(note);
    }
}

// The measures of `render`, for the whole of it and beat by beat at
// `tempo`, with its silences and how long it rang on.
fn answer_of(render: &Render, tempo: u32) -> Value {
    let played = &render.samples[..render.played_frames * CHANNELS];
    let beat_frames = f64::from(render.sample_rate) * 60.0 / f64::from(tempo);

    let mut beats = Vec::new();
    let mut beat = 0;
    loop {
        let start = (beat as f64 * beat_frames).round() as usize;
        if start >= render.played_frames {
            break;
        }
        let mut end = ((beat + 1) as f64 * beat_frames).round() as usize;
        if end > render.played_frames {
            end = render.played_frames;
        }
        let mut measured = json_of(&measure(&played[start * CHANNELS..end * CHANNELS], render.sample_rate));
        measured["beat"] = json!(beat + 1);
        beats.push(measured);
        beat += 1;
    }

    let mut silent = Vec::new();
    for (from, to) in silences(played, render.sample_rate) {
        silent.push(json!({
            "from_beat": beat_at(from, beat_frames),
            "to_beat": beat_at(to, beat_frames),
        }));
    }

    let seconds = render.played_frames as f64 / f64::from(render.sample_rate);
    let mut answer = json!({
        "seconds": (seconds * 10.0).round() / 10.0,
        "tempo": tempo,
        "whole": json_of(&measure(played, render.sample_rate)),
        "beats": beats,
        "silences": silent,
        "rings_on_ms": render.rings_on_ms,
        "still_ringing": render.still_ringing,
    });
    if let Some(full_ms) = render.cut_from_ms {
        let full = full_ms as f64 / 1000.0;
        answer["cut"] = json!(format!("it lasts {full:.1} s, so only the first 60 s were rendered"));
    }
    answer
}

// Which beat frame `frame` falls on, counted from 1 at the start, to a
// hundredth.
fn beat_at(frame: usize, beat_frames: f64) -> f64 {
    let beat = 1.0 + frame as f64 / beat_frames;
    (beat * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::harmony::chord_of_name;
    use crate::song_file::{BeatNote, HeldChord, SongLayer, SongLoop};
    use crate::sound_profile::SoundProfile;

    // A song at 120 beats a minute, so a beat is half a second, with two
    // chords and a one-bar loop playing A4 on beats 1 and 3.
    fn song() -> Song {
        let mut chords = Vec::new();
        for (name, held_ms) in [("Am", 1000), ("C", 500)] {
            chords.push(HeldChord {
                chord: chord_of_name(name).unwrap(),
                held_ms,
            });
        }
        let notes = vec![
            BeatNote {
                note: 69,
                start: 0.0,
                end: 0.5,
            },
            BeatNote {
                note: 69,
                start: 2.0,
                end: 2.5,
            },
        ];
        Song {
            key: None,
            tempo: 120,
            snap: true,
            preset: None,
            chords,
            looped: Some(SongLoop {
                bars: 1,
                layers: vec![SongLayer {
                    bars: 0,
                    volume: 100,
                    playing: true,
                    notes,
                }],
            }),
            sound: SoundProfile::new().sound(),
        }
    }

    fn answer(what: &str, with_loop: bool, phrase: Option<&Phrase>) -> Value {
        let render = render(&song(), KeyMap::new(), what, with_loop, phrase).unwrap();
        let mut answer = answer_of(&render, 120);
        answer["frames"] = json!(render.samples.len() / CHANNELS);
        answer
    }

    fn loudness(beat: &Value) -> Option<f64> {
        beat["loudness_db"].as_f64()
    }

    #[test]
    fn the_loop_renders_once_through_and_is_silent_where_it_plays_nothing() {
        let answer = answer("loop", false, None);
        assert_eq!(answer["seconds"], json!(2.0));
        let beats = answer["beats"].as_array().unwrap();
        assert_eq!(beats.len(), 4);
        assert!(loudness(&beats[0]).unwrap() > -30.0);
        assert!(loudness(&beats[2]).unwrap() > -30.0);
        assert!(loudness(&beats[0]).unwrap() > loudness(&beats[1]).unwrap() + 10.0);

        let silences = answer["silences"].as_array().unwrap();
        assert!(!silences.is_empty());
        let last = silences.last().unwrap();
        assert!(last["from_beat"].as_f64().unwrap() > 3.5);
        assert_eq!(last["to_beat"], json!(5.0));
    }

    #[test]
    fn the_chords_render_for_as_long_as_they_were_held_then_ring_out() {
        let answer = answer("chords", false, None);
        assert_eq!(answer["seconds"], json!(1.5));
        assert_eq!(answer["beats"].as_array().unwrap().len(), 3);
        assert!(answer["silences"].as_array().unwrap().is_empty());

        // A new song's notes fade out over 300 ms once let go.
        let rings_on = answer["rings_on_ms"].as_u64().unwrap();
        assert!(rings_on >= 250 && rings_on <= 400, "rang on {rings_on} ms");
        assert_eq!(answer["still_ringing"], json!(false));
        let frames = answer["frames"].as_u64().unwrap();
        assert!(frames > 72_000);
    }

    #[test]
    fn a_phrase_renders_on_the_copy_of_the_keys() {
        let keys = KeyMap::new();
        let arguments = json!({"presses": [{"note": "C4", "start": 1, "hold": 1}]});
        let phrase = phrases::phrase_of(&arguments, &keys, 120).unwrap();
        let answer = answer("phrase", false, Some(&phrase));

        assert_eq!(answer["seconds"], json!(1.0));
        let beats = answer["beats"].as_array().unwrap();
        assert_eq!(loudness(&beats[0]), None);
        assert!(loudness(&beats[1]).unwrap() > -30.0);
        let rings_on = answer["rings_on_ms"].as_u64().unwrap();
        assert!(rings_on >= 250 && rings_on <= 400, "rang on {rings_on} ms");
    }

    #[test]
    fn a_phrase_can_play_over_the_loop() {
        let keys = KeyMap::new();
        let arguments = json!({"presses": [{"note": "C4", "start": 1, "hold": 1}]});
        let phrase = phrases::phrase_of(&arguments, &keys, 120).unwrap();
        let answer = answer("phrase", true, Some(&phrase));

        let beats = answer["beats"].as_array().unwrap();
        assert!(loudness(&beats[0]).unwrap() > -30.0);
    }

    #[test]
    fn nothing_to_render_is_refused() {
        let mut empty = song();
        empty.looped = None;
        empty.chords.clear();
        let loop_refused = render(&empty, KeyMap::new(), "loop", false, None);
        assert_eq!(loop_refused.err(), Some(String::from("the song has no loop yet")));
        let chords_refused = render(&empty, KeyMap::new(), "chords", false, None);
        assert_eq!(chords_refused.err(), Some(String::from("the song has no chords yet")));
    }

    #[test]
    fn chords_longer_than_a_minute_are_cut() {
        let mut long = song();
        long.chords[0].held_ms = 61_000;
        long.looped = None;
        let render = render(&long, KeyMap::new(), "chords", false, None).unwrap();
        let answer = answer_of(&render, 120);
        assert_eq!(answer["seconds"], json!(60.0));
        assert_eq!(answer["cut"], json!("it lasts 61.5 s, so only the first 60 s were rendered"));
    }
}

// Export precisely one common cycle, ready to repeat without an added tail.
pub(crate) fn export_loop(song: &Song, path: &std::path::Path) -> Result<(), String> {
    let rendered = render(song, KeyMap::new(), "loop", false, None)?;
    if rendered.cut_from_ms.is_some() {
        return Err(String::from("export loops of at most 60 seconds"));
    }
    write_wav(path, &rendered.samples[..rendered.played_frames * CHANNELS], rendered.sample_rate)
}
