// Undo for the agent's replies (docs/design.md#the-agent-harness). Before
// each reply, Jello keeps the whole song as its file text. That reply's
// Undo puts the copy back and the song saves, so everything changed during
// the reply goes, the player's own changes too. Undoing a reply also takes
// back every reply after it, so their copies go with it. The last
// KEPT_REPLIES replies keep their copies, for as long as Jello runs.
//
// The replies are locked last, after the open song, Compose and the engine.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use tauri::State;

use crate::compose::Composer;
use crate::engine::Engine;
use crate::phrases::Phrases;
use crate::song_file::text_of;
use crate::song_session::{OpenSong, put_back_song, song_now};

// How many replies keep their copies.
const KEPT_REPLIES: usize = 20;

// The song as it was before a reply.
struct KeptSong {
    reply: u64,
    name: String,
    text: String,
}

// The replies started so far, and the copies kept before the last ones,
// oldest first.
#[derive(Default)]
pub struct Replies {
    started: u64,
    kept: VecDeque<KeptSong>,
}

impl Replies {
    // Keeps song `name`, whose file text is `text` now, from before a new
    // reply, and returns that reply's number, counted from 1. The oldest
    // copy goes once there are more than KEPT_REPLIES.
    fn start(&mut self, name: &str, text: String) -> u64 {
        self.started += 1;
        let kept = KeptSong {
            reply: self.started,
            name: String::from(name),
            text,
        };
        self.kept.push_back(kept);
        if self.kept.len() > KEPT_REPLIES {
            self.kept.pop_front();
        }
        self.started
    }

    // The copy kept before `reply`, if it is still kept.
    fn kept_before(&self, reply: u64) -> Option<&KeptSong> {
        self.kept.iter().find(|kept| kept.reply == reply)
    }

    // Forgets the copy kept before `reply` and every later one.
    fn forget_from(&mut self, reply: u64) {
        self.kept.retain(|kept| kept.reply < reply);
    }

    // The replies that can still be undone, oldest first.
    fn undoable(&self) -> Vec<u64> {
        let mut replies = Vec::new();
        for kept in &self.kept {
            replies.push(kept.reply);
        }
        replies
    }
}

// Keeps the open song from before a new reply, and returns the reply's
// number.
pub(crate) fn start_reply(replies: &mut Replies, open: &OpenSong, composer: &Composer, engine: &mut Engine) -> u64 {
    let text = text_of(&song_now(composer, engine));
    replies.start(open.name(), text)
}

// Puts back the song as it was before `reply`, at `now`, and returns the
// replies that can still be undone. Fails, keeping every copy, if the copy
// is gone or the song can't be put back.
fn undo(
    replies: &mut Replies,
    open: &mut OpenSong,
    composer: &mut Composer,
    engine: &mut Engine,
    reply: u64,
    now: Instant,
) -> Result<Vec<u64>, String> {
    let Some(kept) = replies.kept_before(reply) else {
        return Err(format!("reply {reply} can't be undone anymore"));
    };
    put_back_song(open, composer, engine, &kept.name, &kept.text, now)?;
    replies.forget_from(reply);
    Ok(replies.undoable())
}

// The panel calls this as the player sends the agent a message, before the
// reply starts. Returns the reply's number, for its Undo.
#[tauri::command]
pub fn agent_reply_starts(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    replies: State<'_, Mutex<Replies>>,
) -> u64 {
    let open = open.lock().unwrap();
    let composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    let mut replies = replies.lock().unwrap();
    start_reply(&mut replies, &open, &composer, &mut engine)
}

