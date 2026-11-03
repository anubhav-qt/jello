// Takes (docs/design.md#ai-and-learning-rethought-2026-11-03): whatever one
// answer of the agent changes is one take, laid on the song as a draft. The
// player flips between their version and the take, keeps it or drops it,
// or drops one part of it (the tape, Sound or Mix). This replaces Undo per
// reply.
//
// A take keeps two copies of the song as file text: `mine`, from before the
// answer, and `theirs`, from after it. Whichever is not on screen is kept up
// to date as the player flips, so their own edits on either side stay.
//
// The takes are locked last, after the open song, Compose and the engine.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::compose::Composer;
use crate::engine::Engine;
use crate::phrases::Phrases;
use crate::song_file::{Song, song_of, text_of};
use crate::song_session::{OpenSong, put_back_song, song_now};
use crate::sound_profile::SoundProfile;

// How many takes are remembered.
const KEPT_TAKES: usize = 20;

// The parts a take can change, as the rail names them.
pub const TAPE: &str = "Tape";
pub const SOUND: &str = "Sound";
pub const MIX: &str = "Mix";

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TakeState {
    // The answer is still running.
    Running,
    Open,
    Kept,
    Dropped,
}

// One answer's take.
struct Take {
    number: u64,
    // The song it was made on.
    name: String,
    // The song before the answer, as the player has it.
    mine: String,
    // The song after the answer. Empty while it runs.
    theirs: String,
    parts: Vec<&'static str>,
    state: TakeState,
    // Whether the take is the one on screen and heard.
    showing_take: bool,
}

// What the rail shows for a take.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TakeView {
    pub number: u64,
    pub parts: Vec<&'static str>,
    pub state: TakeState,
    #[serde(rename = "showingTake")]
    pub showing_take: bool,
}

// Every take of this run, oldest first.
#[derive(Default)]
pub struct Takes {
    started: u64,
    takes: VecDeque<Take>,
}

// What the caller must put on screen after a change: the song `name` as
// `text`, or nothing.
#[derive(Debug, PartialEq)]
pub struct PutBack {
    pub name: String,
    pub text: String,
}

impl Takes {
    // Starts a take on song `name`, whose text is `mine` now, and returns its
    // number, counted from 1. A take still open is kept, since the new
    // answer builds on it.
    pub fn start(&mut self, name: &str, mine: String) -> u64 {
        for take in self.takes.iter_mut() {
            if take.state == TakeState::Open || take.state == TakeState::Running {
                take.state = TakeState::Kept;
            }
        }
        self.started += 1;
        let take = Take {
            number: self.started,
            name: String::from(name),
            mine,
            theirs: String::new(),
            parts: Vec::new(),
            state: TakeState::Running,
            showing_take: true,
        };
        self.takes.push_back(take);
        if self.takes.len() > KEPT_TAKES {
            self.takes.pop_front();
        }
        self.started
    }

    // Ends take `number` with the song as `theirs` now. A take that changed
    // nothing is forgotten and None comes back.
    pub fn end(&mut self, number: u64, theirs: String) -> Result<Option<TakeView>, String> {
        let take = self.running(number)?;
        let parts = parts_changed(&take.mine, &theirs)?;
        if parts.is_empty() {
            self.takes.retain(|take| take.number != number);
            return Ok(None);
        }
        take.theirs = theirs;
        take.parts = parts;
        take.state = TakeState::Open;
        Ok(Some(view_of(take)))
    }

    // Flips take `number` between the player's version and the take. `now`
    // is the song on screen, kept as the side being left.
    pub fn flip(&mut self, number: u64, now: String) -> Result<(TakeView, PutBack), String> {
        let take = self.open(number)?;
        let put_back;
        if take.showing_take {
            take.theirs = now;
            put_back = PutBack { name: take.name.clone(), text: take.mine.clone() };
        } else {
            take.mine = now;
            put_back = PutBack { name: take.name.clone(), text: take.theirs.clone() };
        }
        take.showing_take = !take.showing_take;
        Ok((view_of(take), put_back))
    }

