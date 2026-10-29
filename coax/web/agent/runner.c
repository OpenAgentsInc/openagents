// oa-agent: the project agent's runner (docs/project-agent.md), on Cloud Run
// with scale to zero. It holds what the model never sees: the runner's Nostr
// key, the repositories' deploy keys and git. oa-api wakes it with a project's
// slug; it then takes that project's queued turns one at a time
// (projects.api next), runs each, and ends it (end):
//
//   an ask:  a fresh shallow checkout of the linked repository's main in a new
//            directory; the engine runs the model there; every changed file
//            becomes one commit, "agent turn <id>: <first line>" (only when the
//            turn ended done), rebased on
//            main and pushed. A conflict with a member's commit is not pushed:
//            the turn says so and shows the diff's files.
//   an undo: git revert of the turn's commit, pushed (oa-api has already put
//            the document back).
//
// The engine (OA_AGENT_ENGINE, coax/web/agent/turn.coax) is the model's side:
// the Coax Claude Agent SDK driving the claude CLI with the project tools (in-
// process SDK MCP tools) and Read, Glob, Grep, Edit and Write inside the
// checkout. It runs there with only ANTHROPIC_API_KEY, HOME, PATH and
// OA_AGENT_MODEL, and speaks JSON lines:
//   in   {"t":"turn", "turn":{...}, "history":[...], "version":N, "model":M, "budget_usd":X, "slug":S, "repo":B, "cwd":DIR}
//        {"t":"reply", "id":K, "status":S, "body":B}     the answer to a call
//        {"t":"stop"}                                     a member pressed STOP
//   out  {"t":"call", "id":K, "tool":T, "args":{...}}    a project tool; the runner makes it one oa-api op:
//          project_get → state; project_note, project_plan → note; milestone_status, checklist_status → status;
//          milestone_edit, goal_propose → the op of that name; document_patch → patch
//        {"t":"event", "type":T, "data":{...}}            working, agent.message, agent.tool_use, session.error
//        {"t":"done", "status":"done"|"failed"|"stopped", "answer":A, "error":E, "cost_usd":C}
// The runner adds the turn to every call and signs it; events go to oa-api in
// batches every 4 s, and each batch's answer says whether STOP was pressed.
//
//   oa-agent [--listen HOST:PORT]
//   POST /wake {"slug": S}   202; the turn loop runs on (Cloud Run: CPU always allocated)
//   GET  /                   200
//
// Environment (Secret Manager at deploy time; coax/web/agent/deploy.sh):
//   OA_API_ORIGIN       https://openagents.com
//   OA_RUNNER_KEY       the runner's Nostr secret key, 64 hex (registered in each project with role runner)
//   OA_AGENT_REPOS      "slug=git-url=key-file" entries, ';' between them; key-file empty for none
//   ANTHROPIC_API_KEY   handed to the engine only, then removed from this process's environment
//   OA_AGENT_MODEL      claude-sonnet-5-5
//   OA_AGENT_ENGINE     the engine's path (/usr/local/bin/oa-turn)
//   OA_AGENT_WORK       where checkouts go (/tmp/oa-agent)
// Logs keep turn ids, event types, statuses and costs: never messages, document contents or keys.
#define _GNU_SOURCE
#include <curl/curl.h>
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <openssl/evp.h>
#include <openssl/sha.h>
#include <poll.h>
#include <secp256k1.h>
#include <secp256k1_extrakeys.h>
#include <secp256k1_schnorrsig.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static const char *env(const char *k, const char *d) { const char *v = getenv(k); return v && *v ? v : d; }
static void say(const char *fmt, ...) {
  va_list a; va_start(a, fmt);
  fprintf(stderr, "oa-agent  "); vfprintf(stderr, fmt, a); fputc('\n', stderr);
  va_end(a);
}
static double now_ms(void) { struct timeval t; gettimeofday(&t, NULL); return t.tv_sec * 1000.0 + t.tv_usec / 1000.0; }
static char anthropic_key[512];

// ---- growable text
typedef struct { char *p; size_t n, cap; } text;
static void put(text *t, const char *s, size_t n) {
  if (t->n + n + 1 > t->cap) {
    size_t cap = (t->n + n + 1) * 2;
    char *p = realloc(t->p, cap);
    if (!p) { say("out of memory"); exit(1); }
    t->p = p; t->cap = cap;
  }
  memcpy(t->p + t->n, s, n); t->n += n; t->p[t->n] = 0;
}
static void puts_(text *t, const char *s) { put(t, s, strlen(s)); }
static void putf(text *t, const char *fmt, ...) {
  char buf[4096]; va_list a; va_start(a, fmt);
  int n = vsnprintf(buf, sizeof buf, fmt, a); va_end(a);
  if (n > 0) put(t, buf, (size_t)n < sizeof buf ? (size_t)n : sizeof buf - 1);
}
// s as a JSON string, quotes included.
static void putq(text *t, const char *s) {
  puts_(t, "\"");
  for (const unsigned char *c = (const unsigned char *)s; *c; c++) {
    if (*c == '"' || *c == '\\') { char e[2] = {'\\', (char)*c}; put(t, e, 2); }
    else if (*c == '\n') puts_(t, "\\n");
    else if (*c < 0x20) putf(t, "\\u%04x", *c);
    else put(t, (const char *)c, 1);
  }
  puts_(t, "\"");
}

