// The song's tracks (docs/design.md#tracks-and-sections). A track is an
// instrument: a sound of its own, either a whole sound profile or a drum
// kit with its pads, and a volume. A song has 1 to TRACK_COUNT of them, one
// picked: the keys play it and the Sound panel edits it. The drums track
// used last is kept, so right Option can jump to it and back.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::drums::{DRUM_PAD_COUNT, DrumPad, Drums};
use crate::engine::{Engine, FULL_VOLUME, Kind, OSCILLATOR_COUNT, TRACK_COUNT};
use crate::sound_file::Sound;
use crate::sound_profile::SoundProfile;

// A track's identity lasts only while this process holds it. It lets a
// temporary sound preview find its track after the list is reordered.
static NEXT_TRACK_ID: AtomicU64 = AtomicU64::new(1);

fn new_track_id() -> u64 {
    NEXT_TRACK_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone)]
pub struct Track {
    id: u64,
    // How its notes sound. A drums track keeps one too, unused.
    sound_profile: SoundProfile,
    // Its drum kit's instrument and how each pad sounds.
    drums: Drums,
    // True for a drums track: its letters play its kit's pads.
    is_drums: bool,
    // In percent, from 0 to FULL_VOLUME.
    volume: u32,
    // The name it was given on screen, or None to follow its sound.
    renamed: Option<String>,
    // The preset its sound was loaded from or saved as last
    // (docs/adr/0037-presets.md), which names it. None until a preset is
    // loaded or saved.
    preset: Option<String>,
    // Its sound profile as it was right then, if known. While the track
    // still sounds exactly like that, the Presets page lights the preset's
    // tile.
    preset_profile: Option<SoundProfile>,
}

