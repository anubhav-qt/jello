#pragma once

#include <array>
#include <cstdint>

#include "lfo.h"

// The routing list (docs/adr/0031-lfos-and-a-routing-list.md): what moves
// what. Each connection takes a source, like an LFO, and moves a target,
// like the pitch, by a signed amount in the target's own units. Each song
// has its own list. Only the audio thread uses it, so nothing in here
// locks, allocates or waits.

// How many connections a song has. Unused ones have no source.
constexpr int connection_count = 64;

// How often the routing list is worked out: every this many samples,
// counted from the engine's first sample, so the sound never depends on how
// it was split into chunks. In between, every move goes in a straight line.
constexpr int control_samples = 32;

// The longest the vibrato key can take to rise or fall, in ms.
constexpr int vibrato_key_longest_ms = 10000;

// Who plays notes, each with a vibrato key and key presses of their own:
// live play, the song, and each of the loop's 8 layer slots. See player_of
// in voice.h.
constexpr int player_count = 10;
constexpr int live_player = 0;
constexpr int song_player = 1;
constexpr int first_layer_player = 2;

// What moves a target. Read both ways, each goes from -1 to 1; read one
// way, from 0 to 1.
enum class RoutingSource {
    none,  // an unused connection
    lfo_1,
    lfo_2,
    lfo_3,
    lfo_4,
    lfo_5,
    lfo_6,
    lfo_7,
    lfo_8,
    vibrato_key,        // 0 while up, 1 while held, rising and falling in straight lines
    note_height,        // the note's height: -1 five octaves below middle C, 1 five above
    random,             // a random number each note starts with
    loudness_envelope,  // where the note's loudness envelope is, as a share of its peak
    filter_envelope,    // where the note's filter envelope is
};

// What a connection moves, each in its own units. The first ones move live,
// while a note sounds; the last ones, from `wave` on, are read once as each
// note starts.
enum class RoutingTarget {
    pitch,             // cents: hundredths of a half step
    loudness,          // a share: 1 moves it all the way to silence
    pan,               // a share: 1 moves it all the way right
    cutoff,            // octaves
    resonance,         // a share
    key_tracking,      // a share
    detune,            // cents
    spread,            // a share
    loudness_sustain,  // a share of the peak
    filter_amount,     // octaves
    filter_sustain,    // a share of the amount
    lfo_1_speed,       // octaves: 1 is twice as fast
    lfo_2_speed,
    lfo_3_speed,
    lfo_4_speed,
    lfo_5_speed,
    lfo_6_speed,
    lfo_7_speed,
    lfo_8_speed,
    wave,              // whole steps through sine, triangle, square, saw, and round again
    copies,            // whole copies
    loudness_attack,   // octaves of time: 1 is twice as long
    loudness_decay,
    loudness_release,
    filter_attack,
    filter_decay,
    filter_release,
};

// How many sources and targets there are, `none` included.
constexpr int source_count = 14;
constexpr int target_count = 27;

// One connection: `source` moves `target` by `amount`, in the target's own
// units, up to most_amount(target) either way.
struct Connection {
    RoutingSource source = RoutingSource::none;
    RoutingTarget target = RoutingTarget::pitch;
    float amount = 0.0f;
    // Both ways, the target moves from -amount to amount; one way, from 0
    // to amount.
    bool one_way = false;
    // For pitch only: the move is rounded to whole half steps.
    bool snap = false;
    // Another source the amount is multiplied by, read one way, from 0
    // (no move at all) to 1 (the full amount). None leaves it at full.
    RoutingSource scaled_by = RoutingSource::none;
};

// The most a connection can move `target` either way.
float most_amount(RoutingTarget target);

// Which LFO `source` is, from 0, or -1 if it is no LFO.
int lfo_of(RoutingSource source);

// Which LFO's speed `target` is, from 0, or -1 if it is no LFO's speed.
int lfo_speed_of(RoutingTarget target);

// True for the targets read once as each note starts.
bool is_read_at_start(RoutingTarget target);

// How far `connection` moves its target. `source` is its source's reading,
// both ways, and `fade` how far that has faded in, from 0 to 1. `scale` is
// its scaled-by source's reading, one way and faded in, or 1 for none.
double connection_move(const Connection& connection, const Slide& source, double fade, double scale);

// The settings every note reads, and everything shared between notes: the
// LFOs that run for every note, each player's vibrato key, and each
// player's key presses. Every 32 samples the synth moves it on with tick,
// and then each sounding voice works out its own moves from it.
class Routing {
public:
    explicit Routing(int sample_rate);

    // Changes LFO `index`, from 0. A shared LFO keeps its place in its cycle.
    void set_lfo(int index, const LfoSettings& settings);

