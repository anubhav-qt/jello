// The arrangement played (docs/design.md#tracks-and-sections): the song's
// sections in order, each its layers playing together for its bars, turned
// into timed messages for the engine's sequencer. The player keeps the
// sequencer about 2 seconds ahead of the sound, a section at a time, so a
// song of any length never needs all its notes at once. While it plays the
// loop is put away, since the looper plays on the same layer slots, and it
// comes back paused once the arrangement stops.
use std::collections::VecDeque;
use std::time::Instant;

use crate::automation::{Part, own_part, part_at, send_part};
use crate::compose::Composer;
use crate::engine::{Engine, FASTEST_TEMPO, LooperState, SLOWEST_TEMPO, VIBRATO_KEY_NOTE};
use crate::key_map::LAPTOP_VELOCITY;
use crate::mix::Mix;
use crate::mix_automation::{MixPart, is_track_mix_value, mix_part_at, own_mix_part, send_mix_part};
use crate::song_file::{Automation, Song, SongLayer, SongLoop, SongSection, TEMPO_WORD};
use crate::sound_file::Sound;
use crate::song_session::{give_back_practice_loop, loop_now, open_song_loop};

// How far ahead of the sound the sequencer is kept, in seconds.
const AHEAD_SECONDS: u64 = 2;
// How long the last notes ring after the arrangement's end before the loop
// comes back, in seconds.
const TAIL_SECONDS: u64 = 1;
// How often a ramped tempo steps, in milliseconds.
const STEP_MS: u64 = 10;
// The most notes one section may play, counting each repeat of its layers.
const MOST_SECTION_NOTES: usize = 65_536;
// The highest note a key shift may reach; past it a note is left out.
const HIGHEST_NOTE: i32 = 127;
// Beats in a bar.
const BAR_BEATS: f64 = 4.0;

// One message for the sequencer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    // A layer's note on track `track`'s layer slot `slot`.
    NoteOn { track: usize, slot: usize, note: u8 },
    NoteOff { track: usize, slot: usize, note: u8 },
    // How loud a layer slot plays, in percent.
    SlotVolume { track: usize, slot: usize, percent: u32 },
    // The beat's tempo.
    Tempo(u32),
    // A part of track `track`'s sound, or its volume, as automation moves it.
    Part { track: usize, part: Part },
    // A value of the mix, as automation moves it.
    Mix(MixPart),
}

// A track as the arrangement plays it: drums never shift key, and each
// value automation moves goes back to the track's own after the section.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackSound {
    pub drums: bool,
    pub sound: Sound,
    pub volume: u32,
}

// A message, due `sample` samples after the arrangement starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub sample: u64,
    pub message: Message,
}

// How a section's beats become seconds: its tempo, or a tempo moving in a
// straight line across its beats, from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Clock {
    from: f64,
    to: f64,
    beats: f64,
}

impl Clock {
    // `section`'s clock: its tempo, or its tempo automation if it has one.
    fn of(section: &SongSection) -> Clock {
        let mut from = section.tempo as f64;
        let mut to = from;
        for moved in &section.automation {
            if is_tempo(moved) {
                from = moved.from;
                to = moved.to;
                break;
            }
        }
        Clock {
            from: from.clamp(SLOWEST_TEMPO as f64, FASTEST_TEMPO as f64),
            to: to.clamp(SLOWEST_TEMPO as f64, FASTEST_TEMPO as f64),
            beats: section.bars as f64 * BAR_BEATS,
        }
    }

    // The seconds from the section's start to beat `beat`. While the tempo
    // moves, each beat takes 60 / tempo seconds at the tempo it is at.
    fn seconds_at(&self, beat: f64) -> f64 {
        if self.from == self.to {
            return beat * 60.0 / self.from;
        }
        let change = self.to - self.from;
        let tempo = self.from + change * beat / self.beats;
        60.0 * self.beats / change * (tempo / self.from).ln()
    }

    // The tempo `seconds` into the section.
    fn tempo_after(&self, seconds: f64) -> f64 {
        if self.from == self.to {
            return self.from;
        }
        let change = self.to - self.from;
        self.from * (seconds * change / (60.0 * self.beats)).exp()
    }
}

// One use of a section in the arrangement: its place among the sections,
// and when it starts and how long it lasts, in samples.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Use {
    section: usize,
    start: u64,
    length: u64,
}

// The arrangement as it plays.
pub struct Player {
    sections: Vec<SongSection>,
    // The loop put away while the arrangement plays. Its layers are the ones
    // the sections play.
    looped: SongLoop,
    tracks: Vec<TrackSound>,
    // The song's mix, which automation moves and puts back.
    mix: Mix,
    sample_rate: u32,
    uses: Vec<Use>,
    // The next use whose messages are not planned yet.
    next_use: usize,
    // Planned messages not sent to the engine yet, in the order they are due.
    waiting: VecDeque<Timed>,
    // When the last section ends, in samples.
    end: u64,
}

impl Player {
    // The player for `arrangement`, places in `sections`, playing `looped`'s
    // layers on `tracks`. Fails if there is nothing to play, a section has
    // too many notes, or its automation moves a value it can't.
    pub fn new(
        sections: &[SongSection],
        arrangement: &[usize],
        looped: SongLoop,
        tracks: Vec<TrackSound>,
        mix: Mix,
        sample_rate: u32,
    ) -> Result<Player, String> {
        if arrangement.is_empty() {
            return Err(String::from("there are no sections to play"));
        }
        let mut uses = Vec::new();
        let mut start = 0;
        for &place in arrangement {
            let Some(section) = sections.get(place) else {
                continue;
            };
            if note_count(section, &looped) > MOST_SECTION_NOTES {
                return Err(format!(
                    "{} plays more than {} notes; use fewer or longer layers",
                    section.name, MOST_SECTION_NOTES
                ));
            }
            if let Err(problem) = automation_problem(section, &tracks, &mix) {
                return Err(format!("{}: {}", section.name, problem));
            }
            let clock = Clock::of(section);
            let length = samples_of_seconds(clock.seconds_at(clock.beats), sample_rate);
            uses.push(Use {
                section: place,
                start,
                length,
            });
            start += length;
        }
        if uses.is_empty() {
            return Err(String::from("there are no sections to play"));
        }
        Ok(Player {
            sections: sections.to_vec(),
            looped,
            tracks,
            mix,
            sample_rate,
            uses,
            next_use: 0,
            waiting: VecDeque::new(),
            end: start,
        })
    }

    // The loop put away while the arrangement plays.
    pub fn looped(&self) -> &SongLoop {
        &self.looped
    }

    // When the last section ends, in samples.
    pub fn end(&self) -> u64 {
        self.end
    }

