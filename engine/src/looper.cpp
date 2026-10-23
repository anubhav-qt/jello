#include "looper.h"

#include <algorithm>
#include <limits>

namespace {

// In held_: no recorded note of this pitch is held.
constexpr int no_note = -1;

// The end of a recorded note whose key is still held.
constexpr std::uint64_t still_held = std::numeric_limits<std::uint64_t>::max();

}  // namespace

// The looper and the synth agree on how many layers there can be.
static_assert(Looper::max_layers == Synth::layer_count, "the looper and the synth hold the same number of layers");

Looper::Looper(LoopChanges& changes) : changes_(changes) {
    held_.fill(no_note);
}

void Looper::press(const Clock& clock, Synth& synth) {
    if (state_ == State::idle) {
        // The conductor has just started a new bar, so the count-in's first
        // beat is now. The tempo is locked from here until the loop is
        // cleared, so recording starts exactly one bar later. The first loop
        // is the first layer, so notes played just before recording starts
        // have somewhere to go.
        state_ = State::counting_in;
        count_in_start_ = clock.now();
        count_in_beat_ = 1;
        beat_ = clock.beat_length();
        recording_start_ = count_in_start_ + clock.samples_per_bar();
        start_layer(synth);
    } else if (state_ == State::counting_in) {
        clear(synth);
    } else if (state_ == State::recording) {
        close_at_nearest_bar_line(clock, synth);
    } else if (state_ == State::looping) {
        if (layer_count_ < max_layers) {
            state_ = State::layering;
            start_layer(synth);
        }
    } else if (state_ == State::layering) {
        stop_layer(synth);
    } else if (state_ == State::paused) {
        play(clock.now(), synth);
    }
    // While closing, a press does nothing.
}

void Looper::pause(std::uint64_t now, Synth& synth) {
    if (!is_playing()) {
        return;
    }
    if (state_ == State::layering) {
        stop_layer(synth);
    }
    // Stopping a layer with no notes, when it was the only one, clears the
    // loop.
    if (state_ == State::idle) {
        return;
    }

    // Keys still held while the layer was recorded end here.
    for (int note = 0; note < pitch_count; note++) {
        end_held_note(note, now);
    }
    synth.all_notes_off(Source::loop);
    state_ = State::paused;
}

void Looper::play(std::uint64_t now, Synth& synth) {
    if (state_ != State::paused) {
        return;
    }
    state_ = State::looping;

    // Every layer is planned again, since snap may have changed while
    // paused, and the loop starts over from its start right away.
    for (int i = 0; i < layer_count_; i++) {
        make_plan(i, layers_[i].count);
    }
    start_repeat(now);
    catch_up(0, synth);
}

void Looper::set_layer_playing(int layer, bool playing, std::uint64_t now, Synth& synth) {
    if (layer < 0) {
        return;
    }
    if (layer >= layer_count_) {
        return;
    }
    Layer& changed = layers_[layer];
    if (changed.playing == playing) {
        return;
    }
    changed.playing = playing;

    if (!playing) {
        synth.layer_notes_off(changed.slot);
        return;
    }
    if (!is_playing()) {
        return;
    }

    // Like the snap switch: the notes that should be sounding by now start
    // right away, and the rest wait their turn.
    changed.next_start = 0;
    changed.next_end = 0;
    catch_up_layer(changed, now - repeat_start_, synth);
}

void Looper::load(LoadedNotes& loaded, int count, int layers, int bars, BeatLength beat, Synth& synth) {
    clear(synth);

    beat_ = beat;
    bars_ = bars;
    std::uint64_t whole_bars = static_cast<std::uint64_t>(bars);
    length_ = whole_bars * beats_per_bar * beat.samples_per_minute / beat.tempo;

    for (int i = 0; i < layers; i++) {
        Layer& layer = layers_[i];
        layer.first = 0;
        layer.count = 0;
        layer.planned = 0;
        layer.slot = i;
        layer.next_start = 0;
        layer.next_end = 0;
        layer.playing = true;
        synth.set_layer_volume(i, 1.0f);
    }
    layer_count_ = layers;

    // The notes come layer by layer, so each layer's first note is where
    // the notes taken so far end when its own first note comes.
    LoadedNote taken{};
    for (int i = 0; i < count; i++) {
        if (!loaded.receive(taken)) {
            break;
        }
        Layer& layer = layers_[taken.layer];
        if (layer.count == 0) {
            layer.first = note_count_;
        }

        LoopNote kept = taken.note;
        if (kept.end > length_) {
            kept.end = length_;
        }
        if (kept.start > kept.end) {
            kept.start = kept.end;
        }
        notes_[note_count_] = kept;
        note_count_ += 1;
        layer.count += 1;
    }

    for (int i = 0; i < layer_count_; i++) {
        make_plan(i, layers_[i].count);
    }
    state_ = State::paused;
    tell(LoopChange::Type::loaded, layers, 0, 0, 0);
}

