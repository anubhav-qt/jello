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

// Middle C, and how many half steps the note height counts to either side
// of it: five octaves.
constexpr int middle_c = 60;
constexpr double note_height_half_steps = 60.0;

// The most cents the routing list can move a note's pitch either way, all
// its connections added up: 4 octaves.
constexpr double most_pitch_cents = 4800.0;

// A copy never plays above this share of the sample rate, 21,600 Hz at
// 48,000 samples a second, the same as the filter's highest cutoff. Closer
// to half the rate, the highest pitch the samples can hold, the waves' math
// stops behaving.
constexpr double highest_share_of_rate = 0.45;

// How many waves there are, for stepping through them.
constexpr int wave_count = 4;

// What a note's random numbers are for (random_height's `what`). Each LFO's
// own random heights use its number, from 0; these come after.
constexpr std::uint64_t spread_what = lfo_count;  // plus the LFO's number
constexpr std::uint64_t random_what = 2 * lfo_count;

// A hair before the end of a cycle, where an LFO set to "once" holds.
constexpr double once_end_nudge = 1e-9;

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

// A both-ways reading, from -1 to 1, read one way, from 0 to 1.
double one_way(double both_ways) {
    return (both_ways + 1.0) / 2.0;
}

// `wave` moved `steps` steps through sine, triangle, square and saw, round
// and round. Steps are rounded to whole ones.
Wave moved_wave(Wave wave, double steps) {
    int index = static_cast<int>(wave) + static_cast<int>(std::round(steps));
    index = index % wave_count;
    if (index < 0) {
        index += wave_count;
    }
    return static_cast<Wave>(index);
}

// A stage `ms` long moved by `octaves` of time: each one doubles it. Kept
// from `shortest` to `longest`.
int moved_ms(int ms, double octaves, int shortest, int longest) {
    double moved = std::round(ms * std::exp2(octaves));
    moved = std::clamp(moved, static_cast<double>(shortest), static_cast<double>(longest));
    return static_cast<int>(moved);
}

}  // namespace

int player_of(Source source, int layer) {
    if (source == Source::live) {
        return live_player;
    } else if (source == Source::song) {
        return song_player;
    } else {
        return first_layer_player + layer;
    }
}

void Voice::Line::head_to(double to, int samples) {
    // Land exactly, however the steps added up.
    value = target;
    target = to;
    if (samples < 1) {
        value = to;
        step = 0.0;
        return;
    }
    step = (to - value) / samples;
}

void Voice::Line::next() {
    value += step;
}

double Voice::Moves::of(RoutingTarget target) const {
    return by_target[static_cast<int>(target)];
}

void Voice::Moves::add(RoutingTarget target, double move) {
    by_target[static_cast<int>(target)] += move;
}

void Voice::Readings::set(RoutingSource source, Slide slide, double fade) {
    slides[static_cast<int>(source)] = slide;
    fades[static_cast<int>(source)] = fade;
}

Slide Voice::Readings::slide(RoutingSource source) const {
    return slides[static_cast<int>(source)];
}

double Voice::Readings::fade(RoutingSource source) const {
    return fades[static_cast<int>(source)];
}

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
    gain_.head_to(1.0, 0);
}

