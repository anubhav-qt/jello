// Scoring a practice try (docs/design.md#the-tutor-to-mastery). Each press
// is matched to the nearest note of the player's parts inside a window:
// hit, missed, wrong or extra, how many ms early or late, and how much of
// the hold was held. A try sums these up, by hand and by section, with the
// together window for notes both hands play at once. The window is the
// difficulty: wide for a beginner, narrow for a master.
//
// Times here are ms from the try's first beat; Jello's own clock, not the
// speakers', so a player on slow speakers reads as steadily late.

use serde::Serialize;

use crate::harmony::{Fit, Key, note_fit};

// Whose a part is: the player's left or right hand, or Jello's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Hand {
    Left,
    Right,
    Jello,
}

// A note the player should play: the place of the piece's note, its hand,
// its pitches (one, or a chord's, lowest first), when and for how long.
#[derive(Clone, Debug, PartialEq)]
pub struct Expected {
    pub index: usize,
    pub hand: Hand,
    pub pitches: Vec<u8>,
    pub at_ms: f64,
    pub hold_ms: f64,
    pub section: Option<String>,
}

// A press the player made: its notes, when, and how long it was held, or
// None while it still is.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub pitches: Vec<u8>,
    pub at_ms: f64,
    pub held_ms: Option<f64>,
}

// What became of one expected note.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NoteResult {
    pub index: usize,
    pub hit: bool,
    // Negative is early.
    pub offset_ms: Option<f64>,
    // How much of the hold was held: 1 is all of it.
    pub held_share: Option<f64>,
}

// How one hand, one section or the whole try went.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Tally {
    pub notes: usize,
    pub hits: usize,
    pub missed: usize,
    pub accuracy_percent: f64,
    // The average offset: negative early, positive late.
    pub mean_offset_ms: Option<f64>,
    // How far the offsets spread around their average, the timing's
    // evenness.
    pub spread_ms: Option<f64>,
    // How close the holds came to their lengths, 100 for exact.
    pub hold_percent: Option<f64>,
}

// A try's results.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scored {
    pub whole: Tally,
    pub left: Option<Tally>,
    pub right: Option<Tally>,
    pub sections: Vec<(String, Tally)>,
    // Presses near a note but of the wrong notes, and presses near none.
    pub wrong: usize,
    pub extras: usize,
    // How far apart the hands landed on notes they share a beat on, on
    // average.
    pub together_ms: Option<f64>,
    // Every note hit, nothing extra, and the timing on average inside half
    // the window.
    pub clean: bool,
    pub notes: Vec<NoteResult>,
}

// Scores `heard` against `expected`, with presses counted inside
// `window_ms` of a note.
pub fn score(expected: &[Expected], heard: &[Heard], window_ms: f64) -> Scored {
    let mut matched: Vec<Option<usize>> = vec![None; expected.len()];
    let mut wrong = 0;
    let mut extras = 0;
    for (press_place, press) in heard.iter().enumerate() {
        let mut nearest: Option<(usize, f64)> = None;
        let mut near_any = false;
        for (place, note) in expected.iter().enumerate() {
            let distance = (press.at_ms - note.at_ms).abs();
            if distance > window_ms {
                continue;
            }
            if matched[place].is_some() {
                continue;
            }
            near_any = true;
            if note.pitches != press.pitches {
                continue;
            }
            let closer = match nearest {
                Some((_, best)) => distance < best,
                None => true,
            };
            if closer {
                nearest = Some((place, distance));
            }
        }
        match nearest {
            Some((place, _)) => matched[place] = Some(press_place),
            None => {
                if near_any {
                    wrong += 1;
                } else {
                    extras += 1;
                }
            }
        }
    }

    let mut notes = Vec::new();
    for (place, note) in expected.iter().enumerate() {
        let result = match matched[place] {
            Some(press_place) => {
                let press = &heard[press_place];
                let mut held_share = None;
                if let Some(held) = press.held_ms {
                    held_share = Some((held / note.hold_ms).min(2.0));
                }
                NoteResult {
                    index: note.index,
                    hit: true,
                    offset_ms: Some(press.at_ms - note.at_ms),
                    held_share,
                }
            }
            None => NoteResult {
                index: note.index,
                hit: false,
                offset_ms: None,
                held_share: None,
            },
        };
        notes.push(result);
    }

    let whole = tally(&notes, |_| true);
    let left = hand_tally(&notes, expected, Hand::Left);
    let right = hand_tally(&notes, expected, Hand::Right);
    let mut sections = Vec::new();
    for note in expected {
        if let Some(name) = &note.section {
            let known = sections.iter().any(|(known, _): &(String, Tally)| known == name);
            if !known {
                let section_tally = tally(&notes, |place| expected[place].section.as_ref() == Some(name));
                sections.push((name.clone(), section_tally));
            }
        }
    }
    let together_ms = together(&notes, expected);

    let mut clean = whole.missed == 0 && wrong == 0 && extras == 0;
    if let Some(offset) = whole.mean_offset_ms {
        if offset.abs() > window_ms / 2.0 {
            clean = false;
        }
    }
    Scored {
        whole,
        left,
        right,
        sections,
        wrong,
        extras,
        together_ms,
        clean,
        notes,
    }
}

