// Firm reason: 1 (flat browser ABI for the Coax component module; no product logic).
#include <stdint.h>
#include "app.h"
#define EXPORT(n) __attribute__((export_name(n)))
static coax_t_Starter *raw(uint32_t s) { return (void *)(uintptr_t)s; }
static coax_v_t_Starter at(uint32_t s) { return (coax_v_t_Starter){raw(s),1}; }
EXPORT("coaxial_state_size") uint32_t state_size(void) { return sizeof(coax_t_Starter); }
EXPORT("coaxial_state_init") void state_init(uint32_t s) { coax_default_t_Starter(raw(s)); }
EXPORT("dom_frame") int32_t frame(uint32_t s,float dt,uint32_t reduced) { return (int32_t)st_frame(at(s),dt,reduced!=0); }
EXPORT("dom_out") uint32_t output(uint32_t s) { return (uintptr_t)raw(s)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t busy(uint32_t s) { return raw(s)->dirty || raw(s)->ui.e[0].busy; }
EXPORT("dom_inbox") uint32_t inbox(uint32_t s) { return (uintptr_t)raw(s)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t cap(void) { return 8192; }
EXPORT("dom_event") void event(uint32_t s,int32_t k,int32_t slot,int32_t n) { st_event(at(s),k,slot,n); }
EXPORT("dom_size") void size(uint32_t s,int32_t slot,float w,float h) { st_size(at(s),slot,w,h); }
EXPORT("dom_act_kind") int32_t action(uint32_t s) { return raw(s)->action; }
EXPORT("dom_act") uint32_t act(uint32_t s) { (void)s; return 0; }
EXPORT("dom_act_len") uint32_t act_len(uint32_t s) { (void)s; return 0; }
EXPORT("dom_act_done") void act_done(uint32_t s) { st_done(at(s)); }
