#include "routing.h"

#include <algorithm>
#include <cmath>

#include "filter.h"
#include "oscillator.h"
#include "unison.h"

namespace {

// How far a pitch move snaps: to whole half steps, 100 cents each.
constexpr double cents_per_half_step = 100.0;

// The most a connection can move the pitch either way: 4 octaves.
constexpr float most_pitch_cents = 4800.0f;

// The most octaves a connection can move the cutoff either way. The cutoff
// runs from 20 to 21,600 Hz, about 10 octaves.
constexpr float most_cutoff_octaves = 10.0f;

// The most octaves a connection can move an LFO's speed or an envelope's
// time either way. Both run over about 13 octaves: 0.01 to 100 Hz, and 1 to
// 10,000 ms.
constexpr float most_octaves_of_speed = 14.0f;

// The most steps a connection can move an oscillator's kind or the filter's
// type either way: through all of them.
constexpr float most_kind_steps = static_cast<float>(stepped_kind_count - 1);
constexpr float most_filter_type_steps = static_cast<float>(filter_type_count - 1);

// The most a connection can move a square's width either way: from the
// narrowest, 5%, to the widest, 95%.
constexpr float most_width_share = 0.9f;

// How long a shared LFO's jump glides, the same as a moved filter control.
constexpr int glide_ms = 20;

// A both-ways reading, from -1 to 1, read one way, from 0 to 1.
double one_way(double both_ways) {
    return (both_ways + 1.0) / 2.0;
}

// True if `target` is one of the oscillators' targets that starts at
// `first`, oscillator 1's.
bool is_oscillator_target(RoutingTarget target, RoutingTarget first) {
    int index = static_cast<int>(target) - static_cast<int>(first);
    if (index < 0) {
        return false;
    }
    return index < oscillator_count;
}

}  // namespace

float most_amount(RoutingTarget target) {
    if (lfo_speed_of(target) >= 0) {
        return most_octaves_of_speed;
    }

    if (is_pitch(target)) {
        return most_pitch_cents;
    } else if (target == RoutingTarget::cutoff) {
        return most_cutoff_octaves;
    } else if (target == RoutingTarget::detune) {
        return unison_most_detune_cents;
    } else if (target == RoutingTarget::filter_amount) {
        return filter_most_octaves;
    } else if (target == RoutingTarget::filter_type) {
        return most_filter_type_steps;
    } else if (is_oscillator_target(target, RoutingTarget::oscillator_1_width)) {
        return most_width_share;
    } else if (is_oscillator_target(target, RoutingTarget::oscillator_1_kind)) {
        return most_kind_steps;
    } else if (target == RoutingTarget::copies) {
        return static_cast<float>(unison_most_copies - 1);
    } else if (is_read_at_start(target)) {
        // The envelopes' times and the glide time.
        return most_octaves_of_speed;
    } else {
        // Loudness, pan, resonance, key tracking, spread, the sustains and
        // the oscillators' levels, each a share.
        return 1.0f;
    }
}

int lfo_of(RoutingSource source) {
    int index = static_cast<int>(source) - static_cast<int>(RoutingSource::lfo_1);
    if (index < 0) {
        return -1;
    }
    if (index >= lfo_count) {
        return -1;
    }
    return index;
}

int lfo_speed_of(RoutingTarget target) {
    int index = static_cast<int>(target) - static_cast<int>(RoutingTarget::lfo_1_speed);
    if (index < 0) {
        return -1;
    }
    if (index >= lfo_count) {
        return -1;
    }
    return index;
}

RoutingTarget oscillator_level(int index) {
    return static_cast<RoutingTarget>(static_cast<int>(RoutingTarget::oscillator_1_level) + index);
}

RoutingTarget oscillator_pitch(int index) {
    return static_cast<RoutingTarget>(static_cast<int>(RoutingTarget::oscillator_1_pitch) + index);
}

RoutingTarget oscillator_width(int index) {
    return static_cast<RoutingTarget>(static_cast<int>(RoutingTarget::oscillator_1_width) + index);
}

