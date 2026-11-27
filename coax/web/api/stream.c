// Coax Web Server non-blocking streaming plane (kqueue on macOS, epoll on Linux)
// and shared SSE helpers (#14088).
//
// Sync HTTP workers (api.c) authenticate, validate ownership/cursors, and hand
// accepted SSE client sockets over an AF_UNIX SCM_RIGHTS socketpair to one
// dedicated non-blocking streaming plane process. The streaming plane holds up
// to SP_MAX_CLIENTS simultaneous SSE streams on a single event loop with at most
// two lazy Postgres connections total (one LISTEN connection + one replay query
// connection), delivering newly committed chat.turn_events_v2 rows via
// LISTEN/NOTIFY with zero polling and zero sync-worker pinning.
#include "stream.h"
#include "stream-policy.generated.h"

#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <sys/event.h>
#elif defined(__linux__)
#include <sys/epoll.h>
#endif

#define STREAM_HANDOFF_MAGIC 0x4F415354u
#define STREAM_NOTIFY_CHANNEL "oa_turn_events_v2"
#define STREAM_CONV_NOTIFY_CHANNEL "oa_conv_events_v2"
#define STREAM_ACCT_NOTIFY_CHANNEL "oa_account_events_v2"
#define STREAM_MAX_CLIENTS 1024
#define STREAM_RETRY_MS 3000
#define STREAM_DB_IDLE_MS 30000
#define STREAM_FRAME_CAP (160 * 1024)
#define STREAM_MAX_EVENTS 128

enum {
  STREAM_KIND_TURN = 0,
  STREAM_KIND_CONV = 1,
  STREAM_KIND_ACCOUNT = 2
};

static coax_string sp_cs(const char *s, size_t n) { return (coax_string){s ? s : "", (int64_t)n}; }
static coax_string sp_cstr(const char *s) { return sp_cs(s, s ? strlen(s) : 0); }
static coax_v_u8 sp_buf(void *p, size_t n) { return (coax_v_u8){(uint8_t *)p, (int64_t)n}; }

// ---- shared HTTP/SSE response helpers (usable by any route, including inference.c)

bool api_stream_write_all(int fd, const void *data, size_t n) {
  const uint8_t *p = (const uint8_t *)data;
  while (n > 0) {
    ssize_t k = write(fd, p, n);
    if (k < 0) {
      if (errno == EINTR) continue;
      return false;
    }
    if (k == 0) return false;
    p += (size_t)k;
    n -= (size_t)k;
  }
  return true;
}

bool api_stream_begin(int fd, int status, const char *content_type, const char *extra_headers) {
  uint8_t head[2048];
  int64_t n = sp_sse_headers(sp_buf(head, sizeof head), (int64_t)status,
                             sp_cstr(content_type), sp_cstr(extra_headers));
  if (n <= 0) return false;
  return api_stream_write_all(fd, head, (size_t)n);
}

bool api_stream_begin_sse(int fd, int status, const char *extra_headers) {
  return api_stream_begin(fd, status, "text/event-stream", extra_headers);
}

bool api_stream_write_event(int fd, int64_t id, const char *event_name, const char *data, size_t data_len) {
  size_t cap = data_len * 2 + 256;
  uint8_t stack[4096];
  uint8_t *buf = cap <= sizeof stack ? stack : malloc(cap);
  if (!buf) return false;
  int64_t n = sp_sse_event_frame(sp_buf(buf, cap), id, sp_cstr(event_name), sp_cs(data, data_len));
  bool ok = n > 0 && api_stream_write_all(fd, buf, (size_t)n);
  if (buf != stack) free(buf);
  return ok;
}

bool api_stream_write_keepalive(int fd) {
  uint8_t frame[32];
  int64_t n = sp_sse_keepalive_frame(sp_buf(frame, sizeof frame));
  return n > 0 && api_stream_write_all(fd, frame, (size_t)n);
}

bool api_stream_write_retry(int fd, int64_t retry_ms) {
  uint8_t frame[64];
  int64_t n = sp_sse_retry_frame(sp_buf(frame, sizeof frame), retry_ms);
  return n > 0 && api_stream_write_all(fd, frame, (size_t)n);
}

// ---- SCM_RIGHTS handoff (sync worker -> streaming plane)

typedef struct {
  uint32_t magic;
  int32_t kind;
  int32_t turn_state;
  int64_t after_seq;
  int64_t turn_seq;
  char account[40];
  char target[40];
  char extra_headers[512];
} stream_handoff_msg;

static int stream_send_fd = -1;

