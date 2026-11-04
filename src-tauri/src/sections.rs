// The song's sections and the order it plays them in
// (docs/design.md#tracks-and-sections): saving the layers playing now as a
// section, changing a section's bars, tempo, key shift, layers and
// automation, and the sections lane's reordering, reusing and removing.
// Sections, tracks, layers and places in the order are counted from 1 in
// every edit, as on screen and in the song file.
use serde::{Deserialize, Serialize};

use crate::arrange;
use crate::automation::part_at;
use crate::compose::Composer;
use crate::harmony::{Key, key_name};
use crate::key_mapping::KeyMapping;
use crate::engine::{Engine, FASTEST_TEMPO, SLOWEST_TEMPO, TRACK_COUNT};
use crate::mix_automation::{forget_bus, is_song_wide, is_track_mix_value, mix_part_at};
use crate::song_file::{
    Automation, MOST_ARRANGED, MOST_AUTOMATION, MOST_KEY_SHIFT, MOST_SECTIONS, SECTION_MOST_BARS, SectionLayer,
    SongLoop, SongSection, TEMPO_WORD,
};
use crate::song_session::{loop_kept, loop_now, open_song_loop};

// The most letters in a section's name.
const NAME_LONGEST: usize = 40;

// The sections, each once, and the order they play in, as places among
// them. A section no place uses is forgotten, since nothing shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sections {
    list: Vec<SongSection>,
    order: Vec<usize>,
}

// One change to the sections, as the lane, the details and the agent send
// it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SectionEdit {
    // Saves the layers playing now as a new section at the end.
    Save,
    // Takes the last place in the order off.
    RemoveLast,
    // The section's layers become the ones playing now.
    SaveAgain { section: usize },
    // Turns the section's layers on in the looper, and the rest off, at its
    // tempo, and plays the loop.
    Open { section: usize },
    Rename { section: usize, name: String },
    Bars { section: usize, bars: u32 },
    Tempo { section: usize, tempo: u32 },
    KeyShift { section: usize, key_shift: i32 },
    // Plays track `track`'s layer `layer` in the section, or not.
    Layer { section: usize, track: usize, layer: usize, on: bool },
    // Moves `value` of track `track`, or with no track the tempo, from
    // `from` to `to` across the section, in place of any move of it there
    // was.
    // The tempo's move needs no value.
    Automate {
        section: usize,
        track: Option<usize>,
        #[serde(default)]
        value: String,
        from: f64,
        to: f64,
    },
    StopAutomating {
        section: usize,
        track: Option<usize>,
        #[serde(default)]
        value: String,
    },
    // Moves the section at place `place` in the order to place `to`.
    Move { place: usize, to: usize },
    // Plays the section at place `place` again, just after it.
    UseAgain { place: usize },
    RemoveUse { place: usize },
    // Plays the order from its start, or stops it.
    Play,
    Stop,
}

impl SectionEdit {
    // True for the edits that play something, so the speakers open first.
    pub fn plays(&self) -> bool {
        match self {
            SectionEdit::Open { .. } => true,
            SectionEdit::Play => true,
            _ => false,
        }
    }

    // What it does, in a line for the agent's panel.
    pub fn line(&self) -> String {
        match self {
            SectionEdit::Save => String::from("Saved the layers playing now as a section"),
            SectionEdit::RemoveLast => String::from("Took the last section out of the order"),
            SectionEdit::SaveAgain { section } => format!("Section {section} plays the layers playing now"),
            SectionEdit::Open { section } => format!("Opened section {section}"),
            SectionEdit::Rename { section, name } => format!("Section {section} is called {}", name.trim()),
            SectionEdit::Bars { section, bars } => format!("Section {section} lasts {bars} bars"),
            SectionEdit::Tempo { section, tempo } => format!("Section {section} plays at {tempo} bpm"),
            SectionEdit::KeyShift { section, key_shift } => {
                format!("Section {section} shifted {key_shift} half steps")
            }
            SectionEdit::Layer {
                section,
                track,
                layer,
                on,
            } => {
                if *on {
                    format!("Section {section} plays track {track}'s layer {layer}")
                } else {
                    format!("Section {section} leaves out track {track}'s layer {layer}")
                }
            }
            SectionEdit::Automate {
                section,
                track,
                value,
                from,
                to,
            } => match track {
                Some(track) => format!("Section {section} moves track {track}'s {value} from {from} to {to}"),
                None => format!("Section {section} moves the tempo from {from} to {to}"),
            },
            SectionEdit::StopAutomating { section, track, value } => match track {
                Some(track) => format!("Section {section} stops moving track {track}'s {value}"),
                None => format!("Section {section} stops moving the tempo"),
            },
            SectionEdit::Move { place, to } => format!("Moved the section at place {place} to {to}"),
            SectionEdit::UseAgain { place } => format!("The section at place {place} plays again after it"),
            SectionEdit::RemoveUse { place } => format!("Took place {place} out of the order"),
            SectionEdit::Play => String::from("Playing the sections"),
            SectionEdit::Stop => String::from("Sections stopped"),
        }
    }
}

impl Sections {
    pub fn new(list: Vec<SongSection>, order: Vec<usize>) -> Sections {
        Sections { list, order }
    }