// ---- just enough JSON: the span of a top-level member's value, and a string's contents
static const char *skip_ws(const char *p) { while (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t') p++; return p; }
static const char *skip_value(const char *p) {
  p = skip_ws(p);
  if (*p == '"') { for (p++; *p && *p != '"'; p++) if (*p == '\\' && p[1]) p++; return *p ? p + 1 : p; }
  if (*p == '{' || *p == '[') {
    int depth = 0;
    for (; *p; p++) {
      if (*p == '"') { for (p++; *p && *p != '"'; p++) if (*p == '\\' && p[1]) p++; if (!*p) return p; continue; }
      if (*p == '{' || *p == '[') depth++;
      else if (*p == '}' || *p == ']') { if (--depth == 0) return p + 1; }
    }
    return p;
  }
  while (*p && *p != ',' && *p != '}' && *p != ']' && *p != ' ' && *p != '\n') p++;
  return p;
}
// The value of key in the object at json, as a span, or false.
static bool jget(const char *json, const char *key, const char **at, size_t *len) {
  const char *p = skip_ws(json);
  if (*p != '{') return false;
  p++;
  size_t kn = strlen(key);
  for (;;) {
    p = skip_ws(p);
    if (*p != '"') return false;
    const char *k = p + 1, *e = skip_value(p);
    bool match = (size_t)(e - k - 1) == kn && !strncmp(k, key, kn);
    p = skip_ws(e);
    if (*p != ':') return false;
    const char *v = skip_ws(p + 1), *ve = skip_value(v);
    if (match) { *at = v; *len = (size_t)(ve - v); return true; }
    p = skip_ws(ve);
    if (*p != ',') return false;
    p++;
  }
}
// A member as its own string (a raw JSON value), or "" / dflt.
static char *jraw(const char *json, const char *key, const char *dflt) {
  const char *at; size_t n;
  if (!jget(json, key, &at, &n)) return strdup(dflt);
  char *s = malloc(n + 1); memcpy(s, at, n); s[n] = 0; return s;
}
// A string member's contents, unescaped (\uXXXX below 0x80 kept, others as '?'), or dflt.
static char *jstr(const char *json, const char *key, const char *dflt) {
  const char *at; size_t n;
  if (!jget(json, key, &at, &n) || *at != '"') return strdup(dflt);
  char *s = malloc(n + 1), *o = s;
  for (const char *p = at + 1; p < at + n - 1; p++) {
    if (*p != '\\') { *o++ = *p; continue; }
    p++;
    if (*p == 'n') *o++ = '\n'; else if (*p == 't') *o++ = '\t'; else if (*p == 'r') *o++ = '\r';
    else if (*p == 'u') { unsigned v = 0; sscanf(p + 1, "%4x", &v); *o++ = v < 0x80 ? (char)v : '?'; p += 4; }
    else *o++ = *p;
  }
  *o = 0; return s;
}
static long long jint(const char *json, const char *key) { char *r = jraw(json, key, "0"); long long v = atoll(r); free(r); return v; }

// ---- NIP-98: the runner signs each request to oa-api (as coax/web/api/sign.c does)
static secp256k1_context *ctx;
static secp256k1_keypair pair;
static char pubhex[65];
static void hex(const unsigned char *in, size_t n, char *out) { for (size_t i = 0; i < n; i++) snprintf(out + 2 * i, 3, "%02x", in[i]); }
static bool load_key(void) {
  const char *k = getenv("OA_RUNNER_KEY");
  if (!k || strlen(k) < 64) return false;
  unsigned char key[32];
  for (int i = 0; i < 32; i++) { unsigned v; if (sscanf(k + 2 * i, "%2x", &v) != 1) return false; key[i] = (unsigned char)v; }
  ctx = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
  bool ok = secp256k1_keypair_create(ctx, &pair, key);
  memset(key, 0, sizeof key);
  unsetenv("OA_RUNNER_KEY");
  if (!ok) return false;
  secp256k1_xonly_pubkey x; unsigned char pub[32];
  secp256k1_keypair_xonly_pub(ctx, &x, NULL, &pair);
  secp256k1_xonly_pubkey_serialize(ctx, pub, &x);
  hex(pub, 32, pubhex);
  return true;
}
static void nip98(const char *method, const char *url, const char *body, char *out, size_t cap) {
  char payload[65] = "";
  if (body) { unsigned char d[32]; SHA256((const unsigned char *)body, strlen(body), d); hex(d, 32, payload); }
  long long t = (long long)time(NULL);
  text tags = {0}, serial = {0}, ev = {0};
  puts_(&tags, "[[\"u\","); putq(&tags, url); puts_(&tags, "],[\"method\","); putq(&tags, method); puts_(&tags, "]");
  if (body) putf(&tags, ",[\"payload\",\"%s\"]", payload);
  puts_(&tags, "]");
  putf(&serial, "[0,\"%s\",%lld,27235,%s,\"\"]", pubhex, t, tags.p);
  unsigned char id[32], sig[64], aux[32];
  SHA256((const unsigned char *)serial.p, serial.n, id);
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, aux, 32) != 32) memset(aux, 7, 32);
  if (fd >= 0) close(fd);
  secp256k1_schnorrsig_sign32(ctx, sig, id, &pair, aux);
  char idhex[65], sighex[129]; hex(id, 32, idhex); hex(sig, 64, sighex);
  putf(&ev, "{\"id\":\"%s\",\"pubkey\":\"%s\",\"created_at\":%lld,\"kind\":27235,\"tags\":%s,\"content\":\"\",\"sig\":\"%s\"}", idhex, pubhex, t, tags.p, sighex);
  unsigned char *b64 = malloc(ev.n * 2 + 8);
  EVP_EncodeBlock(b64, (const unsigned char *)ev.p, (int)ev.n);
  snprintf(out, cap, "Authorization: Nostr %s", b64);
  free(b64); free(tags.p); free(serial.p); free(ev.p);
}

