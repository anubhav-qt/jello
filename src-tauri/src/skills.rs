// The player's skills, measured from every practice try Jello kept in
// Music/Jello/Memory/practice.jsonl (docs/design.md#the-tutor-to-mastery).
// Each area gets a level out of ten and a trend, from the tries of that
// area. Writing isn't measured yet: the Composer works with it.
//
// A try of a section of the player's own song (practice_song.rs) also
// counts per bar: each bar's points go to every skill it needs
// (song_skills.rs), and to that skill's area.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::practice::LEVELS;
use crate::song_skills;

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
        for line in lines {
            for (worth, skills) in bar_points(line) {
                for skill in skills {
                    if song_skills::area_of(&skill) == Some(area) {
                        points.push(worth);
                    }
                }
            }
        }
        areas.push(area_view(area, &points));
    }
    areas.push(json!({"area": "writing", "measured": false}));
    json!(areas)
}

// The points of each bar a song try played, with what the bar needs.
// Not a song try, or a wait-mode one, gives none.
fn bar_points(line: &Value) -> Vec<(f64, Vec<String>)> {
    let mut found = Vec::new();
    let Some(bars) = line["song"]["bars"].as_array() else {
        return found;
    };
    let sections = line["sections"].as_array().cloned().unwrap_or_default();
    for bar in bars {
        let Some(number) = bar["bar"].as_u64() else {
            continue;
        };
        let name = format!("Bar {number}");
        let mut tally = Value::Null;
        for section in &sections {
            if section["name"] == json!(name) {
                tally = section["tally"].clone();
            }
        }
        if tally.is_null() {
            continue;
        }
        let notes = tally["notes"].as_u64().unwrap_or(0);
        let hits = tally["hits"].as_u64().unwrap_or(0);
        let bar_line = json!({
            "level": line["level"],
            "wait": line["wait"],
            "clean": notes > 0 && hits == notes && tally["missed"] == json!(0),
            "whole": {"accuracy_percent": tally["accuracy_percent"]},
        });
        let Some(worth) = points_of(&bar_line) else {
            continue;
        };
        let mut skills = Vec::new();
        for skill in bar["skills"].as_array().cloned().unwrap_or_default() {
            if let Some(skill) = skill.as_str() {
                skills.push(skill.to_string());
            }
        }
        found.push((worth, skills));
    }
    found
}

// Every skill from songs the player practiced, in the order first met:
// its area, level and trend from the tries of bars that need it, and the
// song, section and bar it was last met in.
pub fn song_skills_of(lines: &[Value]) -> Value {
    let mut names: Vec<String> = Vec::new();
    let mut points: Vec<Vec<f64>> = Vec::new();
    let mut last_seen: Vec<Value> = Vec::new();
    for line in lines {
        let Some(bars) = line["song"]["bars"].as_array() else {
            continue;
        };
        let worths = bar_points(line);
        for (place, (worth, skills)) in worths.into_iter().enumerate() {
            let bar = bars.get(place).map(|found| found["bar"].clone()).unwrap_or(Value::Null);
            for skill in skills {
                let known = names.iter().position(|name| *name == skill);
                let at;
                if let Some(known) = known {
                    at = known;
                } else {
                    names.push(skill.clone());
                    points.push(Vec::new());
                    last_seen.push(Value::Null);
                    at = names.len() - 1;
                }
                points[at].push(worth);
                last_seen[at] = json!({
                    "song": line["song"]["song"],
                    "section": line["song"]["section"],
                    "bar": bar,
                });
            }
        }
    }
    let mut skills = Vec::new();
    for (place, name) in names.iter().enumerate() {
        let mut view = area_view(song_skills::area_of(name).unwrap_or("pieces"), &points[place]);
        view["skill"] = json!(name);
        view["from"] = last_seen[place].clone();
        skills.push(view);
    }
    json!(skills)
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

// The skills from songs, for jello_look and the Memory page, the same way.
pub fn song_skills_view(folder: Option<&Path>) -> Value {
    let mut tries = Vec::new();
    if let Some(folder) = folder {
        tries = tries_in(folder);
    }
    song_skills_of(&tries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::test_folder;

    // A song try at `level` whose bars each had `notes` notes and these
    // misses, needing these skills.
    fn song_try(level: &str, bars: &[(u64, &[&str])]) -> Value {
        let mut sections = Vec::new();
        let mut listed = Vec::new();
        for (place, (missed, skills)) in bars.iter().enumerate() {
            let bar = place + 1;
            let accuracy = 100.0 * (4 - missed) as f64 / 4.0;
            sections.push(json!({
                "name": format!("Bar {bar}"),
                "tally": {"notes": 4, "hits": 4 - missed, "missed": missed, "accuracy_percent": accuracy},
            }));
            listed.push(json!({"bar": bar, "skills": skills}));
        }
        json!({
            "level": level,
            "mode": "along",
            "clean": false,
            "whole": {"accuracy_percent": 50.0},
            "sections": sections,
            "song": {"song": "Song", "section": "Verse", "track": "Piano", "bars": listed},
        })
    }

    #[test]
    fn each_bar_counts_toward_the_skills_it_needs_and_their_areas() {
        let lines = vec![
            song_try("beginner", &[(0, &["run of eighths"]), (2, &["hands together"])]),
            song_try("beginner", &[(0, &["run of eighths"])]),
        ];
        let skills = song_skills_of(&lines);
        assert_eq!(skills.as_array().unwrap().len(), 2);
        assert_eq!(skills[0]["skill"], json!("run of eighths"));
        assert_eq!(skills[0]["area"], json!("rhythm"));
        assert_eq!(skills[0]["tries"], json!(2));
        // A clean bar at beginner is worth 10 * (0 + 1) / 6, rounded: 2.
        assert_eq!(skills[0]["level"], json!(2.0));
        assert_eq!(skills[0]["from"], json!({"song": "Song", "section": "Verse", "bar": 1}));
        assert_eq!(skills[1]["skill"], json!("hands together"));
        assert_eq!(skills[1]["area"], json!("technique"));

        let areas = skills_of(&lines);
        let rhythm = areas.as_array().unwrap().iter().find(|area| area["area"] == json!("rhythm")).unwrap();
        assert_eq!(rhythm["tries"], json!(2));
        let pieces = areas.as_array().unwrap().iter().find(|area| area["area"] == json!("pieces")).unwrap();
        assert_eq!(pieces["tries"], json!(2));
    }

    #[test]
    fn tries_that_are_not_from_songs_name_no_song_skills() {
        let lines = vec![along("beginner", 80.0, false)];
        assert_eq!(song_skills_of(&lines), json!([]));
    }

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
