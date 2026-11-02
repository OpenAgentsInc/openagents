/* The browser side of aethyr/replay.coax's host check: aethyr/tests/replay-walk.coax's one
   export, for coax/web/replay-hosts.coax. */
#include "replay-walk.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("replay_walk") int64_t walk(int64_t mode) {return replay_walk(mode);}
