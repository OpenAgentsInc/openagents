// Firm reason: 6 (an app without a microphone; no decision lives here).
// Dictation for an app that holds the chat component (chat.h) without it: no recorder and no
// model, so dn_new gives none and the chat never offers the microphone's states. Everglade's
// Workshop links this in place of dictation-host.c and Psionic's Parakeet.
#include "dictation-host.h"
#include <stdio.h>

Dictation *dn_new(int fixture, const char *model, const char *stand_in, const char *recorder, DictationTo to) {
  (void)fixture; (void)model; (void)stand_in; (void)recorder; (void)to;
  return NULL;
}
void dn_enable(Dictation *d, int on) { (void)d; (void)on; }
void dn_activate(Dictation *d) { (void)d; }
void dn_deactivate(Dictation *d) { (void)d; }
uint64_t dn_begin(Dictation *d) { (void)d; return 0; }
void dn_end(Dictation *d) { (void)d; }
void dn_cancel(Dictation *d) { (void)d; }
void dn_pump(Dictation *d) { (void)d; }
int dn_ready(Dictation *d) { (void)d; return 0; }
void dn_fixture_time(Dictation *d, int64_t ms) { (void)d; (void)ms; }
void dn_fixture_audio(Dictation *d, const float *mono, int n, int rate, int64_t first_ms) { (void)d; (void)mono; (void)n; (void)rate; (void)first_ms; }
void dn_inspect(Dictation *d, int *phase, int *outcome, int *samples) { (void)d; *phase = 0; *outcome = 0; *samples = 0; }
void dn_close(Dictation *d) { (void)d; }
int dn_model_dir(char *out, int cap) { return snprintf(out, (size_t)cap, "%s", ""); }
