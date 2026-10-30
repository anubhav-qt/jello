#include "mixer.h"

#include <algorithm>
#include <cmath>

namespace {

// How long a volume, pan or send takes to ease all the way across its range.
constexpr float ease_seconds = 0.01f;

// A quarter of a turn: the angle between fully left and fully right.
constexpr float quarter_turn = 1.5707963f;

}  // namespace

PanGains pan_gains(float pan) {
    // In the middle exactly as it is, and fully to one side exactly silent
    // in the other speaker, without the rounding of cos and sin.
    float to_middle = std::sqrt(2.0f);
    if (pan == 0.0f) {
        return PanGains{};
    }
    if (pan == 1.0f) {
        return PanGains{0.0f, to_middle};
    }
    if (pan == -1.0f) {
        return PanGains{to_middle, 0.0f};
    }

    // The pan as an angle, from 0 (fully left) to a quarter turn (fully
    // right). Cos and sin share the sound's power between the speakers.
    float angle = (pan + 1.0f) * 0.5f * quarter_turn;

    // Both are about 0.707 in the middle, so they are scaled up by
    // to_middle, to 1 there, where most sounds sit.
    PanGains gains;
    gains.left = std::cos(angle) * to_middle;
    gains.right = std::sin(angle) * to_middle;
    return gains;
}

Mixer::Mixer(int sample_rate) : master_(2 * buffer_frames, 0.0f), silence_(2 * buffer_frames, 0.0f) {
    for (int i = 0; i < chain_count; i++) {
        chains_.push_back(std::make_unique<Chain>(sample_rate));
    }
    for (int i = 0; i < master_chain; i++) {
        order_[i] = i;
        sounds_[i].assign(2 * buffer_frames, 0.0f);
    }

    float ease_samples = ease_seconds * static_cast<float>(sample_rate);

    // Volumes and sends go from 0 to 1, pans from -1 to 1, each all the way
    // in 10 ms.
    float volume_step = 1.0f / ease_samples;
    float pan_step = 2.0f / ease_samples;

    Strip strip{Ease(1.0f, volume_step), Ease(0.0f, pan_step), PanGains{}};
    tracks_.assign(Tracks::count, strip);
    buses_.assign(bus_count, strip);
    sends_.assign(Tracks::count * bus_count, Ease(0.0f, volume_step));

    routings_.assign(chain_count - first_bus_chain, Routing(sample_rate));
}

void Mixer::set_volume(int track, float volume) {
    tracks_[track].volume.set(volume);
}

void Mixer::set_pan(int track, float pan) {
    tracks_[track].pan.set(pan);
}

void Mixer::set_send(int track, int bus, float amount) {
    send(track, bus).set(amount);
}

void Mixer::set_bus_volume(int bus, float volume) {
    buses_[bus].volume.set(volume);
}

void Mixer::set_bus_pan(int bus, float pan) {
    buses_[bus].pan.set(pan);
}

void Mixer::set_order(const std::array<int, master_chain>& order) {
    order_ = order;
}

void Mixer::set_beats_per_sample(double beats_per_sample) {
    beats_per_sample_ = beats_per_sample;
    for (Routing& each : routings_) {
        each.set_beats_per_sample(beats_per_sample);
    }
}

void Mixer::on_beat(std::uint64_t number, double beats_per_sample) {
    beat_ = static_cast<double>(number);
    beats_per_sample_ = beats_per_sample;
    for (Routing& each : routings_) {
        each.on_beat(number, beats_per_sample, now_);
    }
}

void Mixer::set_lfo(int chain, int index, const LfoSettings& settings) {
    routing(chain).set_lfo(index, settings);
}

void Mixer::set_connection(int chain, int index, const Connection& connection) {
    routing(chain).set_connection(index, connection);
}

Routing& Mixer::routing(int number) {
    return routings_[number - first_bus_chain];
}

Chain& Mixer::chain(int number) {
    return *chains_[number];
}

Ease& Mixer::send(int track, int bus) {
    return sends_[track * bus_count + bus];
}