void stream_client_init(int send_fd) {
  stream_send_fd = send_fd;
}

static bool stream_handoff_send(const request *q, int32_t kind, const char *account,
                                const char *target, int64_t after_seq, int64_t turn_seq, int turn_state) {
  if (stream_send_fd < 0 || !q || q->fd < 0 || !account) return false;
  stream_handoff_msg payload;
  memset(&payload, 0, sizeof payload);
  payload.magic = STREAM_HANDOFF_MAGIC;
  payload.kind = kind;
  payload.turn_state = (int32_t)turn_state;
  payload.after_seq = after_seq;
  payload.turn_seq = turn_seq;
  snprintf(payload.account, sizeof payload.account, "%s", account);
  if (target) snprintf(payload.target, sizeof payload.target, "%s", target);

  const char *auth_hdr = auth_extra_headers();
  const char *route_env = getenv("OA_API_ROUTE_HEADER");
  bool want_route = route_env && !strcmp(route_env, "1");
  snprintf(payload.extra_headers, sizeof payload.extra_headers, "%s%s",
           auth_hdr ? auth_hdr : "",
           want_route ? "X-OA-Route: account_chat_turn_route\r\n" : "");

  struct iovec iov = {&payload, sizeof payload};
  union {
    struct cmsghdr hdr;
    unsigned char buf[CMSG_SPACE(sizeof(int))];
  } ctrl;
  memset(&ctrl, 0, sizeof ctrl);

  struct msghdr msg;
  memset(&msg, 0, sizeof msg);
  msg.msg_iov = &iov;
  msg.msg_iovlen = 1;
  msg.msg_control = ctrl.buf;
  msg.msg_controllen = sizeof ctrl.buf;

  struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg);
  cmsg->cmsg_level = SOL_SOCKET;
  cmsg->cmsg_type = SCM_RIGHTS;
  cmsg->cmsg_len = CMSG_LEN(sizeof(int));
  int client_fd = q->fd;
  memcpy(CMSG_DATA(cmsg), &client_fd, sizeof(int));

  for (int attempt = 0; attempt < 20; attempt++) {
    ssize_t sent = sendmsg(stream_send_fd, &msg, 0);
    if (sent == (ssize_t)sizeof payload) {
      api_mark_status(200);
      return true;
    }
    if (sent < 0 && errno == EINTR) continue;
    if (sent < 0 && (errno == EAGAIN || errno == EWOULDBLOCK || errno == ENOBUFS)) {
      struct pollfd pfd = {stream_send_fd, POLLOUT, 0};
      (void)poll(&pfd, 1, 25);
      continue;
    }
    api_log("stream: handoff sendmsg failed (errno=%d)", errno);
    return false;
  }
  api_log("stream: handoff sendmsg timed out under backpressure");
  return false;
}

bool stream_handoff_turn_events(const request *q, const char *account, const char *turn,
                                int64_t after_seq, int64_t turn_seq, int turn_state) {
  if (!turn) return false;
  return stream_handoff_send(q, STREAM_KIND_TURN, account, turn, after_seq, turn_seq, turn_state);
}

bool stream_handoff_conv_events(const request *q, const char *account, const char *conversation,
                                int64_t after_event_id) {
  if (!conversation) return false;
  return stream_handoff_send(q, STREAM_KIND_CONV, account, conversation, after_event_id, 0, 0);
}

bool stream_handoff_account_events(const request *q, const char *account, int64_t after_id) {
  return stream_handoff_send(q, STREAM_KIND_ACCOUNT, account, "", after_id, 0, 0);
}

// ---- non-blocking streaming plane process

typedef struct {
  int fd;
  int slot;
  int kind;
  char account[40];
  char target[40];
  int64_t last_seq;
  int64_t connected_ms;
  int64_t last_send_ms;
  uint8_t *out;
  size_t out_off;
  size_t out_len;
  size_t out_cap;
  bool write_armed;
  bool closing;
} stream_client;

typedef struct {
  int ev_fd;
  int recv_fd;
  PGconn *listen_db;
  PGconn *query_db;
  int listen_sock;
  int64_t idle_since_ms;
  int64_t heartbeat_ms;
  int64_t max_lifetime_ms;
  int64_t max_queue_bytes;
  int64_t replay_batch;
  int nclients;
  stream_client *clients[STREAM_MAX_CLIENTS];
  uint8_t *frame_buf;
} stream_plane;

static volatile sig_atomic_t plane_stopping;
static void on_plane_term(int sig) { (void)sig; plane_stopping = 1; }