RoutingTarget oscillator_kind(int index) {
    return static_cast<RoutingTarget>(static_cast<int>(RoutingTarget::oscillator_1_kind) + index);
}

bool is_pitch(RoutingTarget target) {
    if (target == RoutingTarget::pitch) {
        return true;
    }
    return is_oscillator_target(target, RoutingTarget::oscillator_1_pitch);
}

bool is_read_at_start(RoutingTarget target) {
    return static_cast<int>(target) >= static_cast<int>(RoutingTarget::oscillator_1_kind);
}

double connection_move(const Connection& connection, const Slide& source, double fade, double scale) {
    Slide reading = source;
    if (connection.one_way) {
        reading.from = one_way(reading.from);
        reading.to = one_way(reading.to);
    }

    double size = connection.amount * fade * scale;
    double from = reading.from * size;
    double to = reading.to * size;

    // Both ends snap, and the move still slides between them, so a
    // smoothed step slides from one half step to the next.
    bool snaps = false;
    if (connection.snap) {
        snaps = is_pitch(connection.target);
    }
    if (snaps) {
        from = std::round(from / cents_per_half_step) * cents_per_half_step;
        to = std::round(to / cents_per_half_step) * cents_per_half_step;
    }
    return from + (to - from) * reading.done;
}

Routing::Routing(int sample_rate)
    : sample_rate_(sample_rate),
      glide_samples_(sample_rate * glide_ms / 1000),
      beats_per_sample_(0.0) {
    for (int i = 0; i < lfo_count; i++) {
        settle_press_slides(i);
    }
    read_shared_lfos();
}

void Routing::set_lfo(int index, const LfoSettings& settings) {
    bool was_shared = is_shared_running(index);
    double before = readings_[index].value();
    lfos_[index] = settings;

    // A shared LFO glides from where it was to where the change puts it, so
    // it never clicks. One that only now runs for every note moved nothing
    // before, so it starts right where the change puts it.
    gap_samples_left_[index] = 0.0;
    if (was_shared) {
        if (is_shared_running(index)) {
            Slide after = lfo_height(settings, cycles_[index] + settings.start, 0, index);
            gaps_[index] = before - after.value();
            gap_samples_left_[index] = glide_samples_;
        }
    }
    read_shared_lfos();
    settle_press_slides(index);
}

void Routing::set_connection(int index, const Connection& connection) {
    connections_[index] = connection;

    used_count_ = 0;
    moves_cutoff_ = false;
    for (int i = 0; i < connection_count; i++) {
        const Connection& each = connections_[i];
        if (each.source == RoutingSource::none) {
            continue;
        }
        used_[used_count_] = i;
        used_count_ += 1;
        if (each.target == RoutingTarget::cutoff) {
            moves_cutoff_ = true;
        }
    }
}

void Routing::set_vibrato_key_times(int rise_ms, int fall_ms) {
    key_rise_samples_ = static_cast<double>(rise_ms) * sample_rate_ / 1000.0;
    key_fall_samples_ = static_cast<double>(fall_ms) * sample_rate_ / 1000.0;
}

const LfoSettings& Routing::lfo(int index) const {
    return lfos_[index];
}

int Routing::used_count() const {
    return used_count_;
}

const Connection& Routing::used(int place) const {
    return connections_[used_[place]];
}

bool Routing::moves_cutoff() const {
    return moves_cutoff_;
}

void Routing::set_beats_per_sample(double beats_per_sample) {
    beats_per_sample_ = beats_per_sample;
}

double Routing::beats_per_sample() const {
    return beats_per_sample_;
}

