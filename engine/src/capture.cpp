#include "capture.h"

#include <cmath>

CaptureRing::CaptureRing() : samples_(capacity, 0.0f) {}

void CaptureRing::write(const float* samples, int frames) {
    std::uint64_t written = written_.load();
    std::uint64_t read = read_.load();
    std::uint64_t room = capacity - (written - read);

    std::uint64_t kept = static_cast<std::uint64_t>(frames);
    if (kept > room) {
        dropped_.store(dropped_.load() + (kept - room));
        kept = room;
    }
    for (std::uint64_t i = 0; i < kept; i++) {
        samples_[(written + i) % capacity] = samples[i];
    }

    // Only now count them, so the reader never sees a half-written place.
    written_.store(written + kept);

    float loudest = 0.0f;
    for (int i = 0; i < frames; i++) {
        float level = std::fabs(samples[i]);
        if (level > loudest) {
            loudest = level;
        }
    }
    // The reader may set the peak back to 0 at any moment, so it only goes
    // up when this one is still louder than what is there.
    float before = peak_.load();
    while (loudest > before) {
        if (peak_.compare_exchange_weak(before, loudest)) {
            break;
        }
    }
}

int CaptureRing::read(float* out, int most) {
    if (most <= 0) {
        return 0;
    }
    std::uint64_t read = read_.load();
    std::uint64_t written = written_.load();

    std::uint64_t taken = written - read;
    if (taken > static_cast<std::uint64_t>(most)) {
        taken = static_cast<std::uint64_t>(most);
    }
    for (std::uint64_t i = 0; i < taken; i++) {
        out[i] = samples_[(read + i) % capacity];
    }

    // Only now count them, so the writer never reuses a place still being
    // read.
    read_.store(read + taken);
    return static_cast<int>(taken);
}

float CaptureRing::take_peak() {
    return peak_.exchange(0.0f);
}

std::uint64_t CaptureRing::dropped() const {
    return dropped_.load();
}

void CaptureRing::clear() {
    written_.store(0);
    read_.store(0);
    peak_.store(0.0f);
    dropped_.store(0);
}
