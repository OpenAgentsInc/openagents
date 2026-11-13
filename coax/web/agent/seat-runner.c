// Hosted seats (docs/verse/realm-seat.md slice 5, #12655), included in runner.c: oa-api opens a seat
// (coax/web/api/seats.c, its limits coax/web/api/seat-policy.coax) and asks this runner to hold its
// page, for agents with no Chrome of their own. Each seat is an oa-seat process (seat.coax: the
// seat engine and its headless Chrome) speaking JSON lines on its stdin and stdout.
//
//   POST /seat/open  {"seat": {"id", "slug", "player": {"id", "key"}, ...}, "origin": O}
//                    200 {the first observation} | 409 already open | 429 full | 502 it didn't open
//   POST /seat/act   {"id", "action": {...}}  200 {the observation} | 404 no such seat | 502
//   POST /seat/close {"id"}                   200 | 404
//
// The seat's key reaches oa-seat on its stdin and nowhere else: never an argument, the environment
// or a log. At most SEATS_MOST at once (seat-policy.coax's site limit, kept here too since each is a
// Chrome on this machine); one not acted in for OA_SEAT_IDLE_MS (10 minutes) is closed, as oa-api
// closes it then.
#include <sys/wait.h>
#include "seat-runner-wire.h"   // the bodies (srw_*): deploy.sh compiles seat-runner-wire.coax

#define SEATS_MOST 8
static struct { long long id; pid_t pid; int in, out; double last; } seats[SEATS_MOST];
static double seat_idle_ms(void) { double v = atof(env("OA_SEAT_IDLE_MS", "600000")); return v > 0 ? v : 600000; }

// A line from fd within ms, into out (NUL-terminated, without its newline): its length, or -1.
static long seat_read_line(int fd, char *out, size_t cap, int ms) {
  size_t n = 0; double until = now_ms() + ms;
  while (n + 1 < cap) {
    int left = (int)(until - now_ms());
    if (left <= 0) return -1;
    struct pollfd p = {fd, POLLIN, 0};
    int r = poll(&p, 1, left);
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) return -1;
    char c;
    ssize_t k = read(fd, &c, 1);
    if (k <= 0) return -1;
    if (c == '\n') { out[n] = 0; return (long)n; }
    out[n++] = c;
  }
  return -1;
}
static bool seat_write_line(int fd, const char *s) {
  size_t n = strlen(s);
  while (n) { ssize_t k = write(fd, s, n); if (k <= 0) { if (errno == EINTR) continue; return false; } s += k; n -= (size_t)k; }
  return write(fd, "\n", 1) == 1;
}
static int seat_find(long long id) { for (int i = 0; i < SEATS_MOST; i++) if (seats[i].pid && seats[i].id == id) return i; return -1; }
// Ends seat i: its input closes (oa-seat closes its Chrome and goes), then it is made to go.
static void seat_end(int i) {
  if (!seats[i].pid) return;
  close(seats[i].in); close(seats[i].out);
  for (int k = 0; k < 50; k++) { if (waitpid(seats[i].pid, NULL, WNOHANG) == seats[i].pid) { seats[i].pid = 0; break; } usleep(100000); }
  if (seats[i].pid) { kill(seats[i].pid, SIGKILL); waitpid(seats[i].pid, NULL, 0); }
  say("seat %lld closed", seats[i].id);
  seats[i].pid = 0;
}
// Closes the seats not acted in for the idle limit (called between requests).
static void seats_idle(void) {
  double now = now_ms();
  for (int i = 0; i < SEATS_MOST; i++) if (seats[i].pid && now - seats[i].last >= seat_idle_ms()) seat_end(i);
}
// A seat whose oa-seat went on its own (its browser died, its own idle limit), reaped by runner.c.
static void seat_reaped(pid_t p) {
  for (int i = 0; i < SEATS_MOST; i++) if (seats[i].pid == p) { close(seats[i].in); close(seats[i].out); seats[i].pid = 0; say("seat %lld went", seats[i].id); }
}
static void seats_reap(void) { reap(); }

