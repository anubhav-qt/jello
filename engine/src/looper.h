#pragma once

#include <array>
#include <cstdint>

#include "clock.h"
#include "mailbox.h"
#include "snap.h"
#include "synth.h"

// One change to the loop's layers or notes, for the screen
// (docs/adr/0024-the-looper-s-look.md, docs/adr/0026-layers.md). The looper
// sends one each time they change, and Rust keeps its own copy by applying
// them in order. Layers count from 0 in the order they were recorded, and a
// layer's notes from 0 in the order they started. Samples count from the
// loop's start.
struct LoopChange {
    enum class Type {
        forgot,         // every layer is forgotten: the loop was cleared
        started,        // note `index` of layer `layer` started: its pitch `note`, on `sample`
        ended,          // note `index` of layer `layer` ended on `sample`
        kept,           // the first loop closed, and layer 0 kept only its first `index` notes
        layer_started,  // layer `layer` started recording, after the others
        layer_stopped,  // layer `layer` stopped recording
        layer_dropped,  // layer `layer` stopped with no notes, so it is gone
        layer_removed,  // layer `layer` was removed; the layers after it move up
    };

    Type type = Type::forgot;
    int layer = 0;
    // Which note of the layer. For `kept`, how many notes the layer keeps.
    int index = 0;
    int note = 0;  // its pitch, 0 to 127; only for `started`
    std::uint64_t sample = 0;
};

// The mailbox that carries the loop's changes from the audio thread to Rust.
// Rust takes them out each time the screen asks what the looper is doing,
// every 50 ms while it is busy, so this many never pile up: it is room for
// as many notes as a loop holds.
using LoopChanges = Mailbox<LoopChange, 4096>;

// Records what the player plays and plays it back on the beat
// (docs/adr/0021-a-looper-on-a-beat.md). A loop is a list of notes, not
// sound: each note's pitch, how hard it was hit, and the samples it started
// and ended on, counted from the loop's start. It keeps them as they were
// played, and each time around plays them as planned by plan_loop: moved to
// the nearest half beat while "snap to beat" is on. The notes come in layers
// (docs/adr/0026-layers.md): the first loop is layer 0, and each layer
// recorded over it adds one. The conductor tells it about every live note
// and every beat, and it tells the synth when to play each loop note. Only
// the audio thread uses it, so nothing in here locks, allocates or waits.
class Looper {
public:
    // The most notes a loop holds, in all its layers. The room for them is
    // set aside when the engine is created, so recording never asks for more
    // memory. Once it is full, new notes still sound live but are not
    // recorded.
    static constexpr int max_notes = 4096;

    // The most layers a loop holds.
    static constexpr int max_layers = 8;

    // For remove_layer: the layer recorded last.
    static constexpr int newest_layer = -1;

    // What the looper is doing.
    enum class State {
        idle,         // no loop
        counting_in,  // one bar of clicks before recording starts
        recording,    // recording, with the click on
        closing,      // recording on to the next bar line, where it closes
        looping,      // playing the loop over and over, with the click off
        layering,     // playing the loop and recording a layer over it, with the click on
    };

    // The looper sends the loop's changes to `changes`.
    explicit Looper(LoopChanges& changes);

    // What right Shift does. When idle, it starts the count-in on the
    // current sample; the conductor starts a new bar there for it first.
    // During the count-in, it cancels it. While recording, it closes the
    // loop at the nearest bar line. While looping, it starts recording a new
    // layer over the loop at once, unless the loop already has max_layers.
    // While recording a layer, it stops: what was played plays from the next
    // time around, and a layer with no notes is dropped. While closing, it
    // does nothing.
    void press(const Clock& clock, Synth& synth);

    // Stops every layer's notes and forgets the loop, or cancels the
    // count-in or the recording. Live notes play on.
    void clear(Synth& synth);

    // What Esc does: stops layer `layer`'s notes and forgets it, or the
    // newest layer's for newest_layer. The layers after it move up, and if it
    // was being recorded, the recording stops. Removing the only layer clears
    // the loop, and so does removing any during the count-in or the first
    // recording. A layer that doesn't exist changes nothing.
    void remove_layer(int layer, Synth& synth);

    // How loud layer `layer` plays, from 0 (silent) to 1 (as played), its
    // notes already sounding too. Each layer starts at 1. A layer that
    // doesn't exist changes nothing.
    void set_layer_volume(int layer, float volume, Synth& synth);

    // The "snap to beat" switch, on to start with. While looping, the loop
    // changes right away: its notes stop, and those that should be sounding
    // on sample `now` start again. Otherwise the next loop plays by it.
    void set_snap(bool snap, std::uint64_t now, Synth& synth);

    // Live notes, so they can be recorded. `now` is the sample they are
    // heard from. Notes played in the count-in's last quarter beat count as
    // played on beat 1.
    void note_on(int note, int velocity, std::uint64_t now);
    void note_off(int note, std::uint64_t now);
    void all_notes_off(std::uint64_t now);

    // True when the looper has something to do on the current sample: a
    // beat starts, or a loop note starts or ends.
    bool has_something_now(const Clock& clock) const;

    // Does everything the looper has to do on the current sample.
    void act_now(const Clock& clock, Synth& synth);

    State state() const;
    bool is_idle() const;

    // True while the player needs the click: counting in, recording and
    // recording a layer.
    bool wants_click() const;

    // A number for the screen: the count-in's beat (1 to 4), the bar being
    // recorded (from 1), or how many bars the loop lasts. 0 when idle.
    int number() const;

