// Minimal C Wasm face for apps/openagents-web/demos/docs-starter/page.coax.
#include "page.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_DocsStarter at(uint32_t state) { return (coax_v_t_DocsStarter){(coax_t_DocsStarter *)(uintptr_t)state, 1}; }
static coax_t_DocsStarter *raw(uint32_t state) { return (coax_t_DocsStarter *)(uintptr_t)state; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_DocsStarter); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_DocsStarter(raw(state)); }
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_ds_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_ds_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_ds_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_ds_event(at(state), kind, slot, length); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_ds_size(at(state), slot, w, h); }
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->clip.e; }
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)coaxial_ds_clip(at(state)); }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { coaxial_ds_clip_done(at(state)); }