    pub fn list(&self) -> &[SongSection] {
        &self.list
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    // Gives section `number`, from 0, its own mapping, or with None, puts it
    // back on the song's main one.
    pub fn set_keys(&mut self, number: usize, keys: Option<KeyMapping>) -> Result<(), String> {
        let Some(section) = self.list.get_mut(number) else {
            return Err(format!("the song has no section {}", number + 1));
        };
        section.keys = keys;
        Ok(())
    }

    // Puts every section back on the song's main mapping.
    pub fn forget_keys(&mut self) {
        for section in &mut self.list {
            section.keys = None;
        }
    }

    // Saves the layers of `looped` that are playing as a new section at the
    // end of the order, at `tempo`, lasting as long as the longest of them.
    // Returns its number.
    pub fn save(&mut self, looped: Option<&SongLoop>, tempo: u32) -> Result<usize, String> {
        if self.list.len() == MOST_SECTIONS {
            return Err(format!("{MOST_SECTIONS} sections is the most"));
        }
        if self.order.len() == MOST_ARRANGED {
            return Err(format!("a song plays at most {MOST_ARRANGED} sections"));
        }
        let Some(looped) = looped else {
            return Err(String::from("there is no loop to save a section from"));
        };
        let layers = playing_layers(looped)?;
        let section = SongSection {
            name: self.next_name(),
            bars: longest_bars(looped),
            tempo,
            key_shift: 0,
            layers,
            automation: Vec::new(),
            keys: None,
        };
        self.list.push(section);
        self.order.push(self.list.len() - 1);
        Ok(self.list.len())
    }

    // "Section 1", or the next number no section's name has.
    fn next_name(&self) -> String {
        let mut number = 1;
        loop {
            let name = format!("Section {number}");
            if !self.has_name(&name) {
                return name;
            }
            number += 1;
        }
    }

    fn has_name(&self, name: &str) -> bool {
        for section in &self.list {
            if section.name == name {
                return true;
            }
        }
        false
    }

    // Section `number`, counted from 1.
    fn section_mut(&mut self, number: usize) -> Result<&mut SongSection, String> {
        let index = index_of(number, self.list.len(), "section")?;
        Ok(&mut self.list[index])
    }

    pub fn save_again(&mut self, number: usize, looped: Option<&SongLoop>) -> Result<(), String> {
        let Some(looped) = looped else {
            return Err(String::from("there is no loop to save a section from"));
        };
        let layers = playing_layers(looped)?;
        let section = self.section_mut(number)?;
        section.layers = layers;
        Ok(())
    }

    pub fn rename(&mut self, number: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(String::from("a section needs a name"));
        }
        if name.chars().count() > NAME_LONGEST {
            return Err(format!("a section's name has up to {NAME_LONGEST} letters"));
        }
        let index = index_of(number, self.list.len(), "section")?;
        if self.list[index].name == name {
            return Ok(());
        }
        if self.has_name(name) {
            return Err(format!("another section is called {name}"));
        }
        self.list[index].name = String::from(name);
        Ok(())
    }

    pub fn set_bars(&mut self, number: usize, bars: u32) -> Result<(), String> {
        if bars < 1 || bars > SECTION_MOST_BARS {
            return Err(format!("a section lasts 1 to {SECTION_MOST_BARS} bars, not {bars}"));
        }
        self.section_mut(number)?.bars = bars;
        Ok(())
    }

    pub fn set_tempo(&mut self, number: usize, tempo: u32) -> Result<(), String> {
        if tempo < SLOWEST_TEMPO || tempo > FASTEST_TEMPO {
            return Err(format!("the tempo is {SLOWEST_TEMPO} to {FASTEST_TEMPO}, not {tempo}"));
        }
        self.section_mut(number)?.tempo = tempo;
        Ok(())
    }

    pub fn set_key_shift(&mut self, number: usize, key_shift: i32) -> Result<(), String> {
        if key_shift < -MOST_KEY_SHIFT || key_shift > MOST_KEY_SHIFT {
            return Err(format!(
                "the key shift is -{MOST_KEY_SHIFT} to {MOST_KEY_SHIFT} half steps, not {key_shift}"
            ));
        }
        self.section_mut(number)?.key_shift = key_shift;
        Ok(())
    }

    // Plays track `track`'s layer `layer` in section `number`, or not. The
    // loop must have that layer.
    pub fn set_layer(
        &mut self,
        number: usize,
        track: usize,
        layer: usize,
        on: bool,
        looped: Option<&SongLoop>,
    ) -> Result<(), String> {
        let track_index = index_of(track, TRACK_COUNT, "track")?;
        let layer_count = layers_on(looped, track_index);
        let layer_index = index_of(layer, layer_count, &format!("layer on track {track}"))?;
        let wanted = SectionLayer {
            track: track_index,
            layer: layer_index,
        };
        let section = self.section_mut(number)?;
        section.layers.retain(|used| *used != wanted);
        if on {
            section.layers.push(wanted);
            section.layers.sort_by_key(|used| (used.track, used.layer));
        }
        Ok(())
    }

    // Keeps `moved` in section `number`, in place of the move of the same
    // value there was.
    pub fn automate(&mut self, number: usize, moved: Automation) -> Result<(), String> {
        let section = self.section_mut(number)?;
        let place = section
            .automation
            .iter()
            .position(|before| before.track == moved.track && before.value == moved.value);
        if let Some(place) = place {
            section.automation[place] = moved;
            return Ok(());
        }
        if section.automation.len() == MOST_AUTOMATION {
            return Err(format!("a section moves at most {MOST_AUTOMATION} values"));
        }
        section.automation.push(moved);
        Ok(())
    }

    pub fn stop_automating(&mut self, number: usize, track: Option<usize>, value: &str) -> Result<(), String> {
        let section = self.section_mut(number)?;
        let before = section.automation.len();
        section
            .automation
            .retain(|moved| !(moved.track == track && moved.value == value));
        if section.automation.len() == before {
            return Err(format!("{} doesn't move {value}", section.name));
        }
        Ok(())
    }