void Voice::start(int note, int velocity, const NoteStart& start, float volume, const FilterTargets& filter,
                  const NoteSound& sound, const Routing& routing) {
    // Where the sound was heading next on each side: the old wave, plus what
    // is left of any earlier gap.
    bool was_sounding = !is_free();
    Stereo old_height = height_now();
    Stereo old_gap = gap_now();
    old_height.left += old_gap.left;
    old_height.right += old_gap.right;

    note_ = note;
    press_number_ = start.press_number;
    source_ = start.source;
    layer_ = start.layer;
    player_ = player_of(start.source, start.layer);
    start_sample_ = start.sample;
    lfo_clock_ = start.sample;
    age_beats_ = 0.0;

    // The note's own random number and height, and where each of its own
    // LFOs starts: at its start point, moved on at random by up to its
    // spread.
    random_ = random_height(press_number_, random_what, 0);
    note_height_ = std::clamp((note - middle_c) / note_height_half_steps, -1.0, 1.0);
    for (int i = 0; i < lfo_count; i++) {
        const LfoSettings& lfo = routing.lfo(i);
        double chance = one_way(random_height(press_number_, spread_what + i, 0));
        double offset = lfo.spread * chance;
        lfo_starts_[i] = lfo.start + offset;
        lfo_cycles_[i] = lfo_starts_[i];
        lfo_speed_moves_[i] = 0.0;
        press_heights_[i] = routing.press_height(i, start.press, offset, press_number_);
    }

    // The routing list moves the settings read as a note starts once, now.
    Readings readings = read_sources(routing, 0.0);
    Moves moves = add_moves(routing, readings, true);
    NoteSound moved = moved_sound(sound, moves);

    // The phases are left where they are, so a voice that is still sounding
    // carries on from the same point in its wave instead of jumping.
    wave_ = moved.wave;
    hz_ = note_to_frequency(note, moved.a4_hz);
    copy_count_ = moved.unison.copies;
    copy_scale_ = 1.0 / std::sqrt(static_cast<double>(moved.unison.copies));
    detune_cents_ = moved.unison.detune_cents;
    spread_ = moved.unison.spread;
    key_tracking_ = moved.key_tracking;
    loudness_sustain_ = moved.loudness_envelope.sustain;
    filter_sustain_ = moved.filter_envelope.sustain;

    // The note's own loudness envelope, kept until it ends.
    const LoudnessEnvelopeSettings& loudness = moved.loudness_envelope;
    envelope_.set_stages(loudness.attack_ms, loudness.decay_ms, loudness.sustain, loudness.release_ms);
    envelope_.start(velocity_to_peak(velocity));
    filter_.start(note, !was_sounding, filter, moved.filter_envelope, moved.key_tracking);

    // The live settings start right where the routing list puts them, which
    // tunes and places the copies.
    head_for(moves, 0);

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

void Voice::tick(const Routing& routing, std::uint64_t block_end) {
    // Its own LFOs move on to the end of the block, each at its speed as
    // last moved.
    double elapsed = static_cast<double>(block_end - lfo_clock_);
    for (int i = 0; i < lfo_count; i++) {
        lfo_cycles_[i] += routing.cycles_per_sample(i, lfo_speed_moves_[i]) * elapsed;
    }
    age_beats_ += elapsed * routing.beats_per_sample();
    lfo_clock_ = block_end;

    double age_ms = static_cast<double>(block_end - start_sample_) * 1000.0 / sample_rate_;
    Readings readings = read_sources(routing, age_ms);
    Moves moves = add_moves(routing, readings, false);
    head_for(moves, control_samples);
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
        FilterMoves moves;
        moves.cutoff = cutoff_move_.value;
        moves.resonance = resonance_move_.value;
        moves.amount = amount_move_.value;
        Stereo filtered = filter_.next(exact_height, filter, moves);
        float loudness = envelope_.next();
        float gain = static_cast<float>(gain_.value);

        float left = static_cast<float>(filtered.left) * loudness * voice_level;
        left = left * volume_;
        left = left * gain;
        float right = static_cast<float>(filtered.right) * loudness * voice_level;
        right = right * volume_;
        right = right * gain;

        // The gap shrinks a step each sample, until it is gone.
        if (gap_samples_left_ > 0) {
            gap_samples_left_ -= 1;
        }

        // Move every copy along its wave. Wrapping back to 0 keeps the
        // numbers small and precise. Then every moved setting takes its
        // step along its straight line.
        for (int c = 0; c < copy_count_; c++) {
            Copy& copy = copies_[c];
            copy.phase += copy.phase_step.value;
            if (copy.phase >= 1.0) {
                copy.phase -= 1.0;
            }
            copy.phase_step.next();
            copy.left.next();
            copy.right.next();
        }
        gain_.next();
        cutoff_move_.next();
        resonance_move_.next();
        amount_move_.next();

        out[2 * i] += left;
        out[2 * i + 1] += right;
    }
}

