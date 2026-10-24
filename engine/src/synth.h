#pragma once

#include <array>
#include <cstdint>
#include <vector>

#include "drum_kit.h"
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
    // looper records, snaps and plays its presses back like a note's. And
    // the glide key's (docs/adr/0036-glide.md), one past that.
    static constexpr int vibrato_key_note = 128;
    static constexpr int glide_key_note = 129;

    explicit Synth(int sample_rate);

    // These act at once, so the next sound rendered hears them. `source`
    // says who plays the note: a note only ever touches the notes of its own
    // source, so the loop's C and a C played live never cut each other off.

    // Starts `note` on a voice. For live notes and the song; the loop plays
    // its notes by layer, below. A drum note hits its pad on the drum kit
    // instead (drum_kit.h).
    void note_on(int note, int velocity, Source source);
    // Lets go of `note`, so it fades out. A drum plays to its end anyway,
    // so letting go of one does nothing.
    void note_off(int note, Source source);
    // Lets go of every note `source` plays, every layer's for the loop.
    void all_notes_off(Source source);

    // The same for a loop's notes, each played by its layer. `layer` is the
    // slot the looper gave the layer, 0 to 7. A layer's notes only ever touch
    // its own, so two layers' Cs never cut each other off. Its
    // vibrato_key_note and glide_key_note hold and let go of the layer's own
    // vibrato and glide keys, and letting go of its notes lets go of those
    // too.
    void loop_note_on(int note, int velocity, int layer);
    void loop_note_off(int note, int layer);
    void layer_notes_off(int layer);

    // Holds or lets go of the live vibrato key
    // (docs/adr/0032-the-vibrato-key.md). It only moves live notes. Letting
    // go of every live note lets go of it too.
    void set_vibrato_key(bool held);

    // Holds or lets go of the live glide key (docs/adr/0036-glide.md). While
    // a player holds its glide key, each of its melody notes, a note that
    // starts on its own, slides in from its last melody note. Pressed while
    // the last one is still held, the voice already sounding bends to it
    // instead, and lets go of it to bend back to the newest note still held.
    // A chord, notes starting together, never glides. It only glides live
    // notes. Letting go of every live note lets go of it too.
    void set_glide_key(bool held);

    // Notes started after this take `glide_ms`, from 0 to glide_longest_ms,
    // to slide while the glide key is held. It starts at 100.
    void set_glide_time(int glide_ms);

    // How loud layer `layer` plays, from 0 (silent) to 1 (as played), its
    // notes already sounding too. Each voice eases there, so it never clicks.
    // Every layer starts at 1. Live notes and the song always play at 1.
    void set_layer_volume(int layer, float volume);

    // Notes started after this play oscillator `index`, from 0, as
    // `oscillator` says (docs/adr/0035-oscillators-with-kinds.md). Notes
    // started before it keep theirs.
    void set_oscillator(int index, const OscillatorSettings& oscillator);

    // Notes started after this play `set` on oscillator `index`, from 0,
    // while it is a sample (docs/design.md#sampler), or nothing for
    // nullptr. Notes started before keep theirs.
    void set_instrument(int index, const SampleSet* set);

    // The drum kit's recordings, and pad `pad`'s settings
    // (DrumKit::set_kit and set_pad).
    void set_drum_kit(const SampleSet* set);
    void set_drum_pad(int pad, const DrumPad& settings);

    // The bit of every sample set new notes, notes waiting or sounding,
    // and the drum kit still play, so the engine knows which it can free.
    std::uint32_t sets_in_use() const;

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

    // How many notes can sound at once.
    static constexpr int voice_count = 32;

private:
    // How many notes a bent voice remembers it took over, still held.
    static constexpr int most_taken = 16;

    // A note waiting to start until every note of its moment has come, so a
    // chord can be told from a melody note: a player's notes wait while it
    // holds its glide key. It keeps how notes sounded when it was pressed.
    struct Waiting {
        int note = 0;
        int velocity = 0;
        Source source = Source::live;
        int layer = 0;
        NoteSound sound{};
        // Let go of before it started: it starts, then is let go at once.
        bool let_go = false;
    };

    // One player's melody (docs/adr/0036-glide.md).
    struct Melody {
        bool glide_key = false;
        // Its last melody note: the voice that played it, by its place in
        // voices_, with the press number it had, so it is known whether the
        // voice still plays it, and its pitch.
        bool has_last = false;
        int last_voice = 0;
        std::uint64_t last_press = 0;
        double last_hz = 0.0;
        // The notes that voice bent away from that are still held, oldest
        // first.
        std::array<int, most_taken> taken{};
        int taken_count = 0;
    };

    // Moves the routing list on and has every sounding voice work out its
    // moves for the next control_samples samples.
    void tick();

    // Once every note of the moment has come: the notes that waited start,
    // a melody note gliding, and each player's last melody note is kept.
    void settle();

    // How many of the notes started or waiting this moment `player` has, and
    // the first one's place.
    int started_by(int player, int& first) const;
    int waiting_by(int player, int& first) const;

    // Starts `waiting`'s melody note for `player` with its glide.
    void glide(int player, const Waiting& waiting);

    // Starts `waiting`'s note as any note starts, and lets go of it again if
    // it was let go of while it waited.
    void start_waiting(const Waiting& waiting);

    // `player`'s last melody note's voice, or nullptr once that voice has
    // fallen silent or plays another note.
    Voice* melody_voice(const Melody& melody);

    // Keeps `voice`'s note as `player`'s last melody note. The notes it
    // took over stay as they are.
    void remember_melody(int player, const Voice& voice);

    // Lets go of `note` for `source` and `layer`: a note waiting is let go
    // as soon as it starts, and the others right away.
    void let_go(int note, Source source, int layer);

    // Lets go of `note` now. A note a bent voice took over is forgotten, and
    // the bent voice bends back to the newest one still held instead of
    // fading out.
    void let_go_now(int note, Source source, int layer);

    // Forgets the notes waiting for `player`, and its melody, when every
    // one of its notes is let go at once.
    void forget_melody(int player);

    // Where the voices' filters glide to: the filter's settings, brought in
    // even when open if a connection moves the cutoff.
    void update_filter_targets();

    // Starts `note` for `source`, and for the loop, layer `layer`. Every
    // other note says layer 0. While its player holds the glide key, it
    // waits for the end of the moment.
    void start_note(int note, int velocity, Source source, int layer);

    // Starts `note` on a voice right away, sounding as `sound` says, and
    // returns the voice.
    Voice* play_note(int note, int velocity, Source source, int layer, const NoteSound& sound);

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

    // The notes waiting for the end of the moment, and the voices of the
    // notes started this moment without waiting, by their place in voices_.
    std::array<Waiting, voice_count> waiting_{};
    int waiting_count_ = 0;
    std::array<int, voice_count> started_{};
    int started_count_ = 0;

    // Every player's melody.
    std::array<Melody, player_count> melodies_{};

    // The drums, beside the voices.
    DrumKit drums_;
};
