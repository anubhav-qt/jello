#pragma once

#include <array>
#include <vector>

#include "biquad.h"
#include "delay_line.h"
#include "effect.h"
#include "filter.h"

// The reverb's values, numbered.
constexpr int reverb_size = 0;        // 0 to 1
constexpr int reverb_decay = 1;       // seconds, reverb_shortest_decay to reverb_longest_decay
constexpr int reverb_pre_delay = 2;   // ms, 0 to reverb_longest_pre_delay_ms
constexpr int reverb_damping = 3;     // Hz, reverb_lowest_damping_hz to reverb_highest_damping_hz
constexpr int reverb_low_cut = 4;     // Hz, reverb_lowest_cut_hz to reverb_highest_cut_hz
constexpr int reverb_diffusion = 5;   // 0 to 1
constexpr int reverb_modulation = 6;  // 0 to 1
constexpr int reverb_width = 7;       // 0 to 1
constexpr int reverb_freeze = 8;      // 0 or 1
constexpr int reverb_mix = 9;         // 0 to 1
constexpr int reverb_value_count = 10;

constexpr float reverb_shortest_decay = 0.1f;
constexpr float reverb_longest_decay = 30.0f;
constexpr float reverb_longest_pre_delay_ms = 500.0f;
constexpr float reverb_lowest_damping_hz = 1000.0f;
constexpr float reverb_highest_damping_hz = 20000.0f;
constexpr float reverb_lowest_cut_hz = 20.0f;
constexpr float reverb_highest_cut_hz = 1000.0f;

// True if the reverb has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange reverb_value_range(int value);

bool is_valid_reverb_value(int value, float number);

// The reverb (docs/design.md#mix-in-the-engine): a feedback delay network,
// the usual way to build a room. Eight delays, each a different length,
// feed into each other through a mix that keeps the loudness, so every
// echo splits into more and the echoes smear into a wash. Each time round
// they are turned down just enough to fade by 60 dB over the decay, and a
// high cut on each (the damping) makes the highs fade sooner, as in a real
// room. The size sets the delays' lengths and slides there over 200 ms.
// Before the network, the pre-delay holds the sound back and four
// all-pass filters a side smear it at once, more with more diffusion.
// Modulation swings each delay a little, for a lush tail. After it, a low
// cut and the width, from one middle sound to the full stereo. Freeze
// holds the tail forever and lets nothing new in. A new one: half size,
// 1.5 s, no pre-delay, 8 kHz damping, nothing cut, 70% diffusion, 30%
// modulation, full width, and the starting mix (start_mix).
class Reverb : public Effect {
public:
    Reverb(int sample_rate, bool on_bus);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    static constexpr int line_count = 8;
    static constexpr int smear_count = 4;

    // An all-pass filter: it lets every pitch through as loud as it came,
    // but smears it in time.
    struct Smear {
        DelayLine line;
        int length;
    };

    // `sound` through the all-passes in `smears`, each at `share`.
    static double smeared(double sound, std::vector<Smear>& smears, double share);

    // Works out each delay's fade per time round for the size and decay
    // heard now.
    void set_fades(double scale, double decay);

    // Sets the low cut to `hz`.
    void set_low_cut(double hz);

    int sample_rate_;

    double size_ = 0.5;
    double decay_ = 1.5;
    double pre_delay_ms_ = 0.0;
    double damping_hz_ = 8000.0;
    double low_cut_hz_ = reverb_lowest_cut_hz;
    double diffusion_ = 0.7;
    double modulation_ = 0.3;
    double width_ = 1.0;
    bool freeze_ = false;
    double mix_;

    Glide heard_size_;
    Glide heard_decay_;
    Glide heard_pre_delay_ms_;
    Glide heard_damping_hz_;
    Glide heard_low_cut_hz_;
    Glide heard_diffusion_;
    Glide heard_modulation_;
    Glide heard_width_;
    Glide heard_freeze_;
    Glide heard_mix_;

    DelayLine pre_left_;
    DelayLine pre_right_;
    std::vector<Smear> left_smears_;
    std::vector<Smear> right_smears_;
    std::vector<DelayLine> lines_;
    Biquad low_cut_;
    double set_low_cut_hz_ = 0.0;

    // How far each damping filter moves toward its input each sample, and
    // the damping it was worked out for.
    double damping_share_ = 1.0;
    double set_damping_hz_ = 0.0;

    // Each delay's length at full size, in samples; its fade per time
    // round; what its damping remembers; and where its swing is, as a
    // share of a turn.
    std::array<double, line_count> full_lengths_{};
    std::array<double, line_count> fades_{};
    std::array<double, line_count> damped_{};
    std::array<double, line_count> turns_{};

    // The scale and decay the fades were worked out for.
    double set_scale_ = -1.0;
    double set_decay_ = -1.0;
};