    // The place in the arrangement of the section playing at `position`, in
    // samples, or None past the end.
    pub fn use_at(&self, position: u64) -> Option<usize> {
        for (place, used) in self.uses.iter().enumerate() {
            if position < used.start + used.length {
                return Some(place);
            }
        }
        None
    }

    // True once the arrangement, and a moment after it, has played by
    // `position`.
    pub fn is_done(&self, position: u64) -> bool {
        position >= self.end + TAIL_SECONDS * self.sample_rate as u64
    }

    // The messages due by `until`, in order, planning sections as they are
    // needed, at most `room` of them.
    pub fn take_due(&mut self, until: u64, room: usize) -> Vec<Timed> {
        let mut due = Vec::new();
        while due.len() < room {
            if self.waiting.is_empty() {
                if self.next_use == self.uses.len() {
                    break;
                }
                if self.uses[self.next_use].start > until {
                    break;
                }
                self.plan_next_use();
                continue;
            }
            let Some(next) = self.waiting.front() else {
                break;
            };
            if next.sample > until {
                break;
            }
            if let Some(next) = self.waiting.pop_front() {
                due.push(next);
            }
        }
        due
    }

    // Sends the engine the messages due within AHEAD_SECONDS of `position`,
    // as many as fit.
    pub fn feed(&mut self, engine: &mut Engine, position: u64) {
        let until = position + AHEAD_SECONDS * self.sample_rate as u64;
        let due = self.take_due(until, engine.sequence_room());
        for timed in due {
            send(engine, timed);
        }
    }

    // Puts every value the sections' automation moves back to its own, for
    // an arrangement stopped before its sections could. `tracks` and `mix`
    // are the song's as they are now.
    fn put_back_moved(&self, engine: &mut Engine, tracks: &[TrackSound], mix: &Mix) {
        for section in &self.sections {
            for moved in &section.automation {
                if is_tempo(moved) {
                    continue;
                }
                if moves_mix(moved) {
                    if let Ok(part) = mix_part_at(moved.track, &moved.value, moved.from, mix) {
                        send_mix_part(engine, own_mix_part(&part, mix));
                    }
                    continue;
                }
                let Some(track) = moved.track else {
                    continue;
                };
                let Some(own) = tracks.get(track) else {
                    continue;
                };
                if let Ok(part) = part_at(&moved.value, moved.from, &own.sound) {
                    send_part(engine, track, own_part(&part, &own.sound, own.volume));
                }
            }
        }
    }

    fn plan_next_use(&mut self) {
        let used = self.uses[self.next_use];
        self.next_use += 1;
        let section = &self.sections[used.section];
        let messages = section_messages(section, used.start, &self.looped, &self.tracks, &self.mix, self.sample_rate);
        self.waiting.extend(messages);
    }
}

// Sends `timed` to the engine's sequencer.
fn send(engine: &mut Engine, timed: Timed) {
    engine.time_next(timed.sample);
    match timed.message {
        Message::NoteOn { track, slot, note } => engine.layer_note_on(track, slot, note, LAPTOP_VELOCITY),
        Message::NoteOff { track, slot, note } => engine.layer_note_off(track, slot, note),
        Message::SlotVolume { track, slot, percent } => engine.set_slot_volume(track, slot, percent),
        Message::Tempo(tempo) => engine.send_tempo(tempo),
        Message::Part { track, part } => send_part(engine, track, part),
        Message::Mix(part) => send_mix_part(engine, part),
    }
    engine.send_now();
}

// `section`'s messages, starting `start` samples into the arrangement, in
// the order they are due: its tempo and its layers' volumes as it starts,
// the steps of a ramped tempo and of its automation, and its layers' notes,
// each layer repeating on its own length from the section's start, shifted
// by its key unless on drums. Notes still sounding at its end let go there,
// and each value it moved goes back to its track's own. At the same sample,
// notes let go before others start, so a repeated note plays again.
pub fn section_messages(
    section: &SongSection,
    start: u64,
    looped: &SongLoop,
    tracks: &[TrackSound],
    mix: &Mix,
    sample_rate: u32,
) -> Vec<Timed> {
    let clock = Clock::of(section);
    let sample_at = |beat: f64| start + samples_of_seconds(clock.seconds_at(beat), sample_rate);
    let mut messages = Vec::new();

    let mut tempo = clock.from.round() as u32;
    messages.push(Timed {
        sample: start,
        message: Message::Tempo(tempo),
    });
    if clock.from != clock.to {
        let length = clock.seconds_at(clock.beats);
        let mut step = 1;
        loop {
            let seconds = (step * STEP_MS) as f64 / 1000.0;
            if seconds >= length {
                break;
            }
            let now = clock.tempo_after(seconds).round() as u32;
            if now != tempo {
                tempo = now;
                messages.push(Timed {
                    sample: start + samples_of_seconds(seconds, sample_rate),
                    message: Message::Tempo(tempo),
                });
            }
            step += 1;
        }
    }

    let length = clock.seconds_at(clock.beats);
    for moved in &section.automation {
        if is_tempo(moved) {
            continue;
        }
        if moves_mix(moved) {
            messages.extend(mix_messages(moved, start, length, mix, sample_rate));
            continue;
        }
        let Some(track) = moved.track else {
            continue;
        };
        let Some(own) = tracks.get(track) else {
            continue;
        };
        let mut last: Option<Part> = None;
        let mut step = 0;
        loop {
            let seconds = (step * STEP_MS) as f64 / 1000.0;
            if seconds >= length {
                break;
            }
            let value = moved.from + (moved.to - moved.from) * seconds / length;
            step += 1;
            let Ok(part) = part_at(&moved.value, value, &own.sound) else {
                continue;
            };
            if last == Some(part) {
                continue;
            }
            last = Some(part);
            messages.push(Timed {
                sample: start + samples_of_seconds(seconds, sample_rate),
                message: Message::Part { track, part },
            });
        }
        if let Some(part) = last {
            messages.push(Timed {
                sample: start + samples_of_seconds(length, sample_rate),
                message: Message::Part {
                    track,
                    part: own_part(&part, &own.sound, own.volume),
                },
            });
        }
    }

    for used in &section.layers {
        let Some(layer) = layer_of(looped, used.track, used.layer) else {
            continue;
        };
        let track = used.track;
        let slot = used.layer;
        messages.push(Timed {
            sample: start,
            message: Message::SlotVolume {
                track,
                slot,
                percent: layer.volume,
            },
        });

        let mut shift = section.key_shift;
        if let Some(own) = tracks.get(track) {
            if own.drums {
                shift = 0;
            }
        }
        let period = period_beats(layer, looped);
        if period <= 0.0 {
            continue;
        }
        let mut base = 0.0;
        while base < clock.beats {
            for played in &layer.notes {
                if played.start >= period {
                    continue;
                }
                let on = base + played.start;
                if on >= clock.beats {
                    continue;
                }
                let off = (base + played.end.min(period)).min(clock.beats);
                let Some(note) = shifted(played.note, shift) else {
                    continue;
                };
                let on_sample = sample_at(on);
                let off_sample = sample_at(off).max(on_sample + 1);
                messages.push(Timed {
                    sample: on_sample,
                    message: Message::NoteOn { track, slot, note },
                });
                messages.push(Timed {
                    sample: off_sample,
                    message: Message::NoteOff { track, slot, note },
                });
            }
            base += period;
        }
    }

    messages.sort_by_key(|timed| (timed.sample, order_of(&timed.message)));
    messages
}

