// The C API of the Jello sound engine. This is the only part of the engine
// that the Rust core can see (docs/adr/0001-three-layers.md).

#ifndef JELLO_ENGINE_H
#define JELLO_ENGINE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// One sound engine. What is inside it is private to the C++ code.
typedef struct JelloEngine JelloEngine;

// Makes an engine. Returns NULL if there is not enough memory.
JelloEngine* jello_engine_create(void);

// Stops the engine's audio and frees it.
void jello_engine_destroy(JelloEngine* engine);

// Opens the default speakers and starts playing. Returns false if no audio
// device could be opened. Does nothing if the audio is already running.
bool jello_engine_start_audio(JelloEngine* engine);

// Closes the speakers. Does nothing if the audio is not running.
void jello_engine_stop_audio(JelloEngine* engine);

// The functions from here to jello_engine_set_filter_envelope are for
// playing notes, picking their wave, running the click and the looper,
// playing Compose's song and moving the filter. They are safe to call while the audio is running, from one
// thread at a time. Each one only leaves a message for the audio thread,
// which acts on it at the start of its next chunk of sound (about every
// 10 ms). If 1024 messages are already waiting, new ones are dropped
// (docs/adr/0009-how-notes-become-sound.md).

// Starts `note` (0 to 127, middle C is 60) at `velocity`, how hard the key was
// hit (1 to 127). A note or velocity outside those ranges is ignored.
void jello_engine_note_on(JelloEngine* engine, int note, int velocity);

// Lets go of `note`, so it fades out. A note outside 0 to 127 is ignored.
void jello_engine_note_off(JelloEngine* engine, int note);

// Lets go of every note.
void jello_engine_all_notes_off(JelloEngine* engine);

// The waves a note can play.
enum {
    JELLO_WAVE_SINE = 0,
    JELLO_WAVE_TRIANGLE = 1,
    JELLO_WAVE_SQUARE = 2,
    JELLO_WAVE_SAW = 3,
};

// Makes notes sent after this play `wave`, one of the JELLO_WAVE_ numbers
// above. Notes sent before it keep their wave, even while they ring on. Any
// other number is ignored. A new engine starts on JELLO_WAVE_TRIANGLE.
void jello_engine_set_wave(JelloEngine* engine, int wave);

// Starts the click: a tick on every beat, higher on the first beat of each
// bar. Its first tick is beat 1 of a bar. Does nothing if the click is
// already on (docs/adr/0021-a-looper-on-a-beat.md).
void jello_engine_start_click(JelloEngine* engine);

// Stops the click. A tick already sounding rings out.
void jello_engine_stop_click(JelloEngine* engine);

// The slowest and fastest tempos the engine plays, in beats per minute.
enum {
    JELLO_SLOWEST_TEMPO = 40,
    JELLO_FASTEST_TEMPO = 240,
};

// Plays the beat at `tempo` beats per minute from the next beat on, so the
// beat already playing keeps its length. A tempo outside JELLO_SLOWEST_TEMPO
// to JELLO_FASTEST_TEMPO is ignored. So is any tempo while the looper is
// busy, from its first press until it is cleared, so a loop keeps its beat.
// A new engine starts at 120.
void jello_engine_set_tempo(JelloEngine* engine, int tempo);

// Right Shift's looper (docs/adr/0021-a-looper-on-a-beat.md). The first
// press counts in one bar of clicks, then records with the click on. The
// second closes the loop at the nearest bar line, at least 1 bar in, and the
// loop repeats with the click off while live notes play over it. A press
// during the count-in cancels it. The loop's notes come in layers
// (docs/adr/0026-layers.md): the first loop is layer 1, and each later pair
// of presses adds one. The first of the pair starts recording a layer at
// once, with the click on, round and round: what is played joins the loop
// from the next time around. The second stops it; a layer with no notes is
// dropped. A loop holds at most JELLO_LOOP_MAX_LAYERS layers, and past that
// the first of the pair does nothing.
void jello_engine_press_looper(JelloEngine* engine);

// Stops every layer's notes and forgets the loop, or cancels the count-in or
// the recording. Live notes play on.
void jello_engine_clear_loop(JelloEngine* engine);

// The most layers a loop holds, and the number for the layer recorded last.
enum {
    JELLO_LOOP_MAX_LAYERS = 8,
    JELLO_NEWEST_LAYER = -1,
};

// What Esc does (docs/adr/0026-layers.md): stops layer `layer`'s notes and
// forgets it, counting from 0 in the order the layers were recorded, or the
// newest layer for JELLO_NEWEST_LAYER. The layers after it move up, and if it
// was being recorded, the recording stops. Removing the only layer clears
// the loop, and so does removing any during the count-in or the first
// recording. A layer that doesn't exist changes nothing, and any other
// number is ignored.
void jello_engine_remove_layer(JelloEngine* engine, int layer);

