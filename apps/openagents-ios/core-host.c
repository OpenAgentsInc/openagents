// Firm reason: 6 (native carrier for the shared app core's C entry points; every decision is Coax).
// core-host.h over the generated app_host_ procedures. host.c includes it, so it shares the one
// generated library. A dispatch that answers APP_GUEST is routed by the root (app.coax) at once.
#include "core-host.h"

static coax_v_t_AppCore oa_core(void *core) { return (coax_v_t_AppCore){core, core ? 1 : 0}; }
static coax_string oa_bytes(const char *p, int64_t n) { return (coax_string){p ? p : "", p ? n : 0}; }
static coax_string oa_cstr(const char *p) { return oa_bytes(p, p ? (int64_t)strlen(p) : 0); }
// The page that takes a guest result (oa_core_on_guest): the environment picker's reads. With none,
// the root lets the guest go (oa_root_guest).
static void (*oa_guest_hook)(void *context, const uint8_t *body, int64_t n);
static void *oa_guest_context;
void oa_core_on_guest(void (*hook)(void *context, const uint8_t *body, int64_t n), void *context) { oa_guest_hook = hook; oa_guest_context = context; }
static int64_t oa_routed_body(void *core, int64_t answer, const uint8_t *body, int64_t n) {
  if (answer == 2) { if (oa_guest_hook) oa_guest_hook(oa_guest_context, body, n); else oa_root_guest(oa_core(core)); }
  return answer;
}
static int64_t oa_routed(void *core, int64_t answer) { return oa_routed_body(core, answer, NULL, 0); }

void *oa_core_open(void) {
  coax_t_AppCore *c = calloc(1, sizeof *c);
  if (c && !app_host_init(oa_core(c))) { free(c); return NULL; }
  return c;
}
void oa_core_close(void *core) { free(core); }
int64_t oa_core_launch(void *core, int64_t mono, int64_t wall, const char *rejoin) { return oa_routed(core, app_host_launch(oa_core(core), mono, wall, oa_cstr(rejoin))); }
int64_t oa_core_clock(void *core, int64_t mono, int64_t wall) { return oa_routed(core, app_host_clock(oa_core(core), mono, wall)); }
int64_t oa_core_lifecycle(void *core, int64_t mono, int64_t wall, int64_t code) { return oa_routed(core, app_host_lifecycle(oa_core(core), mono, wall, code)); }
int64_t oa_core_action(void *core, int64_t mono, int64_t wall, int64_t code, int flag, const char *text, const char *more) {
  return oa_routed(core, app_host_action(oa_core(core), mono, wall, code, flag != 0, oa_cstr(text), oa_cstr(more)));
}
int64_t oa_core_next(void *core) { return app_host_next(oa_core(core)); }
int oa_core_request(void *core, int64_t op, int64_t out[6]) { return app_host_request(oa_core(core), op, (coax_v_s64){out, 6}); }
int64_t oa_core_path(void *core, int64_t op, uint8_t *out, int64_t cap) { return app_host_path(oa_core(core), op, (coax_v_u8){out, cap}); }
int64_t oa_core_text(void *core, int64_t op, uint8_t *out, int64_t cap) { return app_host_text(oa_core(core), op, (coax_v_u8){out, cap}); }
int64_t oa_core_body(void *core, int64_t op, uint8_t *out, int64_t cap) { return app_host_body(oa_core(core), op, (coax_v_u8){out, cap}); }
int64_t oa_core_taken(void *core, int64_t mono, int64_t wall, int64_t op, int accepted) { return oa_routed(core, app_host_taken(oa_core(core), mono, wall, op, accepted != 0)); }
int64_t oa_core_cancel_next(void *core) { return app_host_cancel_next(oa_core(core)); }
int64_t oa_core_done(void *core, int64_t mono, int64_t wall, int64_t op, int64_t generation, int64_t outcome, int64_t status, int flag,
                     const uint8_t *text, int64_t text_n, const uint8_t *more, int64_t more_n, int64_t value) {
  return oa_routed_body(core, app_host_done(oa_core(core), mono, wall, op, generation, outcome, status, flag != 0, oa_bytes((const char *)text, text_n),
                                            oa_bytes((const char *)more, more_n), value), text, text_n);
}
int64_t oa_core_wake(void *core) { return app_host_wake(oa_core(core)); }
int64_t oa_core_busy(void *core) { return app_host_busy(oa_core(core)); }
int64_t oa_core_dirty_take(void *core, int64_t mask) { return app_host_dirty_take(oa_core(core), mask); }
int64_t oa_core_keep(void *core, uint8_t *out, int64_t cap) { return app_host_keep(oa_core(core), (coax_v_u8){out, cap}); }
int oa_policy_http(int64_t method, const char *path, int64_t body_bytes) { return up_http(method, oa_cstr(path), body_bytes); }