// ---- oa-api
static size_t take(char *d, size_t s, size_t n, void *u) { put(u, d, s * n); return s * n; }
static unsigned long nonce;
// One signed request to /api/private/<slug>/<op>; the answer's body in *out. Every request
// differs (a nonce in the body or the query), since oa-api takes each signed event once.
static long api(const char *method, const char *slug, const char *op, const char *body, text *out) {
  char url[1024], auth[8192];
  bool post = !strcmp(method, "POST");
  snprintf(url, sizeof url, "%s/api/private/%s/%s%s", env("OA_API_ORIGIN", "https://openagents.com"), slug, op, post ? "" : "?n=");
  if (!post) { size_t k = strlen(url); snprintf(url + k, sizeof url - k, "%lu-%lld", ++nonce, (long long)now_ms()); }
  text b = {0};
  if (post) {
    const char *in = body && *body ? body : "{}";
    const char *end = strrchr(in, '}');
    put(&b, in, end ? (size_t)(end - in) : strlen(in));
    putf(&b, "%s\"nonce\":\"%lu-%lld\"}", (end && skip_ws(in + 1) != end) ? "," : "", ++nonce, (long long)now_ms());
  }
  nip98(method, url, post ? b.p : NULL, auth, sizeof auth);
  long status = 0;
  for (int attempt = 0; attempt < 3; attempt++) {
    CURL *h = curl_easy_init();
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    out->n = 0; if (out->p) out->p[0] = 0;
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    if (post) curl_easy_setopt(h, CURLOPT_POSTFIELDS, b.p);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, out);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 30L);
    status = 0;
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
    if (status && status != 429 && status < 500) break;
    sleep(attempt + 2);
    // a retry is a new request: sign it again (the event may have been taken)
    if (post) {
      b.n = 0; const char *in = body && *body ? body : "{}"; const char *end = strrchr(in, '}');
      put(&b, in, end ? (size_t)(end - in) : strlen(in));
      putf(&b, "%s\"nonce\":\"%lu-%lld\"}", (end && skip_ws(in + 1) != end) ? "," : "", ++nonce, (long long)now_ms());
    }
    nip98(method, url, post ? b.p : NULL, auth, sizeof auth);
  }
  if (!out->p) puts_(out, "");
  free(b.p);
  if (status != 200) say("%s %s: %ld", method, op, status);
  return status;
}

// ---- processes: run argv in dir with a clean environment; its output in *out (may be NULL)
static int run(const char *dir, const char *const argv[], char *const envp[], text *out) {
  int fds[2]; if (pipe(fds)) return -1;
  pid_t pid = fork();
  if (pid == 0) {
    if (dir && chdir(dir)) _exit(127);
    dup2(fds[1], 1); dup2(fds[1], 2); close(fds[0]); close(fds[1]);
    execve(argv[0], (char *const *)argv, envp);
    _exit(127);
  }
  close(fds[1]);
  char buf[4096]; ssize_t k;
  while ((k = read(fds[0], buf, sizeof buf)) > 0) if (out) put(out, buf, (size_t)k);
  close(fds[0]);
  int st = 0; waitpid(pid, &st, 0);
  return WIFEXITED(st) ? WEXITSTATUS(st) : -1;
}
static const char *git_bin(void) { return env("OA_GIT", "/usr/bin/git"); }
static char git_env_ssh[1200];
static char *git_envp[8];
static void git_env(const char *keyfile, const char *home) {
  static char h[600], path[] = "PATH=/usr/local/bin:/usr/bin:/bin", name[] = "GIT_AUTHOR_NAME=Project agent",
              mail[] = "GIT_AUTHOR_EMAIL=agent@openagents.com", cname[] = "GIT_COMMITTER_NAME=Project agent",
              cmail[] = "GIT_COMMITTER_EMAIL=agent@openagents.com";
  snprintf(h, sizeof h, "HOME=%s", home);
  int n = 0;
  git_envp[n++] = h; git_envp[n++] = path; git_envp[n++] = name; git_envp[n++] = mail; git_envp[n++] = cname; git_envp[n++] = cmail;
  if (keyfile && *keyfile) {
    snprintf(git_env_ssh, sizeof git_env_ssh, "GIT_SSH_COMMAND=ssh -i %s -o IdentitiesOnly=yes -o UserKnownHostsFile=%s -o StrictHostKeyChecking=yes",
             keyfile, env("OA_KNOWN_HOSTS", "/etc/oa-agent/known_hosts"));
    git_envp[n++] = git_env_ssh;
  }
  git_envp[n] = NULL;
}
static int git7(const char *dir, text *out, const char *a, const char *b, const char *c, const char *d, const char *e, const char *f, const char *g) {
  const char *argv[10] = {git_bin(), a, b, c, d, e, f, g, NULL};
  return run(dir, argv, git_envp, out);
}
static int git(const char *dir, text *out, const char *a, const char *b, const char *c, const char *d, const char *e, const char *f) {
  const char *argv[10] = {git_bin(), a, b, c, d, e, f, NULL};
  return run(dir, argv, git_envp, out);
}