void Looper::clear(Synth& synth) {
    synth.all_notes_off(Source::loop);
    state_ = State::idle;
    layer_count_ = 0;
    note_count_ = 0;
    held_.fill(no_note);
    tell(LoopChange::Type::forgot, 0, 0, 0, 0);
}

void Looper::remove_layer(int layer, Synth& synth) {
    if (state_ == State::idle) {
        return;
    }
    // Before the first loop closes it is the only layer, being counted in or
    // recorded.
    if (!has_loop()) {
        clear(synth);
        return;
    }

    int place = layer;
    if (layer == newest_layer) {
        place = layer_count_ - 1;
    }
    if (place < 0) {
        return;
    }
    if (place >= layer_count_) {
        return;
    }

    if (layer_count_ == 1) {
        clear(synth);
        return;
    }

    // The layer being recorded is always the last one.
    bool was_recording = false;
    if (state_ == State::layering) {
        if (place == layer_count_ - 1) {
            was_recording = true;
        }
    }

    take_out_layer(place, synth);
    tell(LoopChange::Type::layer_removed, place, 0, 0, 0);
    if (was_recording) {
        state_ = State::looping;
    }
}

void Looper::set_layer_volume(int layer, float volume, Synth& synth) {
    if (layer < 0) {
        return;
    }
    if (layer >= layer_count_) {
        return;
    }
    synth.set_layer_volume(layers_[layer].slot, volume);
}

void Looper::set_snap(bool snap, std::uint64_t now, Synth& synth) {
    if (snap == snap_) {
        return;
    }
    snap_ = snap;
    if (!is_playing()) {
        return;
    }

    // Like closing at the bar line just passed: the notes that should be
    // sounding by now start right away, and the rest wait their turn. Each
    // layer is planned again from the notes it already plays; the ones it
    // was given since this repeat started still wait for the next.
    synth.all_notes_off(Source::loop);
    for (int i = 0; i < layer_count_; i++) {
        make_plan(i, layers_[i].planned);
        layers_[i].next_start = 0;
        layers_[i].next_end = 0;
    }
    catch_up(now - repeat_start_, synth);
}

void Looper::note_on(int note, int velocity, std::uint64_t now) {
    if (!records_at(now)) {
        return;
    }

    // Pressed again while still held, say by a second chord key: the synth
    // plays it again on the voice it already has. So the recording ends the
    // note here and starts it again, and plays back the same way.
    end_held_note(note, now);

    // A full loop records no more, but the note still sounds live.
    if (note_count_ == max_notes) {
        return;
    }

    // Notes are only ever recorded into the newest layer, so they go on the
    // end of notes_.
    int newest = layer_count_ - 1;
    Layer& layer = layers_[newest];
    LoopNote& added = notes_[note_count_];
    added.note = note;
    added.velocity = velocity;
    if (state_ == State::layering) {
        // Where it falls in the loop as it plays.
        added.start = now - repeat_start_;
    } else if (now < recording_start_) {
        // Played a little before recording starts, it counts as played on
        // beat 1.
        added.start = 0;
    } else {
        added.start = now - recording_start_;
    }
    added.end = still_held;
    held_[note] = note_count_;
    pressed_at_[note] = now;
    tell(LoopChange::Type::started, newest, layer.count, note, added.start);
    layer.count += 1;
    note_count_ += 1;
}

// Only notes being recorded are held, so these do nothing at other times.

void Looper::note_off(int note, std::uint64_t now) {
    end_held_note(note, now);
}

void Looper::all_notes_off(std::uint64_t now) {
    for (int note = 0; note < pitch_count; note++) {
        end_held_note(note, now);
    }
}

bool Looper::has_something_now(const Clock& clock) const {
    if (state_ == State::idle) {
        return false;
    }
    if (clock.beat_starts_now()) {
        return true;
    }
    if (!is_playing()) {
        return false;
    }

    std::uint64_t position = clock.now() - repeat_start_;
    for (int i = 0; i < layer_count_; i++) {
        if (is_end_due(layers_[i], position)) {
            return true;
        }
        if (is_start_due(layers_[i], position)) {
            return true;
        }
    }
    return false;
}

