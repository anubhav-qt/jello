// Skills from songs (docs/design.md#ai-and-learning-rethought-2026-11-03).
// Jello names what each practiced bar needs, measured from its presses:
// like "chord change on the and", "left hand holds while the right moves"
// or "run of sixteenths". Each skill sits under one of the areas of
// skills.rs, and gets its own level from the tries of the bars that need
// it. New ones appear only when a song needs something new.

use crate::scoring::Hand;

pub const BAR_BEATS: f64 = 4.0;

// Every skill a bar can need, and its area.
pub const SKILLS: [(&str, &str); 10] = [
    ("run of sixteenths", "rhythm"),
    ("run of eighths", "rhythm"),
    ("chord change on the and", "rhythm"),
    ("notes held across the beat", "rhythm"),
    ("left hand holds while the right moves", "technique"),
    ("right hand holds while the left moves", "technique"),
    ("hands together", "technique"),
    ("leap past a fifth", "technique"),
    ("same key again and again", "technique"),
    ("chords in both hands", "harmony"),
];

// How close two times must be, in beats, to count as the same.
const SAME_TIME: f64 = 0.06;
// How many presses in a row make a run.
const RUN_PRESSES: usize = 4;
// How long a hold must be, in beats, for the other hand to move under it,
// and how many presses the other hand must start meanwhile.
const LONG_HOLD_BEATS: f64 = 1.5;
const MOVES_UNDER_HOLD: usize = 2;
// How far past the next beat a held note must reach, in beats.
const ACROSS_BEAT: f64 = 0.25;
// The widest step, in half steps, before it is a leap.
const WIDEST_STEP: i32 = 7;
// How many presses of one key in a row are "again and again".
const SAME_KEY_PRESSES: usize = 3;
// How many times both hands must start together.
const TOGETHER_TIMES: usize = 2;

// One press of the practiced track: its notes, its key and hand, and when,
// in beats from the section's start.
#[derive(Clone, Debug, PartialEq)]
pub struct SongPress {
    pub pitches: Vec<u8>,
    pub key: String,
    pub hand: Hand,
    pub start: f64,
    pub hold: f64,
}

// The area of `skill`, or None if Jello doesn't measure it.
pub fn area_of(skill: &str) -> Option<&'static str> {
    for (name, area) in SKILLS {
        if name == skill {
            return Some(area);
        }
    }
    None
}

// What bar `bar` (from 1) needs, in the order of SKILLS. `presses` are the
// whole section's, in the order they start.
pub fn skills_of_bar(presses: &[SongPress], bar: u32) -> Vec<String> {
    let from = f64::from(bar - 1) * BAR_BEATS;
    let to = from + BAR_BEATS;
    let mut in_bar = Vec::new();
    for press in presses {
        if press.start >= from - SAME_TIME / 2.0 && press.start < to - SAME_TIME / 2.0 {
            in_bar.push(press.clone());
        }
    }
    let left = of_hand(&in_bar, Hand::Left);
    let right = of_hand(&in_bar, Hand::Right);

    let mut needs = Vec::new();
    if has_run(&left, 0.25) || has_run(&right, 0.25) {
        needs.push("run of sixteenths");
    }
    if has_run(&left, 0.5) || has_run(&right, 0.5) {
        needs.push("run of eighths");
    }
    if has_chord_on_the_and(&in_bar) {
        needs.push("chord change on the and");
    }
    if has_hold_across_the_beat(&in_bar) {
        needs.push("notes held across the beat");
    }
    if holds_while_other_moves(&left, &right) {
        needs.push("left hand holds while the right moves");
    }
    if holds_while_other_moves(&right, &left) {
        needs.push("right hand holds while the left moves");
    }
    if together_count(&left, &right, false) >= TOGETHER_TIMES {
        needs.push("hands together");
    }
    if has_leap(&left) || has_leap(&right) {
        needs.push("leap past a fifth");
    }
    if has_same_key_again(&left) || has_same_key_again(&right) {
        needs.push("same key again and again");
    }
    if together_count(&left, &right, true) > 0 {
        needs.push("chords in both hands");
    }

    let mut names = Vec::new();
    for need in needs {
        names.push(String::from(need));
    }
    names
}

