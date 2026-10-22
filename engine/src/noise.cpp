#include "noise.h"

#include "lfo.h"

namespace {

// How far the state moves each sample: the golden ratio's share of all
// 64-bit numbers, the step SplitMix64 takes.
constexpr std::uint64_t state_step = 0x9e3779b97f4a7c15ULL;

// `bits` as a height from -1 to 1: the top 53 bits, as many as a double
// holds exactly, as a share from 0 to 1, then stretched.
double height_of(std::uint64_t bits) {
    double share = static_cast<double>(bits >> 11) / 9007199254740992.0;
    return 2.0 * share - 1.0;
}

}  // namespace

void Noise::start(std::uint64_t who, std::uint64_t what) {
    state_ = mix(mix(who) ^ what);
    height_ = height_of(mix(state_));
}

void Noise::next() {
    state_ += state_step;
    height_ = height_of(mix(state_));
}

double Noise::height() const {
    return height_;
}
