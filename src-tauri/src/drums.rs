// The drum kit (docs/design.md#sampler): sixteen pads, each one drum,
// played as the notes after the vibrato and glide keys', so the looper
// records them like notes. The song says which instrument's recordings the
// kit plays and how each pad sounds. Drums belong to the song only, never
// to a preset.

// The pads' notes start here (`JELLO_DRUM_FIRST_NOTE` in
// engine/include/jello_engine.h), and there are this many.
pub const DRUM_FIRST_NOTE: u8 = 130;
pub const DRUM_PAD_COUNT: usize = 16;

// The most a pad is tuned either way, in half steps, and the choke groups
// there are, from 1 (`JELLO_DRUM_MOST_` in engine/include/jello_engine.h).
pub const DRUM_MOST_HALF_STEPS: i32 = 12;
pub const DRUM_MOST_GROUP: u32 = 4;

// Every pad's name, in pad order. Song files, the keys and the agent all
// call the pads by these.
pub const PAD_NAMES: [&str; DRUM_PAD_COUNT] = [
    "kick",
    "snare",
    "rim",
    "clap",
    "closed hat",
    "pedal hat",
    "open hat",
    "low tom",
    "mid tom",
    "high tom",
    "crash",
    "crash 2",
    "ride",
    "ride bell",
    "china",
    "tambourine",
];

// The General MIDI drum notes each pad plays from an instrument, the most
// fitting first. A drum kit's SFZ file maps its drums to these, and a WAV
// named by a drum, like kick.wav, is read as the first of its pad's.
const GM_NOTES: [&[u8]; DRUM_PAD_COUNT] = [
    &[36, 35],
    &[38, 40],
    &[37],
    &[39],
    &[42],
    &[44],
    &[46],
    &[41, 43],
    &[47, 45, 48],
    &[50, 48],
    &[49],
    &[57, 49],
    &[51, 59],
    &[53],
    &[52],
    &[54],
];

// The hi-hats share a choke group in a new song, so closing the hat cuts
// off an open one, as on a real kit.
const HAT_GROUP: u32 = 1;

// One pad's settings (`jello_engine_set_drum_pad`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrumPad {
    // How loud, in percent, from 0 (silent) to 100 (as recorded).
    pub level: u32,
    // How far it is tuned, in half steps, from minus to plus
    // DRUM_MOST_HALF_STEPS.
    pub half_steps: i32,
    // Where it sits, in percent, from -100 (left) to 100 (right).
    pub pan: i32,
    // Its choke group, from 0 (none) to DRUM_MOST_GROUP.
    pub group: u32,
}

impl DrumPad {
    // The pad with every value brought within its range.
    pub fn within_ranges(self) -> DrumPad {
        DrumPad {
            level: self.level.min(100),
            half_steps: self.half_steps.clamp(-DRUM_MOST_HALF_STEPS, DRUM_MOST_HALF_STEPS),
            pan: self.pan.clamp(-100, 100),
            group: self.group.min(DRUM_MOST_GROUP),
        }
    }
}

// A pad at full level, untuned, in the middle, in no group: where the C++
// engine's pads start.
const PLAIN_PAD: DrumPad = DrumPad {
    level: 100,
    half_steps: 0,
    pan: 0,
    group: 0,
};

// The song's drums: which instrument the kit plays, by name, and how each
// pad sounds.
#[derive(Clone, Debug, PartialEq)]
pub struct Drums {
    pub kit: Option<String>,
    pub pads: [DrumPad; DRUM_PAD_COUNT],
}

impl Drums {
    // A new song's drums: no kit picked, every pad plain but the hats,
    // which choke each other.
    pub fn new() -> Drums {
        let mut pads = [PLAIN_PAD; DRUM_PAD_COUNT];
        for name in ["closed hat", "pedal hat", "open hat"] {
            if let Some(pad) = pad_named(name) {
                pads[pad].group = HAT_GROUP;
            }
        }
        Drums { kit: None, pads }
    }
}

// The pad called `name`, ignoring case, with an underscore or dash as good
// as a space ("closed_hat"), or None.
pub fn pad_named(name: &str) -> Option<usize> {
    let plain = name.trim().to_lowercase().replace(['_', '-'], " ");
    for (pad, pad_name) in PAD_NAMES.iter().enumerate() {
        if *pad_name == plain {
            return Some(pad);
        }
    }
    None
}

// Pad `pad`'s General MIDI notes, the most fitting first.
pub fn gm_notes_of(pad: usize) -> &'static [u8] {
    GM_NOTES[pad]
}

// The pad note `note` hits, or None for a note that is not a pad's.
pub fn pad_of_note(note: u8) -> Option<usize> {
    if note < DRUM_FIRST_NOTE {
        return None;
    }
    let pad = usize::from(note - DRUM_FIRST_NOTE);
    if pad >= DRUM_PAD_COUNT {
        return None;
    }
    Some(pad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_are_found_by_name_however_they_are_written() {
        assert_eq!(pad_named("kick"), Some(0));
        assert_eq!(pad_named("Closed_Hat"), Some(4));
        assert_eq!(pad_named("open-hat"), Some(6));
        assert_eq!(pad_named(" crash 2 "), Some(11));
        assert_eq!(pad_named("cowbell"), None);
    }

    #[test]
    fn every_pad_has_its_own_note_and_its_name_is_its_own() {
        for (pad, name) in PAD_NAMES.iter().enumerate() {
            assert_eq!(pad_named(name), Some(pad));
            assert!(!gm_notes_of(pad).is_empty());
            let note = DRUM_FIRST_NOTE + pad as u8;
            assert_eq!(pad_of_note(note), Some(pad));
        }
        assert_eq!(pad_of_note(129), None);
        assert_eq!(pad_of_note(146), None);
    }

    #[test]
    fn a_new_song_s_hats_choke_each_other_and_nothing_else_does() {
        let drums = Drums::new();
        assert_eq!(drums.kit, None);
        for (pad, settings) in drums.pads.iter().enumerate() {
            let name = PAD_NAMES[pad];
            if name.ends_with("hat") {
                assert_eq!(settings.group, HAT_GROUP);
            } else {
                assert_eq!(settings.group, 0);
            }
        }
    }

    #[test]
    fn a_pad_s_values_are_brought_within_their_ranges() {
        let wild = DrumPad {
            level: 300,
            half_steps: -40,
            pan: 250,
            group: 9,
        };
        let tamed = DrumPad {
            level: 100,
            half_steps: -DRUM_MOST_HALF_STEPS,
            pan: 100,
            group: DRUM_MOST_GROUP,
        };
        assert_eq!(wild.within_ranges(), tamed);
    }
}
