// Guided composing (docs/design.md, "Chord guidance" and "Compose screen"):
// the song's key, the chords added to the song so far with how long each was
// held, how well each chord and melody key fits the key, playing the song
// back, the song's tempo and its sound profile.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::arrange::{self, Player};
use crate::sections::Sections;
use crate::engine::{Engine, LooperState, FASTEST_TEMPO, SLOWEST_TEMPO, SONG_MAX_CHORDS, START_TEMPO, SongChord, SongPlace};
use crate::harmony::{Chord, Fit, Guide, Key, Mode, Quality, BestNext, all_keys, chord_name, chord_of_notes, guide_after, key_name, note_fit};
use crate::key_map::{KeyMap, LAPTOP_VELOCITY};
use crate::key_mapping::KeyMapping;
use crate::scales::Scale;
use crate::song_file::{HeldChord, Song, SongLoop, SongSection};
use crate::sound_file::Sound;
use crate::drums::{DrumPad, Drums};
use crate::sound_profile::SoundProfile;
use crate::mix::{Mix, Place, check_track_routing, send_track_routing};
use crate::path::SongPath;
use crate::tracks::{Track, Tracks};

// How much slowing down or speeding up changes the song's tempo.
const TEMPO_STEP: u32 = 5;

pub struct Composer {
    // The song's key, picked before starting. None until then. It is the
    // key `scale` reads as.
    key: Option<Key>,
    // The scale the keys follow, and the song's main mapping
    // (docs/adr/0047-song-key-mappings.md). Keys play nothing until a scale
    // is picked.
    scale: Option<Scale>,
    keys: KeyMapping,
    // The section whose mapping the keys follow while the arrangement plays
    // it, from 0.
    section_playing: Option<usize>,
    // The chord played last, as it was played. None until a chord is.
    tried: Option<PlayedChord>,
    // The chords added so far, in order, each as it was played. Trying a
    // chord doesn't add it: only asking to add it does.
    progression: Vec<PlayedChord>,
    history: Vec<PlayedChord>,
    selected_history: Option<usize>,
    last_note: Option<u8>,
    performance: Vec<PerformanceEvent>,
    session_started: Instant,
    performance_id: u64,
    // Where in the song the chords played last start, so the playhead can
    // tell which chord is playing.
    playing_from: usize,
    // Where they stop: the place just after the last of them, so the
    // keyboard knows whether another chord comes after the one playing.
    playing_until: usize,
    // The song's tempo, in beats per minute: the click and the looper play
    // at it. A new song starts at 120.
    tempo: u32,
    // The song's instruments, each with its own sound
    // (docs/adr/0030-a-sound-profile-per-song.md, src/tracks.rs), and the
    // one picked, which the keys play and the Sound panel edits.
    tracks: Tracks,
    // The song's sections and the order it plays them in
    // (docs/design.md#tracks-and-sections, src/sections.rs).
    sections: Sections,
    // The arrangement while it plays (src/arrange.rs), holding the loop it
    // put away. None otherwise.
    arranging: Option<Player>,
    // The loop a section's practice put away while it plays
    // (src/practice_run.rs), the way the arrangement does. None otherwise.
    practice_loop: Option<SongLoop>,
    // The song's mix (src/mix.rs): pan, sends, effects, buses and the
    // master. It may have fewer strips than there are tracks; mix_mut and
    // mix_now fit it.
    mix: Mix,
    // Whether Learn is on and which stages are done (src/path.rs).
    path: SongPath,
}

// A press as practice reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct HeardPress {
    pub id: u64,
    pub notes: Vec<u8>,
    pub pressed_ms: u64,
    pub held_ms: Option<u64>,
}

#[derive(Clone, Serialize)]
pub struct PerformanceEvent {
    id: u64,
    key: String,
    notes: Vec<u8>,
    pressed_ms: u64,
    held_ms: Option<u64>,
}

// A chord as it was played: by which key, when, and for how long. The player
// composes by feel, so the length stays exactly as held, never moved onto
// the beat (docs/adr/0022-compose-as-a-tape.md).
#[derive(Clone)]
struct PlayedChord {
    chord: Chord,
    // The notes it played, lowest first: the chord's plain three, or the
    // voicing its key plays.
    notes: Vec<u8>,
    // The laptop key that played it, like "KeyC", so letting go of that key
    // ends it.
    key: String,
    pressed_at: Instant,
    // How long it was held, or None while it still is.
    held_for: Option<Duration>,
}

impl PlayedChord {
    // How long it was held, or while it still is, how long so far at `now`.
    fn length(&self, now: Instant) -> Duration {
        match self.held_for {
            Some(held_for) => held_for,
            None => self.time_since_pressed(now),
        }
    }

    // Ends the chord at `now`. A chord already let go keeps its length.
    fn let_go(&mut self, now: Instant) {
        if self.held_for.is_none() {
            self.held_for = Some(self.time_since_pressed(now));
        }
    }

    // Each command reads the clock before it waits for its turn, so `now` can
    // be a moment before a press another command handled first. That counts
    // as no time at all.
    fn time_since_pressed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.pressed_at)
    }

    // Whether `other` comes from the very same press of the same key.
    fn is_same_press(&self, other: &PlayedChord) -> bool {
        self.key == other.key && self.pressed_at == other.pressed_at
    }

    // The chord as the engine plays it back at `now`: its notes, as hard as
    // the laptop keys play them, for as long as it was held, or so far while
    // it still is. The same milliseconds the tape shows.
    fn to_play(&self, now: Instant) -> SongChord {
        SongChord {
            notes: self.notes.clone(),
            velocity: LAPTOP_VELOCITY,
            length_ms: self.length(now).as_millis() as u64,
        }
    }
}

// A key the player can pick.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KeyChoice {
    tonic: u8,
    mode: Mode,
    // Like "Db major".
    name: String,
}

// The scale picked, for the UI to draw.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScaleInSong {
    tonic: u8,
    // Like "dorian".
    kind: &'static str,
    // Like "A dorian".
    name: String,
}

// A chord in the song, for the UI to draw.
#[derive(Serialize)]
pub struct ChordInSong {
    // Its name, spelled for the key.
    name: String,
    // Its root as a pitch, from 0 (C) to 11 (B), so the tape colors it like
    // the key that plays it.
    root: u8,
    // How long it was held, in milliseconds. While it still is, how long so
    // far.
    held_ms: u64,
    // Whether it is still held, so the UI keeps it growing.
    still_held: bool,
}

// Where the song is playing, for the UI to draw the playhead and light the
// chord keys the song plays.
#[derive(Debug, PartialEq, Serialize)]
pub struct Playhead {
    // The chord playing, by its place in the song, from 0.
    chord: usize,
    // How far into it, in milliseconds.
    ms_in: u64,
    // The chord keys that play the chord playing, like ["KeyC"].
    now_keys: Vec<String>,
    // The chord keys that play the chord after it, so the keyboard shows it
    // coming. Empty when the song stops after the chord playing.
    next_keys: Vec<String>,
}

// Everything Compose shows, for the UI to draw.
#[derive(Serialize)]
pub struct ComposeView {
    // Every key there is to pick.
    keys: Vec<KeyChoice>,
    // The key picked, or None before one is.
    key: Option<KeyChoice>,
    // The scale picked.
    scale: Option<ScaleInSong>,
    // The chords added so far.
    progression: Vec<ChordInSong>,
    // The chord being held that isn't in the song yet, or None. The tape
    // draws it as a dashed block growing at the end.
    holding: Option<ChordInSong>,
    // For each chord key, what guidance says about its chord. Empty until a
    // key is picked.
    guide: HashMap<String, Guide>,
    // For each melody key, whether its note is in the key or outside it.
    // Empty until a key is picked.
    melody: HashMap<String, Fit>,
    history: Vec<ChordInSong>,
    selected_history: Option<usize>,
    reference: Option<ChordInSong>,
    live_guide: HashMap<String, Guide>,
    compatible: HashMap<String, String>,
    melody_next: HashMap<String, BestNext>,
    last_note: Option<u8>,
    performance: Vec<PerformanceEvent>,
    // Where the song is playing, or None while it isn't.
    playing: Option<Playhead>,
}

impl Composer {
    // No key yet, no chords, a new song's tempo and a new song's sound
    // profile.
    pub fn new() -> Composer {
        Composer {
            key: None,
            scale: None,
            keys: KeyMapping::default(),
            section_playing: None,
            tried: None,
            progression: Vec::new(),
            history: Vec::new(),
            selected_history: None,
            last_note: None,
            performance: Vec::new(),
            session_started: Instant::now(),
            performance_id: 0,
            playing_from: 0,
            playing_until: 0,
            tempo: START_TEMPO,
            tracks: Tracks::new(),
            sections: Sections::default(),
            arranging: None,
            practice_loop: None,
            mix: Mix::new(1),
            path: SongPath::default(),
        }
    }

