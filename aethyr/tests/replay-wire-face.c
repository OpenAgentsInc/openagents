/* Browser SDK projections only; all record policy and replay are Coax. */
#include "replay-wire-walk.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_RwWalk record;
static coax_v_t_RwWalk owned(void) {return (coax_v_t_RwWalk){&record,1};}
EXPORT("rw_make") int64_t make(int64_t seed) {return rw_make(owned(),seed);}
EXPORT("rw_play") int64_t play(int64_t n,int64_t fault) {return rw_play(owned(),n,fault);}
EXPORT("rw_mark") int64_t mark(int64_t tick) {return rw_mark(owned(),tick);}
EXPORT("rw_pointer") uint32_t pointer(void) {return (uint32_t)(uintptr_t)record.wire.e;}
EXPORT("rw_steps") int64_t steps(void) {return record.steps;}
EXPORT("rw_digest") int64_t digest(void) {return record.digest;}
EXPORT("rw_remote") int64_t remote(void) {return record.remote;}
EXPORT("rw_diverged") int64_t diverged(void) {return record.diverged;}
