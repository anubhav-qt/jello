// The song optimizer (docs/design.md#song-key-mappings): moves the notes a song
// plays to the keys that are most comfortable for it. Layout analyzers score
// text; this scores note pairs and chord changes. The rules are only the start:
// once the player has played a pair of keys enough, their own timing and misses
// take over (src/key_stats.rs).
//
// It searches by simulated annealing, with a soft pull towards the mapping it
// started from, so a key only moves if that really helps. Locked keys never
// move. A section's mapping pays for each key that differs from the song's
// main mapping, since the player has to read a change.

use std::collections::BTreeMap;

use crate::key_mapping::KeyMapping;
use crate::key_stats::KeyStats;
use crate::song_file::{Song, SongLayer};

// Notes starting this close together, in beats, are played as one press.
const SAME_PRESS_BEATS: f64 = 0.02;

// How much moving one key costs, and how much one that differs from the
// song's main mapping costs in a section.
const MOVE_PULL: f64 = 0.5;
const SECTION_CHANGE_COST: f64 = 1.0;

// The search: how many tries at most, how much work it does in all, and the
// temperature it cools through.
const MOST_TRIES: usize = 30_000;
const MOST_WORK: usize = 8_000_000;
const FIRST_TEMPERATURE: f64 = 2.0;
const LAST_TEMPERATURE: f64 = 0.01;
const SEED: u64 = 0x4A45_4C4C_4F5F_4B45;

// How many plays of a pair before the player's own timing counts in full, and
// the most it ever counts for. A pair played in this many ms costs 1.
const PAIR_PLAYS_IN_FULL: f64 = 50.0;
const MOST_LEARNED_SHARE: f64 = 0.8;
const MS_PER_COST: f64 = 200.0;
const MISS_COST: f64 = 5.0;

// Where a key sits under the hand. Fingers count from the left pinky at 0
// through the thumb at 4 to the right pinky at 8.
#[derive(Clone, Copy)]
struct Spot {
    finger: i8,
    row: i8,
    column: i8,
}

const NUMBER: i8 = 0;
const UPPER: i8 = 1;
const HOME: i8 = 2;
const BOTTOM: i8 = 3;
const THUMB: i8 = 4;

// Every mappable key's spot.
const SPOTS: [(&str, i8, i8, i8); 47] = [
    ("Backquote", 0, NUMBER, -1),
    ("Digit1", 0, NUMBER, 0),
    ("Digit2", 1, NUMBER, 1),
    ("Digit3", 2, NUMBER, 2),
    ("Digit4", 3, NUMBER, 3),
    ("Digit5", 3, NUMBER, 4),
    ("Digit6", 5, NUMBER, 5),
    ("Digit7", 5, NUMBER, 6),
    ("Digit8", 6, NUMBER, 7),
    ("Digit9", 7, NUMBER, 8),
    ("Digit0", 8, NUMBER, 9),
    ("Tab", 0, UPPER, -1),
    ("KeyQ", 0, UPPER, 0),
    ("KeyW", 1, UPPER, 1),
    ("KeyE", 2, UPPER, 2),
    ("KeyR", 3, UPPER, 3),
    ("KeyT", 3, UPPER, 4),
    ("KeyY", 5, UPPER, 5),
    ("KeyU", 5, UPPER, 6),
    ("KeyI", 6, UPPER, 7),
    ("KeyO", 7, UPPER, 8),
    ("KeyP", 8, UPPER, 9),
    ("BracketLeft", 8, UPPER, 10),
    ("BracketRight", 8, UPPER, 11),
    ("Backslash", 8, UPPER, 12),
    ("KeyA", 0, HOME, 0),
    ("KeyS", 1, HOME, 1),
    ("KeyD", 2, HOME, 2),
    ("KeyF", 3, HOME, 3),
    ("KeyG", 3, HOME, 4),
    ("KeyH", 5, HOME, 5),
    ("KeyJ", 5, HOME, 6),
    ("KeyK", 6, HOME, 7),
    ("KeyL", 7, HOME, 8),
    ("Semicolon", 8, HOME, 9),
    ("Quote", 8, HOME, 10),
    ("ControlLeft", 0, BOTTOM, -1),
    ("KeyZ", 0, BOTTOM, 0),
    ("KeyX", 1, BOTTOM, 1),
    ("KeyC", 2, BOTTOM, 2),
    ("KeyV", 3, BOTTOM, 3),
    ("KeyB", 3, BOTTOM, 4),
    ("KeyN", 5, BOTTOM, 5),
    ("KeyM", 5, BOTTOM, 6),
    ("Comma", 6, BOTTOM, 7),
    ("Period", 7, BOTTOM, 8),
    ("Slash", 8, BOTTOM, 9),
];

