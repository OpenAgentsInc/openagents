/* The room server's vitals for the admins' room view (#12791; docs/research/mmo.md section 15): the
   whole action path, measured where it happens, as plain numbers over a 10 s window.
     - the everglade tick's time, p50, p95 and p99, in all and by workload: players (the movement
       steps), raiders (the rules' step, which runs the raid), effects (the abilities and the spells),
       folk (the town's watch and people), writing (the frames written, serialization) and sending
     - each socket's send queue: the bytes waiting and how long the oldest has waited
     - bytes and messages a second by type, out (each frame's letter, each D entry's, an s row's by
       its id: player, townsfolk or made) and in (each frame's letter)
     - relevant entities per seat (room-stats.coax rm_st_relevant), once a second
     - replication's age bound per seat (room-stats.coax rm_st_wire, #12836): rows sent for their age,
       and, sampled once a second, the oldest changed row's wait and the rows a tick left for later
     - the initial-state burst: a socket's bytes from its join to its first whole snapshot, and how
       long until they were all sent
     - rejoins tried and seated, and joins refused by reason (room-stats.coax)
     - input to acknowledgement per seat: the input heard to its ACK sent, plus the socket's round
       trip (TCP_INFO), p50 and p95
     - what the measuring itself costs, as a share of the tick
   The host includes this after room.h and its conn table (room-host.c). It is all counters and
   histograms bumped on the paths that already run; the report is written once a window, off the
   tick. st_report writes it as "name=value" lines and "seat|..." lines (GET /api/room/vitals, and the
   signed POST /api/playtest/room-stats the admin view reads). */
#include <netinet/tcp.h>
#include <sys/socket.h>
static int64_t now_us(void);   /* host monotonic SDK clock, defined after this include */

#define ST_WINDOW_MS 10000
#define ST_BODY_CAP 12000     /* under room-stats.coax ST_BODY (what rm_st_sign signs), with room for its escapes in the poster's 16 KB */
static char st_last[ST_BODY_CAP];   /* the last window's report, for GET /api/room/vitals */
#define ST_BUCKETS 4000          /* 10 us buckets: 0 to 40 ms, the last is everything past */
typedef struct { uint32_t b[ST_BUCKETS]; uint32_t n; } st_hist;
static void st_hist_add(st_hist *h, int64_t us) {
  int64_t i = us / 10; h->b[i < 0 ? 0 : i >= ST_BUCKETS ? ST_BUCKETS - 1 : i]++; h->n++;
}
/* The q-th thousandth in microseconds (0: nothing counted). */
static int64_t st_hist_at(const st_hist *h, int q) {
  if (!h->n) return 0;
  uint64_t want = ((uint64_t)h->n * (uint64_t)q + 999) / 1000, k = 0;
  for (int i = 0; i < ST_BUCKETS; i++) { k += h->b[i]; if (k >= want) return (int64_t)(i + 1) * 10; }
  return ST_BUCKETS * 10;
}

/* the tick's workloads: room-host.c's parts, summed per tick into these */
enum { STW_ALL, STW_PLAYERS, STW_RAIDERS, STW_EFFECTS, STW_FOLK, STW_WRITE, STW_SEND, STW_N };
static const char *st_work_name[STW_N] = {"tick", "players", "raiders", "effects", "folk", "writing", "sending"};
static st_hist st_work[STW_N];
static int64_t st_ticks, st_tick_us, st_cost_us;   /* this window's ticks, their time, and the measuring's */

/* bytes and messages by type: a frame's letter (0-127), a D entry's (128 + letter), an s row by id */
#define ST_KEYS 260
#define ST_K_ENTRY 128
#define ST_K_S_PLAYER 256
#define ST_K_S_FOLK 257
#define ST_K_S_MADE 258
#define ST_K_D_HEAD 259
static int64_t st_out_b[ST_KEYS], st_out_m[ST_KEYS], st_in_b[128], st_in_m[128];
static int64_t st_out_frames, st_out_bytes, st_in_frames, st_in_bytes;