// How loud layer `layer` plays (counted as above), from 0 (silent) to 1 (as
// played), its notes already sounding too, eased so it never clicks. Each
// layer starts at 1. A layer that doesn't exist changes nothing, and a
// layer or volume outside those ranges is ignored.
void jello_engine_set_layer_volume(JelloEngine* engine, int layer, float volume);

// The "snap to beat" switch (docs/adr/0021-a-looper-on-a-beat.md). While it
// is on, each time around the loop plays its notes moved to the nearest half
// beat; while it is off, as they were played. Live notes are never moved. It
// changes a loop already playing right away: the loop's notes stop, and
// those that should be sounding start again. A new engine starts with it on.
void jello_engine_set_snap(JelloEngine* engine, bool snap);

// The most chords a song can have, and the most notes in each chord.
enum {
    JELLO_SONG_MAX_CHORDS = 512,
    JELLO_CHORD_MAX_NOTES = 4,
};

// One chord of a song.
typedef struct JelloSongChord {
    // Its notes, 0 to 127; only the first `note_count` count.
    int notes[JELLO_CHORD_MAX_NOTES];
    // How many notes it has, 0 to JELLO_CHORD_MAX_NOTES.
    int note_count;
    // How hard its notes are hit, 1 to 127.
    int velocity;
    // How long it plays, in samples.
    uint64_t length;
} JelloSongChord;

// Plays `chords`, `count` of them, one after another, each for its own
// length, with no gap between them (docs/adr/0022-compose-as-a-tape.md). The
// first starts at the next chunk of sound, and each one after on the exact
// sample the one before ends. Whatever the song was playing stops first. The
// song ignores the beat and the tempo. Its notes are its own: live notes and
// the loop never cut them off, and the looper never records them. Returns
// false and plays nothing if `count` is below 0 or above
// JELLO_SONG_MAX_CHORDS, if any chord is outside the ranges above, or if the
// mailbox has no room for all of it.
bool jello_engine_play_song(JelloEngine* engine, const JelloSongChord* chords, int count);

// Lets go of what the song is playing and forgets it. Live notes and the
// loop play on.
void jello_engine_stop_song(JelloEngine* engine);

// The low-pass filter's ranges (docs/adr/0028-a-low-pass-filter.md): the
// lowest cutoff and the cutoff that means open, in Hz, the most octaves the
// filter envelope opens it, and the longest any of its stages takes.
enum {
    JELLO_FILTER_LOWEST_HZ = 20,
    JELLO_FILTER_OPEN_HZ = 20000,
    JELLO_FILTER_MOST_OCTAVES = 8,
    JELLO_FILTER_LONGEST_MS = 10000,
};

// Every note goes through a low-pass filter of its own: it lets low sounds
// through and turns high ones down, 12 dB per octave above its cutoff.
// `cutoff_hz` is middle C's cutoff, from JELLO_FILTER_LOWEST_HZ to
// JELLO_FILTER_OPEN_HZ, and every other note's follows it, an octave higher
// for each octave up. At JELLO_FILTER_OPEN_HZ the filter is open: off, so
// notes sound exactly as without it. `resonance`, from 0 to 1, boosts the
// sound right at the cutoff. Every note hears a change, even those already
// sounding, each gliding there over 20 ms so it never clicks. Values
// outside those ranges are ignored. A new engine starts open, with no
// resonance.
void jello_engine_set_filter(JelloEngine* engine, float cutoff_hz, float resonance);

// The filter envelope: when a note starts, it opens the cutoff by up to
// `amount` octaves, from 0 to JELLO_FILTER_MOST_OCTAVES. It rises over
// `attack_ms`, falls over `decay_ms` to `sustain` (a share of the amount,
// from 0 to 1) while the key is held, and closes over `release_ms` once it
// is let go; each from 0 to JELLO_FILTER_LONGEST_MS. Notes started after
// this use it, and notes started before keep theirs. Values outside those
// ranges are ignored. A new engine starts at an amount of 0, which changes
// nothing, with an attack of 5 ms, a decay of 300 ms, a sustain of 0 and a
// release of 300 ms.
void jello_engine_set_filter_envelope(JelloEngine* engine, float amount, int attack_ms, int decay_ms, float sustain,
                                      int release_ms);

// What the looper can be doing.
enum {
    JELLO_LOOPER_IDLE = 0,
    JELLO_LOOPER_COUNTING_IN = 1,
    JELLO_LOOPER_RECORDING = 2,
    JELLO_LOOPER_LOOPING = 3,
    // Playing the loop and recording a layer over it, with the click on.
    JELLO_LOOPER_LAYERING = 4,
};