fn of_hand(presses: &[SongPress], hand: Hand) -> Vec<SongPress> {
    let mut found = Vec::new();
    for press in presses {
        if press.hand == hand {
            found.push(press.clone());
        }
    }
    found
}

fn is_near(a: f64, b: f64) -> bool {
    (a - b).abs() < SAME_TIME
}

// Whether RUN_PRESSES presses in a row start `gap` beats apart.
fn has_run(presses: &[SongPress], gap: f64) -> bool {
    let mut in_row = 1;
    for place in 1..presses.len() {
        let step = presses[place].start - presses[place - 1].start;
        if is_near(step, gap) {
            in_row += 1;
        } else {
            in_row = 1;
        }
        if in_row >= RUN_PRESSES {
            return true;
        }
    }
    false
}

// The part of `beat` after its whole beats.
fn after_the_beat(beat: f64) -> f64 {
    beat - beat.floor()
}

fn has_chord_on_the_and(presses: &[SongPress]) -> bool {
    for press in presses {
        if press.pitches.len() >= 2 && is_near(after_the_beat(press.start), 0.5) {
            return true;
        }
    }
    false
}

// A press that starts off the beat and is held well past the next one.
fn has_hold_across_the_beat(presses: &[SongPress]) -> bool {
    for press in presses {
        let rest = after_the_beat(press.start);
        let on_the_beat = is_near(rest, 0.0) || is_near(rest, 1.0);
        if on_the_beat {
            continue;
        }
        let next_beat = press.start.ceil();
        if press.start + press.hold >= next_beat + ACROSS_BEAT {
            return true;
        }
    }
    false
}

// Whether one of `holding`'s presses is held long while `moving` starts
// MOVES_UNDER_HOLD presses or more under it.
fn holds_while_other_moves(holding: &[SongPress], moving: &[SongPress]) -> bool {
    for held in holding {
        if held.hold < LONG_HOLD_BEATS {
            continue;
        }
        let end = held.start + held.hold;
        let mut moves = 0;
        for press in moving {
            if press.start > held.start + SAME_TIME && press.start < end - SAME_TIME {
                moves += 1;
            }
        }
        if moves >= MOVES_UNDER_HOLD {
            return true;
        }
    }
    false
}

// How many times both hands start together; with `chords`, only when both
// play two notes or more.
fn together_count(left: &[SongPress], right: &[SongPress], chords: bool) -> usize {
    let mut count = 0;
    for first in left {
        for second in right {
            if !is_near(first.start, second.start) {
                continue;
            }
            if chords && (first.pitches.len() < 2 || second.pitches.len() < 2) {
                continue;
            }
            count += 1;
        }
    }
    count
}

fn top_of(press: &SongPress) -> i32 {
    let mut top = 0;
    for pitch in &press.pitches {
        top = top.max(i32::from(*pitch));
    }
    top
}

fn has_leap(presses: &[SongPress]) -> bool {
    for place in 1..presses.len() {
        let step = top_of(&presses[place]) - top_of(&presses[place - 1]);
        if step.abs() > WIDEST_STEP {
            return true;
        }
    }
    false
}