void Looper::act_now(const Clock& clock, Synth& synth) {
    if (clock.beat_starts_now()) {
        on_beat(clock, synth);
    }
    if (is_playing()) {
        play_due(clock.now() - repeat_start_, synth);
    }
}

Looper::State Looper::state() const {
    return state_;
}

bool Looper::is_idle() const {
    return state_ == State::idle;
}

bool Looper::is_paused() const {
    return state_ == State::paused;
}

bool Looper::needs_beat() const {
    if (state_ == State::idle) {
        return false;
    }
    return state_ != State::paused;
}

bool Looper::wants_click() const {
    if (state_ == State::counting_in) {
        return true;
    }
    if (state_ == State::layering) {
        return true;
    }
    return is_recording();
}

int Looper::number() const {
    if (state_ == State::counting_in) {
        return count_in_beat_;
    } else if (is_recording()) {
        return bar_;
    } else if (has_loop()) {
        return bars_;
    }
    return 0;
}

std::uint64_t Looper::position(std::uint64_t now) const {
    if (state_ == State::counting_in) {
        return now - count_in_start_;
    } else if (is_recording()) {
        return now - recording_start_;
    } else if (is_playing()) {
        return now - repeat_start_;
    }
    return 0;
}

bool Looper::is_recording() const {
    if (state_ == State::recording) {
        return true;
    }
    return state_ == State::closing;
}

bool Looper::is_playing() const {
    if (state_ == State::looping) {
        return true;
    }
    return state_ == State::layering;
}

bool Looper::has_loop() const {
    if (is_playing()) {
        return true;
    }
    return state_ == State::paused;
}

bool Looper::records_at(std::uint64_t now) const {
    if (is_recording()) {
        return true;
    }
    if (state_ == State::layering) {
        return true;
    }
    if (state_ != State::counting_in) {
        return false;
    }
    std::uint64_t quarter_beat = beat_.samples_per_minute / (4 * beat_.tempo);
    return now + quarter_beat >= recording_start_;
}

void Looper::tell(LoopChange::Type type, int layer, int index, int note, std::uint64_t sample) {
    LoopChange change;
    change.type = type;
    change.layer = layer;
    change.index = index;
    change.note = note;
    change.sample = sample;
    changes_.send(change);
}

void Looper::on_beat(const Clock& clock, Synth& synth) {
    if (state_ == State::counting_in) {
        count_in(clock.now());
        return;
    }
    if (clock.bar_starts_now()) {
        on_bar(clock.now(), synth);
    }
}

void Looper::count_in(std::uint64_t now) {
    // After the count-in's 4 beats, recording starts on the next one: beat 1
    // of the next bar.
    if (count_in_beat_ == beats_per_bar) {
        start_recording(now);
    } else {
        count_in_beat_ += 1;
    }
}

void Looper::on_bar(std::uint64_t now, Synth& synth) {
    if (state_ == State::recording) {
        bar_ += 1;
        bar_start_ = now;
    } else if (state_ == State::closing) {
        close(now, bar_);
        start_repeat(now);
    } else if (is_playing()) {
        bars_played_ += 1;
        if (bars_played_ == bars_) {
            // Beats start on whole samples, so a repeat can come out one
            // sample shorter than the loop. Whatever it has left plays now,
            // before the next repeat starts.
            play_due(length_, synth);
            start_repeat(now);
        }
    }
}

void Looper::start_recording(std::uint64_t now) {
    // `now` is recording_start_, worked out at the first press. Notes played
    // just before it are already recorded.
    state_ = State::recording;
    bar_ = 1;
    bar_start_ = now;
}

void Looper::start_layer(Synth& synth) {
    Layer& layer = layers_[layer_count_];
    layer.first = note_count_;
    layer.count = 0;
    layer.planned = 0;
    layer.slot = free_slot();
    layer.next_start = 0;
    layer.next_end = 0;
    layer.playing = true;
    synth.set_layer_volume(layer.slot, 1.0f);
    tell(LoopChange::Type::layer_started, layer_count_, 0, 0, 0);
    layer_count_ += 1;
}

void Looper::stop_layer(Synth& synth) {
    state_ = State::looping;
    int newest = layer_count_ - 1;
    if (layers_[newest].count > 0) {
        // Its notes already play from the next time around. Notes still held
        // record on until they are let go.
        tell(LoopChange::Type::layer_stopped, newest, 0, 0, 0);
        return;
    }

    // Nothing was played, so there is nothing to keep.
    tell(LoopChange::Type::layer_dropped, newest, 0, 0, 0);
    layer_count_ -= 1;

    // It was the only layer left, so the loop is empty.
    if (layer_count_ == 0) {
        clear(synth);
    }
}

