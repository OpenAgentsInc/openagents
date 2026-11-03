/* The browser side of aethyr/scatter.coax's host check: aethyr/tests/scatter-hosts.coax's one
   export, for coax/web/replay-hosts.coax. */
#include "scatter-hosts.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("scatter_case") int64_t scatter_case_wasm(int64_t k) {return scatter_case(k);}