fn spot_of(key: &str) -> Spot {
    for (name, finger, row, column) in SPOTS {
        if name == key {
            return Spot { finger, row, column };
        }
    }
    Spot {
        finger: 4,
        row: THUMB,
        column: 4,
    }
}

// How hard a key is to press, whatever came before it.
fn press_effort(spot: Spot) -> f64 {
    let mut effort = match spot.row {
        HOME => 0.0,
        UPPER => 0.6,
        BOTTOM => 0.9,
        NUMBER => 1.4,
        _ => 0.2,
    };
    if spot.finger == 0 || spot.finger == 8 {
        effort += 0.5;
    }
    if spot.finger == 1 || spot.finger == 7 {
        effort += 0.2;
    }
    effort
}

fn same_hand(from: Spot, to: Spot) -> bool {
    if from.finger == 4 || to.finger == 4 {
        return false;
    }
    (from.finger < 4) == (to.finger < 4)
}

// How hard going from one key to another is, by the rules: the same finger
// is worst, a stretch on one hand next, and changing hands is easy.
fn rule_cost(from: Spot, to: Spot) -> f64 {
    let rows = f64::from((from.row - to.row).abs());
    let columns = f64::from((from.column - to.column).abs());
    if from.row == to.row && from.column == to.column {
        return 0.2;
    }
    if from.finger == to.finger {
        return 3.0 + rows + columns;
    }
    if same_hand(from, to) {
        return 1.0 + 0.3 * columns + 0.5 * rows;
    }
    0.5
}

// The rules' cost, moved towards what the player's own timing and misses say
// the more they have played this pair.
fn move_cost(from: &str, to: &str, stats: &KeyStats) -> f64 {
    let rules = rule_cost(spot_of(from), spot_of(to));
    let Some(pair) = stats.pair(from, to) else {
        return rules;
    };
    if pair.count == 0 {
        return rules + MISS_COST * f64::from(pair.misses.min(1));
    }
    let count = f64::from(pair.count);
    let share = (count / PAIR_PLAYS_IN_FULL).min(MOST_LEARNED_SHARE);
    let mean_ms = pair.total_ms as f64 / count;
    let learned = mean_ms / MS_PER_COST + MISS_COST * f64::from(pair.misses) / count;
    rules * (1.0 - share) + learned * share
}

// What a stretch of a song plays: how often each set of notes is pressed, and
// how often each follows another.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub counts: BTreeMap<Vec<u8>, u32>,
    pub pairs: BTreeMap<(Vec<u8>, Vec<u8>), u32>,
}

impl Usage {
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    fn add_stream(&mut self, stream: &[Vec<u8>]) {
        for (place, notes) in stream.iter().enumerate() {
            *self.counts.entry(notes.clone()).or_insert(0) += 1;
            if place > 0 {
                let pair = (stream[place - 1].clone(), notes.clone());
                *self.pairs.entry(pair).or_insert(0) += 1;
            }
        }
    }
}

// What `song` plays: with no section, its chords and every instrument layer;
// with section `section`, from 0, that section's layers only. Chords and each
// track are played apart, so nothing follows across them. `mapping` says when
// notes starting together are one key's press: if no key plays them all, they
// count as separate presses.
pub fn usage_of(song: &Song, section: Option<usize>, mapping: &KeyMapping) -> Usage {
    let mut usage = Usage::default();
    if section.is_none() {
        let mut chords = Vec::new();
        for held in &song.chords {
            match &held.notes {
                Some(notes) => chords.push(notes.clone()),
                None => chords.push(held.chord.notes().to_vec()),
            }
        }
        usage.add_stream(&chords);
    }

    let Some(looped) = &song.looped else {
        return usage;
    };
    for (number, track) in song.tracks.iter().enumerate() {
        if track.drums.is_some() {
            continue;
        }
        let mut layer_places: Vec<usize> = Vec::new();
        let mut places_in_track = 0;
        for (place, layer) in looped.layers.iter().enumerate() {
            if layer.track != number {
                continue;
            }
            let picked = match section {
                Some(section) => song.sections.get(section).is_some_and(|section| {
                    section.layers.iter().any(|used| used.track == number && used.layer == places_in_track)
                }),
                None => true,
            };
            if picked {
                layer_places.push(place);
            }
            places_in_track += 1;
        }
        for place in layer_places {
            usage.add_stream(&presses_of(&looped.layers[place], mapping));
        }
    }
    usage
}

