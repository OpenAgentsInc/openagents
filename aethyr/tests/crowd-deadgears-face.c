/* The browser side of Dead Gears' crowd check: aethyr/tests/crowd-deadgears.coax's one export,
   for coax/web/replay-hosts.coax. The store is static: a fight's state is too big for the stack. */
#include "crowd-deadgears.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_Store store;
EXPORT("deadgears_npc_case") int64_t deadgears_npc_case_wasm(int64_t k) {return deadgears_npc_case((coax_v_t_Store){&store, 1}, k);}
