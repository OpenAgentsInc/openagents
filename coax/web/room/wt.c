/* The room's WebTransport server (#13175): see wt.h. QUIC, HTTP/3 and TLS are the vendored
   libraries' (coax/web/room/third_party/); the WebTransport session layer is this file's:
   - SETTINGS: nghttp3's ENABLE_CONNECT_PROTOCOL and H3_DATAGRAM, and (third_party/PATCHES.md) the
     WebTransport settings Chrome reads;
   - extended CONNECT (RFC 9220): a request with :method CONNECT and :protocol webtransport opens a
     session, whose id is its stream's id; the 200 response leaves the stream open, and its body is
     the capsule protocol (RFC 9297), where CLOSE_WEBTRANSPORT_SESSION ends the session;
   - datagrams: a QUIC DATAGRAM frame (RFC 9221) whose payload starts with the session's quarter
     stream id (RFC 9297), routed to that session;
   - streams: a peer stream that starts 0x41 (bidirectional) or 0x54 (unidirectional) and a session
     id belongs to that session, and nghttp3 never sees it; every other stream is HTTP/3's.
   Every stream and connection-level byte the session layer takes is credited back to QUIC's flow
   control at once (it buffers nothing the host has not been handed). */
#define _POSIX_C_SOURCE 200809L
#include "wt.h"
#include <ngtcp2/ngtcp2.h>
#include <ngtcp2/ngtcp2_crypto.h>
#include <ngtcp2/ngtcp2_crypto_picotls.h>
#include <nghttp3/nghttp3.h>
#include <picotls.h>
#include <picotls/minicrypto.h>
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define SCID_LEN 18
#define CIDS 8192
#define DQ 64                     /* datagrams waiting per connection */
#define CONNS_MAX 1024
#define PKT_MAX 1452              /* the UDP payload we send at most (an Ethernet path's IPv4 or IPv6) */
#define CAP_CLOSE 0x2843          /* CLOSE_WEBTRANSPORT_SESSION */
#define WT_STREAM_REJECTED 0x3994bd84ULL   /* WT_BUFFERED_STREAM_REJECTED */
enum { K_NEW, K_H3, K_WT };

typedef struct wt_strm {
  int64_t id; int kind; int sid;
  uint8_t pre[16]; size_t pre_n;          /* a new peer stream's first bytes, until its type is known */
  uint8_t *out; size_t out_n, out_cap, sent;   /* bytes of ours not yet acknowledged; sent of them handed to QUIC */
  bool fin_want, fin_sent, blocked, ended;
  char method[16], protocol[32], path[512], authority[256], origin[256];
  struct wt_strm *next;
} wt_strm;

typedef struct qconn qconn;
typedef struct { int sid; size_t n; uint8_t b[WT_DATAGRAM_MAX + 8]; int tries; } dgram;
struct qconn {
  wt_server *s; ngtcp2_conn *conn; nghttp3_conn *h3;
  ngtcp2_crypto_conn_ref ref; ngtcp2_crypto_picotls_ctx cptls; ptls_raw_extension_t exts[2];
  struct sockaddr_storage local, remote; socklen_t local_len, remote_len;
  wt_strm *streams;
  dgram *dq; int dq_head, dq_n; uint64_t dgram_id;
  bool dead;
};
typedef struct {
  bool live, closing; qconn *c; int64_t connect;   /* the CONNECT stream (the session id) */
  uint8_t cin[64]; size_t cin_n;                   /* capsule bytes heard, a capsule's head at most */
  uint64_t skip;                                   /* capsule body bytes still to pass over */
  uint8_t cap[16]; size_t cap_n, cap_given;        /* our capsules (CLOSE), handed to nghttp3 */
} session;
typedef struct { ngtcp2_cid cid; qconn *c; } cident;
struct wt_server {
  int fd; wt_events ev; void *user;
  struct sockaddr_storage local; socklen_t local_len;
  ptls_context_t tls; ptls_on_client_hello_t hello;
  qconn *conns[CONNS_MAX]; int nconns;
  cident cids[CIDS]; int ncids;
  session sess[WT_SESSIONS];
  uint8_t reset_secret[32];
  wt_stats st;
};

static ngtcp2_tstamp ts_now(void) {
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  return (ngtcp2_tstamp)t.tv_sec * NGTCP2_SECONDS + (ngtcp2_tstamp)t.tv_nsec;
}
static void rnd(uint8_t *p, size_t n) { ptls_minicrypto_random_bytes(p, n); }

/* QUIC variable-length integers (RFC 9000 16) */
static size_t vdec(const uint8_t *p, size_t n, uint64_t *v) {
  if (!n) return 0;
  size_t len = (size_t)1 << (p[0] >> 6);
  if (n < len) return 0;
  uint64_t x = p[0] & 0x3f;
  for (size_t i = 1; i < len; i++) x = (x << 8) | p[i];
  *v = x; return len;
}
static size_t venc(uint8_t *p, uint64_t v) {
  if (v < 64) { p[0] = (uint8_t)v; return 1; }
  if (v < 16384) { p[0] = (uint8_t)(0x40 | (v >> 8)); p[1] = (uint8_t)v; return 2; }
  if (v < 1073741824) { p[0] = (uint8_t)(0x80 | (v >> 24)); p[1] = (uint8_t)(v >> 16); p[2] = (uint8_t)(v >> 8); p[3] = (uint8_t)v; return 4; }
  p[0] = (uint8_t)(0xc0 | (v >> 56)); for (int i = 1; i < 8; i++) p[i] = (uint8_t)(v >> (8 * (7 - i)));
  return 8;
}

/* ---- connection ids */
static void cid_add(wt_server *s, const ngtcp2_cid *cid, qconn *c) {
  if (s->ncids < CIDS) s->cids[s->ncids++] = (cident){*cid, c};
}
static void cid_remove(wt_server *s, const ngtcp2_cid *cid) {
  for (int i = 0; i < s->ncids; i++)
    if (ngtcp2_cid_eq(&s->cids[i].cid, cid)) { s->cids[i] = s->cids[--s->ncids]; return; }
}
static qconn *cid_find(wt_server *s, const uint8_t *id, size_t n) {
  for (int i = 0; i < s->ncids; i++)
    if (s->cids[i].cid.datalen == n && !memcmp(s->cids[i].cid.data, id, n)) return s->cids[i].c;
  return NULL;
}