// A layer's presses in time order. Key notes for vibrato and glide are not
// notes to place.
fn presses_of(layer: &SongLayer, mapping: &KeyMapping) -> Vec<Vec<u8>> {
    let mut notes: Vec<(f64, u8)> = Vec::new();
    for note in &layer.notes {
        if note.note <= 127 {
            notes.push((note.start, note.note));
        }
    }
    notes.sort_by(|one, other| one.0.total_cmp(&other.0));

    let mut presses = Vec::new();
    let mut place = 0;
    while place < notes.len() {
        let start = notes[place].0;
        let mut together = Vec::new();
        while place < notes.len() && notes[place].0 - start <= SAME_PRESS_BEATS {
            together.push(notes[place].1);
            place += 1;
        }
        together.sort();
        together.dedup();
        if together.len() > 1 && mapping.key_playing(&together).is_none() {
            for note in together {
                presses.push(vec![note]);
            }
        } else {
            presses.push(together);
        }
    }
    presses
}

// How a mapping was improved.
#[derive(Clone, Debug, PartialEq)]
pub struct Optimized {
    pub after: KeyMapping,
    // By how much the score fell, in percent. 0 if nothing better was found.
    pub gain_percent: f64,
    // Sets of notes the song plays that found no key.
    pub left_out: Vec<Vec<u8>>,
}

// The search's view of a mapping: the used sets of notes, each at a key.
struct Search<'a> {
    keys: Vec<&'a str>,
    locked: Vec<bool>,
    // Each used set's count, and the used sets that follow one another.
    counts: Vec<f64>,
    pairs: Vec<(usize, usize, f64)>,
    // The set at each key at the start, by its place in `sets`.
    effort: Vec<f64>,
    moves: Vec<Vec<f64>>,
    // For each used set, the key it started on, or None.
    started: Vec<Option<usize>>,
    // For each key and used set, whether the reference mapping has that set
    // on that key.
    on_reference: Vec<Vec<bool>>,
    section_cost: f64,
}

impl Search<'_> {
    // How comfortable `at` is for the song alone.
    fn comfort_cost(&self, at: &[usize]) -> f64 {
        let mut cost = 0.0;
        for (set, count) in self.counts.iter().enumerate() {
            cost += count * self.effort[at[set]];
        }
        for (from, to, count) in &self.pairs {
            cost += count * self.moves[at[*from]][at[*to]];
        }
        cost
    }

    // The comfort, plus what moving keys away from where they were costs.
    fn score(&self, at: &[usize]) -> f64 {
        let mut cost = self.comfort_cost(at);
        for (set, key) in at.iter().enumerate() {
            if self.started[set] != Some(*key) {
                cost += MOVE_PULL;
            }
            if self.section_cost > 0.0 && !self.on_reference[*key][set] {
                cost += self.section_cost;
            }
        }
        cost
    }
}

// A small random number generator, so the same song always gets the same
// answer.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    fn share(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