    // The song in `song`, opened at `now` (docs/adr/0038-song-files.md):
    // its key, its chords with their lengths, its tracks and their sounds,
    // sent to `engine` too, and its sections. The tempo is the one `engine`
    // plays, which opening the song's loop sets first (Engine::open_loop).
    // No preset is lit until remember_preset_if_same says so.
    pub fn opened(engine: &mut Engine, song: &Song, now: Instant) -> Composer {
        let mut composer = Composer::new();
        composer.key = song.key;
        composer.scale = song.scale;
        composer.keys = song.keys.clone();
        // A key picked with no scale yet is its major or minor scale.
        if composer.scale.is_none() {
            composer.scale = song.key.map(Scale::of_key);
        }
        if composer.keys.is_empty() {
            if let Some(scale) = composer.scale {
                composer.keys = composer.keys.with_default_for(scale);
            }
        }
        // No laptop key holds them, so none lets go of them.
        for held in &song.chords {
            let notes = match &held.notes {
                Some(notes) => notes.clone(),
                None => held.chord.notes().to_vec(),
            };
            let played = PlayedChord {
                chord: held.chord,
                notes,
                key: String::new(),
                pressed_at: now,
                held_for: Some(Duration::from_millis(held.held_ms)),
            };
            composer.progression.push(played);
        }
        composer.tempo = engine.tempo();
        composer.path = song.path.clone();

        let mut tracks = Vec::new();
        for (number, song_track) in song.tracks.iter().enumerate() {
            let sound = &song_track.sound;
            let drums = song_track.drums.as_ref();
            let preset = song_track.preset.as_deref();
            engine.make_room();
            let track = engine.on_track(number, |engine| {
                Track::of_song(engine, sound, drums, song_track.volume, preset)
            });
            tracks.push(track);
        }
        // A name is a new one only if it is not the one the sound gives.
        for (number, song_track) in song.tracks.iter().enumerate() {
            let Some(name) = &song_track.name else {
                continue;
            };
            if *name != tracks[number].followed_name(number + 1) {
                tracks[number].rename(Some(name));
            }
        }
        composer.tracks = Tracks::of_list(tracks, song.picked);
        composer.tracks.send_all_but_sounds(engine);
        composer.sections = Sections::new(song.sections.clone(), song.arrangement.clone());
        composer.mix = song.mix.clone();
        composer.mix.fit_tracks(song.tracks.len());
        composer.mix.send_all(engine);
        composer.send_track_routings(engine);
        composer
    }

    // The song's key, or None before one is picked.
    pub fn key(&self) -> Option<Key> {
        self.key
    }

    // The chords added so far that have been let go, with how long each was
    // held, for the song file. A chord still held waits until it is let go.
    pub fn held_chords(&self) -> Vec<HeldChord> {
        let mut chords = Vec::new();
        for played in &self.progression {
            if let Some(held_for) = played.held_for {
                let mut notes = None;
                if played.notes != played.chord.notes() {
                    notes = Some(played.notes.clone());
                }
                chords.push(HeldChord {
                    chord: played.chord,
                    held_ms: held_for.as_millis() as u64,
                    notes,
                });
            }
        }
        chords
    }

    // Lights preset `name`'s tile on track `track`, from 0, if it sounds
    // exactly like `sound`, the preset's file as it is now. Opening a song
    // that names its presets does this.
    pub fn remember_preset_if_same(&mut self, track: usize, name: &str, sound: &Sound) {
        if let Some(track) = self.tracks.get_mut(track) {
            track.remember_preset_if_same(name, sound);
        }
    }

    // Sends the song's tempo and every track's sound to `engine`, so
    // everything after this plays them. The app does this as it starts,
    // before the first note.
    pub fn send_all(&self, engine: &mut Engine) {
        engine.set_tempo(self.tempo);
        self.tracks.send_all(engine);
        self.mix_now().send_all(engine);
        self.send_track_routings(engine);
    }

    pub fn send_track_routings(&self, engine: &mut Engine) {
        for (number, track) in self.tracks.list().iter().enumerate() {
            let Some(links) = self.mix.links(Place::Track(number)) else {
                continue;
            };
            engine.make_room();
            send_track_routing(engine, number, track.sound_profile().connections(), links);
        }
    }

    pub fn check_picked_routing(&self, connections: &[crate::modulation::Connection; crate::modulation::CONNECTION_COUNT]) -> Result<(), String> {
        self.check_track_routing(self.tracks.picked(), connections)
    }

    pub fn check_track_routing(&self, number: usize, connections: &[crate::modulation::Connection; crate::modulation::CONNECTION_COUNT]) -> Result<(), String> {
        let links = self.mix.links(Place::Track(number)).unwrap_or(&[]);
        check_track_routing(connections, links)
    }

    pub fn send_picked_routing(&self, engine: &mut Engine) {
        let number = self.tracks.picked();
        let links = self.mix.links(Place::Track(number)).unwrap_or(&[]);
        send_track_routing(engine, number, self.sound_profile().connections(), links);
    }

    fn prepare_picked_routing(&mut self, engine: &mut Engine, connections: &[crate::modulation::Connection; crate::modulation::CONNECTION_COUNT]) -> Result<(), String> {
        self.check_picked_routing(connections)?;
        let current = self.sound_profile().connections();
        let mut changed = false;
        for (before, after) in current.iter().zip(connections) {
            let was_used = before.source != crate::modulation::Source::None;
            let will_be_used = after.source != crate::modulation::Source::None;
            if was_used != will_be_used {
                changed = true;
                break;
            }
        }
        if changed {
            crate::arrange::stop(self, engine);
        }
        Ok(())
    }

    // Applies a complete candidate sound after checking the shared routing
    // limit. Sound edits and temporary previews use the same path.
    pub fn apply_sound(&mut self, engine: &mut Engine, wanted: &Sound) -> Result<Sound, String> {
        self.prepare_picked_routing(engine, &wanted.connections)?;
        let before = self.sound_profile().sound();
        crate::sound_changes::take_changes(self.sound_profile_mut(), engine, &before, wanted);
        self.send_picked_routing(engine);
        Ok(self.sound_profile().sound())
    }

    // The song's mix as it is now, with a strip for every track.
    pub fn mix_now(&self) -> Mix {
        let mut mix = self.mix.clone();
        mix.fit_tracks(self.tracks.len());
        mix
    }

    // For changing the mix, with a strip for every track. Each change goes
    // on to the engine.
    pub fn mix_mut(&mut self) -> &mut Mix {
        self.mix.fit_tracks(self.tracks.len());
        &mut self.mix
    }

    pub fn tracks(&self) -> &Tracks {
        &self.tracks
    }

    // For picking, making and changing tracks. Each change goes on to the
    // engine.
    pub fn tracks_mut(&mut self) -> &mut Tracks {
        &mut self.tracks
    }

    pub fn sections(&self) -> &[SongSection] {
        self.sections.list()
    }

    // The order the song plays its sections in, as places among them.
    pub fn path(&self) -> &SongPath {
        &self.path
    }

    pub fn path_mut(&mut self) -> &mut SongPath {
        &mut self.path
    }

    pub fn arrangement(&self) -> &[usize] {
        self.sections.order()
    }

    pub fn sections_mut(&mut self) -> &mut Sections {
        &mut self.sections
    }

    // Keeps `sections` as the song's, played in the order `arrangement`
    // gives as places among them.
    #[cfg(test)]
    pub fn set_sections(&mut self, sections: Vec<SongSection>, arrangement: Vec<usize>) {
        self.sections = Sections::new(sections, arrangement);
    }

    // The arrangement playing, if it plays.
    pub fn arranging(&self) -> Option<&Player> {
        self.arranging.as_ref()
    }

    pub fn arranging_mut(&mut self) -> Option<&mut Player> {
        self.arranging.as_mut()
    }

    // Keeps `arranging` as the arrangement playing, and returns the one
    // there was.
    pub fn set_arranging(&mut self, arranging: Option<Player>) -> Option<Player> {
        std::mem::replace(&mut self.arranging, arranging)
    }

    // The loop a section's practice put away, if it did.
    pub fn practice_loop(&self) -> Option<&SongLoop> {
        self.practice_loop.as_ref()
    }

    // Keeps `looped` as the loop practice put away, and returns the one
    // there was.
    pub fn set_practice_loop(&mut self, looped: Option<SongLoop>) -> Option<SongLoop> {
        std::mem::replace(&mut self.practice_loop, looped)
    }

    // Takes track `removed`, from 0, out of the sections and the mix, on
    // `engine` too: its layers, values and strip go, and the later tracks'
    // move one track down.
    pub fn forget_track(&mut self, engine: &mut Engine, removed: usize) {
        self.sections.forget_track(removed);
        self.mix.remove_track(engine, removed);
        self.send_track_routings(engine);
    }

    // Stop the arrangement before changing the bus numbers in the song.
    pub fn remove_bus(&mut self, engine: &mut Engine, bus: usize) -> Result<(), String> {
        if bus >= self.mix.buses.len() {
            return Err(format!("the song has no {}", Place::Bus(bus).name()));
        }
        arrange::stop(self, engine);
        self.mix_mut().remove_bus(engine, bus)?;
        self.sections.forget_bus(bus);
        Ok(())
    }