int Looper::free_slot() const {
    // There are as many slots as layers, so one is always free.
    for (int slot = 0; slot < max_layers; slot++) {
        bool used = false;
        for (int i = 0; i < layer_count_; i++) {
            if (layers_[i].slot == slot) {
                used = true;
            }
        }
        if (!used) {
            return slot;
        }
    }
    return 0;
}

void Looper::take_out_layer(int layer, Synth& synth) {
    Layer gone = layers_[layer];
    synth.layer_notes_off(gone.slot);

    // Every note after its notes moves down into the gap they leave, and its
    // plan and the order its plan's notes end in with it. Those count from
    // the layer's first note, so they stay right.
    int after = gone.first + gone.count;
    for (int i = after; i < note_count_; i++) {
        notes_[i - gone.count] = notes_[i];
        plan_[i - gone.count] = plan_[i];
        plan_ends_[i - gone.count] = plan_ends_[i];
    }
    note_count_ -= gone.count;

    // Its notes still held are forgotten, and those of the layers after it
    // move down too.
    for (int note = 0; note < pitch_count; note++) {
        int held = held_[note];
        if (held == no_note) {
            continue;
        }
        if (held < gone.first) {
            continue;
        }
        if (held < after) {
            held_[note] = no_note;
        } else {
            held_[note] = held - gone.count;
        }
    }

    // The layers after it move up.
    for (int i = layer + 1; i < layer_count_; i++) {
        layers_[i - 1] = layers_[i];
        layers_[i - 1].first -= gone.count;
    }
    layer_count_ -= 1;
}

int Looper::layer_holding(int place) const {
    for (int i = 0; i < layer_count_; i++) {
        const Layer& layer = layers_[i];
        if (place < layer.first + layer.count) {
            return i;
        }
    }
    return layer_count_ - 1;
}

void Looper::end_held_note(int note, std::uint64_t now) {
    int held = held_[note];
    if (held == no_note) {
        return;
    }

    // It lasts as long as its key was held, so a note played a little early
    // keeps its length on beat 1. A layer's note never runs past the loop's
    // end, since one still held there is let go when the next repeat starts.
    LoopNote& ended = notes_[held];
    ended.end = ended.start + (now - pressed_at_[note]);

    int layer = layer_holding(held);
    tell(LoopChange::Type::ended, layer, held - layers_[layer].first, 0, ended.end);
    held_[note] = no_note;
}

void Looper::end_held_notes_at_the_end() {
    for (int note = 0; note < pitch_count; note++) {
        int held = held_[note];
        if (held == no_note) {
            continue;
        }
        notes_[held].end = length_;
        int layer = layer_holding(held);
        tell(LoopChange::Type::ended, layer, held - layers_[layer].first, 0, length_);
        held_[note] = no_note;
    }
}

void Looper::close_at_nearest_bar_line(const Clock& clock, Synth& synth) {
    // A loop is at least 1 bar, so in the first bar it closes at its end.
    if (bar_ == 1) {
        state_ = State::closing;
        return;
    }

    // Nearer the bar line just passed: the loop closes there, so its next
    // repeat has already started. Nearer the next one: recording goes on up
    // to it.
    std::uint64_t now = clock.now();
    std::uint64_t since_bar_line = now - bar_start_;
    if (since_bar_line * 2 < clock.samples_per_bar()) {
        close(bar_start_, bar_ - 1);
        start_repeat(bar_start_);
        catch_up(since_bar_line, synth);
    } else {
        state_ = State::closing;
    }
}

void Looper::close(std::uint64_t end, int bars) {
    state_ = State::looping;
    length_ = end - recording_start_;
    bars_ = bars;

    // Notes started on the loop's end or after it are left out: they were
    // played in the next bar, where the loop plays instead. The first loop
    // is the only layer, so its notes are all of them.
    Layer& layer = layers_[0];
    int kept = 0;
    while (kept < note_count_) {
        if (notes_[kept].start >= length_) {
            break;
        }
        kept += 1;
    }
    note_count_ = kept;
    layer.count = kept;
    tell(LoopChange::Type::kept, 0, kept, 0, 0);

    // A note still sounding at the loop's end is let go there, so nothing
    // keeps ringing.
    for (int i = 0; i < note_count_; i++) {
        if (notes_[i].end >= length_) {
            notes_[i].end = length_;
            tell(LoopChange::Type::ended, 0, i, 0, length_);
        }
    }

    held_.fill(no_note);
    make_plan(0, kept);
}