static int64_t plane_env_i64(const char *name) {
  const char *v = getenv(name);
  if (!v || !*v) return 0;
  char *end = NULL;
  long long n = strtoll(v, &end, 10);
  if (!end || *end != 0) return 0;
  return (int64_t)n;
}

static bool set_nonblock(int fd) {
  int fl = fcntl(fd, F_GETFL, 0);
  if (fl < 0) return false;
  return fcntl(fd, F_SETFL, fl | O_NONBLOCK) == 0;
}

static void set_nodelay(int fd) {
  int one = 1;
  (void)setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
}

enum {
  TAG_HANDOFF = 1,
  TAG_PG_LISTEN = 2,
  TAG_CLIENT_BASE = 16
};

static bool ev_add_read(int ev_fd, int fd, uintptr_t tag) {
#if defined(__APPLE__)
  struct kevent ev;
  EV_SET(&ev, (uintptr_t)fd, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, (void *)tag);
  return kevent(ev_fd, &ev, 1, NULL, 0, NULL) == 0;
#elif defined(__linux__)
  struct epoll_event ev;
  memset(&ev, 0, sizeof ev);
  ev.events = EPOLLIN | EPOLLRDHUP;
  ev.data.u64 = (uint64_t)tag;
  return epoll_ctl(ev_fd, EPOLL_CTL_ADD, fd, &ev) == 0;
#else
  (void)ev_fd; (void)fd; (void)tag; return false;
#endif
}

static void ev_del_fd(int ev_fd, int fd) {
  if (ev_fd < 0 || fd < 0) return;
#if defined(__APPLE__)
  struct kevent evs[2];
  EV_SET(&evs[0], (uintptr_t)fd, EVFILT_READ, EV_DELETE, 0, 0, NULL);
  EV_SET(&evs[1], (uintptr_t)fd, EVFILT_WRITE, EV_DELETE, 0, 0, NULL);
  (void)kevent(ev_fd, evs, 2, NULL, 0, NULL);
#elif defined(__linux__)
  (void)epoll_ctl(ev_fd, EPOLL_CTL_DEL, fd, NULL);
#else
  (void)ev_fd; (void)fd;
#endif
}

static void ev_set_client_write(stream_plane *p, stream_client *c, bool want) {
  if (c->write_armed == want) return;
  uintptr_t tag = (uintptr_t)(TAG_CLIENT_BASE + c->slot);
#if defined(__APPLE__)
  struct kevent ev;
  EV_SET(&ev, (uintptr_t)c->fd, EVFILT_WRITE, want ? (EV_ADD | EV_ENABLE) : EV_DELETE, 0, 0, (void *)tag);
  if (kevent(p->ev_fd, &ev, 1, NULL, 0, NULL) == 0) c->write_armed = want;
#elif defined(__linux__)
  struct epoll_event ev;
  memset(&ev, 0, sizeof ev);
  ev.events = EPOLLIN | EPOLLRDHUP | (want ? EPOLLOUT : 0);
  ev.data.u64 = (uint64_t)tag;
  if (epoll_ctl(p->ev_fd, EPOLL_CTL_MOD, c->fd, &ev) == 0) c->write_armed = want;
#else
  (void)p; (void)tag;
#endif
}

static void plane_close_client(stream_plane *p, stream_client *c) {
  if (!c) return;
  int slot = c->slot;
  ev_del_fd(p->ev_fd, c->fd);
  close(c->fd);
  free(c->out);
  free(c);
  if (slot >= 0 && slot < STREAM_MAX_CLIENTS && p->clients[slot] == c) {
    p->clients[slot] = NULL;
    p->nclients--;
    if (p->nclients == 0) p->idle_since_ms = (int64_t)api_now_ms();
  }
}

static bool client_enqueue(stream_plane *p, stream_client *c, const void *data, size_t n) {
  if (n == 0) return true;
  size_t queued = c->out_len - c->out_off;
  if (!sp_can_enqueue((int64_t)queued, (int64_t)n, p->max_queue_bytes)) {
    return false;
  }
  if (c->out_off > 0) {
    if (queued > 0) memmove(c->out, c->out + c->out_off, queued);
    c->out_off = 0;
    c->out_len = queued;
  }
  if (c->out_len + n > c->out_cap) {
    size_t want = c->out_cap ? c->out_cap : 4096;
    while (want < c->out_len + n) want *= 2;
    if (want > (size_t)p->max_queue_bytes) want = (size_t)p->max_queue_bytes;
    uint8_t *next = realloc(c->out, want);
    if (!next) return false;
    c->out = next;
    c->out_cap = want;
  }
  memcpy(c->out + c->out_len, data, n);
  c->out_len += n;
  ev_set_client_write(p, c, true);
  return true;
}

