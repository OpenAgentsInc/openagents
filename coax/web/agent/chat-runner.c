// The runner's general mode, for openagents.com/chat (docs/chat.md), included
// by runner.c: the same wake, turn loop, event batches, STOP and 15-minute
// limit as a project turn, with its own queue (oa-api /api/chat/runner/*),
// workspace and engine (chat.coax):
//
//   wake {"chat": true}   a loop takes /chat's queued turns one at a time (next, event, end)
//   the workspace         a read-only checkout of the public openagents repository's main
//                         (OA_CHAT_REPO) at OA_CHAT_DIR: shallow (depth 1), blobless, and
//                         sparse without big binary assets. deploy.sh's image holds one made at
//                         build time (oa-agent --chat-checkout, which fails when it is too big),
//                         so a turn fetches only what main changed since; never committed to
//   the session           each conversation's CLI session file is kept in OA_CHAT_SESSIONS
//                         (a Cloud Storage mount on Cloud Run) and put back before its next
//                         turn, so the turn resumes it, with that turn's model
//   the credential        the asker's own (an Anthropic API key or a Claude subscription
//                         token), opened here from oa-api's sealed copy, else OpenAgents' key
//                         with /chat's daily cap; the engine's environment gets exactly one of
//                         ANTHROPIC_API_KEY or CLAUDE_CODE_OAUTH_TOKEN, with HOME and
//                         CLAUDE_CONFIG_DIR fresh for the turn
//   POST /credential {credential, owner, kind}   checks one (an API key: a models list; a
//                         subscription token: a one-line Haiku turn) and seals it:
//                         {"ok":true,"kind","last4","sealed"} or {"ok":false,"reason"}
//
// Sealing: AES-256-GCM with OA_CHAT_SEAL_KEY (64 hex, Secret Manager
// oa-chat-credential-key, read by this service only), a random 12-byte nonce,
// and "oa-chat-credential.v1|OWNER|KIND" as associated data, so a sealed copy
// opens only for the person and kind it was made for. base64(nonce | ciphertext | tag).
// A credential is never logged, put in an event or answered back.
#include <dirent.h>
#include <ftw.h>
#include <limits.h>
#include <openssl/rand.h>

static unsigned char seal_key[32];
static bool seal_ready;
static char chat_cred[512];   // the turn's credential, opened; wiped after the turn

