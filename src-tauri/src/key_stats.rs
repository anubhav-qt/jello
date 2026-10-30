// The player's own timing and misses between keys, kept only on their machine
// (docs/adr/0047-song-key-mappings.md). The optimizer's score starts from
// rules and follows these once a pair of keys has been played enough.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use toml::{Table, Value};

// The file in the settings folder.
const FILE_NAME: &str = "key-timing.toml";

// A longer gap is the player stopping, not moving between keys.
const LONGEST_GAP: Duration = Duration::from_millis(1500);

// How one pair of keys has gone: from the first key to the second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PairStats {
    pub count: u32,
    pub total_ms: u64,
    pub misses: u32,
}

#[derive(Debug, Default)]
pub struct KeyStats {
    pairs: BTreeMap<(String, String), PairStats>,
    last: Option<(String, Instant)>,
    unsaved: bool,
}

impl KeyStats {
    // `key` was pressed by the player at `now`.
    pub fn heard(&mut self, key: &str, now: Instant) {
        if let Some((before, at)) = &self.last {
            let gap = now.saturating_duration_since(*at);
            if gap <= LONGEST_GAP {
                let pair = (before.clone(), String::from(key));
                let stats = self.pairs.entry(pair).or_default();
                stats.count += 1;
                stats.total_ms += gap.as_millis() as u64;
                self.unsaved = true;
            }
        }
        self.last = Some((String::from(key), now));
    }

    // The player missed `key`, due next after the key they pressed last.
    pub fn missed(&mut self, key: &str) {
        let Some((before, _)) = &self.last else {
            return;
        };
        let pair = (before.clone(), String::from(key));
        self.pairs.entry(pair).or_default().misses += 1;
        self.unsaved = true;
    }

    pub fn pair(&self, from: &str, to: &str) -> Option<PairStats> {
        self.pairs.get(&(String::from(from), String::from(to))).copied()
    }

    // Reads what was saved in `folder`. A file that is missing or can't be
    // read is an empty start, since this only tunes a score.
    pub fn load(&mut self, folder: &Path) {
        let Ok(text) = fs::read_to_string(folder.join(FILE_NAME)) else {
            return;
        };
        self.pairs = pairs_of(&text);
    }

    // Saves to `folder` if anything changed since the last time.
    pub fn save_if_changed(&mut self, folder: &Path) {
        if !self.unsaved {
            return;
        }
        if fs::write(folder.join(FILE_NAME), text_of(&self.pairs)).is_ok() {
            self.unsaved = false;
        }
    }
}

fn text_of(pairs: &BTreeMap<(String, String), PairStats>) -> String {
    let mut text = String::from("# How your fingers move between keys. Only Jello on this machine reads it.\n\n");
    text.push_str("pairs = [\n");
    for ((from, to), stats) in pairs {
        text.push_str(&format!(
            "  {{ from = \"{from}\", to = \"{to}\", count = {}, total_ms = {}, misses = {} }},\n",
            stats.count, stats.total_ms, stats.misses
        ));
    }
    text.push_str("]\n");
    text
}

fn pairs_of(text: &str) -> BTreeMap<(String, String), PairStats> {
    let mut pairs = BTreeMap::new();
    let Ok(table) = text.parse::<Table>() else {
        return pairs;
    };
    let Some(items) = table.get("pairs").and_then(Value::as_array) else {
        return pairs;
    };
    for item in items {
        let Some(from) = item.get("from").and_then(Value::as_str) else {
            continue;
        };
        let Some(to) = item.get("to").and_then(Value::as_str) else {
            continue;
        };
        let stats = PairStats {
            count: count_in(item, "count") as u32,
            total_ms: count_in(item, "total_ms"),
            misses: count_in(item, "misses") as u32,
        };
        pairs.insert((String::from(from), String::from(to)), stats);
    }
    pairs
}

// A whole number in `item`, or 0 for one missing or below 0.
fn count_in(item: &Value, name: &str) -> u64 {
    let number = item.get(name).and_then(Value::as_integer).unwrap_or(0);
    number.max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_between_keys_are_counted_by_pair() {
        let mut stats = KeyStats::default();
        let start = Instant::now();
        stats.heard("KeyF", start);
        stats.heard("KeyJ", start + Duration::from_millis(200));
        stats.heard("KeyF", start + Duration::from_millis(500));
        stats.heard("KeyJ", start + Duration::from_millis(900));
        let pair = stats.pair("KeyF", "KeyJ").unwrap();
        assert_eq!(pair.count, 2);
        assert_eq!(pair.total_ms, 600);
        assert_eq!(stats.pair("KeyJ", "KeyK"), None);
    }

    #[test]
    fn a_long_stop_is_not_a_move() {
        let mut stats = KeyStats::default();
        let start = Instant::now();
        stats.heard("KeyF", start);
        stats.heard("KeyJ", start + Duration::from_secs(5));
        assert_eq!(stats.pair("KeyF", "KeyJ"), None);
    }

    #[test]
    fn misses_count_from_the_key_pressed_last() {
        let mut stats = KeyStats::default();
        stats.missed("KeyJ");
        assert_eq!(stats.pair("KeyF", "KeyJ"), None);
        stats.heard("KeyF", Instant::now());
        stats.missed("KeyJ");
        assert_eq!(stats.pair("KeyF", "KeyJ").unwrap().misses, 1);
    }

    #[test]
    fn what_is_saved_reads_back_and_a_bad_file_is_an_empty_start() {
        let mut pairs = BTreeMap::new();
        pairs.insert(
            (String::from("KeyF"), String::from("KeyJ")),
            PairStats {
                count: 3,
                total_ms: 450,
                misses: 1,
            },
        );
        assert_eq!(pairs_of(&text_of(&pairs)), pairs);
        assert!(pairs_of("not toml at all =").is_empty());
        assert!(pairs_of("pairs = [{ from = 1 }]").is_empty());
    }
}