fn has_same_key_again(presses: &[SongPress]) -> bool {
    let mut in_row = 1;
    for place in 1..presses.len() {
        if presses[place].key == presses[place - 1].key {
            in_row += 1;
        } else {
            in_row = 1;
        }
        if in_row >= SAME_KEY_PRESSES {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(hand: Hand, key: &str, pitches: &[u8], start: f64, hold: f64) -> SongPress {
        SongPress {
            pitches: pitches.to_vec(),
            key: String::from(key),
            hand,
            start,
            hold,
        }
    }

    fn right_run(gap: f64, count: usize) -> Vec<SongPress> {
        let mut presses = Vec::new();
        for place in 0..count {
            let pitch = 60 + place as u8;
            let key = format!("Key{place}");
            presses.push(press(Hand::Right, &key, &[pitch], place as f64 * gap, gap));
        }
        presses
    }

    #[test]
    fn four_sixteenths_in_a_row_are_a_run_and_three_are_not() {
        assert_eq!(skills_of_bar(&right_run(0.25, 4), 1), vec!["run of sixteenths"]);
        assert!(skills_of_bar(&right_run(0.25, 3), 1).is_empty());
        assert_eq!(skills_of_bar(&right_run(0.5, 4), 1), vec!["run of eighths"]);
    }

    #[test]
    fn a_chord_on_the_and_and_a_note_held_across_the_beat() {
        let presses = vec![press(Hand::Left, "KeyC", &[48, 52, 55], 1.5, 0.4)];
        assert_eq!(skills_of_bar(&presses, 1), vec!["chord change on the and"]);
        let held = vec![press(Hand::Right, "KeyJ", &[64], 1.5, 1.0)];
        assert_eq!(skills_of_bar(&held, 1), vec!["notes held across the beat"]);
        let short = vec![press(Hand::Right, "KeyJ", &[64], 1.5, 0.5)];
        assert!(skills_of_bar(&short, 1).is_empty());
    }

    #[test]
    fn one_hand_holds_while_the_other_moves() {
        let mut presses = vec![press(Hand::Left, "KeyC", &[48], 0.0, 4.0)];
        presses.push(press(Hand::Right, "KeyJ", &[64], 1.0, 1.0));
        presses.push(press(Hand::Right, "KeyK", &[65], 2.0, 1.0));
        let needs = skills_of_bar(&presses, 1);
        assert_eq!(needs, vec!["left hand holds while the right moves"]);
    }

    #[test]
    fn hands_together_twice_and_chords_in_both() {
        let presses = vec![
            press(Hand::Left, "KeyC", &[48], 0.0, 1.0),
            press(Hand::Right, "KeyJ", &[64], 0.02, 1.0),
            press(Hand::Left, "KeyD", &[50, 54], 2.0, 1.0),
            press(Hand::Right, "KeyK", &[66, 69], 2.0, 1.0),
        ];
        let needs = skills_of_bar(&presses, 1);
        assert_eq!(needs, vec!["hands together", "chords in both hands"]);
        let once = vec![
            press(Hand::Left, "KeyC", &[48], 0.0, 1.0),
            press(Hand::Right, "KeyJ", &[64], 0.0, 1.0),
        ];
        assert!(skills_of_bar(&once, 1).is_empty());
    }

    #[test]
    fn leaps_and_the_same_key_again() {
        let leap = vec![
            press(Hand::Right, "KeyJ", &[60], 0.0, 1.0),
            press(Hand::Right, "KeyO", &[72], 1.0, 1.0),
        ];
        assert_eq!(skills_of_bar(&leap, 1), vec!["leap past a fifth"]);
        let fifth = vec![
            press(Hand::Right, "KeyJ", &[60], 0.0, 1.0),
            press(Hand::Right, "KeyL", &[67], 1.0, 1.0),
        ];
        assert!(skills_of_bar(&fifth, 1).is_empty());
        let mut again = Vec::new();
        for place in 0..3 {
            again.push(press(Hand::Right, "KeyJ", &[60], place as f64, 1.0));
        }
        assert_eq!(skills_of_bar(&again, 1), vec!["same key again and again"]);
    }

    #[test]
    fn only_the_bar_s_own_presses_count() {
        let mut presses = right_run(0.25, 2);
        presses.extend(right_run(0.25, 4).into_iter().map(|mut moved| {
            moved.start += 4.0;
            moved
        }));
        assert!(skills_of_bar(&presses, 1).is_empty());
        assert_eq!(skills_of_bar(&presses, 2), vec!["run of sixteenths"]);
        assert!(skills_of_bar(&presses, 3).is_empty());
        // A press on the next bar's first beat is that bar's, even a hair early.
        let mut edge = Vec::new();
        for start in [2.0, 3.0, 3.99] {
            edge.push(press(Hand::Right, "KeyJ", &[60], start, 0.5));
        }
        assert!(skills_of_bar(&edge, 1).is_empty());
    }

    #[test]
    fn every_skill_has_an_area_of_the_skill_list() {
        for (name, area) in SKILLS {
            assert_eq!(area_of(name), Some(area));
            assert!(crate::skills::AREAS.contains(&area));
        }
        assert_eq!(area_of("juggling"), None);
    }
}