static bool seal_load(void) {
  const char *k = getenv("OA_CHAT_SEAL_KEY");
  if (!k || strlen(k) < 64) return false;
  for (int i = 0; i < 32; i++) { unsigned v; if (sscanf(k + 2 * i, "%2x", &v) != 1) return false; seal_key[i] = (unsigned char)v; }
  unsetenv("OA_CHAT_SEAL_KEY");
  return seal_ready = true;
}
// The associated data, "oa-chat-credential.v1|OWNER|KIND" (runner-wire.coax rw_aad); false when it
// does not fit, and then nothing is sealed or opened.
static bool aad_of(const char *owner, const char *kind, char *out, size_t cap) {
  int64_t n = rw_aad_write(cs(owner), cs(kind), (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n > 0;
}

// plain → base64 sealed under the 12-byte nonce; false when sealing isn't set up. runner_seal_check.c
// holds it to the goldens' sealed credential (proto/coder/agent-managed-v1alpha1.json).
static bool seal_with(const char *plain, const char *owner, const char *kind, const unsigned char nonce[12], char *out, size_t cap) {
  if (!seal_ready) return false;
  unsigned char buf[12 + 512 + 16]; size_t n = strlen(plain);
  if (n > 512) return false;
  memcpy(buf, nonce, 12);
  char aad[200]; if (!aad_of(owner, kind, aad, sizeof aad)) return false;
  EVP_CIPHER_CTX *c = EVP_CIPHER_CTX_new(); int len = 0, total = 0; bool ok = c != NULL;
  ok = ok && EVP_EncryptInit_ex(c, EVP_aes_256_gcm(), NULL, seal_key, buf) == 1;
  ok = ok && EVP_EncryptUpdate(c, NULL, &len, (const unsigned char *)aad, (int)strlen(aad)) == 1;
  ok = ok && EVP_EncryptUpdate(c, buf + 12, &len, (const unsigned char *)plain, (int)n) == 1; total = len;
  ok = ok && EVP_EncryptFinal_ex(c, buf + 12 + total, &len) == 1; total += len;
  ok = ok && EVP_CIPHER_CTX_ctrl(c, EVP_CTRL_GCM_GET_TAG, 16, buf + 12 + total) == 1;
  EVP_CIPHER_CTX_free(c);
  size_t raw = 12 + (size_t)total + 16;
  if (!ok || cap < raw * 2 + 8) return false;
  EVP_EncodeBlock((unsigned char *)out, buf, (int)raw);
  OPENSSL_cleanse(buf, sizeof buf);
  return true;
}
// plain → base64 sealed under a random nonce.
static bool seal(const char *plain, const char *owner, const char *kind, char *out, size_t cap) {
  unsigned char nonce[12];
  return RAND_bytes(nonce, 12) == 1 && seal_with(plain, owner, kind, nonce, out, cap);
}
// base64 sealed → plain; false when it doesn't open (another owner or kind, a changed key).
static bool unseal(const char *sealed, const char *owner, const char *kind, char *out, size_t cap) {
  if (!seal_ready) return false;
  size_t n = strlen(sealed);
  if (n < 40 || n > 2000 || n % 4) return false;
  unsigned char raw[1600];
  int k = EVP_DecodeBlock(raw, (const unsigned char *)sealed, (int)n);
  if (k < 0) return false;
  if (n >= 1 && sealed[n - 1] == '=') k--;
  if (n >= 2 && sealed[n - 2] == '=') k--;
  if (k < 12 + 16 + 1 || (size_t)(k - 28) + 1 > cap) return false;
  char aad[200]; if (!aad_of(owner, kind, aad, sizeof aad)) return false;
  EVP_CIPHER_CTX *c = EVP_CIPHER_CTX_new(); int len = 0, total = 0; bool ok = c != NULL;
  ok = ok && EVP_DecryptInit_ex(c, EVP_aes_256_gcm(), NULL, seal_key, raw) == 1;
  ok = ok && EVP_DecryptUpdate(c, NULL, &len, (const unsigned char *)aad, (int)strlen(aad)) == 1;
  ok = ok && EVP_DecryptUpdate(c, (unsigned char *)out, &len, raw + 12, k - 28) == 1; total = len;
  ok = ok && EVP_CIPHER_CTX_ctrl(c, EVP_CTRL_GCM_SET_TAG, 16, raw + k - 16) == 1;
  ok = ok && EVP_DecryptFinal_ex(c, (unsigned char *)out + total, &len) == 1;
  EVP_CIPHER_CTX_free(c);
  OPENSSL_cleanse(raw, sizeof raw);
  if (!ok) { OPENSSL_cleanse(out, cap); return false; }
  out[total + len] = 0;
  return true;
}

// ---- checking a credential
// Runs argv with envp in dir, in its own process group, for at most seconds; its output in *out.
static bool run_timed_out;   // the last run_timed was stopped at its limit
static int run_timed_signal;  // or ended by this signal (9 from the kernel: out of memory)
static int run_timed(const char *dir, const char *const argv[], char *const envp[], text *out, int seconds) {
  run_timed_out = false; run_timed_signal = 0;
  int fds[2]; if (pipe(fds)) return -1;
  pid_t pid = fork();
  if (pid == 0) {
    setpgid(0, 0);
    if (dir && chdir(dir)) _exit(127);
    int devnull = open("/dev/null", O_RDONLY); if (devnull >= 0) dup2(devnull, 0);
    dup2(fds[1], 1); dup2(fds[1], 2); close(fds[0]); close(fds[1]);
    execve(argv[0], (char *const *)argv, envp);
    _exit(127);
  }
  close(fds[1]);
  double until = now_ms() + seconds * 1000.0;
  char buf[4096];
  for (;;) {
    struct pollfd p = {fds[0], POLLIN, 0};
    int r = poll(&p, 1, 500);
    if (r > 0) { ssize_t k = read(fds[0], buf, sizeof buf); if (k <= 0) break; if (out) put(out, buf, (size_t)k); }
    if (now_ms() > until) { run_timed_out = true; kill(-pid, SIGKILL); break; }
  }
  close(fds[0]);
  int st = 0; waitpid(pid, &st, 0); kill(-pid, SIGKILL);
  if (WIFSIGNALED(st) && !run_timed_out) run_timed_signal = WTERMSIG(st);
  return WIFEXITED(st) ? WEXITSTATUS(st) : -1;
}
// An API key: one models list. "" when it works, else why not.
static void check_api_key(const char *key, char *why, size_t cap) {
  char url[400], hk[600];
  snprintf(url, sizeof url, "%s/v1/models?limit=1", env("OA_ANTHROPIC_URL", "https://api.anthropic.com"));
  snprintf(hk, sizeof hk, "x-api-key: %s", key);
  CURL *h = curl_easy_init();
  text out = {0};
  long status = 0;
  struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, hk), "anthropic-version: 2023-06-01");
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &out);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 20L);
  CURLcode rc = curl_easy_perform(h);
  if (rc == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd); curl_easy_cleanup(h);
  OPENSSL_cleanse(hk, sizeof hk);
  why[0] = 0;
  if (status == 200) { free(out.p); return; }
  char *msg = NULL;
  const char *e = out.p ? strstr(out.p, "\"error\":") : NULL;
  if (e) msg = jstr(e + 8, "message", "");
  if (!status) snprintf(why, cap, "Anthropic's API could not be reached");
  else snprintf(why, cap, "%s (HTTP %ld)", msg && *msg ? msg : "refused", status);
  free(msg); free(out.p);
}
// A subscription token: a one-line Haiku turn of the CLI, in a fresh home with only that token.
static void check_oauth_token(const char *token, char *why, size_t cap) {
  char dir[600], home[700], cfg[700];
  snprintf(dir, sizeof dir, "%s/check-%d", env("OA_AGENT_WORK", "/tmp/oa-agent"), (int)getpid());
  mkdir(env("OA_AGENT_WORK", "/tmp/oa-agent"), 0700); mkdir(dir, 0700);
  snprintf(home, sizeof home, "HOME=%s", dir); snprintf(cfg, sizeof cfg, "CLAUDE_CONFIG_DIR=%s/cfg", dir);
  char tok[600]; snprintf(tok, sizeof tok, "CLAUDE_CODE_OAUTH_TOKEN=%s", token);
  char path[] = "PATH=/usr/local/bin:/usr/bin:/bin", noup[] = "DISABLE_AUTOUPDATER=1", quiet[] = "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1", nob[] = "BROWSER=/bin/true";
  char *envp[] = {home, cfg, path, tok, noup, quiet, nob, NULL};
  const char *argv[] = {env("OA_CLAUDE", "/usr/local/bin/claude"), "-p", "Reply with the word ok.", "--model", "claude-haiku-5-5", "--max-turns", "1",
                        "--output-format", "json", "--tools", "", "--no-session-persistence", NULL};
  text out = {0};
  int st = run_timed(dir, argv, envp, &out, 90);
  OPENSSL_cleanse(tok, sizeof tok);
  remove_dir(dir);
  why[0] = 0;
  const char *j = out.p ? strchr(out.p, '{') : NULL;
  char *is_error = j ? jraw(j, "is_error", "true") : strdup("true");
  char *result = j ? jstr(j, "result", "") : strdup("");
  if (st == 0 && !strcmp(is_error, "false")) { free(is_error); free(result); free(out.p); return; }
  if (strstr(result, "Login expired") || strstr(result, "/login")) snprintf(why, cap, "the token expired; make a new one with claude setup-token");
  else if (*result) snprintf(why, cap, "%.300s", result);
  else snprintf(why, cap, st < 0 ? "the check took too long" : "the check turn failed");
  free(is_error); free(result); free(out.p);
}
// POST /credential, in a child: check it, seal it, answer.
static void credential(int fd, const char *body) {
  char *cred = jstr(body, "credential", ""), *owner = jstr(body, "owner", ""), *kind = jstr(body, "kind", "");
  char why[400] = "", sealed[2400];
  text out = {0};
  size_t n = strlen(cred);
  bool shape = n >= 24 && n <= 400 && strlen(owner) == 64 &&
               ((!strcmp(kind, "api") && !strncmp(cred, "sk-ant-", 7) && strncmp(cred, "sk-ant-oat", 10)) || (!strcmp(kind, "oauth") && !strncmp(cred, "sk-ant-oat", 10)));
  for (size_t i = 0; shape && i < n; i++) if (!((cred[i] >= 'a' && cred[i] <= 'z') || (cred[i] >= 'A' && cred[i] <= 'Z') || (cred[i] >= '0' && cred[i] <= '9') || cred[i] == '-' || cred[i] == '_')) shape = false;
  if (!shape) snprintf(why, sizeof why, "that is not an Anthropic API key or a Claude subscription token");
  else if (!seal_ready) snprintf(why, sizeof why, "saving keys is not set up on this server");
  else if (!strcmp(kind, "api")) check_api_key(cred, why, sizeof why);
  else check_oauth_token(cred, why, sizeof why);
  if (!why[0] && !seal(cred, owner, kind, sealed, sizeof sealed)) snprintf(why, sizeof why, "it could not be sealed");
  if (why[0]) { puts_(&out, "{\"ok\":false,\"reason\":"); putq(&out, why); puts_(&out, "}"); }
  else putf(&out, "{\"ok\":true,\"kind\":\"%s\",\"last4\":\"%s\",\"sealed\":\"%s\"}", kind, cred + n - 4, sealed);
  OPENSSL_cleanse(cred, n);
  say("credential %s: %s", kind, why[0] ? "refused" : "sealed");
  respond(fd, 200, out.p);
  free(cred); free(owner); free(kind); free(out.p);
}

