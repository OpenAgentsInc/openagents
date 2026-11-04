// Thin ABI transport. Coax owns setup parsing, labels, choices and readiness.
#include "session.h"
#include "face.h"
static coax_t_ForgeSetup setup;
static uint8_t input[65536];
static coax_v_t_ForgeSetup view(void) { return (coax_v_t_ForgeSetup){&setup, 1}; }
void fgs_reset(void) { fsu_reset(view()); }
int64_t fgs_input_byte(int64_t i, int64_t v) {
  if (i < 0 || i >= (int64_t)sizeof input || v < 0 || v > 255) return 0;
  input[i] = (uint8_t)v; return 1;
}
int64_t fgs_accept(int64_t kind, int64_t n) {
  if (n < 0 || n > (int64_t)sizeof input) return 0;
  return fsu_read(view(), kind, (coax_string){(const char *)input, n}) ? 1 : 0;
}
bool fgs_can_enter(bool scene) { return fsu_can_enter(view(), scene); }
bool fgs_completed(void) { return fsu_completed(view()); }
void fgs_choose(int64_t kind) { fsu_choose(view(), kind); }
int64_t fgs_count(int64_t kind) { return fsu_count(view(), kind); }
int64_t fgs_selected(void) { return fsu_selected(view()); }
int64_t fgs_status(int64_t kind) { return fsu_status(view(), kind); }
int64_t fgs_text_byte(int64_t field, int64_t index) { return fsu_text_byte(view(), field, index); }
int64_t fgs_settings_byte(int64_t index) { return fsu_settings_byte(view(), index); }