    // The picked track's drums.
    pub fn drums(&self) -> &Drums {
        self.tracks.picked_track().drums()
    }

    // Makes the picked track's drums play instrument `name`, or nothing for
    // None, on `engine` too. A kit makes it a drums track.
    pub fn set_drum_kit(&mut self, engine: &mut Engine, name: Option<&str>) {
        self.tracks.set_drum_kit(engine, name);
    }

    // Sets how the picked track's pad `pad`, from 0, sounds, on `engine`
    // too, kept as the engine sent it. A pad past the last changes nothing.
    pub fn set_drum_pad(&mut self, engine: &mut Engine, pad: usize, settings: DrumPad) {
        self.tracks.set_drum_pad(engine, pad, settings);
    }

    // Plays the song at `tempo` beats per minute from the next beat on, on
    // `engine` too. The song keeps the tempo the engine plays, which is the
    // one it had for a tempo outside SLOWEST_TEMPO to FASTEST_TEMPO, or while
    // the looper is busy.
    pub fn set_tempo(&mut self, engine: &mut Engine, tempo: u32) {
        self.tempo = engine.set_tempo(tempo);
    }

    // Takes the tempo `engine` plays as the song's: opening the loop again
    // at another tempo sets it there.
    pub fn follow_tempo(&mut self, engine: &Engine) {
        self.tempo = engine.tempo();
    }

    // Slows the song down by 5 beats per minute, but never below
    // SLOWEST_TEMPO.
    pub fn slow_down(&mut self, engine: &mut Engine) {
        let slower = self.tempo - TEMPO_STEP;
        if slower >= SLOWEST_TEMPO {
            self.set_tempo(engine, slower);
        }
    }

    // Speeds the song up by 5 beats per minute, but never above
    // FASTEST_TEMPO.
    pub fn speed_up(&mut self, engine: &mut Engine) {
        let faster = self.tempo + TEMPO_STEP;
        if faster <= FASTEST_TEMPO {
            self.set_tempo(engine, faster);
        }
    }

    // The song's tempo, in beats per minute.
    pub fn tempo(&self) -> u32 {
        self.tempo
    }

    // The picked track's sound profile.
    pub fn sound_profile(&self) -> &SoundProfile {
        self.tracks.picked_track().sound_profile()
    }

    // For changing the picked track's sound. Each change goes on to the
    // engine, whose picked track is the same one.
    pub fn sound_profile_mut(&mut self) -> &mut SoundProfile {
        self.tracks.picked_track_mut().sound_profile_mut()
    }

    // Copies preset `name`'s `sound` into the picked track, on `engine`
    // too, and remembers it as the preset the track sounds like.
    pub fn take_preset(&mut self, engine: &mut Engine, name: &str, sound: &Sound) -> Result<(), String> {
        self.prepare_picked_routing(engine, &sound.connections)?;
        self.tracks.picked_track_mut().take_preset(engine, name, sound);
        self.send_picked_routing(engine);
        Ok(())
    }

    // Remembers that the picked track's sound was just saved as preset
    // `name`.
    pub fn saved_as_preset(&mut self, name: &str) {
        self.tracks.picked_track_mut().saved_as_preset(name);
    }

    // The preset the picked track sounds exactly like, or None.
    pub fn lit_preset(&self) -> Option<&str> {
        self.tracks.picked_track().lit_preset()
    }

    // Sets the song's key: its major or minor scale.
    pub fn pick_key(&mut self, key: Key) {
        self.pick_scale(Scale::of_key(key));
    }

    // Sets the song's scale and the key it reads as. Chords added already
    // stay, named for the new key. The keys go to the scale's default, but
    // locked keys stay, and sections go back to the main mapping, since their
    // own were made for the old scale.
    pub fn pick_scale(&mut self, scale: Scale) {
        self.key = Some(scale.key());
        self.scale = Some(scale);
        self.keys = self.keys.with_default_for(scale);
        self.sections.forget_keys();
    }

    pub fn scale(&self) -> Option<Scale> {
        self.scale
    }

    // The song's main mapping.
    pub fn main_keys(&self) -> &KeyMapping {
        &self.keys
    }

    pub fn set_main_keys(&mut self, keys: KeyMapping) {
        self.keys = keys;
    }

    // The mapping the song's keys play now: the section playing's own, or
    // the main one.
    pub fn keys_now(&self) -> &KeyMapping {
        let Some(number) = self.section_playing else {
            return &self.keys;
        };
        let Some(section) = self.sections.list().get(number) else {
            return &self.keys;
        };
        match &section.keys {
            Some(keys) => keys,
            None => &self.keys,
        }
    }

    // The arrangement moved on to section `number`, from 0, or stopped.
    pub fn set_section_playing(&mut self, number: Option<usize>) {
        self.section_playing = number;
    }

    // Whether the song, its arrangement or its loop plays right now, when an
    // optimized mapping must not change anything.
    pub fn is_playing(&self, engine: &Engine) -> bool {
        if self.arranging.is_some() {
            return true;
        }
        if engine.song_place().is_some() {
            return true;
        }
        !matches!(engine.looper_status().state, LooperState::Idle | LooperState::Paused)
    }

    // `key` went down at `now` and plays `chord`, so it is the chord tried
    // last.
    #[cfg(test)]
    pub fn chord_pressed(&mut self, key: &str, chord: Chord, now: Instant) {
        self.voicing_pressed(key, chord, chord.notes().to_vec(), now);
    }

    // `key` went down at `now` and plays `notes`. If they hold a major or
    // minor chord it is the chord tried last, in this voicing. Notes that
    // hold none are only a press: guidance knows no chord to follow.
    pub fn notes_pressed(&mut self, key: &str, notes: &[u8], now: Instant) {
        match chord_of_notes(notes) {
            Some(chord) => self.voicing_pressed(key, chord, notes.to_vec(), now),
            None => self.record_press(key, notes.to_vec(), now),
        }
    }

    fn voicing_pressed(&mut self, key: &str, chord: Chord, notes: Vec<u8>, now: Instant) {
        self.record_press(key, notes.clone(), now);
        self.selected_history = None;
        let played = PlayedChord {
            chord,
            notes,
            key: String::from(key),
            pressed_at: now,
            held_for: None,
        };
        self.history.push(played.clone());
        self.tried = Some(played);
    }

    // `key` was let go at `now`, so the chords it is still holding end there:
    // the one tried last, and the ones added to the song while it was held.
    // A key can't be pressed again while it is held, so they all come from
    // the same press.
    pub fn key_released(&mut self, key: &str, now: Instant) {
        self.finish_events(Some(key), now);
        if let Some(tried) = &mut self.tried {
            if tried.key == key {
                tried.let_go(now);
            }
        }
        for played in self.history.iter_mut().chain(self.progression.iter_mut()) {
            if played.key == key {
                played.let_go(now);
            }
        }
    }

    // Every key was let go at once at `now`, so every chord still held ends.
    pub fn all_released(&mut self, now: Instant) {
        self.finish_events(None, now);
        if let Some(tried) = &mut self.tried {
            tried.let_go(now);
        }
        for played in self.history.iter_mut().chain(self.progression.iter_mut()) {
            played.let_go(now);
        }
    }

    // Adds the chord played last to the end of the song, so the chords tried
    // before it are left out. It keeps how long it was held. Added while
    // still held, it keeps growing until it is let go. Does nothing if no
    // chord has been played yet.
    pub fn add_last_chord(&mut self) {
        if let Some(tried) = &self.tried {
            self.progression.push(tried.clone());
        }
    }

    // Adds `chord`, held for `held_ms`, to the end of the song, as if it
    // was played at `now`. The agent's chord options add theirs this way.
    pub fn add_chord(&mut self, chord: Chord, held_ms: u64, now: Instant) {
        let played = PlayedChord {
            chord,
            notes: chord.notes().to_vec(),
            key: String::new(),
            pressed_at: now,
            held_for: Some(Duration::from_millis(held_ms)),
        };
        self.progression.push(played);
    }

    // The player's presses from press `first_id` on, oldest first, with
    // their notes, when they went down in ms since Compose started, and how
    // long they were held, or None while they still are. Practice reads
    // them as they come (src/practice.rs).
    pub fn presses_from(&self, first_id: u64) -> Vec<HeardPress> {
        let mut presses = Vec::new();
        for event in &self.performance {
            if event.id < first_id {
                continue;
            }
            presses.push(HeardPress {
                id: event.id,
                notes: event.notes.clone(),
                pressed_ms: event.pressed_ms,
                held_ms: event.held_ms,
            });
        }
        presses
    }

    // The presses from press `first_id` on that have been let go, for the
    // jam card (src/jam.rs): their keys, notes, and times from the first.
    pub fn jam_presses_from(&self, first_id: u64) -> Vec<crate::jam::JamPress> {
        let mut presses = Vec::new();
        for event in &self.performance {
            if event.id < first_id {
                continue;
            }
            let Some(held_ms) = event.held_ms else {
                continue;
            };
            presses.push(crate::jam::JamPress {
                key: event.key.clone(),
                notes: event.notes.clone(),
                start_ms: event.pressed_ms,
                hold_ms: held_ms,
            });
        }
        presses
    }