/* each socket's: its queue's worst this window, its join burst, its input-to-ACK samples */
#define ST_ACK_BUCKETS 512       /* 2 ms buckets: 0 to 1024 ms */
typedef struct {
  int64_t q_bytes, q_ms;         /* the most waiting this window, and the oldest waiting's age */
  int burst;                     /* 0 none, 1 counting its join's bytes, 2 its snapshot is queued: until sent */
  int64_t burst_at, burst_bytes; /* when it joined (us), its bytes since */
  uint16_t ack[ST_ACK_BUCKETS]; uint32_t ack_n;
  int64_t rtt_ms;                /* its round trip, as the kernel has it (-1 unknown) */
  int relevant;
  int64_t aged_was, aged;        /* rows sent for their age: the seat's count when last sampled, and this window's */
  int64_t wait_most, left_most;  /* the oldest changed row's wait (ticks) and the rows left for later, most sampled */
} st_conn;
static st_conn st_c[CONNS];
static uint32_t st_ack_all[ST_ACK_BUCKETS], st_ack_all_n;   /* every seat's input-to-ACK this window */
static int64_t st_burst_n, st_burst_bytes_max, st_burst_ms_max, st_burst_bytes_sum, st_burst_ms_sum;
static int64_t st_refused_was[16], st_re_tries_was, st_re_ok_was, st_re_seat_was, st_stale_was;
static int64_t st_alloc_was[13];   /* rm_st_alloc's totals at the window's start (#12836) */

static void st_ack_add(int i, int64_t ms) {
  int64_t b = ms / 2; b = b < 0 ? 0 : b >= ST_ACK_BUCKETS ? ST_ACK_BUCKETS - 1 : b;
  st_c[i].ack[b]++; st_c[i].ack_n++; st_ack_all[b]++; st_ack_all_n++;
}
/* The q-th thousandth of 2 ms buckets h (n counted) in ms, -1 none. */
static int64_t st_ack_q(const uint16_t *h16, const uint32_t *h32, uint32_t n, int q) {
  if (!n) return -1;
  uint64_t want = ((uint64_t)n * (uint64_t)q + 999) / 1000, k = 0;
  for (int i = 0; i < ST_ACK_BUCKETS; i++) { k += h16 ? h16[i] : h32[i]; if (k >= want) return (int64_t)(i + 1) * 2; }
  return ST_ACK_BUCKETS * 2;
}
static int64_t st_ack_at(const st_conn *c, int q) { return st_ack_q(c->ack, NULL, c->ack_n, q); }
/* The socket's smoothed round trip in ms, -1 when the kernel won't say. */
static int64_t st_rtt(int fd) {
#if defined(__APPLE__) && defined(TCP_CONNECTION_INFO)
  struct tcp_connection_info t; socklen_t n = sizeof t;
  if (getsockopt(fd, IPPROTO_TCP, TCP_CONNECTION_INFO, &t, &n) == 0) return (int64_t)t.tcpi_srtt;
#elif defined(TCP_INFO)
  struct tcp_info t; socklen_t n = sizeof t;
  if (getsockopt(fd, IPPROTO_TCP, TCP_INFO, &t, &n) == 0) return (int64_t)t.tcpi_rtt / 1000;
#endif
  (void)fd; return -1;
}

/* A socket sat (its handshake answered): its burst starts counting. */
static void st_joined(int i, int64_t us) {
  st_c[i] = (st_conn){.burst = 1, .burst_at = us, .rtt_ms = -1};
}
static void st_put(int i, size_t n) { if (st_c[i].burst == 1) st_c[i].burst_bytes += (int64_t)n; }
/* The socket's output emptied: a snapshot queued is now all sent. */
static void st_drained(int i, int64_t us) {
  if (st_c[i].burst != 2) return;
  st_c[i].burst = 0;
  int64_t ms = (us - st_c[i].burst_at) / 1000;
  st_burst_n++; st_burst_bytes_sum += st_c[i].burst_bytes; st_burst_ms_sum += ms;
  if (st_c[i].burst_bytes > st_burst_bytes_max) st_burst_bytes_max = st_c[i].burst_bytes;
  if (ms > st_burst_ms_max) st_burst_ms_max = ms;
}

static bool st_digit(uint8_t c) { return (c >= '0' && c <= '9') || c == '-'; }
/* One frame out (its text, t[0..n)), to `to` sockets, k bytes each on the wire: counted by its letter,
   and a D frame's entries by theirs on one tick in ST_DEEP (counted ST_DEEP times: the entries are
   most of the measuring's cost). Returns D's F (its third number), -1 for any other frame. */