// ---- the workspace
// An indexed repo's mirror (OA_REPO_MIRROR, a bare git repository; docs/repos/2026-11-11-indexed-repos.md,
// #13326): with one, the agent reads the repository through the repo tools at an exact commit, and a turn
// has no checkout to make or refresh. Its working directory is then an empty one that stays the same from
// turn to turn, so the CLI finds the conversation's session again.
static const char *chat_mirror(void) { return env("OA_REPO_MIRROR", ""); }
// The repo service (OA_REPO_URL with its grant key, runner.c): the agent reads through it, and no copy is made here.
static bool chat_service(void) { return *env("OA_REPO_URL", "") && repo_key[0]; }
static const char *chat_repo_dir(char *out, size_t cap) {
  if (*chat_mirror() || chat_service()) { snprintf(out, cap, "%s/chat-space", env("OA_AGENT_WORK", "/tmp/oa-agent")); mkdir(env("OA_AGENT_WORK", "/tmp/oa-agent"), 0700); mkdir(out, 0700); return out; }
  const char *d = env("OA_CHAT_DIR", "");
  if (*d) snprintf(out, cap, "%s", d); else snprintf(out, cap, "%s/chat-repo", env("OA_AGENT_WORK", "/tmp/oa-agent"));
  return out;
}
// Why a git step failed, each with its own words for the person.
typedef enum { CHAT_GIT_OK, CHAT_GIT_MEMORY, CHAT_GIT_TIMEOUT, CHAT_GIT_UNREACHABLE, CHAT_GIT_OTHER } chat_git_why;
static const char *chat_git_words(chat_git_why w) {
  return w == CHAT_GIT_MEMORY ? "We ran out of memory getting our copy of the code, so we couldn't answer. Send your message again."
       : w == CHAT_GIT_TIMEOUT ? "Getting our copy of the code took too long, so we stopped. Send your message again."
       : w == CHAT_GIT_UNREACHABLE ? "We couldn't reach GitHub to get our copy of the code. Send your message again in a minute."
       : "We couldn't get our copy of the code. Send your message again.";
}
// One git step, for at most seconds; a failure is logged with the step and git's own words.
static chat_git_why chat_git(const char *dir, const char *step, int seconds, const char *a, const char *b, const char *c, const char *d, const char *e, const char *f, const char *g) {
  const char *argv[10] = {git_bin(), a, b, c, d, e, f, g, NULL};
  text out = {0};
  double t0 = now_ms();
  // git's own timings (trace2), so a slow step's log line says where the time went
  char trace[700], trace_env[720], *envp[18]; int n = 0;
  snprintf(trace, sizeof trace, "%s/chat-home/git-trace-%d.txt", env("OA_AGENT_WORK", "/tmp/oa-agent"), (int)getpid());
  snprintf(trace_env, sizeof trace_env, "GIT_TRACE2_PERF=%s", trace);
  unlink(trace);
  while (git_envp[n] && n < 16) { envp[n] = git_envp[n]; n++; }
  envp[n++] = trace_env; envp[n] = NULL;
  int st = run_timed(dir, argv, envp, &out, seconds);
  double took = (now_ms() - t0) / 1000;
  if (took > atof(env("OA_CHAT_GIT_SLOW", "3"))) {
    text where = {0};
    FILE *tf = fopen(trace, "r");
    char line[2000];
    while (tf && fgets(line, sizeof line, tf)) {
      // time | depth | thread | event | repo | since start | in region | category | label
      char *col[10]; int k = 0;
      for (char *c0 = line; k < 10; k++) { col[k] = c0; char *bar = strchr(c0, '|'); if (!bar) break; *bar = 0; c0 = bar + 1; }
      if (k < 8 || !(strstr(col[3], "region_leave") || strstr(col[3], "child_exit")) || atof(col[6]) < 0.5) continue;
      size_t ln = strcspn(col[8], "\n");
      putf(&where, "%s%.1f s %.*s", where.n ? "; " : "", atof(col[6]), (int)(ln > 90 ? 90 : ln), col[8]);
    }
    if (tf) fclose(tf);
    say("chat checkout: git %s took %.1f s: %.1500s", step, took, where.p ? where.p : "git recorded no part over 0.5 s");
    free(where.p);
  }
  unlink(trace);
  if (st == 0) { free(out.p); return CHAT_GIT_OK; }
  const char *o = out.p ? out.p : "";
  chat_git_why w = CHAT_GIT_OTHER;
  if (run_timed_out) w = CHAT_GIT_TIMEOUT;
  else if (run_timed_signal == SIGKILL || strstr(o, "ut of memory") || strstr(o, "Cannot allocate memory") || strstr(o, "No space left")) w = CHAT_GIT_MEMORY;
  else if (strstr(o, "Could not resolve host") || strstr(o, "unable to access") || strstr(o, "Failed to connect") || strstr(o, "Could not read from remote") ||
           strstr(o, "Connection timed out") || strstr(o, "Connection refused") || strstr(o, "early EOF") || strstr(o, "remote end hung up")) w = CHAT_GIT_UNREACHABLE;
  char how[60];
  if (run_timed_out) snprintf(how, sizeof how, "stopped at its %d s limit", seconds);
  else if (run_timed_signal) snprintf(how, sizeof how, "ended by signal %d", run_timed_signal);
  else snprintf(how, sizeof how, "exit %d", st);
  say("chat checkout: git %s failed after %.1f s (%s, %s): %.1500s", step, (now_ms() - t0) / 1000, how,
      w == CHAT_GIT_MEMORY ? "out of memory" : w == CHAT_GIT_TIMEOUT ? "too slow" : w == CHAT_GIT_UNREACHABLE ? "the host could not be reached" : "another error", o);
  free(out.p);
  return w;
}
static int chat_secs(const char *name, int otherwise) { int v = atoi(env(name, "")); return v > 0 ? v : otherwise; }
// The checkout at main. With one already there (the image's, or an earlier turn's) only what main
// changed since is fetched; if that fetch fails the turn reads the copy it has, and the log says so.
static chat_git_why chat_checkout(const char *dir) {
  char home[600], git_dir[700], sparse[800];
  snprintf(home, sizeof home, "%s/chat-home", env("OA_AGENT_WORK", "/tmp/oa-agent"));
  mkdir(env("OA_AGENT_WORK", "/tmp/oa-agent"), 0700); mkdir(home, 0700);
  git_env(NULL, home);
  snprintf(git_dir, sizeof git_dir, "%s/.git", dir);
  struct stat sb;
  bool have = !stat(git_dir, &sb);
  double t0 = now_ms();
  if (!have) {
    remove_dir(dir);
    // checkStat and trustctime: the image's copy keeps each file's size and time but not its inode
    // or change time, and git must not read every file again to find nothing changed
    if (git7(NULL, NULL, "init", "-q", "-b", "main", dir, NULL, NULL) ||
        git(dir, NULL, "remote", "add", "origin", env("OA_CHAT_REPO", "https://github.com/OpenAgentsInc/openagents.git"), NULL, NULL) ||
        git(dir, NULL, "config", "core.sparseCheckout", "true", NULL, NULL, NULL) ||
        git(dir, NULL, "config", "core.checkStat", "minimal", NULL, NULL, NULL) ||
        git(dir, NULL, "config", "core.trustctime", "false", NULL, NULL, NULL) ||
        git(dir, NULL, "config", "gc.auto", "0", NULL, NULL, NULL)) { say("chat checkout: git init failed in %s", dir); return CHAT_GIT_OTHER; }
    // everything but big binary assets (non-cone sparse patterns)
    snprintf(sparse, sizeof sparse, "%s/.git/info/sparse-checkout", dir);
    FILE *f = fopen(sparse, "w");
    if (!f) return CHAT_GIT_OTHER;
    fputs("/*\n!*.png\n!*.jpg\n!*.jpeg\n!*.gif\n!*.webp\n!*.bin\n!*.vertices\n!*.indices\n!*.wasm\n!*.mp4\n!*.mov\n!*.glb\n!*.ktx2\n!*.zip\n", f);
    // and packed or raw data (the realms' gzipped packs, model fixtures, sound): 1.7 GB of checkout
    // was 1.2 GB of these on 2026-11-11
    fputs("!*.gz\n!*.pack\n!*.f32\n!*.xz\n!*.wav\n!*.pcm\n!*.ogg\n!*.mp3\n!*.ttf\n!*.otf\n!*.safetensors\n!*.npy\n!*.tar\n!/docs/verification/data/\n", f);
    fclose(f);
  }
  if (have) {
    // The image's index must read as newer than every file it lists. When it doesn't, git takes
    // each file as maybe changed in the index's last second and reads all of them again before it
    // writes the index: 45 s of a 47 s refresh on Cloud Run (2026-11-11). Nothing but git writes here.
    char index[800]; snprintf(index, sizeof index, "%s/index", git_dir);
    utimes(index, NULL);
  }
  chat_git_why w = chat_git(dir, "fetch", chat_secs("OA_CHAT_FETCH_SECONDS", 60), "fetch", "-q", "--depth", "1", "--filter=blob:none", "origin", "main");
  if (w != CHAT_GIT_OK) {
    if (have && !git(dir, NULL, "rev-parse", "-q", "--verify", "HEAD", NULL, NULL)) { say("chat checkout: main could not be fetched; the turn reads the copy already here"); return CHAT_GIT_OK; }
    return w;
  }
  double t1 = now_ms();
  const char *how = "made whole";
  if (have) {
    // already at main: nothing to do
    text at = {0};
    bool same = false;
    if (!git(dir, &at, "rev-parse", "HEAD", "FETCH_HEAD", NULL, NULL, NULL) && at.p) {
      const char *nl = strchr(at.p, '\n');
      same = nl && nl - at.p == 40 && !strncmp(at.p, nl + 1, 40);
    }
    free(at.p);
    if (same) { say("chat checkout: already at main (fetch %.1f s)", (t1 - t0) / 1000); return CHAT_GIT_OK; }
    // a refresh writes only the files main changed, and looks at no others (reset --hard and clean
    // read every file's entry, 12 s on a cold instance); nothing but this writes in the checkout
    how = "refreshed";
    w = chat_git(dir, "read-tree", chat_secs("OA_CHAT_RESET_SECONDS", 120), "read-tree", "-m", "-u", "HEAD", "FETCH_HEAD", NULL, NULL);
    if (w == CHAT_GIT_OK) w = chat_git(dir, "update-ref", 30, "update-ref", "HEAD", "FETCH_HEAD", NULL, NULL, NULL, NULL);
    if (w != CHAT_GIT_OK) { have = false; how = "put back whole"; }
  }
  if (!have) {
    // a whole checkout fetches every file's contents here
    w = chat_git(dir, "reset", chat_secs("OA_CHAT_RESET_SECONDS", 600), "reset", "-q", "--hard", "FETCH_HEAD", NULL, NULL, NULL);
    if (w == CHAT_GIT_OK) w = chat_git(dir, "clean", 120, "clean", "-qfdx", NULL, NULL, NULL, NULL, NULL);
  }
  if (w == CHAT_GIT_OK) say("chat checkout: %s in %.1f s (fetch %.1f s, files %.1f s)", how, (now_ms() - t0) / 1000, (t1 - t0) / 1000, (now_ms() - t1) / 1000);
  return w;
}
// oa-agent --chat-checkout: makes the checkout at OA_CHAT_DIR and measures it, for the image
// (Dockerfile). Fails when it is bigger than OA_CHAT_REPO_MAX_MB (1200): a runner without the
// image's copy holds a whole checkout in memory beside its turns, in 4 GiB.
static long long bake_bytes, bake_files;
static int bake_add(const char *path, const struct stat *sb, int flag, struct FTW *f) {
  (void)path; (void)f;
  bake_bytes += (long long)sb->st_blocks * 512; if (flag == FTW_F) bake_files++;
  return 0;
}
static int chat_bake(void) {
  char repo[600]; chat_repo_dir(repo, sizeof repo);
  chat_git_why w = chat_checkout(repo);
  if (w != CHAT_GIT_OK) { say("chat checkout: %s", chat_git_words(w)); return 1; }
  nftw(repo, bake_add, 64, FTW_PHYS);
  long long mb = bake_bytes / (1024 * 1024), cap = chat_secs("OA_CHAT_REPO_MAX_MB", 1200);
  say("chat checkout: %lld MB in %lld files at %s; the limit is %lld MB", mb, bake_files, repo, cap);
  if (mb > cap) { say("chat checkout: too big. Leave more out of it (the sparse list in chat-runner.c) before the runner runs out of memory"); return 1; }
  return 0;
}