static bool client_flush(stream_plane *p, stream_client *c, int64_t now_ms) {
  while (c->out_off < c->out_len) {
    ssize_t k = send(c->fd, c->out + c->out_off, c->out_len - c->out_off, 0);
    if (k < 0) {
      if (errno == EINTR) continue;
      if (errno == EAGAIN || errno == EWOULDBLOCK) return true;
      plane_close_client(p, c);
      return false;
    }
    if (k == 0) {
      plane_close_client(p, c);
      return false;
    }
    c->out_off += (size_t)k;
    c->last_send_ms = now_ms;
  }
  c->out_off = 0;
  c->out_len = 0;
  if (c->closing) {
    plane_close_client(p, c);
    return false;
  }
  ev_set_client_write(p, c, false);
  return true;
}

static void plane_disconnect_db(stream_plane *p) {
  if (p->listen_db) {
    if (p->listen_sock >= 0) ev_del_fd(p->ev_fd, p->listen_sock);
    PQfinish(p->listen_db);
    p->listen_db = NULL;
    p->listen_sock = -1;
  }
  if (p->query_db) {
    PQfinish(p->query_db);
    p->query_db = NULL;
  }
}

static PGconn *plane_query_db(stream_plane *p) {
  if (p->query_db && PQstatus(p->query_db) == CONNECTION_OK) return p->query_db;
  if (p->query_db) { PQfinish(p->query_db); p->query_db = NULL; }
  p->query_db = api_db_new();
  return p->query_db;
}

static bool plane_ensure_listen_db(stream_plane *p) {
  if (p->listen_db && PQstatus(p->listen_db) == CONNECTION_OK && p->listen_sock >= 0) return true;
  if (p->listen_db) {
    if (p->listen_sock >= 0) ev_del_fd(p->ev_fd, p->listen_sock);
    PQfinish(p->listen_db);
    p->listen_db = NULL;
    p->listen_sock = -1;
  }
  const char *channels[3] = {
    STREAM_NOTIFY_CHANNEL,
    STREAM_CONV_NOTIFY_CHANNEL,
    STREAM_ACCT_NOTIFY_CHANNEL
  };
  for (int i = 0; i < 3; i++) {
    if (!sp_channel_valid(sp_cstr(channels[i]))) return false;
  }
  PGconn *c = api_db_new();
  if (!c) return false;
  for (int i = 0; i < 3; i++) {
    char cmd[128];
    snprintf(cmd, sizeof cmd, "LISTEN %s", channels[i]);
    PGresult *r = PQexec(c, cmd);
    bool ok = r && PQresultStatus(r) == PGRES_COMMAND_OK;
    PQclear(r);
    if (!ok) {
      api_log("stream: LISTEN %s failed: %s", channels[i], PQerrorMessage(c));
      PQfinish(c);
      return false;
    }
  }
  int sock = PQsocket(c);
  if (sock < 0 || !ev_add_read(p->ev_fd, sock, TAG_PG_LISTEN)) {
    api_log("stream: could not register LISTEN socket %d", sock);
    PQfinish(c);
    return false;
  }
  p->listen_db = c;
  p->listen_sock = sock;
  return true;
}