#define ST_DEEP 4
static int st_frame_out(const uint8_t *t, size_t n, int to, size_t k) {
  if (!n || to <= 0) return -1;
  int c = t[0] & 127;
  st_out_frames += to; st_out_bytes += (int64_t)k * to;
  st_out_b[c] += (int64_t)k * to; st_out_m[c] += to;
  if (c != 'D' || n < 2) return -1;
  /* the header's numbers (TICK ACK F [PART]), then the entries, ';' apart */
  size_t i = 1; int nums = 0, f = -1;
  while (i < n) {
    while (i < n && t[i] == ' ') i++;
    if (i >= n || !st_digit(t[i])) break;
    int64_t x = 0; bool neg = t[i] == '-'; if (neg) i++;
    while (i < n && t[i] >= '0' && t[i] <= '9') { x = x * 10 + (t[i] - '0'); i++; }
    if (++nums == 3) f = (int)(neg ? -x : x);
  }
  st_out_b[ST_K_D_HEAD] += (int64_t)(i + (k - n)) * to; st_out_m[ST_K_D_HEAD] += to;   /* the header and the WebSocket's own bytes */
  if (st_ticks % ST_DEEP) return f;
  to *= ST_DEEP;
  while (i < n) {
    const uint8_t *semi = memchr(t + i, ';', n - i);
    size_t e = semi ? (size_t)(semi - t) : n;
    int key = ST_K_ENTRY + (t[i] & 127);
    if (t[i] == 's' && i + 2 < e) {   /* s ID ...: a player, a townsperson (100000 + I) or made (200000 + EID) */
      int64_t id = 0; for (size_t j = i + 2; j < e && t[j] >= '0' && t[j] <= '9'; j++) id = id * 10 + (t[j] - '0');
      key = id >= 200000 ? ST_K_S_MADE : id >= 100000 ? ST_K_S_FOLK : ST_K_S_PLAYER;
    }
    st_out_b[key] += (int64_t)(e - i + (semi ? 1 : 0)) * to; st_out_m[key] += to;
    i = e + 1;
  }
  return f;
}
static void st_frame_in(const uint8_t *t, size_t n) {
  if (!n) return;
  int c = t[0] & 127;
  st_in_frames++; st_in_bytes += (int64_t)n; st_in_b[c] += (int64_t)n; st_in_m[c]++;
}

/* ---- the report */

