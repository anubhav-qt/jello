#pragma once

#include <vector>

// A line of past samples that can be read back from any distance, even
// between two samples: the heart of chorus, delay and reverb. It is sized
// when made, off the audio thread; writing and reading never allocate.
class DelayLine {
public:
    // The shortest distance read can reach, in samples: reading between
    // two samples needs one newer sample on each side.
    static constexpr double shortest = 2.0;

    // Room to read up to `longest` samples back.
    explicit DelayLine(int longest);

    // The sample written `samples` writes ago, 1 being the newest, read
    // before the next write. Between two samples, it is worked out from
    // the four around it on a smooth curve (4-point Hermite), so a moving
    // distance bends the pitch without crackling. The distance is kept
    // from shortest to the longest it was made for.
    double read(double samples) const;

    // Adds the newest sample. Numbers too tiny to hear are kept as 0, so a
    // fading echo never slows the processor down (denormals).
    void write(double sample);

private:
    // The sample written `writes` writes ago, a whole number from 1.
    double at(int writes) const;

    std::vector<float> samples_;
    int mask_;
    int next_ = 0;
    double longest_;
};
