// Guided composing (docs/design.md, "Chord guidance" and "Compose screen"):
// the song's key, the chords added to the song so far with how long each was
// held, how well each chord and melody key fits the key, playing the song
// back, the song's tempo and its sound profile.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::engine::{Engine, FASTEST_TEMPO, SLOWEST_TEMPO, SONG_MAX_CHORDS, START_TEMPO, SongChord, SongPlace};
use crate::harmony::{Chord, Fit, Guide, Key, Mode, all_keys, chord_name, guide_after, key_name, note_fit};
use crate::key_map::{KeyMap, LAPTOP_VELOCITY};
use crate::sound_file::Sound;
use crate::sound_profile::SoundProfile;

// How much slowing down or speeding up changes the song's tempo.
const TEMPO_STEP: u32 = 5;

pub struct Composer {
    // The song's key, picked before starting. None until then.
    key: Option<Key>,
    // The chord played last, as it was played. None until a chord is.
    tried: Option<PlayedChord>,
    // The chords added so far, in order, each as it was played. Trying a
    // chord doesn't add it: only asking to add it does.
    progression: Vec<PlayedChord>,
    // Where in the song the chords played last start, so the playhead can
    // tell which chord is playing.
    playing_from: usize,
    // Where they stop: the place just after the last of them, so the
    // keyboard knows whether another chord comes after the one playing.
    playing_until: usize,
    // The song's tempo, in beats per minute: the click and the looper play
    // at it. A new song starts at 120.
    tempo: u32,
    // How the song's notes sound: its oscillators, envelopes, filter,
    // unison and tuning (docs/adr/0030-a-sound-profile-per-song.md). Every
    // note played while the song is open plays it, in Play too.
    sound_profile: SoundProfile,
    // The preset the song's sound was loaded from or saved as last, with
    // the sound profile as it was right then (docs/adr/0037-presets.md).
    // While the song still sounds exactly like that, the Presets page
    // lights the preset's tile. None until a preset is loaded or saved.
    preset: Option<(String, SoundProfile)>,
}

// A chord as it was played: by which key, when, and for how long. The player
// composes by feel, so the length stays exactly as held, never moved onto
// the beat (docs/adr/0022-compose-as-a-tape.md).
#[derive(Clone)]
struct PlayedChord {
    chord: Chord,
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
            notes: self.chord.notes().to_vec(),
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
    // Where the song is playing, or None while it isn't.
    playing: Option<Playhead>,
}

impl Composer {
    // No key yet, no chords, a new song's tempo and a new song's sound
    // profile.
    pub fn new() -> Composer {
        Composer {
            key: None,
            tried: None,
            progression: Vec::new(),
            playing_from: 0,
            playing_until: 0,
            tempo: START_TEMPO,
            sound_profile: SoundProfile::new(),
            preset: None,
        }
    }

    // Sends the song's tempo and its whole sound profile to `engine`, so
    // everything after this plays them. The app does this as it starts,
    // before the first note.
    pub fn send_all(&self, engine: &mut Engine) {
        engine.set_tempo(self.tempo);
        self.sound_profile.send_all(engine);
    }

    // Plays the song at `tempo` beats per minute from the next beat on, on
    // `engine` too. The song keeps the tempo the engine plays, which is the
    // one it had for a tempo outside SLOWEST_TEMPO to FASTEST_TEMPO, or while
    // the looper is busy.
    fn set_tempo(&mut self, engine: &mut Engine, tempo: u32) {
        self.tempo = engine.set_tempo(tempo);
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

    pub fn sound_profile(&self) -> &SoundProfile {
        &self.sound_profile
    }

    // For changing the song's sound. Each change goes on to the engine.
    pub fn sound_profile_mut(&mut self) -> &mut SoundProfile {
        &mut self.sound_profile
    }

    // Copies preset `name`'s `sound` into the song, on `engine` too, and
    // remembers it as the preset the song sounds like.
    pub fn take_preset(&mut self, engine: &mut Engine, name: &str, sound: &Sound) {
        self.sound_profile.take_sound(engine, sound);
        self.preset = Some((String::from(name), self.sound_profile));
    }

    // Remembers that the song's sound was just saved as preset `name`.
    pub fn saved_as_preset(&mut self, name: &str) {
        self.preset = Some((String::from(name), self.sound_profile));
    }

    // The preset the song sounds exactly like, or None once its sound has
    // changed since loading or saving it, or if it never did either.
    pub fn lit_preset(&self) -> Option<&str> {
        let Some((name, profile)) = &self.preset else {
            return None;
        };
        if *profile == self.sound_profile {
            Some(name.as_str())
        } else {
            None
        }
    }

    // Sets the song's key. Chords added already stay, named for the new key.
    pub fn pick_key(&mut self, key: Key) {
        self.key = Some(key);
    }

    // `key` went down at `now` and plays `chord`, so it is the chord tried
    // last.
    pub fn chord_pressed(&mut self, key: &str, chord: Chord, now: Instant) {
        self.tried = Some(PlayedChord {
            chord,
            key: String::from(key),
            pressed_at: now,
            held_for: None,
        });
    }

    // `key` was let go at `now`, so the chords it is still holding end there:
    // the one tried last, and the ones added to the song while it was held.
    // A key can't be pressed again while it is held, so they all come from
    // the same press.
    pub fn key_released(&mut self, key: &str, now: Instant) {
        if let Some(tried) = &mut self.tried {
            if tried.key == key {
                tried.let_go(now);
            }
        }
        for played in &mut self.progression {
            if played.key == key {
                played.let_go(now);
            }
        }
    }

    // Every key was let go at once at `now`, so every chord still held ends.
    pub fn all_released(&mut self, now: Instant) {
        if let Some(tried) = &mut self.tried {
            tried.let_go(now);
        }
        for played in &mut self.progression {
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

        ComposeView {
            keys,
            key: picked,
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
        let now_keys = keys_playing(key_map, self.progression[chord].chord);
        let next_keys = match self.next_place(chord) {
            Some(next) => keys_playing(key_map, self.progression[next].chord),
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

// The chord keys on `key_map` that play `chord`, in order by name. Every
// chord has its own key (docs/adr/0020-every-chord-on-its-own-key.md), so this
// is one key, or none if no key plays it now.
fn keys_playing(key_map: &KeyMap, chord: Chord) -> Vec<String> {
    let mut keys = Vec::new();
    for (code, key_chord) in key_map.chord_keys() {
        if key_chord == chord {
            keys.push(code);
        }
    }
    keys.sort();
    keys
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

    const C_MAJOR: Key = Key {
        tonic: 0,
        mode: Mode::Major,
    };

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
                key_map: KeyMap::new(),
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
        composer.take_preset(&mut engine, "Low", &sound);
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
        assert_eq!(view.melody.len(), 20);

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

    // The notes laptop key `key` plays.
    fn notes_of(key: &str) -> [u8; 3] {
        let mut key_map = KeyMap::new();
        match key_map.press(key) {
            Some(KeyAction::PlayChord(chord)) => chord.notes(),
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
}