// The best mapping found for `usage`, starting from `current`. `main` is the
// song's main mapping when `current` is a section's.
pub fn optimize(current: &KeyMapping, usage: &Usage, stats: &KeyStats, main: Option<&KeyMapping>) -> Optimized {
    let mut keys: Vec<&str> = Vec::new();
    for (name, _, _, _) in SPOTS {
        keys.push(name);
    }

    // Each used set, most played first, and where it starts.
    let mut used: Vec<(Vec<u8>, u32)> = Vec::new();
    for (notes, count) in &usage.counts {
        used.push((notes.clone(), *count));
    }
    used.sort_by(|one, other| other.1.cmp(&one.1).then_with(|| one.0.cmp(&other.0)));

    let is_used = |notes: &Vec<u8>| used.iter().any(|(set, _)| set == notes);
    let mut free: Vec<usize> = Vec::new();
    let mut locked = Vec::new();
    for (place, key) in keys.iter().enumerate() {
        let is_locked = current.locked.contains(*key);
        locked.push(is_locked);
        if is_locked {
            continue;
        }
        let holds_used = match current.keys.get(*key) {
            Some(notes) => is_used(notes),
            None => false,
        };
        if !holds_used {
            free.push(place);
        }
    }
    // The hardest free keys take the sets with no key yet, and go to the back
    // of the queue first, so the easy ones stay for the search to use.
    free.sort_by(|one, other| {
        let one_effort = press_effort(spot_of(keys[*one]));
        let other_effort = press_effort(spot_of(keys[*other]));
        other_effort.total_cmp(&one_effort)
    });

    let mut starts: Vec<Option<usize>> = Vec::new();
    let mut at: Vec<usize> = Vec::new();
    let mut sets: Vec<Vec<u8>> = Vec::new();
    let mut counts = Vec::new();
    let mut left_out = Vec::new();
    for (notes, count) in &used {
        let start = key_with(&keys, current, notes, &starts);
        let place = match start {
            Some(place) => place,
            None => {
                if free.is_empty() {
                    left_out.push(notes.clone());
                    continue;
                }
                free.remove(0)
            }
        };
        starts.push(start);
        at.push(place);
        sets.push(notes.clone());
        counts.push(f64::from(*count));
    }

    let mut pairs = Vec::new();
    for ((from, to), count) in &usage.pairs {
        let from_place = sets.iter().position(|set| set == from);
        let to_place = sets.iter().position(|set| set == to);
        if let (Some(from_place), Some(to_place)) = (from_place, to_place) {
            pairs.push((from_place, to_place, f64::from(*count)));
        }
    }

    let mut moves = Vec::new();
    let mut effort = Vec::new();
    for from in &keys {
        effort.push(press_effort(spot_of(from)));
        let mut row = Vec::new();
        for to in &keys {
            row.push(move_cost(from, to, stats));
        }
        moves.push(row);
    }

    let mut section_cost = 0.0;
    let mut on_reference = Vec::new();
    if let Some(main) = main {
        section_cost = SECTION_CHANGE_COST;
        for key in &keys {
            let mut row = Vec::new();
            for set in &sets {
                row.push(main.keys.get(*key) == Some(set));
            }
            on_reference.push(row);
        }
    }

    let search = Search {
        keys,
        locked,
        counts,
        pairs,
        effort,
        moves,
        started: starts,
        on_reference,
        section_cost,
    };

    // Where sets that had no key go counts as the starting point: the search
    // compares against the mapping the player would have without it.
    let start_cost = search.comfort_cost(&at);
    let best = anneal(&search, at.clone());

    let best_comfort = search.comfort_cost(&best);
    let gain_is_real = search.score(&best) < search.score(&at);
    if !gain_is_real && left_out.len() == used.len() {
        return Optimized {
            after: current.clone(),
            gain_percent: 0.0,
            left_out,
        };
    }
    let chosen = if gain_is_real { &best } else { &at };

    let mut gain_percent = 0.0;
    if gain_is_real && start_cost > 0.0 {
        gain_percent = (start_cost - best_comfort) / start_cost * 100.0;
    }
    Optimized {
        after: mapping_with(current, &search, &sets, chosen),
        gain_percent: gain_percent.max(0.0),
        left_out,
    }
}

// The key that plays `notes` in `current` and no earlier set has taken.
fn key_with(keys: &[&str], current: &KeyMapping, notes: &[u8], taken: &[Option<usize>]) -> Option<usize> {
    for (place, key) in keys.iter().enumerate() {
        if taken.contains(&Some(place)) {
            continue;
        }
        let holds = current.keys.get(*key).is_some_and(|played| played == notes);
        if holds {
            return Some(place);
        }
    }
    None
}