Voice::Readings Voice::read_sources(const Routing& routing, double age_ms) const {
    Readings readings;
    for (int i = 0; i < lfo_count; i++) {
        RoutingSource lfo = static_cast<RoutingSource>(static_cast<int>(RoutingSource::lfo_1) + i);
        readings.set(lfo, read_lfo(routing, i), routing.fade(i, age_ms, age_beats_));
    }

    // Every other source reads 0 to 1 by nature, stretched to -1 to 1, or
    // -1 to 1 already. None of them fades in.
    double key = routing.vibrato_key(player_);
    readings.set(RoutingSource::vibrato_key, Slide::at(2.0 * key - 1.0), 1.0);
    readings.set(RoutingSource::note_height, Slide::at(note_height_), 1.0);
    readings.set(RoutingSource::random, Slide::at(random_), 1.0);
    double loudness = envelope_.share();
    readings.set(RoutingSource::loudness_envelope, Slide::at(2.0 * loudness - 1.0), 1.0);
    double filter = filter_.envelope_level();
    readings.set(RoutingSource::filter_envelope, Slide::at(2.0 * filter - 1.0), 1.0);
    return readings;
}

Slide Voice::read_lfo(const Routing& routing, int index) const {
    const LfoSettings& lfo = routing.lfo(index);

    if (lfo.speed == LfoSpeed::presses) {
        if (lfo.per_note) {
            return Slide::at(press_heights_[index]);
        }
        return routing.pressed_lfo(player_, index);
    }
    if (!lfo.per_note) {
        return routing.shared_lfo(index);
    }

    double cycles = lfo_cycles_[index];
    if (lfo.once) {
        double end = lfo_starts_[index] + 1.0 - once_end_nudge;
        cycles = std::min(cycles, end);
    }
    return lfo_height(lfo, cycles, press_number_, index);
}

Voice::Moves Voice::add_moves(const Routing& routing, const Readings& readings, bool at_start) const {
    Moves moves;
    for (int place = 0; place < routing.used_count(); place++) {
        const Connection& connection = routing.used(place);

        // A shared LFO's speed is shared by every note, so the routing list
        // moves it itself.
        int speed_of = lfo_speed_of(connection.target);
        if (speed_of >= 0) {
            if (!routing.lfo(speed_of).per_note) {
                continue;
            }
        }

        // The targets read as a note starts are only moved then, and with
        // no fade-in.
        bool read_at_start = is_read_at_start(connection.target);
        if (read_at_start) {
            if (!at_start) {
                continue;
            }
        }

        double fade = readings.fade(connection.source);
        if (read_at_start) {
            fade = 1.0;
        }

        double scale = 1.0;
        if (connection.scaled_by != RoutingSource::none) {
            double by_fade = readings.fade(connection.scaled_by);
            if (read_at_start) {
                by_fade = 1.0;
            }
            scale = one_way(readings.slide(connection.scaled_by).value()) * by_fade;
        }

        Slide source = readings.slide(connection.source);
        moves.add(connection.target, connection_move(connection, source, fade, scale));
    }
    return moves;
}

NoteSound Voice::moved_sound(const NoteSound& sound, const Moves& moves) const {
    NoteSound moved = sound;
    moved.wave = moved_wave(sound.wave, moves.of(RoutingTarget::wave));

    int copies = sound.unison.copies + static_cast<int>(std::round(moves.of(RoutingTarget::copies)));
    moved.unison.copies = std::clamp(copies, 1, unison_most_copies);

    LoudnessEnvelopeSettings& loudness = moved.loudness_envelope;
    loudness.attack_ms = moved_ms(loudness.attack_ms, moves.of(RoutingTarget::loudness_attack),
                                  loudness_shortest_ms, loudness_longest_ms);
    loudness.decay_ms = moved_ms(loudness.decay_ms, moves.of(RoutingTarget::loudness_decay),
                                 loudness_shortest_ms, loudness_longest_ms);
    loudness.release_ms = moved_ms(loudness.release_ms, moves.of(RoutingTarget::loudness_release),
                                   loudness_shortest_ms, loudness_longest_ms);

    FilterEnvelopeSettings& filter = moved.filter_envelope;
    filter.attack_ms = moved_ms(filter.attack_ms, moves.of(RoutingTarget::filter_attack), 0, filter_longest_ms);
    filter.decay_ms = moved_ms(filter.decay_ms, moves.of(RoutingTarget::filter_decay), 0, filter_longest_ms);
    filter.release_ms = moved_ms(filter.release_ms, moves.of(RoutingTarget::filter_release), 0, filter_longest_ms);
    return moved;
}

