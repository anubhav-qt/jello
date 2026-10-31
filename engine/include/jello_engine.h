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

// The functions from here to jello_engine_set_glide_time are for playing
// notes, setting their oscillators, running the click and the looper, playing
// Compose's song, moving the filter, setting unison, the loudness envelope,
// key tracking and tuning, the LFOs, the routing list, the vibrato key and
// glide. They are safe to call while the audio is running,
// from one thread at a time. Each one only leaves a message for the audio
// thread, which acts on it at the start of its next chunk of sound (about
// every 10 ms). If 1024 messages are already waiting, new ones are dropped
// (docs/adr/0009-how-notes-become-sound.md).

// Tracks (docs/design.md#tracks-in-the-engine): an engine has
// JELLO_TRACK_COUNT, counted from 0, each an instrument with its own sound,
// played only by the notes sent to it. Every function below that plays a
// note or sets a sound takes the `track` it is for, and ignores a track
// outside 0 to JELLO_TRACK_COUNT - 1. A track's loop layers play in its
// sound, up to JELLO_TRACK_MAX_LAYERS of them.
enum {
    JELLO_TRACK_COUNT = 9,
    JELLO_TRACK_MAX_LAYERS = 8,
};

// Starts `note` (0 to 127, middle C is 60, or a drum pad's from
// JELLO_DRUM_FIRST_NOTE) at `velocity`, how hard the key was hit (1 to
// 127). A note or velocity outside those ranges is ignored.
void jello_engine_note_on(JelloEngine* engine, int track, int note, int velocity);

// Lets go of `note`, so it fades out. A note outside 0 to 127 is ignored.
void jello_engine_note_off(JelloEngine* engine, int track, int note);

// Lets go of every live note on every track.
void jello_engine_all_notes_off(JelloEngine* engine);

// How loud track `track` plays, from 0 (silent) to 1 (as played), eased
// over 10 ms so it never clicks. Every track starts at 1. A volume outside
// that range is ignored.
void jello_engine_set_track_volume(JelloEngine* engine, int track, float volume);

// The mix (docs/design.md#mix-in-the-engine). Each track has a pan, and a
// send to each of JELLO_BUS_COUNT buses, counted from 0. A bus adds up what
// the tracks send it and plays it at its own volume and pan. Everything
// meets in the master, and the master limiter is the last step before the
// speakers. Every volume, pan and send eases over 10 ms, so it never
// clicks, and anything out of range is ignored.
enum {
    JELLO_BUS_COUNT = 4,
};

// Where track `track` sits between the speakers, from -1 (fully left) to 1
// (fully right), at equal loudness: in the middle, both play it as it is.
// Every track starts at 0.
void jello_engine_set_track_pan(JelloEngine* engine, int track, float pan);

// How much of track `track`'s sound, after its volume and pan, goes to bus
// `bus`, from 0 to 1. Every send starts at 0.
void jello_engine_set_send(JelloEngine* engine, int track, int bus, float amount);

// A bus's volume, from 0 to 1, starting at 1, and its pan, from -1 to 1,
// starting at 0.
void jello_engine_set_bus_volume(JelloEngine* engine, int bus, float volume);
void jello_engine_set_bus_pan(JelloEngine* engine, int bus, float pan);

// The master limiter's limits.
enum {
    JELLO_LIMITER_MOST_BOOST_DB = 24,
    JELLO_LIMITER_LOWEST_CEILING_DB = -12,
    JELLO_LIMITER_SHORTEST_RELEASE_MS = 1,
    JELLO_LIMITER_LONGEST_RELEASE_MS = 1000,
};

// The master limiter: the sound is made `boost_db` louder, from 0 to
// JELLO_LIMITER_MOST_BOOST_DB, then held under `ceiling_db`, from
// JELLO_LIMITER_LOWEST_CEILING_DB to 0, and comes back up two thirds of the
// way in `release_ms`. The boost and the ceiling ease over 10 ms. A new
// engine starts at 0 dB, -1 dB and 100 ms.
void jello_engine_set_limiter(JelloEngine* engine, float boost_db, float ceiling_db, int release_ms);

// Makes the limiter look 1 ms ahead, for WAV renders: cleaner, and it
// catches the peaks between samples too, but every sound comes out that
// much later. Returns how many frames that is; throw them away right after
// starting what plays, and the render lines up again. Call it before the
// first render, on an engine whose audio never runs.
int jello_engine_use_lookahead(JelloEngine* engine);

// The most the limiter turned the sound down in the last chunk, in dB, 0
// for not at all.
float jello_engine_limiter_reduction(JelloEngine* engine);

// Effects. Every track, bus and the master has a chain of JELLO_EFFECT_SLOTS
// slots, and the sound goes through them in order. Chains are numbered:
// the tracks' from 0, the buses' from JELLO_FIRST_BUS_CHAIN, and the
// master's, JELLO_MASTER_CHAIN, which comes before the click and the
// limiter. A chain or slot out of range is ignored.
enum {
    JELLO_EFFECT_SLOTS = 8,
    JELLO_FIRST_BUS_CHAIN = 9,
    JELLO_MASTER_CHAIN = 13,
    JELLO_CHAIN_COUNT = 14,
};

// The kinds of effect.
enum {
    JELLO_EFFECT_NONE = 0,
    JELLO_EFFECT_EQ = 1,
    JELLO_EFFECT_FILTER = 2,
    JELLO_EFFECT_SATURATION = 3,
    JELLO_EFFECT_COMPRESSOR = 4,
    JELLO_EFFECT_GATE = 5,
    JELLO_EFFECT_VOLUME_SHAPER = 6,
    JELLO_EFFECT_CHORUS = 7,
    JELLO_EFFECT_DELAY = 8,
    JELLO_EFFECT_REVERB = 9,
};

// Puts a new effect of `kind`, at its starting values, switched on, or
// none, in a slot. A new effect changes the sound little or not at all,
// but chorus, delay and reverb are heard: fully wet on a bus, and at
// JELLO_EFFECT_START_MIX_PERCENT elsewhere.
// The effect it replaces is freed on a later call. Returns false if the
// kind is unknown or the mailbox is full. Never timed.
enum {
    JELLO_EFFECT_START_MIX_PERCENT = 30,
};
bool jello_engine_set_effect(JelloEngine* engine, int chain, int slot, int kind);

// Moves slot `from`'s effect, with everything it remembers, to slot `to`,
// shifting the slots between by one. Returns false if out of range or the
// mailbox is full. Never timed.
bool jello_engine_move_effect(JelloEngine* engine, int chain, int from, int to);

// Switches a slot's effect on or off; off passes the sound untouched. It
// crossfades over 10 ms. An empty slot ignores it.
void jello_engine_set_effect_on(JelloEngine* engine, int chain, int slot, bool on);

// Sets value `value` of a slot's effect to `number`. Each kind's values are
// below. A value the effect doesn't have, a number out of its range, or an
// empty slot is ignored. Numbers glide there over 20 ms; choices and
// switches crossfade over 10 ms.
void jello_engine_set_effect_value(JelloEngine* engine, int chain, int slot, int value, float number);

// How many effects the engine holds, after freeing the replaced ones that
// are ready to go.
int jello_engine_effect_count(JelloEngine* engine);

// How many more messages the audio thread's mailbox holds right now.
int jello_engine_message_room(JelloEngine* engine);

// Acts on every waiting message now, on the calling thread, without moving
// the sound on. Only on an engine whose audio is not running: a render
// otherwise takes them only as it starts.
void jello_engine_take_messages(JelloEngine* engine);

// How many messages were ever lost to a full mailbox.
uint64_t jello_engine_dropped_messages(JelloEngine* engine);

