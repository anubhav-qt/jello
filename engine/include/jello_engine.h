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

// Turns the test tone on or off. It fades in and out over 10 ms.
// Safe to call while the audio is running.
void jello_engine_set_tone(JelloEngine* engine, bool on);

// How many samples per second the engine makes (48000).
int jello_engine_sample_rate(JelloEngine* engine);

// Makes sound without speakers, for writing files. Fills `out` with `frames`
// frames of stereo, left and right taking turns (left, right, left, ...),
// so `out` must hold frames * 2 floats. Only use this on an engine whose
// audio is not running.
void jello_engine_render(JelloEngine* engine, float* out, int frames);

#ifdef __cplusplus
}
#endif

#endif