    // The number the next press will get.
    pub fn next_press_id(&self) -> u64 {
        self.performance_id + 1
    }

    // `at` in ms since Compose started, as presses count their times.
    pub fn session_ms(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.session_started).as_millis() as u64
    }

    pub fn note_pressed(&mut self, key: &str, note: u8, now: Instant) {
        self.last_note = Some(note);
        self.record_press(key, vec![note], now);
    }

    fn record_press(&mut self, key: &str, notes: Vec<u8>, now: Instant) {
        if self.performance.len() == 256 { self.performance.remove(0); }
        self.performance_id += 1;
        self.performance.push(PerformanceEvent {
            id: self.performance_id,
            key: String::from(key),
            notes,
            pressed_ms: now.saturating_duration_since(self.session_started).as_millis() as u64,
            held_ms: None,
        });
    }

    fn finish_events(&mut self, key: Option<&str>, now: Instant) {
        let elapsed = now.saturating_duration_since(self.session_started).as_millis() as u64;
        for event in &mut self.performance {
            if event.held_ms.is_none() && key.is_none_or(|code| code == event.key) {
                event.held_ms = Some(elapsed.saturating_sub(event.pressed_ms));
            }
        }
    }

    // Selection changes the reference, never the song. Adding is explicit.
    pub fn edit(&mut self, action: &str, place: usize) -> Result<(), String> {
        match action {
            "follow_live" => self.selected_history = None,
            "select_history" | "add_history" => {
                let chord = self.history.get(place).ok_or("No such tried chord")?;
                if action == "add_history" {
                    self.progression.push(chord.clone());
                } else {
                    self.selected_history = Some(place);
                }
            }
            "remove" | "left" | "right" => {
                if place >= self.progression.len() {
                    return Err(String::from("No such song chord"));
                }
                match action {
                    "remove" => { self.progression.remove(place); }
                    "left" if place > 0 => self.progression.swap(place, place - 1),
                    "right" if place + 1 < self.progression.len() => self.progression.swap(place, place + 1),
                    _ => {}
                }
            }
            _ => return Err(String::from("Unknown composition action")),
        }
        Ok(())
    }

    fn reference(&self) -> Option<&PlayedChord> {
        if let Some(place) = self.selected_history {
            return self.history.get(place);
        }
        self.tried.as_ref().or(self.progression.last())
    }

    // Takes the last chord off the end. Does nothing if there are none.
    pub fn remove_last_chord(&mut self) {
        self.progression.pop();
    }

    // Plays the chord at `place` in the song on `engine`, for as long as it
    // was held, or so far if it still is. Whatever the song was playing
    // stops first. Clicking a chord on the tape does this.
    pub fn play_chord(&mut self, place: usize, engine: &mut Engine, now: Instant) -> Result<(), String> {
        let played = match self.progression.get(place) {
            Some(played) => played,
            None => return Err(format!("the song has no chord {place}")),
        };
        engine.play_song(&[played.to_play(now)])?;
        self.playing_from = place;
        self.playing_until = place + 1;
        Ok(())
    }

    // Plays the whole song on `engine` from its first chord, each chord for
    // as long as it was held. Whatever the song was playing stops first. A
    // song longer than the engine plays stops after its first
    // SONG_MAX_CHORDS chords. The tape's play button does this.
    pub fn play_song(&mut self, engine: &mut Engine, now: Instant) -> Result<(), String> {
        // The progression and the sections never play over each other.
        crate::arrange::stop(self, engine);
        let mut chords = Vec::new();
        for played in self.progression.iter().take(SONG_MAX_CHORDS) {
            chords.push(played.to_play(now));
        }
        engine.play_song(&chords)?;
        self.playing_from = 0;
        self.playing_until = chords.len();
        Ok(())
    }

    // What Compose shows at `now`. `song_place` is where the engine says the
    // song is playing.
    pub fn view(&self, key_map: &KeyMap, song_place: Option<SongPlace>, now: Instant) -> ComposeView {
        let mut keys = Vec::new();
        for key in all_keys() {
            keys.push(key_choice(key));
        }

        let picked = self.key.map(key_choice);
        let mut scale = None;
        if let Some(picked_scale) = self.scale {
            scale = Some(ScaleInSong {
                tonic: picked_scale.tonic,
                kind: picked_scale.kind.word(),
                name: picked_scale.name(),
            });
        }

        let mut progression = Vec::new();
        for played in &self.progression {
            progression.push(self.chord_in_song(played, now));
        }

        // Guidance ranks what comes next after the song's last chord, never
        // after the chord tried last (docs/adr/0034-best-next-chords.md).
        let last = self.last_chord();
        let mut chord_guide = HashMap::new();
        let mut melody = HashMap::new();
        if let Some(key) = self.key {
            for (code, chord) in key_map.chord_keys() {
                chord_guide.insert(code, guide_after(key, last, chord));
            }
            for (code, note) in key_map.note_keys() {
                melody.insert(code, note_fit(key, note));
            }
        }

        let reference = self.reference();
        let mut live_guide = HashMap::new();
        let mut compatible = HashMap::new();
        let mut melody_next = HashMap::new();
        // A played chord can guide exploration even before choosing a song key.
        let context_key = self.key.or_else(|| reference.map(|played| Key {
            tonic: played.chord.pitch(),
            mode: match played.chord.quality {
                Quality::Major => Mode::Major,
                Quality::Minor => Mode::Minor,
            },
        }));
        if let Some(key) = context_key {
            if let Some(reference) = reference {
                let chord = reference.chord;
                for (code, candidate) in key_map.chord_keys() {
                    let guide = guide_after(key, Some(chord), candidate);
                    let common = candidate.notes().iter().filter(|note| {
                        chord.notes().iter().any(|held| held % 12 == *note % 12)
                    }).count();
                    if common > 0 && guide.fit != Fit::Outside && candidate != chord {
                        compatible.insert(code.clone(), format!("Shares {common} chord tones with {}", chord_name(chord, self.key)));
                    }
                    live_guide.insert(code, guide);
                }
            }
            if let Some(previous) = self.last_note {
                let mut candidates = Vec::new();
                for (code, note) in key_map.note_keys() {
                    let distance = note.abs_diff(previous);
                    if distance == 0 || distance > 7 || note_fit(key, note) == Fit::Outside {
                        continue;
                    }
                    let chord_tone = reference.is_some_and(|played| {
                        played.chord.notes().iter().any(|held| held % 12 == note % 12)
                    });
                    // Small scale steps come first; nearby chord tones also make steady targets.
                    let mut score = u16::from(distance) * 2;
                    if chord_tone { score = score.saturating_sub(3); }
                    candidates.push((score, distance, note, code, chord_tone));
                }
                candidates.sort();
                for (rank, (_, distance, _, code, chord_tone)) in candidates.into_iter().take(3).enumerate() {
                    let mut why = format!("{distance} half steps from the last note, in the key");
                    if chord_tone { why.push_str(" and in the reference chord"); }
                    melody_next.insert(code, BestNext { rank: (rank + 1) as u8, why });
                }
            }
        }

        ComposeView {
            history: self.history.iter().map(|chord| self.chord_in_song(chord, now)).collect(),
            selected_history: self.selected_history,
            reference: reference.map(|chord| self.chord_in_song(chord, now)),
            live_guide,
            compatible,
            melody_next,
            last_note: self.last_note,
            performance: self.performance.clone(),
            keys,
            key: picked,
            scale,
            progression,
            holding: self.holding(now),
            guide: chord_guide,
            melody,
            playing: self.playhead(key_map, song_place),
        }
    }

    // The song's last chord, or None while it has none.
    fn last_chord(&self) -> Option<Chord> {
        match self.progression.last() {
            Some(played) => Some(played.chord),
            None => None,
        }
    }

    // `played` at `now`, for the UI to draw.
    fn chord_in_song(&self, played: &PlayedChord, now: Instant) -> ChordInSong {
        let length = played.length(now);
        ChordInSong {
            name: chord_name(played.chord, self.key),
            root: played.chord.pitch(),
            held_ms: length.as_millis() as u64,
            still_held: played.held_for.is_none(),
        }
    }

    // The chord tried last, at `now`, while it is still held and not in the
    // song yet. Once it is added, the song shows it instead.
    fn holding(&self, now: Instant) -> Option<ChordInSong> {
        let tried = match &self.tried {
            Some(tried) => tried,
            None => return None,
        };
        if tried.held_for.is_some() {
            return None;
        }
        for played in &self.progression {
            if played.is_same_press(tried) {
                return None;
            }
        }
        Some(self.chord_in_song(tried, now))
    }

    // Where the playhead goes when the engine says the song is at
    // `song_place`, and which keys on `key_map` play the chord playing and
    // the one after it. None while the song isn't playing, or once the
    // chord playing has been taken off the song.
    fn playhead(&self, key_map: &KeyMap, song_place: Option<SongPlace>) -> Option<Playhead> {
        let song_place = match song_place {
            Some(song_place) => song_place,
            None => return None,
        };
        let chord = self.playing_from + song_place.chord;
        if chord >= self.progression.len() {
            return None;
        }
        let now_keys = keys_playing(key_map, &self.progression[chord]);
        let next_keys = match self.next_place(chord) {
            Some(next) => keys_playing(key_map, &self.progression[next]),
            None => Vec::new(),
        };
        Some(Playhead {
            chord,
            ms_in: song_place.ms_in,
            now_keys,
            next_keys,
        })
    }

    // The place of the chord the song plays after the one at `place`, or
    // None if it stops there: at the last chord sent to the engine, or once
    // the chord after has been taken off the song.
    fn next_place(&self, place: usize) -> Option<usize> {
        let next = place + 1;
        if next >= self.playing_until {
            return None;
        }
        if next >= self.progression.len() {
            return None;
        }
        Some(next)
    }
}