// The sidechain. The compressor, the gate and the filter's follower have a
// "listens to" value: JELLO_LISTEN_TO_OWN for their own input, or a
// chain's number for that chain's sound after its effects, volume and pan.
// A track may listen to the other tracks, a bus to the tracks and the
// other buses, and the master to every track and bus; anything else is
// ignored. A chain hears another from the same samples only if that one
// runs first, so the chains run in an order: `order` holds every track's
// and bus's chain once, `count` of them, which must be
// JELLO_MASTER_CHAIN, every track before every bus; the master runs last.
// A chain listening to one that runs after it hears silence. A new engine
// runs them in number order. Returns false and changes nothing if the
// order is not one of those, or the mailbox is full. Never timed.
enum {
    JELLO_LISTEN_TO_OWN = -1,
};
bool jello_engine_set_chain_order(JelloEngine* engine, const int* order, int count);

// The most a slot's effect, a compressor or a gate, turned the sound down
// in the last chunk, in dB, 0 for not at all or another kind. A gate closed
// to silence reads JELLO_GATE_SILENT_REDUCTION_DB.
float jello_engine_gain_reduction(JelloEngine* engine, int chain, int slot);

// The EQ: JELLO_EQ_BAND_COUNT bands, each with JELLO_EQ_BAND_VALUES values
// numbered band * JELLO_EQ_BAND_VALUES + one of the JELLO_EQ_BAND_ ones,
// then JELLO_EQ_OUTPUT. On: 0 or 1. Shape: one of the shapes below.
// Frequency: JELLO_EQ_LOWEST_HZ to JELLO_EQ_HIGHEST_HZ. Gain, for shelves
// and bells, and the output: -JELLO_EQ_MOST_GAIN_DB to
// JELLO_EQ_MOST_GAIN_DB. Width, the Q, for shelves, bells and notches: 0.1
// to JELLO_EQ_WIDEST_WIDTH. Slope, for cuts: 12, 24 or 48 dB an octave. A
// new EQ has every band off.
enum {
    JELLO_EQ_BAND_COUNT = 8,
    JELLO_EQ_BAND_ON = 0,
    JELLO_EQ_BAND_SHAPE = 1,
    JELLO_EQ_BAND_FREQUENCY = 2,
    JELLO_EQ_BAND_GAIN = 3,
    JELLO_EQ_BAND_WIDTH = 4,
    JELLO_EQ_BAND_SLOPE = 5,
    JELLO_EQ_BAND_VALUES = 6,
    JELLO_EQ_OUTPUT = 48,
    JELLO_EQ_LOWEST_HZ = 20,
    JELLO_EQ_HIGHEST_HZ = 20000,
    JELLO_EQ_MOST_GAIN_DB = 24,
    JELLO_EQ_WIDEST_WIDTH = 18,
};

enum {
    JELLO_EQ_LOW_CUT = 0,
    JELLO_EQ_LOW_SHELF = 1,
    JELLO_EQ_BELL = 2,
    JELLO_EQ_HIGH_SHELF = 3,
    JELLO_EQ_HIGH_CUT = 4,
    JELLO_EQ_NOTCH = 5,
};

// The filter effect. Type: one of the filter types (JELLO_FILTER_LOW_PASS
// and on). Slope: 12 or 24 dB an octave. Cutoff: JELLO_FILTER_LOWEST_HZ to
// JELLO_FILTER_OPEN_HZ. Resonance, drive and mix: 0 to 1. Follow, how far
// full loudness moves the cutoff: -JELLO_FILTER_MOST_OCTAVES to
// JELLO_FILTER_MOST_OCTAVES octaves. Follow attack: 0.1 to
// JELLO_FOLLOW_LONGEST_ATTACK_MS. Follow release:
// JELLO_FOLLOW_SHORTEST_RELEASE_MS to JELLO_FOLLOW_LONGEST_RELEASE_MS. A new
// one is an open low-pass, which changes nothing. Listens to: what the
// follower hears (JELLO_LISTEN_TO_OWN, to start).
enum {
    JELLO_FILTER_EFFECT_TYPE = 0,
    JELLO_FILTER_EFFECT_SLOPE = 1,
    JELLO_FILTER_EFFECT_CUTOFF = 2,
    JELLO_FILTER_EFFECT_RESONANCE = 3,
    JELLO_FILTER_EFFECT_DRIVE = 4,
    JELLO_FILTER_EFFECT_FOLLOW = 5,
    JELLO_FILTER_EFFECT_FOLLOW_ATTACK = 6,
    JELLO_FILTER_EFFECT_FOLLOW_RELEASE = 7,
    JELLO_FILTER_EFFECT_MIX = 8,
    JELLO_FILTER_EFFECT_LISTENS_TO = 9,
    JELLO_FOLLOW_LONGEST_ATTACK_MS = 100,
    JELLO_FOLLOW_SHORTEST_RELEASE_MS = 5,
    JELLO_FOLLOW_LONGEST_RELEASE_MS = 2000,
};

// Saturation. Drive: 0 to JELLO_SATURATION_MOST_DRIVE_DB. Curve: one of the
// curves below. Tone: -1 (darker) to 1 (brighter). Output:
// -JELLO_SATURATION_MOST_OUTPUT_DB to JELLO_SATURATION_MOST_OUTPUT_DB.
// Match: 0, or 1 to keep it as loud as its input instead of the output.
// Mix: 0 to 1. A new one has no drive.
enum {
    JELLO_SATURATION_DRIVE = 0,
    JELLO_SATURATION_CURVE = 1,
    JELLO_SATURATION_TONE = 2,
    JELLO_SATURATION_OUTPUT = 3,
    JELLO_SATURATION_MATCH = 4,
    JELLO_SATURATION_MIX = 5,
    JELLO_SATURATION_MOST_DRIVE_DB = 36,
    JELLO_SATURATION_MOST_OUTPUT_DB = 24,
};

enum {
    JELLO_SATURATION_TAPE = 0,
    JELLO_SATURATION_TUBE = 1,
    JELLO_SATURATION_HARD = 2,
    JELLO_SATURATION_FOLD = 3,
};

// The compressor. Threshold: JELLO_COMPRESSOR_LOWEST_THRESHOLD_DB to 0 dB.
// Ratio: 1 to JELLO_COMPRESSOR_MOST_RATIO, or JELLO_COMPRESSOR_LIMIT, so
// nothing passes the threshold. Attack: 0.1 to
// JELLO_COMPRESSOR_LONGEST_ATTACK_MS. Release:
// JELLO_COMPRESSOR_SHORTEST_RELEASE_MS to
// JELLO_COMPRESSOR_LONGEST_RELEASE_MS. Knee, how gently it starts: 0 to
// JELLO_COMPRESSOR_WIDEST_KNEE_DB. Makeup: 0 to
// JELLO_COMPRESSOR_MOST_MAKEUP_DB. Mix: 0 to 1, under 1 for parallel
// compression. Listens to: see the sidechain. Low cut, on what it listens
// to: JELLO_COMPRESSOR_LOWEST_CUT_HZ to JELLO_COMPRESSOR_HIGHEST_CUT_HZ. A
// new one: 0 dB, 4:1, 10 ms, 100 ms, no knee, no makeup, all wet, its own
// input and 20 Hz, so it changes nothing under 0 dB.
enum {
    JELLO_COMPRESSOR_THRESHOLD = 0,
    JELLO_COMPRESSOR_RATIO = 1,
    JELLO_COMPRESSOR_ATTACK = 2,
    JELLO_COMPRESSOR_RELEASE = 3,
    JELLO_COMPRESSOR_KNEE = 4,
    JELLO_COMPRESSOR_MAKEUP = 5,
    JELLO_COMPRESSOR_MIX = 6,
    JELLO_COMPRESSOR_LISTENS_TO = 7,
    JELLO_COMPRESSOR_LOW_CUT = 8,
    JELLO_COMPRESSOR_LOWEST_THRESHOLD_DB = -60,
    JELLO_COMPRESSOR_MOST_RATIO = 20,
    JELLO_COMPRESSOR_LIMIT = 0,
    JELLO_COMPRESSOR_LONGEST_ATTACK_MS = 100,
    JELLO_COMPRESSOR_SHORTEST_RELEASE_MS = 5,
    JELLO_COMPRESSOR_LONGEST_RELEASE_MS = 2000,
    JELLO_COMPRESSOR_WIDEST_KNEE_DB = 24,
    JELLO_COMPRESSOR_MOST_MAKEUP_DB = 24,
    JELLO_COMPRESSOR_LOWEST_CUT_HZ = 20,
    JELLO_COMPRESSOR_HIGHEST_CUT_HZ = 500,
};

