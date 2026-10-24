// The one way the engine includes miniaudio, so every file sees the same
// settings.

#pragma once

// We only use miniaudio to talk to the speakers and to listen to the mic.
// Its file decoders, sound generators and mixing engine are switched off:
// Jello makes its own sound.
#define MA_NO_DECODING
#define MA_NO_ENCODING
#define MA_NO_GENERATION
#define MA_NO_NODE_GRAPH
#define MA_NO_ENGINE

// On macOS, link Apple's audio frameworks the normal way instead of loading
// them while running, which Apple's app signing (notarization) needs.
// build.rs links the frameworks.
#ifdef __APPLE__
#define MA_NO_RUNTIME_LINKING
#endif

#include "miniaudio.h"