    // Keeps take `number`. The take goes on screen if the player's version
    // was showing.
    pub fn keep(&mut self, number: u64) -> Result<(TakeView, Option<PutBack>), String> {
        let take = self.open(number)?;
        let mut put_back = None;
        if !take.showing_take {
            put_back = Some(PutBack { name: take.name.clone(), text: take.theirs.clone() });
            take.showing_take = true;
        }
        take.state = TakeState::Kept;
        Ok((view_of(take), put_back))
    }

    // Drops take `number`. The player's version goes on screen if the take
    // was showing.
    pub fn drop_take(&mut self, number: u64) -> Result<(TakeView, Option<PutBack>), String> {
        let take = self.open(number)?;
        let mut put_back = None;
        if take.showing_take {
            put_back = Some(PutBack { name: take.name.clone(), text: take.mine.clone() });
            take.showing_take = false;
        }
        take.state = TakeState::Dropped;
        Ok((view_of(take), put_back))
    }

    // Drops `part` of take `number`: the take keeps the player's version of
    // it. `now` is the song on screen. Dropping the last part drops the take.
    pub fn drop_part(&mut self, number: u64, part: &str, now: String) -> Result<(TakeView, Option<PutBack>), String> {
        let take = self.open(number)?;
        let Some(place) = take.parts.iter().position(|kept| *kept == part) else {
            return Err(format!("take {number} has no {part} part"));
        };
        if take.parts.len() == 1 {
            return self.drop_take(number);
        }
        if take.showing_take {
            take.theirs = now;
        }
        let theirs = with_part_of(&take.theirs, &take.mine, take.parts[place])?;
        take.parts = parts_changed(&take.mine, &theirs)?;
        take.theirs = theirs;
        let mut put_back = None;
        if take.showing_take {
            put_back = Some(PutBack { name: take.name.clone(), text: take.theirs.clone() });
        }
        if take.parts.is_empty() {
            take.state = TakeState::Dropped;
        }
        Ok((view_of(take), put_back))
    }

    // The take open now, if there is one.
    pub fn open_now(&self) -> Option<TakeView> {
        for take in &self.takes {
            if take.state == TakeState::Open {
                return Some(view_of(take));
            }
        }
        None
    }

    fn running(&mut self, number: u64) -> Result<&mut Take, String> {
        let Some(take) = self.takes.iter_mut().find(|take| take.number == number) else {
            return Err(format!("take {number} is gone"));
        };
        if take.state != TakeState::Running {
            return Err(format!("take {number} has already ended"));
        }
        Ok(take)
    }

    fn open(&mut self, number: u64) -> Result<&mut Take, String> {
        let Some(take) = self.takes.iter_mut().find(|take| take.number == number) else {
            return Err(format!("take {number} is gone"));
        };
        if take.state != TakeState::Open {
            return Err(format!("take {number} is no longer open"));
        }
        Ok(take)
    }
}

fn view_of(take: &Take) -> TakeView {
    TakeView {
        number: take.number,
        parts: take.parts.clone(),
        state: take.state,
        showing_take: take.showing_take,
    }
}

fn song_of_text(text: &str) -> Result<Song, String> {
    song_of(text, &SoundProfile::new())
}

// Whether `a` and `b` differ in `part`.
fn part_differs(a: &Song, b: &Song, part: &str) -> bool {
    if part == TAPE {
        let same = a.chords == b.chords
            && a.looped == b.looped
            && a.sections == b.sections
            && a.arrangement == b.arrangement
            && a.tempo == b.tempo
            && a.snap == b.snap;
        return !same;
    }
    if part == SOUND {
        let same = a.key == b.key
            && a.scale == b.scale
            && a.keys == b.keys
            && a.tracks == b.tracks
            && a.picked == b.picked;
        return !same;
    }
    a.mix != b.mix
}

