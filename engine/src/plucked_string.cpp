#include "plucked_string.h"

#include "noise.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// The lowest pitch a string holds, in Hz: A0, below any guitar. Lower notes
// stay on it.
constexpr double lowest_hz = 27.5;

// The shortest delay it reads, in samples: reading needs two samples ahead.
constexpr double shortest_delay = 2.5;

// How many samples past one period a jump to another note writes, for reading
// between samples and past the smoothing.
constexpr int reading_margin = 4;

// How much a pluck is its shape, with the rest noise. The shape gives it its
// body and the noise its scratch.
constexpr double shape_share = 0.6;

// A pickup's reading takes away as much as it adds, so it is turned down.
constexpr double pickup_trim = 0.7;

// A pickup's coil rings at a pitch and hears little above it, between these
// Hz by its tone, and this sharply.
constexpr double coil_lowest_hz = 1500.0;
constexpr double coil_highest_hz = 8000.0;
constexpr double coil_q = 1.6;

// How much slow drift is let through.
constexpr double blocker_pole = 0.995;

// An acoustic guitar body's main resonances: the air in it, its top, and the
// top's next mode, each with how sharp and how loud it is.
struct BodyResonance {
    double hz;
    double q;
    double level;
};
constexpr BodyResonance body_resonances[3] = {
    {105.0, 9.0, 0.5},
    {210.0, 7.0, 0.6},
    {420.0, 3.0, 0.4},
};

// How much of the plain string stays under the body, however much body there
// is, so the top of the sound never goes.
constexpr double string_through_body = 0.65;

}  // namespace

int PluckedString::memory_size(int sample_rate) {
    int needed = static_cast<int>(sample_rate / lowest_hz) + 8;
    int size = 1;
    while (size < needed) {
        size *= 2;
    }
    return size * 2;
}

void PluckedString::attach(float* memory, int size, int sample_rate) {
    memory_ = memory;
    size_ = size / 2;
    stretched_ = memory + size_;
    mask_ = size_ - 1;
    sample_rate_ = sample_rate;
}

void PluckedString::Resonance::tune(double hz, double q, double level, int sample_rate) {
    double angle = 2.0 * pi * hz / sample_rate;
    double radius = std::exp(-angle / (2.0 * q));
    a1 = 2.0 * radius * std::cos(angle);
    a2 = -radius * radius;
    // At its own pitch it comes out `level` times as loud as it went in.
    gain = level * (1.0 - radius) * 2.0 * std::sin(angle);
    y1 = 0.0;
    y2 = 0.0;
}

double PluckedString::Resonance::next(double in) {
    double out = gain * in + a1 * y1 + a2 * y2;
    y2 = y1;
    y1 = out;
    return out;
}

void PluckedString::pluck(double cycle_step, double hardness, const StringSettings& settings,
                          std::uint64_t press_number, std::uint64_t place) {
    if (memory_ == nullptr) {
        return;
    }
    std::fill(memory_, memory_ + size_, 0.0f);

    double longest_delay = size_ - 4.0;
    double period = std::clamp(1.0 / cycle_step, shortest_delay, longest_delay);
    decay_seconds_ = std::max(static_cast<double>(settings.decay_seconds), 0.001);
    set_loop_gain(period);
    smoothing_ = 0.5 * (1.0 - std::clamp(static_cast<double>(settings.tone), 0.0, 1.0));
    last_read_ = 0.0;
    pickup_ = settings.pickup;
    double coil_hz = coil_lowest_hz * std::pow(coil_highest_hz / coil_lowest_hz, settings.pickup_tone);
    pickup_coil_.set_low_pass(coil_hz, coil_q, sample_rate_);
    pickup_coil_.clear();
    body_ = settings.body;
    blocker_in_ = 0.0;
    blocker_out_ = 0.0;
    height_ = 0.0;
    for (int r = 0; r < resonance_count; r++) {
        const BodyResonance& found = body_resonances[r];
        resonances_[r].tune(found.hz, found.q, found.level, sample_rate_);
    }

    write_pluck(static_cast<int>(std::round(period)), hardness, settings, press_number, place);
}

void PluckedString::set_loop_gain(double period) {
    double seconds_per_trip = period / sample_rate_;
    loop_gain_ = std::pow(10.0, -3.0 * seconds_per_trip / decay_seconds_);
}

void PluckedString::jump(double from_step, double to_step) {
    if (memory_ == nullptr) {
        return;
    }
    double longest_delay = size_ - 4.0;
    double from_period = std::clamp(1.0 / from_step, shortest_delay, longest_delay);
    double to_period = std::clamp(1.0 / to_step, shortest_delay, longest_delay);
    double stretch = from_period / to_period;

    // A little past one period, for reading between samples. The newest sample
    // stays the newest and the next one read stays the next one heard, so the
    // sound doesn't jump.
    int count = std::min(static_cast<int>(std::ceil(to_period)) + reading_margin, size_ - 1);
    for (int i = 0; i < count; i++) {
        double back = std::max(static_cast<double>(count - i) * stretch, shortest_delay);
        stretched_[i] = static_cast<float>(read_back(back));
    }
    for (int i = 0; i < count; i++) {
        memory_[(write_ - count + i) & mask_] = stretched_[i];
    }
    set_loop_gain(to_period);
}

