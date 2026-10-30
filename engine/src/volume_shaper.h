#pragma once

#include <array>
#include <vector>

#include "ease.h"
#include "effect.h"
#include "filter.h"

// How many points the volume shaper's curve can have.
constexpr int shaper_most_points = 16;

// The volume shaper's values, numbered: each point's, at point *
// shaper_point_values + one of the shaper_point_ ones, then the rest.
constexpr int shaper_point_place = 0;  // where in the cycle, 0 to 1
constexpr int shaper_point_level = 1;  // 0 to 1
constexpr int shaper_point_bend = 2;   // -1 to 1, how it bends into the next point
constexpr int shaper_point_values = 3;
constexpr int shaper_cycle = 48;   // beats, shaper_shortest_cycle to shaper_longest_cycle
constexpr int shaper_depth = 49;   // 0 to 1
constexpr int shaper_points = 50;  // how many points the curve has, 1 to shaper_most_points
constexpr int shaper_value_count = 51;

constexpr float shaper_shortest_cycle = 1.0f / 16.0f;
constexpr float shaper_longest_cycle = 16.0f;

// True if the volume shaper has value `value` and `number` is within its
// range.
// The range of value `value`.
ValueRange volume_shaper_value_range(int value);

bool is_valid_volume_shaper_value(int value, float number);

// The volume shaper (docs/design.md#mix-in-the-engine): a volume curve that
// repeats every cycle, on the beat, the easy way to the "pumping" sound.
// The curve goes through its points in order, each bending into the next:
// a bend above 0 moves early and settles, below 0 waits and moves late.
// After the last point it goes on to the first, a cycle later; a place
// lower than the one before it counts as that one. The depth says how much
// of the curve is heard: at 0 none, at 1 all of it. The volume heard never
// moves faster than all the way in 1 ms, so a step in the curve never
// clicks. A new one is the classic pump, down at each beat and back up by
// half way, at 0 depth.
class VolumeShaper : public Effect {
public:
    explicit VolumeShaper(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    struct Point {
        double place = 1.0;
        double level = 1.0;
        double bend = 0.0;
    };

    // Moves every point's glides one sample on, into heard_.
    void glide_points();

    // The curve's level at `phase`, from 0 to 1 through the cycle, from the
    // points heard now.
    double curve_at(double phase) const;

    std::array<Point, shaper_most_points> points_;
    int point_count_ = 3;
    double cycle_ = 1.0;
    double depth_ = 0.0;

    // Each point's glides, shaper_most_points of each.
    std::vector<Glide> heard_places_;
    std::vector<Glide> heard_levels_;
    std::vector<Glide> heard_bends_;
    Glide heard_depth_;

    // The points as heard on this sample, each place at least the one
    // before it.
    std::array<Point, shaper_most_points> heard_;

    Ease heard_volume_;
};
