// /login's C face for the browser: scalar Wasm exports over login-web.coax
// for coax/web/dom-host.js (dark-mode Apps SDK UI, zero WebGPU HUD).
#include "login-web.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))

IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);
IMPORT("dom_stream_stop") void dom_stream_stop(void);
IMPORT("dom_go") void dom_go(uint32_t at, uint32_t len, uint32_t push);

static coax_v_t_LoginWeb at(uint32_t state) { return (coax_v_t_LoginWeb){(coax_t_LoginWeb *)(uintptr_t)state, 1}; }
static coax_t_LoginWeb *raw(uint32_t state) { return (coax_t_LoginWeb *)(uintptr_t)state; }

static void asks(uint32_t state) {
  coax_t_LoginWeb *s = raw(state);
  if (coaxial_loginweb_stop(at(state))) dom_stream_stop();
  int64_t go = coaxial_loginweb_go_len(at(state));
  if (go > 0) {
    uint32_t push = coaxial_loginweb_go_push(at(state)) ? 1u : 0u;
    coaxial_loginweb_go_done(at(state));
    dom_go((uint32_t)(uintptr_t)s->go.e, (uint32_t)go, push);
  }
  if (coaxial_loginweb_asking(at(state))) {
    int64_t n = coaxial_loginweb_ask_len(at(state));
    dom_stream((uint32_t)(uintptr_t)s->door.e, (uint32_t)s->door_n,
               (uint32_t)(uintptr_t)s->request.e, (uint32_t)(n < 0 ? 0 : n));
    coaxial_loginweb_asked(at(state));
  }
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_LoginWeb); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { *raw(state) = (coax_t_LoginWeb){0}; }
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) {
  int32_t n = (int32_t)coaxial_loginweb_frame(at(state), dt, reduced != 0);
  asks(state);
  return n;
}
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_loginweb_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_loginweb_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) {
  coaxial_loginweb_event(at(state), kind, slot, length);
  asks(state);
}
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { (void)state; (void)slot; (void)w; (void)h; }
