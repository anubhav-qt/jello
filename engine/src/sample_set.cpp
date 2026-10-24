#include "sample_set.h"

#include <cmath>

namespace {

// The longest recording a zone can hold: 10 minutes at 48,000 a second.
constexpr std::uint64_t most_frames = 48000ull * 600ull;

// One frame of `zone`'s channel `channel`, by its number counted on from
// the start. A looping zone carries on round its loop past its end, so the
// frames read across the loop's seam are the ones heard there. Before the
// start, and past the end of a zone that plays once, it is silent.
float frame_at(const SampleZone& zone, std::int64_t index, int channel) {
    if (index < 0) {
        return 0.0f;
    }
    std::uint64_t place = static_cast<std::uint64_t>(index);
    if (zone.loops()) {
        if (place >= zone.loop_end) {
            std::uint64_t loop_length = zone.loop_end - zone.loop_start;
            place = zone.loop_start + (place - zone.loop_end) % loop_length;
        }
    }
    if (place >= zone.frame_count) {
        return 0.0f;
    }
    return zone.frames[place * zone.channels + channel];
}

// The height between frames `a` and `b` a share `t` of the way along, from
// the frames on either side too: 4-point, 3rd-order Hermite.
double hermite(double before, double a, double b, double after, double t) {
    double c1 = 0.5 * (b - before);
    double c2 = before - 2.5 * a + 2.0 * b - 0.5 * after;
    double c3 = 0.5 * (after - before) + 1.5 * (a - b);
    return ((c3 * t + c2) * t + c1) * t + a;
}

// How far `note` is outside `zone`'s notes, in notes; 0 inside them.
int distance_to(const SampleZone& zone, int note) {
    if (note < zone.lowest_note) {
        return zone.lowest_note - note;
    }
    if (note > zone.highest_note) {
        return note - zone.highest_note;
    }
    return 0;
}

}  // namespace

bool SampleZone::loops() const {
    return loop_end > loop_start;
}

SampleSet::SampleSet(int id) : id_(id) {}

int SampleSet::id() const {
    return id_;
}

bool SampleSet::add_zone(const SampleZone& settings, const float* frames) {
    if (settings.channels < 1) {
        return false;
    }
    if (settings.channels > 2) {
        return false;
    }
    if (settings.frame_count > most_frames) {
        return false;
    }
    if (settings.lowest_note > settings.highest_note) {
        return false;
    }
    if (settings.loop_end > settings.frame_count) {
        return false;
    }
    if (std::isnan(settings.tune_cents)) {
        return false;
    }
    if (settings.frame_count > 0) {
        if (frames == nullptr) {
            return false;
        }
    }

    SampleZone zone = settings;
    std::uint64_t count = settings.frame_count * static_cast<std::uint64_t>(settings.channels);
    zone.frames.assign(frames, frames + count);
    zones_.push_back(std::move(zone));
    return true;
}

const SampleZone* SampleSet::zone_for(int note) const {
    const SampleZone* nearest = nullptr;
    int nearest_distance = 0;
    for (const SampleZone& zone : zones_) {
        int distance = distance_to(zone, note);
        if (distance == 0) {
            return &zone;
        }
        if (nearest == nullptr || distance < nearest_distance) {
            nearest = &zone;
            nearest_distance = distance;
        }
    }
    return nearest;
}

const SampleZone* SampleSet::exact_zone_for(int note) const {
    for (const SampleZone& zone : zones_) {
        if (distance_to(zone, note) == 0) {
            return &zone;
        }
    }
    return nullptr;
}

Stereo read_zone(const SampleZone& zone, double place) {
    Stereo height;
    if (is_past_end(zone, place)) {
        return height;
    }
    double whole = std::floor(place);
    double share = place - whole;
    std::int64_t index = static_cast<std::int64_t>(whole);

    double left = hermite(frame_at(zone, index - 1, 0), frame_at(zone, index, 0), frame_at(zone, index + 1, 0),
                          frame_at(zone, index + 2, 0), share);
    height.left = left;
    height.right = left;
    if (zone.channels == 2) {
        height.right = hermite(frame_at(zone, index - 1, 1), frame_at(zone, index, 1), frame_at(zone, index + 1, 1),
                               frame_at(zone, index + 2, 1), share);
    }
    return height;
}

double move_through(const SampleZone& zone, double place, double step) {
    double moved = place + step;
    if (zone.loops()) {
        double loop_end = static_cast<double>(zone.loop_end);
        double loop_length = static_cast<double>(zone.loop_end - zone.loop_start);
        while (moved >= loop_end) {
            moved -= loop_length;
        }
        return moved;
    }
    double end = static_cast<double>(zone.frame_count);
    if (moved > end) {
        return end;
    }
    return moved;
}

bool is_past_end(const SampleZone& zone, double place) {
    if (zone.loops()) {
        return false;
    }
    return place >= static_cast<double>(zone.frame_count);
}
