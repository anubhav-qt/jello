// The C API of the Jello sound engine. This is the only part of the engine
// that the Rust core can see (docs/adr/0001-three-layers.md).

#ifndef JELLO_ENGINE_H
#define JELLO_ENGINE_H

#include <stdbool.h>

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

// The next four are for playing notes and picking their wave. They are safe
// to call while the audio is running, from one thread at a time. Each one
// only leaves a message for the audio thread, which acts on it at the start
// of its next chunk of sound (about every 10 ms). If 1024 messages are
// already waiting, new ones are dropped
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