// True for the tempo's automation, which the section's clock plays.
fn is_tempo(moved: &Automation) -> bool {
    if moved.track.is_some() {
        return false;
    }
    moved.value == TEMPO_WORD
}

// True for automation of the mix: the song's own values, with no track, or
// a track's pan, sends and effects.
fn moves_mix(moved: &Automation) -> bool {
    if moved.track.is_none() {
        return true;
    }
    is_track_mix_value(&moved.value)
}

// The steps of `moved`, a value of `mix`, across a section `length` seconds
// long starting `start` samples in, then the step back to the mix's own.
fn mix_messages(moved: &Automation, start: u64, length: f64, mix: &Mix, sample_rate: u32) -> Vec<Timed> {
    let mut messages = Vec::new();
    let mut last: Option<MixPart> = None;
    let mut step = 0;
    loop {
        let seconds = (step * STEP_MS) as f64 / 1000.0;
        if seconds >= length {
            break;
        }
        let value = moved.from + (moved.to - moved.from) * seconds / length;
        step += 1;
        let Ok(part) = mix_part_at(moved.track, &moved.value, value, mix) else {
            continue;
        };
        if last == Some(part) {
            continue;
        }
        last = Some(part);
        messages.push(Timed {
            sample: start + samples_of_seconds(seconds, sample_rate),
            message: Message::Mix(part),
        });
    }
    if let Some(part) = last {
        messages.push(Timed {
            sample: start + samples_of_seconds(length, sample_rate),
            message: Message::Mix(own_mix_part(&part, mix)),
        });
    }
    messages
}

// Which comes first among messages due on the same sample.
fn order_of(message: &Message) -> u8 {
    match message {
        Message::NoteOff { .. } => 0,
        Message::Tempo(_) => 1,
        Message::SlotVolume { .. } => 1,
        Message::Part { .. } => 1,
        Message::Mix(_) => 1,
        Message::NoteOn { .. } => 2,
    }
}

// `note` moved `shift` half steps, or None if that leaves the notes there
// are. The vibrato and glide keys and drum pads never move.
pub(crate) fn shifted(note: u8, shift: i32) -> Option<u8> {
    if note >= VIBRATO_KEY_NOTE {
        return Some(note);
    }
    let moved = note as i32 + shift;
    if moved < 0 {
        return None;
    }
    if moved > HIGHEST_NOTE {
        return None;
    }
    Some(moved as u8)
}

// Layer `layer` of track `track`, both from 0, in the order the track
// recorded them, or None if there is no such layer.
pub fn layer_of(looped: &SongLoop, track: usize, layer: usize) -> Option<&SongLayer> {
    looped.layers.iter().filter(|each| each.track == track).nth(layer)
}

// How many beats `layer` takes to repeat: its own bars, or the loop's.
pub fn period_beats(layer: &SongLayer, looped: &SongLoop) -> f64 {
    if layer.bars > 0 {
        return layer.bars as f64 * BAR_BEATS;
    }
    looped.bars as f64 * BAR_BEATS
}

// Why `section`'s automation can't play on `tracks`, if it can't: a track
// it doesn't have, or a value it can't move to its start or end.
fn automation_problem(section: &SongSection, tracks: &[TrackSound], mix: &Mix) -> Result<(), String> {
    for moved in &section.automation {
        if is_tempo(moved) {
            continue;
        }
        if moves_mix(moved) {
            mix_part_at(moved.track, &moved.value, moved.from, mix)?;
            mix_part_at(moved.track, &moved.value, moved.to, mix)?;
            continue;
        }
        let Some(track) = moved.track else {
            continue;
        };
        let Some(own) = tracks.get(track) else {
            return Err(format!("there is no track {}", track + 1));
        };
        part_at(&moved.value, moved.from, &own.sound)?;
        part_at(&moved.value, moved.to, &own.sound)?;
    }
    Ok(())
}

// How many notes `section` plays, counting each repeat of its layers.
fn note_count(section: &SongSection, looped: &SongLoop) -> usize {
    let beats = section.bars as f64 * BAR_BEATS;
    let mut count = 0;
    for used in &section.layers {
        let Some(layer) = layer_of(looped, used.track, used.layer) else {
            continue;
        };
        let period = period_beats(layer, looped);
        if period <= 0.0 {
            continue;
        }
        let repeats = (beats / period).ceil() as usize;
        count += layer.notes.len() * repeats;
    }
    count
}

fn samples_of_seconds(seconds: f64, sample_rate: u32) -> u64 {
    (seconds * sample_rate as f64).round() as u64
}

// Plays the song's arrangement from its start, putting the loop away. Fails,
// changing nothing, while the looper records, or with no loop or no
// sections to play.
pub fn play(composer: &mut Composer, engine: &mut Engine) -> Result<(), String> {
    stop(composer, engine);
    let state = engine.looper_status().state;
    let recording = match state {
        LooperState::CountingIn => true,
        LooperState::Recording => true,
        LooperState::Layering => true,
        _ => false,
    };
    if recording {
        return Err(String::from("finish the recording before playing the sections"));
    }
    let Some(looped) = loop_now(engine) else {
        return Err(String::from("there is no loop: sections play its layers"));
    };
    let mut player = Player::new(
        composer.sections(),
        composer.arrangement(),
        looped,
        track_sounds(composer),
        composer.mix_now(),
        engine.sample_rate(),
    )?;

    engine.stop_song();
    engine.stop_sequence();
    engine.open_loop(composer.tempo(), 0, &[], LAPTOP_VELOCITY)?;
    player.feed(engine, 0);
    engine.play_sequence();
    composer.set_arranging(Some(player));
    Ok(())
}