// The gate. Threshold: JELLO_GATE_LOWEST_THRESHOLD_DB to 0 dB; at the
// lowest it never closes. Range, how far down it closes:
// JELLO_GATE_SILENT_RANGE_DB, silent, to 0 dB. Attack: 0.1 to
// JELLO_GATE_LONGEST_ATTACK_MS. Hold: 0 to JELLO_GATE_LONGEST_HOLD_MS.
// Release: JELLO_GATE_SHORTEST_RELEASE_MS to JELLO_GATE_LONGEST_RELEASE_MS.
// Listens to: see the sidechain. A new one: the lowest threshold, silent,
// 1 ms, 20 ms, 100 ms and its own input, so it never closes.
enum {
    JELLO_GATE_THRESHOLD = 0,
    JELLO_GATE_RANGE = 1,
    JELLO_GATE_ATTACK = 2,
    JELLO_GATE_HOLD = 3,
    JELLO_GATE_RELEASE = 4,
    JELLO_GATE_LISTENS_TO = 5,
    JELLO_GATE_LOWEST_THRESHOLD_DB = -80,
    JELLO_GATE_SILENT_RANGE_DB = -80,
    JELLO_GATE_SILENT_REDUCTION_DB = 80,
    JELLO_GATE_LONGEST_ATTACK_MS = 50,
    JELLO_GATE_LONGEST_HOLD_MS = 500,
    JELLO_GATE_SHORTEST_RELEASE_MS = 5,
    JELLO_GATE_LONGEST_RELEASE_MS = 2000,
};

// The volume shaper: a volume curve repeating every cycle, on the beat.
// JELLO_SHAPER_MOST_POINTS points, each with JELLO_SHAPER_POINT_VALUES
// values numbered point * JELLO_SHAPER_POINT_VALUES + one of the
// JELLO_SHAPER_POINT_ ones, then the rest. Place, in the cycle, and level:
// 0 to 1. Bend into the next point: -1 (waits, then moves late) to 1
// (moves early, then settles). The curve goes through the first
// JELLO_SHAPER_POINTS points in order, then round to the first again; a
// place lower than the one before it counts as that one. Cycle: 1/16 to
// JELLO_SHAPER_LONGEST_CYCLE beats. Depth, how much of the curve is heard:
// 0 to 1. Points: 1 to JELLO_SHAPER_MOST_POINTS. A new one is the classic
// pump, down at each beat and back up by half way, at 0 depth.
enum {
    JELLO_SHAPER_MOST_POINTS = 16,
    JELLO_SHAPER_POINT_PLACE = 0,
    JELLO_SHAPER_POINT_LEVEL = 1,
    JELLO_SHAPER_POINT_BEND = 2,
    JELLO_SHAPER_POINT_VALUES = 3,
    JELLO_SHAPER_CYCLE = 48,
    JELLO_SHAPER_DEPTH = 49,
    JELLO_SHAPER_POINTS = 50,
    JELLO_SHAPER_LONGEST_CYCLE = 16,
};

// The chorus: copies of the sound, each a little late, the lateness
// swinging so each copy's pitch wobbles. Copies: 1 to
// JELLO_CHORUS_MOST_COPIES. Speed: 0.01 to JELLO_CHORUS_FASTEST_HZ. Cycle:
// 1/16 to JELLO_CHORUS_LONGEST_CYCLE beats. On beat: 0 swings at the
// speed, 1 once a cycle, on the beat. Depth, how far the lateness swings
// as a share of the delay, up to 90% of it: 0 to 1. Delay:
// JELLO_CHORUS_SHORTEST_DELAY_MS to JELLO_CHORUS_LONGEST_DELAY_MS.
// Feedback: 0 to JELLO_CHORUS_MOST_FEEDBACK_PERCENT / 100. Spread, how
// wide the copies sit: 0 to 1. Mix: 0 to 1. A new one: 2 copies, 0.8 Hz,
// 1 beat, at the speed, 0.25 deep, 7 ms, no feedback, fully spread.
enum {
    JELLO_CHORUS_COPIES = 0,
    JELLO_CHORUS_SPEED = 1,
    JELLO_CHORUS_CYCLE = 2,
    JELLO_CHORUS_ON_BEAT = 3,
    JELLO_CHORUS_DEPTH = 4,
    JELLO_CHORUS_DELAY = 5,
    JELLO_CHORUS_FEEDBACK = 6,
    JELLO_CHORUS_SPREAD = 7,
    JELLO_CHORUS_MIX = 8,
    JELLO_CHORUS_MOST_COPIES = 4,
    JELLO_CHORUS_FASTEST_HZ = 10,
    JELLO_CHORUS_LONGEST_CYCLE = 32,
    JELLO_CHORUS_SHORTEST_DELAY_MS = 1,
    JELLO_CHORUS_LONGEST_DELAY_MS = 40,
    JELLO_CHORUS_MOST_FEEDBACK_PERCENT = 95,
};

// The delay. Left and right time: JELLO_DELAY_SHORTEST_MS to
// JELLO_DELAY_LONGEST_MS, or in beats, 1/16 to JELLO_DELAY_LONGEST_BEATS,
// never longer than JELLO_DELAY_LONGEST_MS. A new time slides there over
// 100 ms like tape, bending the echoes' pitch. On beat: 0 uses the ms, 1
// the beats. Linked: 1 gives the right side the left's time. Ping-pong: 1
// bounces the echoes from side to side, starting on the left. Feedback: 0
// to 1. Low cut and high cut, on every echo: JELLO_DELAY_LOWEST_HZ to
// JELLO_DELAY_HIGHEST_HZ. Wobble, a tape-like swing in the time: 0 to 1, at
// a wobble speed of 0.1 to JELLO_DELAY_FASTEST_WOBBLE_HZ. Grit, saturation
// on the echoes: 0 to 1. Duck, how far the echoes quieten while the dry
// sound plays: 0 to 1. Mix: 0 to 1. A new one: 250 ms or half a beat a
// side, on the beat, linked, straight, 35% feedback, nothing cut, no
// wobble at 0.5 Hz, no grit or duck.
enum {
    JELLO_DELAY_LEFT_MS = 0,
    JELLO_DELAY_RIGHT_MS = 1,
    JELLO_DELAY_LEFT_BEATS = 2,
    JELLO_DELAY_RIGHT_BEATS = 3,
    JELLO_DELAY_ON_BEAT = 4,
    JELLO_DELAY_LINKED = 5,
    JELLO_DELAY_PING_PONG = 6,
    JELLO_DELAY_FEEDBACK = 7,
    JELLO_DELAY_LOW_CUT = 8,
    JELLO_DELAY_HIGH_CUT = 9,
    JELLO_DELAY_WOBBLE = 10,
    JELLO_DELAY_WOBBLE_SPEED = 11,
    JELLO_DELAY_GRIT = 12,
    JELLO_DELAY_DUCK = 13,
    JELLO_DELAY_MIX = 14,
    JELLO_DELAY_SHORTEST_MS = 1,
    JELLO_DELAY_LONGEST_MS = 8000,
    JELLO_DELAY_LONGEST_BEATS = 16,
    JELLO_DELAY_LOWEST_HZ = 20,
    JELLO_DELAY_HIGHEST_HZ = 20000,
    JELLO_DELAY_FASTEST_WOBBLE_HZ = 10,
};

