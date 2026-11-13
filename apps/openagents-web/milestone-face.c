// /milestone's C face for the browser: scalar Wasm exports over milestone.coax, for coax/web/dom-host.js.
// The host keeps one Milestone, calls dom_frame each frame it is asked to, applies the commands at dom_out,
// reports presses, typing and framed nodes' sizes, and hands coax drive's operations to dom_drive.
#include "milestone.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_Milestone at(uint32_t state) { return (coax_v_t_Milestone){(coax_t_Milestone *)(uintptr_t)state, 1}; }
static coax_t_Milestone *raw(uint32_t state) { return (coax_t_Milestone *)(uintptr_t)state; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Milestone); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Milestone(raw(state)); }
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_milestone_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_milestone_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_milestone_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_milestone_event(at(state), kind, slot, length); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_milestone_size(at(state), slot, w, h); }
// coax drive: the operation is in the inbox; the answer's bytes are at dom_drive_out.
EXPORT("dom_drive") int32_t dom_drive(uint32_t state, int32_t length) { return (int32_t)coaxial_milestone_drive(at(state), length); }
EXPORT("dom_drive_out") uint32_t dom_drive_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->drive_out.e; }
// What Copy put on the clipboard: the host writes it to the browser's.
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->app.e[0].clip.e; }
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)coaxial_milestone_clip(at(state)); }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { coaxial_milestone_clip_done(at(state)); }
