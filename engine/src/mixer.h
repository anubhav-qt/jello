#pragma once

#include <array>
#include <cstdint>
#include <memory>
#include <vector>

#include "chain.h"
#include "ease.h"
#include "tracks.h"

// How loud a pan puts a sound in each speaker.
struct PanGains {
    float left = 1.0f;
    float right = 1.0f;
};

// The gains for `pan`, from -1 (fully left) to 1 (fully right), at equal
// loudness: in the middle both speakers play the sound as it is, and the
// further it goes to one side, the quieter the other speaker and the louder
// this one, so the sound keeps its loudness. Fully to one side, that speaker
// plays it about 3 dB louder and the other not at all.
PanGains pan_gains(float pan);

// The mix (docs/design.md#mix-in-the-engine): each track's strip, the send
// buses, and the master. A track's sound goes through its chain of effects,
// its volume and its pan, then into the master's input, and into each bus
// at its send. A bus goes through its own chain, volume and pan into the
// master's input too, and the master's input through the master's chain.
// Every volume, pan and send eases over 10 ms, so it never clicks. Only the
// audio thread uses it, so nothing in here locks, allocates or waits.
class Mixer {
public:
    // How many send buses a song can have.
    static constexpr int bus_count = 4;

    // The chains, by number: the tracks' from 0, then the buses', then the
    // master's.
    static constexpr int first_bus_chain = Tracks::count;
    static constexpr int master_chain = first_bus_chain + bus_count;
    static constexpr int chain_count = master_chain + 1;
    static_assert(chain_count == most_chains, "effects know how many chains there are");

    explicit Mixer(int sample_rate);

    // Chain `number`, from 0 to chain_count - 1, which the caller makes
    // sure of.
    Chain& chain(int number);

    // How loud track `track` plays, from 0 (silent) to 1 (as played). Every
    // track starts at 1. The caller makes sure `track` is below
    // Tracks::count, and `bus` below bus_count.
    void set_volume(int track, float volume);

    // Where track `track` sits between the speakers, from -1 (fully left) to
    // 1 (fully right). Every track starts in the middle, at 0.
    void set_pan(int track, float pan);

    // How much of track `track`'s sound, after its volume and pan, goes to
    // bus `bus`, from 0 to 1. Every send starts at 0.
    void set_send(int track, int bus, float amount);

    // A bus's own volume, from 0 to 1, starting at 1, and pan, starting at 0.
    void set_bus_volume(int bus, float volume);
    void set_bus_pan(int bus, float pan);

    // The order the chains before the master run in: every track's and
    // bus's once, every track before every bus, which the caller makes sure
    // of. A chain that listens to another hears it from the same samples
    // only if that one runs first; otherwise it hears silence. It starts in
    // number order.
    void set_order(const std::array<int, master_chain>& order);

    // The beat, for effects that follow it: how many beats pass each
    // sample, and beat `number`, counted from 0 since the beat started,
    // starting on the next sample rendered.
    void set_beats_per_sample(double beats_per_sample);
    void on_beat(std::uint64_t number, double beats_per_sample);

    // A bus's or the master's own LFOs and connections, by chain number,
    // from first_bus_chain to master_chain, which the caller makes sure of.
    // They move only the chain's effects. A track's effects are moved by
    // its synth's (Synth::routing).
    void set_lfo(int chain, int index, const LfoSettings& settings);
    void set_connection(int chain, int index, const Connection& connection);

    // Renders every track of `tracks` and adds the master's sound to
    // `frames` frames of stereo (left, right, left, ...). It adds instead of
    // overwriting, so the conductor can render a chunk in pieces and add the
    // click. It renders in pieces that end where the next control_samples
    // samples start, counted from the first, and moves the effects at
    // the start of each, so they move the same however the sound is split.
    void render(Tracks& tracks, float* out, int frames);

    // The loudest sample chain `number`'s sound reached, after its volume
    // and pan, since the last take, then starts again from silence. Tracks
    // and buses only: from 0 to master_chain - 1, which the caller makes
    // sure of.
    float take_peak(int number);

private:
    // How many frames each track renders into its own buffer at a time.
    static constexpr int buffer_frames = Chain::most_frames;

    // A volume and a pan, with the gains for the pan heard now, worked out
    // again only when the pan moves.
    struct Strip {
        Ease volume;
        Ease pan;
        PanGains gains;
    };

    // Renders up to the end of the control_samples block it starts in and
    // adds it to `out`.
    void render_piece(Tracks& tracks, float* out, int frames);

    // The routing list of bus or master chain `number`.
    Routing& routing(int number);

    // Track `track`'s part of a piece: renders it through its chain and
    // strip, into the master's input and each bus's.
    void run_track(Tracks& tracks, int track, int frames, const EffectContext& context);

    // Bus `bus`'s part: what the tracks sent it through its chain and
    // strip, into the master's input.
    void run_bus(int bus, int frames, const EffectContext& context);

    // Puts `frames` frames of `sound` through `strip`, in place.
    void apply_strip(Strip& strip, float* sound, int frames);

    // Keeps the loudest sample of `frames` frames of chain `number`'s sound.
    void keep_peak(int number, const float* sound, int frames);

    // Adds `frames` frames of `sound` to bus `bus`'s input at track
    // `track`'s send.
    void send_to_bus(int track, int bus, const float* sound, int frames);

    // Track `track`'s send to bus `bus`.
    Ease& send(int track, int bus);

    std::vector<Strip> tracks_;
    std::vector<Strip> buses_;

    // Every track's send to every bus, the first track's first.
    std::vector<Ease> sends_;

    // Every track's chain, every bus's and the master's, made with the
    // mixer.
    std::vector<std::unique_ptr<Chain>> chains_;

    std::array<int, master_chain> order_;

    // Each track's and bus's loudest sample since the last take.
    std::array<float, master_chain> peaks_{};

    // Each track's and bus's sound in this piece, by chain number, after
    // its chain and strip. A bus's holds what the tracks send it until it
    // runs. And where the tracks and buses meet before the master's chain.
    std::array<std::vector<float>, master_chain> sounds_;
    std::vector<float> master_;

    // A piece of silence, and what each chain sounds like to the effects
    // listening to it in this piece: its sound once it has run, silence
    // before.
    std::vector<float> silence_;
    std::array<const float*, chain_count> heard_{};

    // Where the next sample is, in beats since the beat started, and how
    // many beats pass each sample.
    double beat_ = 0.0;
    double beats_per_sample_ = 0.0;

    // Each bus's and the master's routing list, the first bus's first.
    std::vector<Routing> routings_;

    // The sample the next sound rendered starts on, counted from the first.
    std::uint64_t now_ = 0;
};