    // Moves the section at place `place` in the order to place `to`, both
    // from 1.
    pub fn move_use(&mut self, place: usize, to: usize) -> Result<(), String> {
        let from_index = index_of(place, self.order.len(), "place")?;
        let to_index = index_of(to, self.order.len(), "place")?;
        let section = self.order.remove(from_index);
        self.order.insert(to_index, section);
        Ok(())
    }

    pub fn use_again(&mut self, place: usize) -> Result<(), String> {
        let index = index_of(place, self.order.len(), "place")?;
        if self.order.len() == MOST_ARRANGED {
            return Err(format!("a song plays at most {MOST_ARRANGED} sections"));
        }
        let section = self.order[index];
        self.order.insert(index + 1, section);
        Ok(())
    }

    pub fn remove_use(&mut self, place: usize) -> Result<(), String> {
        let index = index_of(place, self.order.len(), "place")?;
        self.order.remove(index);
        self.forget_unused();
        Ok(())
    }

    pub fn remove_last(&mut self) -> Result<(), String> {
        if self.order.is_empty() {
            return Err(String::from("there are no sections to remove"));
        }
        self.remove_use(self.order.len())
    }

    // Forgets the sections no place uses, and counts the places again.
    fn forget_unused(&mut self) {
        let mut kept = Vec::new();
        let mut new_places = Vec::new();
        for (index, section) in self.list.iter().enumerate() {
            if self.order.contains(&index) {
                new_places.push(Some(kept.len()));
                kept.push(section.clone());
            } else {
                new_places.push(None);
            }
        }
        let mut order = Vec::new();
        for place in &self.order {
            if let Some(new_place) = new_places[*place] {
                order.push(new_place);
            }
        }
        self.list = kept;
        self.order = order;
    }

    // Track `track`'s layer `layer`, both from 0, is gone, so no section
    // plays it, and its later layers move one down.
    pub fn forget_layer(&mut self, track: usize, layer: usize) {
        for section in &mut self.list {
            section.layers.retain(|used| !(used.track == track && used.layer == layer));
            for used in &mut section.layers {
                if used.track == track && used.layer > layer {
                    used.layer -= 1;
                }
            }
        }
    }

    // Takes track `removed`, from 0, out of the sections: its layers and
    // values go, and the later tracks' move one track down.
    pub fn forget_track(&mut self, removed: usize) {
        for section in &mut self.list {
            section.layers.retain(|layer| layer.track != removed);
            for layer in &mut section.layers {
                if layer.track > removed {
                    layer.track -= 1;
                }
            }
            section.automation.retain(|moved| moved.track != Some(removed));
            for moved in &mut section.automation {
                if let Some(track) = moved.track {
                    if track > removed {
                        moved.track = Some(track - 1);
                    }
                }
            }
        }
    }

    // Removes moves of bus `removed`, from 0, and renumbers later buses.
    pub fn forget_bus(&mut self, removed: usize) {
        for section in &mut self.list {
            section.automation.retain_mut(|moved| forget_bus(moved.track, &mut moved.value, removed));
        }
    }
}

// `number`, counted from 1 among `count` of `what`, from 0.
fn index_of(number: usize, count: usize, what: &str) -> Result<usize, String> {
    if number < 1 || number > count {
        return Err(format!("there is no {what} {number}"));
    }
    Ok(number - 1)
}

// The layers of `looped` that are playing, as a section keeps them.
fn playing_layers(looped: &SongLoop) -> Result<Vec<SectionLayer>, String> {
    let mut layers = Vec::new();
    let mut counts = [0; TRACK_COUNT];
    for layer in &looped.layers {
        let index = counts[layer.track];
        counts[layer.track] += 1;
        if layer.playing {
            layers.push(SectionLayer {
                track: layer.track,
                layer: index,
            });
        }
    }
    if layers.is_empty() {
        return Err(String::from("no layers are playing: turn some on to save them as a section"));
    }
    Ok(layers)
}

// The bars the longest playing layer of `looped` takes to repeat, within
// a section's.
fn longest_bars(looped: &SongLoop) -> u32 {
    let mut longest = 1;
    for layer in &looped.layers {
        if !layer.playing {
            continue;
        }
        let mut bars = layer.bars;
        if bars == 0 {
            bars = looped.bars;
        }
        longest = longest.max(bars);
    }
    longest.min(SECTION_MOST_BARS)
}

// How many layers track `track`, from 0, has in `looped`.
fn layers_on(looped: Option<&SongLoop>, track: usize) -> usize {
    let Some(looped) = looped else {
        return 0;
    };
    let mut count = 0;
    for layer in &looped.layers {
        if layer.track == track {
            count += 1;
        }
    }
    count
}

// Lets the sections forget the layers removed since this was last called.
// Everything that reads the sections calls this first.
pub fn catch_up(composer: &mut Composer, engine: &mut Engine) {
    for (track, layer) in engine.take_removed_layers() {
        composer.sections_mut().forget_layer(track, layer);
    }
}