// The project's repository from OA_AGENT_REPOS: url and key file, or false.
static bool repo_of(const char *slug, char *url, size_t ucap, char *key, size_t kcap) {
  const char *list = env("OA_AGENT_REPOS", "");
  size_t n = strlen(slug);
  for (const char *p = list; *p; ) {
    while (*p == ';' || *p == ' ' || *p == '\n') p++;
    size_t len = strcspn(p, ";\n");
    if (len > n + 1 && !strncmp(p, slug, n) && p[n] == '=') {
      const char *u = p + n + 1, *eq = memchr(u, '=', (size_t)(p + len - u));
      size_t ul = eq ? (size_t)(eq - u) : (size_t)(p + len - u);
      snprintf(url, ucap, "%.*s", (int)ul, u);
      if (eq) snprintf(key, kcap, "%.*s", (int)(p + len - eq - 1), eq + 1); else key[0] = 0;
      return true;
    }
    p += len;
  }
  return false;
}

// A fresh directory for one turn, with the deploy key copied in beside (never inside) the checkout, 0600.
static bool turn_dir(long long id, const char *keysrc, char *dir, size_t cap, char *key, size_t kcap) {
  snprintf(dir, cap, "%s/turn-%lld-%d", env("OA_AGENT_WORK", "/tmp/oa-agent"), id, (int)getpid());
  mkdir(env("OA_AGENT_WORK", "/tmp/oa-agent"), 0700);
  if (mkdir(dir, 0700)) return false;
  key[0] = 0;
  if (keysrc && *keysrc) {
    snprintf(key, kcap, "%s/deploy-key", dir);
    FILE *in = fopen(keysrc, "rb");
    int fd = open(key, O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (!in || fd < 0) { if (in) fclose(in); if (fd >= 0) close(fd); return false; }
    char buf[4096]; size_t k;
    while ((k = fread(buf, 1, sizeof buf, in)) > 0) if (write(fd, buf, k) != (ssize_t)k) break;
    fclose(in); close(fd);
  }
  return true;
}
static void remove_dir(const char *dir) {
  const char *argv[] = {"/bin/rm", "-rf", dir, NULL};
  char *envp[] = {NULL};
  run(NULL, argv, envp, NULL);
}

// ---- the engine
typedef struct { pid_t pid; int in, out; } engine;
static bool engine_start(engine *e, const char *dir, const char *home) {
  int a[2], b[2];
  if (pipe(a) || pipe(b)) return false;
  e->pid = fork();
  if (e->pid == 0) {
    setpgid(0, 0);   // its own group: STOP and timeouts end the CLI with it
    if (chdir(dir)) _exit(127);
    dup2(a[0], 0); dup2(b[1], 1); close(a[1]); close(b[0]);
    char key[600], h[600], model[200];
    snprintf(key, sizeof key, "ANTHROPIC_API_KEY=%s", anthropic_key);
    snprintf(h, sizeof h, "HOME=%s", home);
    snprintf(model, sizeof model, "OA_AGENT_MODEL=%s", env("OA_AGENT_MODEL", "claude-sonnet-5-5"));
    char path[] = "PATH=/usr/local/bin:/usr/bin:/bin", quiet[] = "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1", noup[] = "DISABLE_AUTOUPDATER=1";
    char *envp[] = {key, h, path, model, quiet, noup, NULL};
    const char *argv[] = {env("OA_AGENT_ENGINE", "/usr/local/bin/oa-turn"), NULL};
    execve(argv[0], (char *const *)argv, envp);
    _exit(127);
  }
  close(a[0]); close(b[1]);
  e->in = a[1]; e->out = b[0];
  return e->pid > 0;
}
static void engine_send(engine *e, const char *line) {
  size_t n = strlen(line);
  if (write(e->in, line, n) != (ssize_t)n || write(e->in, "\n", 1) != 1) say("engine: write failed");
}

// One turn's events, sent in batches.
typedef struct { text batch; int n; double sent; bool stop; } events;
static void flush(events *ev, const char *slug, long long turn) {
  text body = {0}, out = {0};
  putf(&body, "{\"turn\":%lld,\"events\":[%s]}", turn, ev->batch.p ? ev->batch.p : "");
  long st = api("POST", slug, "event", body.p, &out);
  if (st == 200 && strstr(out.p, "\"stop\": true")) ev->stop = true;
  ev->batch.n = 0; if (ev->batch.p) ev->batch.p[0] = 0;
  ev->n = 0; ev->sent = now_ms();
  free(body.p); free(out.p);
}
static void add_event(events *ev, const char *slug, long long turn, const char *type, const char *data) {
  if (ev->n) puts_(&ev->batch, ",");
  puts_(&ev->batch, "{\"type\":"); putq(&ev->batch, type); puts_(&ev->batch, ",\"data\":"); puts_(&ev->batch, data && *data ? data : "{}"); puts_(&ev->batch, "}");
  if (++ev->n >= 40) flush(ev, slug, turn);
}

// Runs the engine for an ask in dir; fills the end's fields. Returns its status word.
static void run_engine(const char *slug, const char *turn_json, long long turn, const char *dir, const char *home,
                       text *answer, text *error, double *cost, char status[16]) {
  engine e;
  snprintf(status, 16, "failed");
  if (!engine_start(&e, dir, home)) { puts_(error, "\"the agent could not start\""); return; }
  engine_send(&e, turn_json);
  events ev = {0}; ev.sent = now_ms();
  text line = {0};
  double started = now_ms(), stopped_at = 0;
  bool done = false;
  char buf[65536];
  while (!done) {
    struct pollfd p = {e.out, POLLIN, 0};
    int r = poll(&p, 1, 500);
    if (r > 0) {
      ssize_t k = read(e.out, buf, sizeof buf);
      if (k <= 0) break;
      put(&line, buf, (size_t)k);
      char *nl;
      while ((nl = memchr(line.p, '\n', line.n))) {
        *nl = 0;
        const char *l = line.p;
        char *t = jstr(l, "t", "");
        if (!strcmp(t, "call")) {
          // a project tool: one oa-api op, signed with the runner's key, with the turn added
          char *id = jraw(l, "id", "0"), *tool = jstr(l, "tool", ""), *args = jraw(l, "args", "{}");
          text reply = {0}, out = {0}, b = {0};
          long st = 0;
          const char *op = NULL;
          if (!strcmp(tool, "project_get")) op = "state";
          else if (!strcmp(tool, "project_note") || !strcmp(tool, "project_plan")) {
            op = "note";
            char *tx = jstr(args, "text", ""), *ms = jstr(args, "milestone", "");
            text full = {0};
            if (!strcmp(tool, "project_plan") && *ms) { puts_(&full, ms); puts_(&full, ": "); }
            puts_(&full, tx);
            puts_(&b, "{\"kind\":"); putq(&b, !strcmp(tool, "project_plan") ? "plan" : "note");
            puts_(&b, ",\"by\":\"Project agent\",\"text\":"); putq(&b, full.p ? full.p : "");
            putf(&b, ",\"turn\":%lld}", turn);
            free(tx); free(ms); free(full.p);
          } else if (!strcmp(tool, "milestone_status") || !strcmp(tool, "checklist_status")) {
            op = "status";
            char *mid = jstr(args, "id", ""), *ss = jstr(args, "status", ""), *ev = jstr(args, "evidence", "");
            puts_(&b, "{\"id\":"); putq(&b, mid); puts_(&b, ",\"status\":"); putq(&b, ss);
            if (*ev) { puts_(&b, ",\"note\":"); putq(&b, ev); }
            putf(&b, ",\"by\":\"Project agent\",\"turn\":%lld}", turn);
            free(mid); free(ss); free(ev);
          } else if (!strcmp(tool, "milestone_edit") || !strcmp(tool, "goal_propose") || !strcmp(tool, "document_patch")) {
            op = !strcmp(tool, "document_patch") ? "patch" : tool;
            const char *end = strrchr(args, '}');
            if (*skip_ws(args) == '{' && end) {
              put(&b, args, (size_t)(end - args));
              putf(&b, "%s\"turn\":%lld}", skip_ws(skip_ws(args) + 1) != end ? "," : "", turn);
            }
          }
          if (!op) { puts_(&out, "{\"error\":\"no such tool\"}"); st = 400; }
          else if (!strcmp(op, "state")) st = api("GET", slug, "state", NULL, &out);
          else if (!b.n) { puts_(&out, "{\"error\":\"the arguments are not an object\"}"); st = 400; }
          else st = api("POST", slug, op, b.p, &out);
          putf(&reply, "{\"t\":\"reply\",\"id\":%s,\"status\":%ld,\"body\":%s}", id, st, out.n && (out.p[0] == '{' || out.p[0] == '[') ? out.p : "{}");
          engine_send(&e, reply.p);
          say("turn %lld tool %s %ld", turn, op ? op : "?", st);
          free(reply.p); free(out.p); free(b.p); free(id); free(tool); free(args);
        } else if (!strcmp(t, "event")) {
          char *type = jstr(l, "type", ""), *data = jraw(l, "data", "{}");
          add_event(&ev, slug, turn, type, data);
          free(type); free(data);
        } else if (!strcmp(t, "done")) {
          char *s = jstr(l, "status", "failed");
          snprintf(status, 16, "%s", !strcmp(s, "done") || !strcmp(s, "stopped") ? s : "failed");
          char *a = jraw(l, "answer", "\"\""), *er = jraw(l, "error", "\"\""), *c = jraw(l, "cost_usd", "0");
          puts_(answer, a); puts_(error, er); *cost = atof(c);
          free(s); free(a); free(er); free(c);
          done = true;
        }
        free(t);
        size_t rest = line.n - (size_t)(nl + 1 - line.p);
        memmove(line.p, nl + 1, rest); line.n = rest; line.p[rest] = 0;
        if (done) break;
      }
    } else if (r < 0 && errno != EINTR) break;
    if (!done && now_ms() - ev.sent > 4000) flush(&ev, slug, turn);
    if (ev.stop && !stopped_at) { engine_send(&e, "{\"t\":\"stop\"}"); stopped_at = now_ms(); say("turn %lld: stop", turn); }
    if ((stopped_at && now_ms() - stopped_at > 30000) || now_ms() - started > 15 * 60000) {
      kill(-e.pid, SIGKILL);
      if (!stopped_at && !error->n) puts_(error, "\"the turn took longer than 15 minutes\"");
      snprintf(status, 16, "%s", stopped_at ? "stopped" : "failed");
      break;
    }
  }
  if (ev.n) flush(&ev, slug, turn);
  close(e.in); close(e.out);
  kill(-e.pid, SIGKILL);
  int st; waitpid(e.pid, &st, 0);
  if (!done && !error->n) puts_(error, stopped_at ? "\"stopped\"" : "\"the agent stopped before it finished\"");
  if (ev.stop && strcmp(status, "failed")) snprintf(status, 16, "stopped");
  free(line.p); free(ev.batch.p);
}

// The files the last commit changed, as a JSON list.
static void changed_files(const char *dir, text *files) {
  text out = {0};
  git(dir, &out, "diff", "--name-only", "HEAD~1", "HEAD", NULL, NULL);
  puts_(files, "[");
  int n = 0;
  for (char *l = out.p ? strtok(out.p, "\n") : NULL; l && n < 200; l = strtok(NULL, "\n")) { if (n++) puts_(files, ","); putq(files, l); }
  puts_(files, "]");
  free(out.p);
}
// Why git failed, as one of a few fixed words for the log (never git's own
// text, which can name the repository).
static const char *git_why(const text *out) {
  static const char *known[] = {"Host key verification failed", "Permission denied", "Could not resolve", "Load key", "invalid format",
                                "Connection timed out", "Connection refused", "Repository not found", "not found", "bad permissions", NULL};
  for (int i = 0; known[i]; i++) if (out && out->p && strstr(out->p, known[i])) return known[i];
  // else git's words, with anything that looks like an address or a path left out
  static char said[240];
  said[0] = 0;
  if (!out || !out->p) return "no output";
  char copy[2048]; snprintf(copy, sizeof copy, "%s", out->p);
  size_t n = 0;
  for (char *w = strtok(copy, " \n\t'\""); w && n < sizeof said - 12; w = strtok(NULL, " \n\t'\"")) {
    const char *word = strchr(w, '@') || strstr(w, "://") || strchr(w, '/') ? "<…>" : w;
    n += (size_t)snprintf(said + n, sizeof said - n, "%s%s", n ? " " : "", word);
  }
  return said;
}

// Rebase the turn's commit on main and push it. false: a conflict (the files are in *why).
static bool push(const char *dir, text *why) {
  text out = {0};
  for (int attempt = 0; attempt < 3; attempt++) {
    out.n = 0;
    if (git(dir, &out, "fetch", "--depth", "50", "origin", "main", NULL)) { say("push: git failed: %s", git_why(&out)); puts_(why, "the repository could not be reached"); free(out.p); return false; }
    if (git(dir, &out, "rebase", "FETCH_HEAD", NULL, NULL, NULL, NULL)) {
      text conflict = {0};
      git(dir, &conflict, "diff", "--name-only", "--diff-filter=U", NULL, NULL, NULL);
      git(dir, NULL, "rebase", "--abort", NULL, NULL, NULL, NULL);
      puts_(why, "A member's commit conflicts with this turn's changes, so they were not pushed. Files: ");
      for (char *c = conflict.p; c && *c; c++) if (*c == '\n') *c = ' ';
      puts_(why, conflict.p ? conflict.p : "");
      free(conflict.p); free(out.p);
      return false;
    }
    out.n = 0;
    if (!git(dir, &out, "push", "origin", "HEAD:main", NULL, NULL, NULL)) { free(out.p); return true; }
  }
  puts_(why, "the push was refused");
  free(out.p);
  return false;
}
// The first line of the member's message, short, for the commit's subject.
static void subject(const char *msg, char *out, size_t cap) {
  size_t n = strcspn(msg, "\n");
  if (n > 72) n = 72;
  snprintf(out, cap, "%.*s", (int)n, msg);
}

static void end_turn(const char *slug, long long id, const char *status, const char *answer, const char *error,
                     const char *commit, const char *commit_message, const char *files, double cost) {
  text body = {0}, out = {0};
  putf(&body, "{\"turn\":%lld,\"status\":\"%s\",\"answer\":%s,\"error\":%s,\"cost_usd\":%.4f", id, status,
       answer && *answer ? answer : "\"\"", error && *error ? error : "\"\"", cost);
  if (commit && *commit) { puts_(&body, ",\"commit\":"); putq(&body, commit); puts_(&body, ",\"commit_message\":"); putq(&body, commit_message); }
  putf(&body, ",\"files\":%s}", files && *files ? files : "[]");
  api("POST", slug, "end", body.p, &out);
  say("turn %lld %s cost %.4f%s", id, status, cost, commit && *commit ? " committed" : "");
  free(body.p); free(out.p);
}

// One turn from next's answer.
static void one_turn(const char *slug, const char *next) {
  const char *tj; size_t tn;
  if (!jget(next, "turn", &tj, &tn) || *tj != '{') return;
  char *turn_json = strndup(tj, tn);
  long long id = jint(turn_json, "id");
  char *kind = jstr(turn_json, "kind", "ask"), *msg = jstr(turn_json, "text", ""), *revert = jstr(turn_json, "revert", "");
  char url[1024], keysrc[1024], dir[1200], key[1300], co[1300], subj[200], cmsg[300] = "", sha[64] = "";
  bool has_repo = repo_of(slug, url, sizeof url, keysrc, sizeof keysrc);
  text answer = {0}, error = {0}, files = {0}, out = {0};
  double cost = 0;
  char status[16] = "failed";
  if (!turn_dir(id, has_repo ? keysrc : NULL, dir, sizeof dir, key, sizeof key)) {
    end_turn(slug, id, "failed", "\"\"", "\"the runner could not make a work directory\"", NULL, NULL, NULL, 0);
    goto out;
  }
  git_env(key, dir);
  snprintf(co, sizeof co, "%s/repo", dir);
  if (!strcmp(kind, "undo")) {
    // UNDO TURN: revert the turn's commit, pushed
    if (!has_repo || !*revert) { end_turn(slug, id, "done", "\"\"", "\"\"", NULL, NULL, NULL, 0); goto out; }
    if (git7(dir, &out, "clone", "--depth", "1", "--branch", "main", url, co) ||
        git(co, &out, "fetch", "--depth", "2", "origin", revert, NULL)) {
      say("turn %lld: git failed: %s", id, git_why(&out));
      end_turn(slug, id, "failed", "\"\"", "\"the repository could not be reached\"", NULL, NULL, NULL, 0); goto out;
    }
    snprintf(cmsg, sizeof cmsg, "agent turn %lld: undo turn %lld", id, jint(turn_json, "undo_of"));
    if (git(co, &out, "revert", "--no-commit", revert, NULL, NULL, NULL) || git(co, &out, "commit", "-q", "-m", cmsg, NULL, NULL)) {
      git(co, NULL, "revert", "--abort", NULL, NULL, NULL, NULL);
      end_turn(slug, id, "failed", "\"\"", "\"later commits changed the same lines, so the turn's commit could not be reverted\"", NULL, NULL, NULL, 0);
      goto out;
    }
    text why = {0};
    if (!push(co, &why)) { text e = {0}; putq(&e, why.p); end_turn(slug, id, "failed", "\"\"", e.p, NULL, NULL, NULL, 0); free(e.p); free(why.p); goto out; }
    free(why.p);
    out.n = 0; git(co, &out, "rev-parse", "HEAD", NULL, NULL, NULL, NULL);
    snprintf(sha, sizeof sha, "%.40s", out.p ? out.p : "");
    changed_files(co, &files);
    end_turn(slug, id, "done", "\"\"", "\"\"", sha, cmsg, files.p, 0);
    goto out;
  }
  // an ask: a fresh checkout (or an empty directory when the project links no repository)
  if (has_repo) {
    if (git7(dir, &out, "clone", "--depth", "1", "--branch", "main", url, co)) {
      say("turn %lld: git failed: %s", id, git_why(&out));
      end_turn(slug, id, "failed", "\"\"", "\"the repository could not be reached\"", NULL, NULL, NULL, 0); goto out;
    }
  } else mkdir(co, 0700);
  {
    // the engine's first line: the turn, what next said, and where the checkout is
    text first = {0};
    char home[1300];
    char *history = jraw(next, "history", "[]"), *version = jraw(next, "version", "0"), *spent = jraw(next, "spent", "0"), *cap = jraw(next, "cap", "10");
    putf(&first, "{\"t\":\"turn\",\"slug\":\"%s\",\"turn\":%s,\"history\":%s,\"version\":%s,\"model\":\"%s\",\"budget_usd\":%.4f,\"repo\":%s,\"cwd\":",
         slug, turn_json, history, version, env("OA_AGENT_MODEL", "claude-sonnet-5-5"), atof(cap) - atof(spent), has_repo ? "true" : "false");
    putq(&first, co);
    char today[16]; time_t now = time(NULL); struct tm tm; localtime_r(&now, &tm); strftime(today, sizeof today, "%Y-%m-%d", &tm);
    // the workspace: what this turn offers (docs/project-agent.md, "The runner's seam")
    putf(&first, ",\"today\":\"%s\",\"workspace\":{\"kind\":\"project\",\"budget\":\"project:%s\",\"tools\":[\"project\"],\"write\":true}}", today, slug);
    snprintf(home, sizeof home, "%s/home", dir); mkdir(home, 0700);
    run_engine(slug, first.p, id, co, home, &answer, &error, &cost, status);
    free(first.p); free(history); free(version); free(spent); free(cap);
  }
  if (has_repo && !strcmp(status, "done")) {
    out.n = 0;
    git(co, &out, "status", "--porcelain", NULL, NULL, NULL, NULL);
    if (out.n > 0) {
      subject(msg, subj, sizeof subj);
      snprintf(cmsg, sizeof cmsg, "agent turn %lld: %s", id, subj);
      git(co, NULL, "add", "-A", NULL, NULL, NULL, NULL);
      if (!git(co, &out, "commit", "-q", "-m", cmsg, NULL, NULL)) {
        text why = {0};
        changed_files(co, &files);
        if (push(co, &why)) {
          out.n = 0; git(co, &out, "rev-parse", "HEAD", NULL, NULL, NULL, NULL);
          snprintf(sha, sizeof sha, "%.40s", out.p ? out.p : "");
        } else {
          text e = {0};
          putq(&e, why.p);
          error.n = 0; puts_(&error, e.p);
          if (!strcmp(status, "done")) snprintf(status, 16, "failed");
          free(e.p);
        }
        free(why.p);
      }
    }
  }
  end_turn(slug, id, status, answer.p, error.p, sha, cmsg, files.p ? files.p : "[]", cost);
out:
  remove_dir(dir);
  free(turn_json); free(kind); free(msg); free(revert);
  free(answer.p); free(error.p); free(files.p); free(out.p);
}

// A project's turns until none waits.
static void turns(const char *slug) {
  for (int i = 0; i < 50; i++) {
    text out = {0};
    long st = api("POST", slug, "next", "{}", &out);
    const char *t; size_t n;
    bool any = st == 200 && jget(out.p, "turn", &t, &n) && *t == '{';
    bool refused = st == 200 && strstr(out.p, "\"refused\"");
    if (any) one_turn(slug, out.p);
    free(out.p);
    if (!any && !refused) return;
  }
}

// ---- the wake server: one turn loop per project at a time
#define SLUGS 32
static struct { char slug[72]; pid_t pid; } loops[SLUGS];
static void reap(void) {
  int st; pid_t p;
  while ((p = waitpid(-1, &st, WNOHANG)) > 0) for (int i = 0; i < SLUGS; i++) if (loops[i].pid == p) loops[i].pid = 0;
}
static void wake(const char *slug) {
  reap();
  int free_slot = -1;
  for (int i = 0; i < SLUGS; i++) {
    if (loops[i].pid && !strcmp(loops[i].slug, slug)) return;   // its loop will take the new turn
    if (!loops[i].pid && free_slot < 0) free_slot = i;
  }
  if (free_slot < 0) return;
  pid_t pid = fork();
  if (pid == 0) { turns(slug); _exit(0); }
  if (pid > 0) { snprintf(loops[free_slot].slug, sizeof loops[free_slot].slug, "%s", slug); loops[free_slot].pid = pid; }
}
static void respond(int fd, int status, const char *body) {
  char head[256];
  int n = snprintf(head, sizeof head, "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n",
                   status, status == 202 ? "Accepted" : status == 200 ? "OK" : status == 400 ? "Bad Request" : "Not Found", strlen(body));
  if (write(fd, head, (size_t)n) < 0 || write(fd, body, strlen(body)) < 0) say("write failed");
}
static void serve(int fd) {
  char req[8192]; size_t n = 0;
  struct timeval tv = {5, 0}; setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
  char *end = NULL;
  while (n < sizeof req - 1) {
    ssize_t k = read(fd, req + n, sizeof req - 1 - n);
    if (k <= 0) break;
    n += (size_t)k; req[n] = 0;
    if ((end = strstr(req, "\r\n\r\n"))) {
      const char *cl = strcasestr(req, "Content-Length:");
      size_t want = cl ? strtoul(cl + 15, NULL, 10) : 0;
      if (n - (size_t)(end + 4 - req) >= want) break;
    }
  }
  if (!end) { respond(fd, 400, "{\"error\":\"bad request\"}"); return; }
  if (!strncmp(req, "POST /wake ", 11)) {
    char *slug = jstr(end + 4, "slug", "");
    bool ok = *slug && strlen(slug) <= 64;
    for (const char *c = slug; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '-')) ok = false;
    if (ok) { respond(fd, 202, "{\"ok\":true}"); shutdown(fd, SHUT_WR); wake(slug); say("wake"); free(slug); return; }
    free(slug);
    respond(fd, 400, "{\"error\":\"a slug is needed\"}");
  } else if (!strncmp(req, "GET / ", 6)) respond(fd, 200, "{\"ok\":true}");
  else respond(fd, 404, "{\"error\":\"not found\"}");
}