// The reverb: eight delays feeding each other, the usual way to build a
// room. Size: 0 to 1, sliding there over 200 ms. Decay, how long the tail
// takes to fall 60 dB: 0.1 to JELLO_REVERB_LONGEST_DECAY seconds.
// Pre-delay, the gap before the room answers: 0 to
// JELLO_REVERB_LONGEST_PRE_DELAY_MS. Damping, a high cut on the tail:
// JELLO_REVERB_LOWEST_DAMPING_HZ to JELLO_REVERB_HIGHEST_DAMPING_HZ. Low
// cut: JELLO_REVERB_LOWEST_CUT_HZ to JELLO_REVERB_HIGHEST_CUT_HZ.
// Diffusion, how fast the echoes smear into a wash, modulation, a slow
// wobble in the tail, and width: 0 to 1. Freeze: 1 holds the tail forever
// and lets nothing new in. Mix: 0 to 1. A new one: half size, 1.5 s, no
// pre-delay, 8 kHz damping, nothing cut, 0.7 diffusion, 0.3 modulation,
// full width, not frozen.
enum {
    JELLO_REVERB_SIZE = 0,
    JELLO_REVERB_DECAY = 1,
    JELLO_REVERB_PRE_DELAY = 2,
    JELLO_REVERB_DAMPING = 3,
    JELLO_REVERB_LOW_CUT = 4,
    JELLO_REVERB_DIFFUSION = 5,
    JELLO_REVERB_MODULATION = 6,
    JELLO_REVERB_WIDTH = 7,
    JELLO_REVERB_FREEZE = 8,
    JELLO_REVERB_MIX = 9,
    JELLO_REVERB_LONGEST_DECAY = 30,
    JELLO_REVERB_LONGEST_PRE_DELAY_MS = 500,
    JELLO_REVERB_LOWEST_DAMPING_HZ = 1000,
    JELLO_REVERB_HIGHEST_DAMPING_HZ = 20000,
    JELLO_REVERB_LOWEST_CUT_HZ = 20,
    JELLO_REVERB_HIGHEST_CUT_HZ = 1000,
};

// Oscillators (docs/adr/0035-oscillators-with-kinds.md): every note is
// played by JELLO_OSCILLATOR_COUNT of them at once, counted from 0, added up
// before the filter. Each is tuned from the note by up to
// JELLO_OSCILLATOR_MOST_OCTAVES octaves, JELLO_OSCILLATOR_MOST_HALF_STEPS
// half steps and JELLO_OSCILLATOR_MOST_CENTS cents, either way.
enum {
    JELLO_OSCILLATOR_COUNT = 3,
    JELLO_OSCILLATOR_MOST_OCTAVES = 3,
    JELLO_OSCILLATOR_MOST_HALF_STEPS = 12,
    JELLO_OSCILLATOR_MOST_CENTS = 100,
};

// What an oscillator can play: the four waves, noise, a new random height
// every sample, which has no pitch, and a sampled instrument's recordings,
// set with jello_engine_set_instrument (silent until one is).
enum {
    JELLO_KIND_SINE = 0,
    JELLO_KIND_TRIANGLE = 1,
    JELLO_KIND_SQUARE = 2,
    JELLO_KIND_SAW = 3,
    JELLO_KIND_NOISE = 4,
    JELLO_KIND_SAMPLE = 5,
};

// How much of each cycle a square can spend up (docs/design.md#pulse-width),
// in percent: 50 is the square, and narrower or wider a thinner pulse.
enum {
    JELLO_PULSE_NARROWEST_PERCENT = 5,
    JELLO_PULSE_WIDEST_PERCENT = 95,
};

// Makes notes sent after this play oscillator `index` as `kind`, one of the
// JELLO_KIND_ numbers above, at `level`, from 0 (silent) to 1, tuned
// `octaves`, `half_steps` and `cents` from the note. A square spends `width`
// of each cycle up, a share from JELLO_PULSE_NARROWEST_PERCENT to
// JELLO_PULSE_WIDEST_PERCENT hundredths; the other kinds leave it be. Notes
// sent before it keep theirs, even while they ring on. An index or any
// value outside the ranges above is ignored. A sample's `touch`, from
// JELLO_TOUCH_SOFTEST to JELLO_TOUCH_HARDEST, says how hard its keys are
// struck: which of its recordings play. A new engine plays oscillator 0 as
// JELLO_KIND_TRIANGLE at level 1, and the others as triangles at level 0, all
// tuned right on the note, at a width of 0.5 and a touch of 80.
enum {
    JELLO_TOUCH_SOFTEST = 1,
    JELLO_TOUCH_HARDEST = 127,
};
void jello_engine_set_oscillator(JelloEngine* engine, int track, int index, int kind, float level, int octaves,
                                 int half_steps, float cents, float width, int touch);

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
// dropped. Each layer belongs to `track`, the track of the press that started
// it: it records only that track's notes and plays them in its sound. A track
// holds at most JELLO_TRACK_MAX_LAYERS layers, and past that the first of the
// pair does nothing.
void jello_engine_press_looper(JelloEngine* engine, int track);

// Stops every layer's notes and forgets the loop, or cancels the count-in or
// the recording. Live notes play on.
void jello_engine_clear_loop(JelloEngine* engine);

