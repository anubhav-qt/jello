// The song's mix (docs/adr/0046-the-mix.md, docs/design.md#mix): each
// track's pan, sends and effect slots, the send buses, and the master's
// slots and limiter. Every change goes on to the engine as it is made, and
// the whole mix is sent when a song opens.
//
// A track's volume stays with the track (src/tracks.rs). The mix keeps one
// strip for each of the song's tracks, in the same order. A track made
// since has a bare strip until the composer fits the mix to the tracks
// again (fit_tracks), which matches the engine: chains no track uses are
// kept empty. Removing a track goes through remove_track.
//
// "Listens to" (the sidechain) is checked here: a track may listen to the
// other tracks, a bus to the tracks and the other buses, the master to any
// of them, and no chain may end up listening to itself through others. The
// engine runs every source before the chains listening to it, in the order
// chain_order works out.

use crate::effects::{Sort, Written, kind_of_word, kind_words, listens_to_word, number_of, values_of, written_of};
use crate::engine::{
    BUS_COUNT, EFFECT_SLOTS, EffectConnection, EffectKind, Engine, FIRST_BUS_CHAIN, FULL_VOLUME, LISTEN_TO_OWN,
    Limiter, MASTER_CHAIN, START_LIMITER, TRACK_COUNT,
};
use crate::modulation::{CONNECTION_COUNT, Connection, LFO_COUNT, Lfo, START_LFO, Source};

// Effect assignments belong to the mix. Their engine routing indices are
// derived from the track's ordinary connections when they are sent.
pub type EffectLink = EffectConnection;

#[derive(Clone, Debug, PartialEq)]
pub struct EffectModulation {
    pub lfos: [Lfo; LFO_COUNT],
    pub links: Vec<EffectLink>,
}

impl EffectModulation {
    pub fn new() -> EffectModulation {
        EffectModulation { lfos: [START_LFO; LFO_COUNT], links: Vec::new() }
    }
}

// Source None alone denotes an unused ordinary slot, even when the amount
// of an occupied connection is zero.
pub fn check_track_routing(connections: &[Connection; CONNECTION_COUNT], links: &[EffectLink]) -> Result<(), String> {
    let occupied = connections.iter().filter(|connection| connection.source != Source::None).count();
    if occupied + links.len() > CONNECTION_COUNT {
        return Err(format!("the track has more than {CONNECTION_COUNT} sound and effect connections"));
    }
    Ok(())
}

// Send every non-sound slot, including empty ones, so replacing or removing
// an assignment cannot leave an old effect connection in the engine.
pub fn send_track_routing(engine: &mut Engine, track: usize, connections: &[Connection; CONNECTION_COUNT], links: &[EffectLink]) {
    let mut next = links.iter();
    for (index, connection) in connections.iter().enumerate() {
        if connection.source != Source::None {
            continue;
        }
        let link = next.next().copied().unwrap_or(empty_link());
        engine.set_effect_connection(track, index, link);
    }
}

fn empty_link() -> EffectLink {
    EffectLink { source: Source::None, slot: 0, value: 0, amount: 0.0, one_way: false, scaled_by: Source::None }
}

pub fn effect_value_can_move(number: i32) -> bool {
    for word in kind_words() {
        let Some(kind) = kind_of_word(word) else { continue; };
        for value in values_of(kind, false) {
            if value.number != number {
                continue;
            }
            match value.sort {
                Sort::Switch | Sort::Choice(_) | Sort::Steps(_) | Sort::ListensTo => {},
                _ => return true,
            }
        }
    }
    false
}

// Where a chain of effects is: on a track, on a bus, both from 0, or on the
// master.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Track(usize),
    Bus(usize),
    Master,
}

impl Place {
    // Its chain's number in the engine.
    pub fn chain(self) -> usize {
        match self {
            Place::Track(track) => track,
            Place::Bus(bus) => FIRST_BUS_CHAIN + bus,
            Place::Master => MASTER_CHAIN,
        }
    }

    // A chorus, delay or reverb starts fully wet on a bus.
    pub fn is_bus(self) -> bool {
        matches!(self, Place::Bus(_))
    }

    // The place `word` names, counted from 1: "track 2", "bus 1" or
    // "master". None for anything else. Whether the song has it is the
    // mix's to say.
    pub fn of_word(word: &str) -> Option<Place> {
        let word = word.trim();
        if word == "master" {
            return Some(Place::Master);
        }
        if let Some(number) = word.strip_prefix("track ") {
            let number: usize = number.trim().parse().ok()?;
            if number == 0 {
                return None;
            }
            return Some(Place::Track(number - 1));
        }
        if let Some(number) = word.strip_prefix("bus ") {
            let number: usize = number.trim().parse().ok()?;
            if number == 0 {
                return None;
            }
            return Some(Place::Bus(number - 1));
        }
        None
    }

    // What to call it, counted from 1: "track 2", "bus 1" or "master".
    pub fn name(self) -> String {
        match self {
            Place::Track(track) => format!("track {}", track + 1),
            Place::Bus(bus) => format!("bus {}", bus + 1),
            Place::Master => String::from("master"),
        }
    }
}

// The place of chain `chain` that can be listened to: a track's or a bus's.
fn place_of_chain(chain: usize) -> Place {
    if chain >= FIRST_BUS_CHAIN {
        return Place::Bus(chain - FIRST_BUS_CHAIN);
    }
    Place::Track(chain)
}

// One slot's effect: its kind, whether it is on, and every value it has, in
// the order effects::values_of gives them, as the engine takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub kind: EffectKind,
    pub on: bool,
    pub values: Vec<f32>,
}

impl Slot {
    // A new effect of `kind`, on and at its starting values, as the engine
    // makes it at `place`.
    pub fn new(kind: EffectKind, place: Place) -> Slot {
        let mut values = Vec::new();
        for value in values_of(kind, place.is_bus()) {
            values.push(value.start);
        }
        Slot { kind, on: true, values }
    }

    // Where its "listens to" value is among its values, if it has one.
    fn listens_to_place(&self) -> Option<usize> {
        let values = values_of(self.kind, false);
        values.iter().position(|value| value.word == "listens_to")
    }

    // The chain it listens to, if it listens to one, not its own input.
    pub fn listens_to(&self) -> Option<usize> {
        let place = self.listens_to_place()?;
        let chain = self.values[place];
        if chain == LISTEN_TO_OWN {
            return None;
        }
        Some(chain as usize)
    }
}

