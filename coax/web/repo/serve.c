// oa-repo's host (docs/repos/2026-11-11-indexed-repos.md, "The service"; GitHub #13326;
// runbook docs/deployment/repo-vm.md). Platform effects only: it listens, forks one process
// per connection, reads the request's bytes, hands them to serve.coax (repo_head reads the
// head; repo_serve decides and answers), writes what that returns, and fetches every mirror
// on a timer in a process of its own. Every rule above the socket is serve.coax's.
//
//   oa-repo --listen 0.0.0.0:8080
//
// Environment (systemd's /etc/oa-repo/UNIT.env and UNIT.secret.env):
//   OA_REPO_GRANT_KEY  64 hex digits shared with the minter; read, then removed from the
//                      environment, so no git process ever inherits it. Required.
//   OA_REPO_DIR        mirrors live in DIR/mirrors/SLUG.git (default ./oa-repo-data)
//   OA_REPO_REPOS      "slug=url;slug=url" (default the public openagents repository)
//   OA_GIT             git (default "git")
//   OA_REPO_FETCH_S    seconds between fetches of every mirror (default 60)
//
// A connection's process has 30 seconds. At most OA_REPO_MAX (64) run at once; past that a
// connection is answered 503. One log line per request on stderr (journald): the route, the
// operation, the status, the answer's size and the time; never a grant or a body.
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#include "serve-lib.h"   // build.sh compiles serve.coax: repo_head, repo_serve, repo_fetch_all, repo_response_head

#define HEAD_MAX 16384
#define BODY_MAX 65536
#define WORK_BYTES (4194304 + 65536)
#define ANSWER_BYTES (65536 * 6 + 1024)
#define MAX_KIDS 64

static char key[65], dir[1024], repos[4096], git[512];
static long fetch_every = 60;
static volatile sig_atomic_t stopping;

