// The logged-in home page's C face for the browser: scalar Wasm exports over home-web.coax, for
// coax/web/dom-host.js. The host keeps one HomeWeb, calls dom_frame each frame it is asked to and
// applies the commands at dom_out.
#include "home-web.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_HomeWeb at(uint32_t state) { return (coax_v_t_HomeWeb){(coax_t_HomeWeb *)(uintptr_t)state, 1}; }
static coax_t_HomeWeb *raw(uint32_t state) { return (coax_t_HomeWeb *)(uintptr_t)state; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_HomeWeb); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_HomeWeb(raw(state)); }
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_homeweb_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_homeweb_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_homeweb_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_homeweb_event(at(state), kind, slot, length); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_homeweb_size(at(state), slot, w, h); }