// The song's tracks as the arrangement plays them.
pub fn track_sounds(composer: &Composer) -> Vec<TrackSound> {
    let mut tracks = Vec::new();
    for track in composer.tracks().list() {
        tracks.push(TrackSound {
            drums: track.is_drums(),
            sound: track.sound_profile().sound(),
            volume: track.volume(),
        });
    }
    tracks
}

// Stops the arrangement, if it plays, and opens the loop again, paused. A
// section's practice puts the loop away too, so its loop comes back here
// as well: whatever calls this needs the loop.
pub fn stop(composer: &mut Composer, engine: &mut Engine) {
    give_back_practice_loop(composer, engine);
    let Some(player) = composer.set_arranging(None) else {
        return;
    };
    engine.stop_sequence();
    let tracks = track_sounds(composer);
    player.put_back_moved(engine, &tracks, &composer.mix_now());
    let _ = open_song_loop(engine, composer.tempo(), Some(player.looped()));
}

// Keeps the arrangement playing: sends what is due soon, and stops it once
// it has played. Returns true if it stopped just now.
pub fn keep_playing(composer: &mut Composer, engine: &mut Engine) -> bool {
    let position = engine.sequence_position();
    let Some(player) = composer.arranging_mut() else {
        return false;
    };
    let Some(position) = position else {
        return false;
    };
    if player.is_done(position) {
        stop(composer, engine);
        return true;
    }
    player.feed(engine, position);
    false
}

// How much a render of the sections renders at a time, in frames.
const RENDER_CHUNK: usize = 512;
// The longest render of the sections, in seconds.
const RENDER_LONGEST_SECONDS: u64 = 600;

// `song`'s sections in order, rendered on a new engine with no speakers, as
// left, right, left, right, and the sample rate. The sequencer is fed between
// chunks just as it is live, so the same song always gives the same sound.
// Fails with no loop or sections, or sections longer than 10 minutes.
pub fn render(song: &Song) -> Result<(Vec<f32>, u32), String> {
    let Some(looped) = &song.looped else {
        return Err(String::from("there is no loop: sections play its layers"));
    };
    let mut engine = Engine::for_render()?;
    let sample_rate = engine.sample_rate();
    let composer = Composer::opened(&mut engine, song, Instant::now());
    if engine.dropped_messages() > 0 {
        return Err(String::from("the engine could not take all of the song"));
    }
    engine.settle_volumes();
    let mut player = Player::new(
        &song.sections,
        &song.arrangement,
        looped.clone(),
        track_sounds(&composer),
        composer.mix_now(),
        sample_rate,
    )?;
    let longest = RENDER_LONGEST_SECONDS * sample_rate as u64;
    if player.end() > longest {
        return Err(format!(
            "the sections last {} minutes, and a render is at most {}",
            player.end() / sample_rate as u64 / 60 + 1,
            RENDER_LONGEST_SECONDS / 60
        ));
    }

    let frames = player.end() + TAIL_SECONDS * sample_rate as u64;
    player.feed(&mut engine, 0);
    engine.play_sequence();
    engine.trim_lookahead();
    let mut samples = Vec::new();
    let mut rendered: u64 = 0;
    while rendered < frames {
        let left = (frames - rendered) as usize;
        let chunk = left.min(RENDER_CHUNK);
        samples.extend(engine.render(chunk));
        rendered += chunk as u64;
        player.feed(&mut engine, rendered);
    }
    Ok((samples, sample_rate))
}

