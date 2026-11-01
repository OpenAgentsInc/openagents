/* The browser side of the Grid's host check: verse/tests/grid-session.coax's session, played
   once, and its step digests, for coax/web/replay-hosts.test.ts. */
#include "grid-session.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_GridWalk g, again;
static coax_v_t_GridWalk gv(void) { return (coax_v_t_GridWalk){&g, 1}; }
EXPORT("grid_play") int64_t play(void) { grid_session_play(gv()); return gridwalk_steps(gv()); }
EXPORT("grid_mark") int64_t mark(int64_t k) { return gridwalk_mark(gv(), k); }
EXPORT("grid_session") int64_t session(void) { return gridwalk_session(gv()); }
EXPORT("grid_records") int64_t records(void) { return g.records; }
EXPORT("grid_replayed") int64_t replayed(void) { return gridwalk_replay(gv(), (coax_v_t_GridWalk){&again, 1}) ? 1 : 0; }