// The most layers a loop holds, on all its tracks, and the number for the
// layer recorded last.
enum {
    JELLO_LOOP_MAX_LAYERS = 72,
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

// Play and pause in the lane (docs/adr/0038-song-files.md). Pausing stops
// every layer's notes and the loop waits, silent, at its start. A layer
// being recorded stops first, as the second press would. Playing starts it
// from beat 1 at once, and Right Shift while paused does the same. Both do
// nothing unless the loop is playing or paused, as they say. The tempo
// stays locked while paused.
void jello_engine_pause_loop(JelloEngine* engine);
void jello_engine_play_loop(JelloEngine* engine);

// Pauses layer `layer` (counted as above) or plays it again. A paused layer
// is silent while the others play on, and keeps its volume. Played again
// while the loop plays, the notes that should be sounding start at once.
// Each layer starts out playing. A layer that doesn't exist changes nothing,
// and any other number is ignored.
void jello_engine_set_layer_playing(JelloEngine* engine, int layer, bool playing);

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
// mailbox has no room for all of it. It plays on track `track`, and false
// is returned for a track out of range too.
bool jello_engine_play_song(JelloEngine* engine, int track, const JelloSongChord* chords, int count);

// Lets go of what the song is playing and forgets it. Live notes and the
// loop play on.
void jello_engine_stop_song(JelloEngine* engine);

// The filter's ranges (docs/adr/0028-a-low-pass-filter.md): the lowest
// cutoff and the highest, in Hz, the most octaves the filter envelope opens
// it, and the longest any of its stages takes.
enum {
    JELLO_FILTER_LOWEST_HZ = 20,
    JELLO_FILTER_OPEN_HZ = 20000,
    JELLO_FILTER_MOST_OCTAVES = 8,
    JELLO_FILTER_LONGEST_MS = 10000,
};

// The filter's types (docs/design.md#filter-types), each 12 dB per octave
// away from its cutoff. A low-pass lets low sounds through and turns high
// ones down; a high-pass the other way round. A band-pass lets only the
// sounds near the cutoff through, and a notch all but those.
enum {
    JELLO_FILTER_LOW_PASS = 0,
    JELLO_FILTER_HIGH_PASS = 1,
    JELLO_FILTER_BAND_PASS = 2,
    JELLO_FILTER_NOTCH = 3,
};

// Every note goes through a filter of its own, of `type`, one of the
// JELLO_FILTER_ types above. `cutoff_hz` is middle C's cutoff, from
// JELLO_FILTER_LOWEST_HZ to JELLO_FILTER_OPEN_HZ, and every other note's
// follows it, an octave higher for each octave up. A low-pass at
// JELLO_FILTER_OPEN_HZ, or a high-pass at JELLO_FILTER_LOWEST_HZ, is open:
// off, so notes sound exactly as without it. `resonance`, from 0 to 1,
// boosts the sound right at the cutoff. Every note hears a change, even
// those already sounding, each gliding there over 20 ms so it never clicks,
// a change of type too. Values outside those ranges are ignored. A new
// engine starts as an open low-pass, with no resonance.
void jello_engine_set_filter(JelloEngine* engine, int track, int type, float cutoff_hz, float resonance);

// The filter envelope: when a note starts, it opens the cutoff by up to
// `amount` octaves, from 0 to JELLO_FILTER_MOST_OCTAVES. It rises over
// `attack_ms`, falls over `decay_ms` to `sustain` (a share of the amount,
// from 0 to 1) while the key is held, and closes over `release_ms` once it
// is let go; each from 0 to JELLO_FILTER_LONGEST_MS. Notes started after
// this use it, and notes started before keep theirs. Values outside those
// ranges are ignored. A new engine starts at an amount of 0, which changes
// nothing, with an attack of 5 ms, a decay of 300 ms, a sustain of 0 and a
// release of 300 ms.
void jello_engine_set_filter_envelope(JelloEngine* engine, int track, float amount, int attack_ms, int decay_ms,
                                      float sustain, int release_ms);

// Unison's ranges (docs/adr/0029-unison.md): the most copies a note can
// have, and the most cents the highest is tuned above the note.
enum {
    JELLO_UNISON_MOST_COPIES = 7,
    JELLO_UNISON_MOST_DETUNE_CENTS = 50,
};

// Unison: every note is played by `copies` copies of its wave at once, from
// 1 to JELLO_UNISON_MOST_COPIES. The highest copy is tuned `detune_cents`
// above the note (a cent is a hundredth of a half step) and the lowest as
// far below, from 0 to JELLO_UNISON_MOST_DETUNE_CENTS, with the others
// evenly between. `spread`, from 0 to 1, places them between the speakers:
// at 0 all in the middle, at 1 the lowest fully left and the highest fully
// right. Their sum is turned down by the square root of `copies`, so a note
// stays about as loud. Notes started after this use it, and notes started
// before keep theirs. Values outside those ranges are ignored. A new engine
// starts at 1 copy, which is off: notes sound exactly as without unison.
// Its detune starts at 20 cents and its spread at 1.
void jello_engine_set_unison(JelloEngine* engine, int track, int copies, float detune_cents, float spread);

// The shortest and longest any stage of the loudness envelope takes, in ms.
// At 0 ms a stage would jump the loudness in one sample, which clicks.
enum {
    JELLO_LOUDNESS_SHORTEST_MS = 1,
    JELLO_LOUDNESS_LONGEST_MS = 10000,
};

// The loudness envelope: when a note starts, its loudness rises over
// `attack_ms`, falls over `decay_ms` to `sustain` (a share of the peak, from
// 0 to 1) while the key is held, and fades out over `release_ms` once it is
// let go; each stage from JELLO_LOUDNESS_SHORTEST_MS to
// JELLO_LOUDNESS_LONGEST_MS. Notes started after this use it, and notes
// started before keep theirs. Values outside those ranges are ignored. A new
// engine starts with an attack of 5 ms, a decay of 300 ms, a sustain of 1
// and a release of 300 ms.
void jello_engine_set_loudness_envelope(JelloEngine* engine, int track, int attack_ms, int decay_ms, float sustain,
                                        int release_ms);

// Key tracking: how much each note's filter cutoff follows middle C's, from
// 0 to 1. At 1, a note's cutoff is an octave higher for each octave it is
// above middle C, so every note is equally bright. At 0, every note has
// middle C's cutoff, so higher notes sound darker. Notes started after this
// use it, and notes started before keep theirs. A value outside 0 to 1 is
// ignored. A new engine starts at 1.
void jello_engine_set_key_tracking(JelloEngine* engine, int track, float key_tracking);

// The lowest and highest A4 notes can be tuned to, in Hz: about a half step
// below and above 440.
enum {
    JELLO_TUNING_LOWEST_HZ = 415,
    JELLO_TUNING_HIGHEST_HZ = 466,
};

// Tunes notes from A4 at `a4_hz`, from JELLO_TUNING_LOWEST_HZ to
// JELLO_TUNING_HIGHEST_HZ: every other note keeps its distance from A4.
// Notes started after this use it, and notes started before keep their
// pitch. A value outside that range is ignored. A new engine starts at 440.
void jello_engine_set_tuning(JelloEngine* engine, int track, float a4_hz);

// LFOs (docs/adr/0031-lfos-and-a-routing-list.md): low frequency
// oscillators, slow waves that move settings up and down while notes play,
// as the routing list below connects them. Each track has JELLO_LFO_COUNT,
// counted from 0, and JELLO_CONNECTION_COUNT connections, which move its
// sound and its effects. Each bus and the master has as many of its own,
// which only move its effects: jello_engine_set_lfo and
// jello_engine_set_connection take its chain number, from
// JELLO_FIRST_BUS_CHAIN to JELLO_MASTER_CHAIN, for `track`.
enum {
    JELLO_LFO_COUNT = 8,
    JELLO_LFO_MOST_STEPS = 32,
    JELLO_LFO_MOST_PRESSES = 32,
    JELLO_LFO_LONGEST_MS = 10000,
    JELLO_LFO_LONGEST_DELAY_BEATS = 128,
    JELLO_CONNECTION_COUNT = 64,
    JELLO_VIBRATO_KEY_LONGEST_MS = 10000,
};

// An LFO's shapes, each from -1 to 1.
enum {
    // Swings smoothly up to 1, down to -1, then back to 0.
    JELLO_LFO_SINE = 0,
    // The same in straight lines.
    JELLO_LFO_TRIANGLE = 1,
    // 1 for the first half of each cycle, -1 for the second.
    JELLO_LFO_SQUARE = 2,
    // Rises from -1 to 1, then jumps back down.
    JELLO_LFO_SAW_UP = 3,
    // Falls from 1 to -1, then jumps back up.
    JELLO_LFO_SAW_DOWN = 4,
    // A new random height each cycle, held.
    JELLO_LFO_RANDOM_STEPS = 5,
    // Glides from one random height to the next each cycle.
    JELLO_LFO_SMOOTH_RANDOM = 6,
    // The heights in `steps`, one after another, each held.
    JELLO_LFO_STEPS = 7,
};

// What an LFO's speed is counted in.
enum {
    // Cycles a second, from 0.01 to 100.
    JELLO_LFO_HZ = 0,
    // How many beats one cycle lasts at the tempo, from 1/16 to 128.
    JELLO_LFO_BEATS = 1,
    // How many key presses one cycle takes, from 1 to
    // JELLO_LFO_MOST_PRESSES. Notes that start together, like a chord's,
    // are one press.
    JELLO_LFO_PRESSES = 2,
};

// One LFO's settings.
typedef struct JelloLfo {
    // One of the JELLO_LFO_ shapes above.
    int shape;
    // For JELLO_LFO_STEPS: how many steps, from 1 to JELLO_LFO_MOST_STEPS,
    // and each one's height, from -1 to 1. Only the first `step_count`
    // count; the rest are ignored.
    int step_count;
    float steps[JELLO_LFO_MOST_STEPS];
    // True: every note starts its own copy of the LFO. False: one LFO runs
    // for every note, from the engine's start in Hz, on the beat in beats
    // (each time the beat starts, it starts again at `start`), and for each
    // player of notes in presses.
    bool per_note;
    // One of the JELLO_LFO_ speeds above, and the speed in each.
    int speed;
    double hz;
    double beats;
    int presses;
    // Where in its cycle it starts, from 0 to 1. And for a per-note LFO, how
    // far each note's start is moved at random on top, from 0 to 1.
    double start;
    double spread;
    // How long after each note starts the LFO waits before it moves the
    // note, then how long it takes to grow in a straight line to its full
    // size: from 0 to JELLO_LFO_LONGEST_MS, or in beats while the speed is in
    // beats, from 0 to JELLO_LFO_LONGEST_DELAY_BEATS.
    int delay_ms;
    int fade_ms;
    double delay_beats;
    double fade_beats;
    // How much of the time between jumps is spent sliding instead, from 0
    // to 1. In presses, it slides over that share of a beat.
    double smooth;
    // For a per-note LFO: one cycle, then it holds where it ended.
    bool once;
} JelloLfo;

// Sets LFO `index`, from 0 to JELLO_LFO_COUNT - 1. Every note hears it from
// the next 32 samples on. A shared LFO glides to where the change puts it
// over 20 ms, so it never clicks. An index or any setting outside the
// ranges above is ignored. A new engine's LFOs are sine waves at 5.5 Hz,
// per note, starting at 0 with a spread of 1, with no delay or fade-in, a
// smoothing of 0.1, 8 steps of 0, a beat and 8 presses per cycle, and not
// once.
void jello_engine_set_lfo(JelloEngine* engine, int track, int index, const JelloLfo* lfo);

// What moves a target. Read both ways, each goes from -1 to 1; read one way,
// from 0 to 1. LFO number n (from 0) is JELLO_SOURCE_LFO_1 + n.
enum {
    // An unused connection.
    JELLO_SOURCE_NONE = 0,
    JELLO_SOURCE_LFO_1 = 1,
    // 0 while up, 1 while held, rising and falling in straight lines.
    JELLO_SOURCE_VIBRATO_KEY = 9,
    // The note's height: -1 five octaves below middle C, 1 five above.
    JELLO_SOURCE_NOTE_HEIGHT = 10,
    // A random number each note starts with.
    JELLO_SOURCE_RANDOM = 11,
    // Where the note's loudness envelope is, as a share of its peak, and
    // where its filter envelope is.
    JELLO_SOURCE_LOUDNESS_ENVELOPE = 12,
    JELLO_SOURCE_FILTER_ENVELOPE = 13,
};

// What a connection moves, each in its own units, up to the amount given
// either way. The ones before JELLO_TARGET_OSCILLATOR_1_KIND move live,
// while a note sounds; the rest up to JELLO_TARGET_GLIDE_TIME are read once
// as each note starts; JELLO_TARGET_EFFECT moves an effect. LFO
// number n's speed (from 0) is JELLO_TARGET_LFO_1_SPEED + n, and oscillator
// number n's level, pitch, width and kind are
// JELLO_TARGET_OSCILLATOR_1_LEVEL + n and so on. Connections to one target add up, and the sum is kept within
// the target's own range.
enum {
    // Cents, up to 4800.
    JELLO_TARGET_PITCH = 0,
    // A share, up to 1: 1 moves it all the way to silence. Never louder
    // than as played.
    JELLO_TARGET_LOUDNESS = 1,
    // A share, up to 1: 1 moves it all the way right.
    JELLO_TARGET_PAN = 2,
    // Octaves, up to 10. Any connection to it brings the filter in, even
    // when it is open.
    JELLO_TARGET_CUTOFF = 3,
    // Shares, up to 1.
    JELLO_TARGET_RESONANCE = 4,
    JELLO_TARGET_KEY_TRACKING = 5,
    // Cents, up to 50.
    JELLO_TARGET_DETUNE = 6,
    // Shares, up to 1.
    JELLO_TARGET_SPREAD = 7,
    JELLO_TARGET_LOUDNESS_SUSTAIN = 8,
    // Octaves, up to 8.
    JELLO_TARGET_FILTER_AMOUNT = 9,
    // A share, up to 1.
    JELLO_TARGET_FILTER_SUSTAIN = 10,
    // Whole steps through the JELLO_FILTER_ types, and round again, up to
    // 3. Like the type itself, it glides to the next one over 20 ms.
    JELLO_TARGET_FILTER_TYPE = 11,
    // Octaves, up to 14: 1 is twice as fast. A shared LFO's speed is only
    // moved by shared LFOs, in Hz or beats.
    JELLO_TARGET_LFO_1_SPEED = 12,
    // A share, up to 1: 1 moves it from silent to full.
    JELLO_TARGET_OSCILLATOR_1_LEVEL = 20,
    // Cents, up to 4800.
    JELLO_TARGET_OSCILLATOR_1_PITCH = 23,
    // A share of the cycle, up to 0.9: from the narrowest pulse to the
    // widest. Only a square hears it.
    JELLO_TARGET_OSCILLATOR_1_WIDTH = 26,
    // Whole steps through the JELLO_KIND_ kinds, and round again, up to 4.
    JELLO_TARGET_OSCILLATOR_1_KIND = 29,
    // Whole copies, up to 6.
    JELLO_TARGET_COPIES = 32,
    // Octaves of time, up to 14: 1 is twice as long.
    JELLO_TARGET_LOUDNESS_ATTACK = 33,
    JELLO_TARGET_LOUDNESS_DECAY = 34,
    JELLO_TARGET_LOUDNESS_RELEASE = 35,
    JELLO_TARGET_FILTER_ATTACK = 36,
    JELLO_TARGET_FILTER_DECAY = 37,
    JELLO_TARGET_FILTER_RELEASE = 38,
    // Octaves of time, up to 14, as each melody note starts to glide.
    JELLO_TARGET_GLIDE_TIME = 39,
    // Value `effect_value` of the effect in slot `effect_slot` of the
    // connection's chain, in that value's own units, up to
    // JELLO_EFFECT_MOST_MOVE. Only LFOs that run for every note, in Hz or
    // beats, move it, and only they scale it; any other source leaves it
    // be. The move is added to what the value is set to and kept within its
    // range, a whole number's rounded. "Listens to", a cut's slope, and a
    // compressor at its limit never move. It glides like any new value.
    JELLO_TARGET_EFFECT = 40,
};

// How many values an effect can have, numbered from 0, and the most a
// connection moves one either way.
enum {
    JELLO_EFFECT_MOST_VALUES = 64,
    JELLO_EFFECT_MOST_MOVE = 20000,
};

// One connection of the routing list: `source` moves `target` by `amount`,
// in the target's own units.
typedef struct JelloConnection {
    // One of the JELLO_SOURCE_ numbers above.
    int source;
    // One of the JELLO_TARGET_ numbers above.
    int target;
    // From minus to plus the most the target allows.
    float amount;
    // Both ways, the target moves from -amount to amount; one way, from 0
    // to amount.
    bool one_way;
    // For JELLO_TARGET_PITCH and the oscillators' pitches only: the move is
    // rounded to whole half steps.
    bool snap;
    // Another JELLO_SOURCE_ the amount is multiplied by, read one way, from
    // 0 (no move) to 1 (the full amount). JELLO_SOURCE_NONE leaves it full.
    int scaled_by;
    // For JELLO_TARGET_EFFECT only: the slot, from 0 to
    // JELLO_EFFECT_SLOTS - 1, and which value of its effect, from 0 to
    // JELLO_EFFECT_MOST_VALUES - 1. A value the effect there doesn't have
    // moves nothing.
    int effect_slot;
    int effect_value;
} JelloConnection;

// Sets connection `index`, from 0 to JELLO_CONNECTION_COUNT - 1. Every note
// hears it from the next 32 samples on, except in the settings read as a
// note starts. An index, source, target or amount outside the ranges above
// is ignored. A new engine's connections all have no source, so nothing
// moves.
void jello_engine_set_connection(JelloEngine* engine, int track, int index, const JelloConnection* connection);

// How long the vibrato key takes to rise from 0 to 1 when held, and to fall
// back when let go, each from 0 (at once) to JELLO_VIBRATO_KEY_LONGEST_MS.
// Values outside that range are ignored. A new engine starts at 0 and 0.
void jello_engine_set_vibrato_key_times(JelloEngine* engine, int track, int rise_ms, int fall_ms);

// Holds or lets go of the vibrato key (docs/adr/0032-the-vibrato-key.md). It
// only moves live notes, as far as the routing list connects it. The looper
// records it with a layer's notes and plays it back, as that layer's own.
// Letting go of every note lets go of it too.
void jello_engine_set_vibrato_key(JelloEngine* engine, int track, bool held);

// Holds or lets go of the sustain pedal (docs/adr/0048-the-grand-piano.md).
// While it is held, the track's notes ring on when their keys come up, until
// it is let go. The looper records it with a layer's notes and plays it back,
// as that layer's own. Letting go of every note lets go of it too.
void jello_engine_set_pedal_key(JelloEngine* engine, int track, bool held);

// The longest a glide takes, in ms (docs/adr/0036-glide.md).
enum {
    JELLO_GLIDE_LONGEST_MS = 10000,
};

// Holds or lets go of the glide key (docs/adr/0036-glide.md). While it is
// held, each melody note, a note started on its own, slides in from the
// last melody note's pitch. If that key is still held, the note already
// sounding bends to the new one instead, as one note, and letting go of the
// new key bends it back. Notes started together, a chord, never glide. It
// only glides live notes. The looper records it with a layer's notes and
// plays it back, as that layer's own. Letting go of every note lets go of
// it too.
void jello_engine_set_glide_key(JelloEngine* engine, int track, bool held);

// How long a glide takes for notes started after this, from 0 (a jump) to
// JELLO_GLIDE_LONGEST_MS, in a straight line through the half steps. A
// value outside that range is ignored. A new engine starts at 100.
void jello_engine_set_glide_time(JelloEngine* engine, int track, int glide_ms);

// The sequencer (docs/design.md#tracks-in-the-engine) plays the
// arrangement: timed messages, each due on its own sample. Between
// jello_engine_time_next and jello_engine_send_now, the note functions above,
// jello_engine_set_track_volume, the mix functions from
// jello_engine_set_track_pan to jello_engine_set_limiter,
// jello_engine_set_effect_on, jello_engine_set_effect_value,
// jello_engine_set_tempo, every sound setting
// from jello_engine_set_oscillator to jello_engine_set_glide_time,
// jello_engine_set_drum_pad and the three layer functions below leave their
// message for the sequencer instead, due `sample` samples after the
// sequence starts. The others are never timed. Keep it a few seconds ahead
// of the sound: 8,192 messages fit, and once they are full new ones are
// dropped.
void jello_engine_time_next(JelloEngine* engine, uint64_t sample);
void jello_engine_send_now(JelloEngine* engine);

// Starts the sequence the timed messages since the last stop belong to,
// from the start of the next chunk. A message that comes late plays at once.
// Stopping lets go of every note the sequence's layers play and drops its
// messages still waiting, and the timed messages sent after a stop belong to
// the next sequence. Stop a sequence before playing the next.
void jello_engine_play_sequence(JelloEngine* engine);
void jello_engine_stop_sequence(JelloEngine* engine);

// How many samples the sequence has played, as of the last chunk, or -1
// when none plays. A play on its way counts as 0, and a stop as -1.
int64_t jello_engine_sequence_position(JelloEngine* engine);

// How many more timed messages fit right now.
int jello_engine_sequence_room(JelloEngine* engine);

// A layer's notes and volume, played straight on track `track`'s layer slot
// `slot`, from 0 to JELLO_TRACK_MAX_LAYERS - 1, like the looper plays them.
// The sequencer plays the arrangement's layers this way, while the loop is
// cleared, since the looper plays on the same slots. A note may be
// JELLO_VIBRATO_KEY_NOTE, JELLO_GLIDE_KEY_NOTE or a drum pad's. Anything out
// of range is ignored.
void jello_engine_layer_note_on(JelloEngine* engine, int track, int slot, int note, int velocity);
void jello_engine_layer_note_off(JelloEngine* engine, int track, int slot, int note);
void jello_engine_set_slot_volume(JelloEngine* engine, int track, int slot, float volume);

// What the looper can be doing.
enum {
    JELLO_LOOPER_IDLE = 0,
    JELLO_LOOPER_COUNTING_IN = 1,
    JELLO_LOOPER_RECORDING = 2,
    JELLO_LOOPER_LOOPING = 3,
    // Playing the loop and recording a layer over it, with the click on.
    JELLO_LOOPER_LAYERING = 4,
    // Holding a loop without playing it, until play or Right Shift.
    JELLO_LOOPER_PAUSED = 5,
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
    // A press of the vibrato key is a note of pitch JELLO_VIBRATO_KEY_NOTE,
    // and of the glide key, JELLO_GLIDE_KEY_NOTE.
    JELLO_LOOP_STARTED = 1,
    // Note `index` of layer `layer` ended on `sample`.
    JELLO_LOOP_ENDED = 2,
    // The first loop closed, and layer 0 kept only its first `index` notes.
    JELLO_LOOP_KEPT = 3,
    // Layer `layer` started recording on track `track`, after the others.
    // The first press starts layer 0, for the first loop.
    JELLO_LOOP_LAYER_STARTED = 4,
    // Layer `layer` stopped recording.
    JELLO_LOOP_LAYER_STOPPED = 5,
    // Layer `layer` stopped with no notes, so it is gone.
    JELLO_LOOP_LAYER_DROPPED = 6,
    // Layer `layer` was removed. The layers after it move up.
    JELLO_LOOP_LAYER_REMOVED = 7,
    // A loop of `layer` layers was loaded (jello_engine_load_loop), every
    // layer playing at full volume. The forgotten change comes first.
    JELLO_LOOP_LOADED = 8,
};

typedef struct JelloLoopChange {
    // One of the JELLO_LOOP_ numbers above.
    int type;
    // Which layer.
    int layer;
    // Which note of the layer. For JELLO_LOOP_KEPT, how many notes it keeps.
    int index;
    // Its pitch, 0 to 127, JELLO_VIBRATO_KEY_NOTE, JELLO_GLIDE_KEY_NOTE or
    // a drum pad's; only for JELLO_LOOP_STARTED.
    int note;
    // Counted from the loop's start.
    uint64_t sample;
    // The layer's track; only for JELLO_LOOP_LAYER_STARTED.
    int track;
} JelloLoopChange;

// Takes out the oldest change to the loop's layers and notes into `change`
// and returns true, or returns false if none is waiting. Applying every
// change in order keeps a copy of the loop's layers and notes. Call it from the same thread as the
// functions above, and often: once 16,384 changes are waiting, newer ones are
// dropped, since the sound never waits for the screen.
bool jello_engine_next_loop_change(JelloEngine* engine, JelloLoopChange* change);

// The most notes a loop holds, in all its layers, and the pitches its notes
// have for presses of the vibrato key, one past the highest note, and of
// the glide key, one past that. Past the most, new notes still sound live
// but are not recorded.
enum {
    JELLO_LOOP_MAX_NOTES = 16384,
    JELLO_VIBRATO_KEY_NOTE = 128,
    JELLO_GLIDE_KEY_NOTE = 129,
    JELLO_PEDAL_KEY_NOTE = 146,
};

// One note of a loop, for jello_engine_plan_loop.
typedef struct JelloLoopNote {
    // Its pitch, 0 to 127, or one of the vibrato key's, the glide key's
    // and the drum pads' after.
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

// One note of a loop opened from a song file, for jello_engine_load_loop.
typedef struct JelloLoadedNote {
    // Which layer it belongs to, from 0, and that layer's track.
    int layer;
    int track;
    // Its pitch, 0 to 127, JELLO_VIBRATO_KEY_NOTE, JELLO_GLIDE_KEY_NOTE or
    // a drum pad's.
    int note;
    // How hard it was hit, 1 to 127.
    int velocity;
    // The samples it starts and ends on, counted from the loop's start.
    uint64_t start;
    uint64_t end;
} JelloLoadedNote;

// Opens a loop from a song file (docs/adr/0038-song-files.md): forgets the
// loop there is, as jello_engine_clear_loop does, and takes `count` notes in
// `layers` layers, `bars` bars long at the tempo asked for last, paused.
// The notes come layer by layer, 0 first, each layer's in the order they
// start, and every layer has at least one, all on its track. Notes past the
// loop's end are kept to it. Each layer plays at full volume until told
// otherwise. Returns false and changes nothing if `layers` is below 1 or
// above JELLO_LOOP_MAX_LAYERS, `count` is below 1 or above
// JELLO_LOOP_MAX_NOTES, `bars` is below 1, a note is out of order or out of
// range, a layer's notes are on different tracks, a track has more than
// JELLO_TRACK_MAX_LAYERS layers, or too many messages are waiting. Call it from the same thread as the functions above.
bool jello_engine_load_loop(JelloEngine* engine, const JelloLoadedNote* notes, int count, int layers, int bars);

// How many samples the song has played, counted from its first chord's
// start, as of the last chunk of sound, or -1 when no song is playing. A
// play still on its way counts as 0, and a stop on its way as -1. When the
// song has played to its very end, its last chord lets go on the next
// sample. Call it from the same thread as the functions above.
int64_t jello_engine_song_position(JelloEngine* engine);

// How many samples per second the engine makes (48000).
int jello_engine_sample_rate(JelloEngine* engine);

// The microphone (docs/design.md#voice). Its own device opens only while
// listening, mono at jello_engine_sample_rate, and what it hears waits in a
// ring of about 21 seconds until read. If nothing reads for that long, the
// newest sound is dropped and counted. Call these from the same thread as
// the functions above, except jello_engine_read_capture,
// jello_engine_capture_peak and jello_engine_capture_dropped, which one other
// thread may call while listening.

// Asks the system which mics there are and returns how many. Their numbers
// count from 0 until the next ask. While listening, it keeps the list it has.
int jello_engine_list_inputs(JelloEngine* engine);

// Copies mic `index`'s name, from the last jello_engine_list_inputs, into
// `name`, which holds `length` bytes, cut short if it must, always ending in
// a 0 byte. Returns false and copies nothing if there is no such mic.
bool jello_engine_input_name(JelloEngine* engine, int index, char* name, int length);

// The mic number for whichever mic the system picks.
#define JELLO_DEFAULT_INPUT -1

// Starts listening to mic `input`, a number from jello_engine_list_inputs or
// JELLO_DEFAULT_INPUT, forgetting anything heard before. Returns true if it
// listens, or was already. Returns false if there is no such mic, or it
// can't be opened: none plugged in, or the player said no to Jello using it.
bool jello_engine_start_capture(JelloEngine* engine, int input);

// Stops listening and closes the mic. What was heard and not read yet can
// still be read.
void jello_engine_stop_capture(JelloEngine* engine);

// Takes up to `most` of the oldest samples heard into `out` and returns how
// many it took.
int jello_engine_read_capture(JelloEngine* engine, float* out, int most);

// The loudest sample heard, from 0 to about 1, since the last time this was
// asked, for a level meter.
float jello_engine_capture_peak(JelloEngine* engine);

// How many samples were dropped since listening started, for want of room.
uint64_t jello_engine_capture_dropped(JelloEngine* engine);

// For tests, with no mic: puts `frames` samples in as if the mic heard them.
// Does nothing while listening.
void jello_engine_feed_capture(JelloEngine* engine, const float* samples, int frames);

// Sampled instruments and the drum kit (docs/design.md#sampler).
//
// A sample set is one instrument's recordings: zones, each a recording
// played for a range of notes. The engine keeps its own copy of the
// recordings, so the caller can let go of theirs once the set is added.

// The most sample sets an engine keeps at once.
enum {
    JELLO_SAMPLE_SET_MOST = 32,
};

// When a zone plays: as a note starts, as its key comes up, or as the pedal
// goes down or up.
enum {
    JELLO_TRIGGER_ATTACK = 0,
    JELLO_TRIGGER_RELEASE = 1,
    JELLO_TRIGGER_PEDAL_DOWN = 2,
    JELLO_TRIGGER_PEDAL_UP = 3,
};

// One zone of a sample set.
typedef struct JelloSampleZone {
    // The notes it plays, 0 to 127; for a drum kit, the pads it plays, 0
    // to JELLO_DRUM_PAD_COUNT - 1, one pad to a zone.
    int lowest_note;
    int highest_note;
    // The note it was recorded at, and how far off that it is, in cents.
    int root_note;
    float tune_cents;
    // 1 (mono) or 2 (stereo, left and right taking turns), at 48,000
    // frames a second.
    int channels;
    const float* frames;
    uint64_t frame_count;
    // Where its loop starts and ends, in frames; a zone whose loop ends
    // where it starts plays once, to its end.
    uint64_t loop_start;
    uint64_t loop_end;
    // The touches it was recorded for, both ends included, and when it plays.
    int lowest_touch;
    int highest_touch;
    int trigger;
    // For a release: how many decibels quieter it starts for each second the
    // note sounded, and the share of its loudness a soft touch takes away,
    // from 0 to 1.
    float fade_db_per_second;
    float touch_share;
} JelloSampleZone;

// Makes a sample set from `count` zones, copying their recordings, and
// returns its number, or -1 if a zone is odd or the engine already keeps
// JELLO_SAMPLE_SET_MOST sets.
int jello_engine_add_sample_set(JelloEngine* engine, const JelloSampleZone* zones, int count);

// Lets go of set `set`: it can no longer be sent anywhere, and its memory
// is freed once nothing plays it any more, on a later call to one of these
// functions or when the engine is destroyed.
void jello_engine_forget_sample_set(JelloEngine* engine, int set);

// Frees the memory of every forgotten set nothing plays any more.
void jello_engine_tidy_sample_sets(JelloEngine* engine);

// How many sets the engine still keeps, forgotten ones not yet freed too,
// for tests.
int jello_engine_sample_sets_kept(JelloEngine* engine);

// Makes notes sent after this play oscillator `index`'s JELLO_KIND_SAMPLE
// from set `set`, or from nothing for -1. Returns false, changing nothing,
// for an odd index, a set that is not kept or is forgotten, or a full
// mailbox, and for a track out of range.
bool jello_engine_set_instrument(JelloEngine* engine, int track, int index, int set);

// Makes drum hits after this play set `set`, its zones one per pad, or
// nothing for -1. Returns false as jello_engine_set_instrument does.
bool jello_engine_set_drum_kit(JelloEngine* engine, int track, int set);

// The drum pads, played with jello_engine_note_on as the notes from
// JELLO_DRUM_FIRST_NOTE, each one shot to the end of its recording; the
// looper records them like notes. The most a pad is tuned either way, in
// half steps, and the choke groups there are, from 1.
enum {
    JELLO_DRUM_FIRST_NOTE = 130,
    JELLO_DRUM_PAD_COUNT = 16,
    JELLO_DRUM_MOST_HALF_STEPS = 12,
    JELLO_DRUM_MOST_GROUP = 4,
};

// Makes hits on pad `pad` after this sound at `level`, from 0 (silent) to 1,
// tuned `half_steps`, at `pan`, from -1 (left) to 1 (right), in choke group
// `group`, from 0 (none) to JELLO_DRUM_MOST_GROUP: a hit cuts off every
// other sounding in its group. Anything out of range is ignored. A new
// engine's pads are all at level 1, untuned, in the middle, in no group.
void jello_engine_set_drum_pad(JelloEngine* engine, int track, int pad, float level, int half_steps, float pan,
                               int group);

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