// ---- the session file
static bool copy_file(const char *from, const char *to) {
  FILE *in = fopen(from, "rb");
  if (!in) return false;
  char tmp[1400]; snprintf(tmp, sizeof tmp, "%s.part", to);
  int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (fd < 0) { fclose(in); return false; }
  char buf[65536]; size_t k; bool ok = true;
  while ((k = fread(buf, 1, sizeof buf, in)) > 0) if (write(fd, buf, k) != (ssize_t)k) { ok = false; break; }
  fclose(in); close(fd);
  if (ok && rename(tmp, to)) ok = false;
  if (!ok) unlink(tmp);
  return ok;
}
static bool uuid_ok(const char *s) {
  if (strlen(s) != 36) return false;
  for (int i = 0; i < 36; i++) {
    bool dash = i == 8 || i == 13 || i == 18 || i == 23;
    if (dash ? s[i] != '-' : !((s[i] >= '0' && s[i] <= '9') || (s[i] >= 'a' && s[i] <= 'f'))) return false;
  }
  return true;
}
// Where the CLI keeps a session for cwd: CLAUDE_CONFIG_DIR/projects/<cwd, every other character '-'>/.
static void session_dir(const char *cfg, const char *cwd, char *out, size_t cap) {
  char real[PATH_MAX]; if (!realpath(cwd, real)) snprintf(real, sizeof real, "%s", cwd);
  int n = snprintf(out, cap, "%s/projects/", cfg);
  for (const char *c = real; *c && (size_t)n + 2 < cap; c++)
    out[n++] = ((*c >= 'a' && *c <= 'z') || (*c >= 'A' && *c <= 'Z') || (*c >= '0' && *c <= '9')) ? *c : '-';
  out[n] = 0;
}
// The session's file after a turn, wherever under cfg/projects/ the CLI put it.
static bool session_found(const char *cfg, const char *sid, char *out, size_t cap) {
  char root[800]; snprintf(root, sizeof root, "%s/projects", cfg);
  DIR *d = opendir(root);
  if (!d) return false;
  struct dirent *e; bool found = false;
  while (!found && (e = readdir(d))) {
    if (e->d_name[0] == '.') continue;
    snprintf(out, cap, "%s/%s/%s.jsonl", root, e->d_name, sid);
    struct stat sb; found = !stat(out, &sb);
  }
  closedir(d);
  return found;
}