// The keys on `key_map` that play `played` as it was played, or if none does
// now, the first that plays its chord in another voicing. Empty if none does.
fn keys_playing(key_map: &KeyMap, played: &PlayedChord) -> Vec<String> {
    let exact = key_map.keys_playing(&played.notes);
    if !exact.is_empty() {
        return exact;
    }
    let mut any = key_map.keys_for_chord(played.chord);
    any.truncate(1);
    any
}

fn key_choice(key: Key) -> KeyChoice {
    KeyChoice {
        tonic: key.tonic,
        mode: key.mode,
        name: key_name(key),
    }
}

// These tests play keys on a key map, which only answers what keys do, at
// made-up times, so they never wait. They play the song back on an engine
// without speakers.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{all_keys_up, key_down, key_up};
    use crate::engine::Kind;
    use crate::key_map::KeyAction;

    // How many samples the engine makes in 10 ms, the size of chunk the
    // speakers ask for.
    const CHUNK: usize = 480;

    #[test]
    fn preset_routing_capacity_preserves_mix_and_the_previous_sound() {
        use crate::engine::{EffectConnection, EQ_OUTPUT};
        use crate::modulation::{CONNECTION_COUNT, Source};
        let mut engine = Engine::for_render().unwrap();
        let mut composer = Composer::new();
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        composer.mix_mut().set_send(&mut engine, 0, 0, 37).unwrap();
        let link = EffectConnection { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: 1.0, one_way: false, scaled_by: Source::None };
        composer.mix_mut().tracks[0].links.push(link);
        let original = composer.sound_profile().sound();
        let mix = composer.mix_now();
        let mut preset = original.clone();
        preset.connections = [original.connections[0]; CONNECTION_COUNT];
        assert!(composer.take_preset(&mut engine, "full", &preset).is_err());
        assert_eq!(composer.sound_profile().sound(), original);
        assert_eq!(composer.mix_now(), mix);

        preset.connections[CONNECTION_COUNT - 1].source = Source::None;
        preset.lfos[0].hz = 2.0;
        composer.take_preset(&mut engine, "fits", &preset).unwrap();
        assert_eq!(composer.sound_profile().lfos()[0].hz, 2.0);
        assert_eq!(composer.mix_now(), mix);
    }

    const C_MAJOR: Key = Key {
        tonic: 0,
        mode: Mode::Major,
    };

    #[test]
    fn a_song_with_only_a_key_keeps_its_strum_speed() {
        use crate::song_file::Song;
        let mut engine = Engine::for_render().unwrap();
        let mut song = Song::new(&SoundProfile::new());
        song.key = Some(C_MAJOR);
        song.keys.guitar.set_strum_ms(40).unwrap();
        let composer = Composer::opened(&mut engine, &song, Instant::now());
        assert!(!composer.main_keys().is_empty());
        assert_eq!(composer.main_keys().guitar.strum_ms, 40);
    }

    // A key map and Compose, played through the same steps the press and
    // release commands take, and an engine to play the song back on. Times
    // are in milliseconds from the start.
    struct Player {
        key_map: KeyMap,
        composer: Composer,
        engine: Engine,
        start: Instant,
    }

    impl Player {
        fn new() -> Player {
            Player {
                key_map: KeyMap::with_test_layout(),
                composer: Composer::new(),
                engine: Engine::new().unwrap(),
                start: Instant::now(),
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.start + Duration::from_millis(ms)
        }

        fn press(&mut self, key: &str, ms: u64) {
            let now = self.at(ms);
            key_down(&mut self.key_map, &mut self.composer, key, now);
        }

        fn release(&mut self, key: &str, ms: u64) {
            let now = self.at(ms);
            key_up(&mut self.key_map, &mut self.composer, key, now);
        }

        // Every key comes up at once at `ms`, as when the window loses
        // focus.
        fn release_all(&mut self, ms: u64) {
            let now = self.at(ms);
            all_keys_up(&mut self.key_map, &mut self.composer, now);
        }

        // Holds `key` down from `from` until `to`.
        fn tap(&mut self, key: &str, from: u64, to: u64) {
            self.press(key, from);
            self.release(key, to);
        }

        fn view(&self, ms: u64) -> ComposeView {
            self.composer.view(&self.key_map, self.engine.song_place(), self.at(ms))
        }

        // Clicks the chord at `place` on the tape at `ms`.
        fn click(&mut self, place: usize, ms: u64) -> Result<(), String> {
            let now = self.at(ms);
            self.composer.play_chord(place, &mut self.engine, now)
        }

        // Presses play at `ms`.
        fn play(&mut self, ms: u64) {
            let now = self.at(ms);
            self.composer.play_song(&mut self.engine, now).unwrap();
        }

        // Lets the engine play on for `ms` milliseconds, in 10 ms chunks,
        // and returns the sound.
        fn listen(&mut self, ms: usize) -> Vec<f32> {
            let mut sound = Vec::new();
            for _ in 0..ms / 10 {
                let chunk = self.engine.render(CHUNK);
                sound.extend(chunk);
            }
            sound
        }

        // Where the playhead is, as the chord playing and how far into it,
        // read the way the UI reads it. The clock plays no part, so it is
        // read at 0.
        fn playhead(&self) -> Option<(usize, u64)> {
            match self.view(0).playing {
                Some(playing) => Some((playing.chord, playing.ms_in)),
                None => None,
            }
        }

        // The chord keys the keyboard lights while the song plays: the ones
        // for the chord playing, then the ones for the chord after it.
        fn lit_keys(&self) -> Option<(Vec<String>, Vec<String>)> {
            match self.view(0).playing {
                Some(playing) => Some((playing.now_keys, playing.next_keys)),
                None => None,
            }
        }

        // The song at `ms`, each chord with how long it was held, like
        // "C for 2000 ms", or "C for 500 ms so far" while it still is.
        fn song(&self, ms: u64) -> Vec<String> {
            let mut song = Vec::new();
            for chord in self.view(ms).progression {
                song.push(chord_words(&chord));
            }
            song
        }

        // The chord held but not in the song at `ms`, said the same way.
        fn holding(&self, ms: u64) -> Option<String> {
            match self.view(ms).holding {
                Some(chord) => Some(chord_words(&chord)),
                None => None,
            }
        }
    }

    fn chord_words(chord: &ChordInSong) -> String {
        if chord.still_held {
            format!("{} for {} ms so far", chord.name, chord.held_ms)
        } else {
            format!("{} for {} ms", chord.name, chord.held_ms)
        }
    }

    #[test]
    fn a_preset_s_tile_is_lit_while_the_song_sounds_exactly_like_it() {
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        assert_eq!(composer.lit_preset(), None);

        let mut sound = SoundProfile::new().sound();
        sound.tuning_hz = 432.0;
        composer.take_preset(&mut engine, "Low", &sound).unwrap();
        assert_eq!(composer.lit_preset(), Some("Low"));

        // Changed, it no longer sounds like the preset...
        composer.sound_profile_mut().set_tuning(&mut engine, 435.0);
        assert_eq!(composer.lit_preset(), None);
        // ...until it is changed back.
        composer.sound_profile_mut().set_tuning(&mut engine, 432.0);
        assert_eq!(composer.lit_preset(), Some("Low"));

        // Saving lights the name it was saved as.
        composer.sound_profile_mut().set_tuning(&mut engine, 450.0);
        composer.saved_as_preset("High");
        assert_eq!(composer.lit_preset(), Some("High"));
    }

    #[test]
    fn a_new_song_beats_at_120() {
        let player = Player::new();
        assert_eq!(player.composer.tempo(), START_TEMPO);
    }

    #[test]
    fn slowing_down_and_speeding_up_go_by_5_and_stop_at_the_ends() {
        // The engine plays each new tempo too.
        let mut player = Player::new();
        player.composer.speed_up(&mut player.engine);
        assert_eq!(player.composer.tempo(), 125);
        assert_eq!(player.engine.tempo(), 125);
        player.composer.slow_down(&mut player.engine);
        player.composer.slow_down(&mut player.engine);
        assert_eq!(player.composer.tempo(), 115);
        assert_eq!(player.engine.tempo(), 115);

        // From 120 it takes 16 steps to reach 40, and then it stays there.
        let mut player = Player::new();
        for _ in 0..20 {
            player.composer.slow_down(&mut player.engine);
        }
        assert_eq!(player.composer.tempo(), SLOWEST_TEMPO);
        assert_eq!(player.engine.tempo(), SLOWEST_TEMPO);

        // And 24 steps to reach 240.
        for _ in 0..50 {
            player.composer.speed_up(&mut player.engine);
        }
        assert_eq!(player.composer.tempo(), FASTEST_TEMPO);
        assert_eq!(player.engine.tempo(), FASTEST_TEMPO);
    }

    #[test]
    fn the_song_keeps_the_tempo_the_engine_plays() {
        // While the looper is busy, the engine keeps its beat, and so does
        // the song. Once the loop is cleared, both move on together.
        let mut player = Player::new();
        player.engine.press_looper();
        player.composer.speed_up(&mut player.engine);
        assert_eq!(player.composer.tempo(), START_TEMPO);

        player.engine.clear_loop();
        player.engine.render(CHUNK);
        player.composer.speed_up(&mut player.engine);
        assert_eq!(player.composer.tempo(), 125);
        assert_eq!(player.engine.tempo(), 125);
    }

    #[test]
    fn sending_the_song_sends_its_tempo_and_its_sound_profile() {
        // A song at 125 on a square wave, sent to a new engine, plays at 125
        // there, on a square.
        let mut player = Player::new();
        player.composer.speed_up(&mut player.engine);
        player.composer.sound_profile_mut().switch_first_kind(&mut player.engine);
        let mut engine = Engine::new().unwrap();
        player.composer.send_all(&mut engine);
        assert_eq!(engine.tempo(), 125);

        let mut square = Engine::new().unwrap();
        square.set_first_kind(Kind::Square);
        let a4 = 69;
        engine.note_on(a4, LAPTOP_VELOCITY);
        square.note_on(a4, LAPTOP_VELOCITY);
        let sent_sound = engine.render(CHUNK);
        let square_sound = square.render(CHUNK);
        assert!(sent_sound == square_sound, "sending the song did not send its sound profile");
    }

    #[test]
    fn compose_starts_with_no_key_and_no_chords() {
        let view = Player::new().view(0);
        assert_eq!(view.keys.len(), 24);
        assert_eq!(view.key, None);
        assert!(view.progression.is_empty());
        assert!(view.guide.is_empty());
        assert!(view.melody.is_empty());
    }

    #[test]
    fn a_chord_added_after_it_is_let_go_keeps_how_long_it_was_held() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 2000);
        player.composer.add_last_chord();
        assert_eq!(player.song(3000), ["C for 2000 ms"]);
    }

    #[test]
    fn only_the_last_chord_tried_is_added() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);

        // Try C, then G, then F: only F goes in.
        player.tap("KeyC", 0, 500);
        player.tap("KeyG", 600, 1100);
        player.tap("KeyF", 1200, 2600);
        player.composer.add_last_chord();
        assert_eq!(player.song(3000), ["F for 1400 ms"]);

        // Try Am, add it, then add it again: both keep its length.
        player.tap("KeyQ", 3000, 3800);
        player.composer.add_last_chord();
        player.composer.add_last_chord();
        assert_eq!(player.song(4000), ["F for 1400 ms", "Am for 800 ms", "Am for 800 ms"]);
    }

    #[test]
    fn a_melody_note_is_not_a_chord() {
        let mut player = Player::new();
        player.tap("KeyQ", 0, 800);
        player.tap("KeyM", 1000, 1500);
        player.composer.add_last_chord();
        assert_eq!(player.song(2000), ["Am for 800 ms"]);
    }

    #[test]
    fn letting_go_of_a_melody_key_leaves_a_held_chord_held() {
        // Hold C, play M over it, and add C after letting go of M.
        let mut player = Player::new();
        player.press("KeyC", 0);
        player.tap("KeyM", 100, 200);
        player.composer.add_last_chord();
        assert_eq!(player.song(500), ["C for 500 ms so far"]);
    }

    #[test]
    fn a_chord_added_while_held_grows_until_it_is_let_go() {
        let mut player = Player::new();
        player.press("KeyG", 0);
        player.composer.add_last_chord();
        assert_eq!(player.song(1000), ["G for 1000 ms so far"]);

        player.release("KeyG", 2600);
        assert_eq!(player.song(5000), ["G for 2600 ms"]);
    }

    #[test]
    fn letting_go_of_another_key_leaves_a_held_chord_growing() {
        // Hold C, then G over it, and add G. Letting go of C first doesn't
        // end G.
        let mut player = Player::new();
        player.press("KeyC", 0);
        player.press("KeyG", 500);
        player.composer.add_last_chord();
        player.release("KeyC", 1000);
        assert_eq!(player.song(1500), ["G for 1000 ms so far"]);

        player.release("KeyG", 2000);
        assert_eq!(player.song(3000), ["G for 1500 ms"]);
    }

    #[test]
    fn letting_go_ends_only_the_chords_its_key_is_still_holding() {
        // C held for a second and added, then C again, added while held.
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.press("KeyC", 2000);
        player.composer.add_last_chord();
        player.release("KeyC", 2500);
        assert_eq!(player.song(3000), ["C for 1000 ms", "C for 500 ms"]);
    }

    #[test]
    fn letting_go_of_everything_ends_every_held_chord() {
        // C is added while held, then the window loses focus.
        let mut player = Player::new();
        player.press("KeyC", 0);
        player.composer.add_last_chord();
        player.release_all(700);
        assert_eq!(player.song(2000), ["C for 700 ms"]);

        // The chord tried last ended there too.
        player.composer.add_last_chord();
        assert_eq!(player.song(2000), ["C for 700 ms", "C for 700 ms"]);
    }

    #[test]
    fn a_chord_pressed_after_another_command_read_the_clock_has_no_length_yet() {
        // A view that read the clock at 1000 waited while G was pressed at
        // 1200, then added.
        let mut player = Player::new();
        player.press("KeyG", 1200);
        player.composer.add_last_chord();
        assert_eq!(player.song(1000), ["G for 0 ms so far"]);
    }

    #[test]
    fn adding_before_any_chord_is_played_does_nothing() {
        let mut player = Player::new();
        player.composer.add_last_chord();
        assert!(player.song(0).is_empty());
    }

    #[test]
    fn removing_takes_the_last_chord_off_the_end() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.tap("KeyG", 1000, 1500);
        player.composer.add_last_chord();

        player.composer.remove_last_chord();
        assert_eq!(player.song(2000), ["C for 1000 ms"]);
        player.composer.remove_last_chord();
        assert!(player.song(2000).is_empty());

        // With nothing left, removing does nothing.
        player.composer.remove_last_chord();
        assert!(player.song(2000).is_empty());
    }

    #[test]
    fn chords_added_are_named_for_the_key() {
        let mut player = Player::new();

        // 5 plays the chord on A#, also called Bb.
        player.tap("Digit5", 0, 1000);
        player.composer.add_last_chord();
        assert_eq!(player.song(1000), ["Bb for 1000 ms"]);

        // B major writes it A#. Picking a key keeps the chords.
        let b_major = Key {
            tonic: 11,
            mode: Mode::Major,
        };
        player.composer.pick_key(b_major);
        assert_eq!(player.song(1000), ["A# for 1000 ms"]);
    }

    #[test]
    fn a_held_chord_shows_at_the_end_until_it_is_added_or_let_go() {
        let mut player = Player::new();
        assert_eq!(player.holding(0), None);

        // Held, C grows at the end.
        player.press("KeyC", 0);
        assert_eq!(player.holding(500).as_deref(), Some("C for 500 ms so far"));

        // Added, the song shows it instead.
        player.composer.add_last_chord();
        assert_eq!(player.holding(600), None);
        assert_eq!(player.song(600), ["C for 600 ms so far"]);

        // Taken off again while still held, it is back at the end.
        player.composer.remove_last_chord();
        assert_eq!(player.holding(700).as_deref(), Some("C for 700 ms so far"));

        // Let go, it is gone, though Return can still add it.
        player.release("KeyC", 1000);
        assert_eq!(player.holding(1500), None);
        player.composer.add_last_chord();
        assert_eq!(player.song(1500), ["C for 1000 ms"]);
    }

    #[test]
    fn pressing_a_chord_in_the_song_again_holds_it_anew() {
        // C is in the song, and C pressed again is a new press, not in it.
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.press("KeyC", 2000);
        assert_eq!(player.holding(2500).as_deref(), Some("C for 500 ms so far"));
    }

    #[test]
    fn every_chord_has_its_root_for_its_color() {
        // Bb is played on A#, pitch 10, and Am on A, pitch 9, in any octave.
        let mut player = Player::new();
        player.tap("Digit5", 0, 500);
        player.composer.add_last_chord();
        player.press("KeyQ", 500);
        let view = player.view(600);
        assert_eq!(view.progression[0].root, 10);
        assert_eq!(view.holding.unwrap().root, 9);
    }

    #[test]
    fn every_chord_key_is_guided() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        let view = player.view(0);
        assert_eq!(view.guide.len(), 24);

        // In C major, A leads into Dm, Am is the key's own, Bb is borrowed
        // from C minor, and Bbm is outside.
        let fits = [
            ("KeyA", "A", Fit::CanWork),
            ("KeyQ", "Am", Fit::InKey),
            ("Digit5", "Bb", Fit::CanWork),
            ("Space", "Bbm", Fit::Outside),
        ];
        for (code, name, fit) in fits {
            let chord = &view.guide[code];
            assert_eq!((chord.name.as_str(), chord.fit), (name, fit), "key {code}");
        }
    }

    // The best chords to play next, as the view says, strongest first.
    fn best_next(view: &ComposeView) -> Vec<String> {
        let mut ranked = Vec::new();
        for chord in view.guide.values() {
            if let Some(next) = &chord.next {
                ranked.push((next.rank, chord.name.clone()));
            }
        }
        ranked.sort();
        let mut names = Vec::new();
        for (_, name) in ranked {
            names.push(name);
        }
        names
    }

    #[test]
    fn best_next_follows_the_song_s_last_chord() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);

        // Before the first chord, the best starts.
        assert_eq!(best_next(&player.view(0)), ["C", "F", "Am"]);

        // After C, then after G, added while still held.
        player.tap("KeyC", 0, 500);
        player.composer.add_last_chord();
        assert_eq!(best_next(&player.view(500)), ["F", "G", "Am"]);
        player.press("KeyG", 500);
        player.composer.add_last_chord();
        assert_eq!(best_next(&player.view(600)), ["C", "Am", "F"]);

        // After Bbm, outside the key, home.
        player.release("KeyG", 1000);
        player.tap("Space", 1000, 1500);
        player.composer.add_last_chord();
        assert_eq!(best_next(&player.view(1500)), ["C"]);

        // Delete takes the last chord off, so the one before is last again.
        player.composer.remove_last_chord();
        assert_eq!(best_next(&player.view(1500)), ["C", "Am", "F"]);
        player.composer.remove_last_chord();
        player.composer.remove_last_chord();
        assert_eq!(best_next(&player.view(1500)), ["C", "F", "Am"]);
    }

    #[test]
    fn trying_a_chord_never_changes_the_best_next() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        player.tap("KeyC", 0, 500);
        player.composer.add_last_chord();

        // G tried and let go, then Bbm held: C is still the last chord.
        player.tap("KeyG", 500, 1000);
        assert_eq!(best_next(&player.view(1000)), ["F", "G", "Am"]);
        player.press("Space", 1000);
        assert_eq!(best_next(&player.view(1200)), ["F", "G", "Am"]);
    }

    #[test]
    fn a_new_key_ranks_the_song_s_last_chord_again() {
        // G is the tension chord in C major, and home in G major.
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        player.tap("KeyG", 0, 500);
        player.composer.add_last_chord();
        assert_eq!(best_next(&player.view(500)), ["C", "Am", "F"]);

        player.composer.pick_key(Key {
            tonic: 7,
            mode: Mode::Major,
        });
        assert_eq!(best_next(&player.view(500)), ["C", "D", "Em"]);
    }

    #[test]
    fn every_melody_key_is_guided() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        let view = player.view(0);
        assert_eq!(view.melody.len(), 24);

        // M plays C4 and N plays G#3. In C major, C is in the key and G#
        // is outside. In A minor, G# is the raised note below home, so it
        // is in the key too, and A#3 on Y is outside.
        assert_eq!(view.melody["KeyM"], Fit::InKey);
        assert_eq!(view.melody["KeyJ"], Fit::Outside);
        assert_eq!(view.melody["KeyN"], Fit::Outside);

        player.composer.pick_key(Key {
            tonic: 9,
            mode: Mode::Minor,
        });
        let view = player.view(0);
        assert_eq!(view.melody["KeyN"], Fit::InKey);
        assert_eq!(view.melody["KeyY"], Fit::Outside);
    }

    fn playhead(chord: usize, ms_in: u64) -> Option<(usize, u64)> {
        Some((chord, ms_in))
    }

    // The keys lit while the song plays, as `lit_keys` reads them.
    fn lit(now: &[&str], next: &[&str]) -> Option<(Vec<String>, Vec<String>)> {
        let mut now_keys = Vec::new();
        for key in now {
            now_keys.push(String::from(*key));
        }
        let mut next_keys = Vec::new();
        for key in next {
            next_keys.push(String::from(*key));
        }
        Some((now_keys, next_keys))
    }

    #[test]
    fn keys_follow_a_section_only_while_it_plays_and_a_new_scale_forgets_them() {
        let c_major = Scale { tonic: 0, kind: crate::scales::ScaleKind::Major };
        let a_minor = Scale { tonic: 9, kind: crate::scales::ScaleKind::Minor };
        let mut composer = Composer::new();
        composer.pick_scale(c_major);
        let main = composer.main_keys().clone();
        let own = KeyMapping::default_for(a_minor);
        let section = SongSection {
            keys: Some(own.clone()),
            name: String::from("Verse"),
            bars: 1,
            tempo: 120,
            key_shift: 0,
            layers: Vec::new(),
            automation: Vec::new(),
        };
        composer.set_sections(vec![section], vec![0]);
        assert_eq!(*composer.keys_now(), main);
        composer.set_section_playing(Some(0));
        assert_eq!(*composer.keys_now(), own);
        composer.set_section_playing(None);
        assert_eq!(*composer.keys_now(), main);

        composer.pick_scale(a_minor);
        assert_eq!(composer.sections()[0].keys, None);
    }

    // The notes laptop key `key` plays.
    fn notes_of(key: &str) -> [u8; 3] {
        let mut key_map = KeyMap::with_test_layout();
        match key_map.press(key) {
            Some(KeyAction::PlayNotes(notes)) => notes.try_into().unwrap(),
            _ => panic!("{key} plays no chord"),
        }
    }

    #[test]
    fn clicking_a_chord_plays_it_as_it_was_played_for_as_long_as_it_was_held() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.tap("KeyG", 1000, 1700);
        player.composer.add_last_chord();

        // Clicking G plays it for its 700 ms, then lets it fade.
        player.click(1, 2000).unwrap();
        let clicked = player.listen(1100);

        // The same G played on the keys for 700 ms.
        let mut by_hand = Engine::new().unwrap();
        for note in notes_of("KeyG") {
            by_hand.note_on(note, LAPTOP_VELOCITY);
        }
        let mut played = by_hand.render(700 * 48);
        for note in notes_of("KeyG") {
            by_hand.note_off(note);
        }
        let fade = by_hand.render(400 * 48);
        played.extend(fade);

        assert!(clicked == played, "clicking G sounds different from playing it");
    }

    #[test]
    fn play_plays_every_chord_in_order_and_the_playhead_follows() {
        // C for 2 s, G for 1 s, then F added while held: at 3500 it has
        // been held for 500 ms so far, and plays that long.
        let mut player = Player::new();
        player.tap("KeyC", 0, 2000);
        player.composer.add_last_chord();
        player.tap("KeyG", 2000, 3000);
        player.composer.add_last_chord();
        player.press("KeyF", 3000);
        player.composer.add_last_chord();
        assert_eq!(player.playhead(), None);

        player.play(3500);
        assert_eq!(player.playhead(), playhead(0, 0));
        player.listen(1990);
        assert_eq!(player.playhead(), playhead(0, 1990));
        player.listen(10);
        assert_eq!(player.playhead(), playhead(1, 0));
        player.listen(1000);
        assert_eq!(player.playhead(), playhead(2, 0));
        player.listen(490);
        assert_eq!(player.playhead(), playhead(2, 490));
        player.listen(10);
        assert_eq!(player.playhead(), None);
    }

    #[test]
    fn clicking_a_chord_moves_the_playhead_to_it() {
        let mut player = Player::new();
        for (key, from) in [("KeyC", 0), ("KeyG", 1000), ("KeyF", 2000)] {
            player.tap(key, from, from + 1000);
            player.composer.add_last_chord();
        }

        // Clicked while the song plays, F starts over on its own.
        player.play(3000);
        player.listen(500);
        player.click(2, 3500).unwrap();
        assert_eq!(player.playhead(), playhead(2, 0));
        player.listen(100);
        assert_eq!(player.playhead(), playhead(2, 100));
        player.listen(900);
        assert_eq!(player.playhead(), None);

        // Play starts from the first chord again.
        player.play(5000);
        assert_eq!(player.playhead(), playhead(0, 0));

        // There is no fourth chord to click.
        assert!(player.click(3, 5000).is_err());
    }

    #[test]
    fn the_playhead_leaves_a_chord_taken_off_the_song() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.tap("KeyG", 1000, 2000);
        player.composer.add_last_chord();
        player.play(2000);
        player.listen(1500);
        assert_eq!(player.playhead(), playhead(1, 500));

        player.composer.remove_last_chord();
        assert_eq!(player.playhead(), None);
    }

    #[test]
    fn a_song_longer_than_the_engine_plays_stops_after_512_chords() {
        // 513 chords of 10 ms: the first 512 play, 5,120 ms in all. The
        // 512th lights nothing as next, since the 513th never plays.
        let mut player = Player::new();
        player.tap("KeyC", 0, 10);
        for _ in 0..513 {
            player.composer.add_last_chord();
        }
        player.play(1000);
        player.listen(5100);
        assert_eq!(player.playhead(), playhead(510, 0));
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyC"]));
        player.listen(10);
        assert_eq!(player.playhead(), playhead(511, 0));
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &[]));
        player.listen(10);
        assert_eq!(player.playhead(), None);
    }

    #[test]
    fn while_the_song_plays_the_keyboard_lights_the_chord_playing_and_the_next() {
        // C, G and Am, a second each. Am is on Q.
        let mut player = Player::new();
        for (key, from) in [("KeyC", 0), ("KeyG", 1000), ("KeyQ", 2000)] {
            player.tap(key, from, from + 1000);
            player.composer.add_last_chord();
        }
        assert_eq!(player.lit_keys(), None);

        player.play(3000);
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyG"]));
        player.listen(990);
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyG"]));
        player.listen(10);
        assert_eq!(player.lit_keys(), lit(&["KeyG"], &["KeyQ"]));

        // Am is the song's last chord, so nothing comes after it.
        player.listen(1000);
        assert_eq!(player.lit_keys(), lit(&["KeyQ"], &[]));
        player.listen(1000);
        assert_eq!(player.lit_keys(), None);
    }

    #[test]
    fn the_keys_lit_are_the_ones_that_play_those_chords() {
        // Bbm, outside every key here, is on Space, and F#m on left
        // Control. Picking a key renames chords but never moves them.
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        for (key, from) in [("Space", 0), ("ControlLeft", 1000)] {
            player.tap(key, from, from + 1000);
            player.composer.add_last_chord();
        }
        player.play(2000);
        assert_eq!(player.lit_keys(), lit(&["Space"], &["ControlLeft"]));

        player.composer.pick_key(Key {
            tonic: 6,
            mode: Mode::Minor,
        });
        assert_eq!(player.lit_keys(), lit(&["Space"], &["ControlLeft"]));
    }

    #[test]
    fn a_chord_played_twice_in_a_row_lights_its_key_now_and_next() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.composer.add_last_chord();
        player.play(1000);
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyC"]));
        player.listen(1000);
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &[]));
    }

    #[test]
    fn a_chord_clicked_on_its_own_lights_nothing_next() {
        // Clicking C plays C alone, so G after it is not coming.
        let mut player = Player::new();
        for (key, from) in [("KeyC", 0), ("KeyG", 1000)] {
            player.tap(key, from, from + 1000);
            player.composer.add_last_chord();
        }
        player.click(0, 2000).unwrap();
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &[]));

        // Play then plays on to G again.
        player.play(2500);
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyG"]));
    }

    #[test]
    fn nothing_is_lit_next_once_the_chord_after_is_taken_off() {
        // Chords added while the song plays don't play, so they aren't lit
        // either.
        let mut player = Player::new();
        player.tap("KeyC", 0, 1000);
        player.composer.add_last_chord();
        player.tap("KeyG", 1000, 2000);
        player.composer.add_last_chord();
        player.play(2000);
        player.listen(500);
        player.composer.remove_last_chord();
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &[]));

        player.tap("KeyF", 2500, 3000);
        player.composer.add_last_chord();
        assert_eq!(player.lit_keys(), lit(&["KeyC"], &["KeyF"]));
    }

    #[test]
    fn playing_an_empty_song_plays_nothing() {
        let mut player = Player::new();
        player.play(0);
        assert_eq!(player.playhead(), None);
        let sound = player.listen(100);
        assert!(sound.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn the_ui_reads_compose_by_these_names() {
        // ui/compose.ts reads these names, so renaming them here would break
        // Compose.
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        player.tap("KeyG", 0, 1000);
        player.composer.add_last_chord();
        player.press("KeyF", 1000);
        player.composer.add_last_chord();

        let json = serde_json::to_value(player.view(1500)).unwrap();
        assert_eq!(json["playing"], serde_json::Value::Null);
        assert_eq!(json["holding"], serde_json::Value::Null);
        assert_eq!(json["keys"][0]["name"], "C major");
        assert_eq!(json["keys"][12]["mode"], "minor");
        assert_eq!(json["key"]["tonic"], 0);
        assert_eq!(json["key"]["mode"], "major");
        assert_eq!(json["progression"][0]["name"], "G");
        assert_eq!(json["progression"][0]["root"], 7);
        assert_eq!(json["progression"][0]["held_ms"], 1000);
        assert_eq!(json["progression"][0]["still_held"], false);
        assert_eq!(json["progression"][1]["held_ms"], 500);
        assert_eq!(json["progression"][1]["still_held"], true);
        assert_eq!(json["guide"]["KeyG"]["name"], "G");
        assert_eq!(json["guide"]["KeyG"]["fit"], "in_key");
        assert_eq!(json["guide"]["KeyG"]["why"], "Tension that pulls back home to C.");
        assert_eq!(json["guide"]["KeyQ"]["fit"], "in_key");
        // F is the song's last chord, so C is the best next and Am isn't one.
        assert_eq!(json["guide"]["KeyC"]["next"]["rank"], 1);
        assert_eq!(json["guide"]["KeyC"]["next"]["why"], "Home, the most common move of all.");
        assert_eq!(json["guide"]["KeyQ"]["next"], serde_json::Value::Null);
        assert_eq!(json["melody"]["KeyM"], "in_key");
        assert_eq!(json["melody"]["KeyJ"], "outside");

        player.play(1500);
        let json = serde_json::to_value(player.view(1500)).unwrap();
        assert_eq!(json["playing"]["now_keys"][0], "KeyG");
        assert_eq!(json["playing"]["next_keys"][0], "KeyF");

        player.click(1, 1500).unwrap();
        player.listen(100);
        let json = serde_json::to_value(player.view(1600)).unwrap();
        assert_eq!(json["playing"]["chord"], 1);
        assert_eq!(json["playing"]["ms_in"], 100);
        assert_eq!(json["playing"]["now_keys"][0], "KeyF");
        assert_eq!(json["playing"]["next_keys"], serde_json::json!([]));

        player.press("KeyQ", 1600);
        let json = serde_json::to_value(player.view(1700)).unwrap();
        assert_eq!(json["holding"]["name"], "Am");
        assert_eq!(json["holding"]["root"], 9);
        assert_eq!(json["holding"]["held_ms"], 100);
        assert_eq!(json["holding"]["still_held"], true);
    }
    #[test]
    fn live_guidance_tracks_playing_without_changing_the_saved_song_guidance() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        player.tap("KeyC", 0, 500);
        let first = player.view(500);
        player.tap("KeyG", 600, 900);
        let second = player.view(900);
        assert_eq!(first.guide, second.guide);
        assert_ne!(first.live_guide, second.live_guide);
        assert_eq!(second.reference.unwrap().name, "G");
        assert!(second.progression.is_empty());
        assert_eq!(second.history.len(), 2);
    }

    #[test]
    fn history_selection_addition_and_reordering_preserve_the_original_timing() {
        let mut player = Player::new();
        player.tap("KeyC", 0, 500);
        player.tap("KeyG", 600, 900);
        player.composer.edit("select_history", 0).unwrap();
        assert_eq!(player.view(1000).reference.unwrap().name, "C");
        player.composer.edit("add_history", 1).unwrap();
        player.composer.edit("add_history", 0).unwrap();
        player.composer.edit("left", 1).unwrap();
        let view = player.view(1000);
        assert_eq!(view.progression[0].name, "C");
        assert_eq!(view.progression[0].held_ms, 500);
        assert_eq!(view.progression[1].held_ms, 300);
        player.tap("KeyF", 1100, 1200);
        assert_eq!(player.view(1200).selected_history, None);
        assert!(player.composer.edit("remove", 99).is_err());
    }

    #[test]
    fn every_melody_press_refreshes_nearby_next_notes() {
        let mut player = Player::new();
        player.composer.pick_key(C_MAJOR);
        player.tap("KeyM", 0, 100);
        let first = player.view(100);
        assert_eq!(first.melody_next.len(), 3);
        assert!(!first.melody_next.contains_key("KeyM"));
        player.tap("Digit8", 200, 300);
        let second = player.view(300);
        assert_eq!(second.last_note, Some(67));
        assert_ne!(first.melody_next, second.melody_next);
        for code in second.melody_next.keys() {
            assert_eq!(second.melody[code], Fit::InKey);
        }
    }

    #[test]
    fn tutor_events_measure_actual_pitches_and_lengths_across_octave_changes() {
        let mut player = Player::new();
        player.press("KeyM", 100);
        player.key_map.shift_octave(1);
        player.release("KeyM", 500);
        player.press("KeyM", 600);
        player.composer.all_released(player.start + Duration::from_millis(900));
        let view = player.view(900);
        assert_eq!(view.performance[0].notes, [60]);
        assert_eq!(view.performance[0].held_ms, Some(400));
        assert_eq!(view.performance[1].notes, [72]);
        assert_eq!(view.performance[1].held_ms, Some(300));
        assert!(view.performance[1].id > view.performance[0].id);
    }

}
