// Crew's pane's C face for the browser: scalar Wasm exports over crew-pane.coax, for
// coax/web/dom-host.js. The host keeps one CrewPane, calls dom_frame each frame it is asked to,
// applies the commands at dom_out, and reports presses, typing and framed nodes' sizes. The page
// that opened it (coax/web/host.js, "A page's own pane") puts what it holds in the inbox (event 6)
// and takes the pane's actions (dom_act) as its own messages.
#include "crew-pane.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_CrewPane at(uint32_t state) { return (coax_v_t_CrewPane){(coax_t_CrewPane *)(uintptr_t)state, 1}; }
static coax_t_CrewPane *raw(uint32_t state) { return (coax_t_CrewPane *)(uintptr_t)state; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_CrewPane); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_CrewPane(raw(state)); }
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the pane and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_crewpane_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_crewpane_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_crewpane_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_crewpane_event(at(state), kind, slot, length); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_crewpane_size(at(state), slot, w, h); }
// The action that waits for the page: its kind (0: none), its bytes, and its taking.
EXPORT("dom_act_kind") int32_t dom_act_kind(uint32_t state) { return (int32_t)coaxial_crewpane_act_kind(at(state)); }
EXPORT("dom_act") uint32_t dom_act(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->acts.e[0].e; }
EXPORT("dom_act_len") uint32_t dom_act_len(uint32_t state) { return (uint32_t)coaxial_crewpane_act_len(at(state)); }
EXPORT("dom_act_done") void dom_act_done(uint32_t state) { coaxial_crewpane_act_done(at(state)); }