// A chain's slots, from the top: the sound goes through them in order.
pub type Chain = [Option<Slot>; EFFECT_SLOTS];

// A chain with every slot empty.
pub fn empty_chain() -> Chain {
    std::array::from_fn(|_| None)
}

// A track's part of the mix: its pan, how much of it goes to each bus, and
// its effects.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    // From -PAN_MOST, fully left, to PAN_MOST, fully right.
    pub pan: i32,
    // In percent, from 0 to FULL_VOLUME, one for each bus there could be.
    // Only the song's buses can have one.
    pub sends: [u32; BUS_COUNT],
    pub chain: Chain,
    pub links: Vec<EffectLink>,
}

impl Strip {
    // In the middle, sending nothing, with no effects.
    pub fn new() -> Strip {
        Strip {
            pan: 0,
            sends: [0; BUS_COUNT],
            chain: empty_chain(),
            links: Vec::new(),
        }
    }
}

// A send bus: shared effects, like one reverb for every track.
#[derive(Clone, Debug, PartialEq)]
pub struct Bus {
    // The name it was given, or None for "Bus 1".
    pub name: Option<String>,
    // In percent, from 0 to FULL_VOLUME.
    pub volume: u32,
    pub pan: i32,
    pub chain: Chain,
    pub modulation: EffectModulation,
}

impl Bus {
    // At full volume, in the middle, with no effects.
    pub fn new(name: Option<String>) -> Bus {
        Bus {
            name,
            volume: FULL_VOLUME,
            pan: 0,
            chain: empty_chain(),
            modulation: EffectModulation::new(),
        }
    }
}

// The song's whole mix.
#[derive(Clone, Debug, PartialEq)]
pub struct Mix {
    // One for each of the song's tracks, in order.
    pub tracks: Vec<Strip>,
    // Up to BUS_COUNT; a new song has none.
    pub buses: Vec<Bus>,
    pub master: Chain,
    pub master_modulation: EffectModulation,
    pub limiter: Limiter,
}

impl Mix {
    // A new song's mix for `track_count` tracks: nothing on any of them, no
    // buses, and the limiter where a new engine's starts.
    pub fn new(track_count: usize) -> Mix {
        let mut tracks = Vec::new();
        for _ in 0..track_count {
            tracks.push(Strip::new());
        }
        Mix {
            tracks,
            buses: Vec::new(),
            master: empty_chain(),
            master_modulation: EffectModulation::new(),
            limiter: START_LIMITER,
        }
    }

    // The chain at `place`, or None if the song has no such track or bus.
    pub fn chain(&self, place: Place) -> Option<&Chain> {
        match place {
            Place::Track(track) => self.tracks.get(track).map(|strip| &strip.chain),
            Place::Bus(bus) => self.buses.get(bus).map(|bus| &bus.chain),
            Place::Master => Some(&self.master),
        }
    }

    fn chain_mut(&mut self, place: Place) -> Option<&mut Chain> {
        match place {
            Place::Track(track) => self.tracks.get_mut(track).map(|strip| &mut strip.chain),
            Place::Bus(bus) => self.buses.get_mut(bus).map(|bus| &mut bus.chain),
            Place::Master => Some(&mut self.master),
        }
    }

    pub fn links(&self, place: Place) -> Option<&[EffectLink]> {
        match place {
            Place::Track(track) => self.tracks.get(track).map(|strip| strip.links.as_slice()),
            Place::Bus(bus) => self.buses.get(bus).map(|bus| bus.modulation.links.as_slice()),
            Place::Master => Some(&self.master_modulation.links),
        }
    }

    pub fn links_mut(&mut self, place: Place) -> Option<&mut Vec<EffectLink>> {
        match place {
            Place::Track(track) => self.tracks.get_mut(track).map(|strip| &mut strip.links),
            Place::Bus(bus) => self.buses.get_mut(bus).map(|bus| &mut bus.modulation.links),
            Place::Master => Some(&mut self.master_modulation.links),
        }
    }

    pub fn modulation(&self, place: Place) -> Option<&EffectModulation> {
        match place {
            Place::Track(_) => None,
            Place::Bus(bus) => self.buses.get(bus).map(|bus| &bus.modulation),
            Place::Master => Some(&self.master_modulation),
        }
    }

    pub fn modulation_mut(&mut self, place: Place) -> Option<&mut EffectModulation> {
        match place {
            Place::Track(_) => None,
            Place::Bus(bus) => self.buses.get_mut(bus).map(|bus| &mut bus.modulation),
            Place::Master => Some(&mut self.master_modulation),
        }
    }

    // Validate a newly assigned target. Existing links stay on their slot
    // even if the effect in that slot is replaced or removed.
    pub fn check_effect_link(&self, place: Place, link: EffectLink) -> Result<(), String> {
        match link.source {
            Source::Lfo(index) if index < LFO_COUNT => {},
            _ => return Err(String::from("an effect connection needs an LFO source from 1 to 8")),
        }
        match link.scaled_by {
            Source::None => {},
            Source::Lfo(index) if index < LFO_COUNT => {},
            _ => return Err(String::from("an effect connection can only be scaled by an LFO")),
        }
        if !link.amount.is_finite() || link.amount.abs() > 20_000.0 {
            return Err(String::from("effect connection amount must be from -20000 to 20000"));
        }
        let effect = self.slot(place, link.slot)?;
        let values = values_of(effect.kind, place.is_bus());
        let Some(value) = values.iter().find(|value| value.number == link.value) else {
            return Err(String::from("the effect has no such value"));
        };
        match value.sort {
            Sort::Switch | Sort::Choice(_) | Sort::Steps(_) | Sort::ListensTo => {
                return Err(String::from("that effect value cannot be moved by an LFO"));
            }
            _ => {}
        }
        Ok(())
    }

    // Every place the song has: its tracks, its buses, then the master.
    pub fn places(&self) -> Vec<Place> {
        let mut places = Vec::new();
        for track in 0..self.tracks.len() {
            places.push(Place::Track(track));
        }
        for bus in 0..self.buses.len() {
            places.push(Place::Bus(bus));
        }
        places.push(Place::Master);
        places
    }