// The bodies are read and written through seat-runner-wire.coax (seat-runner-wire.c): the open
// body and the seat's first line from it, an act's id and action, a close's id, the answer's ok,
// {"closed":true} and every refusal.
static void seat_refuse(int fd, int status, const char *why) {
  char out[160];
  int64_t n = srw_error_write(cs(why), (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  out[n < 0 ? 0 : n] = 0;
  respond(fd, status, out);
}

static void seat_open(int fd, const char *body) {
  seats_reap();
  long long id = srw_open_id(cs(body));
  char first[1024];   // the seat's first line: it holds the seat's key
  int64_t fn = id > 0 ? srw_start_write(cs(body), (coax_v_u8){(uint8_t *)first, sizeof first - 1}) : -1;
  int slot = -1;
  if (fn < 0) { seat_refuse(fd, 400, "a seat, its slug and an origin"); goto done; }
  first[fn] = 0;
  if (seat_find(id) >= 0) { seat_refuse(fd, 409, "that seat is open"); goto done; }
  for (int i = 0; i < SEATS_MOST; i++) if (!seats[i].pid) { slot = i; break; }
  if (slot < 0) { seat_refuse(fd, 429, "the runner's seats are full"); goto done; }
  int in[2], out[2];
  if (pipe(in) || pipe(out)) { seat_refuse(fd, 502, "no pipe"); goto done; }
  pid_t pid = fork();
  if (pid == 0) {
    dup2(in[0], 0); dup2(out[1], 1);
    // nothing else of the runner's: no other seat's pipes (each must see its own end), no socket
    for (int k = 3; k < 1024; k++) close(k);
    execl(env("OA_SEAT_ENGINE", "/usr/local/bin/oa-seat"), "oa-seat", (char *)NULL);
    _exit(127);
  }
  close(in[0]); close(out[1]);
  if (pid < 0) { close(in[1]); close(out[0]); seat_refuse(fd, 502, "no process"); goto done; }
  seats[slot].id = id; seats[slot].pid = pid; seats[slot].in = in[1]; seats[slot].out = out[0]; seats[slot].last = now_ms();
  char line[262144];
  bool ok = seat_write_line(seats[slot].in, first) && seat_read_line(seats[slot].out, line, sizeof line, 120000) > 0
            && line[0] == '{' && srw_answer_ok(cs(line));
  if (!ok) { seat_end(slot); seat_refuse(fd, 502, "the seat didn't open"); goto done; }
  say("seat %lld open", id);
  respond(fd, 200, line);
done:
  OPENSSL_cleanse(first, sizeof first);
}

// An action: answered by a child process (a walk takes up to 30 s, an answer 45 s), so the runner
// goes on taking requests meanwhile.
static void seat_act(int fd, const char *body) {
  seats_reap();
  int i = seat_find(srw_id(cs(body)));
  if (i < 0) { seat_refuse(fd, 404, "no such seat"); return; }
  static char action[16385];
  int64_t an = srw_action_write(cs(body), (coax_v_u8){(uint8_t *)action, sizeof action - 1});
  if (an < 0) { seat_refuse(fd, 400, "an action is one JSON object"); return; }
  action[an] = 0;
  seats[i].last = now_ms();
  pid_t pid = fork();
  if (pid == 0) {
    static char line[262144];
    bool ok = seat_write_line(seats[i].in, action) && seat_read_line(seats[i].out, line, sizeof line, 150000) > 0 && line[0] == '{';
    if (ok) respond(fd, 200, line); else seat_refuse(fd, 502, "the seat didn't answer");
    _exit(0);
  }
  if (pid < 0) seat_refuse(fd, 502, "busy");
}

static void seat_close(int fd, const char *body) {
  int i = seat_find(srw_id(cs(body)));
  if (i < 0) { seat_refuse(fd, 404, "no such seat"); return; }
  seat_end(i);
  char out[32];
  int64_t n = srw_closed_write((coax_v_u8){(uint8_t *)out, sizeof out - 1});
  out[n < 0 ? 0 : n] = 0;
  respond(fd, 200, out);
}

static bool seat_serve(int fd, const char *req, const char *body) {
  if (!strncmp(req, "POST /seat/open ", 16)) { seat_open(fd, body); return true; }
  if (!strncmp(req, "POST /seat/act ", 15)) { seat_act(fd, body); return true; }
  if (!strncmp(req, "POST /seat/close ", 17)) { seat_close(fd, body); return true; }
  return false;
}
