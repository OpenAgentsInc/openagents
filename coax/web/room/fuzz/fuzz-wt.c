/* The room's WebTransport layer, coverage-guided (#13283): wt.c (included here, for its statics)
   under ASan and UBSan, given what a peer controls. An input is actions, each OP SEL LEN BYTES
   (LEN in sixteens of a byte when OP & 0x80):
     OP % 8  0 a UDP packet from anyone (on_packet: version negotiation, a new connection's Initial)
             1 stream bytes on one connection, in order (q_stream_data: a stream's type, HTTP/3 and its
               QPACK headers, the extended CONNECT, capsules, a WebTransport stream's session id)
             2 a QUIC DATAGRAM frame's payload (q_datagram: the quarter stream id, its session)
             3, 4, 5 a stream closed, reset, or stopped by the peer
             6 the peer acknowledged bytes of one of our streams
             7 the host's own calls (wt_send_datagram, wt_open_stream, wt_stream_send, wt_close,
               wt_drop_waiting, wt_flush, the counts) with a session the bytes name
   SEL picks the stream (SEL & 7 of the client's bidirectional 0, 4, 8, 12 and unidirectional 2, 6, 10,
   14) and its FIN (SEL & 8). The connection is a real ngtcp2 server connection whose handshake never
   finishes (its packets can't be forged), with its HTTP/3 bound as h3_setup binds it, so stream bytes
   reach wt.c as ngtcp2 would hand them: each stream's in order, none after it closed.
   Besides the sanitizers it holds the session events to wt.h: a datagram or stream event only for a
   session that was opened and not yet closed, each closed once, and none left open at the end.
   Built and run by fuzz.sh (WT_FUZZ_CERT and WT_FUZZ_KEY: a certificate from wt-cert.sh). */
#include "../wt.c"

#define IDS 8
static const int64_t ids[IDS] = {0, 4, 8, 12, 2, 6, 10, 14};
static wt_server *srv;
static bool live[WT_SESSIONS];
static int opened, ended;
static struct sockaddr_storage from; static socklen_t from_len;

static void bad(const char *why) { fprintf(stderr, "fuzz-wt: %s\n", why); abort(); }
static bool ev_session(void *u, int sid, const char *path, const char *authority, const char *origin) {
  (void)u; (void)authority; (void)origin;
  if (sid < 0 || sid >= WT_SESSIONS) bad("a session id out of range");
  if (live[sid]) bad("a session opened twice");
  if (path[0] != '/') return false;   /* the room refuses paths outside /api/room/ (room-wt.c) */
  live[sid] = true; opened++;
  return true;
}
static void ev_datagram(void *u, int sid, const uint8_t *d, size_t n) {
  (void)u;
  if (sid < 0 || sid >= WT_SESSIONS || !live[sid]) bad("a datagram for a session not open");
  volatile uint8_t x = 0; for (size_t i = 0; i < n; i++) x ^= d[i];   /* every byte readable */
  (void)x;
}
static void ev_stream(void *u, int sid, int64_t stream, const uint8_t *d, size_t n, bool fin) {
  (void)u; (void)stream; (void)fin;
  if (sid < 0 || sid >= WT_SESSIONS || !live[sid]) bad("stream bytes for a session not open");
  volatile uint8_t x = 0; for (size_t i = 0; i < n; i++) x ^= d[i];
  (void)x;
}
static void ev_closed(void *u, int sid) {
  (void)u;
  if (sid < 0 || sid >= WT_SESSIONS || !live[sid]) bad("a session closed that was not open");
  live[sid] = false; ended++;
}

int LLVMFuzzerInitialize(int *argc, char ***argv) {
  (void)argc; (void)argv;
  const char *cert = getenv("WT_FUZZ_CERT"), *key = getenv("WT_FUZZ_KEY");
  if (!cert || !key) bad("WT_FUZZ_CERT and WT_FUZZ_KEY name a certificate and its key (fuzz.sh makes them)");
  static const wt_events ev = {ev_session, ev_datagram, ev_stream, ev_closed};
  srv = wt_listen("127.0.0.1", 0, cert, key, &ev, NULL);
  if (!srv) bad("no listener");
  close(srv->fd); srv->fd = -1;   /* nothing is sent anywhere: every send fails at once */
  struct sockaddr_in *a = (struct sockaddr_in *)&from;
  a->sin_family = AF_INET; a->sin_port = htons(9); inet_pton(AF_INET, "127.0.0.1", &a->sin_addr);
  from_len = sizeof *a;
  return 0;
}

