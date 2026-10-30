#include "delay_line.h"

#include <algorithm>
#include <cmath>

namespace {

// Under this a sample is kept as silence.
constexpr double tiniest = 1e-20;

}  // namespace

DelayLine::DelayLine(int longest) : longest_(longest) {
    // A power of two, so wrapping round is a mask. The 3 more cover the
    // samples read on each side of the longest distance.
    int size = 1;
    while (size < longest + 3) {
        size *= 2;
    }
    samples_.assign(size, 0.0f);
    mask_ = size - 1;
}

double DelayLine::at(int writes) const {
    return samples_[(next_ - writes) & mask_];
}

double DelayLine::read(double samples) const {
    double distance = std::clamp(samples, shortest, longest_);
    int whole = static_cast<int>(distance);
    double between = distance - whole;

    double newer = at(whole - 1);
    double here = at(whole);
    double older = at(whole + 1);
    double oldest = at(whole + 2);
    if (between == 0.0) {
        return here;
    }

    double slope = 0.5 * (older - newer);
    double bend = newer - 2.5 * here + 2.0 * older - 0.5 * oldest;
    double twist = 0.5 * (oldest - newer) + 1.5 * (here - older);
    return ((twist * between + bend) * between + slope) * between + here;
}

void DelayLine::write(double sample) {
    float kept = static_cast<float>(sample);
    if (std::fabs(sample) < tiniest) {
        kept = 0.0f;
    }
    samples_[next_] = kept;
    next_ = (next_ + 1) & mask_;
}