/* ---- streams and sessions */
static wt_strm *strm_get(qconn *c, int64_t id, bool make) {
  for (wt_strm *t = c->streams; t; t = t->next) if (t->id == id) return t;
  if (!make) return NULL;
  wt_strm *t = calloc(1, sizeof *t);
  if (!t) return NULL;
  t->id = id; t->sid = -1; t->next = c->streams; c->streams = t;
  return t;
}
static void strm_free(qconn *c, int64_t id) {
  for (wt_strm **p = &c->streams; *p; p = &(*p)->next)
    if ((*p)->id == id) { wt_strm *t = *p; *p = t->next; free(t->out); free(t); return; }
}
static int session_of(qconn *c, int64_t connect) {
  for (int i = 0; i < WT_SESSIONS; i++) if (c->s->sess[i].live && c->s->sess[i].c == c && c->s->sess[i].connect == connect) return i;
  return -1;
}
static void session_end(wt_server *s, int sid) {
  session *x = &s->sess[sid];
  if (!x->live) return;
  qconn *c = x->c;
  x->live = false;
  for (wt_strm *t = c->streams; t; t = t->next) if (t->sid == sid && t->kind == K_WT) t->sid = -1;
  if (s->ev.closed) s->ev.closed(s->user, sid);
}
/* Credits n bytes of stream id back to the peer (the stream's window and the connection's). */
static void consumed(qconn *c, int64_t id, size_t n) {
  if (!n) return;
  ngtcp2_conn_extend_max_stream_offset(c->conn, id, n);
  ngtcp2_conn_extend_max_offset(c->conn, n);
}

/* ---- HTTP/3 (nghttp3's callbacks) */
static int h3_acked(nghttp3_conn *h, int64_t id, uint64_t n, void *u, void *su) { (void)h; (void)id; (void)n; (void)u; (void)su; return 0; }
static int h3_stream_close(nghttp3_conn *h, int64_t id, uint64_t code, void *u, void *su) {
  (void)h; (void)code; (void)su; qconn *c = u;
  int sid = session_of(c, id);
  if (sid >= 0) session_end(c->s, sid);
  return 0;
}
/* The CONNECT stream's body: capsules. A CLOSE_WEBTRANSPORT_SESSION ends the session; others pass. */
static void capsules(qconn *c, int sid, const uint8_t *d, size_t n) {
  session *x = &c->s->sess[sid];
  while (n) {
    if (x->skip) { size_t k = n < x->skip ? n : (size_t)x->skip; x->skip -= k; d += k; n -= k; continue; }
    size_t k = sizeof x->cin - x->cin_n; if (k > n) k = n;
    memcpy(x->cin + x->cin_n, d, k); x->cin_n += k; d += k; n -= k;
    uint64_t type, len; size_t a = vdec(x->cin, x->cin_n, &type), b = a ? vdec(x->cin + a, x->cin_n - a, &len) : 0;
    if (!a || !b) { if (x->cin_n == sizeof x->cin) x->cin_n = 0; continue; }
    if (type == CAP_CLOSE) { session_end(c->s, sid); return; }
    size_t have = x->cin_n - a - b;
    if (have >= len) { size_t rest = have - (size_t)len; memmove(x->cin, x->cin + a + b + len, rest); x->cin_n = rest; }
    else { x->skip = len - have; x->cin_n = 0; }
  }
}
static int h3_recv_data(nghttp3_conn *h, int64_t id, const uint8_t *d, size_t n, void *u, void *su) {
  (void)h; (void)su; qconn *c = u;
  consumed(c, id, n);
  int sid = session_of(c, id);
  if (sid >= 0) capsules(c, sid, d, n);
  return 0;
}
static int h3_deferred(nghttp3_conn *h, int64_t id, size_t n, void *u, void *su) { (void)h; (void)su; consumed(u, id, n); return 0; }
static void copy_value(char *out, size_t cap, nghttp3_vec v) { size_t k = v.len < cap - 1 ? v.len : cap - 1; memcpy(out, v.base, k); out[k] = 0; }
static int h3_header(nghttp3_conn *h, int64_t id, int32_t token, nghttp3_rcbuf *name, nghttp3_rcbuf *value, uint8_t flags, void *u, void *su) {
  (void)h; (void)token; (void)flags; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, true);
  if (!t) return 0;
  nghttp3_vec nv = nghttp3_rcbuf_get_buf(name), vv = nghttp3_rcbuf_get_buf(value);
  #define IS(lit) (nv.len == sizeof lit - 1 && !memcmp(nv.base, lit, sizeof lit - 1))
  if (IS(":method")) copy_value(t->method, sizeof t->method, vv);
  else if (IS(":protocol")) copy_value(t->protocol, sizeof t->protocol, vv);
  else if (IS(":path")) copy_value(t->path, sizeof t->path, vv);
  else if (IS(":authority")) copy_value(t->authority, sizeof t->authority, vv);
  else if (IS("origin")) copy_value(t->origin, sizeof t->origin, vv);
  #undef IS
  return 0;
}
static nghttp3_ssize read_connect(nghttp3_conn *h, int64_t id, nghttp3_vec *vec, size_t veccnt, uint32_t *pflags, void *u, void *su) {
  (void)h; (void)su; qconn *c = u;
  for (int i = 0; i < WT_SESSIONS; i++) {
    session *x = &c->s->sess[i];
    if (x->c != c || x->connect != id || (!x->live && !x->closing)) continue;
    if (x->cap_given < x->cap_n && veccnt) {
      vec[0].base = x->cap + x->cap_given; vec[0].len = x->cap_n - x->cap_given; x->cap_given = x->cap_n;
      if (x->closing) *pflags |= NGHTTP3_DATA_FLAG_EOF;
      return 1;
    }
    if (x->closing) { *pflags |= NGHTTP3_DATA_FLAG_EOF; return 0; }
    return NGHTTP3_ERR_WOULDBLOCK;
  }
  *pflags |= NGHTTP3_DATA_FLAG_EOF;
  return 0;
}
static int h3_end_headers(nghttp3_conn *h, int64_t id, int fin, void *u, void *su) {
  (void)fin; (void)su; qconn *c = u; wt_server *s = c->s;
  wt_strm *t = strm_get(c, id, true);
  if (!t) return NGHTTP3_ERR_CALLBACK_FAILURE;
  bool wt = !strcmp(t->method, "CONNECT") && !strcmp(t->protocol, "webtransport");
  int sid = -1;
  if (wt) for (int i = 0; i < WT_SESSIONS; i++) if (!s->sess[i].live && !s->sess[i].closing) { sid = i; break; }
  if (sid >= 0) {
    s->sess[sid] = (session){.live = true, .c = c, .connect = id};
    if (s->ev.session && !s->ev.session(s->user, sid, t->path, t->authority, t->origin)) { s->sess[sid].live = false; sid = -1; }
  }
  if (sid >= 0) {
    s->st.sessions++;
    nghttp3_nv nva[] = {
      {(uint8_t *)":status", (uint8_t *)"200", 7, 3, NGHTTP3_NV_FLAG_NONE},
      {(uint8_t *)"sec-webtransport-http3-draft", (uint8_t *)"draft02", 28, 7, NGHTTP3_NV_FLAG_NONE},
    };
    nghttp3_data_reader dr = {read_connect};
    return nghttp3_conn_submit_response(h, id, nva, 2, &dr) ? NGHTTP3_ERR_CALLBACK_FAILURE : 0;
  }
  nghttp3_nv nva[] = {{(uint8_t *)":status", (uint8_t *)(wt ? "404" : "421"), 7, 3, NGHTTP3_NV_FLAG_NONE}};
  return nghttp3_conn_submit_response(h, id, nva, 1, NULL) ? NGHTTP3_ERR_CALLBACK_FAILURE : 0;
}
static int h3_end_stream(nghttp3_conn *h, int64_t id, void *u, void *su) {
  (void)h; (void)su; qconn *c = u;
  int sid = session_of(c, id);
  if (sid >= 0) session_end(c->s, sid);   /* the peer finished the CONNECT stream: the session is over */
  return 0;
}
static int h3_stop_sending(nghttp3_conn *h, int64_t id, uint64_t code, void *u, void *su) { (void)h; (void)su; ngtcp2_conn_shutdown_stream_read(((qconn *)u)->conn, 0, id, code); return 0; }
static int h3_reset_stream(nghttp3_conn *h, int64_t id, uint64_t code, void *u, void *su) { (void)h; (void)su; ngtcp2_conn_shutdown_stream_write(((qconn *)u)->conn, 0, id, code); return 0; }