void Voice::head_for(const Moves& moves, int samples) {
    double cents = std::clamp(moves.of(RoutingTarget::pitch), -most_pitch_cents, most_pitch_cents);
    double most_detune = unison_most_detune_cents;
    double detune = std::clamp(detune_cents_ + moves.of(RoutingTarget::detune), 0.0, most_detune);
    double spread = std::clamp(spread_ + moves.of(RoutingTarget::spread), 0.0, 1.0);
    double pan = moves.of(RoutingTarget::pan);
    double highest_hz = sample_rate_ * highest_share_of_rate;

    for (int i = 0; i < copy_count_; i++) {
        Copy& copy = copies_[i];
        double place = place_among(i, copy_count_);

        // The lowest copy is tuned the detune below the note and the highest
        // as far above, and every copy is moved by the pitch's move.
        double copy_cents = place * detune + cents;
        double copy_hz = hz_ * std::exp2(copy_cents / cents_per_octave);
        copy_hz = std::min(copy_hz, highest_hz);
        copy.phase_step.head_to(copy_hz / sample_rate_, samples);

        // Below 0 is toward the left speaker, above 0 toward the right. Each
        // side gets a square root, so the two squared always add up to 2:
        // the copy is as loud wherever it sits, and in the middle each side
        // gets exactly 1, as a voice did before unison.
        double copy_pan = std::clamp(place * spread + pan, -1.0, 1.0);
        copy.left.head_to(std::sqrt(1.0 - copy_pan), samples);
        copy.right.head_to(std::sqrt(1.0 + copy_pan), samples);
    }

    // Loudness only ever goes down, to silence at most.
    double gain = std::clamp(1.0 + moves.of(RoutingTarget::loudness), 0.0, 1.0);
    gain_.head_to(gain, samples);

    // Moving the key tracking moves the cutoff by how much further, or less
    // far, it follows middle C's.
    double tracking = std::clamp(key_tracking_ + moves.of(RoutingTarget::key_tracking), 0.0, 1.0);
    double octaves_above_middle_c = (note_ - middle_c) / 12.0;
    double tracking_move = (tracking - key_tracking_) * octaves_above_middle_c;
    cutoff_move_.head_to(moves.of(RoutingTarget::cutoff) + tracking_move, samples);
    resonance_move_.head_to(moves.of(RoutingTarget::resonance), samples);
    amount_move_.head_to(moves.of(RoutingTarget::filter_amount), samples);

    double loudness_sustain = std::clamp(loudness_sustain_ + moves.of(RoutingTarget::loudness_sustain), 0.0, 1.0);
    envelope_.set_sustain(loudness_sustain, samples);
    double filter_sustain = std::clamp(filter_sustain_ + moves.of(RoutingTarget::filter_sustain), 0.0, 1.0);
    filter_.set_sustain(filter_sustain, samples);

    for (int i = 0; i < lfo_count; i++) {
        RoutingTarget speed = static_cast<RoutingTarget>(static_cast<int>(RoutingTarget::lfo_1_speed) + i);
        lfo_speed_moves_[i] = moves.of(speed);
    }
}

Stereo Voice::height_now() const {
    Stereo sum;
    for (int i = 0; i < copy_count_; i++) {
        const Copy& copy = copies_[i];
        double height = wave_height(wave_, copy.phase, copy.phase_step.value) * wave_trim(wave_);
        sum.left += height * copy.left.value;
        sum.right += height * copy.right.value;
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
