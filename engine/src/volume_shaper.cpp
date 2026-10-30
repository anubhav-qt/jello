#include "volume_shaper.h"

#include <algorithm>
#include <cmath>

namespace {

// How far a bend of 1 bends a segment: its share t of the way along comes
// out as t to the power 1/8.
constexpr double most_bend_power = 8.0;

// How long the volume heard takes to move all the way, at the fastest, in
// ms.
constexpr float fastest_move_ms = 1.0f;

// The classic pump: at the beat down to silence, back up by half way with
// a bend that moves early and settles, like a compressor letting go, then
// up until the next beat.
constexpr double pump_bend = 0.5;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// `share` of the way along a segment, bent by `bend`.
double bent(double share, double bend) {
    if (bend == 0.0) {
        return share;
    }
    return std::pow(share, std::pow(most_bend_power, -bend));
}

}  // namespace

ValueRange volume_shaper_value_range(int value) {
    if (value == shaper_cycle) {
        return number_range(shaper_shortest_cycle, shaper_longest_cycle);
    }
    if (value == shaper_depth) {
        return number_range(0.0f, 1.0f);
    }
    if (value == shaper_points) {
        return whole_range(1, shaper_most_points);
    }
    if (value < 0) {
        return ValueRange{};
    }
    if (value >= shaper_most_points * shaper_point_values) {
        return ValueRange{};
    }
    int which = value % shaper_point_values;
    if (which == shaper_point_bend) {
        return number_range(-1.0f, 1.0f);
    }
    return number_range(0.0f, 1.0f);
}

bool is_valid_volume_shaper_value(int value, float number) {
    return is_in_range(volume_shaper_value_range(value), number);
}

VolumeShaper::VolumeShaper(int sample_rate)
    : Effect(EffectKind::volume_shaper),
      heard_depth_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_volume_(1.0f, 1000.0f / (fastest_move_ms * static_cast<float>(sample_rate))) {
    points_[0].place = 0.0;
    points_[0].level = 0.0;
    points_[0].bend = pump_bend;
    points_[1].place = 0.5;
    points_[1].level = 1.0;
    points_[2].place = 1.0;
    points_[2].level = 1.0;

    int glide_samples = samples_in_ms(effect_glide_ms, sample_rate);
    for (int i = 0; i < shaper_most_points; i++) {
        heard_places_.emplace_back(glide_samples);
        heard_levels_.emplace_back(glide_samples);
        heard_bends_.emplace_back(glide_samples);
    }
    settle();
}

void VolumeShaper::settle() {
    // The volume heard keeps its 1 ms limit, since where the curve is
    // depends on the beat, known only once the sound starts.
    for (int i = 0; i < shaper_most_points; i++) {
        heard_places_[i].jump_to(points_[i].place);
        heard_levels_[i].jump_to(points_[i].level);
        heard_bends_[i].jump_to(points_[i].bend);
    }
    heard_depth_.jump_to(depth_);
}

void VolumeShaper::set_value(int value, float number) {
    if (!is_valid_volume_shaper_value(value, number)) {
        return;
    }
    if (value == shaper_cycle) {
        cycle_ = number;
    } else if (value == shaper_depth) {
        depth_ = number;
    } else if (value == shaper_points) {
        point_count_ = static_cast<int>(number);
    } else {
        Point& point = points_[value / shaper_point_values];
        int which = value % shaper_point_values;
        if (which == shaper_point_place) {
            point.place = number;
        } else if (which == shaper_point_level) {
            point.level = number;
        } else {
            point.bend = number;
        }
    }
}

float VolumeShaper::value(int value) const {
    if (value == shaper_cycle) {
        return static_cast<float>(cycle_);
    }
    if (value == shaper_depth) {
        return static_cast<float>(depth_);
    }
    if (value == shaper_points) {
        return static_cast<float>(point_count_);
    }
    if (value < 0) {
        return 0.0f;
    }
    if (value >= shaper_most_points * shaper_point_values) {
        return 0.0f;
    }
    const Point& point = points_[value / shaper_point_values];
    int which = value % shaper_point_values;
    if (which == shaper_point_place) {
        return static_cast<float>(point.place);
    }
    if (which == shaper_point_level) {
        return static_cast<float>(point.level);
    }
    return static_cast<float>(point.bend);
}

void VolumeShaper::glide_points() {
    double lowest_place = 0.0;
    for (int i = 0; i < shaper_most_points; i++) {
        double place = heard_places_[i].next(points_[i].place);
        heard_[i].place = std::max(place, lowest_place);
        heard_[i].level = heard_levels_[i].next(points_[i].level);
        heard_[i].bend = heard_bends_[i].next(points_[i].bend);
        lowest_place = heard_[i].place;
    }
}

double VolumeShaper::curve_at(double phase) const {
    int last = point_count_ - 1;

    // The segment the phase is in: from the last point at or before it to
    // the next. Before the first point, or after the last, it is the one
    // from the last point round to the first.
    int from = last;
    for (int i = 0; i < point_count_; i++) {
        if (heard_[i].place <= phase) {
            from = i;
        }
    }
    int to = from + 1;
    double start = heard_[from].place;
    double end = 0.0;
    if (from == last) {
        to = 0;
        end = heard_[0].place + 1.0;
        if (phase < heard_[0].place) {
            start -= 1.0;
            end -= 1.0;
        }
    } else {
        end = heard_[to].place;
    }

    double share = 1.0;
    if (end > start) {
        share = (phase - start) / (end - start);
    }
    double from_level = heard_[from].level;
    double to_level = heard_[to].level;
    return from_level + (to_level - from_level) * bent(share, heard_[from].bend);
}

void VolumeShaper::process(float* stereo, int frames, const EffectContext& context) {
    for (int i = 0; i < frames; i++) {
        glide_points();
        double beat = context.beat + i * context.beats_per_sample;
        double phase = std::fmod(beat, cycle_) / cycle_;
        double depth = heard_depth_.next(depth_);
        double volume = 1.0 - depth * (1.0 - curve_at(phase));
        heard_volume_.set(static_cast<float>(volume));
        float heard = heard_volume_.next();
        stereo[2 * i] *= heard;
        stereo[2 * i + 1] *= heard;
    }
}