static int h3_setup(qconn *c) {
  if (c->h3) return 0;
  nghttp3_callbacks cb = {
    .acked_stream_data = h3_acked, .stream_close = h3_stream_close, .recv_data = h3_recv_data,
    .deferred_consume = h3_deferred, .recv_header = h3_header, .end_headers = h3_end_headers,
    .stop_sending = h3_stop_sending, .end_stream = h3_end_stream, .reset_stream = h3_reset_stream,
  };
  nghttp3_settings st; nghttp3_settings_default(&st);
  st.qpack_max_dtable_capacity = 4096; st.qpack_blocked_streams = 100;
  st.enable_connect_protocol = 1; st.h3_datagram = 1;
  if (nghttp3_conn_server_new(&c->h3, &cb, &st, NULL, c)) return -1;
  const ngtcp2_transport_params *lp = ngtcp2_conn_get_local_transport_params(c->conn);
  nghttp3_conn_set_max_client_streams_bidi(c->h3, lp->initial_max_streams_bidi);
  int64_t ctrl, enc, dec;
  if (ngtcp2_conn_open_uni_stream(c->conn, &ctrl, NULL) || nghttp3_conn_bind_control_stream(c->h3, ctrl)) return -1;
  if (ngtcp2_conn_open_uni_stream(c->conn, &enc, NULL) || ngtcp2_conn_open_uni_stream(c->conn, &dec, NULL)) return -1;
  if (nghttp3_conn_bind_qpack_streams(c->h3, enc, dec)) return -1;
  int64_t ours[3] = {ctrl, enc, dec};
  for (int i = 0; i < 3; i++) { wt_strm *t = strm_get(c, ours[i], true); if (t) t->kind = K_H3; }
  return 0;
}