static void chat_end(long long id, const char *status, const char *answer, const char *error, double cost, bool resumed) {
  text body = {0}, out = {0};
  WIRE(&body, rw_chat_end_write(id, cs(status), cs(answer), cs(error), cost, resumed, chat_expired, room));
  api("POST", "runner", "end", body.p, &out);
  say("chat turn %lld %s cost %.4f%s", id, status, cost, resumed ? " resumed" : "");
  free(body.p); free(out.p);
}

// A Gemini model: the turn runs Microcoder with the repo tool host (chat-micro.coax), which needs the repo service.
static bool chat_gemini(const char *m) {
  return !strcmp(m, "google/gemini-4-hydrogen") || !strcmp(m, "google/gemini-4-argon") || !strcmp(m, "google/gemini-4-carbon");
}
// A turn asked of Gemini 3.8 Flash before it was replaced (#13479) runs on Gemini 4 Hydrogen.
static const char *chat_model(const char *m) {
  if (!strcmp(m, "google/gemini-3.8-flash")) return "google/gemini-4-hydrogen";
  return !strcmp(m, "claude-haiku-5-5") || !strcmp(m, "claude-sonnet-5-5") || chat_gemini(m) ? m : "claude-opus-5-5";
}

// A turn's credential from next's answer (oa-api's "credential": {kind, owner,
// sealed}, the asker's own): opened into chat_cred for the engine's child
// alone, as ANTHROPIC_API_KEY or CLAUDE_CODE_OAUTH_TOKEN, never both. None:
// ours. False when it can't be opened. /chat and project turns both use it.
static bool turn_credential(const char *next) {
  const char *cj; size_t cn;
  engine_cred_name = "ANTHROPIC_API_KEY"; engine_cred = anthropic_key;
  if (!jget(next, "credential", &cj, &cn) || *cj != '{') return true;
  char *c = strndup(cj, cn), *kind = jstr(c, "kind", ""), *owner = jstr(c, "owner", ""), *sealed = jstr(c, "sealed", "");
  bool ok = unseal(sealed, owner, kind, chat_cred, sizeof chat_cred);
  OPENSSL_cleanse(c, cn); free(c); free(owner); free(sealed);
  if (ok) { engine_cred_name = !strcmp(kind, "oauth") ? "CLAUDE_CODE_OAUTH_TOKEN" : "ANTHROPIC_API_KEY"; engine_cred = chat_cred; }
  free(kind);
  return ok;
}
static void turn_credential_done(void) {
  OPENSSL_cleanse(chat_cred, sizeof chat_cred);
  engine_cred_name = "ANTHROPIC_API_KEY"; engine_cred = anthropic_key;
}