    // The effect in slot `slot` at `place`, or an error saying what is
    // missing.
    pub fn slot(&self, place: Place, slot: usize) -> Result<&Slot, String> {
        let Some(chain) = self.chain(place) else {
            return Err(format!("the song has no {}", place.name()));
        };
        let Some(held) = chain.get(slot) else {
            return Err(format!("there are {EFFECT_SLOTS} slots, from 1 to {EFFECT_SLOTS}"));
        };
        let Some(effect) = held else {
            return Err(format!("slot {} on {} is empty", slot + 1, place.name()));
        };
        Ok(effect)
    }

    // Puts a new effect of `kind`, on and at its starting values, in slot
    // `slot` at `place`, on `engine` too, replacing what was there. None
    // empties the slot.
    pub fn place_effect(&mut self, engine: &mut Engine, place: Place, slot: usize, kind: EffectKind) -> Result<(), String> {
        let Some(chain) = self.chain_mut(place) else {
            return Err(format!("the song has no {}", place.name()));
        };
        if slot >= EFFECT_SLOTS {
            return Err(format!("there are {EFFECT_SLOTS} slots, from 1 to {EFFECT_SLOTS}"));
        }
        if !engine.set_effect(place.chain(), slot, kind) {
            return Err(String::from("the engine is busy, try again"));
        }
        if kind == EffectKind::None {
            chain[slot] = None;
        } else {
            chain[slot] = Some(Slot::new(kind, place));
        }
        // What was there may have listened to another chain.
        engine.set_chain_order(&self.chain_order());
        Ok(())
    }

    // Moves slot `from`'s effect at `place` to slot `to`, on `engine` too,
    // shifting the slots between by one.
    pub fn move_effect(&mut self, engine: &mut Engine, place: Place, from: usize, to: usize) -> Result<(), String> {
        let Some(chain) = self.chain_mut(place) else {
            return Err(format!("the song has no {}", place.name()));
        };
        if from >= EFFECT_SLOTS || to >= EFFECT_SLOTS {
            return Err(format!("there are {EFFECT_SLOTS} slots, from 1 to {EFFECT_SLOTS}"));
        }
        if !engine.move_effect(place.chain(), from, to) {
            return Err(String::from("the engine is busy, try again"));
        }
        if from < to {
            chain[from..=to].rotate_left(1);
        } else {
            chain[to..=from].rotate_right(1);
        }
        Ok(())
    }

    // Switches the effect in slot `slot` at `place` on or off, on `engine`
    // too.
    pub fn set_effect_on(&mut self, engine: &mut Engine, place: Place, slot: usize, on: bool) -> Result<(), String> {
        self.slot(place, slot)?;
        let chain = self.chain_mut(place).unwrap();
        let effect = chain[slot].as_mut().unwrap();
        effect.on = on;
        engine.set_effect_on(place.chain(), slot, on);
        Ok(())
    }

    // Sets the value called `word` of the effect in slot `slot` at `place`
    // to `written`, kept within its range, on `engine` too. Returns it as
    // set. "Listens to" must name a chain this place may listen to, without
    // making a loop.
    pub fn set_effect_value(
        &mut self,
        engine: &mut Engine,
        place: Place,
        slot: usize,
        word: &str,
        written: &Written,
    ) -> Result<Written, String> {
        let number = self.checked_effect_value(place, slot, word, written)?;
        let effect = self.slot(place, slot)?;
        let values = values_of(effect.kind, place.is_bus());
        let index = values.iter().position(|value| value.word == word).unwrap();
        let listens = Some(index) == effect.listens_to_place();

        self.set_stored_value(place, slot, index, number);
        let value = &values[index];
        engine.set_effect_value(place.chain(), slot, value.number, number);
        if listens {
            engine.set_chain_order(&self.chain_order());
        }
        Ok(written_of(value, number))
    }

    // The number, in the engine's units, that setting the value called
    // `word` of the effect in slot `slot` at `place` to `written` would
    // set, changing nothing. An error says why it can't be set.
    pub fn checked_effect_value(&self, place: Place, slot: usize, word: &str, written: &Written) -> Result<f32, String> {
        let effect = self.slot(place, slot)?;
        let values = values_of(effect.kind, place.is_bus());
        let Some(index) = values.iter().position(|value| value.word == word) else {
            let mut words = Vec::new();
            for value in &values {
                words.push(value.word.as_str());
            }
            return Err(format!("this effect has no {word}: it has {}", words.join(", ")));
        };
        let number = number_of(&values[index], written)?;
        let listens = Some(index) == effect.listens_to_place();
        if listens && number != LISTEN_TO_OWN {
            self.check_listening(place, slot, number as usize)?;
        }
        Ok(number)
    }

    // Whether the effect in slot `slot` at `place` may listen to chain
    // `chain`: an error says why not.
    fn check_listening(&self, place: Place, slot: usize, chain: usize) -> Result<(), String> {
        let source = place_of_chain(chain);
        if self.chain(source).is_none() {
            return Err(format!("the song has no {}", source.name()));
        }
        if source == place {
            return Err(format!("{} can't listen to itself", place.name()));
        }
        let Place::Track(_) = place else {
            // A bus listening to a bus could close a loop; the master never.
            if place.is_bus() && source.is_bus() {
                return self.check_no_loop(place, slot, source);
            }
            return Ok(());
        };
        if source.is_bus() {
            return Err(String::from("a track can only listen to other tracks"));
        }
        self.check_no_loop(place, slot, source)
    }

    // An error if `source` already hears `place`, through any chain it
    // listens to, leaving out slot `slot` at `place`, which is about to
    // change.
    fn check_no_loop(&self, place: Place, slot: usize, source: Place) -> Result<(), String> {
        let mut to_visit = vec![source];
        let mut visited = Vec::new();
        while let Some(visiting) = to_visit.pop() {
            if visited.contains(&visiting) {
                continue;
            }
            visited.push(visiting);
            for heard in self.heard_by(visiting, Some((place, slot))) {
                if heard == place {
                    let message = format!(
                        "that would make a loop: {} already hears {}",
                        source.name(),
                        place.name()
                    );
                    return Err(message);
                }
                to_visit.push(heard);
            }
        }
        Ok(())
    }

    // The places whose sound `place`'s effects listen to, leaving out the
    // slot `skipped` names.
    fn heard_by(&self, place: Place, skipped: Option<(Place, usize)>) -> Vec<Place> {
        let mut heard = Vec::new();
        let Some(chain) = self.chain(place) else {
            return heard;
        };
        for (slot, held) in chain.iter().enumerate() {
            if skipped == Some((place, slot)) {
                continue;
            }
            let Some(effect) = held else {
                continue;
            };
            if let Some(source) = effect.listens_to() {
                heard.push(place_of_chain(source));
            }
        }
        heard
    }