/* ---- QUIC (ngtcp2's callbacks) */
static ngtcp2_conn *get_conn(ngtcp2_crypto_conn_ref *ref) { return ((qconn *)ref->user_data)->conn; }
static void q_rand(uint8_t *p, size_t n, const ngtcp2_rand_ctx *ctx) { (void)ctx; rnd(p, n); }
static int q_new_cid(ngtcp2_conn *qc, ngtcp2_cid *cid, uint8_t *token, size_t len, void *u) {
  (void)qc; qconn *c = u;
  rnd(cid->data, len); cid->datalen = len;
  if (ngtcp2_crypto_generate_stateless_reset_token(token, c->s->reset_secret, sizeof c->s->reset_secret, cid)) return NGTCP2_ERR_CALLBACK_FAILURE;
  cid_add(c->s, cid, c);
  return 0;
}
static int q_remove_cid(ngtcp2_conn *qc, const ngtcp2_cid *cid, void *u) { (void)qc; cid_remove(((qconn *)u)->s, cid); return 0; }
static int q_handshake_done(ngtcp2_conn *qc, void *u) { (void)qc; return h3_setup(u) ? NGTCP2_ERR_CALLBACK_FAILURE : 0; }
static void deliver(qconn *c, wt_strm *t, const uint8_t *d, size_t n, bool fin) {
  if (t->sid >= 0 && c->s->sess[t->sid].live && c->s->ev.stream && (n || fin)) c->s->ev.stream(c->s->user, t->sid, t->id, d, n, fin);
  if (fin) t->ended = true;
}
static int h3_read(qconn *c, int64_t id, const uint8_t *d, size_t n, bool fin) {
  if (!c->h3 && h3_setup(c)) return -1;
  nghttp3_ssize k = nghttp3_conn_read_stream(c->h3, id, d, n, fin);
  if (k < 0) return -1;
  consumed(c, id, (size_t)k);
  return 0;
}
static int q_stream_data(ngtcp2_conn *qc, uint32_t flags, int64_t id, uint64_t off, const uint8_t *d, size_t n, void *u, void *su) {
  (void)off; (void)su; qconn *c = u;
  bool fin = flags & NGTCP2_STREAM_DATA_FLAG_FIN;
  wt_strm *t = strm_get(c, id, true);
  if (!t) return NGTCP2_ERR_CALLBACK_FAILURE;
  if (t->kind == K_H3) return h3_read(c, id, d, n, fin) ? NGTCP2_ERR_CALLBACK_FAILURE : 0;
  if (t->kind == K_WT) { consumed(c, id, n); deliver(c, t, d, n, fin); return 0; }
  /* a new stream of the peer's: its type says whose it is */
  size_t take = sizeof t->pre - t->pre_n; if (take > n) take = n;
  memcpy(t->pre + t->pre_n, d, take); t->pre_n += take; d += take; n -= take;
  uint64_t type, sess = 0;
  size_t a = vdec(t->pre, t->pre_n, &type), b = 0;
  bool bidi = !(id & 2);
  bool wt = a && ((bidi && type == 0x41) || (!bidi && type == 0x54));
  if (wt) b = vdec(t->pre + a, t->pre_n - a, &sess);
  if ((!a || (wt && !b)) && !fin && t->pre_n < sizeof t->pre) { consumed(c, id, take); return 0; }   /* more to come */
  if (wt && b) {
    t->kind = K_WT; t->sid = session_of(c, (int64_t)sess);
    consumed(c, id, take + n);
    if (t->sid < 0) { ngtcp2_conn_shutdown_stream(qc, 0, id, WT_STREAM_REJECTED); return 0; }
    deliver(c, t, t->pre + a + b, t->pre_n - a - b, fin && !n);
    if (n) deliver(c, t, d, n, fin);
    return 0;
  }
  t->kind = K_H3;
  if (h3_read(c, id, t->pre, t->pre_n, fin && !n)) return NGTCP2_ERR_CALLBACK_FAILURE;
  if (n && h3_read(c, id, d, n, fin)) return NGTCP2_ERR_CALLBACK_FAILURE;
  return 0;
}
static int q_acked(ngtcp2_conn *qc, int64_t id, uint64_t off, uint64_t n, void *u, void *su) {
  (void)qc; (void)off; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, false);
  if (t && t->kind == K_WT) {
    size_t k = n < t->sent ? (size_t)n : t->sent;
    memmove(t->out, t->out + k, t->out_n - k); t->out_n -= k; t->sent -= k;
    return 0;
  }
  if (c->h3 && nghttp3_conn_add_ack_offset(c->h3, id, n)) return NGTCP2_ERR_CALLBACK_FAILURE;
  return 0;
}
static int q_stream_close(ngtcp2_conn *qc, uint32_t flags, int64_t id, uint64_t code, void *u, void *su) {
  (void)qc; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, false);
  if (t && t->kind == K_WT) { if (!t->ended) deliver(c, t, NULL, 0, true); strm_free(c, id); return 0; }
  if (!(flags & NGTCP2_STREAM_CLOSE_FLAG_APP_ERROR_CODE_SET)) code = NGHTTP3_H3_NO_ERROR;
  if (c->h3) { int rv = nghttp3_conn_close_stream(c->h3, id, code); if (rv && rv != NGHTTP3_ERR_STREAM_NOT_FOUND) return NGTCP2_ERR_CALLBACK_FAILURE; }
  int sid = session_of(c, id);
  if (sid >= 0) session_end(c->s, sid);
  for (int i = 0; i < WT_SESSIONS; i++) if (c->s->sess[i].c == c && c->s->sess[i].connect == id) c->s->sess[i].closing = false;
  strm_free(c, id);
  return 0;
}
static int q_stream_reset(ngtcp2_conn *qc, int64_t id, uint64_t size, uint64_t code, void *u, void *su) {
  (void)qc; (void)size; (void)code; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, false);
  if (t && t->kind == K_WT) { if (!t->ended) deliver(c, t, NULL, 0, true); return 0; }
  if (c->h3 && nghttp3_conn_shutdown_stream_read(c->h3, id)) return NGTCP2_ERR_CALLBACK_FAILURE;
  return 0;
}
static int q_stop_sending(ngtcp2_conn *qc, int64_t id, uint64_t code, void *u, void *su) {
  (void)qc; (void)code; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, false);
  if (t && t->kind == K_WT) { t->out_n = t->sent; t->fin_want = false; return 0; }
  if (c->h3 && nghttp3_conn_shutdown_stream_read(c->h3, id)) return NGTCP2_ERR_CALLBACK_FAILURE;
  return 0;
}
static int q_max_bidi(ngtcp2_conn *qc, uint64_t max, void *u) { (void)qc; qconn *c = u; if (c->h3) nghttp3_conn_set_max_client_streams_bidi(c->h3, max); return 0; }
static int q_max_stream_data(ngtcp2_conn *qc, int64_t id, uint64_t max, void *u, void *su) {
  (void)qc; (void)max; (void)su; qconn *c = u;
  wt_strm *t = strm_get(c, id, false);
  if (t && t->kind == K_WT) { t->blocked = false; return 0; }
  if (c->h3 && nghttp3_conn_unblock_stream(c->h3, id)) return NGTCP2_ERR_CALLBACK_FAILURE;
  return 0;
}
static int q_datagram(ngtcp2_conn *qc, uint32_t flags, const uint8_t *d, size_t n, void *u) {
  (void)qc; (void)flags; qconn *c = u;
  uint64_t q; size_t a = vdec(d, n, &q);
  if (!a) return 0;
  c->s->st.datagrams_in++;
  int sid = session_of(c, (int64_t)(q * 4));
  if (sid >= 0 && c->s->ev.datagram) c->s->ev.datagram(c->s->user, sid, d + a, n - a);
  return 0;
}
#include <stdarg.h>
static void q_log(void *u, const char *fmt, ...) { (void)u; va_list a; va_start(a, fmt); vfprintf(stderr, fmt, a); va_end(a); fputc('\n', stderr); }
static int q_lost_datagram(ngtcp2_conn *qc, uint64_t id, void *u) { (void)qc; (void)id; ((qconn *)u)->s->st.datagrams_lost++; return 0; }

