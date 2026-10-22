#pragma once

#include <array>
#include <cstdint>
#include <vector>

#include "filter.h"
#include "lfo.h"
#include "oscillator.h"
#include "routing.h"
#include "unison.h"
#include "voice.h"

// The instrument as a whole (docs/adr/0009-how-notes-become-sound.md). The
// conductor passes it notes on the audio thread, and it turns them into
// sound. It has 32 voices, so up to 32 notes sound at once. Only the audio
// thread uses it, so nothing in here locks, allocates or waits.
class Synth {
public:
    // How many layers a loop can have, each with its own volume
    // (docs/adr/0026-layers.md).
    static constexpr int layer_count = 8;

    // The pitch a loop plays the vibrato key on
    // (docs/adr/0032-the-vibrato-key.md): one past the highest note, so the
    // looper records, snaps and plays its presses back like a note's.
    static constexpr int vibrato_key_note = 128;

    explicit Synth(int sample_rate);

    // These act at once, so the next sound rendered hears them. `source`
    // says who plays the note: a note only ever touches the notes of its own
    // source, so the loop's C and a C played live never cut each other off.

    // Starts `note` on a voice. For live notes and the song; the loop plays
    // its notes by layer, below.
    void note_on(int note, int velocity, Source source);
    // Lets go of `note`, so it fades out.
    void note_off(int note, Source source);
    // Lets go of every note `source` plays, every layer's for the loop.
    void all_notes_off(Source source);

    // The same for a loop's notes, each played by its layer. `layer` is the
    // slot the looper gave the layer, 0 to 7. A layer's notes only ever touch
    // its own, so two layers' Cs never cut each other off. Its
    // vibrato_key_note holds and lets go of the layer's own vibrato key, and
    // letting go of its notes lets go of that too.
    void loop_note_on(int note, int velocity, int layer);
    void loop_note_off(int note, int layer);
    void layer_notes_off(int layer);

    // Holds or lets go of the live vibrato key
    // (docs/adr/0032-the-vibrato-key.md). It only moves live notes. Letting
    // go of every live note lets go of it too.
    void set_vibrato_key(bool held);

    // How loud layer `layer` plays, from 0 (silent) to 1 (as played), its
    // notes already sounding too. Each voice eases there, so it never clicks.
    // Every layer starts at 1. Live notes and the song always play at 1.
    void set_layer_volume(int layer, float volume);

    // Notes started after this play oscillator `index`, from 0, as
    // `oscillator` says (docs/adr/0035-oscillators-with-kinds.md). Notes
    // started before it keep theirs.
    void set_oscillator(int index, const OscillatorSettings& oscillator);

    // Moves the filter's cutoff and resonance (docs/adr/0028-a-low-pass-filter.md).
    // Every note hears it, even those already sounding, each gliding there
    // so it never clicks. The filter starts open.
    void set_filter(const FilterSettings& settings);

    // Notes started after this use `envelope` as their filter envelope.
    // Notes started before it keep theirs. It starts at an amount of 0, so
    // it changes nothing.
    void set_filter_envelope(const FilterEnvelopeSettings& envelope);

    // Notes started after this are played by `unison`'s copies
    // (docs/adr/0029-unison.md). Notes started before keep theirs. It starts
    // at 1 copy, which is off.
    void set_unison(const UnisonSettings& unison);

    // Notes started after this use `envelope` as their loudness envelope.
    // Notes started before keep theirs.
    void set_loudness_envelope(const LoudnessEnvelopeSettings& envelope);

    // Notes started after this have their cutoff follow middle C's by
    // `key_tracking` (NoteSound::key_tracking). Notes started before keep
    // theirs. It starts at 1.
    void set_key_tracking(float key_tracking);

    // Notes started after this are tuned from A4 at `a4_hz`. Notes started
    // before keep their pitch. It starts at 440.
    void set_tuning(float a4_hz);

    // The routing list (docs/adr/0031-lfos-and-a-routing-list.md): LFO
    // `index`, connection `index`, and how long the vibrato key takes to
    // rise and fall. Every note hears a change from the next 32 samples on,
    // even those already sounding, except in the settings read as a note
    // starts. A new synth's LFOs and connections move nothing.
    void set_lfo(int index, const LfoSettings& settings);
    void set_connection(int index, const Connection& connection);
    void set_vibrato_key_times(int rise_ms, int fall_ms);

    // How many beats pass each sample at the song's tempo, for the LFOs in
    // beats, and beat `number` starting on the current sample
    // (Routing::on_beat).
    void set_beats_per_sample(double beats_per_sample);
    void on_beat(std::uint64_t number, double beats_per_sample);

    // Counts the song's key presses from 0 again, so a song played again
    // moves its LFOs in presses the same way.
    void restart_song_presses();

    // Adds every voice to `frames` frames of stereo (left, right, left,
    // ...). It adds instead of overwriting, so the conductor can render a
    // chunk in pieces and put the click in the same buffer. Many voices can
    // add up to more than 1.0: the conductor keeps the sum in range with the
    // soft clip. Every control_samples samples, counted from the first, the
    // routing list is worked out again.
    void render(float* out, int frames);

private:
    // Moves the routing list on and has every sounding voice work out its
    // moves for the next control_samples samples.
    void tick();

    // Where the voices' filters glide to: the filter's settings, brought in
    // even when open if a connection moves the cutoff.
    void update_filter_targets();

    // Starts `note` for `source`, and for the loop, layer `layer`. Every
    // other note says layer 0.
    void start_note(int note, int velocity, Source source, int layer);

    // The voice still sounding `note` for `source` and `layer`, held or
    // fading, or nullptr if none is.
    Voice* find_ringing_voice(int note, Source source, int layer);

    // How loud `voice` should play: its layer's volume for a loop's note, and
    // 1 for any other.
    float volume_for(const Voice& voice) const;

    // A silent voice, or nullptr if every voice is busy.
    Voice* find_free_voice();

    // The voice fading out that is quietest right now, or nullptr if no voice
    // is fading.
    Voice* find_quietest_fading_voice();

    // The voice whose note was pressed longest ago. Only asked when every
    // voice is held, so that is the note held the longest.
    Voice* find_oldest_voice();

    // Made once, when the synth is created, and never resized, so the audio
    // thread never allocates.
    std::vector<Voice> voices_;

    // How new notes sound: their oscillators, envelopes, key tracking,
    // copies and tuning. It starts where a new song does.
    NoteSound note_sound_{};

    // The filter's settings, and where every voice's filter glides to.
    FilterSettings filter_settings_{};
    FilterTargets filter_targets_ = filter_targets_of(FilterSettings{});

    // The song's LFOs and routing list, and everything the notes share.
    Routing routing_;

    // The sample the next sound rendered starts on, counted from the first.
    std::uint64_t now_ = 0;

    // Each layer's volume, by its slot.
    std::array<float, layer_count> layer_volumes_{};

    // How many notes have been pressed so far. Each press gets the next
    // number, so a smaller number means pressed earlier. A counter, not a
    // clock: two notes pressed in the same chunk still get different numbers.
    std::uint64_t press_count_ = 0;
};