typedef struct JelloLooperStatus {
    // One of the JELLO_LOOPER_ numbers above.
    int state;
    // While counting in, the beat (1 to 4). While recording, the bar being
    // recorded, from 1. While looping or recording a layer, how many bars
    // the loop lasts. 0 when idle.
    int number;
    // How many samples into the count-in, the recording or the repeat
    // playing now. 0 when idle. Each starts on beat 1 of a bar.
    uint64_t position;
    // True while a press, clear or removal is still waiting in the mailbox,
    // so the state doesn't show it yet.
    bool waiting;
} JelloLooperStatus;

// What the looper is doing, as of the last chunk of sound. Call it from the
// same thread as the functions above.
JelloLooperStatus jello_engine_looper_status(JelloEngine* engine);

// What can change in the loop's layers and notes
// (docs/adr/0024-the-looper-s-look.md, docs/adr/0026-layers.md). Layers
// count from 0 in the order they were recorded, and a layer's notes from 0
// in the order they started.
enum {
    // Every layer is forgotten: the loop was cleared. Recording only ever
    // starts after that, so a new loop starts with no layers.
    JELLO_LOOP_FORGOT = 0,
    // Note `index` of layer `layer` started: its pitch `note`, on `sample`.
    JELLO_LOOP_STARTED = 1,
    // Note `index` of layer `layer` ended on `sample`.
    JELLO_LOOP_ENDED = 2,
    // The first loop closed, and layer 0 kept only its first `index` notes.
    JELLO_LOOP_KEPT = 3,
    // Layer `layer` started recording, after the others. The first press
    // starts layer 0, for the first loop.
    JELLO_LOOP_LAYER_STARTED = 4,
    // Layer `layer` stopped recording.
    JELLO_LOOP_LAYER_STOPPED = 5,
    // Layer `layer` stopped with no notes, so it is gone.
    JELLO_LOOP_LAYER_DROPPED = 6,
    // Layer `layer` was removed. The layers after it move up.
    JELLO_LOOP_LAYER_REMOVED = 7,
};

typedef struct JelloLoopChange {
    // One of the JELLO_LOOP_ numbers above.
    int type;
    // Which layer.
    int layer;
    // Which note of the layer. For JELLO_LOOP_KEPT, how many notes it keeps.
    int index;
    // Its pitch, 0 to 127; only for JELLO_LOOP_STARTED.
    int note;
    // Counted from the loop's start.
    uint64_t sample;
} JelloLoopChange;

// Takes out the oldest change to the loop's layers and notes into `change`
// and returns true, or returns false if none is waiting. Applying every
// change in order keeps a copy of the loop's layers and notes. Call it from the same thread as the
// functions above, and often: once 4,096 changes are waiting, newer ones are
// dropped, since the sound never waits for the screen.
bool jello_engine_next_loop_change(JelloEngine* engine, JelloLoopChange* change);

// The most notes a loop holds, in all its layers. Past that, new notes still
// sound live but are not recorded.
enum {
    JELLO_LOOP_MAX_NOTES = 4096,
};

// One note of a loop, for jello_engine_plan_loop.
typedef struct JelloLoopNote {
    // Its pitch, 0 to 127.
    int note;
    // The samples it starts and ends on, counted from the loop's start.
    uint64_t start;
    uint64_t end;
} JelloLoopNote;

// Works out where a loop plays one layer's notes each time around, by the
// same rule the looper plays them by, so the screen can draw them where they
// are heard. `played` holds `count` notes as they were played, in any order,
// like a layer in the copy that jello_engine_next_loop_change keeps, of a
// loop `bars` bars long at `tempo`. `planned` must have room for `count`
// notes, and gets them in the order they start each time around: moved to
// the nearest half beat if `snap` is on, as played if not. Returns false and
// fills nothing if `count` is below 0 or above JELLO_LOOP_MAX_NOTES, `bars`
// is below 1, the tempo is outside JELLO_SLOWEST_TEMPO to
// JELLO_FASTEST_TEMPO, or a note is outside 0 to 127. It leaves no message
// and never touches the sound, so any thread can call it.
bool jello_engine_plan_loop(JelloEngine* engine, const JelloLoopNote* played, int count, int bars, int tempo,
                            bool snap, JelloLoopNote* planned);

// How many samples the song has played, counted from its first chord's
// start, as of the last chunk of sound, or -1 when no song is playing. A
// play still on its way counts as 0, and a stop on its way as -1. When the
// song has played to its very end, its last chord lets go on the next
// sample. Call it from the same thread as the functions above.
int64_t jello_engine_song_position(JelloEngine* engine);

// How many samples per second the engine makes (48000).
int jello_engine_sample_rate(JelloEngine* engine);

// Makes sound without speakers, for writing files. Fills `out` with `frames`
// frames of stereo, left and right taking turns (left, right, left, ...),
// so `out` must hold frames * 2 floats. Notes sent before this call take
// effect at its first frame. Only use this on an engine whose audio is not
// running.
void jello_engine_render(JelloEngine* engine, float* out, int frames);

#ifdef __cplusplus
}
#endif

#endif