static int on_client_hello(ptls_on_client_hello_t *self, ptls_t *tls, ptls_on_client_hello_parameters_t *p) {
  (void)self;
  for (size_t i = 0; i < p->negotiated_protocols.count; i++) {
    ptls_iovec_t v = p->negotiated_protocols.list[i];
    if (v.len == 2 && !memcmp(v.base, "h3", 2)) return ptls_set_negotiated_protocol(tls, "h3", 2) ? -1 : 0;
  }
  return PTLS_ALERT_NO_APPLICATION_PROTOCOL;
}

/* ---- the socket */
static void send_to(wt_server *s, const uint8_t *b, size_t n, const struct sockaddr *to, socklen_t tl) {
  for (;;) {
    ssize_t w = sendto(s->fd, b, n, 0, to, tl);
    if (w < 0 && errno == EINTR) continue;
    if (w > 0) { s->st.packets_out++; s->st.bytes_out += w; }
    return;   /* a full socket buffer drops the packet, as the path would: QUIC resends what must arrive */
  }
}
static void conn_close_error(qconn *c, int liberr) {
  if (c->dead) return;
  uint8_t buf[PKT_MAX]; ngtcp2_path_storage ps; ngtcp2_path_storage_zero(&ps); ngtcp2_pkt_info pi;
  ngtcp2_ccerr e; ngtcp2_ccerr_default(&e); ngtcp2_ccerr_set_liberr(&e, liberr, NULL, 0);
  ngtcp2_ssize w = ngtcp2_conn_write_connection_close(c->conn, &ps.path, &pi, buf, sizeof buf, &e, ts_now());
  if (w > 0) send_to(c->s, buf, (size_t)w, (struct sockaddr *)ps.path.remote.addr, ps.path.remote.addrlen);
  c->dead = true;
}
/* Everything connection c may send now: its waiting datagrams, then its streams' bytes (HTTP/3's
   and the sessions'), then whatever QUIC itself owes (acknowledgements, resends, probes). */
static void conn_write(qconn *c) {
  if (c->dead) return;
  wt_server *s = c->s;
  uint8_t buf[PKT_MAX]; ngtcp2_path_storage ps; ngtcp2_path_storage_zero(&ps); ngtcp2_pkt_info pi;
  ngtcp2_tstamp now = ts_now();
  while (c->dq_n) {
    dgram *g = &c->dq[c->dq_head];
    int acc = 0; ngtcp2_vec v = {g->b, g->n};
    ngtcp2_ssize w = ngtcp2_conn_writev_datagram(c->conn, &ps.path, &pi, buf, sizeof buf, &acc, 0, c->dgram_id++, &v, 1, now);
    if (w < 0) {
      if (w == NGTCP2_ERR_INVALID_ARGUMENT) { s->st.datagrams_dropped++; c->dq_head = (c->dq_head + 1) % DQ; c->dq_n--; continue; }
      conn_close_error(c, (int)w); return;
    }
    if (w == 0) break;   /* the congestion window is full: they wait (or the host drops them as stale) */
    send_to(s, buf, (size_t)w, (struct sockaddr *)ps.path.remote.addr, ps.path.remote.addrlen);
    if (acc || ++g->tries > 2) {
      if (acc) s->st.datagrams_out++; else s->st.datagrams_dropped++;
      c->dq_head = (c->dq_head + 1) % DQ; c->dq_n--;
    }
  }
  for (wt_strm *t = c->streams; t; t = t->next) t->blocked = false;
  for (int guard = 0; guard < 512; guard++) {
    int64_t sid = -1; int fin = 0; nghttp3_vec vec[16]; nghttp3_ssize cnt = 0; wt_strm *t = NULL;
    if (c->h3 && ngtcp2_conn_get_max_data_left(c->conn)) {
      cnt = nghttp3_conn_writev_stream(c->h3, &sid, &fin, vec, 16);
      if (cnt < 0) { conn_close_error(c, NGTCP2_ERR_INTERNAL); return; }
    }
    if (sid < 0) {
      for (wt_strm *x = c->streams; x; x = x->next)
        if (x->kind == K_WT && !x->blocked && (x->out_n > x->sent || (x->fin_want && !x->fin_sent))) { t = x; break; }
      if (t) { sid = t->id; vec[0].base = t->out + t->sent; vec[0].len = t->out_n - t->sent; cnt = vec[0].len ? 1 : 0; fin = t->fin_want; }
    }
    ngtcp2_ssize nd = -1;
    ngtcp2_ssize w = ngtcp2_conn_writev_stream(c->conn, &ps.path, &pi, buf, sizeof buf, &nd, fin ? NGTCP2_WRITE_STREAM_FLAG_FIN : 0, sid, (const ngtcp2_vec *)vec, (size_t)cnt, now);
    if (w < 0) {
      if (w == NGTCP2_ERR_STREAM_DATA_BLOCKED) { if (t) t->blocked = true; else if (c->h3) nghttp3_conn_block_stream(c->h3, sid); continue; }
      if (w == NGTCP2_ERR_STREAM_SHUT_WR || w == NGTCP2_ERR_STREAM_NOT_FOUND) {
        if (t) { t->out_n = t->sent; t->fin_want = false; t->blocked = true; }
        else if (c->h3) nghttp3_conn_shutdown_stream_write(c->h3, sid);
        continue;
      }
      conn_close_error(c, (int)w); return;
    }
    if (nd >= 0) {
      if (t) { t->sent += (size_t)nd; if (fin && t->sent == t->out_n) t->fin_sent = true; }
      else if (sid >= 0 && c->h3 && nghttp3_conn_add_write_offset(c->h3, sid, (size_t)nd)) { conn_close_error(c, NGTCP2_ERR_INTERNAL); return; }
    }
    if (w == 0) break;
    send_to(s, buf, (size_t)w, (struct sockaddr *)ps.path.remote.addr, ps.path.remote.addrlen);
  }
  ngtcp2_conn_update_pkt_tx_time(c->conn, now);
}