fn hand_tally(notes: &[NoteResult], expected: &[Expected], hand: Hand) -> Option<Tally> {
    let has_hand = expected.iter().any(|note| note.hand == hand);
    if !has_hand {
        return None;
    }
    Some(tally(notes, |place| expected[place].hand == hand))
}

// The tally of the notes at the places `counts` picks.
fn tally(notes: &[NoteResult], counts: impl Fn(usize) -> bool) -> Tally {
    let mut counted = Tally::default();
    let mut offsets = Vec::new();
    let mut holds = Vec::new();
    for (place, note) in notes.iter().enumerate() {
        if !counts(place) {
            continue;
        }
        counted.notes += 1;
        if !note.hit {
            counted.missed += 1;
            continue;
        }
        counted.hits += 1;
        if let Some(offset) = note.offset_ms {
            offsets.push(offset);
        }
        if let Some(share) = note.held_share {
            let closeness = 1.0 - (1.0 - share).abs();
            holds.push(closeness.max(0.0) * 100.0);
        }
    }
    if counted.notes > 0 {
        counted.accuracy_percent = rounded(counted.hits as f64 * 100.0 / counted.notes as f64);
    }
    counted.mean_offset_ms = mean(&offsets).map(rounded);
    counted.spread_ms = spread(&offsets).map(rounded);
    counted.hold_percent = mean(&holds).map(rounded);
    counted
}

// How far apart the hands landed on the beats they share, on average.
fn together(notes: &[NoteResult], expected: &[Expected]) -> Option<f64> {
    let mut gaps = Vec::new();
    for (place, note) in expected.iter().enumerate() {
        if note.hand != Hand::Left {
            continue;
        }
        let Some(left_offset) = notes[place].offset_ms else {
            continue;
        };
        for (other_place, other) in expected.iter().enumerate() {
            if other.hand != Hand::Right {
                continue;
            }
            if (other.at_ms - note.at_ms).abs() > 1.0 {
                continue;
            }
            if let Some(right_offset) = notes[other_place].offset_ms {
                gaps.push((left_offset - right_offset).abs());
            }
        }
    }
    mean(&gaps).map(rounded)
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sum: f64 = values.iter().sum();
    Some(sum / values.len() as f64)
}

// The standard deviation, how far values spread around their average.
fn spread(values: &[f64]) -> Option<f64> {
    let average = mean(values)?;
    let mut squares = Vec::new();
    for value in values {
        squares.push((value - average) * (value - average));
    }
    let variance = mean(&squares)?;
    Some(variance.sqrt())
}

fn rounded(value: f64) -> f64 {
    value.round()
}

// The ear's results: the player played back what Jello played, in order,
// with timing left out. Notes Jello played together count as one step.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Echoed {
    pub steps: usize,
    pub right: usize,
    // The step, from 1, where the first mistake was, or None.
    pub first_wrong: Option<usize>,
    pub extra_presses: usize,
}

// Compares the steps of `expected` (in order) with the presses of `heard`.
pub fn echo(expected: &[Expected], heard: &[Heard]) -> Echoed {
    let steps = steps_of(expected);
    let mut right = 0;
    let mut first_wrong = None;
    for (place, step) in steps.iter().enumerate() {
        let Some(press) = heard.get(place) else {
            if first_wrong.is_none() {
                first_wrong = Some(place + 1);
            }
            continue;
        };
        if press.pitches == *step {
            right += 1;
        } else if first_wrong.is_none() {
            first_wrong = Some(place + 1);
        }
    }
    let mut extra_presses = 0;
    if heard.len() > steps.len() {
        extra_presses = heard.len() - steps.len();
    }
    Echoed {
        steps: steps.len(),
        right,
        first_wrong,
        extra_presses,
    }
}