void Routing::on_beat(std::uint64_t number, double beats_per_sample, std::uint64_t now) {
    beats_per_sample_ = beats_per_sample;

    // Everything shared has already been moved on to the end of this block
    // of 32 samples, so the beat count is too.
    double ahead = 0.0;
    if (clock_ > now) {
        ahead = static_cast<double>(clock_ - now) * beats_per_sample;
    }
    beat_position_ = static_cast<double>(number) + ahead;

    for (int i = 0; i < lfo_count; i++) {
        if (lfos_[i].speed != LfoSpeed::beats) {
            continue;
        }
        // One whose speed is being moved runs free until the beat starts
        // again.
        if (number != 0) {
            if (speed_moves_[i] != 0.0) {
                continue;
            }
        }

        double before = readings_[i].value();
        cycles_[i] = cycles_by_beat(i);
        if (is_shared_running(i)) {
            Slide after = lfo_height(lfos_[i], cycles_[i] + lfos_[i].start, 0, i);
            gaps_[i] = before - after.value();
            gap_samples_left_[i] = glide_samples_;
        }
    }
    read_shared_lfos();
}

void Routing::set_vibrato_key(int player, bool held) {
    key_held_[player] = held;
}

double Routing::vibrato_key(int player) const {
    return key_values_[player];
}

std::uint64_t Routing::press(int player, std::uint64_t now) {
    // Notes starting on the same sample are one press.
    if (pressed_yet_[player]) {
        if (last_press_sample_[player] == now) {
            return presses_[player] - 1;
        }
    }

    presses_[player] += 1;
    pressed_yet_[player] = true;
    last_press_sample_[player] = now;
    std::uint64_t press = presses_[player] - 1;

    // Each shared LFO in presses slides on to its height for this press,
    // from wherever it is, over its smoothing's share of a beat.
    for (int i = 0; i < lfo_count; i++) {
        const LfoSettings& lfo = lfos_[i];
        if (lfo.per_note) {
            continue;
        }
        if (lfo.speed != LfoSpeed::presses) {
            continue;
        }

        PressSlide& slide = press_slides_[player][i];
        slide.from = pressed_lfo(player, i).value();
        slide.to = press_height(i, press, 0.0, 0);
        slide.samples_done = 0.0;
        slide.samples_long = 0.0;
        if (beats_per_sample_ > 0.0) {
            slide.samples_long = lfo.smooth / beats_per_sample_;
        }
    }
    return press;
}

void Routing::restart_presses(int player) {
    presses_[player] = 0;
    pressed_yet_[player] = false;
}

Slide Routing::shared_lfo(int index) const {
    return readings_[index];
}

Slide Routing::pressed_lfo(int player, int index) const {
    const PressSlide& slide = press_slides_[player][index];
    Slide reading;
    reading.from = slide.from;
    reading.to = slide.to;
    reading.done = 1.0;
    if (slide.samples_done < slide.samples_long) {
        reading.done = slide.samples_done / slide.samples_long;
    }
    return reading;
}

double Routing::press_height(int index, std::uint64_t press, double offset, std::uint64_t who) const {
    // In presses, an LFO slides over time instead of over its cycle, so its
    // own smoothing is left out here.
    LfoSettings unsmoothed = lfos_[index];
    unsmoothed.smooth = 0.0;

    double into = static_cast<double>(press) / unsmoothed.presses;
    double cycles = into + unsmoothed.start + offset;
    return lfo_height(unsmoothed, cycles, who, index).value();
}

double Routing::cycles_per_sample(int index, double speed_move) const {
    const LfoSettings& lfo = lfos_[index];
    double faster = std::exp2(speed_move);

    if (lfo.speed == LfoSpeed::hz) {
        double hz = std::clamp(lfo.hz * faster, lfo_slowest_hz, lfo_fastest_hz);
        return hz / sample_rate_;
    } else if (lfo.speed == LfoSpeed::beats) {
        double beats = std::clamp(lfo.beats / faster, lfo_shortest_beats, lfo_longest_beats);
        return beats_per_sample_ / beats;
    } else {
        return 0.0;
    }
}

double Routing::fade(int index, double age_ms, double age_beats) const {
    const LfoSettings& lfo = lfos_[index];

    double age = age_ms;
    double delay = lfo.delay_ms;
    double fade_in = lfo.fade_ms;
    if (lfo.speed == LfoSpeed::beats) {
        age = age_beats;
        delay = lfo.delay_beats;
        fade_in = lfo.fade_beats;
    }

    if (age < delay) {
        return 0.0;
    }
    if (fade_in <= 0.0) {
        return 1.0;
    }
    double grown = (age - delay) / fade_in;
    return std::min(grown, 1.0);
}