void Mixer::render(Tracks& tracks, float* out, int frames) {
    // A piece at a time, up to the end of each block of control_samples.
    int done = 0;
    while (done < frames) {
        int into_block = static_cast<int>(now_ % control_samples);
        int piece = std::min(control_samples - into_block, frames - done);
        render_piece(tracks, out + 2 * done, piece);
        done += piece;
    }
}

void Mixer::render_piece(Tracks& tracks, float* out, int frames) {
    for (int bus = 0; bus < bus_count; bus++) {
        std::vector<float>& input = sounds_[first_bus_chain + bus];
        std::fill(input.begin(), input.begin() + 2 * frames, 0.0f);
    }
    std::fill(master_.begin(), master_.begin() + 2 * frames, 0.0f);

    // Every chain is silent to its listeners until it has run.
    for (const float*& heard : heard_) {
        heard = silence_.data();
    }
    EffectContext context;
    context.chains = heard_.data();
    context.beat = beat_;
    context.beats_per_sample = beats_per_sample_;

    // At the start of a block the routing lists move on, as the synths'
    // do when they render it, and move the effects.
    bool block_starts = now_ % control_samples == 0;
    if (block_starts) {
        for (Routing& each : routings_) {
            each.tick(now_ + control_samples);
        }
    }

    for (int number : order_) {
        if (number < first_bus_chain) {
            run_track(tracks, number, frames, context);
        } else {
            run_bus(number - first_bus_chain, frames, context);
        }
    }

    Chain& master = chain(master_chain);
    if (block_starts) {
        master.move_values(routing(master_chain));
    }
    master.process(master_.data(), frames, context);
    for (int i = 0; i < 2 * frames; i++) {
        out[i] += master_[i];
    }
    beat_ += frames * beats_per_sample_;
    now_ += frames;
}

void Mixer::run_track(Tracks& tracks, int track, int frames, const EffectContext& context) {
    float* sound = sounds_[track].data();
    std::fill(sound, sound + 2 * frames, 0.0f);
    Synth& synth = tracks.synth(track);
    synth.render(sound, frames);
    Chain& effects = chain(track);
    if (now_ % control_samples == 0) {
        effects.move_values(synth.routing());
    }
    effects.process(sound, frames, context);
    apply_strip(tracks_[track], sound, frames);
    heard_[track] = sound;

    for (int i = 0; i < 2 * frames; i++) {
        master_[i] += sound[i];
    }
    for (int bus = 0; bus < bus_count; bus++) {
        send_to_bus(track, bus, sound, frames);
    }
}

void Mixer::run_bus(int bus, int frames, const EffectContext& context) {
    int number = first_bus_chain + bus;
    float* sound = sounds_[number].data();
    Chain& effects = chain(number);
    if (now_ % control_samples == 0) {
        effects.move_values(routing(number));
    }
    effects.process(sound, frames, context);
    apply_strip(buses_[bus], sound, frames);
    heard_[number] = sound;

    for (int i = 0; i < 2 * frames; i++) {
        master_[i] += sound[i];
    }
}

void Mixer::apply_strip(Strip& strip, float* sound, int frames) {
    for (int i = 0; i < frames; i++) {
        float volume = strip.volume.next();

        // The gains only change while the pan moves.
        bool pan_was_moving = strip.pan.is_moving();
        float pan = strip.pan.next();
        if (pan_was_moving) {
            strip.gains = pan_gains(pan);
        }

        sound[2 * i] *= volume * strip.gains.left;
        sound[2 * i + 1] *= volume * strip.gains.right;
    }
}

void Mixer::send_to_bus(int track, int bus, const float* sound, int frames) {
    // A send at 0, staying there, adds nothing, so it is skipped.
    Ease& amount = send(track, bus);
    if (!amount.is_moving()) {
        if (amount.heard() == 0.0f) {
            return;
        }
    }

    float* input = sounds_[first_bus_chain + bus].data();
    for (int i = 0; i < frames; i++) {
        float share = amount.next();
        input[2 * i] += sound[2 * i] * share;
        input[2 * i + 1] += sound[2 * i + 1] * share;
    }
}
