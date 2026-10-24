// The player's skills, measured from every practice try Jello kept in
// Music/Jello/Memory/practice.jsonl (docs/design.md#the-tutor-to-mastery).
// Each area gets a level out of ten and a trend, from the tries of that
// area. Writing isn't measured yet: the Composer works with it.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::practice::LEVELS;

// The areas a try can count toward, then writing, which none does.
pub const AREAS: [&str; 6] = ["technique", "rhythm", "harmony", "ear", "improvising", "pieces"];

// How many tries the level and the trend look at.
const LAST_TRIES: usize = 5;
// How many recent points each area shows, for a chart.
const MOST_RECENT: usize = 20;
// How far the last tries' mean must move from the ones before to count.
const TREND_STEP: f64 = 0.5;
// The most lines read, newest kept.
const MOST_LINES: usize = 5000;

// The area a try counts toward: the one its piece was set for, or else the
// one its mode practices.
pub fn area_for(piece_area: Option<&str>, mode: &str) -> String {
    if let Some(area) = piece_area {
        return area.to_string();
    }
    if mode == "ear" {
        return String::from("ear");
    }
    if mode == "improvise" {
        return String::from("improvising");
    }
    String::from("pieces")
}

// What one try is worth, from 0 to 10: its level, plus how well it went.
// A clean try at master is ten. Wait-mode tries aren't timed, so they
// count for nothing.
pub fn points_of(line: &Value) -> Option<f64> {
    if line["wait"] == json!(true) {
        return None;
    }
    let level = line["level"].as_str()?;
    let level_place = LEVELS.iter().position(|(name, _)| *name == level)?;
    let quality = quality_of(line)?;
    let points = 10.0 * (level_place as f64 + quality) / LEVELS.len() as f64;
    Some(points.round())
}

// How well a try went, from 0 to 1: 1 when clean, or else most of how
// much of it was right.
fn quality_of(line: &Value) -> Option<f64> {
    if line["clean"] == json!(true) {
        return Some(1.0);
    }
    let percent = percent_of(line)?;
    Some(percent / 100.0 * 0.8)
}

// A try's headline percent: the notes hit, the steps echoed right, or
// the notes improvised in key.
fn percent_of(line: &Value) -> Option<f64> {
    if let Some(percent) = line["whole"]["accuracy_percent"].as_f64() {
        return Some(percent);
    }
    let echo = &line["echo"];
    if let (Some(right), Some(steps)) = (echo["right"].as_f64(), echo["steps"].as_f64()) {
        if steps > 0.0 {
            return Some(100.0 * right / steps);
        }
        return None;
    }
    line["improvised"]["in_key_percent"].as_f64()
}

// Every area's level and trend from the tries in `lines`, oldest first.
pub fn skills_of(lines: &[Value]) -> Value {
    let mut areas = Vec::new();
    for area in AREAS {
        let mut points = Vec::new();
        for line in lines {
            let mode = line["mode"].as_str().unwrap_or("along");
            let line_area = area_for(line["area"].as_str(), mode);
            if line_area != area {
                continue;
            }
            if let Some(worth) = points_of(line) {
                points.push(worth);
            }
        }
        areas.push(area_view(area, &points));
    }
    areas.push(json!({"area": "writing", "measured": false}));
    json!(areas)
}

fn area_view(area: &str, points: &[f64]) -> Value {
    if points.is_empty() {
        return json!({"area": area, "measured": true, "tries": 0, "level": null, "trend": null, "recent": []});
    }
    let last_start = points.len().saturating_sub(LAST_TRIES);
    let last = &points[last_start..];
    let before_start = last_start.saturating_sub(LAST_TRIES);
    let before = &points[before_start..last_start];
    let recent_start = points.len().saturating_sub(MOST_RECENT);
    json!({
        "area": area,
        "measured": true,
        "tries": points.len(),
        "level": median_of(last),
        "trend": trend_of(last, before),
        "recent": &points[recent_start..],
    })
}

// The middle of `points`, rounded down to a whole dot.
fn median_of(points: &[f64]) -> f64 {
    let mut sorted = points.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        return sorted[middle];
    }
    ((sorted[middle - 1] + sorted[middle]) / 2.0).floor()
}

// Whether the last tries went up, down or held steady against the ones
// before them, or "new" with nothing before to compare.
fn trend_of(last: &[f64], before: &[f64]) -> &'static str {
    if before.is_empty() {
        return "new";
    }
    let change = mean_of(last) - mean_of(before);
    if change >= TREND_STEP {
        return "up";
    }
    if change <= -TREND_STEP {
        return "down";
    }
    "steady"
}