int main(int argc, char **argv) {
  setenv("TZ", "America/Chicago", 1); tzset();   // "today" in the engine's first line
  const char *listen_at = NULL;
  for (int i = 1; i < argc; i++) if (!strcmp(argv[i], "--listen") && i + 1 < argc) listen_at = argv[++i];
  signal(SIGPIPE, SIG_IGN);
  curl_global_init(CURL_GLOBAL_DEFAULT);
  if (!load_key()) { say("OA_RUNNER_KEY is missing or not a key"); return 2; }
  snprintf(anthropic_key, sizeof anthropic_key, "%s", env("ANTHROPIC_API_KEY", ""));
  unsetenv("ANTHROPIC_API_KEY");   // only the engine gets it
  char host[256] = "0.0.0.0"; int port = atoi(env("PORT", "8080"));
  if (listen_at) { const char *c = strrchr(listen_at, ':'); if (c) { snprintf(host, sizeof host, "%.*s", (int)(c - listen_at), listen_at); port = atoi(c + 1); } }
  int lfd = socket(AF_INET, SOCK_STREAM, 0), one = 1;
  setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0}; a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port); inet_pton(AF_INET, host, &a.sin_addr);
  if (bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 64)) { perror("oa-agent: listen"); return 1; }
  say("listening on %s:%d as %.8s", host, port, pubhex);
  for (;;) {
    int fd = accept(lfd, NULL, NULL);
    if (fd < 0) { if (errno == EINTR) continue; reap(); continue; }
    serve(fd);
    close(fd);
    reap();
  }
}