impl Track {
    // An instrument track on a new song's sound, at full volume.
    pub fn new() -> Track {
        Track {
            id: new_track_id(),
            sound_profile: SoundProfile::new(),
            drums: Drums::new(),
            is_drums: false,
            volume: FULL_VOLUME,
            renamed: None,
            preset: None,
            preset_profile: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    // A drums track with no kit picked yet, at full volume.
    pub fn new_drums() -> Track {
        let mut track = Track::new();
        track.is_drums = true;
        track
    }

    // A track as a song file holds it, sent to `engine`'s picked track: an
    // instrument track on `sound`, or with `drums`, a drums track.
    // `preset` is the preset its sound came from, lit only once
    // remember_preset_if_same finds it still the same.
    pub fn of_song(
        engine: &mut Engine,
        sound: &Sound,
        drums: Option<&Drums>,
        volume: u32,
        preset: Option<&str>,
    ) -> Track {
        let mut track = Track::new();
        track.sound_profile.take_sound(engine, sound);
        if let Some(drums) = drums {
            track.drums = drums.clone();
            track.is_drums = true;
        }
        track.volume = volume.min(FULL_VOLUME);
        track.preset = preset.map(String::from);
        track
    }

    // Its name: the one it was given, or else the one its sound gives it.
    // `number` counts from 1.
    pub fn name(&self, number: usize) -> String {
        if let Some(name) = &self.renamed {
            return name.clone();
        }
        self.followed_name(number)
    }

    // The name its sound gives it: a drums track's kit, the preset its sound
    // came from, the first sample instrument it plays, or "Track 3".
    pub fn followed_name(&self, number: usize) -> String {
        if self.is_drums {
            if let Some(kit) = &self.drums.kit {
                return kit.clone();
            }
            return format!("Track {number}");
        }
        if let Some(preset) = &self.preset {
            return preset.clone();
        }
        let sound = self.sound_profile.sound();
        for index in 0..OSCILLATOR_COUNT {
            if sound.oscillators[index].kind != Kind::Sample {
                continue;
            }
            if let Some(instrument) = &sound.instruments[index] {
                return instrument.clone();
            }
        }
        format!("Track {number}")
    }

    // Gives it `name`, or with None or a blank name, lets its name follow
    // its sound again.
    pub fn rename(&mut self, name: Option<&str>) {
        self.renamed = None;
        if let Some(name) = name {
            let name = name.trim();
            if !name.is_empty() {
                self.renamed = Some(String::from(name));
            }
        }
    }

    // The name it was given, or None while its name follows its sound.
    #[cfg(test)]
    pub fn renamed(&self) -> Option<&str> {
        self.renamed.as_deref()
    }

    pub fn is_drums(&self) -> bool {
        self.is_drums
    }

    pub fn volume(&self) -> u32 {
        self.volume
    }

    pub fn sound_profile(&self) -> &SoundProfile {
        &self.sound_profile
    }

    pub fn sound_profile_mut(&mut self) -> &mut SoundProfile {
        &mut self.sound_profile
    }

    pub fn drums(&self) -> &Drums {
        &self.drums
    }

    // Sends its whole sound, its drums and its volume to `engine`'s picked
    // track, which must be this one.
    fn send_all(&self, engine: &mut Engine) {
        self.sound_profile.send_all(engine);
        self.send_all_but_sound(engine);
    }

    // Its kit, pads and volume, for a track whose sound was just sent.
    fn send_all_but_sound(&self, engine: &mut Engine) {
        engine.set_drum_kit(self.drums.kit.as_deref());
        for (pad, settings) in self.drums.pads.iter().enumerate() {
            engine.set_drum_pad(pad, *settings);
        }
        let track = engine.track();
        engine.set_track_volume(track, self.volume);
    }

    // Copies preset `name`'s `sound` into it, on `engine`'s picked track
    // too, and remembers it as the preset it sounds like.
    pub fn take_preset(&mut self, engine: &mut Engine, name: &str, sound: &Sound) {
        self.sound_profile.take_sound(engine, sound);
        self.saved_as_preset(name);
    }

    // Remembers that its sound was just saved as preset `name`.
    pub fn saved_as_preset(&mut self, name: &str) {
        self.preset = Some(String::from(name));
        self.preset_profile = Some(self.sound_profile.clone());
    }

    // Lights preset `name`'s tile if it sounds exactly like `sound`, the
    // preset's file as it is now. Opening a song does this.
    pub fn remember_preset_if_same(&mut self, name: &str, sound: &Sound) {
        if self.sound_profile.sound() == *sound {
            self.saved_as_preset(name);
        }
    }

    // The preset its sound came from, lit or not.
    pub fn preset(&self) -> Option<&str> {
        self.preset.as_deref()
    }

    // The preset it sounds exactly like, or None once its sound has changed
    // since loading or saving it, or if it never did either.
    pub fn lit_preset(&self) -> Option<&str> {
        let Some(profile) = &self.preset_profile else {
            return None;
        };
        if *profile == self.sound_profile {
            self.preset.as_deref()
        } else {
            None
        }
    }
}

pub struct Tracks {
    // From 1 to TRACK_COUNT of them, in order.
    list: Vec<Track>,
    // The picked one, from 0.
    picked: usize,
    // The drums track picked last, from 0, where right Option jumps.
    last_drums: Option<usize>,
    // The track right Option jumped from, where it jumps back.
    jumped_from: Option<usize>,
}

impl Tracks {
    // A new song's: one instrument track, picked.
    pub fn new() -> Tracks {
        Tracks {
            list: vec![Track::new()],
            picked: 0,
            last_drums: None,
            jumped_from: None,
        }
    }

    // `list` with track `picked` picked, as a song file holds them. No
    // tracks at all gives a new song's, and a pick past the last picks the
    // first.
    pub fn of_list(mut list: Vec<Track>, picked: usize) -> Tracks {
        list.truncate(TRACK_COUNT);
        if list.is_empty() {
            return Tracks::new();
        }
        let mut tracks = Tracks {
            list,
            picked: 0,
            last_drums: None,
            jumped_from: None,
        };
        if picked < tracks.list.len() {
            tracks.picked = picked;
        }
        if tracks.list[tracks.picked].is_drums {
            tracks.last_drums = Some(tracks.picked);
        }
        tracks
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn list(&self) -> &[Track] {
        &self.list
    }

    // Track `track`, from 0, or None past the last.
    #[cfg(test)]
    pub fn get(&self, track: usize) -> Option<&Track> {
        self.list.get(track)
    }

    pub fn get_mut(&mut self, track: usize) -> Option<&mut Track> {
        self.list.get_mut(track)
    }

    // The picked track's number, from 0.
    pub fn picked(&self) -> usize {
        self.picked
    }

    pub fn picked_track(&self) -> &Track {
        &self.list[self.picked]
    }

    pub fn picked_track_mut(&mut self) -> &mut Track {
        &mut self.list[self.picked]
    }

    // Sends every track's sound, drums and volume to `engine`, each to its
    // own track there, and picks the picked one there too.
    pub fn send_all(&self, engine: &mut Engine) {
        for (number, track) in self.list.iter().enumerate() {
            engine.make_room();
            engine.on_track(number, |engine| track.send_all(engine));
        }
        for number in self.list.len()..TRACK_COUNT {
            engine.on_track(number, let_go_of_instruments);
        }
        engine.set_track(self.picked);
    }

    // The same as send_all, for tracks whose sounds were just sent as they
    // were made.
    pub fn send_all_but_sounds(&self, engine: &mut Engine) {
        for (number, track) in self.list.iter().enumerate() {
            engine.make_room();
            engine.on_track(number, |engine| track.send_all_but_sound(engine));
        }
        for number in self.list.len()..TRACK_COUNT {
            engine.on_track(number, let_go_of_instruments);
        }
        engine.set_track(self.picked);
    }

    // Picks track `track`, from 0, on `engine` too. Returns false, picking
    // nothing, for a track past the last.
    pub fn pick(&mut self, engine: &mut Engine, track: usize) -> bool {
        if track >= self.list.len() {
            return false;
        }
        self.picked = track;
        self.jumped_from = None;
        if self.list[track].is_drums {
            self.last_drums = Some(track);
        }
        engine.set_track(track);
        true
    }

    // Picks track `track`, from 0, for a moment, on `engine` too, leaving
    // right Option's jump as it was: an agent's tool working on a track
    // that isn't picked picks it for the while. Returns false for a track
    // past the last.
    pub fn pick_for_a_while(&mut self, engine: &mut Engine, track: usize) -> bool {
        if track >= self.list.len() {
            return false;
        }
        self.picked = track;
        engine.set_track(track);
        true
    }

    // Adds `track` after the last and sends it to `engine`, without picking
    // it. Returns its number, from 0, or None with TRACK_COUNT already.
    pub fn add(&mut self, engine: &mut Engine, mut track: Track) -> Option<usize> {
        if self.list.len() == TRACK_COUNT {
            return None;
        }
        let number = self.list.len();
        track.id = new_track_id();
        engine.on_track(number, |engine| track.send_all(engine));
        self.list.push(track);
        Some(number)
    }

    // Right Option's jump: from an instrument track to the drums track
    // picked last (or the first one), making one after the last if there
    // is none, and from there back to the track it came from. Returns
    // false, changing nothing, when a drums track is needed and there is
    // no room for one, or from a drums track nothing jumped to.
    pub fn jump(&mut self, engine: &mut Engine) -> bool {
        if self.list[self.picked].is_drums {
            let Some(from) = self.jumped_from else {
                return false;
            };
            return self.pick(engine, from);
        }

        let mut target = None;
        if let Some(last) = self.last_drums {
            if last < self.list.len() && self.list[last].is_drums {
                target = Some(last);
            }
        }
        if target.is_none() {
            for (number, track) in self.list.iter().enumerate() {
                if track.is_drums {
                    target = Some(number);
                    break;
                }
            }
        }
        if target.is_none() {
            target = self.add(engine, Track::new_drums());
        }
        let Some(target) = target else {
            return false;
        };

        let from = self.picked;
        self.pick(engine, target);
        self.jumped_from = Some(from);
        true
    }

    // Removes track `track`, from 0, and sends the tracks after it to
    // `engine` one track down. The picked track stays picked; removing it
    // picks the one that takes its place, or the one before it. Returns
    // false, removing nothing, for the only track or one past the last.
    // The loop's layers are the caller's to move.
    pub fn remove(&mut self, engine: &mut Engine, track: usize) -> bool {
        if track >= self.list.len() {
            return false;
        }
        if self.list.len() == 1 {
            return false;
        }
        self.list.remove(track);

        if self.picked > track {
            self.picked -= 1;
        } else if self.picked == track && self.picked == self.list.len() {
            self.picked -= 1;
        }
        self.last_drums = place_after_removing(self.last_drums, track);
        self.jumped_from = place_after_removing(self.jumped_from, track);

        for number in track..self.list.len() {
            let moved = &self.list[number];
            engine.on_track(number, |engine| moved.send_all(engine));
        }
        let emptied = self.list.len();
        engine.on_track(emptied, let_go_of_instruments);
        engine.set_track(self.picked);
        true
    }

    // Sets track `track`'s volume to `percent`, on `engine` too, kept at
    // most FULL_VOLUME. A track past the last changes nothing.
    pub fn set_volume(&mut self, engine: &mut Engine, track: usize, percent: u32) {
        let Some(changed) = self.list.get_mut(track) else {
            return;
        };
        changed.volume = engine.set_track_volume(track, percent);
    }

    // Makes the picked track's drums play instrument `name`, on `engine`
    // too. A kit makes it a drums track; no kit makes it an instrument track.
    pub fn set_drum_kit(&mut self, engine: &mut Engine, name: Option<&str>) {
        let track = &mut self.list[self.picked];
        track.drums.kit = name.map(String::from);
        track.is_drums = name.is_some();
        engine.set_drum_kit(name);
        if track.is_drums {
            self.last_drums = Some(self.picked);
        }
    }

    // Sets how the picked track's pad `pad`, from 0, sounds, on `engine`
    // too, kept as the engine sent it. A pad past the last changes nothing.
    pub fn set_drum_pad(&mut self, engine: &mut Engine, pad: usize, settings: DrumPad) {
        if pad >= DRUM_PAD_COUNT {
            return;
        }
        let track = &mut self.list[self.picked];
        track.drums.pads[pad] = engine.set_drum_pad(pad, settings);
    }
}

// Lets `engine`'s picked track, one no track of the song uses, let go of
// the instruments and kit it was playing, so their samples can go.
fn let_go_of_instruments(engine: &mut Engine) {
    for index in 0..OSCILLATOR_COUNT {
        let (instrument, _) = engine.instrument(index);
        if instrument.is_some() {
            engine.set_instrument(index, None);
        }
    }
    let (kit, _) = engine.drum_kit();
    if kit.is_some() {
        engine.set_drum_kit(None);
    }
}

// Where a track numbered `place` is after track `removed` is taken out:
// one down if it came after, and None if it was the one removed.
fn place_after_removing(place: Option<usize>, removed: usize) -> Option<usize> {
    let place = place?;
    if place == removed {
        return None;
    }
    if place > removed {
        return Some(place - 1);
    }
    Some(place)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::key_map::LAPTOP_VELOCITY;

    const CHUNK: usize = 2048;

    fn saw_sound() -> Sound {
        let mut sound = SoundProfile::new().sound();
        sound.oscillators[0].kind = Kind::Saw;
        sound
    }

    #[test]
    fn a_new_song_has_one_instrument_track_picked() {
        let tracks = Tracks::new();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks.picked(), 0);
        assert!(!tracks.picked_track().is_drums());
        assert_eq!(tracks.picked_track().name(1), "Track 1");
        assert_eq!(tracks.picked_track().volume(), FULL_VOLUME);
    }

    #[test]
    fn a_track_s_name_follows_its_sound_until_it_is_renamed() {
        let mut engine = Engine::new().unwrap();
        let mut track = Track::new();
        assert_eq!(track.name(3), "Track 3");

        // A sample instrument names it...
        let mut sound = SoundProfile::new().sound();
        sound.oscillators[1].kind = Kind::Sample;
        sound.instruments[1] = Some(String::from("Grand"));
        track.sound_profile.take_sound(&mut engine, &sound);
        assert_eq!(track.name(3), "Grand");
        // ...and a preset more so.
        track.take_preset(&mut engine, "Warm", &sound);
        assert_eq!(track.name(3), "Warm");

        track.rename(Some("  Lead  "));
        assert_eq!(track.name(3), "Lead");
        track.rename(Some(" "));
        assert_eq!(track.name(3), "Warm");

        // A drums track is named for its kit.
        let mut drums = Track::new_drums();
        assert_eq!(drums.name(2), "Track 2");
        drums.drums.kit = Some(String::from("Black Pearl"));
        assert_eq!(drums.name(2), "Black Pearl");
    }

    #[test]
    fn picking_a_track_picks_it_on_the_engine_too() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        assert!(!tracks.pick(&mut engine, 1));
        assert_eq!(tracks.add(&mut engine, Track::new()), Some(1));
        assert_eq!(tracks.picked(), 0);
        assert!(tracks.pick(&mut engine, 1));
        assert_eq!(engine.track(), 1);
    }

    #[test]
    fn there_are_never_more_than_9_tracks() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        for _ in 1..TRACK_COUNT {
            assert!(tracks.add(&mut engine, Track::new()).is_some());
        }
        assert_eq!(tracks.add(&mut engine, Track::new()), None);
        assert_eq!(tracks.len(), TRACK_COUNT);
    }

