/* Offline factory regression over the real generated desktop types/functions.
 * No aggregate-padding comparison and no real child process or environment read. */
#include "zeron.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static FILE *zdf_stderr(void) { return stderr; }
#undef stderr

static unsigned defaults_calls, state_allocations, slot0_releases;
static int fail_next_state;
static unsigned char *state_raw[4];
static coax_t_ZeronState *states[4];
enum { GUARD = 64 };
#define REQUIRE(x) do { if (!(x)) { fprintf(zdf_stderr(), "FAIL %s:%d: %s\n", __FILE__, __LINE__, #x); return 1; } } while (0)
static void *factory_calloc(size_t count, size_t size) {
  if (count != 1 || size != sizeof(coax_t_ZeronState)) return calloc(count, size);
  state_allocations++;
  if (fail_next_state) { fail_next_state = 0; return NULL; }
  unsigned char *raw = calloc(1, size + 2 * GUARD);
  if (!raw) return NULL;
  memset(raw, 0xa5, GUARD); memset(raw + GUARD + size, 0x5a, GUARD);
  for (unsigned i = 0; i < 4; i++) if (!states[i]) {
    state_raw[i] = raw; states[i] = (coax_t_ZeronState *)(raw + GUARD); return states[i];
  }
  abort();
}
static void factory_defaults(coax_t_ZeronState *state) {
  if (!state) { fputs("initializer called with null\n", zdf_stderr()); abort(); }
  defaults_calls++;
  coax_default_t_ZeronState(state);
}
/* Instrument only the production C face, not generated Coax definitions. */
#define calloc factory_calloc
#define coax_default_t_ZeronState factory_defaults
#include "factory-face.inc"
#undef coax_default_t_ZeronState
#undef calloc

/* The actual process pool is linked below. Prevent attachment's executable
 * probe from spawning anything, and prevent configuration environment reads. */
int64_t cxproc_spawn(int64_t handle) { (void)handle; return 2; }
int64_t cxproc_getenv(void) { return 0; }
int64_t cxproc_getenv_byte(int64_t index) { (void)index; return 0; }
int64_t cxproc_new(void);
int64_t zdf_real_release(int64_t handle);
int64_t cxproc_release(int64_t handle) {
  if (handle == 0) slot0_releases++;
  return zdf_real_release(handle);
}

