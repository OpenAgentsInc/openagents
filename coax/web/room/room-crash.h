/* Crash capture for the room server (room-crash.c, #13298, docs/deployment/crash-reports.md). */
#ifndef ROOM_CRASH_H
#define ROOM_CRASH_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
/* The handlers, and where a crash's dump goes ($STATE_DIRECTORY/crash.txt; none: the frames to stderr only). */
void crash_install(int shard);
/* One tick for the dump's ring of the last 16: when, how long, how many players. */
void crash_tick(int64_t wall_ms, int64_t us, int64_t players);
/* Whether SIGUSR2 asked for the crash test since the last call (vm-deploy.sh crash). */
bool crash_test_asked(void);
/* The dump the last process left, into dump (its length; 0 none), and addr2line's lines for its
   offsets into names (addr2line -a -f -i; *names_n bytes; 0 when its binary or addr2line is not there). */
size_t crash_pending(char *dump, size_t dcap, char *names, size_t ncap, size_t *names_n);
#endif