fn anneal(search: &Search, start: Vec<usize>) -> Vec<usize> {
    let mut movable: Vec<usize> = Vec::new();
    for (set, key) in start.iter().enumerate() {
        if !search.locked[*key] {
            movable.push(set);
        }
    }
    let mut open_keys: Vec<usize> = Vec::new();
    for (place, is_locked) in search.locked.iter().enumerate() {
        if !is_locked {
            open_keys.push(place);
        }
    }
    if movable.is_empty() || open_keys.is_empty() {
        return start;
    }

    let work_per_try = search.pairs.len() + search.counts.len() + 1;
    let tries = MOST_TRIES.min(MOST_WORK / work_per_try);
    let cooling = (LAST_TEMPERATURE / FIRST_TEMPERATURE).powf(1.0 / tries.max(1) as f64);

    let mut random = Random(SEED);
    let mut at = start;
    let mut cost = search.score(&at);
    let mut best = at.clone();
    let mut best_cost = cost;
    let mut temperature = FIRST_TEMPERATURE;
    for _ in 0..tries {
        temperature *= cooling;
        let set = movable[random.below(movable.len())];
        let to = open_keys[random.below(open_keys.len())];
        let from = at[set];
        if to == from {
            continue;
        }
        let other = at.iter().position(|key| *key == to);
        at[set] = to;
        if let Some(other) = other {
            at[other] = from;
        }
        let next_cost = search.score(&at);
        let change = next_cost - cost;
        if change <= 0.0 || random.share() < (-change / temperature).exp() {
            cost = next_cost;
            if cost < best_cost {
                best_cost = cost;
                best.clone_from(&at);
            }
        } else {
            at[set] = from;
            if let Some(other) = other {
                at[other] = to;
            }
        }
    }
    best
}