static void conn_free(wt_server *s, qconn *c) {
  for (int i = 0; i < WT_SESSIONS; i++) if (s->sess[i].c == c) { session_end(s, i); s->sess[i] = (session){0}; }
  for (int i = 0; i < s->ncids;) { if (s->cids[i].c == c) s->cids[i] = s->cids[--s->ncids]; else i++; }
  while (c->streams) { wt_strm *t = c->streams; c->streams = t->next; free(t->out); free(t); }
  if (c->h3) nghttp3_conn_del(c->h3);
  if (c->conn) ngtcp2_conn_del(c->conn);
  if (c->cptls.ptls) { ngtcp2_crypto_picotls_deconfigure_session(&c->cptls); ptls_free(c->cptls.ptls); }
  free(c->dq); free(c);
}

static qconn *conn_new(wt_server *s, const ngtcp2_pkt_hd *hd, const struct sockaddr_storage *from, socklen_t fl) {
  if (s->nconns >= CONNS_MAX) return NULL;
  qconn *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  c->dq = calloc(DQ, sizeof *c->dq);
  if (!c->dq) { free(c); return NULL; }
  c->s = s; c->remote = *from; c->remote_len = fl; c->local = s->local; c->local_len = s->local_len;
  ngtcp2_callbacks cb = {
    .recv_client_initial = ngtcp2_crypto_recv_client_initial_cb, .recv_crypto_data = ngtcp2_crypto_recv_crypto_data_cb,
    .handshake_completed = q_handshake_done, .encrypt = ngtcp2_crypto_encrypt_cb, .decrypt = ngtcp2_crypto_decrypt_cb,
    .hp_mask = ngtcp2_crypto_hp_mask_cb, .recv_stream_data = q_stream_data, .acked_stream_data_offset = q_acked,
    .stream_close = q_stream_close, .rand = q_rand, .get_new_connection_id = q_new_cid, .remove_connection_id = q_remove_cid,
    .update_key = ngtcp2_crypto_update_key_cb, .stream_reset = q_stream_reset, .extend_max_remote_streams_bidi = q_max_bidi,
    .extend_max_stream_data = q_max_stream_data, .delete_crypto_aead_ctx = ngtcp2_crypto_delete_crypto_aead_ctx_cb,
    .delete_crypto_cipher_ctx = ngtcp2_crypto_delete_crypto_cipher_ctx_cb, .recv_datagram = q_datagram,
    .lost_datagram = q_lost_datagram, .get_path_challenge_data = ngtcp2_crypto_get_path_challenge_data_cb,
    .stream_stop_sending = q_stop_sending, .version_negotiation = ngtcp2_crypto_version_negotiation_cb,
  };
  ngtcp2_settings st; ngtcp2_settings_default(&st);
  st.initial_ts = ts_now();
  st.max_tx_udp_payload_size = PKT_MAX;
  st.no_tx_udp_payload_size_shaping = 1;   /* a whole WT_DATAGRAM_MAX datagram fits one packet from the first */
  if (getenv("WT_DEBUG")) st.log_printf = q_log;
  ngtcp2_transport_params p; ngtcp2_transport_params_default(&p);
  p.initial_max_stream_data_bidi_local = 256 * 1024; p.initial_max_stream_data_bidi_remote = 256 * 1024;
  p.initial_max_stream_data_uni = 256 * 1024; p.initial_max_data = 4 * 1024 * 1024;
  p.initial_max_streams_bidi = 100; p.initial_max_streams_uni = 100;
  p.max_idle_timeout = 30 * NGTCP2_SECONDS;
  p.max_datagram_frame_size = 65535;
  p.original_dcid = hd->dcid; p.original_dcid_present = 1;
  ngtcp2_cid scid; scid.datalen = SCID_LEN; rnd(scid.data, SCID_LEN);
  p.stateless_reset_token_present = 1;
  ngtcp2_crypto_generate_stateless_reset_token(p.stateless_reset_token, s->reset_secret, sizeof s->reset_secret, &scid);
  ngtcp2_path path = {{(struct sockaddr *)&c->local, c->local_len}, {(struct sockaddr *)&c->remote, c->remote_len}, NULL};
  if (ngtcp2_conn_server_new(&c->conn, &hd->scid, &scid, &path, hd->version, &cb, &st, &p, NULL, c)) { free(c->dq); free(c); return NULL; }
  ngtcp2_crypto_picotls_ctx_init(&c->cptls);
  c->cptls.ptls = ptls_server_new(&s->tls);
  if (!c->cptls.ptls) { conn_free(s, c); return NULL; }
  c->ref.get_conn = get_conn; c->ref.user_data = c;
  *ptls_get_data_ptr(c->cptls.ptls) = &c->ref;
  c->exts[0].type = UINT16_MAX; c->exts[1].type = UINT16_MAX;
  c->cptls.handshake_properties.additional_extensions = c->exts;
  if (ngtcp2_crypto_picotls_configure_server_session(&c->cptls)) { conn_free(s, c); return NULL; }
  ngtcp2_conn_set_tls_native_handle(c->conn, &c->cptls);
  cid_add(s, &scid, c);
  cid_add(s, &hd->dcid, c);   /* the client's first DCID, for its resent Initials */
  s->conns[s->nconns++] = c;
  s->st.connections++;
  return c;
}