// The parts `theirs` changed from `mine`, in the rail's order.
fn parts_changed(mine: &str, theirs: &str) -> Result<Vec<&'static str>, String> {
    let mine = song_of_text(mine)?;
    let theirs = song_of_text(theirs)?;
    let mut parts = Vec::new();
    for part in [TAPE, SOUND, MIX] {
        if part_differs(&mine, &theirs, part) {
            parts.push(part);
        }
    }
    Ok(parts)
}

// `theirs` with `part` taken from `mine`.
fn with_part_of(theirs: &str, mine: &str, part: &str) -> Result<String, String> {
    let mut song = song_of_text(theirs)?;
    let mine = song_of_text(mine)?;
    if part == TAPE {
        song.chords = mine.chords;
        song.looped = mine.looped;
        song.sections = mine.sections;
        song.arrangement = mine.arrangement;
        song.tempo = mine.tempo;
        song.snap = mine.snap;
    } else if part == SOUND {
        song.key = mine.key;
        song.scale = mine.scale;
        song.keys = mine.keys;
        song.tracks = mine.tracks;
        song.picked = mine.picked;
    } else {
        song.mix = mine.mix;
    }
    Ok(text_of(&song))
}

// The song on screen as text.
fn text_now(composer: &Composer, engine: &mut Engine) -> String {
    text_of(&song_now(composer, engine))
}

// Puts `put_back` on screen, keeping the path (Learn and the stages) as it
// is now: a take never changes those.
fn put_on_screen(
    open: &mut OpenSong,
    composer: &mut Composer,
    engine: &mut Engine,
    put_back: &PutBack,
    now: Instant,
) -> Result<(), String> {
    let mut song = song_of_text(&put_back.text)?;
    song.path = composer.path().clone();
    put_back_song(open, composer, engine, &put_back.name, &text_of(&song), now)
}

// Starts a take on the song open now, before an answer changes it.
pub(crate) fn start_take(takes: &mut Takes, open: &OpenSong, composer: &Composer, engine: &mut Engine) -> u64 {
    takes.start(open.name(), text_now(composer, engine))
}

// The panel calls this as the player sends the agent a message.
#[tauri::command]
pub fn take_starts(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    takes: State<'_, Mutex<Takes>>,
) -> u64 {
    let open = open.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    let mut takes = takes.lock().unwrap();
    start_take(&mut takes, &open, &composer, &mut engine)
}

// The panel calls this when the answer ends. None if it changed nothing.
#[tauri::command]
pub fn take_ends(
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    takes: State<'_, Mutex<Takes>>,
    take: u64,
) -> Result<Option<TakeView>, String> {
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    let theirs = text_now(&composer, &mut engine);
    takes.lock().unwrap().end(take, theirs)
}

// What the player does to a take: "flip", "keep", "drop", or "drop part".
fn change_take(
    open: &mut OpenSong,
    composer: &mut Composer,
    engine: &mut Engine,
    takes: &mut Takes,
    take: u64,
    action: &str,
    part: Option<&str>,
    now: Instant,
) -> Result<TakeView, String> {
    let screen = text_now(composer, engine);
    let (view, put_back) = match action {
        "flip" => {
            let (view, put_back) = takes.flip(take, screen)?;
            (view, Some(put_back))
        }
        "keep" => takes.keep(take)?,
        "drop" => takes.drop_take(take)?,
        "drop part" => {
            let Some(part) = part else {
                return Err(String::from("drop part needs a part"));
            };
            takes.drop_part(take, part, screen)?
        }
        _ => return Err(format!("a take can't {action}")),
    };
    if let Some(put_back) = put_back {
        put_on_screen(open, composer, engine, &put_back, now)?;
    }
    Ok(view)
}

// The rail's buttons and keys on a take. Refused while a phrase plays,
// since its presses would land on the song put back.
#[tauri::command]
pub fn change_agent_take(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    phrases: State<'_, Phrases>,
    takes: State<'_, Mutex<Takes>>,
    take: u64,
    action: String,
    part: Option<String>,
) -> Result<TakeView, String> {
    if phrases.is_playing() {
        return Err(String::from("a phrase is playing: stop it first"));
    }
    let now = Instant::now();
    let mut open = open.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let mut takes = takes.lock().unwrap();
    change_take(&mut open, &mut composer, &mut engine, &mut takes, take, &action, part.as_deref(), now)
}

