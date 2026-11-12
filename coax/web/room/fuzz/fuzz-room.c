/* The room's wire, coverage-guided (#13283): libFuzzer's inputs as wire-fuzz.coax's actions (one
   seat's frames on a WebSocket, a WebTransport datagram or its stream, and ticks) into the room
   library that file builds, under ASan and UBSan. Fails (abort) when an action that is not a tick
   changes any byte of another seat (every Everglade room's seats but seat 0 of the sender's room),
   of another connection or of its WebTransport record; a trap, a sanitizer report or a leak fails
   on its own. Built and run by fuzz.sh. */
#define _GNU_SOURCE
#include "wire-fuzz.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#define PEERS 4               /* wire-fuzz.coax FZ_PEERS */
#define MAX_INPUT 65536
/* Each input starts from the room fz_room_begin left (one per kind of room). The server is 51 MB, so
   rather than copying it per input, it is mapped copy-on-write from a memory file of that room: only
   the pages an input writes are copied (Linux; elsewhere a copy). */
static coax_t_RmServer *v, *fresh[2];
static int fresh_fd[2] = {-1, -1};
static coax_t_EgSeat seat_was[16][16];
static _Bool live_was[16];
static coax_t_RmConn conn_was[PEERS];
static coax_t_RmWt wt_was[PEERS];

static coax_v_t_RmServer view(coax_t_RmServer *s) { return (coax_v_t_RmServer){s, 1}; }
static void take(void) {
  for (int k = 0; k < 16; k++) {
    live_was[k] = v->eg.e[k].live;
    if (live_was[k]) for (int s = 0; s < 16; s++) if (k != 0 || s != 0) seat_was[k][s] = v->eg.e[k].seat.e[s];
  }
  for (int c = 1; c < PEERS; c++) { conn_was[c] = v->conns.e[c]; wt_was[c] = v->wt.e[c]; }
}
/* The sender's room is the first everglade room made (k 0), its seat 0. No frame opens or closes a
   room (a join does), so a room that was not live stays so. */
static int changed(void) {
  for (int k = 0; k < 16; k++) {
    if (v->eg.e[k].live != live_was[k]) return 1;
    if (live_was[k]) for (int s = 0; s < 16; s++) if ((k != 0 || s != 0) && memcmp(&seat_was[k][s], &v->eg.e[k].seat.e[s], sizeof seat_was[k][s])) return 1;
  }
  for (int c = 1; c < PEERS; c++) if (memcmp(&conn_was[c], &v->conns.e[c], sizeof conn_was[c]) || memcmp(&wt_was[c], &v->wt.e[c], sizeof wt_was[c])) return 1;
  return 0;
}
static void bad(const char *why) { fprintf(stderr, "fuzz-room: %s\n", why); abort(); }
static void reset(int m) {
#ifdef __linux__
  if (fresh_fd[m] >= 0) {
    if (mmap(v, sizeof *v, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_FIXED, fresh_fd[m], 0) != v) bad("no map");
    return;
  }
#endif
  memcpy(v, fresh[m], sizeof *v);
}
int LLVMFuzzerInitialize(int *argc, char ***argv) {
  (void)argc; (void)argv;
  for (int m = 0; m < 2; m++) {
    fresh[m] = malloc(sizeof *fresh[m]);
    if (!fresh[m]) bad("out of memory");
    coax_default_t_RmServer(fresh[m]);   /* the declared defaults, as room-host.c server_new */
    if (fz_room_begin(view(fresh[m]), m) != 0) bad("no room to begin from");
#ifdef __linux__
    int fd = memfd_create("fuzz-room", 0);
    size_t done = 0;
    if (fd >= 0 && ftruncate(fd, sizeof *fresh[m]) == 0) {
      while (done < sizeof *fresh[m]) { ssize_t w = write(fd, (char *)fresh[m] + done, sizeof *fresh[m] - done); if (w <= 0) break; done += (size_t)w; }
    }
    if (done == sizeof *fresh[m]) fresh_fd[m] = fd; else if (fd >= 0) close(fd);
#endif
  }
  v = mmap(NULL, sizeof *v, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (v == MAP_FAILED) bad("no memory for the room");
  return 0;
}
int LLVMFuzzerTestOneInput(const uint8_t *data, size_t n) {
  if (n == 0 || n > MAX_INPUT) return 0;
  reset(data[0] & 1);
  if ((data[0] & 1) == 0 && !(v->eg.e[0].live && v->eg.e[0].relay == v->conns.e[1].relay && v->conns.e[0].seat == 0)) bad("the room is not where it is looked for");
  uint8_t *in = malloc(n);   /* its own copy, exactly n bytes: ASan sees a read past the input */
  memcpy(in, data, n);
  coax_v_u8 input = {in, (int64_t)n};
  int64_t at = 1;
  while (at < (int64_t)n) {
    _Bool tick = fz_ticks(input, at);
    if (!tick) take();
    int64_t next = fz_room_step(view(v), input, at);
    if (next <= at && next < (int64_t)n) bad("an action that doesn't move on");
    if (!tick && changed()) bad("a frame of seat 0 changed another seat or socket");
    at = next;
  }
  free(in);
  return 0;
}
