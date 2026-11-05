// Strums (docs/design.md#guitar): a strum's plucks follow each other by the
// song's strum gap. They wait here, each with when it is due and the track
// it was strummed on, until the engine plays them (Engine::play_due_plucks,
// asked every few ms by a thread in lib.rs).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::key_map::Pluck;

// One pluck of a strum, waiting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaterPluck {
    pub due: Instant,
    pub track: usize,
    pub pluck: Pluck,
}

// The plucks waiting, soonest first.
#[derive(Default)]
pub struct Strums {
    waiting: VecDeque<LaterPluck>,
}

impl Strums {
    // Lines up `plucks` on `track`, the first due at `start` and each next
    // one `gap` after the one before.
    pub fn add(&mut self, plucks: &[Pluck], track: usize, start: Instant, gap: Duration) {
        let mut due = start;
        for pluck in plucks {
            self.waiting.push_back(LaterPluck { due, track, pluck: *pluck });
            due += gap;
        }
    }

    // The plucks due by `now`, soonest first, taken out.
    pub fn take_due(&mut self, now: Instant) -> Vec<LaterPluck> {
        let mut due = Vec::new();
        while let Some(first) = self.waiting.front() {
            if first.due > now {
                break;
            }
            due.push(*first);
            self.waiting.pop_front();
        }
        due
    }

    // Every pluck still waiting, soonest first, taken out.
    pub fn take_all(&mut self) -> Vec<LaterPluck> {
        self.waiting.drain(..).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pluck(note: u8) -> Pluck {
        Pluck { after: None, note, velocity: 80 }
    }

    #[test]
    fn plucks_come_due_one_gap_after_another() {
        let start = Instant::now();
        let gap = Duration::from_millis(25);
        let mut strums = Strums::default();
        strums.add(&[pluck(40), pluck(45), pluck(50)], 2, start, gap);

        let first = strums.take_due(start);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].pluck.note, 40);
        assert_eq!(first[0].track, 2);
        assert!(strums.take_due(start + Duration::from_millis(24)).is_empty());
        let second = strums.take_due(start + gap);
        assert_eq!(second[0].pluck.note, 45);
        let third = strums.take_due(start + Duration::from_secs(1));
        assert_eq!(third[0].pluck.note, 50);
        assert!(strums.is_empty());
    }

    #[test]
    fn no_gap_makes_every_pluck_due_at_once() {
        let start = Instant::now();
        let mut strums = Strums::default();
        strums.add(&[pluck(40), pluck(45)], 0, start, Duration::ZERO);
        assert_eq!(strums.take_due(start).len(), 2);
    }

    #[test]
    fn taking_all_empties_the_wait_in_order() {
        let start = Instant::now();
        let mut strums = Strums::default();
        strums.add(&[pluck(40), pluck(45)], 0, start, Duration::from_millis(25));
        let all = strums.take_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].pluck.note, 45);
        assert!(strums.take_all().is_empty());
    }
}