// The take open now, for right Command, Return and Esc.
#[tauri::command]
pub fn open_take(takes: State<'_, Mutex<Takes>>) -> Option<TakeView> {
    takes.lock().unwrap().open_now()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::harmony::{Key, key_of_name};
    use crate::presets::test_folder;
    use crate::song_session::{Folders, new_song};
    use crate::songs;

    fn starting(label: &str) -> (OpenSong, Composer, Engine) {
        let folders = Folders {
            songs: test_folder(&format!("{label}-songs")),
            sounds: test_folder(&format!("{label}-sounds")),
            settings: test_folder(&format!("{label}-settings")),
            renders: test_folder(&format!("{label}-renders")),
        };
        let mut open = OpenSong::new(Some(folders));
        let mut composer = Composer::new();
        let mut engine = Engine::new().unwrap();
        new_song(&mut open, &mut composer, &mut engine).unwrap();
        (open, composer, engine)
    }

    fn key(name: &str) -> Key {
        key_of_name(name).unwrap()
    }

    fn key_in_file(open: &OpenSong) -> Option<Key> {
        let folder = open.folders().unwrap().songs;
        songs::read(&folder, open.name(), &SoundProfile::new()).unwrap().key
    }

    // A take on a new song that picks A minor and sets the tempo to 140, as
    // an answer would: Sound and Tape parts.
    fn take_with_two_parts(open: &mut OpenSong, composer: &mut Composer, engine: &mut Engine, takes: &mut Takes) -> u64 {
        let number = start_take(takes, open, composer, engine);
        let mut song = song_now(composer, engine);
        song.key = Some(key("A minor"));
        song.tempo = 140;
        let name = String::from(open.name());
        put_back_song(open, composer, engine, &name, &text_of(&song), Instant::now()).unwrap();
        let view = takes.end(number, text_now(composer, engine)).unwrap().unwrap();
        assert_eq!(view.parts, vec![TAPE, SOUND]);
        number
    }

    fn act(
        open: &mut OpenSong,
        composer: &mut Composer,
        engine: &mut Engine,
        takes: &mut Takes,
        number: u64,
        action: &str,
        part: Option<&str>,
    ) -> Result<TakeView, String> {
        change_take(open, composer, engine, takes, number, action, part, Instant::now())
    }

    #[test]
    fn an_answer_that_changes_nothing_makes_no_take() {
        let (open, composer, mut engine) = starting("takes-nothing");
        let mut takes = Takes::default();
        let number = start_take(&mut takes, &open, &composer, &mut engine);
        assert_eq!(takes.end(number, text_now(&composer, &mut engine)).unwrap(), None);
        assert_eq!(takes.open_now(), None);
        assert!(takes.takes.is_empty());
    }

    #[test]
    fn a_take_names_the_parts_it_changed() {
        let (mut open, mut composer, mut engine) = starting("takes-parts");
        let mut takes = Takes::default();
        take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        let view = takes.open_now().unwrap();
        assert_eq!(view.state, TakeState::Open);
        assert!(view.showing_take);
    }

    #[test]
    fn dropping_a_take_puts_the_players_version_back_and_saves_it() {
        let (mut open, mut composer, mut engine) = starting("takes-drop");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop", None).unwrap();
        assert_eq!(view.state, TakeState::Dropped);
        assert_eq!(composer.key(), None);
        assert_eq!(key_in_file(&open), None);
        assert_eq!(takes.open_now(), None);
    }

    #[test]
    fn keeping_a_take_leaves_it_on_screen() {
        let (mut open, mut composer, mut engine) = starting("takes-keep");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "keep", None).unwrap();
        assert_eq!(view.state, TakeState::Kept);
        assert_eq!(composer.key(), Some(key("A minor")));
    }

    #[test]
    fn flipping_twice_comes_back_to_the_take() {
        let (mut open, mut composer, mut engine) = starting("takes-flip");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        assert!(!view.showing_take);
        assert_eq!(composer.key(), None);
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        assert!(view.showing_take);
        assert_eq!(composer.key(), Some(key("A minor")));
    }

    #[test]
    fn keeping_while_the_players_version_shows_puts_the_take_on() {
        let (mut open, mut composer, mut engine) = starting("takes-keep-flipped");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "keep", None).unwrap();
        assert!(view.showing_take);
        assert_eq!(composer.key(), Some(key("A minor")));
        assert_eq!(key_in_file(&open), Some(key("A minor")));
    }

    #[test]
    fn the_players_edits_on_their_own_version_stay_when_flipping_back() {
        let (mut open, mut composer, mut engine) = starting("takes-edits");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        composer.pick_key(key("D major"));
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop", None).unwrap();
        assert_eq!(composer.key(), Some(key("D major")));
    }

    #[test]
    fn the_players_edits_on_the_take_stay_when_flipping_back() {
        let (mut open, mut composer, mut engine) = starting("takes-take-edits");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        composer.pick_key(key("D major"));
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        assert_eq!(composer.key(), None);
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "flip", None).unwrap();
        assert_eq!(composer.key(), Some(key("D major")));
    }

    #[test]
    fn dropping_one_part_keeps_the_rest() {
        let (mut open, mut composer, mut engine) = starting("takes-part");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop part", Some(SOUND)).unwrap();
        assert_eq!(view.parts, vec![TAPE]);
        assert_eq!(view.state, TakeState::Open);
        assert_eq!(composer.key(), None);
        assert_eq!(song_now(&composer, &mut engine).tempo, 140);
    }

    #[test]
    fn dropping_the_last_part_drops_the_take() {
        let (mut open, mut composer, mut engine) = starting("takes-last-part");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop part", Some(SOUND)).unwrap();
        let view = act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop part", Some(TAPE)).unwrap();
        assert_eq!(view.state, TakeState::Dropped);
        assert_ne!(song_now(&composer, &mut engine).tempo, 140);
    }

    #[test]
    fn a_new_answer_keeps_the_open_take() {
        let (mut open, mut composer, mut engine) = starting("takes-new");
        let mut takes = Takes::default();
        take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        start_take(&mut takes, &open, &composer, &mut engine);
        assert_eq!(takes.open_now(), None);
    }

    #[test]
    fn wrong_calls_change_nothing() {
        let (mut open, mut composer, mut engine) = starting("takes-wrong");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        assert!(act(&mut open, &mut composer, &mut engine, &mut takes, number, "shout", None).is_err());
        assert!(act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop part", Some(MIX)).is_err());
        assert!(act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop part", None).is_err());
        assert!(act(&mut open, &mut composer, &mut engine, &mut takes, 99, "keep", None).is_err());
        assert!(takes.end(number, String::new()).is_err());
        assert_eq!(takes.open_now().unwrap().parts, vec![TAPE, SOUND]);
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "keep", None).unwrap();
        assert!(act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop", None).is_err());
        assert_eq!(composer.key(), Some(key("A minor")));
    }

    #[test]
    fn a_take_never_changes_learn() {
        let (mut open, mut composer, mut engine) = starting("takes-learn");
        let mut takes = Takes::default();
        let number = take_with_two_parts(&mut open, &mut composer, &mut engine, &mut takes);
        composer.path_mut().learn = true;
        act(&mut open, &mut composer, &mut engine, &mut takes, number, "drop", None).unwrap();
        assert!(composer.path().learn);
    }

    #[test]
    fn only_the_last_takes_are_remembered() {
        let mut takes = Takes::default();
        for _ in 0..KEPT_TAKES + 3 {
            takes.start("Song", String::new());
        }
        assert_eq!(takes.takes.len(), KEPT_TAKES);
        assert_eq!(takes.takes.front().unwrap().number, 4);
    }
}
