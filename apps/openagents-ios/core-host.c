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
  // the device lane (apps/shared/app/device/lane.coax): its own record beside the core, for the core's life
  coax_t_DeviceLane *d = calloc(1, sizeof *d);
  if (!c || !d || !app_host_init(oa_core(c)) || !app_host_device(oa_core(c), (coax_v_t_DeviceLane){d, 1})) { free(c); free(d); return NULL; }
  return c;
}
void oa_core_close(void *core) {
  coax_t_AppCore *c = core;
  if (c) free(c->device.data);
  free(core);
}
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

// ---------------------------------------------------------------- the device's seal (seal.coax dv_*)

void *oa_device_work_open(void) { return calloc(1, sizeof(coax_t_DvWork)); }
void oa_device_work_close(void *work) { if (work) { memset(work, 0, sizeof(coax_t_DvWork)); free(work); } }
static coax_v_t_DvWork oa_work(void *w) { return (coax_v_t_DvWork){w, w ? 1 : 0}; }
static coax_v_u8 oa_out(uint8_t *p, int64_t n) { return (coax_v_u8){p, p ? n : 0}; }
static coax_v_u8 oa_in(const uint8_t *p, int64_t n) { return (coax_v_u8){(uint8_t *)p, p ? n : 0}; }
int64_t oa_device_public(const uint8_t secret[32], uint8_t out[64]) { return dv_public(oa_in(secret, 32), oa_out(out, 64)); }
int oa_device_host(const char *key) {return oa_root_host_key(oa_cstr(key));}
int32_t oa_environment_read(const char *method,const char *session,const char *before,uint8_t *out,int32_t cap) {return (int32_t)ea_read_body(oa_cstr(method),oa_cstr(session),oa_cstr(before),oa_out(out,cap));}
int32_t oa_environment_count(const char *host,const char *method,const uint8_t *raw,int32_t size) {return (int32_t)oa_root_resident_count(oa_cstr(host),oa_cstr(method),oa_bytes((const char *)raw,size));}
int32_t oa_environment_stage(void *core,const char *host,const char *marker) {return (int32_t)oa_root_resident_stage((coax_v_t_AppCore){core,core?1:0},oa_cstr(host),oa_cstr(marker));}
int64_t oa_device_seal(void *work, int64_t op, const uint8_t secret[32], const char *host, const char *relay, const char *arg, int64_t epoch,
                       const uint8_t *params, int64_t params_n, int64_t now, const uint8_t random[96], uint8_t *out, int64_t cap) {
  return dv_seal(oa_work(work), op, oa_in(secret, 32), oa_cstr(host), oa_cstr(relay), oa_cstr(arg), epoch, oa_bytes((const char *)params, params_n), now, oa_in(random, 96), oa_out(out, cap));
}
int64_t oa_device_filter(void *work, uint8_t *out, int64_t cap) { return dv_filter(oa_work(work), oa_out(out, cap)); }
int64_t oa_device_event_id(void *work, uint8_t out[64]) { return dv_event_id(oa_work(work), oa_out(out, 64)); }
int64_t oa_device_auth(const uint8_t secret[32], const char *relay, const uint8_t *challenge, int64_t challenge_n, int64_t now, const uint8_t aux[32], uint8_t *out, int64_t cap) {
  return dv_auth(oa_in(secret, 32), oa_cstr(relay), oa_bytes((const char *)challenge, challenge_n), now, oa_in(aux, 32), oa_out(out, cap));
}
int64_t oa_device_open(void *work, const uint8_t secret[32], const char *host, const uint8_t *event, int64_t event_n, int64_t now, uint8_t *out, int64_t cap) {
  return dv_open(oa_work(work), oa_in(secret, 32), oa_cstr(host), oa_bytes((const char *)event, event_n), now, oa_out(out, cap));
}

int ou_effect(int32_t mode,const char *host,const uint8_t *raw,int32_t n) {return us_effect(mode,oa_cstr(host),oa_bytes((const char *)raw,n));}
int ou_http(int32_t method,const char *path,int32_t bytes) {return us_http(method,oa_cstr(path),bytes);}
int32_t ou_mutation(const uint8_t *raw,int32_t n) {return (int32_t)us_mutation(oa_bytes((const char *)raw,n));}
int32_t ou_pending(const uint8_t *raw,int32_t n,uint8_t *out,int32_t cap) {return (int32_t)us_pending_public(oa_bytes((const char *)raw,n),oa_out(out,cap));}
int32_t ou_request(const uint8_t *raw,int32_t n,const uint8_t *kept,int32_t kn,const char *input,uint8_t *out,int32_t cap) {return (int32_t)us_private_request(oa_bytes((const char *)raw,n),oa_bytes((const char *)kept,kn),oa_cstr(input),oa_out(out,cap));}
int32_t ou_reply(const char *host,const uint8_t *metadata,int32_t mn,const uint8_t *raw,int32_t n,uint8_t *out,int32_t cap) {return (int32_t)us_reply(oa_cstr(host),oa_bytes((const char *)metadata,mn),oa_bytes((const char *)raw,n),oa_out(out,cap));}

int ou_input(const uint8_t *raw,int32_t n) {return us_private_input(oa_bytes((const char *)raw,n));}

int ou_settled(const uint8_t *raw,int32_t n) {return us_settled(oa_bytes((const char *)raw,n));}
