// oa-script-job: the compile job behind a socket (GitHub #13467; docs/protocol/world-scripts-v1.md, "The
// compile job"; the image is this directory's Dockerfile, the service coax/web/script-job/deploy.sh). The API
// (coax/web/api/scripts.c) asks it through OA_SCRIPT_JOB_URL when it has no compiler of its own. Platform
// effects only: it reads one request a connection, hands the source to the job exactly as the API would
// (coax/web/api/script-job-run.c, the job coax/web/script-job.sh) and answers what the job made or said.
// Nothing here decides what a source may be: the job does, and the API checks the module again.
//
//   POST /compile/NAME   body: a script's Coax source, at most 65,536 bytes.
//                        200  application/wasm: the module; X-Script-Manifest: the job's manifest, one line
//                             of realm.script.v1, which the API believes only for the name and state fields
//                        422  text/plain: the job refused the source, in its own words
//                        500  text/plain: the job could not be run, made nothing or ran past its time
//                        400, 404, 405, 411, 413: the request itself
//   GET  /healthz        200 "ok"
//
//   oa-script-job [--listen HOST:PORT]     PORT from the environment (Cloud Run) when set, on 0.0.0.0
//
// Environment: OA_SCRIPT_JOB the job's command (the image: /opt/openagents/coax/web/script-job.sh), and what
// the job is handed (PATH, TMPDIR, COAX, COAXIAL_CLANG, COAXIAL_RESOURCES, OA_SCRIPT_JOB_SANDBOX). Nothing
// else: the service holds no key, token or database address, and Cloud Run lets only the API's account call it.
// One request at a time, in this process (Cloud Run gives it one at a time). One log line a request on
// stderr: the status, the source's size and the time; never the source or the words.
#include "script-job-run.h"

#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#define HEAD_MAX 8192
#define SOURCE_MAX 65536          // script-job.coax SJ_SOURCE: a longer one is refused here, before the job
#define JOB_WAIT_MS 70000         // the job's own limit is 60 s
#define READ_MS 15000             // a client that sends nothing for this long is let go