void PluckedString::head_for(double cycle_step) {
    double longest_delay = size_ - 4.0;
    set_loop_gain(std::clamp(1.0 / cycle_step, shortest_delay, longest_delay));
}

void PluckedString::write_pluck(int period, double hardness, const StringSettings& settings,
                                std::uint64_t press_number, std::uint64_t place) {
    // Written from sample 4 on, with silence behind it for reading into.
    constexpr int first = 4;
    double pick = std::clamp(static_cast<double>(settings.pick), 0.02, 0.5);
    int pick_place = std::max(1, static_cast<int>(pick * period));

    // A pluck's noise, softer for a softer pluck and a duller string.
    Noise noise;
    noise.start(press_number, place);
    double softening = 0.05 + 0.9 * std::clamp(hardness, 0.0, 1.0) * (0.3 + 0.7 * settings.tone);
    double soft = 0.0;
    double mean = 0.0;
    for (int n = 0; n < period; n++) {
        soft += softening * (noise.height() - soft);
        noise.next();
        memory_[first + n] = static_cast<float>(soft);
        mean += soft;
    }
    mean /= period;

    // Plucking at a place silences what has a still point there, which is
    // the noise taking away a copy of itself that far behind. Working from the
    // end leaves the samples it reads still as they were.
    for (int n = period - 1; n >= 0; n--) {
        double here = memory_[first + n] - mean;
        double behind = 0.0;
        if (n >= pick_place) {
            behind = memory_[first + n - pick_place] - mean;
        }
        memory_[first + n] = static_cast<float>(here - behind);
    }

    // The shape a finger pulls the string into: up to the pick's place and
    // back down.
    double tallest = 0.0;
    for (int n = 0; n < period; n++) {
        double shape = 0.0;
        if (n < pick_place) {
            shape = static_cast<double>(n) / pick_place;
        } else {
            shape = static_cast<double>(period - n) / (period - pick_place);
        }
        // Centred on the middle line, so it never drifts.
        shape -= 0.5;
        double height = shape_share * shape * 2.0 + (1.0 - shape_share) * memory_[first + n];
        memory_[first + n] = static_cast<float>(height);
        tallest = std::max(tallest, std::abs(height));
    }
    if (tallest > 0.0) {
        for (int n = 0; n < period; n++) {
            memory_[first + n] = static_cast<float>(memory_[first + n] / tallest);
        }
    }
    write_ = first + period;
}

double PluckedString::read_back(double back) const {
    double place = static_cast<double>(write_) - back;
    double whole = std::floor(place);
    double t = place - whole;
    int at = static_cast<int>(whole);
    double before = memory_[(at - 1) & mask_];
    double here = memory_[at & mask_];
    double next = memory_[(at + 1) & mask_];
    double after = memory_[(at + 2) & mask_];

    // A curve through the four samples, so a pitch between samples keeps its
    // top.
    double a = -before / 6.0 + here / 2.0 - next / 2.0 + after / 6.0;
    double b = before / 2.0 - here + next / 2.0;
    double c = -before / 3.0 - here / 2.0 + next - after / 6.0;
    return ((a * t + b) * t + c) * t + here;
}

double PluckedString::height() const {
    return height_;
}

void PluckedString::advance(double cycle_step) {
    if (memory_ == nullptr) {
        return;
    }
    double longest_delay = size_ - 4.0;
    double period = std::clamp(1.0 / cycle_step, shortest_delay + smoothing_, longest_delay);

    // Averaging with the sample before delays it by the smoothing, which the
    // delay gives back so the pitch stays true.
    double read = read_back(period - smoothing_);
    double smoothed = (1.0 - smoothing_) * read + smoothing_ * last_read_;
    last_read_ = read;
    memory_[write_ & mask_] = static_cast<float>(loop_gain_ * smoothed);
    write_ = (write_ + 1) & mask_;

    double heard = read;
    if (pickup_ > 0.0) {
        double behind = read_back(std::max(shortest_delay, pickup_ * period));
        Stereo coil;
        coil.left = (read - behind) * pickup_trim;
        heard = pickup_coil_.next(coil).left;
    }
    if (body_ > 0.0) {
        double through = 0.0;
        for (int r = 0; r < resonance_count; r++) {
            through += resonances_[r].next(heard);
        }
        heard = heard * (1.0 - body_ * (1.0 - string_through_body)) + through * body_;
    }

    double blocked = heard - blocker_in_ + blocker_pole * blocker_out_;
    blocker_in_ = heard;
    blocker_out_ = blocked;
    height_ = blocked;
}