// Replay newly committed events strictly after c->last_seq in exact ascending
// order for turn, conversation, or account-wide streams. Returns false if the
// client was closed (backpressure, terminal turn end, or socket error).
static bool plane_catchup_client(stream_plane *p, stream_client *c, int64_t now_ms) {
  PGconn *qdb = plane_query_db(p);
  if (!qdb) return true;

  char limit_s[32];
  snprintf(limit_s, sizeof limit_s, "%lld", (long long)p->replay_batch);

  for (;;) {
    char after_s[32];
    snprintf(after_s, sizeof after_s, "%lld", (long long)c->last_seq);
    PGresult *r = NULL;
    if (c->kind == STREAM_KIND_TURN) {
      const char *params[4] = {c->account, c->target, after_s, limit_s};
      r = PQexecParams(qdb,
        "SELECT sequence, 'turn', record FROM chat.turn_events_v2 "
        "WHERE account = $1 AND turn = $2 AND sequence > $3::bigint "
        "ORDER BY sequence ASC LIMIT $4::int",
        4, NULL, params, NULL, NULL, 0);
    } else if (c->kind == STREAM_KIND_CONV) {
      const char *params[4] = {c->account, c->target, after_s, limit_s};
      r = PQexecParams(qdb,
        "SELECT event_id, 'chat_event', record FROM chat.turn_events_v2 "
        "WHERE account = $1 AND conversation = $2 AND event_id > $3::bigint "
        "ORDER BY event_id ASC LIMIT $4::int",
        4, NULL, params, NULL, NULL, 0);
    } else {
      const char *params[3] = {c->account, after_s, limit_s};
      r = PQexecParams(qdb,
        "SELECT id, event_type, record FROM chat.account_events_v2 "
        "WHERE account = $1 AND id > $2::bigint "
        "ORDER BY id ASC LIMIT $3::int",
        3, NULL, params, NULL, NULL, 0);
    }
    if (!r || PQresultStatus(r) != PGRES_TUPLES_OK) {
      PQclear(r);
      return true;
    }
    int rows = PQntuples(r);
    for (int i = 0; i < rows; i++) {
      int64_t seq = strtoll(PQgetvalue(r, i, 0), NULL, 10);
      if (seq <= c->last_seq) continue;
      const char *ev_name = PQgetvalue(r, i, 1);
      const char *rec = PQgetvalue(r, i, 2);
      int rec_len = PQgetlength(r, i, 2);
      int64_t fn = sp_sse_event_frame(sp_buf(p->frame_buf, STREAM_FRAME_CAP),
                                      seq, sp_cstr(ev_name), sp_cs(rec, (size_t)rec_len));
      if (fn <= 0) continue;
      if (!client_enqueue(p, c, p->frame_buf, (size_t)fn)) {
        api_log("stream: closing slow client kind=%d target=%s (queue limit %lld bytes)",
                c->kind, c->target[0] ? c->target : c->account, (long long)p->max_queue_bytes);
        PQclear(r);
        plane_close_client(p, c);
        return false;
      }
      c->last_seq = seq;
    }
    PQclear(r);
    if (!client_flush(p, c, now_ms)) return false;
    if (rows < (int)p->replay_batch) break;
  }

  // Only single-turn streams auto-close once the turn is terminal and caught up.
  if (c->kind == STREAM_KIND_TURN) {
    const char *tparams[2] = {c->account, c->target};
    PGresult *tr = PQexecParams(qdb,
      "SELECT state, sequence FROM chat.turns_v2 WHERE account = $1 AND id = $2",
      2, NULL, tparams, NULL, NULL, 0);
    if (tr && PQresultStatus(tr) == PGRES_TUPLES_OK && PQntuples(tr) == 1) {
      int64_t st = strtoll(PQgetvalue(tr, 0, 0), NULL, 10);
      int64_t tseq = strtoll(PQgetvalue(tr, 0, 1), NULL, 10);
      if (sp_is_terminal_state(st) && c->last_seq >= tseq) {
        c->closing = true;
      }
    }
    PQclear(tr);
    if (c->closing && c->out_off == c->out_len) {
      plane_close_client(p, c);
      return false;
    }
  }
  return true;
}

