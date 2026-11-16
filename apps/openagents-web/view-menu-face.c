// A page's inspection menu's C face for the browser: scalar Wasm exports over view-menu.coax, for coax/web/dom-host.js.
// The host keeps one ViewMenu, calls dom_frame each frame it is asked to, applies the commands at dom_out, and
// reports presses; coax/web/host.js tells it the page's recipe (event 6) and takes its actions (a key to press, a refresh).
#include "view-menu.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_ViewMenu at(uint32_t state) { return (coax_v_t_ViewMenu){(coax_t_ViewMenu *)(uintptr_t)state, 1}; }
static coax_t_ViewMenu *raw(uint32_t state) { return (coax_t_ViewMenu *)(uintptr_t)state; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ViewMenu); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_ViewMenu(raw(state)); }
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_viewmenu_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_viewmenu_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_viewmenu_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_viewmenu_event(at(state), kind, slot, length); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_viewmenu_size(at(state), slot, w, h); }
EXPORT("dom_act_kind") int32_t dom_act_kind(uint32_t state) { return (int32_t)coaxial_viewmenu_act_kind(at(state)); }
EXPORT("dom_act") uint32_t dom_act(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->act.e; }
EXPORT("dom_act_len") uint32_t dom_act_len(uint32_t state) { return (uint32_t)coaxial_viewmenu_act_len(at(state)); }
EXPORT("dom_act_done") void dom_act_done(uint32_t state) { coaxial_viewmenu_act_done(at(state)); }