static int guards(void) {
  for (unsigned i = 0; i < 4; i++) if (states[i]) {
    for (unsigned j = 0; j < GUARD; j++) {
      REQUIRE(state_raw[i][j] == 0xa5);
      REQUIRE(state_raw[i][GUARD + sizeof(coax_t_ZeronState) + j] == 0x5a);
    }
  }
  return 0;
}
static void cleanup(void) {
  for (unsigned i = 0; i < 4; i++) { free(state_raw[i]); states[i] = NULL; state_raw[i] = NULL; }
}
static int defaults_case(void) {
  fail_next_state = 1;
  REQUIRE(zr_new() == NULL && defaults_calls == 0 && state_allocations == 1);
  coax_t_ZeronState *s = zr_new();
  REQUIRE(s != NULL && defaults_calls == 1 && state_allocations == 2);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].h == -1);
  REQUIRE(s->lab.e[0].owner.e[0].generation == 0 && s->lab.e[0].owner.e[0].phase == 0 && s->lab.e[0].ticket == 0);
  REQUIRE(!s->lab.e[0].owner.e[0].call.e[0].done && !s->ready);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.status == 0);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.body.count == 0);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.stderr.count == 0);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.problem.count == 0);
  REQUIRE(s->lab.e[0].out.count == 0 && s->lab.e[0].out.data == NULL);
  REQUIRE(s->codex.count == 0 && s->codex.data == NULL);
  REQUIRE(s->engine.e[0].p.e[0].handle == -1);
  REQUIRE(s->composer.e[0].open == -1 && s->composer.e[0].hover == -1);
  REQUIRE(guards() == 0);
  coax_t_ZeronState *other = zr_new();
  REQUIRE(other && other != s && defaults_calls == 2 && state_allocations == 3);
  REQUIRE(other->lab.e[0].owner.e[0].call.e[0].h == -1);
  /* A later factory allocation must not reconstruct a previous owner. */
  s->lab.e[0].owner.e[0].call.e[0].r.status = 299;
  zr_quit(s); /* Fresh unattached quit is intentionally inert. */
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.status == 299 && defaults_calls == 2);
  REQUIRE(guards() == 0);
  cleanup(); puts("factory defaults: failed allocation skips init; each successful owner initialized once; nested defaults and guards pass");
  return 0;
}
static int attachment_case(void) {
  int64_t unrelated = cxproc_new();
  REQUIRE(unrelated == 0);
  coax_t_ZeronState *s = zr_new(); REQUIRE(s);
  unsigned initialized = defaults_calls;
  /* Full settings/screen emission is analysis-refused by the retained tools.
   * This explicitly bounded owner-ready projection uses the exact authored
   * lab_attach and quit/close functions over the complete actual ZeronState.
   * It does not execute the full zr_attach catalog/process-probe path. */
  struct { uint8_t guard1[16], bytes[31], guard2[16]; } body, response, error;
  memset(&body, 0xa5, sizeof body); memset(&response, 0xb6, sizeof response); memset(&error, 0xc7, sizeof error);
  zdf_attach((coax_v_t_ZeronState){s,1}, (coax_v_u8){body.bytes,sizeof body.bytes},
             (coax_v_u8){response.bytes,sizeof response.bytes}, (coax_v_u8){error.bytes,sizeof error.bytes});
  REQUIRE(s->engine.e[0].on);
  REQUIRE(defaults_calls == initialized);
  REQUIRE(s->lab.e[0].body.count > 0 && s->lab.e[0].out.count > 0 && s->lab.e[0].err.count > 0);
  uint8_t *out = s->lab.e[0].out.data, *err = s->lab.e[0].err.data;
  int64_t out_count = s->lab.e[0].out.count, err_count = s->lab.e[0].err.count;
  out[0] = 0x31; out[out_count - 1] = 0x72; err[0] = 0x43; err[err_count - 1] = 0x64;
  s->lab.e[0].owner.e[0].call.e[0].r.status = 299;
  zr_quit(s);
  REQUIRE(slot0_releases == 0);
  REQUIRE(defaults_calls == initialized);
  REQUIRE(s->lab.e[0].out.data == out && s->lab.e[0].out.count == out_count);
  REQUIRE(s->lab.e[0].err.data == err && s->lab.e[0].err.count == err_count);
  REQUIRE(out[0] == 0x31 && out[out_count - 1] == 0x72 && err[0] == 0x43 && err[err_count - 1] == 0x64);
  REQUIRE(s->lab.e[0].owner.e[0].call.e[0].r.status == 299);
  zr_quit(s); REQUIRE(slot0_releases == 0 && defaults_calls == initialized);
  int64_t next = cxproc_new(); REQUIRE(next == 1);
  REQUIRE(zdf_real_release(next) == 0 && zdf_real_release(unrelated) == 0);
  REQUIRE(guards() == 0);
  for (unsigned j=0; j<16; j++) { REQUIRE(body.guard1[j]==0xa5 && body.guard2[j]==0xa5); REQUIRE(response.guard1[j]==0xb6 && response.guard2[j]==0xb6); REQUIRE(error.guard1[j]==0xc7 && error.guard2[j]==0xc7); }
  /* Attached backing buffers are app-life storage in production. No new free
   * or descriptor-invalidation guarantee is inferred by this check. */
  cleanup(); puts("factory owner-ready projection: exact lab_attach and repeated actual quit preserve unrelated slot0, descriptors and guarded bytes; no reinitialization");
  return 0;
}
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  if (!strcmp(argv[1], "defaults")) return defaults_case();
  if (!strcmp(argv[1], "attachment")) return attachment_case();
  return 2;
}
