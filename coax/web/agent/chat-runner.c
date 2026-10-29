// The runner's general mode, for openagents.com/chat (docs/chat.md), included
// by runner.c: the same wake, turn loop, event batches, STOP and 15-minute
// limit as a project turn, with its own queue (oa-api /api/chat/runner/*),
// workspace and engine (chat.coax):
//
//   wake {"chat": true}   a loop takes /chat's queued turns one at a time (next, event, end)
//   the workspace         a read-only checkout of the public openagents repository's main
//                         (OA_CHAT_REPO), refreshed at the start of every turn: shallow
//                         (depth 1), blobless, and sparse without big binary assets, kept
//                         at OA_AGENT_WORK/chat-repo between turns; never committed to
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
static void aad_of(const char *owner, const char *kind, char *out, size_t cap) { snprintf(out, cap, "oa-chat-credential.v1|%s|%s", owner, kind); }

// plain → base64 sealed; false when sealing isn't set up.
static bool seal(const char *plain, const char *owner, const char *kind, char *out, size_t cap) {
  if (!seal_ready) return false;
  unsigned char buf[12 + 512 + 16]; size_t n = strlen(plain);
  if (n > 512 || RAND_bytes(buf, 12) != 1) return false;
  char aad[200]; aad_of(owner, kind, aad, sizeof aad);
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
  char aad[200]; aad_of(owner, kind, aad, sizeof aad);
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
static int run_timed(const char *dir, const char *const argv[], char *const envp[], text *out, int seconds) {
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
    if (now_ms() > until) { kill(-pid, SIGKILL); break; }
  }
  close(fds[0]);
  int st = 0; waitpid(pid, &st, 0); kill(-pid, SIGKILL);
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
               ((!strcmp(kind, "api") && !strncmp(cred, "sk-ant-api", 10)) || (!strcmp(kind, "oauth") && !strncmp(cred, "sk-ant-oat", 10)));
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
static const char *chat_repo_dir(char *out, size_t cap) { snprintf(out, cap, "%s/chat-repo", env("OA_AGENT_WORK", "/tmp/oa-agent")); return out; }
// The checkout, fresh from main. false: the repository could not be reached.
static bool chat_checkout(const char *dir) {
  char home[600], git_dir[700], sparse[800];
  snprintf(home, sizeof home, "%s/chat-home", env("OA_AGENT_WORK", "/tmp/oa-agent"));
  mkdir(env("OA_AGENT_WORK", "/tmp/oa-agent"), 0700); mkdir(home, 0700);
  git_env(NULL, home);
  snprintf(git_dir, sizeof git_dir, "%s/.git", dir);
  struct stat sb;
  if (stat(git_dir, &sb)) {
    remove_dir(dir);
    if (git7(NULL, NULL, "init", "-q", "-b", "main", dir, NULL, NULL) ||
        git(dir, NULL, "remote", "add", "origin", env("OA_CHAT_REPO", "https://github.com/OpenAgentsInc/openagents.git"), NULL, NULL) ||
        git(dir, NULL, "config", "core.sparseCheckout", "true", NULL, NULL, NULL)) return false;
    // everything but big binary assets (non-cone sparse patterns)
    snprintf(sparse, sizeof sparse, "%s/.git/info/sparse-checkout", dir);
    FILE *f = fopen(sparse, "w");
    if (!f) return false;
    fputs("/*\n!*.png\n!*.jpg\n!*.jpeg\n!*.gif\n!*.webp\n!*.bin\n!*.vertices\n!*.indices\n!*.wasm\n!*.mp4\n!*.mov\n!*.glb\n!*.ktx2\n!*.zip\n", f);
    fclose(f);
  }
  text out = {0};
  bool ok = !git7(dir, &out, "fetch", "-q", "--depth", "1", "--filter=blob:none", "origin", "main") &&
            !git(dir, &out, "reset", "-q", "--hard", "FETCH_HEAD", NULL, NULL) &&
            !git(dir, &out, "clean", "-qfdx", NULL, NULL, NULL, NULL);
  free(out.p);
  return ok;
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
  putf(&body, "{\"turn\":%lld,\"status\":\"%s\",\"answer\":%s,\"error\":%s,\"cost_usd\":%.4f,\"resumed\":%s,\"credential_expired\":%s}", id, status,
       answer && *answer ? answer : "\"\"", error && *error ? error : "\"\"", cost, resumed ? "true" : "false", chat_expired ? "true" : "false");
  api("POST", "runner", "end", body.p, &out);
  say("chat turn %lld %s cost %.4f%s", id, status, cost, resumed ? " resumed" : "");
  free(body.p); free(out.p);
}

static const char *chat_model(const char *m) {
  return !strcmp(m, "claude-haiku-5-5") || !strcmp(m, "claude-sonnet-5-5") ? m : "claude-opus-5-5";
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
  if (!uuid_ok(sid)) { chat_end(id, "failed", "\"\"", "\"the conversation has no session\"", 0, false); goto out; }
  if (!chat_checkout(repo)) { chat_end(id, "failed", "\"\"", "\"the repository could not be reached\"", 0, false); goto out; }
  if (mkdir(dir, 0700) || mkdir(home, 0700) || mkdir(cfg, 0700)) { chat_end(id, "failed", "\"\"", "\"the runner could not make a work directory\"", 0, false); goto out; }
  // the credential: the asker's own, else ours
  if (!turn_credential(next)) { chat_end(id, "failed", "\"\"", "\"Your saved key could not be opened. Save it again under KEY.\"", 0, false); goto out; }
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
    putf(&first, "{\"t\":\"turn\",\"mode\":\"chat\",\"turn\":%s,\"model\":\"%s\",\"budget_usd\":%.4f,\"%s\":\"%s\",\"history\":%s,\"cwd\":",
         turn_json, chat_model(model), budget > 0 ? budget : (atof(cap) > 0 ? 0.0001 : 0), resumed ? "resume" : "session_id", sid, resumed ? "[]" : history);
    putq(&first, repo);
    char today[16]; time_t now = time(NULL); struct tm tm; localtime_r(&now, &tm); strftime(today, sizeof today, "%Y-%m-%d", &tm);
    putf(&first, ",\"today\":\"%s\",\"workspace\":{\"kind\":\"chat\",\"budget\":\"chat\",\"tools\":[\"web\"],\"write\":false}}", today);
    engine_bin = env("OA_CHAT_ENGINE", "/usr/local/bin/oa-chat-turn");
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