static void plane_accept_handoffs(stream_plane *p, int64_t now_ms) {
  for (;;) {
    stream_handoff_msg payload;
    memset(&payload, 0, sizeof payload);
    struct iovec iov = {&payload, sizeof payload};
    union {
      struct cmsghdr hdr;
      unsigned char buf[CMSG_SPACE(sizeof(int))];
    } ctrl;
    memset(&ctrl, 0, sizeof ctrl);

    struct msghdr msg;
    memset(&msg, 0, sizeof msg);
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = ctrl.buf;
    msg.msg_controllen = sizeof ctrl.buf;

    ssize_t got = recvmsg(p->recv_fd, &msg, 0);
    if (got < 0) {
      if (errno == EINTR) continue;
      if (errno == EAGAIN || errno == EWOULDBLOCK) return;
      api_log("stream: recvmsg error (errno=%d)", errno);
      return;
    }
    if (got == 0) return;

    int client_fd = -1;
    for (struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg); cmsg; cmsg = CMSG_NXTHDR(&msg, cmsg)) {
      if (cmsg->cmsg_level == SOL_SOCKET && cmsg->cmsg_type == SCM_RIGHTS &&
          cmsg->cmsg_len >= CMSG_LEN(sizeof(int))) {
        memcpy(&client_fd, CMSG_DATA(cmsg), sizeof(int));
      }
    }
    if (client_fd < 0) continue;

    if (got != (ssize_t)sizeof payload || payload.magic != STREAM_HANDOFF_MAGIC) {
      close(client_fd);
      continue;
    }

    int slot = -1;
    for (int i = 0; i < STREAM_MAX_CLIENTS; i++) {
      if (!p->clients[i]) { slot = i; break; }
    }
    if (slot < 0) {
      api_error(client_fd, 503, "Too many concurrent streams.");
      close(client_fd);
      continue;
    }

    // Ensure LISTEN is active BEFORE initial replay so no NOTIFY can be missed in the window.
    if (!plane_ensure_listen_db(p)) {
      api_error(client_fd, 503, "Event stream unavailable.");
      close(client_fd);
      continue;
    }

    if (!set_nonblock(client_fd)) {
      close(client_fd);
      continue;
    }
    set_nodelay(client_fd);

    stream_client *c = calloc(1, sizeof *c);
    if (!c) { close(client_fd); continue; }
    c->fd = client_fd;
    c->slot = slot;
    c->kind = payload.kind;
    snprintf(c->account, sizeof c->account, "%.*s", (int)(sizeof c->account - 1), payload.account);
    snprintf(c->target, sizeof c->target, "%.*s", (int)(sizeof c->target - 1), payload.target);
    c->last_seq = payload.after_seq;
    c->connected_ms = now_ms;
    c->last_send_ms = now_ms;

    if (!ev_add_read(p->ev_fd, client_fd, (uintptr_t)(TAG_CLIENT_BASE + slot))) {
      free(c);
      close(client_fd);
      continue;
    }
    p->clients[slot] = c;
    p->nclients++;
    p->idle_since_ms = 0;

    uint8_t head[2048];
    int64_t hn = sp_sse_headers(sp_buf(head, sizeof head), 200,
                                sp_cstr("text/event-stream"), sp_cstr(payload.extra_headers));
    uint8_t retry_buf[64];
    int64_t rn = sp_sse_retry_frame(sp_buf(retry_buf, sizeof retry_buf), STREAM_RETRY_MS);
    if (hn <= 0 || rn <= 0 ||
        !client_enqueue(p, c, head, (size_t)hn) ||
        !client_enqueue(p, c, retry_buf, (size_t)rn)) {
      plane_close_client(p, c);
      continue;
    }

    if (!client_flush(p, c, now_ms)) continue;
    (void)plane_catchup_client(p, c, now_ms);
  }
}

typedef struct {
  int kind;
  char key[41];
  int64_t max_seq;
} notified_target;

static void plane_wake_matching(stream_plane *p, int kind, const char *key, int64_t max_seq, int64_t now_ms) {
  for (int s = 0; s < STREAM_MAX_CLIENTS; s++) {
    stream_client *c = p->clients[s];
    if (!c || c->kind != kind || c->last_seq >= max_seq) continue;
    const char *ckey = (kind == STREAM_KIND_ACCOUNT) ? c->account : c->target;
    if (!strcmp(ckey, key)) {
      (void)plane_catchup_client(p, c, now_ms);
    }
  }
}

