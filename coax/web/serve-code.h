/* Split from coax/web/serve.c (#13848): browser editor realm code route (E12). */
// ---- a realm's code from the browser (the editor's E12, #13421)
// POST /coaxial/code, realm.code.v1 (metaverse-engine/editor/code-wire.coax is the one codec and
// the one rule for a path). The editor shell on a page this server serves lists, reads and writes
// the page's own realm sources:
//   - only while watching: --static (a deployed page) and the realm origin never reach here;
//   - only this server's own page, on this computer: the browser must say the request is
//     same-origin (Sec-Fetch-Site, which a page can't set) and the Host must be a loopback name,
//     so no other site and no other machine can write a source;
//   - only a file the page's module was built from (APP_DIR/build/.drive/PAGE.sources, and what
//     its units' dependency lists name), under the realm's own folder, that is a plain file where
//     its name says (no link on the way). No other path is read or written.
// A save writes the file and answers with the compiler's check of the module it is part of (the
// resident compiler: well under a second once it has seen the module). The watch below then
// builds the module again and tells the tabs; a failed build keeps the last good engine running.
#define CSTR(s) ((coax_string){(s), (int64_t)strlen(s)})
typedef struct { char path[200]; long long bytes; } code_file;
static const char *repo_root(void) {
  static char root[PATH_MAX];
  if (!root[0]) { char up[PATH_MAX + 8]; snprintf(up, sizeof up, "%s/../..", host_dir); if (!realpath(up, root)) root[0] = 0; }
  return root;
}
// Whether rel (from the repository's root) is the page's own source by name and a plain file there.
static bool code_own(const char *page, const char *rel, long long *bytes) {
  char full[PATH_MAX], real[PATH_MAX]; struct stat st;
  const char *root = repo_root();
  if (!root[0] || !rc_path_ok(CSTR(page), CSTR(rel))) return false;
  if (snprintf(full, sizeof full, "%s/%s", root, rel) >= (int)sizeof full) return false;
  if (lstat(full, &st) || !S_ISREG(st.st_mode)) return false;
  if (!realpath(full, real) || strcmp(real, full)) return false;   // a link on the way leads elsewhere
  *bytes = (long long)st.st_size;
  return true;
}
static int code_add(code_file *list, int n, int cap, const char *page, const char *rel) {
  long long bytes;
  if (n >= cap || strlen(rel) >= sizeof list[0].path || !code_own(page, rel, &bytes)) return n;
  for (int i = 0; i < n; i++) if (!strcmp(list[i].path, rel)) return n;
  snprintf(list[n].path, sizeof list[n].path, "%s", rel); list[n].bytes = bytes;
  return n + 1;
}
static int code_order(const void *a, const void *b) { return strcmp(((const code_file *)a)->path, ((const code_file *)b)->path); }
// The page's own realm sources: from its module's source list and its units' dependency lists.
static int code_list(const char *page, code_file *list, int cap) {
  char path[PATH_MAX + 128], wasm[64], wgsl[64];
  int n = 0;
  const char *root = repo_root(); size_t r = strlen(root);
  snprintf(path, sizeof path, "%s/build/.drive/%s.sources", app_dir, page);
  FILE *f = fopen(path, "r");
  char *line = NULL; size_t lcap = 0; ssize_t k;
  while (f && (k = getline(&line, &lcap, f)) > 0) {
    if (line[k - 1] == '\n') line[--k] = 0;
    if (k > 66 && line[64] == ' ' && line[65] == ' ') n = code_add(list, n, cap, page, line + 66);   // SHA256, two spaces, the path
  }
  if (f) fclose(f);
  free(line);
  snprintf(wasm, sizeof wasm, "/%s.wasm", page); snprintf(wgsl, sizeof wgsl, "/%s.wgsl", page);
  for (int i = 0; i < unit_count && r; i++) {
    if (!ends_with(units[i].out, wasm) && !ends_with(units[i].out, wgsl)) continue;
    for (int s = 0; s < units[i].nsrc; s++) {
      const char *p = files[units[i].src[s]].path;
      if (!strncmp(p, root, r) && p[r] == '/') n = code_add(list, n, cap, page, p + r + 1);
    }
  }
  qsort(list, (size_t)n, sizeof *list, code_order);
  return n;
}
// Runs argv in dir, its output (both streams) into out: its exit status, or -1 when it did not
// end in ms. The compiler may leave its resident process holding the pipe, so the wait is for the
// child, not for the pipe's end. With no out, nothing is waited for (reap_children takes it).
static void check_changes(bool all);
static int code_run(const char *dir, char *const argv[], char *out, size_t cap, size_t *got, int ms) {
  int fds[2] = {-1, -1};
  if (out && pipe(fds)) return -1;
  pid_t pid = fork();
  if (pid < 0) { if (out) { close(fds[0]); close(fds[1]); } return -1; }
  if (pid == 0) {
    int null = open("/dev/null", O_RDWR);
    dup2(null, 0); dup2(out ? fds[1] : null, 1); dup2(out ? fds[1] : null, 2);
    for (int fd = 3; fd < 1024; fd++) close(fd);
    if (chdir(dir)) _exit(127);
    execv(argv[0], argv);
    _exit(127);
  }
  if (!out) return 0;
  close(fds[1]);
  fcntl(fds[0], F_SETFL, O_NONBLOCK);
  double deadline = monotonic_ms() + ms;
  int status = 0; bool ended = false; size_t n = 0;
  for (;;) {
    struct pollfd p = {fds[0], POLLIN, 0};
    poll(&p, 1, ended ? 0 : 10);
    char tmp[4096]; ssize_t k;
    while ((k = read(fds[0], tmp, sizeof tmp)) > 0) { size_t take = (size_t)k < cap - n ? (size_t)k : cap - n; memcpy(out + n, tmp, take); n += take; }
    if (ended) break;
    if (waitpid(pid, &status, WNOHANG) == pid) { ended = true; continue; }
    if (monotonic_ms() > deadline) { kill(pid, SIGKILL); waitpid(pid, &status, 0); close(fds[0]); *got = n; return -1; }
  }
  close(fds[0]);
  *got = n;
  return WIFEXITED(status) ? WEXITSTATUS(status) : -1;
}
static void code_answer(client *c, int status, int what, const char *name, long long count, const char *rest, size_t rest_n) {
  size_t cap = rest_n + 512; char *out = malloc(cap);
  int64_t n = out ? realm_code_head(what, CSTR(name), count, (coax_v_u8){(uint8_t *)out, (int64_t)cap}) : 0;
  if (n <= 0) { respond(c->fd, 500, "Error", "text/plain; charset=utf-8", "", 0); free(out); drop(c); return; }
  if (rest_n) memcpy(out + n, rest, rest_n);
  respond(c->fd, status, status == 200 ? "OK" : "Refused", "text/plain; charset=utf-8", out, (size_t)n + rest_n);
  free(out);
  drop(c);
}
static void code_refuse(client *c, int status, const char *words) { code_answer(c, status, 4, words, 0, NULL, 0); }
static bool header_is(const char *request, const char *name, const char *want) {
  size_t n = strlen(name), w = strlen(want);
  for (const char *p = request; (p = strstr(p, "\r\n")) && p[2] && p[2] != '\r'; p += 2) {
    if (strncasecmp(p + 2, name, n) || p[2 + n] != ':') continue;
    const char *v = p + 3 + n; while (*v == ' ') v++;
    return !strncmp(v, want, w) && v[w] == '\r';
  }
  return false;
}
static void code_local(client *c) {
  char host[256]; request_host(c->buf, host, sizeof host);
  size_t h = strcspn(host, ":"); if (host[0] == '[') h = strcspn(host, "]") + 1;
  bool loopback = (h == 9 && !strncmp(host, "127.0.0.1", 9)) || (h == 9 && !strncasecmp(host, "localhost", 9)) || (h == 5 && !strncmp(host, "[::1]", 5));
  if (!loopback || !header_is(c->buf, "Sec-Fetch-Site", "same-origin")) { code_refuse(c, 403, "Only this computer's own page may open a realm's code."); return; }
  const char *end = strstr(c->buf, "\r\n\r\n");
  size_t head = end ? (size_t)(end - c->buf) + 4 : c->n, want = 0;
  for (const char *p = c->buf; end && p < end; p++)
    if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Content-Length:", 15)) want = (size_t)strtoul(p + 15, NULL, 10);
  if (!end || c->n != head + want) { code_refuse(c, 413, "That is more than the editor saves at once."); return; }
  coax_t_RcAsk ask = rc_ask_read((coax_string){c->buf + head, (int64_t)want});
  if (!ask.ok) { code_refuse(c, 400, "That file is not one of this realm's."); return; }
  char page[64], rel[200], full[PATH_MAX];
  snprintf(page, sizeof page, "%.*s", (int)ask.page.count, ask.page.data);
  snprintf(rel, sizeof rel, "%.*s", (int)ask.path.count, ask.path.data);
  int cap = (int)realm_code_limit(2);
  code_file *list = calloc((size_t)cap, sizeof *list);
  if (!list) { code_refuse(c, 500, "The dev server is out of memory."); return; }
  int count = code_list(page, list, cap);
  char coax_bin[PATH_MAX + 16], root_rel[256];
  snprintf(coax_bin, sizeof coax_bin, "%s/../bin/coax", host_dir);
  if (ask.op == 0) {
    size_t lcap = (size_t)count * 256 + 1, n = 0; char *lines = malloc(lcap);
    for (int i = 0; lines && i < count; i++) n += (size_t)realm_code_file_line(CSTR(list[i].path), list[i].bytes, (coax_v_u8){(uint8_t *)lines + n, (int64_t)(lcap - n)});
    // the compiler meets the realm's module now, so the first save's check is as quick as the rest
    int64_t rn = count ? realm_code_root(CSTR(page), CSTR(list[0].path), (coax_v_u8){(uint8_t *)root_rel, sizeof root_rel - 1}) : 0;
    for (int i = 0; i < count && rn <= 0; i++) rn = realm_code_root(CSTR(page), CSTR(list[i].path), (coax_v_u8){(uint8_t *)root_rel, sizeof root_rel - 1});
    if (rn > 0) { root_rel[rn] = 0; char *argv[] = {coax_bin, "check", root_rel, NULL}; code_run(repo_root(), argv, NULL, 0, NULL, 0); }
    code_answer(c, 200, 0, page, count, lines ? lines : "", lines ? n : 0);
    free(lines); free(list); return;
  }
  long long bytes = -1;
  for (int i = 0; i < count; i++) if (!strcmp(list[i].path, rel)) bytes = list[i].bytes;
  free(list);
  if (bytes < 0 || !code_own(page, rel, &bytes) || snprintf(full, sizeof full, "%s/%s", repo_root(), rel) >= (int)sizeof full) { code_refuse(c, 404, "That file is not one of this realm's."); return; }
  if (ask.op == 1) {
    if (bytes > realm_code_limit(0)) { code_refuse(c, 413, "That file is longer than the editor opens (192 KB)."); return; }
    char *text = malloc((size_t)bytes + 1); size_t n = 0;
    FILE *f = fopen(full, "rb");
    if (f && text) n = fread(text, 1, (size_t)bytes, f);
    if (f) fclose(f);
    if (!f || !text || n != (size_t)bytes) { free(text); code_refuse(c, 500, "That file could not be read."); return; }
    code_answer(c, 200, 1, rel, bytes, text, n);
    free(text); return;
  }
  int fd = open(full, O_WRONLY | O_TRUNC | O_NOFOLLOW);
  bool wrote = fd >= 0;
  for (size_t at = 0; wrote && at < (size_t)ask.text.count; ) { ssize_t k = write(fd, ask.text.data + at, (size_t)ask.text.count - at); if (k <= 0) wrote = false; else at += (size_t)k; }
  if (fd >= 0 && close(fd)) wrote = false;
  if (!wrote) { code_refuse(c, 500, "That file could not be written."); return; }
  logf_("code: %s saved from the editor (%lld bytes)", rel, (long long)ask.text.count);
  // The watch sees the save now, before the compiler's check below holds this loop (#13464):
  // the first check after a start meets a cold compiler and can take the whole 20 s, and the
  // save was seen only after it (and any request behind it), half a minute late. A whole pass,
  // and a second from the top when the first began mid-pass and started nothing: a running
  // build sees the save when it ends, as before.
  if (!build_pid) check_changes(true);
  if (!build_pid) check_changes(true);
  int64_t rn = realm_code_root(CSTR(page), CSTR(rel), (coax_v_u8){(uint8_t *)root_rel, sizeof root_rel - 1});
  if (rn > 0) {
    root_rel[rn] = 0;
    struct stat st; char at[PATH_MAX + 300];
    snprintf(at, sizeof at, "%s/%s", repo_root(), root_rel);
    char *argv[] = {coax_bin, "check", stat(at, &st) ? rel : root_rel, NULL};
    size_t cap_words = (size_t)realm_code_limit(1), n = 0; char *words = malloc(cap_words + 1);
    int ended = words ? code_run(repo_root(), argv, words, cap_words, &n, 20000) : 0;
    if (ended > 0) {
      // whole lines of the compiler's words, as many as an answer carries
      if (n == cap_words) { while (n > 0 && words[n - 1] != '\n') n--; }
      while (n > 0 && (words[n - 1] == '\n' || words[n - 1] == ' ')) n--;
      if (n == 0) n = (size_t)snprintf(words, cap_words, "The compiler found an error and did not say where.");
      code_answer(c, 200, 3, rel, (long long)n, words, n);
      free(words); return;
    }
    free(words);
  }
  code_answer(c, 200, 2, rel, 0, NULL, 0);
}

// The changed units' own commands, side by side, as one shell script; its status is 1 when
// any failed.