static double now_ms(void) { struct timeval t; gettimeofday(&t, NULL); return t.tv_sec * 1000.0 + t.tv_usec / 1000.0; }
static void say(int status, size_t bytes, double ms) {
  char t[32]; time_t s = time(NULL); strftime(t, sizeof t, "%Y-%m-%dT%H:%M:%SZ", gmtime(&s));
  fprintf(stderr, "%s oa-script-job: %d, a source of %zu bytes, %.0f ms\n", t, status, bytes, ms);
}
static bool send_all(int fd, const char *p, size_t n) {
  while (n) { ssize_t k = write(fd, p, n); if (k < 0 && errno == EINTR) continue; if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
static const char *reason(int status) {
  switch (status) {
    case 200: return "OK"; case 400: return "Bad Request"; case 404: return "Not Found"; case 405: return "Method Not Allowed";
    case 411: return "Length Required"; case 413: return "Content Too Large"; case 422: return "Unprocessable Content";
    default: return "Internal Server Error";
  }
}
// One answer: the status, a body of its type, and the manifest header when there is one.
static void reply(int fd, int status, const char *type, const char *body, size_t n, const char *manifest) {
  char head[SCRIPT_JOB_MANIFEST + 512];
  int h = snprintf(head, sizeof head, "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\n%s%s%sCache-Control: no-store\r\nConnection: close\r\n\r\n",
                   status, reason(status), type, n, manifest ? "X-Script-Manifest: " : "", manifest ? manifest : "", manifest ? "\r\n" : "");
  if (h > 0 && (size_t)h < sizeof head && send_all(fd, head, (size_t)h)) send_all(fd, body, n);
}
static void text(int fd, int status, const char *words) { reply(fd, status, "text/plain; charset=utf-8", words, strlen(words), NULL); }

// One connection: the head, then Content-Length bytes of body; then the job.
static void serve_one(int fd, const char *job) {
  static char in[HEAD_MAX + SOURCE_MAX + 1];
  static char wasm[SCRIPT_JOB_MODULE], made[SCRIPT_JOB_MANIFEST + 1], words[SCRIPT_JOB_WORDS];
  double started = now_ms();
  size_t got = 0, want = 0; char *end = NULL;
  for (;;) {
    struct pollfd p = {fd, POLLIN, 0};
    double left = READ_MS - (now_ms() - started);
    if (left <= 0 || poll(&p, 1, (int)left) <= 0) return;
    ssize_t k = read(fd, in + got, sizeof in - 1 - got);
    if (k < 0 && errno == EINTR) continue;
    if (k <= 0) return;
    got += (size_t)k; in[got] = 0;
    if (!end) {
      end = strstr(in, "\r\n\r\n");
      if (!end) { if (got >= HEAD_MAX) { text(fd, 413, "the request's head is too long\n"); return; } continue; }
      // the request line and Content-Length: one, a number, no Transfer-Encoding
      int lengths = 0;
      for (char *line = strstr(in, "\r\n"); line && line < end; line = strstr(line + 2, "\r\n")) {
        if (!strncasecmp(line + 2, "Transfer-Encoding:", 18)) { text(fd, 400, "no transfer coding\n"); return; }
        if (strncasecmp(line + 2, "Content-Length:", 15)) continue;
        char *v = line + 17, *e = NULL;
        while (*v == ' ') v++;
        errno = 0; unsigned long n = strtoul(v, &e, 10);
        if (e == v || errno || (*e != '\r' && *e != ' ')) { text(fd, 400, "a Content-Length that is not a number\n"); return; }
        want = n; lengths++;
      }
      if (lengths > 1) { text(fd, 400, "two Content-Lengths\n"); return; }
      bool post = !strncmp(in, "POST /compile/", 14);
      if (!strncmp(in, "GET /healthz HTTP/1.", 20) && lengths == 0) { text(fd, 200, "ok\n"); return; }
      if (!post) { text(fd, strncmp(in, "GET ", 4) && strncmp(in, "POST ", 5) ? 405 : 404, "POST /compile/NAME, or GET /healthz\n"); return; }
      if (lengths == 0) { text(fd, 411, "a source's length\n"); return; }
      if (want > SOURCE_MAX) { text(fd, 413, "the source is over 65,536 bytes\n"); return; }
    }
    if (got - (size_t)(end + 4 - in) >= want) break;
  }
  // the name: POST /compile/NAME HTTP/1.x
  char name[64] = "";
  const char *n0 = in + 14, *sp = strchr(n0, ' ');
  if (sp && sp > n0 && (size_t)(sp - n0) < sizeof name && sp < end && !strncmp(sp, " HTTP/1.", 8)) { memcpy(name, n0, (size_t)(sp - n0)); name[sp - n0] = 0; }
  if (!script_job_name_safe(name)) { text(fd, 404, "a script's name is lowercase letters, digits and hyphens, at most 48\n"); say(404, want, now_ms() - started); return; }
  long wasm_n = 0, made_n = 0;
  int ran = script_job_run(job, name, end + 4, want, JOB_WAIT_MS, wasm, &wasm_n, made, &made_n, words, sizeof words);
  int status = 500;
  if (ran == 0) {
    // the manifest goes in a header: one line of printable ASCII, else the job made nothing usable
    made[made_n] = 0;
    bool line = true;
    for (long i = 0; i < made_n; i++) if ((unsigned char)made[i] < 32 || (unsigned char)made[i] > 126) line = false;
    if (line) { status = 200; reply(fd, 200, "application/wasm", wasm, (size_t)wasm_n, made); }
    else text(fd, 500, "the compile job's manifest is not one line\n");
  } else if (ran == 1) { status = 422; text(fd, 422, words); }
  else text(fd, 500, words);
  say(status, want, now_ms() - started);
}

int main(int argc, char **argv) {
  const char *listen_at = NULL;
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], "--listen")) listen_at = argv[++i];
  const char *job = getenv("OA_SCRIPT_JOB");
  if (!job || !*job || access(job, X_OK) != 0) { fprintf(stderr, "oa-script-job: OA_SCRIPT_JOB names no command; not starting\n"); return 2; }
  char host[256] = "127.0.0.1"; int port = 8080;
  const char *env_port = getenv("PORT");
  if (env_port && *env_port) { snprintf(host, sizeof host, "0.0.0.0"); port = atoi(env_port); }
  if (listen_at) { const char *colon = strrchr(listen_at, ':'); if (colon) { snprintf(host, sizeof host, "%.*s", (int)(colon - listen_at), listen_at); port = atoi(colon + 1); } }
  int ls = socket(AF_INET, SOCK_STREAM, 0), one = 1;
  setsockopt(ls, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0}; a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  if (inet_pton(AF_INET, host, &a.sin_addr) != 1 || bind(ls, (struct sockaddr *)&a, sizeof a) != 0 || listen(ls, 16) != 0) {
    fprintf(stderr, "oa-script-job: cannot listen on %s:%d: %s\n", host, port, strerror(errno)); return 1;
  }
  signal(SIGPIPE, SIG_IGN);
  fprintf(stderr, "oa-script-job: listening on %s:%d; the job is %s\n", host, port, job);
  for (;;) {
    int c = accept(ls, NULL, NULL);
    if (c < 0) { if (errno == EINTR) continue; usleep(100000); continue; }
    serve_one(c, job);
    shutdown(c, SHUT_WR); close(c);
  }
}