static void plane_drain_notifies(stream_plane *p, int64_t now_ms) {
  if (!p->listen_db) return;
  if (!PQconsumeInput(p->listen_db)) {
    api_log("stream: LISTEN connection lost (%s); reconnecting", PQerrorMessage(p->listen_db));
    if (plane_ensure_listen_db(p)) {
      for (int i = 0; i < STREAM_MAX_CLIENTS; i++) {
        if (p->clients[i]) (void)plane_catchup_client(p, p->clients[i], now_ms);
      }
    }
    return;
  }

  notified_target batch[128];
  int nbatch = 0;
  PGnotify *nfy;
  while ((nfy = PQnotifies(p->listen_db)) != NULL) {
    int kind = -1;
    char key[41] = {0};
    int64_t seq = 0;
    if (nfy->relname && nfy->extra) {
      if (!strcmp(nfy->relname, STREAM_NOTIFY_CHANNEL)) {
        uint8_t turn_bytes[36];
        if (sp_notify_parse(sp_cstr(nfy->extra), sp_buf(turn_bytes, sizeof turn_bytes), (coax_v_s64){&seq, 1})) {
          kind = STREAM_KIND_TURN;
          memcpy(key, turn_bytes, 36);
          key[36] = 0;
        }
      } else if (!strcmp(nfy->relname, STREAM_CONV_NOTIFY_CHANNEL)) {
        uint8_t conv_bytes[36];
        if (sp_conv_notify_parse(sp_cstr(nfy->extra), sp_buf(conv_bytes, sizeof conv_bytes), (coax_v_s64){&seq, 1})) {
          kind = STREAM_KIND_CONV;
          memcpy(key, conv_bytes, 36);
          key[36] = 0;
        }
      } else if (!strcmp(nfy->relname, STREAM_ACCT_NOTIFY_CHANNEL)) {
        uint8_t acct_bytes[40];
        int64_t acct_len = 0;
        if (sp_acct_notify_parse(sp_cstr(nfy->extra), sp_buf(acct_bytes, sizeof acct_bytes),
                                 (coax_v_s64){&acct_len, 1}, (coax_v_s64){&seq, 1}) &&
            acct_len > 0 && acct_len <= 40) {
          kind = STREAM_KIND_ACCOUNT;
          memcpy(key, acct_bytes, (size_t)acct_len);
          key[acct_len] = 0;
        }
      }
    }
    if (kind >= 0) {
      int found = -1;
      for (int i = 0; i < nbatch; i++) {
        if (batch[i].kind == kind && !strcmp(batch[i].key, key)) { found = i; break; }
      }
      if (found >= 0) {
        if (seq > batch[found].max_seq) batch[found].max_seq = seq;
      } else if (nbatch < (int)(sizeof batch / sizeof batch[0])) {
        batch[nbatch].kind = kind;
        memcpy(batch[nbatch].key, key, sizeof key);
        batch[nbatch].max_seq = seq;
        nbatch++;
      } else {
        plane_wake_matching(p, kind, key, seq, now_ms);
      }
    }
    PQfreemem(nfy);
  }

  for (int b = 0; b < nbatch; b++) {
    plane_wake_matching(p, batch[b].kind, batch[b].key, batch[b].max_seq, now_ms);
  }
}

static void plane_handle_client_read(stream_plane *p, stream_client *c) {
  uint8_t discard[256];
  for (;;) {
    ssize_t k = recv(c->fd, discard, sizeof discard, 0);
    if (k == 0) { plane_close_client(p, c); return; }
    if (k < 0) {
      if (errno == EINTR) continue;
      if (errno == EAGAIN || errno == EWOULDBLOCK) return;
      plane_close_client(p, c);
      return;
    }
  }
}

static int plane_tick_and_timeout(stream_plane *p, int64_t now_ms) {
  int wait_ms = 1000;
  for (int i = 0; i < STREAM_MAX_CLIENTS; i++) {
    stream_client *c = p->clients[i];
    if (!c) continue;
    if (sp_lifetime_expired(now_ms, c->connected_ms, p->max_lifetime_ms)) {
      c->closing = true;
      if (c->out_off == c->out_len) { plane_close_client(p, c); continue; }
    } else if (!c->closing && c->out_off == c->out_len &&
               sp_heartbeat_due(now_ms, c->last_send_ms, p->heartbeat_ms)) {
      uint8_t ka[32];
      int64_t kn = sp_sse_keepalive_frame(sp_buf(ka, sizeof ka));
      if (kn > 0 && client_enqueue(p, c, ka, (size_t)kn)) {
        c->last_send_ms = now_ms;
        if (!client_flush(p, c, now_ms)) continue;
      } else {
        plane_close_client(p, c);
        continue;
      }
    }
    int64_t next = sp_next_timeout_ms(now_ms, c->last_send_ms, c->connected_ms,
                                      p->heartbeat_ms, p->max_lifetime_ms);
    if (next < wait_ms) wait_ms = (int)next;
  }

  if (p->nclients == 0 && p->listen_db && p->idle_since_ms > 0 &&
      now_ms - p->idle_since_ms >= STREAM_DB_IDLE_MS) {
    plane_disconnect_db(p);
  }
  return wait_ms;
}