// One /chat turn from next's answer.
static void chat_turn(const char *next) {
  const char *tj; size_t tn;
  if (!jget(next, "turn", &tj, &tn) || *tj != '{') return;
  char *turn_json = strndup(tj, tn);
  long long id = jint(turn_json, "id");
  char *model = jstr(turn_json, "model", ""), *sid = jstr(turn_json, "session", "");
  char repo[600], dir[700], home[800], cfg[800], sdir[1400], kept[1400], found[1400], store[600];
  text answer = {0}, error = {0};
  double cost = 0; char status[16] = "failed";
  bool resumed = false;
  chat_expired = false;
  chat_repo_dir(repo, sizeof repo);
  snprintf(store, sizeof store, "%s", env("OA_CHAT_SESSIONS", ""));
  if (!store[0]) snprintf(store, sizeof store, "%s/sessions", env("OA_AGENT_WORK", "/tmp/oa-agent"));
  mkdir(store, 0700);
  snprintf(dir, sizeof dir, "%s/chat-%lld-%d", env("OA_AGENT_WORK", "/tmp/oa-agent"), id, (int)getpid());
  snprintf(home, sizeof home, "%s/home", dir); snprintf(cfg, sizeof cfg, "%s/cfg", dir);
  {
    // A devbox conversation (#13474, docs/cloud/boat-devbox.md): the turn runs on that boat dev box, through
    // the devbox engine, which holds the boat key alone (OA_BOAT_API_KEY, a scoped key from Secret Manager)
    // and no Claude credential: the box has its own. No checkout here, no session kept here.
    char *box = jstr(turn_json, "devbox", "");
    if (*box) {
      free(box);
      if (!boat_key[0]) { chat_end(id, "failed", "\"\"", "\"This runner has no boat key yet, so it can't reach dev boxes.\"", 0, false); goto out; }
      if (mkdir(dir, 0700) || mkdir(home, 0700)) { chat_end(id, "failed", "\"\"", "\"the runner could not make a work directory\"", 0, false); goto out; }
      text first = {0};
      char *history = jraw(next, "history", "[]");
      char today[16]; time_t now = time(NULL); struct tm tm; localtime_r(&now, &tm); strftime(today, sizeof today, "%Y-%m-%d", &tm);
      WIRE(&first, rw_chat_turn_write(cs(turn_json), cs(chat_model(model)), 0, false, cs(sid), cs(history), cs(""), cs(""), false, cs(today), room));
      engine_bin = env("OA_CHAT_DEVBOX_ENGINE", "/usr/local/bin/oa-chat-devbox");
      engine_cred_name = "BOAT_API_KEY"; engine_cred = boat_key;
      run_engine("runner", first.p, id, dir, home, &answer, &error, &cost, status);
      engine_bin = NULL; engine_cred_name = "ANTHROPIC_API_KEY"; engine_cred = anthropic_key;
      chat_end(id, status, answer.p, error.p, cost, false);
      free(first.p); free(history);
      goto out;
    }
    free(box);
  }
  if (!uuid_ok(sid)) { chat_end(id, "failed", "\"\"", "\"the conversation has no session\"", 0, false); goto out; }
  if (chat_service()) {
    // this turn's read grant on the repository, an hour at most (coax/web/repo/grant.coax)
    char session[64]; snprintf(session, sizeof session, "chat-%lld", id);
    int64_t gn = runner_coax_repo_grant((coax_string){repo_key, 64}, (coax_string){"openagents", 10}, (int64_t)time(NULL) + 3000,
                                        (coax_string){session, (int64_t)strlen(session)}, (coax_v_u8){(uint8_t *)repo_grant, sizeof repo_grant - 1});
    repo_grant[gn > 0 ? gn : 0] = 0;
    if (gn <= 0) { chat_end(id, "failed", "\"\"", "\"the runner could not make a grant to read the code\"", 0, false); goto out; }
  } else if (!*chat_mirror()) {
    // the page's status line while the copy refreshes
    text eb = {0}, eo = {0};
    WIRE(&eb, rw_working_batch_write(id, cs("getting the latest code"), room));
    api("POST", "runner", "event", eb.p, &eo);
    free(eb.p); free(eo.p);
    chat_git_why w = chat_checkout(repo);
    if (w != CHAT_GIT_OK) {
      text e = {0}; quote(&e, chat_git_words(w));
      chat_end(id, "failed", "\"\"", e.p, 0, false);
      free(e.p); goto out;
    }
  }
  if (mkdir(dir, 0700) || mkdir(home, 0700) || mkdir(cfg, 0700)) { chat_end(id, "failed", "\"\"", "\"the runner could not make a work directory\"", 0, false); goto out; }
  // the credential: the asker's own, else ours
  bool gemini = chat_gemini(chat_model(model));
  if (gemini && !chat_service()) { chat_end(id, "failed", "\"\"", "\"The Gemini agent reads the code through the repository service, which isn't connected yet. Pick a Claude model.\"", 0, false); goto out; }
  if (!gemini && !turn_credential(next)) { chat_end(id, "failed", "\"\"", "\"Your saved key could not be opened. Save it again under KEY.\"", 0, false); goto out; }
  // the session: put back where the CLI looks for it
  snprintf(kept, sizeof kept, "%s/%s.jsonl", store, sid);
  session_dir(cfg, repo, sdir, sizeof sdir);
  {
    struct stat sb;
    if (!stat(kept, &sb)) {
      char projects[900]; snprintf(projects, sizeof projects, "%s/projects", cfg); mkdir(projects, 0700); mkdir(sdir, 0700);
      snprintf(found, sizeof found, "%s/%s.jsonl", sdir, sid);
      resumed = copy_file(kept, found);
    }
  }
  {
    text first = {0};
    char *history = jraw(next, "history", "[]"), *spent = jraw(next, "spent", "0"), *cap = jraw(next, "cap", "0");
    double budget = atof(cap) > 0 ? atof(cap) - atof(spent) : 0;
    char today[16]; time_t now = time(NULL); struct tm tm; localtime_r(&now, &tm); strftime(today, sizeof today, "%Y-%m-%d", &tm);
    WIRE(&first, rw_chat_turn_write(cs(turn_json), cs(chat_model(model)), budget > 0 ? budget : (atof(cap) > 0 ? 0.0001 : 0), resumed, cs(sid),
                                    cs(resumed ? "[]" : history), cs(repo), cs(chat_service() ? "" : chat_mirror()), chat_service(), cs(today), room));
    engine_bin = gemini ? env("OA_CHAT_MICRO_ENGINE", "/usr/local/bin/oa-chat-micro") : env("OA_CHAT_ENGINE", "/usr/local/bin/oa-chat-turn");
    snprintf(engine_cfg, sizeof engine_cfg, "%s", cfg);
    run_engine("runner", first.p, id, repo, home, &answer, &error, &cost, status);
    engine_cfg[0] = 0; engine_bin = NULL;
    free(first.p); free(history); free(spent); free(cap);
  }
  // keep the session for the next turn
  if (session_found(cfg, sid, found, sizeof found) && !copy_file(found, kept)) say("chat turn %lld: the session was not kept", id);
  chat_end(id, status, answer.p, error.p, cost, resumed);
out:
  turn_credential_done();
  OPENSSL_cleanse(repo_grant, sizeof repo_grant);
  remove_dir(dir);
  free(turn_json); free(model); free(sid); free(answer.p); free(error.p);
}

// /chat's turns until none waits.
static void chat_turns(void) {
  api_base = "/api/chat/";
  for (int i = 0; i < 50; i++) {
    text out = {0};
    long st = api("POST", "runner", "next", "{}", &out);
    const char *t; size_t n;
    bool any = st == 200 && jget(out.p, "turn", &t, &n) && *t == '{';
    bool refused = st == 200 && strstr(out.p, "\"refused\"");
    if (any) chat_turn(out.p);
    free(out.p);
    if (!any && !refused) return;
  }
}
