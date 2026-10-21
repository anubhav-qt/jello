// Guided composing (docs/design.md, "Chord guidance" and "Compose screen"):
// the song's key, the chords added to the song so far with how long each was
// held, how well each chord and melody key fits the key, and playing the song
// back.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::engine::{Engine, SONG_MAX_CHORDS, SongChord, SongPlace};
use crate::harmony::{Chord, Fit, Guide, Key, Mode, all_keys, chord_name, guide, key_name, note_fit};
use crate::key_map::{KeyMap, LAPTOP_VELOCITY};

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

// Where the song is playing, for the UI to draw the playhead.
#[derive(Debug, PartialEq, Serialize)]
pub struct Playhead {
    // The chord playing, by its place in the song, from 0.
    chord: usize,
    // How far into it, in milliseconds.
    ms_in: u64,
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
    // No key yet, and no chords.
    pub fn new() -> Composer {
        Composer {
            key: None,
            tried: None,
            progression: Vec::new(),
            playing_from: 0,
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

        let mut chord_guide = HashMap::new();
        let mut melody = HashMap::new();
        if let Some(key) = self.key {
            for (code, chord) in key_map.chord_keys() {
                chord_guide.insert(code, guide(key, chord));
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
            playing: self.playhead(song_place),
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
    // `song_place`. None while the song isn't playing, or once the chord
    // playing has been taken off the song.
    fn playhead(&self, song_place: Option<SongPlace>) -> Option<Playhead> {
        let song_place = match song_place {
            Some(song_place) => song_place,
            None => return None,
        };
        let chord = self.playing_from + song_place.chord;
        if chord >= self.progression.len() {
            return None;
        }
        Some(Playhead {
            chord,
            ms_in: song_place.ms_in,
        })
    }
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

        // Where the playhead is, read the way the UI reads it. The clock
        // plays no part, so it is read at 0.
        fn playhead(&self) -> Option<Playhead> {
            self.view(0).playing
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

    fn playhead(chord: usize, ms_in: u64) -> Option<Playhead> {
        Some(Playhead { chord, ms_in })
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
        // 513 chords of 10 ms: the first 512 play, 5,120 ms in all.
        let mut player = Player::new();
        player.tap("KeyC", 0, 10);
        for _ in 0..513 {
            player.composer.add_last_chord();
        }
        player.play(1000);
        player.listen(5110);
        assert_eq!(player.playhead(), playhead(511, 0));
        player.listen(10);
        assert_eq!(player.playhead(), None);
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
        assert_eq!(json["melody"]["KeyM"], "in_key");
        assert_eq!(json["melody"]["KeyJ"], "outside");

        player.click(1, 1500).unwrap();
        player.listen(100);
        let json = serde_json::to_value(player.view(1600)).unwrap();
        assert_eq!(json["playing"]["chord"], 1);
        assert_eq!(json["playing"]["ms_in"], 100);

        player.press("KeyQ", 1600);
        let json = serde_json::to_value(player.view(1700)).unwrap();
        assert_eq!(json["holding"]["name"], "Am");
        assert_eq!(json["holding"]["root"], 9);
        assert_eq!(json["holding"]["held_ms"], 100);
        assert_eq!(json["holding"]["still_held"], true);
    }
}