    // The order the engine runs the chains before the master in: every
    // track's chain, then every bus's, each after the chains it listens to,
    // the lowest number first where there is a choice.
    pub fn chain_order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        let mut tracks = Vec::new();
        for track in 0..TRACK_COUNT {
            tracks.push(Place::Track(track));
        }
        let mut buses = Vec::new();
        for bus in 0..BUS_COUNT {
            buses.push(Place::Bus(bus));
        }
        for group in [tracks, buses] {
            let mut left = group;
            while !left.is_empty() {
                let ready = self.first_ready(&left, &order);
                order.push(left[ready].chain());
                left.remove(ready);
            }
        }
        order
    }

    // Where in `left` the first place is whose sources have all run, in
    // `done`. A loop, which check_listening never lets in, would leave none:
    // the first then runs anyway.
    fn first_ready(&self, left: &[Place], done: &[usize]) -> usize {
        for (index, place) in left.iter().enumerate() {
            let mut ready = true;
            for heard in self.heard_by(*place, None) {
                let is_waiting = left.contains(&heard);
                if is_waiting && !done.contains(&heard.chain()) {
                    ready = false;
                }
            }
            if ready {
                return index;
            }
        }
        0
    }

    // Sets the pan of `place`, a track or a bus, from -PAN_MOST to
    // PAN_MOST, on `engine` too. Returns it as set.
    pub fn set_pan(&mut self, engine: &mut Engine, place: Place, pan: i32) -> Result<i32, String> {
        match place {
            Place::Track(track) => {
                let Some(strip) = self.tracks.get_mut(track) else {
                    return Err(format!("the song has no {}", place.name()));
                };
                strip.pan = engine.set_track_pan(track, pan);
                Ok(strip.pan)
            }
            Place::Bus(bus) => {
                let Some(held) = self.buses.get_mut(bus) else {
                    return Err(format!("the song has no {}", place.name()));
                };
                held.pan = engine.set_bus_pan(bus, pan);
                Ok(held.pan)
            }
            Place::Master => Err(String::from("the master has no pan")),
        }
    }

    // Sends `percent` of track `track`'s sound to bus `bus`, on `engine`
    // too. Returns it as set.
    pub fn set_send(&mut self, engine: &mut Engine, track: usize, bus: usize, percent: u32) -> Result<u32, String> {
        if bus >= self.buses.len() {
            return Err(format!("the song has no {}", Place::Bus(bus).name()));
        }
        let Some(strip) = self.tracks.get_mut(track) else {
            return Err(format!("the song has no {}", Place::Track(track).name()));
        };
        strip.sends[bus] = engine.set_send(track, bus, percent);
        Ok(strip.sends[bus])
    }

    // Sets bus `bus`'s volume, on `engine` too. Returns it as set.
    pub fn set_bus_volume(&mut self, engine: &mut Engine, bus: usize, percent: u32) -> Result<u32, String> {
        let Some(held) = self.buses.get_mut(bus) else {
            return Err(format!("the song has no {}", Place::Bus(bus).name()));
        };
        held.volume = engine.set_bus_volume(bus, percent);
        Ok(held.volume)
    }

    // Names bus `bus` `name`, or with None or a blank name, "Bus 2".
    pub fn rename_bus(&mut self, bus: usize, name: Option<&str>) -> Result<(), String> {
        let Some(held) = self.buses.get_mut(bus) else {
            return Err(format!("the song has no {}", Place::Bus(bus).name()));
        };
        held.name = None;
        if let Some(name) = name {
            let name = name.trim();
            if !name.is_empty() {
                held.name = Some(String::from(name));
            }
        }
        Ok(())
    }

    // Bus `bus`'s name: the one it was given, or "Bus 2".
    pub fn bus_name(&self, bus: usize) -> String {
        if let Some(held) = self.buses.get(bus) {
            if let Some(name) = &held.name {
                return name.clone();
            }
        }
        format!("Bus {}", bus + 1)
    }

    // Adds a bus after the last, named `name`, on `engine` too. Returns its
    // number, from 0.
    pub fn add_bus(&mut self, engine: &mut Engine, name: Option<&str>) -> Result<usize, String> {
        if self.buses.len() == BUS_COUNT {
            return Err(format!("a song has at most {BUS_COUNT} buses"));
        }
        let bus = self.buses.len();
        self.buses.push(Bus::new(None));
        self.rename_bus(bus, name)?;
        send_bus(engine, bus, &self.buses[bus]);
        Ok(bus)
    }

    // Removes bus `bus`, on `engine` too. The buses after it move one down,
    // with their sends. Effects that listened to it listen to their own
    // input again.
    pub fn remove_bus(&mut self, engine: &mut Engine, bus: usize) -> Result<(), String> {
        if bus >= self.buses.len() {
            return Err(format!("the song has no {}", Place::Bus(bus).name()));
        }
        self.buses.remove(bus);
        for strip in &mut self.tracks {
            strip.sends[bus..].rotate_left(1);
            strip.sends[BUS_COUNT - 1] = 0;
        }
        self.renumber_listening(engine, Place::Bus(bus));

        for (track, strip) in self.tracks.iter().enumerate() {
            for each in bus..BUS_COUNT {
                engine.set_send(track, each, strip.sends[each]);
            }
        }
        for each in bus..BUS_COUNT {
            if let Some(held) = self.buses.get(each) {
                send_bus(engine, each, held);
            } else {
                send_bus(engine, each, &Bus::new(None));
            }
        }
        engine.set_chain_order(&self.chain_order());
        Ok(())
    }

    // Sets the master limiter, each value kept within its limits, on
    // `engine` too. Returns it as set.
    pub fn set_limiter(&mut self, engine: &mut Engine, limiter: Limiter) -> Limiter {
        self.limiter = engine.set_limiter(limiter);
        self.limiter
    }

    // Removes track `track`'s strip, on `engine` too. The strips after it
    // move one down. Effects that listened to it listen to their own input
    // again.
    pub fn remove_track(&mut self, engine: &mut Engine, track: usize) {
        if track >= self.tracks.len() {
            return;
        }
        self.tracks.remove(track);
        self.renumber_listening(engine, Place::Track(track));
        for each in track..TRACK_COUNT {
            if let Some(strip) = self.tracks.get(each) {
                send_strip(engine, each, strip, self.buses.len());
            } else {
                send_strip(engine, each, &Strip::new(), self.buses.len());
            }
        }
        engine.set_chain_order(&self.chain_order());
    }

    // After `removed` is taken out, makes every effect that listened to it
    // listen to its own input, and every one that listened to a track or
    // bus after it listen one down, on `engine` too.
    fn renumber_listening(&mut self, engine: &mut Engine, removed: Place) {
        for place in self.places() {
            let chain = self.chain_mut(place).unwrap();
            for (slot, held) in chain.iter_mut().enumerate() {
                let Some(effect) = held else {
                    continue;
                };
                let Some(index) = effect.listens_to_place() else {
                    continue;
                };
                let Some(source) = effect.listens_to() else {
                    continue;
                };
                let renumbered = renumbered_source(place_of_chain(source), removed);
                let mut number = LISTEN_TO_OWN;
                if let Some(renumbered) = renumbered {
                    number = renumbered.chain() as f32;
                }
                effect.values[index] = number;
                let value_number = values_of(effect.kind, false)[index].number;
                engine.set_effect_value(place.chain(), slot, value_number, number);
            }
        }
    }

    // Sends the whole mix to `engine`: every track's and bus's strip, the
    // master's chain and limiter, and the chain order. Chains the song
    // doesn't use are emptied.
    pub fn send_all(&self, engine: &mut Engine) {
        for track in 0..TRACK_COUNT {
            engine.make_room();
            if let Some(strip) = self.tracks.get(track) {
                send_strip(engine, track, strip, self.buses.len());
            } else {
                send_strip(engine, track, &Strip::new(), self.buses.len());
            }
        }
        for bus in 0..BUS_COUNT {
            engine.make_room();
            if let Some(held) = self.buses.get(bus) {
                send_bus(engine, bus, held);
            } else {
                send_bus(engine, bus, &Bus::new(None));
            }
        }
        engine.make_room();
        send_chain(engine, Place::Master, &self.master);
        send_effect_modulation(engine, Place::Master, &self.master_modulation);
        engine.set_limiter(self.limiter);
        engine.set_chain_order(&self.chain_order());
    }

    // Makes every effect whose "listens to" names a chain it may not listen
    // to, or one that would close a loop, listen to its own input. Earlier
    // places and slots keep theirs first. A song file edited by hand may
    // hold such a one.
    pub fn keep_only_good_listening(&mut self) {
        // Every listener starts on its own input, then each is checked in
        // turn against the ones kept before it.
        let mut listeners = Vec::new();
        for place in self.places() {
            for slot in 0..EFFECT_SLOTS {
                let Ok(effect) = self.slot(place, slot) else {
                    continue;
                };
                let Some(index) = effect.listens_to_place() else {
                    continue;
                };
                let Some(source) = effect.listens_to() else {
                    continue;
                };
                listeners.push((place, slot, index, source));
                self.set_stored_value(place, slot, index, LISTEN_TO_OWN);
            }
        }
        for (place, slot, index, source) in listeners {
            if self.check_listening(place, slot, source).is_ok() {
                self.set_stored_value(place, slot, index, source as f32);
            }
        }
    }

    // Sets value `index` of the effect in slot `slot` at `place` to
    // `number`, here only, not on the engine. The slot must hold an effect.
    fn set_stored_value(&mut self, place: Place, slot: usize, index: usize, number: f32) {
        let chain = self.chain_mut(place).unwrap();
        let effect = chain[slot].as_mut().unwrap();
        effect.values[index] = number;
    }

    // Keeps one strip for each of `track_count` tracks: strips past the
    // last go, and missing ones are new. A song file read by hand may hold
    // too few or too many.
    pub fn fit_tracks(&mut self, track_count: usize) {
        self.tracks.truncate(track_count);
        while self.tracks.len() < track_count {
            self.tracks.push(Strip::new());
        }
    }

    // The word for what slot `slot` at `place` listens to, for showing.
    pub fn listens_to_word(&self, place: Place, slot: usize) -> Option<String> {
        let effect = self.slot(place, slot).ok()?;
        let chain = effect.listens_to()?;
        Some(listens_to_word(chain as f32))
    }
}