    #[test]
    fn each_track_plays_its_own_sound() {
        // Track 2 on a saw, picked, sounds like an engine on a saw alone.
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        let mut saw = Track::new();
        saw.take_preset(&mut engine, "Saw", &saw_sound());
        tracks.add(&mut engine, saw);
        tracks.send_all(&mut engine);
        tracks.pick(&mut engine, 1);

        let mut alone = Engine::new().unwrap();
        alone.set_first_kind(Kind::Saw);
        engine.note_on(69, LAPTOP_VELOCITY);
        alone.note_on(69, LAPTOP_VELOCITY);
        assert!(engine.render(CHUNK) == alone.render(CHUNK));
    }

    #[test]
    fn right_option_jumps_to_the_drums_track_and_back() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        tracks.add(&mut engine, Track::new());
        tracks.pick(&mut engine, 1);

        // With no drums track, one is made after the last.
        assert!(tracks.jump(&mut engine));
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks.picked(), 2);
        assert!(tracks.picked_track().is_drums());
        assert!(tracks.jump(&mut engine));
        assert_eq!(tracks.picked(), 1);

        // Then it goes to that one again, from any track.
        tracks.pick(&mut engine, 0);
        assert!(tracks.jump(&mut engine));
        assert_eq!(tracks.picked(), 2);
        assert_eq!(tracks.len(), 3);

        // Picked by number, a drums track has nowhere to jump back to.
        tracks.pick(&mut engine, 2);
        assert!(!tracks.jump(&mut engine));
        assert_eq!(tracks.picked(), 2);
    }

    #[test]
    fn right_option_goes_to_the_drums_track_picked_last() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        tracks.add(&mut engine, Track::new_drums());
        tracks.add(&mut engine, Track::new_drums());
        tracks.pick(&mut engine, 2);
        tracks.pick(&mut engine, 0);
        tracks.jump(&mut engine);
        assert_eq!(tracks.picked(), 2);
    }

    #[test]
    fn with_9_tracks_and_no_drums_right_option_does_nothing() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        for _ in 1..TRACK_COUNT {
            tracks.add(&mut engine, Track::new());
        }
        assert!(!tracks.jump(&mut engine));
        assert_eq!(tracks.picked(), 0);
    }

    #[test]
    fn a_kit_makes_a_drums_track_and_no_kit_an_instrument_track() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        tracks.set_drum_kit(&mut engine, Some("Kit"));
        assert!(tracks.picked_track().is_drums());
        assert_eq!(tracks.picked_track().name(1), "Kit");
        tracks.set_drum_kit(&mut engine, None);
        assert!(!tracks.picked_track().is_drums());
    }

    #[test]
    fn removing_a_track_moves_the_later_ones_down() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        let mut saw = Track::new();
        saw.rename(Some("Saw"));
        saw.take_preset(&mut engine, "Saw", &saw_sound());
        tracks.add(&mut engine, Track::new());
        tracks.add(&mut engine, saw);
        tracks.pick(&mut engine, 2);

        assert!(tracks.remove(&mut engine, 1));
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks.picked(), 1);
        assert_eq!(tracks.picked_track().name(2), "Saw");
        assert_eq!(engine.track(), 1);

        // Track 2, now the saw, plays a saw on the engine's track 2.
        let mut alone = Engine::new().unwrap();
        alone.set_first_kind(Kind::Saw);
        engine.note_on(69, LAPTOP_VELOCITY);
        alone.note_on(69, LAPTOP_VELOCITY);
        assert!(engine.render(CHUNK) == alone.render(CHUNK));

        // Removing the picked last track picks the one before it.
        assert!(tracks.remove(&mut engine, 1));
        assert_eq!(tracks.picked(), 0);
        // The only track stays.
        assert!(!tracks.remove(&mut engine, 0));
        assert!(!tracks.remove(&mut engine, 4));
    }

    #[test]
    fn a_track_s_volume_is_kept_at_most_full() {
        let mut engine = Engine::new().unwrap();
        let mut tracks = Tracks::new();
        tracks.set_volume(&mut engine, 0, 40);
        assert_eq!(tracks.picked_track().volume(), 40);
        tracks.set_volume(&mut engine, 0, 400);
        assert_eq!(tracks.picked_track().volume(), FULL_VOLUME);
        tracks.set_volume(&mut engine, 5, 10);
    }

    #[test]
    fn a_song_file_s_pick_past_the_last_picks_the_first() {
        let tracks = Tracks::of_list(vec![Track::new(), Track::new_drums()], 7);
        assert_eq!(tracks.picked(), 0);
        let tracks = Tracks::of_list(vec![Track::new(), Track::new_drums()], 1);
        assert_eq!(tracks.picked(), 1);
        assert_eq!(Tracks::of_list(Vec::new(), 0).len(), 1);
    }
}