// A reply's Undo. Refused while a phrase plays, since its presses would
// land on the song put back. Returns the replies that can still be undone.
#[tauri::command]
pub fn undo_agent_reply(
    open: State<'_, Mutex<OpenSong>>,
    composer: State<'_, Mutex<Composer>>,
    engine: State<'_, Mutex<Engine>>,
    phrases: State<'_, Phrases>,
    replies: State<'_, Mutex<Replies>>,
    reply: u64,
) -> Result<Vec<u64>, String> {
    if phrases.is_playing() {
        return Err(String::from("a phrase is playing: stop it first"));
    }
    let now = Instant::now();
    let mut open = open.lock().unwrap();
    let mut composer = composer.lock().unwrap();
    let mut engine = engine.lock().unwrap();
    engine.start_audio()?;
    let mut replies = replies.lock().unwrap();
    undo(&mut replies, &mut open, &mut composer, &mut engine, reply, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    use crate::harmony::{Key, key_of_name};
    use crate::layer_notes;
    use crate::presets::test_folder;
    use crate::sections::{self, SectionEdit};
    use crate::tracks::Track;
    use crate::song_session::{Folders, new_song, rename_open_song};
    use crate::songs;
    use crate::sound_profile::SoundProfile;

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

    fn key(name: &str) -> Option<Key> {
        key_of_name(name)
    }

    fn key_in_file(open: &OpenSong, name: &str) -> Option<Key> {
        let folder = open.folders().unwrap().songs;
        songs::read(&folder, name, &SoundProfile::new()).unwrap().key
    }

    #[test]
    fn undo_puts_the_song_back_as_it_was_before_the_reply_and_saves_it() {
        let (mut open, mut composer, mut engine) = starting("undo-back");
        let mut replies = Replies::default();
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);
        assert_eq!(reply, 1);

        composer.pick_key(key("A minor").unwrap());
        undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now()).unwrap();

        assert_eq!(composer.key(), None);
        assert_eq!(key_in_file(&open, "Untitled 1"), None);
    }

    #[test]
    fn undo_puts_back_the_tracks_and_sections_too() {
        let (mut open, mut composer, mut engine) = starting("undo-tracks");
        let notes = layer_notes::notes_of(&json!({"notes": [{"note": "E4", "start": 0, "hold": 1}]})).unwrap();
        layer_notes::write_layer(&mut engine, None, None, None, &notes).unwrap();
        sections::edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        let mut replies = Replies::default();
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);

        composer.tracks_mut().add(&mut engine, Track::new_drums());
        sections::edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        assert_eq!(composer.tracks().len(), 2);
        assert_eq!(composer.sections().len(), 2);

        undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now()).unwrap();
        assert_eq!(composer.tracks().len(), 1);
        assert_eq!(composer.sections().len(), 1);
        let (_, layers) = engine.loop_to_keep().unwrap();
        assert_eq!(layers.len(), 1);
    }

    #[test]
    fn undo_restores_drum_shared_lfos_and_all_effect_connections() {
        use crate::engine::{EffectConnection, EQ_OUTPUT};
        use crate::modulation::{CONNECTION_COUNT, NO_CONNECTION, Source};
        let (mut open, mut composer, mut engine) = starting("undo-drum-routing");
        let track = composer.tracks_mut().add(&mut engine, Track::new_drums()).unwrap();
        assert!(composer.tracks_mut().pick(&mut engine, track));
        let mut lfo = composer.sound_profile().lfos()[0];
        lfo.per_note = false;
        lfo.hz = 2.0;
        composer.sound_profile_mut().set_lfo(&mut engine, 0, lfo);
        composer.sound_profile_mut().set_connection(&mut engine, 0, NO_CONNECTION);
        let link = EffectConnection { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: 1.0, one_way: false, scaled_by: Source::None };
        composer.mix_mut().tracks[track].links = vec![link; CONNECTION_COUNT];
        composer.send_picked_routing(&mut engine);

        let snapshot = song_now(&composer, &mut engine);
        let written = text_of(&snapshot);
        let read = crate::song_file::song_of(&written, &SoundProfile::new()).unwrap();
        assert_eq!(read.tracks[track].sound.lfos[0], lfo);
        assert_eq!(read.tracks[track].sound.connections, [NO_CONNECTION; CONNECTION_COUNT]);
        assert_eq!(read.mix.tracks[track].links.len(), CONNECTION_COUNT);

        let mut replies = Replies::default();
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);
        composer.mix_mut().tracks[track].links.clear();
        composer.sound_profile_mut().set_lfo(&mut engine, 0, crate::modulation::START_LFO);
        undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now()).unwrap();
        assert_eq!(composer.sound_profile().lfos()[0], lfo);
        assert_eq!(*composer.sound_profile().connections(), [NO_CONNECTION; CONNECTION_COUNT]);
        assert_eq!(composer.mix_now().tracks[track].links.len(), CONNECTION_COUNT);
        let saved = songs::read(&open.folders().unwrap().songs, open.name(), &SoundProfile::new()).unwrap();
        assert_eq!(saved.tracks[track].sound.lfos[0], lfo);
        assert_eq!(saved.mix.tracks[track].links.len(), CONNECTION_COUNT);
    }

    #[test]
    fn undo_restores_sparse_sound_connections_on_tracks_and_drums() {
        use crate::modulation::{CONNECTION_COUNT, NO_CONNECTION};
        let (mut open, mut composer, mut engine) = starting("undo-sparse-routing");
        let drums = composer.tracks_mut().add(&mut engine, Track::new_drums()).unwrap();
        for track in [0, drums] {
            composer.tracks_mut().pick(&mut engine, track);
            let connection = composer.sound_profile().connections()[0];
            composer.sound_profile_mut().set_connection(&mut engine, 0, NO_CONNECTION);
            composer.sound_profile_mut().set_connection(&mut engine, CONNECTION_COUNT - 1, connection);
        }
        let snapshot = song_now(&composer, &mut engine);
        let read = crate::song_file::song_of(&text_of(&snapshot), &SoundProfile::new()).unwrap();
        for track in [0, drums] {
            assert_eq!(read.tracks[track].sound.connections[0], NO_CONNECTION);
            assert_ne!(read.tracks[track].sound.connections[CONNECTION_COUNT - 1], NO_CONNECTION);
        }

        let mut replies = Replies::default();
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);
        composer.sound_profile_mut().set_connection(&mut engine, CONNECTION_COUNT - 1, NO_CONNECTION);
        undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now()).unwrap();
        for track in [0, drums] {
            let sound = composer.tracks().list()[track].sound_profile();
            assert_eq!(sound.connections()[0], NO_CONNECTION);
            assert_ne!(sound.connections()[CONNECTION_COUNT - 1], NO_CONNECTION);
        }
    }

    #[test]
    fn undoing_a_reply_takes_back_every_later_one_too() {
        let (mut open, mut composer, mut engine) = starting("undo-later");
        let mut replies = Replies::default();
        start_reply(&mut replies, &open, &composer, &mut engine);
        composer.pick_key(key("C major").unwrap());
        let second = start_reply(&mut replies, &open, &composer, &mut engine);
        composer.pick_key(key("A minor").unwrap());
        start_reply(&mut replies, &open, &composer, &mut engine);
        composer.pick_key(key("E minor").unwrap());

        let left = undo(&mut replies, &mut open, &mut composer, &mut engine, second, Instant::now()).unwrap();

        assert_eq!(composer.key(), key("C major"));
        assert_eq!(key_in_file(&open, "Untitled 1"), key("C major"));
        assert_eq!(left, vec![1]);
        let again = undo(&mut replies, &mut open, &mut composer, &mut engine, 3, Instant::now());
        assert_eq!(again, Err(String::from("reply 3 can't be undone anymore")));
    }

    #[test]
    fn only_the_last_twenty_replies_keep_their_copies() {
        let (mut open, mut composer, mut engine) = starting("undo-twenty");
        let mut replies = Replies::default();
        for _ in 0..21 {
            start_reply(&mut replies, &open, &composer, &mut engine);
        }

        let first = undo(&mut replies, &mut open, &mut composer, &mut engine, 1, Instant::now());
        assert_eq!(first, Err(String::from("reply 1 can't be undone anymore")));
        let left = undo(&mut replies, &mut open, &mut composer, &mut engine, 21, Instant::now()).unwrap();
        assert_eq!(left.len(), 19);
        assert_eq!(left[0], 2);
    }

    #[test]
    fn undo_reopens_the_song_open_before_the_reply() {
        let (mut open, mut composer, mut engine) = starting("undo-other");
        let mut replies = Replies::default();
        composer.pick_key(key("D major").unwrap());
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);

        new_song(&mut open, &mut composer, &mut engine).unwrap();
        rename_open_song(&mut open, &composer, &mut engine, "Night drive").unwrap();
        undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now()).unwrap();

        assert_eq!(open.name(), "Untitled 1");
        assert_eq!(composer.key(), key("D major"));
    }

    #[test]
    fn a_copy_that_can_t_be_put_back_is_kept() {
        let (mut open, mut composer, mut engine) = starting("undo-kept");
        let mut replies = Replies::default();
        let reply = start_reply(&mut replies, &open, &composer, &mut engine);
        replies.kept[0].text = String::from("not a song");

        let undone = undo(&mut replies, &mut open, &mut composer, &mut engine, reply, Instant::now());

        assert!(undone.is_err());
        assert_eq!(replies.undoable(), vec![1]);
        assert_eq!(key_in_file(&open, "Untitled 1"), None);
    }
}