// Where `source` is after `removed` is taken out: one down if it came after
// it among its own kind, and None if it was the one removed.
fn renumbered_source(source: Place, removed: Place) -> Option<Place> {
    if source == removed {
        return None;
    }
    match (source, removed) {
        (Place::Track(track), Place::Track(gone)) if track > gone => Some(Place::Track(track - 1)),
        (Place::Bus(bus), Place::Bus(gone)) if bus > gone => Some(Place::Bus(bus - 1)),
        _ => Some(source),
    }
}

// Sends track `track`'s strip to `engine`: its pan, its sends to the song's
// `bus_count` buses, nothing to the others, and its chain.
fn send_strip(engine: &mut Engine, track: usize, strip: &Strip, bus_count: usize) {
    engine.set_track_pan(track, strip.pan);
    for bus in 0..BUS_COUNT {
        let mut send = 0;
        if bus < bus_count {
            send = strip.sends[bus];
        }
        engine.set_send(track, bus, send);
    }
    send_chain(engine, Place::Track(track), &strip.chain);
}

// Sends bus `bus` to `engine`: its volume, pan and chain.
fn send_bus(engine: &mut Engine, bus: usize, held: &Bus) {
    engine.set_bus_volume(bus, held.volume);
    engine.set_bus_pan(bus, held.pan);
    send_chain(engine, Place::Bus(bus), &held.chain);
    send_effect_modulation(engine, Place::Bus(bus), &held.modulation);
}

pub fn send_effect_modulation(engine: &mut Engine, place: Place, modulation: &EffectModulation) {
    for (index, lfo) in modulation.lfos.iter().enumerate() {
        engine.set_chain_lfo(place.chain(), index, *lfo);
    }
    send_effect_links(engine, place, &modulation.links);
}

pub fn send_effect_links(engine: &mut Engine, place: Place, links: &[EffectLink]) {
    for index in 0..CONNECTION_COUNT {
        let link = links.get(index).copied().unwrap_or(empty_link());
        engine.set_effect_connection(place.chain(), index, link);
    }
}