    // For the screen too: how many samples into the count-in, the recording
    // or the repeat playing now the looper is on sample `now`. 0 when idle.
    std::uint64_t position(std::uint64_t now) const;

private:
    // How many notes MIDI has, 0 to 127.
    static constexpr int pitch_count = 128;

    // One layer of the loop. Its notes sit together in notes_, and the
    // layers one after another in the order they were recorded, so the
    // layer being recorded is always last and its new notes go on the end.
    struct Layer {
        int first = 0;    // the place of its first note, in notes_ and in plan_
        int count = 0;    // how many notes it has
        int planned = 0;  // how many of them its plan covers, from its first
        // Which of the synth's layer slots plays its notes, 0 to 7. A layer
        // keeps its slot while the layers before it are removed and its
        // number changes, so the notes it has sounding are always its own.
        int slot = 0;
        int next_start = 0;  // the next note to start, in its plan
        int next_end = 0;    // the next note to end, in its plan_ends_
    };

    // True while recording the first loop, including on to the bar line
    // where it closes.
    bool is_recording() const;

    // True while the loop plays: looping, or recording a layer over it.
    bool is_playing() const;

    // True if a note played on sample `now` is recorded: while recording,
    // in the count-in's last quarter beat, and while recording a layer.
    bool records_at(std::uint64_t now) const;

    // Sends one change to the screen. If its mailbox is full, the change is
    // dropped: the sound never waits for the screen.
    void tell(LoopChange::Type type, int layer, int index, int note, std::uint64_t sample);

    // Each beat and bar moves the looper along.
    void on_beat(const Clock& clock, Synth& synth);
    void count_in(std::uint64_t now);
    void on_bar(std::uint64_t now, Synth& synth);

    void start_recording(std::uint64_t now);

    // Adds an empty layer after the others, being recorded.
    void start_layer(Synth& synth);

    // The press that stops recording a layer.
    void stop_layer(Synth& synth);

    // A slot no layer plays on.
    int free_slot() const;

    // Takes layer `layer` out, with its notes, and moves the ones after it
    // up.
    void take_out_layer(int layer, Synth& synth);

    // The layer that note `place` in notes_ belongs to.
    int layer_holding(int place) const;

    // Ends the recorded note `note` that is still held, if there is one.
    void end_held_note(int note, std::uint64_t now);

    // Ends every recorded note still held on the loop's end.
    void end_held_notes_at_the_end();

    // The second press, while recording.
    void close_at_nearest_bar_line(const Clock& clock, Synth& synth);

    // Ends the first loop on sample `end`, `bars` bars after it started, and
    // starts looping.
    void close(std::uint64_t end, int bars);

    // Works out where each time around plays the first `count` notes of
    // layer `layer`, by the snap switch.
    void make_plan(int layer, int count);

    // Starts a repeat of the loop on sample `start`. What each layer was
    // given since the repeat before plays from this one on.
    void start_repeat(std::uint64_t start);

    // When a repeat starts on or before the current sample, after the looper
    // has acted on it: starts the notes that would be sounding `position`
    // samples in, those due on that very sample too, and skips the ones
    // already over.
    void catch_up(std::uint64_t position, Synth& synth);
    void catch_up_layer(Layer& layer, std::uint64_t position, Synth& synth);

    // Whether a layer's next note to start, or to end, is due `position`
    // samples into the repeat.
    bool is_start_due(const Layer& layer, std::uint64_t position) const;
    bool is_end_due(const Layer& layer, std::uint64_t position) const;

    // Plays every start and end due `position` samples into the repeat.
    void play_due(std::uint64_t position, Synth& synth);
    bool play_next_end(Layer& layer, std::uint64_t position, Synth& synth);
    bool play_next_start(Layer& layer, std::uint64_t position, Synth& synth);

    LoopChanges& changes_;

    bool snap_ = true;  // the "snap to beat" switch

    State state_ = State::idle;
    std::uint64_t count_in_start_ = 0;  // the sample the count-in started on
    int count_in_beat_ = 0;             // the count-in's beat, 1 to 4
    // The beat's length, locked from the count-in on.
    BeatLength beat_{};
    // The sample the loop starts on, one bar after the count-in started.
    std::uint64_t recording_start_ = 0;
    int bar_ = 0;                  // the bar being recorded, from 1
    std::uint64_t bar_start_ = 0;  // the sample that bar started on

    // The loop's layers, in the order they were recorded.
    std::array<Layer, max_layers> layers_{};
    int layer_count_ = 0;

    // Every layer's notes as they were played, in the order they started,
    // one layer after another.
    std::array<LoopNote, max_notes> notes_{};
    int note_count_ = 0;

    // For each pitch, the place in notes_ of the recorded note still held,
    // or no note, and the sample its key went down on.
    std::array<int, pitch_count> held_{};
    std::array<std::uint64_t, pitch_count> pressed_at_{};

    // Where each time around plays the same notes (plan_loop): each layer's
    // in the order they start, in the same places as in notes_.
    std::array<LoopNote, max_notes> plan_{};

    // For each layer, its planned notes by their place in its plan, counted
    // from its first, in the order they end. In the same places too.
    std::array<int, max_notes> plan_ends_{};

    std::uint64_t length_ = 0;  // how many samples the loop lasts
    int bars_ = 0;              // how many bars the loop lasts

    // The repeat playing now.
    std::uint64_t repeat_start_ = 0;  // the sample it started on
    int bars_played_ = 0;             // its bars already over
};
