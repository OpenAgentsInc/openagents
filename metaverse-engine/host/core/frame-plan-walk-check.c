/* Checks frame-plan-walk.h against what Coax says a walk must call (GitHub #13303).
 *   coax/web/tool.sh metaverse-engine/tools/frame-plan-fixture.coax | frame-plan-walk-check
 * Then the refusals: a plan the walk must not start. Prints one line and exits 0, or says what differed. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "frame-plan-walk.h"

static char got[1 << 20]; static size_t got_n; static int calls;
static void say(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
#include <stdarg.h>
static void say(const char *fmt, ...) {
  va_list a; va_start(a, fmt);
  int n = vsnprintf(got + got_n, sizeof got - got_n, fmt, a); va_end(a);
  if (n > 0 && got_n + (size_t)n < sizeof got) got_n += (size_t)n;
  calls++;
}
static void h_begin(void *s, const aethyr_fp_op *o) { (void)s; say("begin %u %u %u %u %u\n", o->colour0, o->colour1, o->depth, (o->flags & AETHYR_FP_FLAG_KEEP) ? 1u : 0u, (o->flags & AETHYR_FP_FLAG_STORE_DEPTH) ? 1u : 0u); }
static void h_end(void *s) { (void)s; say("end\n"); }
static void h_draw(void *s, const aethyr_fp_op *o, uint32_t item) { (void)s; say("draw %u %u %u %u %u %u\n", o->pass, o->source, o->first, o->count, o->instances, item); }
static void h_copy(void *s, uint32_t from, uint32_t to) { (void)s; say("copy %u %u\n", from, to); }
static void h_rect(void *s, int scissor, uint32_t x, uint32_t y, uint32_t w, uint32_t h) { (void)s; say("rect %d %u %u %u %u\n", scissor, x, y, w, h); }
static void h_compute(void *s, uint32_t kind, uint32_t runs) { (void)s; say("compute %u %u\n", kind, runs); }
static void h_media(void *s, int after) { (void)s; say("media %d\n", after); }
static const aethyr_fp_host host = {0, h_begin, h_end, h_draw, h_copy, h_rect, h_compute, h_media};

static uint32_t words[AETHYR_FP_WORDS * (AETHYR_FP_MOST_OPS + 1)];
static char line[1 << 16], want[1 << 20];

static int refused(const char *what, const uint32_t *w, uint32_t n) {
  got_n = 0; calls = 0;
  if (aethyr_frame_plan_walk(w, n, &host) != -1 || calls != 0) { fprintf(stderr, "frame-plan-walk: walked %s (%d calls)\n", what, calls); return 1; }
  return 0;
}

int main(void) {
  int plans = 0, bad = 0, ops = 0; uint32_t last_n = 0; static uint32_t last[sizeof words / 4];
  while (fgets(line, sizeof line, stdin)) {
    unsigned n = 0;
    if (sscanf(line, "plan %u", &n) != 1 || n == 0 || n > sizeof words / 4) { fprintf(stderr, "frame-plan-walk: not a plan line: %s", line); return 1; }
    if (!fgets(line, sizeof line, stdin)) return 1;
    char *p = line;
    for (unsigned i = 0; i < n; i++) words[i] = (uint32_t)strtoul(p, &p, 16);
    size_t want_n = 0;
    while (fgets(line, sizeof line, stdin) && strcmp(line, "done\n") != 0) { size_t l = strlen(line); memcpy(want + want_n, line, l); want_n += l; }
    got_n = 0; calls = 0;
    int32_t walked = aethyr_frame_plan_walk(words, n, &host);
    if (walked < 0 || (uint32_t)walked != words[3] || got_n != want_n || memcmp(got, want, want_n) != 0) {
      fprintf(stderr, "frame-plan-walk: plan %d differs (walked %d of %u operations)\n--- Coax says\n%.*s--- the walk called\n%.*s", plans, walked, words[3], (int)want_n, want, (int)got_n, got);
      bad++;
    }
    plans++; ops += walked > 0 ? walked : 0;
    memcpy(last, words, n * 4); last_n = n;
  }
  if (plans < 6 || last_n < 32) { fprintf(stderr, "frame-plan-walk: only %d plans read\n", plans); return 1; }
  /* refusals, each from a plan that walks */
  uint32_t w[sizeof words / 4];
#define AGAIN memcpy(w, last, last_n * 4)
  AGAIN; w[0] ^= 1u; bad += refused("another magic", w, last_n);
  AGAIN; w[1] = AETHYR_FP_VERSION + 1u; bad += refused("a newer version", w, last_n);
  AGAIN; w[2] = 9u; bad += refused("another record size", w, last_n);
  AGAIN; bad += refused("a short buffer", w, last_n - 1u);
  AGAIN; w[3] = AETHYR_FP_MOST_OPS + 1u; bad += refused("too many operations", w, last_n);
  AGAIN; w[8] = (w[8] & ~255u) | 13u; bad += refused("an operation it does not know", w, last_n);
  AGAIN; w[8] = (w[8] & ~255u) | AETHYR_FP_OP_END; bad += refused("an end with no pass open", w, last_n);
  AGAIN; w[last_n - 8] = AETHYR_FP_OP_DRAW; bad += refused("a pass left open", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_DRAW | (13u << 8); bad += refused("a pass it does not know", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_DRAW | (18u << 8); bad += refused("a pass past the last", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_BEGIN; bad += refused("a pass begun inside a pass", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_EACH_END; bad += refused("an each loop never begun", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_EACH_BEGIN; bad += refused("an each loop left open", w, last_n);
  AGAIN; w[16] = AETHYR_FP_OP_COPY; bad += refused("a copy inside a pass", w, last_n);
  AGAIN; w[9] = (w[9] & ~255u) | 12u; bad += refused("a target it does not know", w, last_n);
  bad += refused("no words", 0, 0);
  got_n = 0; calls = 0;
  if (aethyr_frame_plan_walk(last, last_n, 0) != -1) bad++;
  if (bad) return 1;
  printf("frame-plan-walk: %d plans, %d operations walked as Coax says; 16 malformed plans refused with no call\n", plans, ops);
  return 0;
}
