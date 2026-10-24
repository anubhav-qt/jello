#pragma once

#include <atomic>
#include <cstdint>
#include <vector>

// What the microphone hears, on its way from the audio thread to the Rust
// core (docs/design.md#voice). The mic's audio thread writes, one other
// thread reads, and neither ever locks or waits, like the mailbox. If the
// reader falls so far behind that the room runs out, the newest sound is
// dropped and counted: the reader owns where reading starts, so the writer
// can never move it.
class CaptureRing {
public:
    // How many mono samples fit: about 21 seconds at 48000 a second.
    static constexpr std::uint64_t capacity = 1 << 20;

    // Asks for all its memory now, so writing never does.
    CaptureRing();

    // The mic's audio thread only: keeps `frames` samples, as many as fit,
    // and remembers the loudest.
    void write(const float* samples, int frames);

    // The reading thread only: takes out up to `most` of the oldest samples
    // into `out` and returns how many.
    int read(float* out, int most);

    // The loudest sample, from 0, since the last time anyone asked.
    float take_peak();

    // How many samples were dropped for want of room, ever.
    std::uint64_t dropped() const;

    // Forgets everything kept. Only while nothing writes or reads.
    void clear();

private:
    static_assert(std::atomic<std::uint64_t>::is_always_lock_free,
                  "the capture ring needs lock-free 64-bit atomics");
    static_assert(std::atomic<float>::is_always_lock_free, "the capture ring needs a lock-free float");

    std::vector<float> samples_;

    // Both counters only ever go up. Sample number n lives in place
    // n % capacity.
    std::atomic<std::uint64_t> written_{0};  // how many were ever kept
    std::atomic<std::uint64_t> read_{0};     // how many were ever taken out
    std::atomic<float> peak_{0.0f};
    std::atomic<std::uint64_t> dropped_{0};
};