void Looper::make_plan(int layer, int count) {
    Layer& planning = layers_[layer];
    const LoopNote* played = notes_.data() + planning.first;
    LoopNote* plan = plan_.data() + planning.first;
    plan_loop(played, count, length_, beat_, snap_, plan);
    planning.planned = count;

    // Playback lets go of notes in the order they end, which is not the
    // order they start in: a chord can be held under a quicker note, and
    // moving notes can change it too. Sorting moves the numbers around in
    // place, so it asks for no memory.
    int* ends = plan_ends_.data() + planning.first;
    for (int i = 0; i < count; i++) {
        ends[i] = i;
    }
    auto ends_sooner = [plan](int a, int b) {
        return plan[a].end < plan[b].end;
    };
    std::sort(ends, ends + count, ends_sooner);
}

void Looper::start_repeat(std::uint64_t start) {
    repeat_start_ = start;
    bars_played_ = 0;

    // Recording a layer goes round and round: what was played since the
    // last repeat started joins the layer's plan here, so it plays from now
    // on, and a note still held is let go at the loop's end.
    end_held_notes_at_the_end();
    for (int i = 0; i < layer_count_; i++) {
        Layer& layer = layers_[i];
        if (layer.planned < layer.count) {
            make_plan(i, layer.count);
        }
        layer.next_start = 0;
        layer.next_end = 0;
    }
}

void Looper::catch_up(std::uint64_t position, Synth& synth) {
    for (int i = 0; i < layer_count_; i++) {
        catch_up_layer(layers_[i], position, synth);
    }
}

void Looper::catch_up_layer(Layer& layer, std::uint64_t position, Synth& synth) {
    const LoopNote* plan = plan_.data() + layer.first;
    const int* ends = plan_ends_.data() + layer.first;

    while (layer.next_start < layer.planned) {
        const LoopNote& starting = plan[layer.next_start];
        if (starting.start > position) {
            break;
        }
        // Due by now: it starts now if it would still be sounding, and its
        // layer is playing.
        if (starting.end > position) {
            if (layer.playing) {
                synth.loop_note_on(starting.note, starting.velocity, layer.slot);
            }
        }
        layer.next_start += 1;
    }

    while (layer.next_end < layer.planned) {
        const LoopNote& ending = plan[ends[layer.next_end]];
        if (ending.end > position) {
            break;
        }
        layer.next_end += 1;
    }
}

bool Looper::is_start_due(const Layer& layer, std::uint64_t position) const {
    if (layer.next_start == layer.planned) {
        return false;
    }
    const LoopNote& starting = plan_[layer.first + layer.next_start];
    return starting.start <= position;
}

bool Looper::is_end_due(const Layer& layer, std::uint64_t position) const {
    if (layer.next_end == layer.planned) {
        return false;
    }
    int ending = plan_ends_[layer.first + layer.next_end];
    return plan_[layer.first + ending].end <= position;
}

void Looper::play_due(std::uint64_t position, Synth& synth) {
    // Each layer plays on its own voices, so one layer's notes never touch
    // another's. Within a layer, ends go first, so a note that ends just as
    // the same note starts again lets go before it starts again. One at a
    // time, since each one played can let another through.
    for (int i = 0; i < layer_count_; i++) {
        Layer& layer = layers_[i];
        bool played = true;
        while (played) {
            played = play_next_end(layer, position, synth);
            if (!played) {
                played = play_next_start(layer, position, synth);
            }
        }
    }
}

bool Looper::play_next_end(Layer& layer, std::uint64_t position, Synth& synth) {
    if (!is_end_due(layer, position)) {
        return false;
    }

    // A note so short that it ends on the sample it starts waits for its
    // start.
    int ending = plan_ends_[layer.first + layer.next_end];
    if (ending >= layer.next_start) {
        return false;
    }

    synth.loop_note_off(plan_[layer.first + ending].note, layer.slot);
    layer.next_end += 1;
    return true;
}

bool Looper::play_next_start(Layer& layer, std::uint64_t position, Synth& synth) {
    if (!is_start_due(layer, position)) {
        return false;
    }

    // A paused layer's notes go by unheard.
    const LoopNote& starting = plan_[layer.first + layer.next_start];
    if (layer.playing) {
        synth.loop_note_on(starting.note, starting.velocity, layer.slot);
    }
    layer.next_start += 1;
    return true;
}