static double now_ms(void) { struct timeval t; gettimeofday(&t, NULL); return t.tv_sec * 1000.0 + t.tv_usec / 1000.0; }
static coax_string cs(const char *s, size_t n) { return (coax_string){s, (int64_t)n}; }
static coax_string cz(const char *s) { return cs(s, strlen(s)); }
static void say(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
static void say(const char *fmt, ...) {
  va_list a; va_start(a, fmt);
  char t[32]; time_t s = time(NULL); strftime(t, sizeof t, "%Y-%m-%dT%H:%M:%SZ", gmtime(&s));
  fprintf(stderr, "%s oa-repo: ", t); vfprintf(stderr, fmt, a); fputc('\n', stderr);
  va_end(a);
}
static bool send_all(int fd, const char *p, size_t n) {
  while (n) { ssize_t k = write(fd, p, n); if (k < 0 && errno == EINTR) continue; if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
static void reply(int fd, int status, const char *body, size_t n, bool json) {
  char head[512];
  int64_t h = repo_response_head(status, (int64_t)n, json, (coax_v_u8){(uint8_t *)head, sizeof head});
  if (send_all(fd, head, (size_t)h)) send_all(fd, body, n);
}

// One connection, in its own process.
static void serve_one(int fd) {
  alarm(30);
  double started = now_ms();
  static char in[HEAD_MAX + BODY_MAX];
  size_t got = 0;
  coax_t_RsHead h = {0};
  for (;;) {
    ssize_t k = read(fd, in + got, sizeof in - got);
    if (k < 0 && errno == EINTR) continue;
    if (k <= 0) return;
    got += (size_t)k;
    h = repo_head(cs(in, got < HEAD_MAX ? got : HEAD_MAX));
    if (h.state != 0) break;
  }
  if (h.state == 200) {
    size_t want = (size_t)h.head_n + (size_t)h.length;
    while (got < want) {
      ssize_t k = read(fd, in + got, want - got);
      if (k < 0 && errno == EINTR) continue;
      if (k <= 0) return;
      got += (size_t)k;
    }
  }
  if (h.state != 200) { char m[64]; int n = snprintf(m, sizeof m, "refused (%d)\n", (int)h.state); reply(fd, (int)h.state, m, (size_t)n, false); say("- %d", (int)h.state); return; }
  if (h.route == 3) { reply(fd, 200, "ok\n", 3, false); return; }   // RS_HEALTH
  char *work = malloc(WORK_BYTES), *out = malloc(ANSWER_BYTES);
  if (!work || !out) { reply(fd, 500, "out of memory\n", 14, false); return; }
  coax_string auth = h.auth_to > h.auth_from ? cs(in + h.auth_from, (size_t)(h.auth_to - h.auth_from)) : cs("", 0);
  coax_t_RsReply r = repo_serve(h.route, auth, cs(in + h.head_n, (size_t)h.length), cz(repos), cz(dir), cz(git), cz(key), (int64_t)time(NULL),
                                (coax_v_u8){(uint8_t *)work, WORK_BYTES}, (coax_v_u8){(uint8_t *)out, ANSWER_BYTES});
  if (r.n > 0) reply(fd, (int)r.status, out, (size_t)r.n, true);
  else reply(fd, 500, "{\"error\":\"the answer could not be written\"}", 43, true);
  coax_string op = repo_op_name(r.op);
  say("%s %.*s %d %lld bytes %.0f ms", h.route == 2 ? "fetch" : "call", (int)op.count, op.data, (int)r.status, (long long)r.n, now_ms() - started);
  free(work); free(out);
}

static void on_term(int s) { (void)s; stopping = 1; }

int main(int argc, char **argv) {
  const char *listen_at = getenv("OA_REPO_LISTEN") ? getenv("OA_REPO_LISTEN") : "127.0.0.1:8080";
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], "--listen")) listen_at = argv[++i];
  const char *k = getenv("OA_REPO_GRANT_KEY");
  if (!k || strlen(k) != 64) { say("OA_REPO_GRANT_KEY is missing or not 64 hex digits; not starting"); return 2; }
  snprintf(key, sizeof key, "%s", k);
  unsetenv("OA_REPO_GRANT_KEY");
  snprintf(dir, sizeof dir, "%s", getenv("OA_REPO_DIR") ? getenv("OA_REPO_DIR") : "oa-repo-data");
  snprintf(repos, sizeof repos, "%s", getenv("OA_REPO_REPOS") ? getenv("OA_REPO_REPOS") : "openagents=https://github.com/OpenAgentsInc/openagents.git");
  snprintf(git, sizeof git, "%s", getenv("OA_GIT") ? getenv("OA_GIT") : "git");
  if (getenv("OA_REPO_FETCH_S")) fetch_every = atol(getenv("OA_REPO_FETCH_S"));
  if (fetch_every < 10) fetch_every = 10;
  setenv("GIT_TERMINAL_PROMPT", "0", 1);
  setenv("GIT_CONFIG_NOSYSTEM", "1", 1);
  char mirrors[1100]; snprintf(mirrors, sizeof mirrors, "%s/mirrors", dir);
  mkdir(dir, 0700); mkdir(mirrors, 0700);

  char host[256] = "127.0.0.1"; int port = 8080;
  const char *colon = strrchr(listen_at, ':');
  if (colon) { snprintf(host, sizeof host, "%.*s", (int)(colon - listen_at), listen_at); port = atoi(colon + 1); }
  int ls = socket(AF_INET, SOCK_STREAM, 0), one = 1;
  setsockopt(ls, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0}; a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  if (inet_pton(AF_INET, host, &a.sin_addr) != 1 || bind(ls, (struct sockaddr *)&a, sizeof a) != 0 || listen(ls, 128) != 0) {
    say("cannot listen on %s: %s", listen_at, strerror(errno)); return 1;
  }
  signal(SIGPIPE, SIG_IGN);
  struct sigaction t = {0}; t.sa_handler = on_term; sigaction(SIGTERM, &t, NULL); sigaction(SIGINT, &t, NULL);
  say("listening on %s; mirrors in %s; a fetch every %lds", listen_at, mirrors, fetch_every);

  int kids = 0; pid_t fetcher = 0; time_t next_fetch = 0; double fetch_started = 0;
  while (!stopping) {
    int st; pid_t p;
    while ((p = waitpid(-1, &st, WNOHANG)) > 0) {
      if (p == fetcher) {
        fetcher = 0;
        say("fetch: %s in %.0f ms", WIFEXITED(st) && WEXITSTATUS(st) == 0 ? "every mirror is current" : "a mirror could not be fetched", now_ms() - fetch_started);
      } else if (kids > 0) kids--;
    }
    if (!fetcher && time(NULL) >= next_fetch) {
      next_fetch = time(NULL) + fetch_every; fetch_started = now_ms();
      fetcher = fork();
      if (fetcher == 0) { close(ls); _exit(repo_fetch_all(cz(repos), cz(dir), cz(git)) == 0 ? 0 : 1); }
      if (fetcher < 0) fetcher = 0;
    }
    struct pollfd pf = {ls, POLLIN, 0};
    if (poll(&pf, 1, 1000) <= 0) continue;
    int c = accept(ls, NULL, NULL);
    if (c < 0) continue;
    if (kids >= MAX_KIDS) { reply(c, 503, "busy\n", 5, false); close(c); say("- 503 busy"); continue; }
    pid_t kid = fork();
    if (kid == 0) { close(ls); serve_one(c); shutdown(c, SHUT_WR); close(c); _exit(0); }
    if (kid > 0) kids++;
    close(c);
  }
  say("stopping");
  return 0;
}
