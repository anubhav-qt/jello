#include "voice.h"

#include <algorithm>
#include <cmath>

namespace {

// How loud one voice is. With it, a 4-note chord is about as loud as the
// step 1 test tone. Chosen by ear in slice 5.
constexpr float voice_level = 0.35f;

// How long a voice's volume takes to ease from silent to full, or back. A
// starting value, to be tuned by ear.
constexpr int volume_ease_ms = 10;

// How long a gap takes to shrink away, the same as a new song's loudness
// attack. Short enough that the new wave takes over at once, long enough not
// to click. Part of how a voice takes over without a click, not of how a
// song sounds, so it stays the same for every song.
constexpr int gap_ms = 5;

// How far apart the copies start in their cycles when the engine starts:
// the golden ratio's share of a cycle. Steps of it, wrapped back into one
// cycle, never land close together, so the copies never start lined up.
// Lined up, every note would start as one loud wave that then smears out.
constexpr double copy_start_step = 0.6180339887498949;

// How many cents make a half step, and so 12 times as many an octave.
constexpr double cents_per_octave = 1200.0;

// A note's pitch in Hz, with A4, note 69, at `a4_hz`. Every 12 notes is an
// octave, and each octave up doubles the pitch.
double note_to_frequency(int note, double a4_hz) {
    double octaves_from_a4 = (note - 69) / 12.0;
    return a4_hz * std::pow(2.0, octaves_from_a4);
}

// Where copy `index` of `count` sits among them, from -1 (the lowest and
// furthest left) to 1 (the highest and furthest right), evenly spaced. A
// lone copy sits at 0, right on the note and in the middle.
double place_among(int index, int count) {
    if (count == 1) {
        return 0.0;
    }
    return 2.0 * index / (count - 1) - 1.0;
}

// How hard the key was hit, turned into the envelope's peak, from 0 to 1.
// Squared, so half the velocity gives a quarter of the loudness.
float velocity_to_peak(int velocity) {
    float amount = static_cast<float>(velocity) / 127.0f;
    return amount * amount;
}

}  // namespace

Voice::Voice(int sample_rate)
    : sample_rate_(sample_rate),
      gap_samples_(sample_rate * gap_ms / 1000),
      envelope_(sample_rate),
      filter_(sample_rate),
      volume_step_(1000.0f / static_cast<float>(sample_rate * volume_ease_ms)) {
    // The first copy starts at 0, so a note of one copy starts where it
    // always did.
    for (int i = 0; i < unison_most_copies; i++) {
        double start = i * copy_start_step;
        copies_[i].phase = start - std::floor(start);
    }
}

void Voice::start(int note, int velocity, std::uint64_t press_number, Source source, int layer, float volume,
                  const FilterTargets& filter, const NoteSound& sound) {
    // Where the sound was heading next on each side: the old wave, plus what
    // is left of any earlier gap.
    bool was_sounding = !is_free();
    Stereo old_height = height_now();
    Stereo old_gap = gap_now();
    old_height.left += old_gap.left;
    old_height.right += old_gap.right;

    // The phases are left where they are, so a voice that is still sounding
    // carries on from the same point in its wave instead of jumping.
    note_ = note;
    press_number_ = press_number;
    source_ = source;
    layer_ = layer;
    wave_ = sound.wave;
    set_copies(note_to_frequency(note, sound.a4_hz), sound.unison);

    // The note's own loudness envelope, kept until it ends.
    const LoudnessEnvelopeSettings& loudness = sound.loudness_envelope;
    envelope_.set_stages(loudness.attack_ms, loudness.decay_ms, loudness.sustain, loudness.release_ms);
    envelope_.start(velocity_to_peak(velocity));
    filter_.start(note, !was_sounding, filter, sound.filter_envelope, sound.key_tracking);

    // A new wave can be at a different height here. The gap makes up the
    // difference, so the sound carries on from where it was, then shrinks
    // away. A silent voice was heading nowhere, so it has no gap.
    if (was_sounding) {
        Stereo new_height = height_now();
        gap_.left = old_height.left - new_height.left;
        gap_.right = old_height.right - new_height.right;
        gap_samples_left_ = gap_samples_;
    } else {
        gap_ = Stereo{};
        gap_samples_left_ = 0;
    }

    // A silent voice has nothing to ease from, so it starts right at its
    // volume. One still sounding keeps its own and eases there as it renders.
    if (!was_sounding) {
        volume_ = volume;
    }
}