fn mean_of(points: &[f64]) -> f64 {
    let total: f64 = points.iter().sum();
    total / points.len() as f64
}

// The tries kept in `folder`'s practice.jsonl, oldest first. Lines that
// aren't JSON are left out.
pub fn tries_in(folder: &Path) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(folder.join("practice.jsonl")) else {
        return Vec::new();
    };
    let mut tries = Vec::new();
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            tries.push(value);
        }
    }
    if tries.len() > MOST_LINES {
        tries.drain(..tries.len() - MOST_LINES);
    }
    tries
}

// The skills for jello_look and the Memory page, from the memory folder
// `folder`, if Jello has one.
pub fn skills_view(folder: Option<&Path>) -> Value {
    let mut tries = Vec::new();
    if let Some(folder) = folder {
        tries = tries_in(folder);
    }
    skills_of(&tries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::test_folder;

    fn along(level: &str, accuracy: f64, clean: bool) -> Value {
        json!({"mode": "along", "wait": false, "level": level, "clean": clean, "whole": {"accuracy_percent": accuracy}})
    }

    #[test]
    fn a_try_is_worth_its_level_and_how_it_went() {
        assert_eq!(points_of(&along("master", 100.0, true)), Some(10.0));
        assert_eq!(points_of(&along("beginner", 0.0, false)), Some(0.0));
        // Intermediate is level 2; half right is 0.4: 10 * 2.4 / 6 = 4.
        assert_eq!(points_of(&along("intermediate", 50.0, false)), Some(4.0));
        let waited = json!({"mode": "along", "wait": true, "level": "master", "clean": true});
        assert_eq!(points_of(&waited), None);
        let echo = json!({"mode": "ear", "level": "novice", "clean": false, "echo": {"right": 3, "steps": 4}});
        // Novice is 1; 75% is 0.6: 10 * 1.6 / 6 = 2.67, so 3.
        assert_eq!(points_of(&echo), Some(3.0));
        let wizard = json!({"mode": "along", "level": "wizard", "clean": true});
        assert_eq!(points_of(&wizard), None);
    }

    #[test]
    fn tries_count_toward_their_area() {
        assert_eq!(area_for(None, "ear"), "ear");
        assert_eq!(area_for(None, "improvise"), "improvising");
        assert_eq!(area_for(None, "memory"), "pieces");
        assert_eq!(area_for(Some("rhythm"), "along"), "rhythm");

        let mut rhythm = along("master", 100.0, true);
        rhythm["area"] = json!("rhythm");
        let skills = skills_of(&[along("novice", 90.0, false), rhythm]);
        assert_eq!(skills[1]["area"], "rhythm");
        assert_eq!(skills[1]["level"], 10.0);
        assert_eq!(skills[5]["area"], "pieces");
        assert_eq!(skills[5]["tries"], 1);
        assert_eq!(skills[0]["tries"], 0);
        assert_eq!(skills[6], json!({"area": "writing", "measured": false}));
    }

    #[test]
    fn the_level_is_the_middle_of_the_last_five_and_the_trend_compares() {
        let mut lines = Vec::new();
        for _ in 0..5 {
            lines.push(along("beginner", 50.0, false));
        }
        for accuracy in [100.0, 100.0, 50.0, 100.0, 100.0] {
            lines.push(along("advanced", accuracy, accuracy == 100.0));
        }
        let pieces = &skills_of(&lines)[5];
        // Advanced is 3: clean is 10 * 4 / 6 = 7, half right 10 * 3.4 / 6 = 6.
        assert_eq!(pieces["level"], 7.0);
        assert_eq!(pieces["trend"], "up");
        assert_eq!(pieces["recent"].as_array().unwrap().len(), 10);

        let few = skills_of(&[along("novice", 100.0, true)]);
        assert_eq!(few[5]["trend"], "new");
    }

    #[test]
    fn the_file_is_read_and_broken_lines_left_out() {
        let folder = test_folder("skills-file");
        fs::create_dir_all(&folder).unwrap();
        let text = format!("{}\nnot json\n{}\n", along("novice", 80.0, false), along("expert", 100.0, true));
        fs::write(folder.join("practice.jsonl"), text).unwrap();
        assert_eq!(tries_in(&folder).len(), 2);
        let skills = skills_view(Some(&folder));
        assert_eq!(skills[5]["tries"], 2);
        let empty = skills_view(None);
        assert_eq!(empty[5]["tries"], 0);
    }
}