// Sends every slot of `chain` at `place` to `engine`. A new effect starts
// at its starting values, so only the values that differ are sent, which
// keeps the engine's mailbox from filling.
fn send_chain(engine: &mut Engine, place: Place, chain: &Chain) {
    let number = place.chain();
    for (slot, held) in chain.iter().enumerate() {
        let Some(effect) = held else {
            engine.set_effect(number, slot, EffectKind::None);
            continue;
        };
        engine.set_effect(number, slot, effect.kind);
        let values = values_of(effect.kind, place.is_bus());
        for (value, set) in values.iter().zip(&effect.values) {
            if *set != value.start {
                engine.set_effect_value(number, slot, value.number, *set);
            }
        }
        if !effect.on {
            engine.set_effect_on(number, slot, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        Engine::for_render().unwrap()
    }

    #[test]
    fn effect_routing_counts_sparse_and_zero_amount_sound_connections() {
        use crate::modulation::{NO_CONNECTION, Target};
        let mut connections = [NO_CONNECTION; CONNECTION_COUNT];
        connections[63] = Connection { source: Source::Lfo(0), target: Target::Pitch,
            amount: 0.0, one_way: false, snap: false, scaled_by: Source::None };
        let link = EffectLink { source: Source::Lfo(0), slot: 0, value: 0,
            amount: 0.0, one_way: false, scaled_by: Source::None };
        assert!(check_track_routing(&connections, &vec![link; 63]).is_ok());
        assert!(check_track_routing(&connections, &vec![link; 64]).is_err());
        connections[63] = NO_CONNECTION;
        assert!(check_track_routing(&connections, &vec![link; 64]).is_ok());
    }

    #[test]
    fn removing_a_track_effect_link_clears_the_engine_slot() {
        use crate::engine::EQ_OUTPUT;
        use crate::modulation::{NO_CONNECTION, LfoShape};
        let mut changed = engine();
        let mut plain = engine();
        let mut lfo = START_LFO;
        lfo.per_note = false;
        lfo.shape = LfoShape::Steps;
        lfo.step_count = 1;
        lfo.steps[0] = 100.0;
        for engine in [&mut changed, &mut plain] {
            engine.set_effect(0, 0, EffectKind::Eq);
            engine.set_chain_lfo(0, 0, lfo);
        }
        let connections = [NO_CONNECTION; CONNECTION_COUNT];
        let link = EffectLink { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: -12.0, one_way: false, scaled_by: Source::None };
        send_track_routing(&mut changed, 0, &connections, &[link]);
        send_track_routing(&mut changed, 0, &connections, &[]);
        changed.note_on(69, 100);
        plain.note_on(69, 100);
        assert_eq!(changed.render(4096), plain.render(4096));
    }

    #[test]
    fn an_effect_link_becomes_active_again_when_its_lfo_is_shared() {
        use crate::engine::EQ_OUTPUT;
        use crate::modulation::{NO_CONNECTION, LfoShape};
        let mut moved = engine();
        let mut plain = engine();
        let mut lfo = START_LFO;
        lfo.shape = LfoShape::Steps;
        lfo.step_count = 1;
        lfo.steps[0] = 100.0;
        for engine in [&mut moved, &mut plain] {
            engine.set_effect(0, 0, EffectKind::Eq);
            engine.set_chain_lfo(0, 0, lfo);
        }
        let connections = [NO_CONNECTION; CONNECTION_COUNT];
        let link = EffectLink { source: Source::Lfo(0), slot: 0, value: EQ_OUTPUT,
            amount: -12.0, one_way: false, scaled_by: Source::None };
        send_track_routing(&mut moved, 0, &connections, &[link]);
        moved.note_on(69, 100);
        plain.note_on(69, 100);
        assert_eq!(moved.render(4096), plain.render(4096));

        lfo.per_note = false;
        moved.set_chain_lfo(0, 0, lfo);
        plain.set_chain_lfo(0, 0, lfo);
        let moved_samples = moved.render(8192);
        let plain_samples = plain.render(8192);
        let moved_energy: f32 = moved_samples[4096..].iter().map(|sample| sample.abs()).sum();
        let plain_energy: f32 = plain_samples[4096..].iter().map(|sample| sample.abs()).sum();
        assert!(moved_energy < plain_energy * 0.6);
    }

    fn word(text: &str) -> Written {
        Written::Word(String::from(text))
    }

    // A mix of `tracks` tracks and `buses` buses, sent to a render engine.
    fn mix_of(engine: &mut Engine, tracks: usize, buses: usize) -> Mix {
        let mut mix = Mix::new(tracks);
        for _ in 0..buses {
            mix.add_bus(engine, None).unwrap();
        }
        mix
    }

    #[test]
    fn a_new_mix_has_a_bare_strip_for_each_track_no_buses_and_the_start_limiter() {
        let mix = Mix::new(3);
        assert_eq!(mix.tracks.len(), 3);
        assert_eq!(mix.tracks[0], Strip::new());
        assert!(mix.buses.is_empty());
        assert!(mix.master.iter().all(Option::is_none));
        assert_eq!(mix.limiter, START_LIMITER);
        assert_eq!(mix.places(), vec![Place::Track(0), Place::Track(1), Place::Track(2), Place::Master]);
    }

    #[test]
    fn placing_an_effect_makes_one_in_the_engine_at_its_starting_values() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 2, 1);
        mix.place_effect(&mut engine, Place::Track(1), 3, EffectKind::Reverb).unwrap();
        mix.place_effect(&mut engine, Place::Bus(0), 0, EffectKind::Reverb).unwrap();
        engine.render(64);
        assert_eq!(engine.effect_count(), 2);

        let track_reverb = mix.slot(Place::Track(1), 3).unwrap();
        assert_eq!(track_reverb.kind, EffectKind::Reverb);
        assert!(track_reverb.on);
        let mix_word = |slot: &Slot, place: Place| {
            let values = values_of(slot.kind, place.is_bus());
            let index = values.iter().position(|value| value.word == "mix").unwrap();
            slot.values[index]
        };
        assert_eq!(mix_word(track_reverb, Place::Track(1)), 0.3);
        let bus_reverb = mix.slot(Place::Bus(0), 0).unwrap();
        assert_eq!(mix_word(bus_reverb, Place::Bus(0)), 1.0);

        mix.place_effect(&mut engine, Place::Track(1), 3, EffectKind::None).unwrap();
        assert!(mix.chain(Place::Track(1)).unwrap()[3].is_none());
        engine.render(64);
        assert_eq!(engine.effect_count(), 1);
    }

    #[test]
    fn places_and_slots_the_song_does_not_have_are_refused() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 1, 0);
        let error = mix.place_effect(&mut engine, Place::Track(1), 0, EffectKind::Eq).unwrap_err();
        assert_eq!(error, "the song has no track 2");
        let error = mix.place_effect(&mut engine, Place::Bus(0), 0, EffectKind::Eq).unwrap_err();
        assert_eq!(error, "the song has no bus 1");
        let error = mix.place_effect(&mut engine, Place::Master, 8, EffectKind::Eq).unwrap_err();
        assert_eq!(error, "there are 8 slots, from 1 to 8");
        let error = mix.set_effect_on(&mut engine, Place::Master, 2, false).unwrap_err();
        assert_eq!(error, "slot 3 on master is empty");
    }

    #[test]
    fn moving_an_effect_shifts_the_slots_between() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 1, 0);
        let kinds = [EffectKind::Eq, EffectKind::Compressor, EffectKind::Delay];
        for (slot, kind) in kinds.iter().enumerate() {
            mix.place_effect(&mut engine, Place::Master, slot, *kind).unwrap();
        }
        let kind_at = |mix: &Mix, slot: usize| mix.master[slot].as_ref().map(|effect| effect.kind);

        mix.move_effect(&mut engine, Place::Master, 0, 2).unwrap();
        assert_eq!(kind_at(&mix, 0), Some(EffectKind::Compressor));
        assert_eq!(kind_at(&mix, 1), Some(EffectKind::Delay));
        assert_eq!(kind_at(&mix, 2), Some(EffectKind::Eq));

        mix.move_effect(&mut engine, Place::Master, 2, 0).unwrap();
        assert_eq!(kind_at(&mix, 0), Some(EffectKind::Eq));
        assert_eq!(kind_at(&mix, 1), Some(EffectKind::Compressor));

        mix.move_effect(&mut engine, Place::Master, 1, 5).unwrap();
        assert_eq!(kind_at(&mix, 1), Some(EffectKind::Delay));
        assert_eq!(kind_at(&mix, 5), Some(EffectKind::Compressor));
        assert!(mix.move_effect(&mut engine, Place::Master, 1, 8).is_err());
    }

    #[test]
    fn values_are_set_by_word_kept_in_range_and_said_back_as_set() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 1, 0);
        mix.place_effect(&mut engine, Place::Track(0), 0, EffectKind::Saturation).unwrap();
        let set = mix.set_effect_value(&mut engine, Place::Track(0), 0, "drive_db", &Written::Number(99.0));
        assert_eq!(set, Ok(Written::Number(36.0)));
        let set = mix.set_effect_value(&mut engine, Place::Track(0), 0, "curve", &word("fold"));
        assert_eq!(set, Ok(word("fold")));
        let error = mix.set_effect_value(&mut engine, Place::Track(0), 0, "speed", &Written::Number(1.0));
        assert_eq!(error.unwrap_err(), "this effect has no speed: it has drive_db, curve, tone, output_db, match, mix");
        assert_eq!(mix.slot(Place::Track(0), 0).unwrap().values[0], 36.0);
    }

    #[test]
    fn a_track_listens_only_to_other_tracks_and_never_in_a_loop() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 3, 1);
        for track in 0..3 {
            mix.place_effect(&mut engine, Place::Track(track), 0, EffectKind::Compressor).unwrap();
        }
        let listen = |mix: &mut Mix, engine: &mut Engine, track: usize, to: &str| {
            mix.set_effect_value(engine, Place::Track(track), 0, "listens_to", &word(to))
        };
        assert_eq!(listen(&mut mix, &mut engine, 0, "track 1").unwrap_err(), "track 1 can't listen to itself");
        let error = listen(&mut mix, &mut engine, 0, "bus 1").unwrap_err();
        assert_eq!(error, "a track can only listen to other tracks");
        assert_eq!(listen(&mut mix, &mut engine, 0, "track 4").unwrap_err(), "the song has no track 4");

        assert_eq!(listen(&mut mix, &mut engine, 0, "track 2"), Ok(word("track 2")));
        assert_eq!(listen(&mut mix, &mut engine, 1, "track 3"), Ok(word("track 3")));
        let error = listen(&mut mix, &mut engine, 2, "track 1").unwrap_err();
        assert_eq!(error, "that would make a loop: track 1 already hears track 3");
        // Changing what one slot listens to never counts that slot twice.
        assert_eq!(listen(&mut mix, &mut engine, 0, "track 3"), Ok(word("track 3")));
        assert_eq!(listen(&mut mix, &mut engine, 0, "own"), Ok(word("own")));
        assert_eq!(listen(&mut mix, &mut engine, 2, "track 1"), Ok(word("track 1")));
    }

    #[test]
    fn buses_listen_to_tracks_and_other_buses_and_the_master_to_anything() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 2, 2);
        mix.place_effect(&mut engine, Place::Bus(0), 0, EffectKind::Gate).unwrap();
        mix.place_effect(&mut engine, Place::Bus(1), 0, EffectKind::Gate).unwrap();
        mix.place_effect(&mut engine, Place::Master, 0, EffectKind::Filter).unwrap();
        let listen = |mix: &mut Mix, engine: &mut Engine, place: Place, to: &str| {
            mix.set_effect_value(engine, place, 0, "listens_to", &word(to))
        };
        assert!(listen(&mut mix, &mut engine, Place::Bus(0), "track 2").is_ok());
        assert!(listen(&mut mix, &mut engine, Place::Bus(0), "bus 2").is_ok());
        assert!(listen(&mut mix, &mut engine, Place::Bus(1), "bus 1").is_err());
        assert!(listen(&mut mix, &mut engine, Place::Bus(1), "bus 2").is_err());
        assert!(listen(&mut mix, &mut engine, Place::Master, "bus 1").is_ok());
        assert!(listen(&mut mix, &mut engine, Place::Master, "track 1").is_ok());
    }

    #[test]
    fn the_chain_order_runs_every_source_before_its_listeners() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 4, 2);
        let new_order: Vec<usize> = (0..MASTER_CHAIN).collect();
        assert_eq!(mix.chain_order(), new_order);

        mix.place_effect(&mut engine, Place::Track(0), 0, EffectKind::Compressor).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(0), 0, "listens_to", &word("track 4")).unwrap();
        mix.place_effect(&mut engine, Place::Track(3), 0, EffectKind::Gate).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(3), 0, "listens_to", &word("track 3")).unwrap();
        mix.place_effect(&mut engine, Place::Bus(0), 0, EffectKind::Gate).unwrap();
        mix.set_effect_value(&mut engine, Place::Bus(0), 0, "listens_to", &word("bus 2")).unwrap();
        assert_eq!(mix.chain_order(), vec![1, 2, 3, 0, 4, 5, 6, 7, 8, 10, 9, 11, 12]);

        // Emptying the slot frees the order again.
        mix.place_effect(&mut engine, Place::Track(0), 0, EffectKind::None).unwrap();
        assert_eq!(mix.chain_order(), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 9, 11, 12]);
    }

    #[test]
    fn pan_sends_and_bus_volume_are_kept_in_range_and_need_what_they_name() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 2, 1);
        assert_eq!(mix.set_pan(&mut engine, Place::Track(1), -250), Ok(-100));
        assert_eq!(mix.set_pan(&mut engine, Place::Bus(0), 40), Ok(40));
        assert_eq!(mix.set_pan(&mut engine, Place::Master, 0).unwrap_err(), "the master has no pan");
        assert_eq!(mix.set_send(&mut engine, 0, 0, 150), Ok(100));
        assert_eq!(mix.set_send(&mut engine, 0, 1, 50).unwrap_err(), "the song has no bus 2");
        assert_eq!(mix.set_send(&mut engine, 2, 0, 50).unwrap_err(), "the song has no track 3");
        assert_eq!(mix.set_bus_volume(&mut engine, 0, 70), Ok(70));
        assert_eq!(mix.tracks[1].pan, -100);
        assert_eq!(mix.tracks[0].sends, [100, 0, 0, 0]);
        assert_eq!(mix.buses[0].volume, 70);
    }

    #[test]
    fn buses_are_named_counted_and_at_most_four() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 1, 0);
        assert_eq!(mix.add_bus(&mut engine, Some("  Verb  ")), Ok(0));
        assert_eq!(mix.bus_name(0), "Verb");
        assert_eq!(mix.add_bus(&mut engine, Some("   ")), Ok(1));
        assert_eq!(mix.bus_name(1), "Bus 2");
        mix.add_bus(&mut engine, None).unwrap();
        mix.add_bus(&mut engine, None).unwrap();
        assert_eq!(mix.add_bus(&mut engine, None).unwrap_err(), "a song has at most 4 buses");
        mix.rename_bus(1, Some("Echo")).unwrap();
        assert_eq!(mix.bus_name(1), "Echo");
        assert!(mix.rename_bus(4, Some("x")).is_err());
    }

    #[test]
    fn removing_a_bus_moves_the_ones_after_it_down_with_their_sends_and_listeners() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 1, 3);
        mix.set_send(&mut engine, 0, 0, 10).unwrap();
        mix.set_send(&mut engine, 0, 1, 20).unwrap();
        mix.set_send(&mut engine, 0, 2, 30).unwrap();
        mix.place_effect(&mut engine, Place::Bus(2), 0, EffectKind::Reverb).unwrap();
        mix.place_effect(&mut engine, Place::Master, 0, EffectKind::Compressor).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 0, "listens_to", &word("bus 3")).unwrap();
        mix.place_effect(&mut engine, Place::Master, 1, EffectKind::Gate).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 1, "listens_to", &word("bus 2")).unwrap();

        mix.remove_bus(&mut engine, 1).unwrap();
        assert_eq!(mix.buses.len(), 2);
        assert_eq!(mix.tracks[0].sends, [10, 30, 0, 0]);
        assert!(mix.chain(Place::Bus(1)).unwrap()[0].is_some());
        assert_eq!(mix.listens_to_word(Place::Master, 0), Some(String::from("bus 2")));
        assert_eq!(mix.listens_to_word(Place::Master, 1), None);
        engine.render(64);
        assert_eq!(engine.effect_count(), 3);
        assert!(mix.remove_bus(&mut engine, 2).is_err());
    }

    #[test]
    fn removing_a_track_moves_the_strips_after_it_down_and_renumbers_listeners() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 3, 0);
        mix.set_pan(&mut engine, Place::Track(2), 50).unwrap();
        mix.place_effect(&mut engine, Place::Track(2), 0, EffectKind::Compressor).unwrap();
        mix.set_effect_value(&mut engine, Place::Track(2), 0, "listens_to", &word("track 2")).unwrap();
        mix.place_effect(&mut engine, Place::Master, 0, EffectKind::Compressor).unwrap();
        mix.set_effect_value(&mut engine, Place::Master, 0, "listens_to", &word("track 3")).unwrap();

        mix.remove_track(&mut engine, 1);
        assert_eq!(mix.tracks.len(), 2);
        assert_eq!(mix.tracks[1].pan, 50);
        assert_eq!(mix.listens_to_word(Place::Track(1), 0), None);
        assert_eq!(mix.listens_to_word(Place::Master, 0), Some(String::from("track 2")));
        engine.render(64);
        assert_eq!(engine.effect_count(), 2);

        mix.fit_tracks(3);
        assert_eq!(mix.tracks[2], Strip::new());
    }

    #[test]
    fn sending_the_whole_mix_makes_every_effect_and_empties_the_rest() {
        let mut engine = engine();
        let mut mix = mix_of(&mut engine, 2, 1);
        mix.place_effect(&mut engine, Place::Track(0), 0, EffectKind::Eq).unwrap();
        mix.place_effect(&mut engine, Place::Bus(0), 7, EffectKind::Delay).unwrap();
        mix.place_effect(&mut engine, Place::Master, 4, EffectKind::Chorus).unwrap();

        let mut fresh = Engine::for_render().unwrap();
        // Something the song doesn't have, which sending the mix clears.
        fresh.set_effect(5, 0, EffectKind::Reverb);
        mix.send_all(&mut fresh);
        fresh.render(64);
        assert_eq!(fresh.effect_count(), 3);
    }

    #[test]
    fn strips_fit_the_tracks_a_song_file_has() {
        let mut mix = Mix::new(2);
        mix.tracks[1].pan = 30;
        mix.fit_tracks(4);
        assert_eq!(mix.tracks.len(), 4);
        assert_eq!(mix.tracks[1].pan, 30);
        mix.fit_tracks(1);
        assert_eq!(mix.tracks.len(), 1);
    }
}
