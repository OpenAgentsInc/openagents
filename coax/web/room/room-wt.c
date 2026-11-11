/* The room host's WebTransport (#13176), included by room-host.c: the transport under aethyr/net's
   interface for pages that open a session (host.js, "wt+" URLs). One UDP socket on the same port
   number as the WebSocket's TCP one (wt.c, the session layer over the vendored QUIC stack). Each
   session is one of conns[], marked wt, with the WebSocket's join, seat and frames:
     - its join is the CONNECT's path, as a WebSocket's request line (rm_join);
     - what it sends: datagrams, each frames '\n' apart (rm_wt_heard lets each repeated input through
       once), and one reliable stream of frames, each a 2-byte big-endian length and its bytes;
     - what it is sent: each frame on the channel room-wt.coax names (rm_wt_channel). A datagram as it
       is; a reliable frame on its domain's own stream (opened at its first), length-first as above;
     - stale state is dropped at the sender: each tick's state replaces whatever of the last tick's
       still waits for the congestion window (wt_drop_waiting), counted per session;
     - each second a session hears "nq STALE WAITING LOST" on its session stream, for the ?net overlay.
   Off unless the host is given a certificate: --wt CERT KEY (ECDSA P-256, PKCS#8), or OA_ROOM_WT_CERT
   and OA_ROOM_WT_KEY; OA_ROOM_WT_PORT a UDP port of its own (default the TCP port). */
#include "wt.h"
#define WT_CHANNELS 5           /* aethyr/net/transport.coax NET_CHANNELS: 0 the datagram, 1-4 the streams */
#define WT_SESSION_STREAM 4     /* NET_SESSION */
static wt_server *wts;
static int wt_conn_of[WT_SESSIONS];   /* session -> conn + 1 (0 none) */
static int64_t wt_oversize, wt_refused;
static int wt_sid(int i) { return conns[i].wt - 1; }

/* One frame onto conn i's stream for channel ch (opened at its first frame), its length first. */
static void wt_stream_frame(int i, int ch, const uint8_t *text, size_t n) {
  conn *c = &conns[i];
  if (n > 65535) { wt_oversize++; return; }
  if (!c->wt_stream[ch]) {
    int64_t id = wt_open_stream(wts, wt_sid(i), false);
    if (id < 0) { c->doomed = true; return; }
    c->wt_stream[ch] = id + 1;
  }
  uint8_t head[2] = {(uint8_t)(n >> 8), (uint8_t)n};
  if (wt_stream_send(wts, wt_sid(i), c->wt_stream[ch] - 1, head, 2, false) || wt_stream_send(wts, wt_sid(i), c->wt_stream[ch] - 1, text, n, false)) { c->doomed = true; return; }
  if (wt_stream_waiting(wts, wt_sid(i)) > OUT_MAX) { c->doomed = true; slow_closed++; }
}
/* Frame text to WebTransport conn i, on its channel. */
static void wt_out(int i, const uint8_t *text, size_t n) {
  if (conns[i].fd < 0 || conns[i].doomed) return;
  int ch = (int)rm_wt_channel(view(), (coax_string){(const char *)text, (int64_t)n});
  if (ch < 0 || ch >= WT_CHANNELS) ch = WT_SESSION_STREAM;
  if (ch == 0 && n > WT_DATAGRAM_MAX) { wt_oversize++; ch = WT_SESSION_STREAM; }   /* never cut: whole on the stream */
  if (ch == 0) { if (wt_send_datagram(wts, wt_sid(i), text, n) == 0) conns[i].wt_dg++; return; }
  wt_stream_frame(i, ch, text, n);
}

