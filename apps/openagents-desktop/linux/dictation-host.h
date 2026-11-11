// Firm reason: 6 (the recorder process, the model's thread and the clock; every decision is Coax).
// Dictation's effects for OpenAgents on Linux: what CoderDictation is on the Mac
// (Sources/Dictation.swift). It runs the recorder the Coax names and hands its samples to
// the shared controller (apps/shared/dictation.coax behind dictation-face.h), opens Psionic's
// resident Parakeet on a thread of its own, and gives each finished clip to it one segment at a
// time. The recorder and the model's worker are the CLI's (apps/openagents-cli/dictate-native.c).
// Admission, timing, silence, segment cuts and the joined text are the controller's. Nothing
// here types, sends or writes audio to disk.
//
// Everything is called from the window's thread. dn_pump is called once a frame.
#ifndef OA_LINUX_DICTATION_HOST_H
#define OA_LINUX_DICTATION_HOST_H
#include <stdint.h>

typedef struct Dictation Dictation;
typedef struct {
  void *user;
  // The controller's facts after a change (dictation-face.h: phase, outcome, fault, what it is preparing).
  void (*state)(void *user, int phase, int outcome, int fault, int preparing);
  // A finished request's words, once, after its facts.
  void (*words)(void *user, uint64_t ticket, const uint8_t *text, int n);
} DictationTo;

// model: the pinned Parakeet model's folder.
// fixture: offscreen checks only. No recorder runs and the clock is dn_fixture_time's; audio
// comes from dn_fixture_audio.
// stand_in: NULL, or the words a check's stand-in recognizer answers each call with. No model
// is opened.
// recorder: NULL for the Coax's own (pw-record), or the program run in its place with its
// arguments (a check's stand-in). It writes 48 kHz mono 32-bit float samples to its stdout.
Dictation *dn_new(int fixture, const char *model, const char *stand_in, const char *recorder, DictationTo to);
void dn_enable(Dictation *d, int on);     // the user's consent (settings.json)
void dn_activate(Dictation *d);           // the window has the keyboard
void dn_deactivate(Dictation *d);
uint64_t dn_begin(Dictation *d);          // the key or the button went down: the request's ticket, 0 for none
void dn_end(Dictation *d);                // it came up
void dn_cancel(Dictation *d);
void dn_pump(Dictation *d);               // the recorder's samples, the clock, a finished model call, what changed
int dn_ready(Dictation *d);               // the model is open and the input is running
void dn_fixture_time(Dictation *d, int64_t ms);
void dn_fixture_audio(Dictation *d, const float *mono, int n, int rate, int64_t first_ms);
void dn_inspect(Dictation *d, int *phase, int *outcome, int *samples);
void dn_close(Dictation *d);              // the recorder ends and the model closes
// The pinned model's folder into out, its length: OPENAGENTS_PARAKEET_MODEL, else the shared model
// home (OPENAGENTS_MODEL_CACHE, else ~/.openagents/models; coax/lib/sys/openagents-paths.coax).
int dn_model_dir(char *out, int cap);
#endif
