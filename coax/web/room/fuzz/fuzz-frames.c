/* The room's framing, coverage-guided (#13283): what reads a socket's bytes before the room's Coax
   hears a frame, under ASan and UBSan. The input's first byte says which:
     even  a WebSocket's bytes as room-host.c reads them: the upgrade request (ws_server_handshake),
           then frame after frame (ws_parse, at most 4,096 bytes a payload) until one is not whole
     odd   a WebTransport stream's bytes as room-wt.c reads them: frame after frame, each its
           2-byte length first (wt_frame_next, at most 4,096 bytes)
   Besides the sanitizers it holds each reader to its word: a frame lies wholly inside the bytes
   read, never past its limit, and every frame read moves on. Built and run by fuzz.sh. */
#include "../../../../verse/voice-host/ws.h"
#include "../wt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX 4096
static void bad(const char *why) { fprintf(stderr, "fuzz-frames: %s\n", why); abort(); }

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t n) {
  if (n == 0 || n > 65536) return 0;
  uint8_t *b = malloc(n - 1 ? n - 1 : 1);   /* exactly the bytes: ASan sees a read past them */
  size_t m = n - 1; memcpy(b, data + 1, m);
  if ((data[0] & 1) == 0) {
    /* the request: up to its blank line, as opening() takes it */
    size_t req = 0;
    for (size_t i = 0; i + 3 < m; i++) if (!memcmp(b + i, "\r\n\r\n", 4)) { req = i + 4; break; }
    if (req) {
      char out[512];
      size_t k = ws_server_handshake((const char *)b, req, out, sizeof out);
      if (k > sizeof out) bad("a handshake longer than its room");
    }
    size_t at = req;
    while (at < m) {
      ws_frame f;
      int r = ws_parse(b + at, m - at, MAX, &f);
      if (r <= 0) break;
      if (f.length > MAX) bad("a frame past its limit");
      if (f.consumed == 0 || f.consumed > m - at) bad("a frame past the bytes read");
      if (f.payload < b + at || f.payload + f.length > b + at + f.consumed) bad("a payload outside its frame");
      volatile uint8_t x = 0; for (size_t i = 0; i < f.length; i++) x ^= f.payload[i];
      (void)x;
      at += f.consumed;
    }
  } else {
    size_t at = 0, off = 0, len = 0;
    int r;
    while ((r = wt_frame_next(b, m, at, MAX, &off, &len)) == 1) {
      if (len > MAX) bad("a frame past its limit");
      if (off < at + 2 || off > m || len > m - off) bad("a frame past the bytes read");
      volatile uint8_t x = 0; for (size_t i = 0; i < len; i++) x ^= b[off + i];
      (void)x;
      if (off + len <= at) bad("a frame that doesn't move on");
      at = off + len;
    }
  }
  free(b);
  return 0;
}