// The pitches of each step: notes starting together joined, in time order.
fn steps_of(expected: &[Expected]) -> Vec<Vec<u8>> {
    let mut sorted: Vec<&Expected> = expected.iter().collect();
    sorted.sort_by(|first, second| first.at_ms.total_cmp(&second.at_ms));
    let mut steps: Vec<(f64, Vec<u8>)> = Vec::new();
    for note in sorted {
        if let Some((at, pitches)) = steps.last_mut() {
            if (*at - note.at_ms).abs() <= 1.0 {
                pitches.extend(note.pitches.iter().copied());
                pitches.sort();
                continue;
            }
        }
        steps.push((note.at_ms, note.pitches.clone()));
    }
    let mut pitches = Vec::new();
    for (_, step) in steps {
        pitches.push(step);
    }
    pitches
}

// What improvising over a progression measured.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Improvised {
    pub notes: usize,
    pub in_key_percent: Option<f64>,
    // Of the notes landing on beats 1 and 3, how many are in the chord
    // sounding then.
    pub chord_tones_on_strong_beats_percent: Option<f64>,
    // How many different gaps between notes, in eighths: more is more
    // rhythmic variety.
    pub rhythm_kinds: usize,
    // How often a run of three steps (its intervals) comes back.
    pub motif_repeats: usize,
}