static bool wt_session(void *user, int sid, const char *path, const char *authority, const char *origin) {
  (void)user; (void)origin;
  char req[1200];
  int rn = snprintf(req, sizeof req, "GET %s HTTP/1.1\r\nHost: %s\r\n\r\n", path, authority);
  if (rn <= 0 || (size_t)rn >= sizeof req || strncmp(path, "/api/room/", 10)) { wt_refused++; return false; }
  coax_string r = {req, rn};
  if (!rm_work_request_ready(r, rooms_warmed)) { wt_refused++; return false; }
  int slot = -1;
  for (int i = 0; i < CONNS; i++) if (conns[i].fd < 0) { slot = i; break; }
  if (slot < 0) { rm_st_refuse_more(view(), 0); wt_refused++; return false; }
  int fd = dup(wt_fd(wts));   /* a real descriptor, so every conns[].fd >= 0 test holds; never polled */
  if (fd < 0) { wt_refused++; return false; }
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  conns[slot] = (conn){.fd = fd, .heard = now_ms(), .ws = true, .wt = sid + 1};
  rm_wt_conn(view(), slot, true);
  if (rm_join(view(), slot, r) < 0) {
    rm_wt_conn(view(), slot, false); close(fd); conns[slot] = (conn){.fd = -1}; wt_refused++;
    return false;
  }
  wt_conn_of[sid] = slot + 1;
  st_joined(slot, now_us());
  printf("room: WebTransport session %d is conn %d\n", sid, slot);
  return true;
}
static void wt_datagram(void *user, int sid, const uint8_t *data, size_t n) {
  (void)user;
  int i = wt_conn_of[sid] - 1;
  if (i < 0 || conns[i].doomed) return;
  conns[i].heard = now_ms();
  st_frame_in(data, n, n);
  rm_wt_heard(view(), i, (coax_string){(const char *)data, (int64_t)n});
  if (rm_relay_kicked(view(), i)) conns[i].doomed = true;
}
/* The page's reliable frames: its first stream's bytes, length-first (another stream of its is closed). */
static void wt_stream_in(void *user, int sid, int64_t stream, const uint8_t *data, size_t n, bool fin) {
  (void)user; (void)fin;
  int i = wt_conn_of[sid] - 1;
  if (i < 0 || conns[i].doomed) return;
  conn *c = &conns[i];
  if (!c->wt_in) c->wt_in = stream + 1;
  if (c->wt_in != stream + 1) return;
  c->heard = now_ms();
  if (c->n + n > IN_CAP) { c->doomed = true; return; }
  memcpy(c->in + c->n, data, n); c->n += n;
  size_t at = 0;
  while (c->n - at >= 2) {
    size_t len = ((size_t)c->in[at] << 8) | c->in[at + 1];
    if (len > 4096) { c->doomed = true; return; }
    if (c->n - at - 2 < len) break;
    st_frame_in(c->in + at + 2, len, len + 2);
    rm_heard(view(), i, (coax_string){(const char *)c->in + at + 2, (int64_t)len});
    at += 2 + len;
    if (rm_relay_kicked(view(), i)) { c->doomed = true; return; }
  }
  memmove(c->in, c->in + at, c->n - at); c->n -= at;
}
static void wt_closed(void *user, int sid) {
  (void)user;
  int i = wt_conn_of[sid] - 1;
  wt_conn_of[sid] = 0;
  if (i >= 0 && conns[i].wt == sid + 1) { conns[i].wt_gone = true; conns[i].doomed = true; }
}
/* Before shut(i) forgets conn i: its session ends (when it hasn't), and it is no longer WebTransport. */
static void wt_forget(int i) {
  if (!conns[i].wt) return;
  int sid = wt_sid(i);
  if (wt_conn_of[sid] == i + 1) wt_conn_of[sid] = 0;
  conns[i].wt = 0;
  if (!conns[i].wt_gone) wt_close(wts, sid, 0);
  rm_wt_conn(view(), i, false);
}
/* A new tick's state is coming: what of the last still waits is stale (counted). */
static void wt_tick_begin(void) {
  if (!wts) return;
  for (int j = 0; j < CONNS; j++) {
    if (conns[j].fd < 0 || !conns[j].wt) continue;
    conns[j].wt_stale += wt_drop_waiting(wts, wt_sid(j));
    rm_conn_waiting(view(), j, (int64_t)wt_stream_waiting(wts, wt_sid(j)));
  }
}
/* Each second: every session's count of stale drops, datagrams waiting and lost, on its session stream. */
static void wt_note(void) {
  if (!wts) return;
  for (int j = 0; j < CONNS; j++) {
    if (conns[j].fd < 0 || !conns[j].wt || conns[j].doomed) continue;
    char t[96]; int64_t lost = wt_session_lost(wts, wt_sid(j));
    int n = snprintf(t, sizeof t, "nq %lld %d %lld", (long long)conns[j].wt_stale, wt_datagrams_waiting(wts, wt_sid(j)), (long long)(lost < 0 ? 0 : lost));
    if (n > 0 && (size_t)n < sizeof t) wt_stream_frame(j, WT_SESSION_STREAM, (const uint8_t *)t, (size_t)n);
  }
}
static int wt_rtt(int i) { return wts && conns[i].wt ? wt_rtt_ms(wts, wt_sid(i)) : -1; }
static bool wt_start(const char *address, int port, const char *cert, const char *key) {
  static const wt_events ev = {wt_session, wt_datagram, wt_stream_in, wt_closed};
  wts = wt_listen(address, port, cert, key, &ev, NULL);
  if (!wts) { fprintf(stderr, "room: WebTransport: no listener on udp %s:%d (certificate %s)\n", address, port, cert); return false; }
  printf("room: WebTransport on udp %s:%d\n", address, port);
  return true;
}