static void on_packet(wt_server *s, const uint8_t *d, size_t n, const struct sockaddr_storage *from, socklen_t fl) {
  ngtcp2_version_cid vc;
  int rv = ngtcp2_pkt_decode_version_cid(&vc, d, n, SCID_LEN);
  if (rv == NGTCP2_ERR_VERSION_NEGOTIATION) {
    uint8_t buf[PKT_MAX], r; rnd(&r, 1);
    uint32_t sv[] = {NGTCP2_PROTO_VER_V1};
    ngtcp2_ssize w = ngtcp2_pkt_write_version_negotiation(buf, sizeof buf, r, vc.scid, vc.scidlen, vc.dcid, vc.dcidlen, sv, 1);
    if (w > 0) send_to(s, buf, (size_t)w, (const struct sockaddr *)from, fl);
    return;
  }
  if (rv) return;
  qconn *c = cid_find(s, vc.dcid, vc.dcidlen);
  if (!c) {
    ngtcp2_pkt_hd hd;
    if (ngtcp2_accept(&hd, d, n)) return;
    c = conn_new(s, &hd, from, fl);
    if (!c) return;
  }
  if (c->dead) return;
  struct sockaddr_storage rem = *from;
  ngtcp2_path path = {{(struct sockaddr *)&c->local, c->local_len}, {(struct sockaddr *)&rem, fl}, NULL};
  ngtcp2_pkt_info pi = {0};
  rv = ngtcp2_conn_read_pkt(c->conn, &path, &pi, d, n, ts_now());
  if (!rv) return;
  if (rv == NGTCP2_ERR_DRAINING || rv == NGTCP2_ERR_DROP_CONN || rv == NGTCP2_ERR_CLOSING) { c->dead = true; return; }
  conn_close_error(c, rv);
}