// Makes `edit` to the song's sections in `composer`, and on `engine` for
// the ones that play. A failed edit changes nothing.
pub fn edit(composer: &mut Composer, engine: &mut Engine, edit: SectionEdit) -> Result<(), String> {
    catch_up(composer, engine);
    match edit {
        SectionEdit::Save => {
            arrange::stop(composer, engine);
            let looped = loop_now(engine);
            let tempo = composer.tempo();
            composer.sections_mut().save(looped.as_ref(), tempo)?;
        }
        SectionEdit::RemoveLast => {
            arrange::stop(composer, engine);
            composer.sections_mut().remove_last()?;
        }
        SectionEdit::SaveAgain { section } => {
            arrange::stop(composer, engine);
            let looped = loop_now(engine);
            composer.sections_mut().save_again(section, looped.as_ref())?;
        }
        SectionEdit::Open { section } => {
            arrange::stop(composer, engine);
            open(composer, engine, section)?;
        }
        SectionEdit::Rename { section, name } => {
            composer.sections_mut().rename(section, &name)?;
        }
        SectionEdit::Bars { section, bars } => {
            composer.sections_mut().set_bars(section, bars)?;
        }
        SectionEdit::Tempo { section, tempo } => {
            composer.sections_mut().set_tempo(section, tempo)?;
        }
        SectionEdit::KeyShift { section, key_shift } => {
            composer.sections_mut().set_key_shift(section, key_shift)?;
        }
        SectionEdit::Layer {
            section,
            track,
            layer,
            on,
        } => {
            let looped = loop_kept(composer, engine);
            composer.sections_mut().set_layer(section, track, layer, on, looped.as_ref())?;
        }
        SectionEdit::Automate {
            section,
            track,
            value,
            from,
            to,
        } => {
            let moved = automation_of(composer, track, &value, from, to)?;
            composer.sections_mut().automate(section, moved)?;
        }
        SectionEdit::StopAutomating { section, track, value } => {
            let track_index = track_index_of(composer, track)?;
            let value = value_word(track_index, &value);
            composer.sections_mut().stop_automating(section, track_index, &value)?;
        }
        SectionEdit::Move { place, to } => {
            arrange::stop(composer, engine);
            composer.sections_mut().move_use(place, to)?;
        }
        SectionEdit::UseAgain { place } => {
            arrange::stop(composer, engine);
            composer.sections_mut().use_again(place)?;
        }
        SectionEdit::RemoveUse { place } => {
            arrange::stop(composer, engine);
            composer.sections_mut().remove_use(place)?;
        }
        SectionEdit::Play => {
            arrange::play(composer, engine)?;
        }
        SectionEdit::Stop => {
            arrange::stop(composer, engine);
        }
    }
    Ok(())
}

// Turns section `number`'s layers on in the looper and every other layer
// off, opens the loop again at the section's tempo, which becomes the
// song's, and plays it from beat 1.
fn open(composer: &mut Composer, engine: &mut Engine, number: usize) -> Result<(), String> {
    let index = index_of(number, composer.sections().len(), "section")?;
    let section = composer.sections()[index].clone();
    let Some(mut looped) = loop_now(engine) else {
        return Err(String::from("there is no loop: sections play its layers"));
    };
    let mut counts = [0; TRACK_COUNT];
    for layer in &mut looped.layers {
        let used = SectionLayer {
            track: layer.track,
            layer: counts[layer.track],
        };
        counts[layer.track] += 1;
        layer.playing = section.layers.contains(&used);
    }
    open_song_loop(engine, section.tempo, Some(&looped))?;
    composer.follow_tempo(engine);
    engine.play_loop();
    Ok(())
}

// Track `track`, counted from 1, from 0, or None for the tempo.
fn track_index_of(composer: &Composer, track: Option<usize>) -> Result<Option<usize>, String> {
    let Some(track) = track else {
        return Ok(None);
    };
    let index = index_of(track, composer.tracks().list().len(), "track")?;
    Ok(Some(index))
}

// The word a move keeps for `value`. With no track it is the tempo's own,
// unless it names a value of the song's mix, like "bus 1 volume".
fn value_word(track: Option<usize>, value: &str) -> String {
    let value = value.trim();
    if track.is_none() {
        if !is_song_wide(value) {
            return String::from(TEMPO_WORD);
        }
    }
    String::from(value)
}

// The move of `value` of track `track` from `from` to `to`, or why it
// can't move: a value a track doesn't have, or a start or end outside its
// range.
fn automation_of(
    composer: &Composer,
    track: Option<usize>,
    value: &str,
    from: f64,
    to: f64,
) -> Result<Automation, String> {
    if from.is_nan() || to.is_nan() {
        return Err(String::from("a move goes from a number to a number"));
    }
    let track_index = track_index_of(composer, track)?;
    let word = value_word(track_index, value);
    let mut moves_mix = is_track_mix_value(&word);
    if track_index.is_none() {
        moves_mix = word != TEMPO_WORD;
    }
    if moves_mix {
        let mix = composer.mix_now();
        mix_part_at(track_index, &word, from, &mix)?;
        mix_part_at(track_index, &word, to, &mix)?;
        return Ok(Automation {
            track: track_index,
            value: word,
            from,
            to,
        });
    }
    match track_index {
        None => {
            let slowest = SLOWEST_TEMPO as f64;
            let fastest = FASTEST_TEMPO as f64;
            for tempo in [from, to] {
                if tempo < slowest || tempo > fastest {
                    return Err(format!("the tempo is {SLOWEST_TEMPO} to {FASTEST_TEMPO}, not {tempo}"));
                }
            }
        }
        Some(index) => {
            let sound = composer.tracks().list()[index].sound_profile().sound();
            part_at(&word, from, &sound)?;
            part_at(&word, to, &sound)?;
        }
    }
    Ok(Automation {
        track: track_index,
        value: word,
        from,
        to,
    })
}