    // Changes connection `index`, from 0.
    void set_connection(int index, const Connection& connection);

    // How long the vibrato key takes to rise from 0 to 1 and to fall back,
    // from 0 (at once) to vibrato_key_longest_ms.
    void set_vibrato_key_times(int rise_ms, int fall_ms);

    const LfoSettings& lfo(int index) const;

    // The connections that have a source, in order.
    int used_count() const;
    const Connection& used(int place) const;

    // True if any connection moves the cutoff, which brings the filter in
    // even when it is open.
    bool moves_cutoff() const;

    // How many beats pass each sample, at the song's tempo. The beat need
    // not be running: LFOs in beats count beats at the tempo anyway.
    void set_beats_per_sample(double beats_per_sample);
    double beats_per_sample() const;

    // Beat `number` starts on sample `now`, counted from 0 since the beat
    // started, at `beats_per_sample`. On beat 0, every shared LFO in beats
    // jumps to its start point, gliding there so it never clicks. On the
    // others, those whose speed nothing moves line up with the beat again.
    void on_beat(std::uint64_t number, double beats_per_sample, std::uint64_t now);

    // Holds or lets go of `player`'s vibrato key.
    void set_vibrato_key(int player, bool held);

    // Where `player`'s vibrato key is, from 0 (up) to 1 (held).
    double vibrato_key(int player) const;

    // Counts a key press by `player` on sample `now` and returns its number,
    // from 0. Notes that start on the same sample are one press, so a chord
    // is one press.
    std::uint64_t press(int player, std::uint64_t now);

    // Counts `player`'s presses from 0 again.
    void restart_presses(int player);

    // Where shared LFO `index` is, for LFOs in Hz or beats.
    Slide shared_lfo(int index) const;

    // Where shared LFO `index` is for `player`, for LFOs in presses.
    Slide pressed_lfo(int player, int index) const;

    // Where a per-note LFO in presses starts for press `press`, `offset`
    // cycles on from its start point.
    double press_height(int index, std::uint64_t press, double offset, std::uint64_t who) const;

    // How many cycles LFO `index` moves each sample, its speed moved by
    // `speed_move` octaves. 0 for LFOs in presses.
    double cycles_per_sample(int index, double speed_move) const;

    // How far LFO `index` has faded in, `age_ms` and `age_beats` after a
    // note started, from 0 to 1.
    double fade(int index, double age_ms, double age_beats) const;

    // Moves everything shared on to sample `block_end`, the end of the
    // next 32 samples: shared LFOs, vibrato keys and slides between presses.
    void tick(std::uint64_t block_end);

private:
    // How one shared LFO in presses moves for one player: from where it was
    // to its height for the latest press, over its smoothing.
    struct PressSlide {
        double from = 0.0;
        double to = 0.0;
        double samples_done = 0.0;
        double samples_long = 0.0;
    };

    // Works out the shared LFOs' heights after they move.
    void read_shared_lfos();

    // Works out how far the shared LFOs move each other's speeds.
    void move_shared_speeds();

    // True for an LFO that runs for every note, in Hz or beats.
    bool is_shared_running(int index) const;

    // Where shared LFO `index` would be in its cycle, by the beat.
    double cycles_by_beat(int index) const;

    // Puts every player's slide of LFO `index` right on its latest press.
    void settle_press_slides(int index);

    int sample_rate_;
    int glide_samples_;  // how long a jump to the beat glides

    std::array<LfoSettings, lfo_count> lfos_{};
    std::array<Connection, connection_count> connections_{};
    std::array<int, connection_count> used_{};
    int used_count_ = 0;
    bool moves_cutoff_ = false;

    double key_rise_samples_ = 0.0;
    double key_fall_samples_ = 0.0;

    // The sample everything shared has been moved on to.
    std::uint64_t clock_ = 0;

    double beats_per_sample_;
    // Beats counted since the beat last started, or since the engine
    // started.
    double beat_position_ = 0.0;

    // Each shared LFO's place: cycles since cycle 0 began, not counting its
    // start point.
    std::array<double, lfo_count> cycles_{};
    // How far its speed is moved, in octaves.
    std::array<double, lfo_count> speed_moves_{};
    // A jump to the beat, added on and shrinking away, like a voice's gap.
    std::array<double, lfo_count> gaps_{};
    std::array<double, lfo_count> gap_samples_left_{};
    // Its height now.
    std::array<Slide, lfo_count> readings_{};

    std::array<bool, player_count> key_held_{};
    std::array<double, player_count> key_values_{};

    std::array<std::uint64_t, player_count> presses_{};
    std::array<bool, player_count> pressed_yet_{};
    std::array<std::uint64_t, player_count> last_press_sample_{};
    std::array<std::array<PressSlide, lfo_count>, player_count> press_slides_{};
};