wt_server *wt_listen(const char *address, int port, const char *cert_pem, const char *key_pem, const wt_events *events, void *user) {
  wt_server *s = calloc(1, sizeof *s);
  if (!s) return NULL;
  s->ev = *events; s->user = user;
  rnd(s->reset_secret, sizeof s->reset_secret);
  static ptls_key_exchange_algorithm_t *kx[] = {&ptls_minicrypto_x25519, &ptls_minicrypto_secp256r1, NULL};
  static ptls_cipher_suite_t *cs[] = {&ptls_minicrypto_aes128gcmsha256, &ptls_minicrypto_aes256gcmsha384, &ptls_minicrypto_chacha20poly1305sha256, NULL};
  s->tls = (ptls_context_t){.random_bytes = ptls_minicrypto_random_bytes, .get_time = &ptls_get_time, .key_exchanges = kx,
                            .cipher_suites = cs, .require_dhe_on_psk = 1, .server_cipher_preference = 1};
  s->hello.cb = on_client_hello; s->tls.on_client_hello = &s->hello;
  if (ngtcp2_crypto_picotls_configure_server_context(&s->tls)) { free(s); return NULL; }
  if (ptls_load_certificates(&s->tls, cert_pem)) { fprintf(stderr, "wt: can't read the certificate %s\n", cert_pem); free(s); return NULL; }
  if (ptls_minicrypto_load_private_key(&s->tls, key_pem)) { fprintf(stderr, "wt: can't read the key %s (PKCS#8 ECDSA P-256)\n", key_pem); free(s); return NULL; }
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  if (inet_pton(AF_INET, address, &a.sin_addr) != 1) { free(s); return NULL; }
  s->fd = socket(AF_INET, SOCK_DGRAM, 0);
  if (s->fd < 0) { free(s); return NULL; }
  int big = 4 << 20; setsockopt(s->fd, SOL_SOCKET, SO_RCVBUF, &big, sizeof big); setsockopt(s->fd, SOL_SOCKET, SO_SNDBUF, &big, sizeof big);
  if (bind(s->fd, (struct sockaddr *)&a, sizeof a)) { perror("wt: bind"); close(s->fd); free(s); return NULL; }
  fcntl(s->fd, F_SETFL, O_NONBLOCK); fcntl(s->fd, F_SETFD, FD_CLOEXEC);
  s->local_len = sizeof s->local; getsockname(s->fd, (struct sockaddr *)&s->local, &s->local_len);
  return s;
}
int wt_fd(const wt_server *s) { return s->fd; }
void wt_read(wt_server *s) {
  uint8_t buf[65536];
  for (int k = 0; k < 4096; k++) {
    struct sockaddr_storage from; socklen_t fl = sizeof from;
    ssize_t r = recvfrom(s->fd, buf, sizeof buf, 0, (struct sockaddr *)&from, &fl);
    if (r < 0) { if (errno == EINTR) continue; return; }
    s->st.packets_in++; s->st.bytes_in += r;
    on_packet(s, buf, (size_t)r, &from, fl);
  }
}
void wt_flush(wt_server *s) {
  ngtcp2_tstamp now = ts_now();
  for (int i = 0; i < s->nconns; i++) {
    qconn *c = s->conns[i];
    if (!c->dead && ngtcp2_conn_get_expiry(c->conn) <= now) {
      int rv = ngtcp2_conn_handle_expiry(c->conn, now);
      if (rv == NGTCP2_ERR_IDLE_CLOSE) c->dead = true;
      else if (rv) conn_close_error(c, rv);
    }
    if (!c->dead && (ngtcp2_conn_in_closing_period(c->conn) || ngtcp2_conn_in_draining_period(c->conn))) c->dead = true;
    conn_write(c);
  }
  for (int i = 0; i < s->nconns;) {
    if (s->conns[i]->dead) { conn_free(s, s->conns[i]); s->conns[i] = s->conns[--s->nconns]; } else i++;
  }
}
int wt_timeout_ms(const wt_server *s) {
  ngtcp2_tstamp now = ts_now(), soon = now + 1000 * NGTCP2_MILLISECONDS;
  for (int i = 0; i < s->nconns; i++) {
    if (s->conns[i]->dead || s->conns[i]->dq_n) return 0;
    ngtcp2_tstamp e = ngtcp2_conn_get_expiry(s->conns[i]->conn);
    if (e < soon) soon = e;
  }
  return soon <= now ? 0 : (int)((soon - now + NGTCP2_MILLISECONDS - 1) / NGTCP2_MILLISECONDS);
}
static session *live_session(const wt_server *s, int sid) {
  if (sid < 0 || sid >= WT_SESSIONS || !s->sess[sid].live || s->sess[sid].c->dead) return NULL;
  return (session *)&s->sess[sid];
}
int wt_send_datagram(wt_server *s, int sid, const uint8_t *data, size_t n) {
  session *x = live_session(s, sid);
  if (!x || n > WT_DATAGRAM_MAX || x->c->dq_n >= DQ) { s->st.datagrams_dropped++; return -1; }
  qconn *c = x->c;
  dgram *g = &c->dq[(c->dq_head + c->dq_n) % DQ];
  size_t h = venc(g->b, (uint64_t)x->connect / 4);
  memcpy(g->b + h, data, n); g->n = h + n; g->sid = sid; g->tries = 0;
  c->dq_n++;
  return 0;
}
int wt_datagrams_waiting(const wt_server *s, int sid) {
  session *x = live_session(s, sid);
  if (!x) return 0;
  int k = 0;
  for (int i = 0; i < x->c->dq_n; i++) if (x->c->dq[(x->c->dq_head + i) % DQ].sid == sid) k++;
  return k;
}
int wt_drop_waiting(wt_server *s, int sid) {
  session *x = live_session(s, sid);
  if (!x) return 0;
  qconn *c = x->c; int kept = 0, dropped = 0;
  dgram *tmp = malloc(sizeof(dgram) * DQ);
  if (!tmp) return 0;
  for (int i = 0; i < c->dq_n; i++) { dgram *g = &c->dq[(c->dq_head + i) % DQ]; if (g->sid == sid) dropped++; else tmp[kept++] = *g; }
  for (int i = 0; i < kept; i++) c->dq[i] = tmp[i];
  free(tmp);
  c->dq_head = 0; c->dq_n = kept;
  s->st.datagrams_dropped += dropped;
  return dropped;
}
int64_t wt_open_stream(wt_server *s, int sid, bool bidi) {
  session *x = live_session(s, sid);
  if (!x) return -1;
  int64_t id;
  if ((bidi ? ngtcp2_conn_open_bidi_stream(x->c->conn, &id, NULL) : ngtcp2_conn_open_uni_stream(x->c->conn, &id, NULL))) return -1;
  wt_strm *t = strm_get(x->c, id, true);
  if (!t) return -1;
  t->kind = K_WT; t->sid = sid;
  uint8_t head[16]; size_t h = venc(head, bidi ? 0x41 : 0x54); h += venc(head + h, (uint64_t)x->connect);
  if (wt_stream_send(s, sid, id, head, h, false)) return -1;
  return id;
}
int wt_stream_send(wt_server *s, int sid, int64_t stream, const uint8_t *data, size_t n, bool fin) {
  session *x = live_session(s, sid);
  if (!x) return -1;
  wt_strm *t = strm_get(x->c, stream, false);
  if (!t || t->kind != K_WT || t->sid != sid || t->fin_want) return -1;
  if (t->out_n + n > t->out_cap) {
    size_t cap = t->out_cap ? t->out_cap : 4096;
    while (cap < t->out_n + n) cap *= 2;
    if (cap > 16u << 20) return -1;   /* a peer that acknowledges nothing for 16 MB is gone */
    uint8_t *g = realloc(t->out, cap);
    if (!g) return -1;
    t->out = g; t->out_cap = cap;
  }
  if (n) memcpy(t->out + t->out_n, data, n);
  t->out_n += n;
  if (fin) t->fin_want = true;
  return 0;
}
size_t wt_stream_waiting(const wt_server *s, int sid) {
  session *x = live_session(s, sid);
  if (!x) return 0;
  size_t k = 0;
  for (wt_strm *t = x->c->streams; t; t = t->next) if (t->kind == K_WT && t->sid == sid) k += t->out_n;
  return k;
}
void wt_close(wt_server *s, int sid, uint32_t code) {
  session *x = live_session(s, sid);
  if (!x) return;
  /* CLOSE_WEBTRANSPORT_SESSION: the code (4 bytes) and an empty reason */
  size_t k = venc(x->cap, CAP_CLOSE); k += venc(x->cap + k, 4);
  x->cap[k++] = (uint8_t)(code >> 24); x->cap[k++] = (uint8_t)(code >> 16); x->cap[k++] = (uint8_t)(code >> 8); x->cap[k++] = (uint8_t)code;
  x->cap_n = k; x->cap_given = 0;
  qconn *c = x->c; int64_t id = x->connect;
  session_end(s, sid);
  s->sess[sid].closing = true;   /* the slot waits for the CONNECT stream to close (its capsule's bytes) */
  for (wt_strm *t = c->streams; t; t = t->next) if (t->kind == K_WT && t->sid < 0 && !t->fin_want) t->fin_want = true;
  if (c->h3) nghttp3_conn_resume_stream(c->h3, id);
}
int wt_rtt_ms(const wt_server *s, int sid) {
  session *x = live_session(s, sid);
  if (!x) return -1;
  ngtcp2_conn_info ci; ngtcp2_conn_get_conn_info(x->c->conn, &ci);
  return (int)(ci.smoothed_rtt / NGTCP2_MILLISECONDS);
}
wt_stats wt_get_stats(const wt_server *s) { return s->st; }
void wt_free(wt_server *s) {
  if (!s) return;
  for (int i = 0; i < s->nconns; i++) conn_free(s, s->conns[i]);
  close(s->fd);
  free(s);
}