// `current` with `sets` on the keys `at` says. What a used set pushes off a key
// goes to a key a used set left, so notes the song doesn't use stay reachable.
fn mapping_with(current: &KeyMapping, search: &Search, sets: &[Vec<u8>], at: &[usize]) -> KeyMapping {
    let mut after = current.clone();
    let mut taken: Vec<usize> = at.to_vec();
    taken.sort();

    let mut vacated = Vec::new();
    for start in search.started.iter().flatten() {
        if !taken.contains(start) {
            vacated.push(*start);
        }
    }
    let mut pushed_off = Vec::new();
    for key in &taken {
        let name = search.keys[*key];
        let Some(notes) = current.keys.get(name) else {
            continue;
        };
        let started_here = search.started.contains(&Some(*key));
        if !started_here {
            pushed_off.push(notes.clone());
        }
    }

    for place in &vacated {
        after.keys.remove(search.keys[*place]);
    }
    for (set, key) in at.iter().enumerate() {
        after.keys.insert(String::from(search.keys[*key]), sets[set].clone());
    }
    for (place, notes) in vacated.iter().zip(pushed_off) {
        after.keys.insert(String::from(search.keys[*place]), notes);
    }
    after
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::{Key, Mode};
    use crate::key_mapping::mappable_keys;
    use crate::scales::{Scale, ScaleKind};
    use crate::song_file::{BeatNote, SectionLayer, SongLoop, SongSection};
    use crate::sound_profile::SoundProfile;
    use std::time::{Duration, Instant};

    fn c_major() -> KeyMapping {
        KeyMapping::default_for(Scale {
            tonic: 0,
            kind: ScaleKind::Major,
        })
    }

    fn usage_of_notes(notes: &[u8]) -> Usage {
        let mut usage = Usage::default();
        let stream: Vec<Vec<u8>> = notes.iter().map(|note| vec![*note]).collect();
        usage.add_stream(&stream);
        usage
    }

    fn score_of(mapping: &KeyMapping, usage: &Usage) -> f64 {
        let mut cost = 0.0;
        let key_of = |notes: &Vec<u8>| -> String {
            String::from(mapping.key_playing(notes).expect("every set has a key"))
        };
        let stats = KeyStats::default();
        for (notes, count) in &usage.counts {
            cost += f64::from(*count) * press_effort(spot_of(&key_of(notes)));
        }
        for ((from, to), count) in &usage.pairs {
            cost += f64::from(*count) * move_cost(&key_of(from), &key_of(to), &stats);
        }
        cost
    }

    #[test]
    fn every_mappable_key_has_a_spot() {
        for key in mappable_keys() {
            assert!(SPOTS.iter().any(|(name, _, _, _)| *name == key), "{key}");
        }
        assert_eq!(SPOTS.len(), mappable_keys().len());
    }

    #[test]
    fn the_rules_rank_same_finger_worst_and_changing_hands_easy() {
        let same_finger = rule_cost(spot_of("KeyJ"), spot_of("KeyU"));
        let stretch = rule_cost(spot_of("KeyJ"), spot_of("KeyK"));
        let hands = rule_cost(spot_of("KeyF"), spot_of("KeyJ"));
        assert!(same_finger > stretch);
        assert!(stretch > hands);
    }

    #[test]
    fn a_melody_on_hard_keys_moves_to_easier_ones_and_scores_better() {
        let mut mapping = KeyMapping::default();
        // All on the far number row, on one finger.
        mapping.set_key("Backquote", &[60]).unwrap();
        mapping.set_key("Digit1", &[62]).unwrap();
        mapping.set_key("Tab", &[64]).unwrap();
        let usage = usage_of_notes(&[60, 62, 64, 62, 60, 62, 64, 62, 60]);
        let found = optimize(&mapping, &usage, &KeyStats::default(), None);
        assert!(found.gain_percent > 10.0, "gain {}", found.gain_percent);
        assert!(score_of(&found.after, &usage) < score_of(&mapping, &usage));
        assert!(found.left_out.is_empty());
    }

    #[test]
    fn it_gives_the_same_answer_every_time() {
        let usage = usage_of_notes(&[60, 62, 64, 65, 67, 69, 71, 72, 69, 67]);
        let first = optimize(&c_major(), &usage, &KeyStats::default(), None);
        let second = optimize(&c_major(), &usage, &KeyStats::default(), None);
        assert_eq!(first, second);
    }

    #[test]
    fn a_key_stays_put_unless_moving_it_gains_enough() {
        let usage = usage_of_notes(&[60]);
        let mut best = f64::MAX;
        for (key, _, _, _) in SPOTS {
            best = best.min(press_effort(spot_of(key)));
        }
        let mut stayed_off_the_best = 0;
        for (key, _, _, _) in SPOTS {
            let mut mapping = KeyMapping::default();
            mapping.set_key(key, &[60]).unwrap();
            let found = optimize(&mapping, &usage, &KeyStats::default(), None);
            let effort = press_effort(spot_of(key));
            if found.after.key_playing(&[60]) == Some(key) {
                if effort > best {
                    stayed_off_the_best += 1;
                }
            } else {
                assert!(effort - best >= MOVE_PULL, "{key} moved for too little");
            }
        }
        assert!(stayed_off_the_best > 0);
    }

    #[test]
    fn locked_keys_never_move() {
        let mut mapping = KeyMapping::default();
        mapping.set_key("Digit1", &[60]).unwrap();
        mapping.set_key("Backquote", &[62]).unwrap();
        mapping.set_locked("Digit1", true).unwrap();
        let usage = usage_of_notes(&[60, 62, 60, 62, 60, 62]);
        let found = optimize(&mapping, &usage, &KeyStats::default(), None);
        assert_eq!(found.after.keys["Digit1"], vec![60]);
        assert!(found.after.locked.contains("Digit1"));
    }

    #[test]
    fn a_key_can_cross_hands_when_that_helps() {
        let mut mapping = KeyMapping::default();
        mapping.set_key("KeyF", &[60]).unwrap();
        mapping.set_key("KeyR", &[62]).unwrap();
        let usage = usage_of_notes(&[60, 62, 60, 62, 60, 62, 60, 62]);
        let found = optimize(&mapping, &usage, &KeyStats::default(), None);
        let first = found.after.key_playing(&[60]).unwrap();
        let second = found.after.key_playing(&[62]).unwrap();
        assert_ne!(spot_of(first).finger, spot_of(second).finger);
    }

    #[test]
    fn a_note_with_no_key_gets_one_and_nothing_else_is_lost_while_there_is_room() {
        let mut mapping = KeyMapping::default();
        mapping.set_key("KeyJ", &[60]).unwrap();
        let usage = usage_of_notes(&[60, 61, 60, 61]);
        let found = optimize(&mapping, &usage, &KeyStats::default(), None);
        assert!(found.after.key_playing(&[61]).is_some());
        assert!(found.after.key_playing(&[60]).is_some());
        assert!(found.left_out.is_empty());
    }

    #[test]
    fn with_every_key_locked_the_song_notes_stay_out() {
        let mut mapping = KeyMapping::default();
        for key in mappable_keys() {
            mapping.set_key(key, &[40]).unwrap();
            mapping.set_locked(key, true).unwrap();
        }
        let usage = usage_of_notes(&[60, 61]);
        let found = optimize(&mapping, &usage, &KeyStats::default(), None);
        assert_eq!(found.after, mapping);
        assert_eq!(found.left_out.len(), 2);
        assert_eq!(found.gain_percent, 0.0);
    }

    #[test]
    fn the_players_own_timing_outweighs_the_rules() {
        let mut stats = KeyStats::default();
        let start = Instant::now();
        // By the rules F to J is easy; this player is very slow at it.
        for round in 0..100u64 {
            stats.heard("KeyF", start + Duration::from_secs(round * 4));
            stats.heard("KeyJ", start + Duration::from_secs(round * 4) + Duration::from_millis(1400));
        }
        let learned = move_cost("KeyF", "KeyJ", &stats);
        let rules = rule_cost(spot_of("KeyF"), spot_of("KeyJ"));
        assert!(learned > rules * 3.0, "{learned} against {rules}");
    }

    #[test]
    fn a_section_keeps_closer_to_the_main_mapping_than_the_song_would() {
        let main = c_major();
        let usage = usage_of_notes(&[60, 62, 64, 60, 62, 64]);
        let free = optimize(&main, &usage, &KeyStats::default(), None);
        let section = optimize(&main, &usage, &KeyStats::default(), Some(&main));
        let moved = |mapping: &KeyMapping| {
            let mut count = 0;
            for (key, notes) in &mapping.keys {
                if main.keys.get(key) != Some(notes) {
                    count += 1;
                }
            }
            count
        };
        assert!(moved(&section.after) <= moved(&free.after));
    }

    fn song_with_a_melody() -> Song {
        let mut song = Song::new(&SoundProfile::new());
        song.key = Some(Key {
            tonic: 0,
            mode: Mode::Major,
        });
        let mut notes = Vec::new();
        for (place, note) in [60, 62, 64].iter().enumerate() {
            notes.push(BeatNote {
                note: *note,
                start: place as f64,
                end: place as f64 + 0.5,
            });
        }
        // Two notes together, then a vibrato key's press.
        notes.push(BeatNote { note: 60, start: 3.0, end: 3.5 });
        notes.push(BeatNote { note: 67, start: 3.0, end: 3.5 });
        notes.push(BeatNote {
            note: crate::engine::VIBRATO_KEY_NOTE,
            start: 3.2,
            end: 3.4,
        });
        song.looped = Some(SongLoop {
            bars: 1,
            layers: vec![SongLayer {
                track: 0,
                bars: 0,
                volume: 100,
                playing: true,
                notes,
            }],
        });
        song
    }

    #[test]
    fn a_songs_usage_counts_its_presses_and_skips_key_notes() {
        let song = song_with_a_melody();
        let usage = usage_of(&song, None, &KeyMapping::default());
        assert_eq!(usage.counts[&vec![60]], 2);
        assert_eq!(usage.counts[&vec![67]], 1);
        assert_eq!(usage.pairs[&(vec![60], vec![62])], 1);
        assert!(!usage.counts.keys().any(|set| set.iter().any(|note| *note > 127)));

        // A key that plays both makes them one press.
        let mut mapping = KeyMapping::default();
        mapping.set_key("KeyF", &[60, 67]).unwrap();
        let usage = usage_of(&song, None, &mapping);
        assert_eq!(usage.counts[&vec![60, 67]], 1);
    }

    #[test]
    fn a_sections_usage_is_only_its_own_layers() {
        let mut song = song_with_a_melody();
        song.sections.push(SongSection {
            name: String::from("Verse"),
            bars: 1,
            tempo: 120,
            key_shift: 0,
            layers: vec![SectionLayer { track: 0, layer: 0 }],
            automation: Vec::new(),
            keys: None,
        });
        song.sections.push(SongSection {
            name: String::from("Empty"),
            bars: 1,
            tempo: 120,
            key_shift: 0,
            layers: Vec::new(),
            automation: Vec::new(),
            keys: None,
        });
        assert!(!usage_of(&song, Some(0), &KeyMapping::default()).is_empty());
        assert!(usage_of(&song, Some(1), &KeyMapping::default()).is_empty());
    }
}