// Measures `heard` played over `chords`, the pitches sounding on each beat
// (None for no chord), in `key`, at `ms_per_beat`. A note lands on a beat
// within `window_ms`.
pub fn improvised(heard: &[Heard], chords: &[Option<Vec<u8>>], key: Option<Key>, ms_per_beat: f64, window_ms: f64) -> Improvised {
    let mut notes = Vec::new();
    for press in heard {
        for pitch in &press.pitches {
            notes.push((press.at_ms, *pitch));
        }
    }

    let mut in_key_percent = None;
    if let Some(key) = key {
        if !notes.is_empty() {
            let mut inside = 0;
            for (_, pitch) in &notes {
                if note_fit(key, *pitch) == Fit::InKey {
                    inside += 1;
                }
            }
            in_key_percent = Some(rounded(inside as f64 * 100.0 / notes.len() as f64));
        }
    }

    let mut strong = 0;
    let mut chord_tones = 0;
    for (at, pitch) in &notes {
        let beat = (at / ms_per_beat).round();
        if (at - beat * ms_per_beat).abs() > window_ms || beat < 0.0 {
            continue;
        }
        let beat = beat as usize;
        if beat % 2 != 0 {
            continue;
        }
        let Some(Some(chord)) = chords.get(beat) else {
            continue;
        };
        strong += 1;
        let mut in_chord = false;
        for tone in chord {
            if tone % 12 == pitch % 12 {
                in_chord = true;
            }
        }
        if in_chord {
            chord_tones += 1;
        }
    }
    let mut chord_tones_on_strong_beats_percent = None;
    if strong > 0 {
        chord_tones_on_strong_beats_percent = Some(rounded(chord_tones as f64 * 100.0 / strong as f64));
    }

    let eighth = ms_per_beat / 2.0;
    let mut gaps = Vec::new();
    for pair in heard.windows(2) {
        let gap = ((pair[1].at_ms - pair[0].at_ms) / eighth).round() as i64;
        if !gaps.contains(&gap) {
            gaps.push(gap);
        }
    }

    let mut intervals = Vec::new();
    for pair in heard.windows(2) {
        let first = i32::from(pair[0].pitches[0]);
        let second = i32::from(pair[1].pitches[0]);
        intervals.push(second - first);
    }
    let mut motif_repeats = 0;
    let mut seen: Vec<&[i32]> = Vec::new();
    for motif in intervals.windows(3) {
        if seen.contains(&motif) {
            motif_repeats += 1;
        } else {
            seen.push(motif);
        }
    }

    Improvised {
        notes: notes.len(),
        in_key_percent,
        chord_tones_on_strong_beats_percent,
        rhythm_kinds: gaps.len(),
        motif_repeats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(index: usize, hand: Hand, pitch: u8, at_ms: f64) -> Expected {
        Expected {
            index,
            hand,
            pitches: vec![pitch],
            at_ms,
            hold_ms: 500.0,
            section: None,
        }
    }

    fn press(pitch: u8, at_ms: f64, held_ms: f64) -> Heard {
        Heard {
            pitches: vec![pitch],
            at_ms,
            held_ms: Some(held_ms),
        }
    }

    #[test]
    fn presses_inside_the_window_hit_the_nearest_note() {
        let expected = vec![
            note(0, Hand::Right, 60, 0.0),
            note(1, Hand::Right, 62, 500.0),
            note(2, Hand::Right, 64, 1000.0),
        ];
        let heard = vec![press(60, 20.0, 500.0), press(62, 540.0, 250.0), press(64, 1300.0, 500.0)];
        let scored = score(&expected, &heard, 150.0);
        assert_eq!(scored.whole.hits, 2);
        assert_eq!(scored.whole.missed, 1);
        assert_eq!(scored.extras, 1);
        assert_eq!(scored.wrong, 0);
        assert_eq!(scored.whole.mean_offset_ms, Some(30.0));
        assert_eq!(scored.whole.spread_ms, Some(10.0));
        // One hold full, one half.
        assert_eq!(scored.whole.hold_percent, Some(75.0));
        assert!(!scored.clean);
        assert_eq!(scored.notes[1].offset_ms, Some(40.0));
        assert!(scored.left.is_none());
    }

    #[test]
    fn the_window_is_the_difficulty() {
        let expected = vec![note(0, Hand::Right, 60, 0.0)];
        let heard = vec![press(60, 60.0, 500.0)];
        assert_eq!(score(&expected, &heard, 150.0).whole.hits, 1);
        assert_eq!(score(&expected, &heard, 40.0).whole.hits, 0);
    }

    #[test]
    fn a_wrong_note_near_a_note_is_wrong_not_extra() {
        let expected = vec![note(0, Hand::Right, 60, 0.0)];
        let heard = vec![press(61, 10.0, 500.0), press(60, 30.0, 500.0)];
        let scored = score(&expected, &heard, 150.0);
        assert_eq!(scored.wrong, 1);
        assert_eq!(scored.extras, 0);
        assert_eq!(scored.whole.hits, 1);
        assert!(!scored.clean);
    }

    #[test]
    fn both_hands_are_tallied_with_how_together_they_are() {
        let expected = vec![
            note(0, Hand::Left, 48, 0.0),
            note(1, Hand::Right, 64, 0.0),
            note(2, Hand::Right, 65, 500.0),
        ];
        let heard = vec![press(48, -10.0, 500.0), press(64, 20.0, 500.0), press(65, 505.0, 500.0)];
        let scored = score(&expected, &heard, 100.0);
        assert!(scored.clean);
        assert_eq!(scored.together_ms, Some(30.0));
        assert_eq!(scored.left.as_ref().unwrap().notes, 1);
        assert_eq!(scored.right.as_ref().unwrap().hits, 2);
    }

    #[test]
    fn sections_are_tallied_on_their_own() {
        let mut first = note(0, Hand::Right, 60, 0.0);
        first.section = Some(String::from("A"));
        let mut second = note(1, Hand::Right, 62, 500.0);
        second.section = Some(String::from("B"));
        let scored = score(&[first, second], &[press(60, 0.0, 500.0)], 100.0);
        assert_eq!(scored.sections.len(), 2);
        assert_eq!(scored.sections[0].1.accuracy_percent, 100.0);
        assert_eq!(scored.sections[1].1.accuracy_percent, 0.0);
    }

    #[test]
    fn the_ear_compares_steps_in_order() {
        let expected = vec![note(0, Hand::Right, 60, 0.0), note(1, Hand::Right, 64, 500.0), note(2, Hand::Right, 67, 1000.0)];
        let heard = vec![press(60, 3000.0, 100.0), press(65, 3400.0, 100.0), press(67, 3900.0, 100.0)];
        let echoed = echo(&expected, &heard);
        assert_eq!(echoed.right, 2);
        assert_eq!(echoed.first_wrong, Some(2));
        let short = echo(&expected, &heard[..1]);
        assert_eq!(short.first_wrong, Some(2));
    }

    #[test]
    fn improvising_counts_chord_tones_on_strong_beats() {
        let key = crate::harmony::key_of_name("C major");
        // C on beat 1 over C, F# on beat 3 over C, then E and G off the beat.
        let heard = vec![press(60, 0.0, 100.0), press(66, 1000.0, 100.0), press(64, 1250.0, 100.0), press(67, 1750.0, 100.0)];
        let c = Some(vec![48, 52, 55]);
        let chords = vec![c.clone(), c.clone(), c.clone(), c];
        let measured = improvised(&heard, &chords, key, 500.0, 60.0);
        assert_eq!(measured.chord_tones_on_strong_beats_percent, Some(50.0));
        assert_eq!(measured.in_key_percent, Some(75.0));
        assert_eq!(measured.rhythm_kinds, 3);
    }
}