// The place in the arrangement of the section playing now, or None while
// the arrangement doesn't play.
pub fn playing_now(composer: &Composer, engine: &Engine) -> Option<usize> {
    let player = composer.arranging()?;
    let position = engine.sequence_position().unwrap_or(0);
    player.use_at(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::START_TEMPO;
    use crate::song_file::{Automation, BeatNote, SectionLayer};
    use crate::song_session::{open_song_loop, song_now};

    use crate::sound_profile::SoundProfile;

    const RATE: u32 = 48_000;

    fn track(drums: bool) -> TrackSound {
        TrackSound {
            drums,
            sound: SoundProfile::new().sound(),
            volume: 70,
        }
    }

    fn one_track() -> Vec<TrackSound> {
        vec![track(false)]
    }

    fn with_drums() -> Vec<TrackSound> {
        vec![track(false), track(true)]
    }
    // A bar at 120 beats a minute: 4 beats of half a second.
    const BAR_AT_120: u64 = 96_000;

    fn layer(track: usize, bars: u32, notes: &[(u8, f64, f64)]) -> SongLayer {
        let mut beat_notes = Vec::new();
        for &(note, start, end) in notes {
            beat_notes.push(BeatNote { note, start, end });
        }
        SongLayer {
            track,
            bars,
            volume: 80,
            playing: true,
            notes: beat_notes,
        }
    }

    fn section(name: &str, bars: u32, tempo: u32, layers: &[(usize, usize)]) -> SongSection {
        let mut section_layers = Vec::new();
        for &(track, layer) in layers {
            section_layers.push(SectionLayer { track, layer });
        }
        SongSection {
            keys: None,
            name: String::from(name),
            bars,
            tempo,
            key_shift: 0,
            layers: section_layers,
            automation: Vec::new(),
        }
    }

    fn tempo_ramp(from: f64, to: f64) -> Automation {
        Automation {
            track: None,
            value: String::from("tempo"),
            from,
            to,
        }
    }

    // The note messages, as (sample, on, note).
    fn notes_of(messages: &[Timed]) -> Vec<(u64, bool, u8)> {
        let mut notes = Vec::new();
        for timed in messages {
            match timed.message {
                Message::NoteOn { note, .. } => notes.push((timed.sample, true, note)),
                Message::NoteOff { note, .. } => notes.push((timed.sample, false, note)),
                _ => {}
            }
        }
        notes
    }

    fn tempos_of(messages: &[Timed]) -> Vec<(u64, u32)> {
        let mut tempos = Vec::new();
        for timed in messages {
            if let Message::Tempo(tempo) = timed.message {
                tempos.push((timed.sample, tempo));
            }
        }
        tempos
    }

    fn one_layer_loop(bars: u32, notes: &[(u8, f64, f64)]) -> SongLoop {
        SongLoop {
            bars,
            layers: vec![layer(0, 0, notes)],
        }
    }

    #[test]
    fn a_layer_repeats_on_its_samples_across_the_section() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let verse = section("Verse", 2, 120, &[(0, 0)]);
        let messages = section_messages(&verse, 0, &looped, &one_track(), &Mix::new(1), RATE);
        assert_eq!(
            notes_of(&messages),
            vec![(0, true, 60), (24_000, false, 60), (BAR_AT_120, true, 60), (BAR_AT_120 + 24_000, false, 60)]
        );
        assert_eq!(tempos_of(&messages), vec![(0, 120)]);
        assert_eq!(
            messages[1].message,
            Message::SlotVolume { track: 0, slot: 0, percent: 80 }
        );
    }

    #[test]
    fn a_layer_with_its_own_bars_repeats_on_them() {
        let looped = SongLoop {
            bars: 2,
            layers: vec![layer(0, 1, &[(60, 2.0, 3.0)])],
        };
        let verse = section("Verse", 2, 120, &[(0, 0)]);
        let messages = section_messages(&verse, 0, &looped, &one_track(), &Mix::new(1), RATE);
        let ons: Vec<u64> = notes_of(&messages).iter().filter(|note| note.1).map(|note| note.0).collect();
        assert_eq!(ons, vec![48_000, BAR_AT_120 + 48_000]);
    }

    #[test]
    fn notes_still_sounding_let_go_at_the_section_end() {
        let looped = one_layer_loop(2, &[(60, 2.0, 8.0)]);
        let short = section("Short", 1, 120, &[(0, 0)]);
        let messages = section_messages(&short, 1_000, &looped, &one_track(), &Mix::new(1), RATE);
        assert_eq!(notes_of(&messages), vec![(49_000, true, 60), (1_000 + BAR_AT_120, false, 60)]);
    }

    #[test]
    fn a_repeated_note_lets_go_before_it_plays_again() {
        let looped = one_layer_loop(1, &[(60, 0.0, 4.0)]);
        let verse = section("Verse", 2, 120, &[(0, 0)]);
        let notes = notes_of(&section_messages(&verse, 0, &looped, &one_track(), &Mix::new(1), RATE));
        assert_eq!(notes[1], (BAR_AT_120, false, 60));
        assert_eq!(notes[2], (BAR_AT_120, true, 60));
    }

    #[test]
    fn a_note_with_no_length_still_lets_go_after_it_starts() {
        let looped = one_layer_loop(1, &[(60, 1.0, 1.0)]);
        let verse = section("Verse", 1, 120, &[(0, 0)]);
        let notes = notes_of(&section_messages(&verse, 0, &looped, &one_track(), &Mix::new(1), RATE));
        assert_eq!(notes, vec![(24_000, true, 60), (24_001, false, 60)]);
    }

    #[test]
    fn the_key_shift_moves_notes_but_never_drums_or_the_expression_keys() {
        let looped = SongLoop {
            bars: 1,
            layers: vec![
                layer(0, 0, &[(60, 0.0, 1.0), (126, 1.0, 2.0), (VIBRATO_KEY_NOTE, 2.0, 3.0)]),
                layer(1, 0, &[(130, 0.0, 0.5), (60, 1.0, 1.5)]),
            ],
        };
        let mut lifted = section("Lifted", 1, 120, &[(0, 0), (1, 0)]);
        lifted.key_shift = 2;
        let notes = notes_of(&section_messages(&lifted, 0, &looped, &with_drums(), &Mix::new(2), RATE));
        let ons: Vec<u8> = notes.iter().filter(|note| note.1).map(|note| note.2).collect();
        // 126 + 2 is past the highest note, so it is left out.
        assert_eq!(ons, vec![62, 130, 60, VIBRATO_KEY_NOTE]);

        lifted.key_shift = -12;
        let notes = notes_of(&section_messages(&lifted, 0, &looped, &with_drums(), &Mix::new(2), RATE));
        assert_eq!(notes[0], (0, true, 48));
    }

    #[test]
    fn sections_play_one_after_another_at_their_own_tempos() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let sections = vec![section("Fast", 1, 120, &[(0, 0)]), section("Slow", 1, 60, &[(0, 0)])];
        let mut player = Player::new(&sections, &[0, 1, 0], looped, one_track(), Mix::new(1), RATE).unwrap();
        assert_eq!(player.end(), BAR_AT_120 + 2 * BAR_AT_120 + BAR_AT_120);
        let all = player.take_due(u64::MAX, usize::MAX);
        assert_eq!(
            notes_of(&all),
            vec![
                (0, true, 60),
                (24_000, false, 60),
                (BAR_AT_120, true, 60),
                (BAR_AT_120 + 48_000, false, 60),
                (3 * BAR_AT_120, true, 60),
                (3 * BAR_AT_120 + 24_000, false, 60),
            ]
        );
        assert_eq!(tempos_of(&all), vec![(0, 120), (BAR_AT_120, 60), (3 * BAR_AT_120, 120)]);
        assert_eq!(player.use_at(0), Some(0));
        assert_eq!(player.use_at(BAR_AT_120), Some(1));
        assert_eq!(player.use_at(3 * BAR_AT_120 - 1), Some(1));
        assert_eq!(player.use_at(player.end()), None);
        assert!(!player.is_done(player.end()));
        assert!(player.is_done(player.end() + RATE as u64));
    }

    #[test]
    fn a_ramped_tempo_times_the_beats_and_steps_every_10_ms() {
        // From 60 to 120 across 4 beats: 4 ln 2 seconds.
        let looped = one_layer_loop(1, &[(60, 0.0, 0.5), (62, 2.0, 2.5)]);
        let mut ramp = section("Ramp", 1, 90, &[(0, 0)]);
        ramp.automation.push(tempo_ramp(60.0, 120.0));
        let player = Player::new(&[ramp.clone()], &[0], looped.clone(), one_track(), Mix::new(1), RATE).unwrap();
        let seconds = 4.0 * 2f64.ln();
        assert_eq!(player.end(), (seconds * RATE as f64).round() as u64);

        let messages = section_messages(&ramp, 0, &looped, &one_track(), &Mix::new(1), RATE);
        // Beat 2 is halfway in tempo, at 90: 4 ln 1.5 seconds in.
        let beat_2 = (4.0 * 1.5f64.ln() * RATE as f64).round() as u64;
        assert_eq!(notes_of(&messages)[2], (beat_2, true, 62));

        let tempos = tempos_of(&messages);
        assert_eq!(tempos[0], (0, 60));
        assert_eq!(tempos.last().unwrap().1, 120);
        for pair in tempos.windows(2) {
            assert_eq!(pair[1].1, pair[0].1 + 1);
            assert_eq!(pair[1].0 % 480, 0);
        }
    }

    #[test]
    fn a_held_tempo_automation_wins_over_the_section_tempo() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let mut held = section("Held", 1, 120, &[(0, 0)]);
        held.automation.push(tempo_ramp(60.0, 60.0));
        let messages = section_messages(&held, 0, &looped, &one_track(), &Mix::new(1), RATE);
        assert_eq!(tempos_of(&messages), vec![(0, 60)]);
        assert_eq!(notes_of(&messages)[1], (48_000, false, 60));
    }

    // Moves of mix values across a section.
    fn moving(track: Option<usize>, value: &str, from: f64, to: f64) -> Automation {
        Automation {
            track,
            value: String::from(value),
            from,
            to,
        }
    }

    // The mix messages among `messages`, with their samples.
    fn mix_parts_of(messages: &[Timed]) -> Vec<(u64, MixPart)> {
        let mut parts = Vec::new();
        for timed in messages {
            if let Message::Mix(part) = timed.message {
                parts.push((timed.sample, part));
            }
        }
        parts
    }

    #[test]
    fn a_section_moves_mix_values_and_puts_them_back_at_its_end() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let mut engine = Engine::for_render().unwrap();
        let mut mix = Mix::new(1);
        mix.add_bus(&mut engine, None).unwrap();
        mix.set_bus_volume(&mut engine, 0, 80).unwrap();
        let mut swept = section("Swept", 1, 120, &[(0, 0)]);
        swept.automation.push(moving(Some(0), "pan", -100.0, 100.0));
        swept.automation.push(moving(None, "bus 1 volume", 0.0, 100.0));
        let messages = section_messages(&swept, 0, &looped, &one_track(), &mix, RATE);
        let parts = mix_parts_of(&messages);

        assert!(parts.contains(&(0, MixPart::TrackPan(0, -100))));
        assert!(parts.contains(&(0, MixPart::BusVolume(0, 0))));
        // Back to the mix's own at the end of the bar.
        assert!(parts.contains(&(BAR_AT_120, MixPart::TrackPan(0, 0))));
        assert!(parts.contains(&(BAR_AT_120, MixPart::BusVolume(0, 80))));
        // The tempo stays the section's.
        assert_eq!(tempos_of(&messages), vec![(0, 120)]);
    }

    #[test]
    fn a_mix_value_the_song_lacks_refuses_the_arrangement() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let mut swept = section("Swept", 1, 120, &[(0, 0)]);
        swept.automation.push(moving(None, "bus 1 volume", 0.0, 100.0));
        let refused = Player::new(&[swept], &[0], looped, one_track(), Mix::new(1), RATE);
        assert_eq!(refused.err(), Some(String::from("Swept: the song has no bus 1")));
    }

    #[test]
    fn a_section_layer_the_loop_no_longer_has_plays_nothing() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let verse = section("Verse", 1, 120, &[(0, 3), (2, 0)]);
        let messages = section_messages(&verse, 0, &looped, &one_track(), &Mix::new(1), RATE);
        assert_eq!(messages, vec![Timed { sample: 0, message: Message::Tempo(120) }]);
    }

    #[test]
    fn sections_are_planned_only_as_they_come_due() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let sections = vec![section("Verse", 1, 120, &[(0, 0)])];
        let mut player = Player::new(&sections, &[0, 0, 0], looped, one_track(), Mix::new(1), RATE).unwrap();
        let first = player.take_due(BAR_AT_120 - 1, usize::MAX);
        assert_eq!(first.len(), 4);
        assert_eq!(player.next_use, 1);
        // No room takes nothing.
        assert!(player.take_due(u64::MAX, 0).is_empty());
        let two = player.take_due(u64::MAX, 2);
        assert_eq!(two.len(), 2);
        assert_eq!(two[0].sample, BAR_AT_120);
        let rest = player.take_due(u64::MAX, usize::MAX);
        assert_eq!(rest.len(), 2 + 4);
        assert!(player.take_due(u64::MAX, usize::MAX).is_empty());
    }

    #[test]
    fn nothing_to_play_or_too_many_notes_is_refused() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let sections = vec![section("Verse", 1, 120, &[(0, 0)])];
        let empty = Player::new(&sections, &[], looped.clone(), one_track(), Mix::new(1), RATE);
        assert_eq!(empty.err(), Some(String::from("there are no sections to play")));

        let mut many = Vec::new();
        for step in 0..1100 {
            let start = step as f64 / 300.0;
            many.push((60, start, start));
        }
        let dense = one_layer_loop(1, &many);
        let long = vec![section("Long", 64, 120, &[(0, 0)])];
        let refused = Player::new(&long, &[0], dense, one_track(), Mix::new(1), RATE);
        assert_eq!(
            refused.err(),
            Some(String::from("Long plays more than 65536 notes; use fewer or longer layers"))
        );
    }

    // The engine with `looped` open at the new song's tempo, and a composer
    // whose sections are `sections` in the order `arrangement` gives.
    fn arranged(looped: &SongLoop, sections: Vec<SongSection>, arrangement: Vec<usize>) -> (Composer, Engine) {
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        composer.send_all(&mut engine);
        open_song_loop(&mut engine, START_TEMPO, Some(looped)).unwrap();
        engine.render(512);
        composer.set_sections(sections, arrangement);
        (composer, engine)
    }

    fn loudest(samples: &[f32]) -> f32 {
        let mut loudest = 0.0f32;
        for sample in samples {
            loudest = loudest.max(sample.abs());
        }
        loudest
    }

    #[test]
    fn practicing_a_section_puts_the_loop_away_and_anything_needing_it_gives_it_back() {
        use crate::song_session::{give_back_practice_loop, put_loop_away_for_practice};
        let looped = one_layer_loop(1, &[(60, 0.0, 2.0)]);
        let (mut composer, mut engine) = arranged(&looped, vec![section("Verse", 1, 120, &[(0, 0)])], vec![0]);

        put_loop_away_for_practice(&mut composer, &mut engine).unwrap();
        engine.render(512);
        assert_eq!(engine.looper_status().state, LooperState::Idle);
        // The ladder can set the tempo now, and the song still keeps its loop.
        assert_eq!(engine.set_tempo(84), 84);
        assert_eq!(song_now(&composer, &mut engine).looped, Some(looped.clone()));
        // The section's details still show the track's layer.
        let shown = serde_json::to_value(crate::sections::view(&mut composer, &mut engine)).unwrap();
        assert_eq!(shown["tracks"][0]["layers"], 1);

        // Whatever needs the loop stops the sections first, which gives it back.
        stop(&mut composer, &mut engine);
        engine.render(512);
        assert!(composer.practice_loop().is_none());
        assert_eq!(engine.looper_status().state, LooperState::Paused);
        assert_eq!(song_now(&composer, &mut engine).looped, Some(looped.clone()));

        // Practice ending gives it back too, and twice is harmless.
        put_loop_away_for_practice(&mut composer, &mut engine).unwrap();
        give_back_practice_loop(&mut composer, &mut engine);
        give_back_practice_loop(&mut composer, &mut engine);
        engine.render(512);
        assert_eq!(engine.looper_status().state, LooperState::Paused);
        assert_eq!(song_now(&composer, &mut engine).looped, Some(looped));
    }

    #[test]
    fn practicing_with_no_loop_or_while_recording_keeps_the_looper_as_it_is() {
        use crate::song_session::put_loop_away_for_practice;
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        put_loop_away_for_practice(&mut composer, &mut engine).unwrap();
        assert!(composer.practice_loop().is_none());

        engine.press_looper();
        engine.render(512);
        let recording = put_loop_away_for_practice(&mut composer, &mut engine);
        assert_eq!(recording, Err(String::from("the looper is recording: finish it first")));
        assert!(composer.practice_loop().is_none());
    }

    #[test]
    fn playing_the_sections_puts_the_loop_away_and_gives_it_back_after() {
        let looped = one_layer_loop(1, &[(60, 0.0, 2.0)]);
        let (mut composer, mut engine) = arranged(&looped, vec![section("Verse", 1, 120, &[(0, 0)])], vec![0]);
        assert_eq!(engine.looper_status().state, LooperState::Paused);

        play(&mut composer, &mut engine).unwrap();
        assert!(composer.arranging().is_some());
        engine.render(512);
        assert_eq!(engine.looper_status().state, LooperState::Idle);
        // The song still keeps its loop while it is put away.
        assert_eq!(song_now(&composer, &mut engine).looped, Some(looped.clone()));
        assert_eq!(playing_now(&composer, &engine), Some(0));

        let sound = engine.render(24_000);
        assert!(loudest(&sound) > 0.01);

        let mut stopped = false;
        let mut chunks = 0;
        while !stopped {
            stopped = keep_playing(&mut composer, &mut engine);
            engine.render(4_800);
            chunks += 1;
            assert!(chunks < 100);
        }
        assert!(composer.arranging().is_none());
        engine.render(512);
        assert_eq!(engine.looper_status().state, LooperState::Paused);
        assert_eq!(song_now(&composer, &mut engine).looped, Some(looped));
    }

    #[test]
    fn routing_edits_stop_only_after_valid_occupied_slots_change() {
        use crate::engine::{EffectConnection, EQ_OUTPUT};
        use crate::modulation::{CONNECTION_COUNT, NO_CONNECTION, Source};
        let looped = one_layer_loop(1, &[(60, 0.0, 2.0)]);
        let (mut composer, mut engine) = arranged(&looped, vec![section("Verse", 4, 120, &[(0, 0)])], vec![0]);
        let link = EffectConnection { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: 0.0, one_way: false, scaled_by: Source::None };
        composer.mix_mut().tracks[0].links = vec![link; 63];
        composer.send_picked_routing(&mut engine);
        play(&mut composer, &mut engine).unwrap();

        let mut full = composer.sound_profile().sound();
        full.connections[CONNECTION_COUNT - 1] = full.connections[0];
        assert!(composer.apply_sound(&mut engine, &full).is_err());
        assert!(composer.arranging().is_some());

        let mut changed = composer.sound_profile().sound();
        changed.connections[0].amount = 10.0;
        composer.apply_sound(&mut engine, &changed).unwrap();
        assert!(composer.arranging().is_some());

        changed.connections[0] = NO_CONNECTION;
        composer.apply_sound(&mut engine, &changed).unwrap();
        assert!(composer.arranging().is_none());
        assert_eq!(composer.sound_profile().connections()[0], NO_CONNECTION);
        assert_eq!(composer.mix_now().tracks[0].links.len(), 63);

        engine.render(512);
        play(&mut composer, &mut engine).unwrap();
        let mut preset = composer.sound_profile().sound();
        preset.lfos[0].hz = 2.0;
        composer.take_preset(&mut engine, "same slots", &preset).unwrap();
        assert!(composer.arranging().is_some());
        preset.connections[0] = full.connections[0];
        composer.take_preset(&mut engine, "new slot", &preset).unwrap();
        assert!(composer.arranging().is_none());
    }

    #[test]
    fn stopping_for_routing_change_discards_queued_ordinary_connection_writes() {
        use crate::engine::{EffectConnection, EffectKind, EQ_OUTPUT};
        use crate::mix::Place;
        use crate::modulation::{LfoShape, NO_CONNECTION, Source};
        let looped = one_layer_loop(1, &[(60, 3.0, 3.5)]);
        let mut verse = section("Verse", 4, 120, &[(0, 0)]);
        verse.automation.push(moving(Some(0), "connection 1 amount", 10.0, 20.0));
        let (mut composer, mut engine) = arranged(&looped, vec![verse], vec![0]);
        composer.mix_mut().place_effect(&mut engine, Place::Track(0), 0, EffectKind::Eq).unwrap();
        let mut lfo = composer.sound_profile().lfos()[0];
        lfo.per_note = false;
        lfo.shape = LfoShape::Steps;
        lfo.step_count = 1;
        lfo.steps[0] = 100.0;
        composer.sound_profile_mut().set_lfo(&mut engine, 0, lfo);
        composer.mix_mut().tracks[0].links.push(EffectConnection { source: Source::Lfo(0), slot: 0,
            value: EQ_OUTPUT, amount: -6.0, one_way: false, scaled_by: Source::None });
        composer.send_picked_routing(&mut engine);
        play(&mut composer, &mut engine).unwrap();
        let mut wanted = composer.sound_profile().sound();
        wanted.connections[0] = NO_CONNECTION;
        composer.apply_sound(&mut engine, &wanted).unwrap();
        assert!(composer.arranging().is_none());

        // The old automation had already been queued for slot 1, which is
        // now the effect link's slot. Let its former deadline pass.
        engine.render(96_000);
        // Live like `engine`, whose limiter does not look ahead.
        let mut expected = Engine::new().unwrap();
        composer.send_all(&mut expected);
        expected.render(96_000);
        engine.note_on(69, 100);
        expected.note_on(69, 100);
        let actual = engine.render(4096);
        let reference = expected.render(4096);
        assert!(reference.iter().any(|sample| sample.abs() > 0.001));
        assert_eq!(actual, reference);
    }

    #[test]
    fn the_looper_keys_stop_the_sections() {
        let looped = one_layer_loop(1, &[(60, 0.0, 2.0)]);
        let (mut composer, mut engine) = arranged(&looped, vec![section("Verse", 4, 120, &[(0, 0)])], vec![0]);
        play(&mut composer, &mut engine).unwrap();
        engine.render(512);
        stop(&mut composer, &mut engine);
        assert!(composer.arranging().is_none());
        assert_eq!(engine.sequence_position(), None);
        engine.render(512);
        assert_eq!(engine.looper_status().state, LooperState::Paused);
    }

    #[test]
    fn with_no_loop_or_while_recording_nothing_plays() {
        let mut engine = Engine::new().unwrap();
        let mut composer = Composer::new();
        composer.set_sections(vec![section("Verse", 1, 120, &[(0, 0)])], vec![0]);
        let none = play(&mut composer, &mut engine);
        assert_eq!(none, Err(String::from("there is no loop: sections play its layers")));

        engine.press_looper();
        engine.render(512);
        let counting = play(&mut composer, &mut engine);
        assert_eq!(counting, Err(String::from("finish the recording before playing the sections")));
        assert!(composer.arranging().is_none());
    }

    #[test]
    fn a_song_far_ahead_is_fed_a_little_at_a_time() {
        let looped = one_layer_loop(1, &[(60, 0.0, 0.25), (62, 1.0, 1.25), (64, 2.0, 2.25), (65, 3.0, 3.25)]);
        let (mut composer, mut engine) = arranged(&looped, vec![section("Long", 64, 120, &[(0, 0)])], vec![0]);
        play(&mut composer, &mut engine).unwrap();
        // 2 seconds ahead is one bar: 4 notes on, 4 off, a tempo and a volume.
        let room = engine.sequence_room();
        assert!(room > 0);
        let player = composer.arranging().unwrap();
        assert_eq!(player.next_use, 1);
        assert_eq!(player.waiting.len(), 64 * 8 + 2 - 10 - 1);
    }

    fn song_of(looped: Option<SongLoop>, sections: Vec<SongSection>, arrangement: Vec<usize>) -> Song {
        let mut song = Song::new(&SoundProfile::new());
        song.looped = looped;
        song.sections = sections;
        song.arrangement = arrangement;
        song
    }

    #[test]
    fn a_render_of_the_sections_is_the_same_every_time() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0), (64, 2.0, 3.0)]);
        let sections = vec![section("Verse", 1, 120, &[(0, 0)]), section("Chorus", 1, 90, &[(0, 0)])];
        let song = song_of(Some(looped), sections, vec![0, 1, 0]);
        let (first, rate) = render(&song).unwrap();
        let (second, _) = render(&song).unwrap();
        assert_eq!(rate, RATE);
        // 2 + 8/3 + 2 seconds, then the tail, in two channels.
        let seconds = 2.0 + 4.0 * 60.0 / 90.0 + 2.0 + TAIL_SECONDS as f64;
        let frames = (seconds * RATE as f64).round() as usize;
        assert!(first.len().abs_diff(frames * 2) <= 4);
        assert!(loudest(&first) > 0.01);
        assert!(first == second);
    }

    #[test]
    fn stopping_puts_back_what_automation_moved() {
        let looped = one_layer_loop(1, &[(60, 3.0, 3.5)]);
        let mut verse = section("Verse", 4, 120, &[(0, 0)]);
        verse.automation.push(moving(Some(0), "pan", -100.0, -100.0));
        let (mut composer, mut engine) = arranged(&looped, vec![verse], vec![0]);
        play(&mut composer, &mut engine).unwrap();
        engine.render(24_000);
        stop(&mut composer, &mut engine);
        engine.render(24_000);

        engine.note_on(69, 100);
        let samples = engine.render(8192);
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        for frame in samples.chunks(2) {
            left = left.max(frame[0].abs());
            right = right.max(frame[1].abs());
        }
        assert!(left > 0.01);
        assert!((left - right).abs() < 0.001);
    }

    #[test]
    fn stopping_puts_back_the_volume_automation_moved() {
        let looped = one_layer_loop(1, &[(60, 3.0, 3.5)]);
        let mut verse = section("Verse", 4, 120, &[(0, 0)]);
        verse.automation.push(moving(Some(0), "volume", 0.0, 0.0));
        let (mut composer, mut engine) = arranged(&looped, vec![verse], vec![0]);
        play(&mut composer, &mut engine).unwrap();
        engine.render(24_000);
        stop(&mut composer, &mut engine);
        engine.render(24_000);

        engine.note_on(69, 100);
        let samples = engine.render(8192);
        assert!(loudest(&samples) > 0.01);
    }

    #[test]
    fn a_render_of_every_track_keeps_the_whole_mix() {
        use crate::effects::Written;
        use crate::engine::{EffectKind, TRACK_COUNT};
        use crate::mix::{Mix, Place};
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let mut song = song_of(Some(looped), vec![section("Verse", 1, 120, &[(0, 0)])], vec![0]);
        while song.tracks.len() < TRACK_COUNT {
            let copy = song.tracks[0].clone();
            song.tracks.push(copy);
        }
        song.mix = Mix::new(TRACK_COUNT);
        let (open, _) = render(&song).unwrap();

        let mut engine = Engine::for_render().unwrap();
        song.mix.place_effect(&mut engine, Place::Master, 0, EffectKind::Eq).unwrap();
        song.mix.set_effect_value(&mut engine, Place::Master, 0, "output_db", &Written::Number(-24.0)).unwrap();
        let (quieter, _) = render(&song).unwrap();
        assert!(loudest(&quieter) < loudest(&open) * 0.1);
    }

    #[test]
    fn a_render_starts_on_its_first_sample() {
        // The limiter's lookahead is trimmed, so a note on beat 1 sounds
        // within the first 1 ms instead of after it.
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let sections = vec![section("Verse", 1, 120, &[(0, 0)])];
        let song = song_of(Some(looped), sections, vec![0]);
        let (samples, rate) = render(&song).unwrap();
        let lookahead = 2 * rate as usize / 1000;
        assert!(loudest(&samples[..lookahead]) > 0.0);
    }

    #[test]
    fn a_render_never_hears_a_muted_track() {
        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let sections = vec![section("Verse", 1, 120, &[(0, 0)])];
        let mut song = song_of(Some(looped), sections, vec![0]);
        song.tracks[0].volume = 0;
        let (samples, _) = render(&song).unwrap();
        assert_eq!(loudest(&samples), 0.0);
    }

    #[test]
    fn a_render_with_no_loop_or_too_long_is_refused() {
        let section = section("Verse", 1, 120, &[(0, 0)]);
        let none = song_of(None, vec![section.clone()], vec![0]);
        assert_eq!(render(&none), Err(String::from("there is no loop: sections play its layers")));

        let looped = one_layer_loop(1, &[(60, 0.0, 1.0)]);
        let mut long = section;
        long.bars = 64;
        long.tempo = 30;
        let arrangement = vec![0; 3];
        let too_long = song_of(Some(looped), vec![long], arrangement);
        let refused = render(&too_long).unwrap_err();
        assert!(refused.contains("a render is at most 10"), "{refused}");
    }
}