static qconn *peer(void) {
  ngtcp2_pkt_hd hd = {0};
  hd.version = NGTCP2_PROTO_VER_V1;
  hd.dcid.datalen = 8; memset(hd.dcid.data, 0xd1, 8);
  hd.scid.datalen = 8; memset(hd.scid.data, 0x5c, 8);
  qconn *c = conn_new(srv, &hd, &from, from_len);
  if (!c) bad("no connection");
  if (h3_new(c) || nghttp3_conn_bind_control_stream(c->h3, 3) || nghttp3_conn_bind_qpack_streams(c->h3, 7, 11)) bad("no HTTP/3");
  return c;
}

/* wt_flush frees a connection once it is dead, so the pointer here may be gone after any step */
static bool conn_live(const qconn *c) {
  for (int i = 0; i < srv->nconns; i++) if (srv->conns[i] == c) return true;
  return false;
}

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t n) {
  if (n > 65536) return 0;
  qconn *c = peer();
  uint64_t off[IDS] = {0}; bool closed[IDS] = {0};
  bool broken = false;   /* a callback failed: ngtcp2 would close the connection, so it hears no more */
  size_t at = 0;
  while (at + 3 <= n) {
    int op = data[at], sel = data[at + 1];
    size_t len = data[at + 2]; if (op & 0x80) len *= 16;
    at += 3;
    if (len > n - at) len = n - at;
    uint8_t *b = malloc(len ? len : 1);   /* its own copy, exactly len bytes: ASan sees a read past it */
    memcpy(b, data + at, len); at += len;
    int k = sel & 7; int64_t id = ids[k]; bool fin = sel & 8;
    uint64_t word = 0; for (size_t i = 0; i < len && i < 8; i++) word = word << 8 | b[i];
    switch (op & 7) {
    case 0: on_packet(srv, b, len, &from, from_len); break;
    case 1:
      if (broken || closed[k]) break;
      if (q_stream_data(c->conn, fin ? NGTCP2_STREAM_DATA_FLAG_FIN : 0, id, off[k], b, len, c, NULL)) broken = true;
      off[k] += len;
      if (fin) closed[k] = true;   /* nothing after a FIN; ngtcp2 closes it when both sides are done */
      break;
    case 2: if (!broken && q_datagram(c->conn, 0, b, len, c)) broken = true; break;
    case 3:
      if (broken || closed[k]) break;
      if (q_stream_close(c->conn, (sel & 16) ? NGTCP2_STREAM_CLOSE_FLAG_APP_ERROR_CODE_SET : 0, id, word, c, NULL)) broken = true;
      closed[k] = true;
      break;
    case 4: if (!broken && !closed[k] && q_stream_reset(c->conn, id, off[k], word, c, NULL)) broken = true; break;
    case 5: if (!broken && !closed[k] && q_stop_sending(c->conn, id, word, c, NULL)) broken = true; break;
    case 6: {
      /* only what was sent: ngtcp2 never acknowledges past it */
      wt_strm *t = broken ? NULL : strm_get(c, id, false);
      if (!broken && t && t->kind == K_WT && t->sent > t->acked && q_acked(c->conn, id, t->acked, (word % (t->sent - t->acked)) + 1, c, NULL)) broken = true;
      break;
    }
    case 7: {
      int sid = (int)(word % (WT_SESSIONS + 2)) - 1;   /* -1 and WT_SESSIONS too: the host's own range checks */
      switch (sel % 9) {
      case 0: wt_send_datagram(srv, sid, b, len); break;
      case 1: wt_open_stream(srv, sid, sel & 16); break;
      case 2: wt_stream_send(srv, sid, ids[(sel >> 4) & 7] + ((sel & 128) ? 1 : 0), b, len, fin); break;
      case 3: wt_close(srv, sid, (uint32_t)word); break;
      case 4: wt_drop_waiting(srv, sid); break;
      case 5: wt_flush(srv); break;
      case 6: (void)wt_datagrams_waiting(srv, sid); (void)wt_stream_waiting(srv, sid); break;
      case 7: (void)wt_rtt_ms(srv, sid); (void)wt_session_lost(srv, sid); (void)wt_timeout_ms(srv); break;
      default: (void)wt_get_stats(srv); break;
      }
      break;
    }
    }
    free(b);
    if (c && !conn_live(c)) { c = NULL; broken = true; }   /* freed by wt_flush: only the host's calls go on */
    if (c && c->dead) broken = true;   /* closed for an error: ngtcp2 hands it nothing more */
  }
  /* every connection goes: each of its sessions ends once */
  while (srv->nconns) conn_free(srv, srv->conns[--srv->nconns]);
  for (int i = 0; i < WT_SESSIONS; i++) if (live[i]) bad("a session left open after its connection went");
  memset(srv->sess, 0, sizeof srv->sess);
  srv->ncids = 0;
  return 0;
}
