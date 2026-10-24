// Independent layer lengths, expanded onto their common cycle before playback.
// The audio thread keeps its existing allocation-free, sample-timed scheduler.
use crate::engine::{LoopLayer, LoopNote};

const MAX_NOTES: usize = 4096;
const MAX_CYCLE_BARS: u32 = 4096;

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

pub fn expand(bars: u32, layers: &[LoopLayer], tempo: u32, sample_rate: u32) -> Result<(u32, Vec<LoopLayer>), String> {
    if !layers.iter().any(|layer| layer.bars > 0) {
        return Ok((bars, layers.to_vec()));
    }
    if tempo == 0 || sample_rate == 0 {
        return Err(String::from("a loop needs a tempo and sample rate"));
    }
    let mut cycle = 1;
    for layer in layers {
        let period = period_of(layer, bars);
        if !(1..=64).contains(&period) {
            return Err(String::from("a layer must be 1 to 64 bars"));
        }
        cycle = (cycle / gcd(cycle, period)).checked_mul(period)
            .ok_or_else(|| String::from("these layer lengths make too long a cycle"))?;
        if cycle > MAX_CYCLE_BARS {
            return Err(String::from("these layer lengths need more than 4096 bars to repeat together"));
        }
    }
    let samples = |count: u32| u64::from(count) * 4 * u64::from(sample_rate) * 60 / u64::from(tempo);
    let mut expanded = Vec::new();
    let mut total = 0;
    for layer in layers {
        let period = period_of(layer, bars);
        let length = samples(period);
        let pattern: Vec<_> = layer.notes.iter().filter(|note| note.start < length).collect();
        if pattern.is_empty() {
            total += 1;
        } else {
            total += pattern.len() * (cycle / period) as usize;
        }
        if total > MAX_NOTES {
            return Err(String::from("these layer lengths need more than 4096 playback notes; use shorter or matching lengths"));
        }
        let mut copy = layer.clone();
        copy.bars = 0;
        copy.notes.clear();
        for repeat in 0..cycle / period {
            let offset = samples(repeat * period);
            for note in &pattern {
                let end = note.end.unwrap_or(note.start).min(length).max(note.start);
                copy.notes.push(LoopNote {
                    note: note.note,
                    start: offset + note.start,
                    end: Some(offset + end),
                });
            }
        }
        // A length shortened past every note remains a silent layer, so it
        // still retains its place and can be lengthened again later.
        if copy.notes.is_empty() {
            copy.notes.push(LoopNote { note: 0, start: 0, end: Some(0) });
            copy.volume = 0;
        }
        expanded.push(copy);
    }
    Ok((cycle, expanded))
}

fn period_of(layer: &LoopLayer, common: u32) -> u32 {
    if layer.bars == 0 {
        return common;
    }
    layer.bars
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(bars: u32) -> LoopLayer {
        LoopLayer {
            bars,
            notes: vec![LoopNote { note: 60, start: 0, end: Some(24000) }],
            volume: 50,
            playing: true,
            recording: false,
        }
    }

    #[test]
    fn three_and_two_bar_patterns_repeat_independently() {
        let (bars, layers) = expand(8, &[layer(3), layer(2)], 120, 48000).unwrap();
        assert_eq!(bars, 6);
        assert_eq!(layers[0].notes.iter().map(|note| note.start).collect::<Vec<_>>(), vec![0, 288000]);
        assert_eq!(layers[1].notes.iter().map(|note| note.start).collect::<Vec<_>>(), vec![0, 192000, 384000]);
        assert_eq!(layers[0].volume, 50);
    }

    #[test]
    fn shortening_clips_playback_without_changing_original_notes() {
        let mut original = layer(1);
        original.notes[0].end = Some(200000);
        original.notes.push(LoopNote { note: 67, start: 100000, end: Some(120000) });
        let (_, expanded) = expand(2, &[original.clone(), layer(2)], 120, 48000).unwrap();
        assert_eq!(expanded[0].notes.len(), 2);
        assert_eq!(expanded[0].notes[0].end, Some(96000));
        assert_eq!(original.notes[0].end, Some(200000));
    }

    #[test]
    fn rejects_large_cycles_before_changing_the_engine() {
        assert!(expand(1, &[layer(61), layer(59), layer(53)], 120, 48000).is_err());
        let mut dense = layer(1);
        dense.notes = vec![dense.notes[0]; 100];
        assert!(expand(1, &[dense, layer(64)], 120, 48000).is_err());
    }

    #[test]
    fn independent_patterns_play_like_their_sample_timed_expansion_and_keep_their_lengths() {
        use crate::engine::Engine;
        let originals = vec![layer(3), layer(2)];
        let (bars, expanded) = expand(8, &originals, 120, 48000).unwrap();
        let mut actual = Engine::new().unwrap();
        let mut reference = Engine::new().unwrap();
        actual.open_loop(120, 8, &originals, 80).unwrap();
        reference.open_loop(120, bars, &expanded, 80).unwrap();
        actual.render(1);
        reference.render(1);
        assert_eq!(actual.loop_layers(), originals.as_slice());
        actual.play_loop();
        reference.play_loop();
        assert_eq!(actual.render(600000), reference.render(600000));
        let before = actual.loop_to_keep().unwrap();
        assert!(actual.set_layer_bars(1, 65).is_err());
        assert_eq!(actual.loop_to_keep().unwrap(), before);
    }

    #[test]
    fn old_loops_keep_their_original_representation() {
        let originals = vec![layer(0)];
        assert_eq!(expand(8, &originals, 120, 48000).unwrap(), (8, originals));
    }
}