typedef struct { char *p; size_t n, cap; } st_buf;
static void st_line(st_buf *o, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
#include <stdarg.h>
static void st_line(st_buf *o, const char *fmt, ...) {
  if (o->n + 1 >= o->cap) return;
  va_list a; va_start(a, fmt);
  int k = vsnprintf(o->p + o->n, o->cap - o->n, fmt, a);
  va_end(a);
  if (k < 0 || o->n + (size_t)k + 1 >= o->cap) { o->p[o->n] = 0; return; }
  o->n += (size_t)k; o->p[o->n++] = '\n'; o->p[o->n] = 0;
}
static double st_ms(int64_t us) { return (double)us / 1000.0; }
static void st_key_name(int key, char *out, size_t cap) {
  if (key == ST_K_S_PLAYER) snprintf(out, cap, "D s players");
  else if (key == ST_K_S_FOLK) snprintf(out, cap, "D s townsfolk");
  else if (key == ST_K_S_MADE) snprintf(out, cap, "D s made");
  else if (key == ST_K_D_HEAD) snprintf(out, cap, "D header");
  else if (key >= ST_K_ENTRY) snprintf(out, cap, "D %c", (char)(key - ST_K_ENTRY));
  else snprintf(out, cap, "%c", (char)key);
}

/* The window's report into o (and the window starts again when reset): its lines, every number
   named as the admins read it. secs: the window's length. */
static void st_report(st_buf *o, coax_v_t_RmServer v, int64_t players, int64_t shard, double secs, bool reset) {
  if (secs <= 0) secs = 1;
  st_line(o, "shard=%lld", (long long)shard);
  st_line(o, "players=%lld", (long long)players);
  st_line(o, "ticks a second=%.1f", st_ticks / secs);
  for (int w = 0; w < STW_N; w++) {
    st_line(o, "%s p50 ms=%.2f", st_work_name[w], st_ms(st_hist_at(&st_work[w], 500)));
    st_line(o, "%s p95 ms=%.2f", st_work_name[w], st_ms(st_hist_at(&st_work[w], 950)));
    st_line(o, "%s p99 ms=%.2f", st_work_name[w], st_ms(st_hist_at(&st_work[w], 990)));
  }
  st_line(o, "measuring %% of tick=%.2f", st_tick_us > 0 ? 100.0 * (double)st_cost_us / (double)st_tick_us : 0.0);
  /* the sockets: the worst queue, relevance, input to ACK */
  int64_t qb = 0, qm = 0, rel_n = 0, rel_sum = 0, rel_max = 0;
  for (int i = 0; i < CONNS; i++) {
    if (st_c[i].q_bytes > qb) qb = st_c[i].q_bytes;
    if (st_c[i].q_ms > qm) qm = st_c[i].q_ms;
  }
  for (int i = 0; i < CONNS; i++) {
    int64_t ks[2];
    if (!rm_st_conn(v, i, (coax_v_s64){ks, 2})) continue;
    rel_n++; rel_sum += st_c[i].relevant; if (st_c[i].relevant > rel_max) rel_max = st_c[i].relevant;
  }
  st_line(o, "queue bytes most=%lld", (long long)qb);
  st_line(o, "queue age ms most=%lld", (long long)qm);
  st_line(o, "relevant entities a seat=%.1f", rel_n ? (double)rel_sum / (double)rel_n : 0.0);
  st_line(o, "relevant entities most=%lld", (long long)rel_max);
  /* Additive gauges from the actual production interest picks. No row/byte or
     presentation claim: each seat contributes its most recent pick. */
  int64_t interest_at = now_us();
  int64_t interest[15] = {0}; rm_st_interest(v, (coax_v_s64){interest, 15});
  int64_t interest_us = now_us() - interest_at;
  st_line(o, "interest scope=process latest picks");
  st_line(o, "interest aggregation us=%lld", (long long)interest_us);
  st_line(o, "interest seats=%lld", (long long)interest[0]);
  static const char *interest_names[4] = {"whole", "health only", "blocked", "hidden"};
  for (int j = 0; j < 4; j++) {
    st_line(o, "interest %s total=%lld", interest_names[j], (long long)interest[1+j]);
    st_line(o, "interest %s most=%lld", interest_names[j], (long long)interest[5+j]);
  }
  if (interest[9] >= 0) {
    coax_string code = rm_st_code(v, interest[9]);
    st_line(o, "interest worst|%.*s|%lld|%lld|%lld|%lld|%lld", (int)code.count, code.data,
      (long long)interest[10], (long long)interest[11], (long long)interest[12],
      (long long)interest[13], (long long)interest[14]);
  }
  int64_t aged = 0, wait_most = 0, left_most = 0;
  for (int i = 0; i < CONNS; i++) {
    aged += st_c[i].aged;
    if (st_c[i].wait_most > wait_most) wait_most = st_c[i].wait_most;
    if (st_c[i].left_most > left_most) left_most = st_c[i].left_most;
  }
  st_line(o, "rows sent for their age a second=%.1f", aged / secs);
  st_line(o, "row wait ticks most=%lld", (long long)wait_most);
  st_line(o, "rows left for later most=%lld", (long long)left_most);
  /* the allowances beside the rows' (#12836): their bytes a second, and the entries each left waiting */
  int64_t al[13] = {0}; rm_st_alloc(v, (coax_v_s64){al, 13});
  st_line(o, "removal bytes a second=%.0f", (al[0] - st_alloc_was[0]) / secs);
  st_line(o, "removals waiting a second=%.1f", (al[1] - st_alloc_was[1]) / secs);
  st_line(o, "player info bytes a second=%.0f", (al[2] - st_alloc_was[2]) / secs);
  st_line(o, "player info waiting a second=%.1f", (al[3] - st_alloc_was[3]) / secs);
  st_line(o, "join pieces bytes a second=%.0f", (al[4] - st_alloc_was[4]) / secs);
  st_line(o, "join pieces waiting a second=%.1f", (al[5] - st_alloc_was[5]) / secs);
  st_line(o, "cue bytes a second=%.0f", (al[6] - st_alloc_was[6]) / secs);
  st_line(o, "cues dropped a second=%.1f", (al[7] - st_alloc_was[7]) / secs);
  st_line(o, "outcome bytes a second=%.0f", (al[8] - st_alloc_was[8]) / secs);
  st_line(o, "outcomes lost a second=%.1f", (al[9] - st_alloc_was[9]) / secs);
  st_line(o, "snapshots after a stall a second=%.2f", (al[10] - st_alloc_was[10]) / secs);
  st_line(o, "towns sent again a second=%.2f", (al[11] - st_alloc_was[11]) / secs);
  st_line(o, "snapshots held for a slow page a second=%.1f", (al[12] - st_alloc_was[12]) / secs);
  st_line(o, "input to ack p50 ms=%lld", (long long)st_ack_q(NULL, st_ack_all, st_ack_all_n, 500));
  st_line(o, "input to ack p95 ms=%lld", (long long)st_ack_q(NULL, st_ack_all, st_ack_all_n, 950));
  st_line(o, "joins=%lld", (long long)st_burst_n);
  st_line(o, "join burst bytes=%lld", (long long)(st_burst_n ? st_burst_bytes_sum / st_burst_n : 0));
  st_line(o, "join burst bytes most=%lld", (long long)st_burst_bytes_max);
  st_line(o, "join burst ms=%lld", (long long)(st_burst_n ? st_burst_ms_sum / st_burst_n : 0));
  st_line(o, "join burst ms most=%lld", (long long)st_burst_ms_max);
  int64_t tries = rm_st_re_tries(v), ok = rm_st_re_ok(v);
  st_line(o, "rejoins tried=%lld", (long long)(tries - st_re_tries_was));
  st_line(o, "rejoins seated=%lld", (long long)(ok - st_re_ok_was));
  int64_t same = rm_st_re_seat(v), stale = rm_st_stale(v);
  st_line(o, "rejoins same seat=%lld", (long long)(same - st_re_seat_was));
  st_line(o, "old socket frames dropped=%lld", (long long)(stale - st_stale_was));
  int64_t nr = rm_st_reasons();
  for (int64_t r = 0; r < nr && r < 16; r++) {
    coax_string name = rm_st_reason(r);
    st_line(o, "refused %.*s=%lld", (int)name.count, name.data, (long long)(rm_st_refused(v, r) - st_refused_was[r]));
  }
  st_line(o, "build engine=%lld rules=%lld content=%lld wire=%lld-%lld schema=%lld", (long long)rm_st_pin(0), (long long)rm_st_pin(1),
          (long long)rm_st_pin(2), (long long)rm_st_pin(3), (long long)rm_st_pin(4), (long long)rm_st_pin(5));
  st_line(o, "out bytes a second=%.0f", st_out_bytes / secs);
  st_line(o, "out messages a second=%.0f", st_out_frames / secs);
  st_line(o, "in bytes a second=%.0f", st_in_bytes / secs);
  st_line(o, "in messages a second=%.0f", st_in_frames / secs);
  char kn[32];
  for (int key = 0; key < ST_KEYS; key++) {
    if (!st_out_m[key]) continue;
    st_key_name(key, kn, sizeof kn);
    st_line(o, "out %s bytes a second=%.0f", kn, st_out_b[key] / secs);
    st_line(o, "out %s a second=%.0f", kn, st_out_m[key] / secs);
  }
  for (int c = 0; c < 128; c++) {
    if (!st_in_m[c]) continue;
    st_line(o, "in %c bytes a second=%.0f", (char)c, st_in_b[c] / secs);
    st_line(o, "in %c a second=%.0f", (char)c, st_in_m[c] / secs);
  }
  /* each seat: its room, seat, queue, relevance, input to ACK and round trip */
  for (int i = 0; i < CONNS; i++) {
    int64_t ks[2];
    if (!rm_st_conn(v, i, (coax_v_s64){ks, 2})) continue;
    coax_string code = rm_st_code(v, ks[0]);
    st_line(o, "seat|%.*s|%lld|%lld|%lld|%d|%lld|%lld|%lld", (int)code.count, code.data, (long long)ks[1],
             (long long)st_c[i].q_bytes, (long long)st_c[i].q_ms, st_c[i].relevant, (long long)st_ack_at(&st_c[i], 500),
             (long long)st_ack_at(&st_c[i], 950), (long long)st_c[i].rtt_ms);
  }
  if (!reset) return;
  memset(st_work, 0, sizeof st_work); memset(st_ack_all, 0, sizeof st_ack_all); st_ack_all_n = 0;
  st_ticks = st_tick_us = st_cost_us = 0;
  memset(st_out_b, 0, sizeof st_out_b); memset(st_out_m, 0, sizeof st_out_m); memset(st_in_b, 0, sizeof st_in_b); memset(st_in_m, 0, sizeof st_in_m);
  st_out_frames = st_out_bytes = st_in_frames = st_in_bytes = 0;
  st_burst_n = st_burst_bytes_max = st_burst_ms_max = st_burst_bytes_sum = st_burst_ms_sum = 0;
  st_re_tries_was = tries; st_re_ok_was = ok; st_re_seat_was = same; st_stale_was = stale;
    for (int q = 0; q < 13; q++) st_alloc_was[q] = al[q];
  for (int64_t r = 0; r < nr && r < 16; r++) st_refused_was[r] = rm_st_refused(v, r);
  for (int i = 0; i < CONNS; i++) { st_c[i].q_bytes = st_c[i].q_ms = 0; st_c[i].aged = st_c[i].wait_most = st_c[i].left_most = 0; memset(st_c[i].ack, 0, sizeof st_c[i].ack); st_c[i].ack_n = 0; }
}