void Voice::release() {
    envelope_.release();
    filter_.release();
}

bool Voice::is_free() const {
    return envelope_.is_silent();
}

bool Voice::is_releasing() const {
    return envelope_.is_releasing();
}

double Voice::level() const {
    return envelope_.level();
}

int Voice::note() const {
    return note_;
}

std::uint64_t Voice::press_number() const {
    return press_number_;
}

Source Voice::source() const {
    return source_;
}

int Voice::layer() const {
    return layer_;
}

void Voice::render(float* out, int frames, float volume, const FilterTargets& filter) {
    for (int i = 0; i < frames; i++) {
        // A free voice is silent, so there is nothing to add. It stops on
        // the sample it falls silent, so the point in the wave where its
        // next note starts never depends on how the sound was split into
        // chunks.
        if (is_free()) {
            return;
        }

        // A step toward the volume asked for, never past it.
        if (volume_ < volume) {
            volume_ = std::min(volume_ + volume_step_, volume);
        } else if (volume_ > volume) {
            volume_ = std::max(volume_ - volume_step_, volume);
        }

        // The wave goes through the filter before the envelope, so the
        // filter always hears the whole wave, however quiet the note is.
        Stereo exact_height = height_now();
        Stereo gap = gap_now();
        exact_height.left += gap.left;
        exact_height.right += gap.right;
        Stereo filtered = filter_.next(exact_height, filter);
        float loudness = envelope_.next();

        float left = static_cast<float>(filtered.left) * loudness * voice_level;
        left = left * volume_;
        float right = static_cast<float>(filtered.right) * loudness * voice_level;
        right = right * volume_;

        // The gap shrinks a step each sample, until it is gone.
        if (gap_samples_left_ > 0) {
            gap_samples_left_ -= 1;
        }

        // Move every copy along its wave. Wrapping back to 0 keeps the
        // numbers small and precise.
        for (int c = 0; c < copy_count_; c++) {
            Copy& copy = copies_[c];
            copy.phase += copy.phase_step;
            if (copy.phase >= 1.0) {
                copy.phase -= 1.0;
            }
        }

        out[2 * i] += left;
        out[2 * i + 1] += right;
    }
}

void Voice::set_copies(double hz, const UnisonSettings& unison) {
    copy_count_ = unison.copies;
    copy_scale_ = 1.0 / std::sqrt(static_cast<double>(unison.copies));

    for (int i = 0; i < copy_count_; i++) {
        Copy& copy = copies_[i];
        double place = place_among(i, copy_count_);

        // The lowest copy is tuned the detune below the note and the highest
        // as far above.
        double cents = place * unison.detune_cents;
        double copy_hz = hz * std::exp2(cents / cents_per_octave);
        copy.phase_step = copy_hz / sample_rate_;

        // Below 0 is toward the left speaker, above 0 toward the right. Each
        // side gets a square root, so the two squared always add up to 2:
        // the copy is as loud wherever it sits, and in the middle each side
        // gets exactly 1, as a voice did before unison.
        double pan = place * unison.spread;
        copy.left = std::sqrt(1.0 - pan);
        copy.right = std::sqrt(1.0 + pan);
    }
}

Stereo Voice::height_now() const {
    Stereo sum;
    for (int i = 0; i < copy_count_; i++) {
        const Copy& copy = copies_[i];
        double height = wave_height(wave_, copy.phase, copy.phase_step) * wave_trim(wave_);
        sum.left += height * copy.left;
        sum.right += height * copy.right;
    }
    sum.left *= copy_scale_;
    sum.right *= copy_scale_;
    return sum;
}

Stereo Voice::gap_now() const {
    Stereo left_over;
    if (gap_samples_left_ == 0) {
        return left_over;
    }
    double share_left = static_cast<double>(gap_samples_left_) / gap_samples_;
    left_over.left = gap_.left * share_left;
    left_over.right = gap_.right * share_left;
    return left_over;
}