void Routing::tick(std::uint64_t block_end) {
    double elapsed = static_cast<double>(block_end - clock_);
    clock_ = block_end;
    beat_position_ += elapsed * beats_per_sample_;

    // Every LFO in Hz or beats keeps a shared place, even while it is per
    // note, so switching it to shared finds it where it would have been.
    for (int i = 0; i < lfo_count; i++) {
        cycles_[i] += cycles_per_sample(i, speed_moves_[i]) * elapsed;
        gap_samples_left_[i] = std::max(gap_samples_left_[i] - elapsed, 0.0);
    }

    for (int player = 0; player < player_count; player++) {
        // The vibrato key rises or falls in a straight line, and a time of
        // 0 jumps.
        double& key = key_values_[player];
        if (key_held_[player]) {
            if (key_rise_samples_ <= 0.0) {
                key = 1.0;
            } else {
                key = std::min(key + elapsed / key_rise_samples_, 1.0);
            }
        } else {
            if (key_fall_samples_ <= 0.0) {
                key = 0.0;
            } else {
                key = std::max(key - elapsed / key_fall_samples_, 0.0);
            }
        }

        for (int i = 0; i < lfo_count; i++) {
            press_slides_[player][i].samples_done += elapsed;
        }
    }

    read_shared_lfos();
    move_shared_speeds();
}

void Routing::read_shared_lfos() {
    for (int i = 0; i < lfo_count; i++) {
        if (!is_shared_running(i)) {
            readings_[i] = Slide::at(0.0);
            continue;
        }

        Slide reading = lfo_height(lfos_[i], cycles_[i] + lfos_[i].start, 0, i);

        // What is left of a jump, shrinking in a straight line.
        double gap = 0.0;
        if (gap_samples_left_[i] > 0.0) {
            gap = gaps_[i] * gap_samples_left_[i] / glide_samples_;
        }
        reading.from += gap;
        reading.to += gap;
        readings_[i] = reading;
    }
}

void Routing::move_shared_speeds() {
    speed_moves_.fill(0.0);

    // Only shared LFOs move a shared LFO's speed, since every note shares
    // it. They move it with no fade-in: no note's age counts.
    for (int place = 0; place < used_count_; place++) {
        const Connection& connection = used(place);

        int target = lfo_speed_of(connection.target);
        if (target < 0) {
            continue;
        }
        if (!is_shared_running(target)) {
            continue;
        }

        int source = lfo_of(connection.source);
        if (source < 0) {
            continue;
        }
        if (!is_shared_running(source)) {
            continue;
        }

        double scale = 1.0;
        if (connection.scaled_by != RoutingSource::none) {
            int by = lfo_of(connection.scaled_by);
            if (by < 0) {
                continue;
            }
            if (!is_shared_running(by)) {
                continue;
            }
            scale = one_way(readings_[by].value());
        }
        speed_moves_[target] += connection_move(connection, readings_[source], 1.0, scale);
    }
}

bool Routing::is_shared_running(int index) const {
    const LfoSettings& lfo = lfos_[index];
    if (lfo.per_note) {
        return false;
    }
    return lfo.speed != LfoSpeed::presses;
}

double Routing::cycles_by_beat(int index) const {
    return beat_position_ / lfos_[index].beats;
}

void Routing::settle_press_slides(int index) {
    for (int player = 0; player < player_count; player++) {
        std::uint64_t press = 0;
        if (presses_[player] > 0) {
            press = presses_[player] - 1;
        }
        double height = press_height(index, press, 0.0, 0);

        PressSlide& slide = press_slides_[player][index];
        slide.from = height;
        slide.to = height;
        slide.samples_done = 0.0;
        slide.samples_long = 0.0;
    }
}