// What the sections lane and details show.
#[derive(Debug, PartialEq, Serialize)]
pub struct SectionsView {
    // Every section, in the order they were saved.
    sections: Vec<SectionView>,
    // The order the song plays them in, as section numbers from 1.
    order: Vec<usize>,
    // The place in the order playing now, from 1, or None.
    playing: Option<usize>,
    // Each track's layers, for the details' layer switches.
    tracks: Vec<TrackLayers>,
    // The song's tempo, so a section shows its own only if it differs.
    tempo: u32,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct SectionView {
    name: String,
    bars: u32,
    tempo: u32,
    key_shift: i32,
    // The key it plays in, with its shift, or None before a key is picked.
    key: Option<String>,
    // The layers it plays, as (track, layer) from 1.
    layers: Vec<(usize, usize)>,
    automation: Vec<AutomationView>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct AutomationView {
    // From 1, or None for the tempo.
    track: Option<usize>,
    value: String,
    from: f64,
    to: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct TrackLayers {
    name: String,
    layers: usize,
}

// What the sections lane and details show now.
pub fn view(composer: &mut Composer, engine: &mut Engine) -> SectionsView {
    catch_up(composer, engine);
    let looped = loop_kept(composer, engine);
    let mut sections = Vec::new();
    for section in composer.sections() {
        let mut layers = Vec::new();
        for used in &section.layers {
            layers.push((used.track + 1, used.layer + 1));
        }
        let mut automation = Vec::new();
        for moved in &section.automation {
            automation.push(AutomationView {
                track: moved.track.map(|track| track + 1),
                value: moved.value.clone(),
                from: moved.from,
                to: moved.to,
            });
        }
        sections.push(SectionView {
            name: section.name.clone(),
            bars: section.bars,
            tempo: section.tempo,
            key_shift: section.key_shift,
            key: shifted_key(composer.key(), section.key_shift),
            layers,
            automation,
        });
    }
    let mut order = Vec::new();
    for place in composer.arrangement() {
        order.push(place + 1);
    }
    let mut tracks = Vec::new();
    for (index, track) in composer.tracks().list().iter().enumerate() {
        tracks.push(TrackLayers {
            name: track.name(index + 1),
            layers: layers_on(looped.as_ref(), index),
        });
    }
    let mut playing = None;
    if let Some(place) = arrange::playing_now(composer, engine) {
        playing = Some(place + 1);
    }
    SectionsView {
        sections,
        order,
        playing,
        tracks,
        tempo: composer.tempo(),
    }
}

// The name of `key` shifted by `key_shift` half steps, like "D major".
fn shifted_key(key: Option<Key>, key_shift: i32) -> Option<String> {
    let key = key?;
    let tonic = (key.tonic as i32 + key_shift).rem_euclid(12) as u8;
    let shifted = Key {
        tonic,
        mode: key.mode,
    };
    Some(key_name(shifted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_file::{BeatNote, SongLayer};

    fn layer(track: usize, bars: u32, playing: bool) -> SongLayer {
        SongLayer {
            track,
            bars,
            volume: 80,
            playing,
            notes: vec![BeatNote {
                note: 60,
                start: 0.0,
                end: 1.0,
            }],
        }
    }

    // Two layers on track 1, the second paused, and one of 4 bars on track
    // 2, in a loop of 2 bars.
    fn looped() -> SongLoop {
        SongLoop {
            bars: 2,
            layers: vec![layer(0, 0, true), layer(0, 0, false), layer(1, 4, true)],
        }
    }

    fn used(track: usize, layer: usize) -> SectionLayer {
        SectionLayer { track, layer }
    }

    fn move_of(track: Option<usize>, value: &str, from: f64, to: f64) -> Automation {
        Automation { track, value: value.to_string(), from, to }
    }

    #[test]
    fn removing_each_bus_updates_automation_in_every_section() {
        let original = vec![
            move_of(None, "bus 1 volume", 1.0, 2.0),
            move_of(None, "bus 2 pan", 3.0, 4.0),
            move_of(None, "bus 3 effect 2 mix", 5.0, 6.0),
            move_of(Some(0), "send 1", 7.0, 8.0),
            move_of(Some(1), "send 2", 9.0, 10.0),
            move_of(Some(0), "send 3", 11.0, 12.0),
            move_of(Some(0), "effect 3 mix", 13.0, 14.0),
            move_of(None, "master effect 3 mix", 15.0, 16.0),
            move_of(None, "limiter boost_db", 17.0, 18.0),
            move_of(None, "tempo", 19.0, 20.0),
            move_of(Some(0), "pan", 21.0, 22.0),
        ];
        let section = SongSection {
            keys: None,
            name: String::from("Verse"), bars: 1, tempo: 120, key_shift: 0,
            layers: Vec::new(), automation: original.clone(),
        };
        for removed in 0..3 {
            let mut sections = Sections::new(vec![section.clone(), section.clone()], vec![0, 1]);
            sections.forget_bus(removed);
            let mut expected = original.clone();
            if removed == 0 {
                expected.remove(3);
                expected.remove(0);
                expected[0].value = String::from("bus 1 pan");
                expected[1].value = String::from("bus 2 effect 2 mix");
                expected[2].value = String::from("send 1");
                expected[3].value = String::from("send 2");
            } else if removed == 1 {
                expected.remove(4);
                expected.remove(1);
                expected[1].value = String::from("bus 2 effect 2 mix");
                expected[3].value = String::from("send 2");
            } else {
                expected.remove(5);
                expected.remove(2);
            }
            assert_eq!(sections.list()[0].automation, expected);
            assert_eq!(sections.list()[1].automation, expected);
            assert_eq!(sections.order(), &[0, 1]);
        }
    }

    #[test]
    fn bus_removal_stops_an_arrangement_only_after_validation() {
        let mut engine = Engine::for_render().unwrap();
        let mut composer = Composer::new();
        composer.send_all(&mut engine);
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        let looped = looped();
        open_song_loop(&mut engine, 120, Some(&looped)).unwrap();
        engine.render(512);
        let section = SongSection {
            keys: None,
            name: String::from("Verse"), bars: 2, tempo: 120, key_shift: 0,
            layers: vec![used(0, 0)],
            automation: vec![move_of(None, "bus 1 volume", 50.0, 80.0)],
        };
        composer.set_sections(vec![section], vec![0]);
        arrange::play(&mut composer, &mut engine).unwrap();
        engine.render(512);
        assert!(composer.arranging().is_some());
        assert!(engine.sequence_position().is_some());
        let before_mix = composer.mix_now();
        let before_sections = composer.sections().to_vec();
        assert!(composer.remove_bus(&mut engine, 1).is_err());
        assert!(composer.arranging().is_some());
        assert!(engine.sequence_position().is_some());
        assert_eq!(composer.mix_now(), before_mix);
        assert_eq!(composer.sections(), before_sections);
        composer.remove_bus(&mut engine, 0).unwrap();
        assert!(composer.arranging().is_none());
        assert_eq!(engine.sequence_position(), None);
        assert!(composer.sections()[0].automation.is_empty());
        assert!(composer.mix_now().buses.is_empty());
    }

    // Play a send on a track made after the mix was last fitted. The
    // reference explicitly clears the engine's send after removal.
    fn sound_after_removing_bus(send: Option<u32>) -> Vec<f32> {
        let mut engine = Engine::for_render().unwrap();
        let mut composer = Composer::new();
        composer.send_all(&mut engine);
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        composer.tracks_mut().add(&mut engine, crate::tracks::Track::new()).unwrap();
        let looped = looped();
        open_song_loop(&mut engine, 120, Some(&looped)).unwrap();
        engine.render(512);
        let section = SongSection {
            keys: None,
            name: String::from("Verse"), bars: 2, tempo: 120, key_shift: 0,
            layers: vec![used(1, 0)],
            automation: vec![move_of(Some(1), "send 1", 100.0, 100.0)],
        };
        composer.set_sections(vec![section], vec![0]);
        arrange::play(&mut composer, &mut engine).unwrap();
        engine.render(512);
        composer.remove_bus(&mut engine, 0).unwrap();
        assert!(composer.arranging().is_none());
        assert_eq!(engine.sequence_position(), None);
        assert!(composer.sections()[0].automation.is_empty());
        if let Some(send) = send {
            engine.set_send(1, 0, send);
        }
        engine.render(48_000);
        engine.on_track(1, |engine| engine.note_on(60, 40));
        engine.render(4_800)
    }

    #[test]
    fn removing_a_bus_clears_an_automated_send_on_a_new_track() {
        let actual = sound_after_removing_bus(None);
        let reference = sound_after_removing_bus(Some(0));
        let with_send = sound_after_removing_bus(Some(100));
        let mut difference = 0.0f64;
        let mut send_difference = 0.0f64;
        let mut level = 0.0f64;
        for ((sample, expected), sent) in actual.iter().zip(&reference).zip(&with_send) {
            difference += f64::from((sample - expected).abs());
            send_difference += f64::from((sent - expected).abs());
            level += f64::from(expected.abs());
        }
        assert!(level > 0.0);
        assert!(difference < level * 0.01);
        assert!(send_difference > level * 0.1);
    }

    #[test]
    fn saving_keeps_the_playing_layers_for_the_longest_one_s_bars() {
        let mut sections = Sections::default();
        let looped = looped();
        assert_eq!(sections.save(Some(&looped), 100), Ok(1));
        assert_eq!(sections.save(Some(&looped), 100), Ok(2));
        let first = &sections.list()[0];
        assert_eq!(first.name, "Section 1");
        assert_eq!(first.bars, 4);
        assert_eq!(first.tempo, 100);
        assert_eq!(first.layers, vec![used(0, 0), used(1, 0)]);
        assert_eq!(sections.list()[1].name, "Section 2");
        assert_eq!(sections.order(), &[0, 1]);
    }

    #[test]
    fn saving_needs_layers_playing() {
        let mut sections = Sections::default();
        let refused = sections.save(None, 120);
        assert_eq!(refused, Err(String::from("there is no loop to save a section from")));
        let mut paused = looped();
        for layer in &mut paused.layers {
            layer.playing = false;
        }
        assert!(sections.save(Some(&paused), 120).unwrap_err().starts_with("no layers are playing"));
        assert!(sections.list().is_empty());
    }

    #[test]
    fn a_new_name_skips_the_ones_taken() {
        let mut sections = Sections::default();
        let looped = looped();
        sections.save(Some(&looped), 120).unwrap();
        sections.save(Some(&looped), 120).unwrap();
        sections.rename(2, "Section 3").unwrap();
        sections.rename(1, "Verse").unwrap();
        sections.save(Some(&looped), 120).unwrap();
        assert_eq!(sections.list()[2].name, "Section 1");
        sections.save(Some(&looped), 120).unwrap();
        assert_eq!(sections.list()[3].name, "Section 2");
    }

    #[test]
    fn names_are_unique_and_short_and_values_in_range() {
        let mut sections = Sections::default();
        let looped = looped();
        sections.save(Some(&looped), 120).unwrap();
        sections.save(Some(&looped), 120).unwrap();
        assert_eq!(sections.rename(2, " Section 1 "), Err(String::from("another section is called Section 1")));
        assert_eq!(sections.rename(1, "  "), Err(String::from("a section needs a name")));
        assert!(sections.rename(1, &"a".repeat(41)).is_err());
        assert_eq!(sections.rename(1, " Section 1 "), Ok(()));
        assert_eq!(sections.rename(3, "Bridge"), Err(String::from("there is no section 3")));

        assert!(sections.set_bars(1, 0).is_err());
        assert!(sections.set_bars(1, 65).is_err());
        assert_eq!(sections.set_bars(1, 64), Ok(()));
        assert!(sections.set_tempo(1, 39).is_err());
        assert_eq!(sections.set_tempo(1, 240), Ok(()));
        assert!(sections.set_key_shift(1, 13).is_err());
        assert!(sections.set_key_shift(1, -13).is_err());
        assert_eq!(sections.set_key_shift(1, -12), Ok(()));
        let first = &sections.list()[0];
        assert_eq!((first.bars, first.tempo, first.key_shift), (64, 240, -12));
    }

    #[test]
    fn layers_go_on_and_off_only_if_the_loop_has_them() {
        let mut sections = Sections::default();
        let looped = looped();
        sections.save(Some(&looped), 120).unwrap();
        sections.set_layer(1, 1, 2, true, Some(&looped)).unwrap();
        assert_eq!(sections.list()[0].layers, vec![used(0, 0), used(0, 1), used(1, 0)]);
        sections.set_layer(1, 1, 1, false, Some(&looped)).unwrap();
        assert_eq!(sections.list()[0].layers, vec![used(0, 1), used(1, 0)]);
        let refused = sections.set_layer(1, 2, 2, true, Some(&looped));
        assert_eq!(refused, Err(String::from("there is no layer on track 2 2")));
        assert!(sections.set_layer(1, 10, 1, true, Some(&looped)).is_err());
    }

    #[test]
    fn the_lane_moves_reuses_and_removes_places() {
        let mut sections = Sections::default();
        let looped = looped();
        sections.save(Some(&looped), 120).unwrap();
        sections.save(Some(&looped), 120).unwrap();
        sections.save(Some(&looped), 120).unwrap();
        sections.move_use(3, 1).unwrap();
        assert_eq!(sections.order(), &[2, 0, 1]);
        sections.use_again(1).unwrap();
        assert_eq!(sections.order(), &[2, 2, 0, 1]);
        assert!(sections.move_use(5, 1).is_err());

        // A section still used elsewhere stays.
        sections.remove_use(1).unwrap();
        assert_eq!(sections.order(), &[2, 0, 1]);
        assert_eq!(sections.list().len(), 3);

        // One used nowhere goes, and the places count again.
        sections.remove_use(2).unwrap();
        assert_eq!(sections.list().len(), 2);
        assert_eq!(sections.list()[0].name, "Section 2");
        assert_eq!(sections.order(), &[1, 0]);

        sections.remove_last().unwrap();
        sections.remove_last().unwrap();
        assert_eq!(sections.remove_last(), Err(String::from("there are no sections to remove")));
        assert!(sections.list().is_empty());
    }

    #[test]
    fn a_removed_layer_leaves_every_section_and_the_later_ones_move_down() {
        let looped = SongLoop {
            bars: 1,
            layers: vec![layer(0, 0, true), layer(0, 0, true), layer(0, 0, true), layer(1, 0, true)],
        };
        let mut sections = Sections::default();
        sections.save(Some(&looped), 120).unwrap();
        sections.forget_layer(0, 1);
        assert_eq!(sections.list()[0].layers, vec![used(0, 0), used(0, 1), used(1, 0)]);
    }

    #[test]
    fn a_value_moves_once_per_section() {
        let mut sections = Sections::default();
        sections.save(Some(&looped()), 120).unwrap();
        let moved = |from: f64| Automation {
            track: Some(0),
            value: String::from("filter cutoff_hz"),
            from,
            to: 8000.0,
        };
        sections.automate(1, moved(200.0)).unwrap();
        sections.automate(1, moved(400.0)).unwrap();
        assert_eq!(sections.list()[0].automation, vec![moved(400.0)]);
        sections.stop_automating(1, Some(0), "filter cutoff_hz").unwrap();
        assert!(sections.list()[0].automation.is_empty());
        let refused = sections.stop_automating(1, Some(0), "filter cutoff_hz");
        assert_eq!(refused, Err(String::from("Section 1 doesn't move filter cutoff_hz")));
    }

    // Composer and engine, with a loop of two layers on track 1, the
    // second paused, at 120.
    fn playing() -> (Composer, Engine) {
        let mut engine = Engine::new().unwrap();
        let composer = Composer::new();
        composer.send_all(&mut engine);
        let looped = SongLoop {
            bars: 1,
            layers: vec![layer(0, 0, true), layer(0, 0, false)],
        };
        open_song_loop(&mut engine, 120, Some(&looped)).unwrap();
        engine.render(512);
        (composer, engine)
    }

    #[test]
    fn editing_saves_and_opens_sections_in_the_looper() {
        let (mut composer, mut engine) = playing();
        edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        edit(&mut composer, &mut engine, SectionEdit::Tempo { section: 1, tempo: 90 }).unwrap();
        let shown = view(&mut composer, &mut engine);
        assert_eq!(shown.order, vec![1]);
        assert_eq!(shown.sections[0].layers, vec![(1, 1)]);
        assert_eq!(shown.tracks[0].layers, 2);

        // Opening turns the section's layers on and the rest off, at its
        // tempo.
        let both = SectionEdit::Layer {
            section: 1,
            track: 1,
            layer: 2,
            on: true,
        };
        edit(&mut composer, &mut engine, both).unwrap();
        let only_first = SectionEdit::Layer {
            section: 1,
            track: 1,
            layer: 1,
            on: false,
        };
        edit(&mut composer, &mut engine, only_first).unwrap();
        edit(&mut composer, &mut engine, SectionEdit::Open { section: 1 }).unwrap();
        engine.render(512);
        let looped = loop_now(&mut engine).unwrap();
        assert!(!looped.layers[0].playing);
        assert!(looped.layers[1].playing);
        assert_eq!(composer.tempo(), 90);
    }

    #[test]
    fn a_layer_removed_with_esc_leaves_the_sections() {
        let (mut composer, mut engine) = playing();
        edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        let second = SectionEdit::Layer {
            section: 1,
            track: 1,
            layer: 2,
            on: true,
        };
        edit(&mut composer, &mut engine, second).unwrap();
        engine.remove_layer(1);
        engine.render(512);
        let shown = view(&mut composer, &mut engine);
        assert_eq!(shown.sections[0].layers, vec![(1, 1)]);
        assert_eq!(shown.tracks[0].layers, 1);
    }

    #[test]
    fn mix_values_are_automated_on_a_track_or_with_no_track() {
        let (mut composer, mut engine) = playing();
        edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        let automate = |track: Option<usize>, value: &str| SectionEdit::Automate {
            section: 1,
            track,
            value: String::from(value),
            from: 0.0,
            to: 50.0,
        };
        let refused = edit(&mut composer, &mut engine, automate(None, "bus 1 volume"));
        assert_eq!(refused.unwrap_err(), "the song has no bus 1");
        composer.mix_mut().add_bus(&mut engine, None).unwrap();
        edit(&mut composer, &mut engine, automate(None, "bus 1 volume")).unwrap();
        edit(&mut composer, &mut engine, automate(Some(1), "pan")).unwrap();
        edit(&mut composer, &mut engine, automate(Some(1), "send 1")).unwrap();
        let refused = edit(&mut composer, &mut engine, automate(Some(1), "effect 1 mix"));
        assert_eq!(refused.unwrap_err(), "slot 1 on track 1 is empty");

        let shown = view(&mut composer, &mut engine);
        let values: Vec<&str> = shown.sections[0].automation.iter().map(|moved| moved.value.as_str()).collect();
        assert_eq!(values, vec!["bus 1 volume", "pan", "send 1"]);
    }

    #[test]
    fn automation_is_checked_against_the_track_s_sound() {
        let (mut composer, mut engine) = playing();
        edit(&mut composer, &mut engine, SectionEdit::Save).unwrap();
        let cutoff = SectionEdit::Automate {
            section: 1,
            track: Some(1),
            value: String::from(" filter cutoff_hz "),
            from: 200.0,
            to: 8000.0,
        };
        edit(&mut composer, &mut engine, cutoff).unwrap();
        let tempo = SectionEdit::Automate {
            section: 1,
            track: None,
            value: String::new(),
            from: 100.0,
            to: 140.0,
        };
        edit(&mut composer, &mut engine, tempo).unwrap();
        let shown = view(&mut composer, &mut engine);
        let values: Vec<&str> = shown.sections[0].automation.iter().map(|moved| moved.value.as_str()).collect();
        assert_eq!(values, vec!["filter cutoff_hz", "tempo"]);

        let too_fast = SectionEdit::Automate {
            section: 1,
            track: None,
            value: String::new(),
            from: 100.0,
            to: 300.0,
        };
        assert!(edit(&mut composer, &mut engine, too_fast).is_err());
        let unknown = SectionEdit::Automate {
            section: 1,
            track: Some(1),
            value: String::from("filter warmth"),
            from: 1.0,
            to: 2.0,
        };
        assert!(edit(&mut composer, &mut engine, unknown).is_err());
        let no_track = SectionEdit::Automate {
            section: 1,
            track: Some(2),
            value: String::from("volume"),
            from: 1.0,
            to: 2.0,
        };
        assert_eq!(edit(&mut composer, &mut engine, no_track), Err(String::from("there is no track 2")));
    }

    #[test]
    fn a_section_s_key_follows_its_shift() {
        let c_major = Key {
            tonic: 0,
            mode: crate::harmony::Mode::Major,
        };
        assert_eq!(shifted_key(Some(c_major), 2), Some(String::from("D major")));
        assert_eq!(shifted_key(Some(c_major), -1), Some(String::from("B major")));
        assert_eq!(shifted_key(Some(c_major), 12), Some(String::from("C major")));
        assert_eq!(shifted_key(None, 2), None);
    }

    #[test]
    fn edits_read_as_the_page_and_the_agent_send_them() {
        let text = r#"{"action": "key_shift", "section": 2, "key_shift": -3}"#;
        let read: SectionEdit = serde_json::from_str(text).unwrap();
        assert_eq!(read, SectionEdit::KeyShift { section: 2, key_shift: -3 });
        let text = r#"{"action": "automate", "section": 1, "value": "tempo", "from": 90, "to": 120}"#;
        let read: SectionEdit = serde_json::from_str(text).unwrap();
        let expected = SectionEdit::Automate {
            section: 1,
            track: None,
            value: String::from("tempo"),
            from: 90.0,
            to: 120.0,
        };
        assert_eq!(read, expected);
    }
}