int stream_plane_run(int recv_fd, pid_t parent_pid) {
  signal(SIGPIPE, SIG_IGN);
  struct sigaction sa;
  memset(&sa, 0, sizeof sa);
  sa.sa_handler = on_plane_term;
  sigaction(SIGTERM, &sa, NULL);
  sigaction(SIGINT, &sa, NULL);

  if (!set_nonblock(recv_fd)) {
    api_log("stream: could not set handoff socket non-blocking");
    return 1;
  }

  stream_plane p;
  memset(&p, 0, sizeof p);
  p.recv_fd = recv_fd;
  p.listen_sock = -1;
  p.heartbeat_ms = sp_clamp_heartbeat_ms(plane_env_i64("OA_STREAM_HEARTBEAT_MS"));
  p.max_lifetime_ms = sp_clamp_lifetime_ms(plane_env_i64("OA_STREAM_MAX_LIFETIME_MS"));
  p.max_queue_bytes = sp_clamp_queue_bytes(plane_env_i64("OA_STREAM_MAX_QUEUE_BYTES"));
  p.replay_batch = sp_replay_limit(0);
  p.frame_buf = malloc(STREAM_FRAME_CAP);
  if (!p.frame_buf) return 1;

#if defined(__APPLE__)
  p.ev_fd = kqueue();
#elif defined(__linux__)
  p.ev_fd = epoll_create1(EPOLL_CLOEXEC);
#else
  p.ev_fd = -1;
#endif
  if (p.ev_fd < 0) {
    api_log("stream: event loop init failed (errno=%d)", errno);
    free(p.frame_buf);
    return 1;
  }

  if (!ev_add_read(p.ev_fd, recv_fd, TAG_HANDOFF)) {
    api_log("stream: could not register handoff fd");
    close(p.ev_fd);
    free(p.frame_buf);
    return 1;
  }

  while (!plane_stopping) {
    if (parent_pid && getppid() != parent_pid) break;
    int64_t now_ms = (int64_t)api_now_ms();
    int timeout_ms = plane_tick_and_timeout(&p, now_ms);

#if defined(__APPLE__)
    struct kevent events[STREAM_MAX_EVENTS];
    struct timespec ts = {timeout_ms / 1000, (long)(timeout_ms % 1000) * 1000000L};
    int n = kevent(p.ev_fd, NULL, 0, events, STREAM_MAX_EVENTS, &ts);
    if (n < 0) {
      if (errno == EINTR) continue;
      api_log("stream: kevent error (errno=%d)", errno);
      break;
    }
    now_ms = (int64_t)api_now_ms();
    for (int i = 0; i < n; i++) {
      uintptr_t tag = (uintptr_t)events[i].udata;
      if (tag == TAG_HANDOFF) {
        plane_accept_handoffs(&p, now_ms);
      } else if (tag == TAG_PG_LISTEN) {
        plane_drain_notifies(&p, now_ms);
      } else if (tag >= TAG_CLIENT_BASE && tag < TAG_CLIENT_BASE + STREAM_MAX_CLIENTS) {
        stream_client *c = p.clients[tag - TAG_CLIENT_BASE];
        if (!c) continue;
        if (events[i].flags & EV_ERROR) { plane_close_client(&p, c); continue; }
        if (events[i].filter == EVFILT_READ) {
          if (events[i].flags & EV_EOF) plane_close_client(&p, c);
          else plane_handle_client_read(&p, c);
        } else if (events[i].filter == EVFILT_WRITE) {
          (void)client_flush(&p, c, now_ms);
        }
      }
    }
#elif defined(__linux__)
    struct epoll_event events[STREAM_MAX_EVENTS];
    int n = epoll_wait(p.ev_fd, events, STREAM_MAX_EVENTS, timeout_ms);
    if (n < 0) {
      if (errno == EINTR) continue;
      api_log("stream: epoll_wait error (errno=%d)", errno);
      break;
    }
    now_ms = (int64_t)api_now_ms();
    for (int i = 0; i < n; i++) {
      uintptr_t tag = (uintptr_t)events[i].data.u64;
      if (tag == TAG_HANDOFF) {
        plane_accept_handoffs(&p, now_ms);
      } else if (tag == TAG_PG_LISTEN) {
        plane_drain_notifies(&p, now_ms);
      } else if (tag >= TAG_CLIENT_BASE && tag < TAG_CLIENT_BASE + STREAM_MAX_CLIENTS) {
        stream_client *c = p.clients[tag - TAG_CLIENT_BASE];
        if (!c) continue;
        uint32_t mask = events[i].events;
        if (mask & (EPOLLERR | EPOLLHUP | EPOLLRDHUP)) { plane_close_client(&p, c); continue; }
        if (mask & EPOLLIN) {
          plane_handle_client_read(&p, c);
          c = p.clients[tag - TAG_CLIENT_BASE];
          if (!c) continue;
        }
        if (mask & EPOLLOUT) (void)client_flush(&p, c, now_ms);
      }
    }
#endif
  }

  for (int i = 0; i < STREAM_MAX_CLIENTS; i++) {
    if (p.clients[i]) plane_close_client(&p, p.clients[i]);
  }
  plane_disconnect_db(&p);
  close(p.ev_fd);
  close(recv_fd);
  free(p.frame_buf);
  return 0;
}
