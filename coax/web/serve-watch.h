/* Split from coax/web/serve.c (#13848): file watcher, asset polling, and incremental build runner. */
// ---- watching
// Files: one row a path, in a table that grows (no cap), found by a hash of the
// path. A row is watched for what put it there: SCAN (a page, shader or source
// file found in the app's folders) or SRC (a unit's dependency list names it).
// NEAR: a source file in a folder a unit's list reads from, that no list names (#13441): a
// new one there is seen at once, and builds again the units whose last build failed.
enum { ROLE_SCAN = 1, ROLE_SRC = 2, ROLE_NEAR = 4 };
typedef struct { char *path; struct timespec mtime; int roles; } watched;
// A unit: one module or shader the build writes, from its .live/NAME.deps.
// started: when its last build began to read its sources (.live/NAME.started; the output's time without one).
typedef struct { bool wgsl; char *out, *cmd; int *src; int nsrc, src_cap; struct timespec out_mtime, started; bool dirty, told; } unit;
// A file a tab loaded (a pack, a texture), by its URL; hash is its SHA-256 when known.
typedef struct { char *url, *path; struct timespec mtime; char hash[65]; } loaded;
static watched *files; static int file_count, file_cap;
static int *slots; static int slot_cap;   // indexes into files, -1 empty; a power of two
static unit *units; static int unit_count, unit_cap;
static loaded *assets; static int asset_count, asset_cap;
static char **notes; static int note_count, note_cap;   // .wgsl paths changed, told once each
static char **near_dirs; static int near_count, near_cap;   // folders outside the app that a unit's list reads from
static char **failed_outs; static int failed_count, failed_cap;   // the units whose last build failed, by output
static struct timespec build_wall;   // the running build's start, on the files' clock
static bool later(struct timespec a, struct timespec b) { return a.tv_sec > b.tv_sec || (a.tv_sec == b.tv_sec && a.tv_nsec > b.tv_nsec); }

static void *grow(void *p, int *cap, int need, size_t size) {
  if (need <= *cap) return p;
  int next = *cap ? *cap : 64;
  while (next < need) next *= 2;
  void *q = realloc(p, (size_t)next * size);
  if (!q) { logf_("out of memory watching files"); exit(1); }
  *cap = next; return q;
}
static char *copy(const char *s) { char *t = strdup(s); if (!t) { logf_("out of memory watching files"); exit(1); } return t; }
static bool same_time(struct timespec a, struct timespec b) { return a.tv_sec == b.tv_sec && a.tv_nsec == b.tv_nsec; }
static struct timespec mtime_of(const char *path) {
  struct stat st; struct timespec none = {0, 0};
  return stat(path, &st) == 0 ? MTIME(st) : none;
}
static uint64_t path_hash(const char *s) { uint64_t h = 1469598103934665603ull; for (; *s; s++) h = (h ^ (uint8_t)*s) * 1099511628211ull; return h; }
static int find_file(const char *path) {
  if (!slot_cap) return -1;
  for (uint64_t at = path_hash(path) & (uint64_t)(slot_cap - 1);; at = (at + 1) & (uint64_t)(slot_cap - 1)) {
    if (slots[at] < 0) return -1;
    if (!strcmp(files[slots[at]].path, path)) return slots[at];
  }
}
static void place(int i) {
  uint64_t at = path_hash(files[i].path) & (uint64_t)(slot_cap - 1);
  while (slots[at] >= 0) at = (at + 1) & (uint64_t)(slot_cap - 1);
  slots[at] = i;
}
// The row for path (added with the file's time now: a missing file is time 0, so its
// appearing is a change), with role added.
static int watch_file(const char *path, int role) {
  int i = find_file(path);
  if (i < 0) {
    if ((file_count + 1) * 2 > slot_cap) {   // keep the table at most half full
      int next = slot_cap ? slot_cap * 2 : 1024;
      while ((file_count + 1) * 2 > next) next *= 2;
      int *s = realloc(slots, (size_t)next * sizeof *s);
      if (!s) { logf_("out of memory watching files"); exit(1); }
      slots = s; slot_cap = next;
      for (int k = 0; k < slot_cap; k++) slots[k] = -1;
      for (int k = 0; k < file_count; k++) place(k);
    }
    files = grow(files, &file_cap, file_count + 1, sizeof *files);
    i = file_count++;
    files[i] = (watched){copy(path), mtime_of(path), 0};
    place(i);
  }
  files[i].roles |= role;
  return i;
}
static void watch_reset(void) {
  for (int i = 0; i < file_count; i++) free(files[i].path);
  for (int i = 0; i < unit_count; i++) { free(units[i].out); free(units[i].cmd); free(units[i].src); }
  for (int i = 0; i < near_count; i++) free(near_dirs[i]);
  file_count = unit_count = near_count = 0;
  for (int k = 0; k < slot_cap; k++) slots[k] = -1;
}

static bool engine_kind(const char *name) { return ends_with(name, ".coax") || ends_with(name, ".c") || ends_with(name, ".h"); }
static bool page_kind(const char *name) {
  return ends_with(name, ".html") || ends_with(name, ".js") || ends_with(name, ".md") || ends_with(name, ".svg");
}
// The app's folders, all the way down (nested app folders: realms/NAME/index.html), but not
// build/ (what the build writes) or a dot folder. sources: also .coax, .c and .h, which run the
// whole build when no unit's list names them. The host folder is only its top level.
static void scan_dir(const char *dir, bool deep, bool sources) {
  DIR *d = opendir(dir);
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d))) {
    if (e->d_name[0] == '.') continue;
    char path[PATH_MAX + 1024];
    if (snprintf(path, sizeof path, "%s/%s", dir, e->d_name) >= (int)sizeof path) continue;
    struct stat st;
    if (lstat(path, &st)) continue;
    if (S_ISDIR(st.st_mode)) {
      if (deep && strcmp(e->d_name, "build") && strcmp(e->d_name, "node_modules")) scan_dir(path, deep, sources);
      continue;
    }
    if (!S_ISREG(st.st_mode)) continue;
    if (page_kind(e->d_name) || ends_with(e->d_name, ".wgsl") || (sources && engine_kind(e->d_name))) watch_file(path, ROLE_SCAN);
  }
  closedir(d);
}
// A unit's output with its folder's symlinks resolved, as app_dir is (/tmp is /private/tmp
// on a Mac), so url_of finds it; as written when the folder doesn't resolve.
static char *real_out(const char *path) {
  char dir[PATH_MAX + 1024], real[PATH_MAX], joined[PATH_MAX + 1024];
  const char *slash = strrchr(path, '/');
  if (!slash || slash == path || (size_t)(slash - path) >= sizeof dir) return copy(path);
  memcpy(dir, path, (size_t)(slash - path)); dir[slash - path] = 0;
  if (!realpath(dir, real) || snprintf(joined, sizeof joined, "%s%s", real, slash) >= (int)sizeof joined) return copy(path);
  return copy(joined);
}
// The folder of a listed source outside the app (the app's own folders are scanned whole).
static void near_dir(const char *path) {
  size_t a = strlen(app_dir);
  if (!strncmp(path, app_dir, a) && path[a] == '/') return;
  const char *slash = strrchr(path, '/');
  if (!slash || slash == path) return;
  size_t n = (size_t)(slash - path);
  for (int i = 0; i < near_count; i++) if (strlen(near_dirs[i]) == n && !strncmp(near_dirs[i], path, n)) return;
  near_dirs = grow(near_dirs, &near_cap, near_count + 1, sizeof *near_dirs);
  char *d = malloc(n + 1);
  if (!d) { logf_("out of memory watching files"); exit(1); }
  memcpy(d, path, n); d[n] = 0;
  near_dirs[near_count++] = d;
}
// The source files of those folders that no row has yet, each added as NEAR.
static void scan_near(void) {
  for (int i = 0; i < near_count; i++) {
    DIR *d = opendir(near_dirs[i]);
    if (!d) continue;
    struct dirent *e;
    while ((e = readdir(d))) {
      if (e->d_name[0] == '.' || !engine_kind(e->d_name)) continue;
      char path[PATH_MAX + 1024];
      if (snprintf(path, sizeof path, "%s/%s", near_dirs[i], e->d_name) >= (int)sizeof path || find_file(path) >= 0) continue;
      struct stat st;
      if (lstat(path, &st) || !S_ISREG(st.st_mode)) continue;
      watch_file(path, ROLE_NEAR);
    }
    closedir(d);
  }
}
// Every APP_DIR/build/.live/*.deps: the units and every file each one read.
static void load_deps(void) {
  char dir[PATH_MAX + 64];
  snprintf(dir, sizeof dir, "%s/build/.live", app_dir);
  DIR *d = opendir(dir);
  if (!d) return;
  struct dirent *e;
  char *line = NULL; size_t cap = 0;
  while ((e = readdir(d))) {
    if (e->d_name[0] == '.' || !ends_with(e->d_name, ".deps")) continue;
    char path[PATH_MAX + 1024];
    if (snprintf(path, sizeof path, "%s/%s", dir, e->d_name) >= (int)sizeof path) continue;
    FILE *f = fopen(path, "r");
    if (!f) continue;
    unit u = {0}; bool gone = false;
    bool ok = getline(&line, &cap, f) > 0 && !strcmp(line, "coax-live-1\n");
    ssize_t n;
    while (ok && (n = getline(&line, &cap, f)) > 0) {
      if (line[n - 1] == '\n') line[--n] = 0;
      if (!strncmp(line, "kind ", 5)) u.wgsl = !strcmp(line + 5, "wgsl");
      else if (!strncmp(line, "out ", 4) && !u.out) u.out = real_out(line + 4);
      else if (!strncmp(line, "cmd ", 4) && !u.cmd) u.cmd = copy(line + 4);
      else if (!strncmp(line, "src /", 5) && access(line + 4, F_OK) != 0) gone = true;   // a source deleted since
      else if (!strncmp(line, "src /", 5)) {
        u.src = grow(u.src, &u.src_cap, u.nsrc + 1, sizeof *u.src);
        u.src[u.nsrc++] = watch_file(line + 4, ROLE_SRC);
        near_dir(line + 4);
      }
    }
    fclose(f);
    // A unit whose source is gone is a module deleted since it was built (home-web.coax): its list is stale, and
    // its command can only fail on every change. The list is dropped; were the source to come back, the scan sees
    // a new file no list names and runs the whole build, which writes the list again.
    if (ok && gone) {
      logf_("dropping %s: a source it lists is gone", e->d_name);
      unlink(path);
      int stem = (int)strlen(path) - 5;
      char stamp[PATH_MAX + 1024];
      if (snprintf(stamp, sizeof stamp, "%.*s.started", stem, path) < (int)sizeof stamp) unlink(stamp);
      ok = false;
    }
    if (!ok || !u.out || !u.cmd || !u.nsrc) { free(u.out); free(u.cmd); free(u.src); continue; }
    u.out_mtime = mtime_of(u.out);
    {
      char stamp[PATH_MAX + 1024];
      int stem = (int)strlen(path) - 5;   // without ".deps"
      u.started = snprintf(stamp, sizeof stamp, "%.*s.started", stem, path) < (int)sizeof stamp ? mtime_of(stamp) : (struct timespec){0, 0};
      if (!u.started.tv_sec || later(u.started, u.out_mtime)) u.started = u.out_mtime;   // no stamp, or a build that began and did not end
    }
    units = grow(units, &unit_cap, unit_count + 1, sizeof *units);
    units[unit_count++] = u;
  }
  free(line);
  closedir(d);
}
// What is watched, read again: after every build (its lists may name new files, and new
// files may be in the folders), so each row's time is the time after that build.
static void watch_all(void) {
  watch_reset();
  scan_dir(app_dir, true, true);
  scan_dir(host_dir, false, false);
  load_deps();
  scan_near();
}
// A file that was not there at the last look (#13441): found between builds, about once a
// second, and given time 0, so check_changes takes it as a change the same tick. Before this a
// new page, shader or source waited for the next build's rescan.
static void scan_new(void) {
  int before = file_count;
  scan_dir(app_dir, true, true);
  scan_dir(host_dir, false, false);
  scan_near();
  for (int i = before; i < file_count; i++) files[i].mtime = (struct timespec){0, 0};
}
static bool unit_failed(const unit *u) {
  for (int i = 0; i < failed_count; i++) if (!strcmp(failed_outs[i], u->out)) return true;
  return false;
}
// After a build of `dirty` units (every unit for a whole build): which of them failed last.
static void note_failed(bool success, bool whole) {
  for (int k = 0; k < unit_count; k++) {
    if (!whole && !units[k].dirty) continue;
    int at = -1;
    for (int i = 0; i < failed_count; i++) if (!strcmp(failed_outs[i], units[k].out)) at = i;
    if (success && at >= 0) { free(failed_outs[at]); failed_outs[at] = failed_outs[--failed_count]; }
    if (!success && at < 0) { failed_outs = grow(failed_outs, &failed_cap, failed_count + 1, sizeof *failed_outs); failed_outs[failed_count++] = copy(units[k].out); }
  }
}

// A watched path as the URL a tab asks for (/build/x.wasm, /coaxial/host.js): false when none.
static bool url_of(const char *path, char *out, size_t cap) {
  size_t a = strlen(app_dir), h = strlen(host_dir);
  if (!strncmp(path, app_dir, a) && path[a] == '/') return snprintf(out, cap, "%s", path + a) < (int)cap;
  if (!strncmp(path, host_dir, h) && path[h] == '/') return snprintf(out, cap, "/coaxial%s", path + h) < (int)cap;
  return false;
}
static void note_wgsl(const char *path) {
  for (int i = 0; i < note_count; i++) if (!strcmp(notes[i], path)) return;
  notes = grow(notes, &note_cap, note_count + 1, sizeof *notes);
  notes[note_count++] = copy(path);
}
// "wgsl" for each changed shader, naming it (a tab whose shaders are another file ignores it).
static void tell_wgsl(void) {
  for (int i = 0; i < note_count; i++) {
    char url[PATH_MAX + 1024], msg[PATH_MAX + 1100];
    if (url_of(notes[i], url, sizeof url)) snprintf(msg, sizeof msg, "{\"kind\":\"wgsl\",\"path\":\"%s\"}", url);
    else snprintf(msg, sizeof msg, "{\"kind\":\"wgsl\"}");
    logf_("shader changed: %s", strrchr(notes[i], '/') + 1);
    broadcast(msg);
    free(notes[i]);
  }
  note_count = 0;
}
// "wasm" naming one unit's module, once, when it has been written.
static void tell_wasm(unit *u) {
  char url[PATH_MAX + 1024], msg[PATH_MAX + 1100];
  u->told = true;
  if (!url_of(u->out, url, sizeof url)) return;   // nothing a tab loads
  double ms = monotonic_ms() - build_started;
  logf_("%s rebuilt in %.0f ms", strrchr(u->out, '/') + 1, ms);
  snprintf(msg, sizeof msg, "{\"kind\":\"wasm\",\"ms\":%.0f,\"path\":\"%s\"}", ms, url);
  broadcast(msg);
}
// While the changed units build: a module written is told at once, before its builder
// exits (wasm.sh writes the module, then its drive files).
static void poll_outputs(void) {
  for (int i = 0; i < unit_count; i++) {
    unit *u = &units[i];
    if (!u->dirty || u->told || u->wgsl) continue;
    if (!same_time(mtime_of(u->out), u->out_mtime)) tell_wasm(u);
  }
}

// ---- the files tabs loaded (packs, textures): "watch URL..." from a tab
// The file's SHA-256 as 64 hex digits into hex; false when it can't be read.
static bool file_sha256(const char *path, char *hex) {
  int fd = open(path, O_RDONLY | O_CLOEXEC);
  if (fd < 0) return false;
  struct stat st;
  if (fstat(fd, &st) || !S_ISREG(st.st_mode)) { close(fd); return false; }
  void *p = st.st_size ? mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0) : NULL;
  close(fd);
  if (st.st_size && p == MAP_FAILED) return false;
  coax_t_S_Digest digest = S_hash((coax_string){st.st_size ? p : "", (int64_t)st.st_size});
  char out[64];
  bool ok = !digest.error && S_hex(digest, (coax_v_u8){(uint8_t *)out, 64});
  if (st.st_size) munmap(p, (size_t)st.st_size);
  if (ok) { memcpy(hex, out, 64); hex[64] = 0; }
  return ok;
}
static void watch_url(const char *url) {
  for (int i = 0; i < asset_count; i++) if (!strcmp(assets[i].url, url)) return;
  char path[PATH_MAX + 1024];
  if (url[0] != '/' || strstr(url, "/coaxial/live") || !resolve(url, path, sizeof path)) return;
  assets = grow(assets, &asset_cap, asset_count + 1, sizeof *assets);
  assets[asset_count++] = (loaded){copy(url), copy(path), mtime_of(path), ""};
}
// "watch URL URL...": what the tab loaded, as a text frame.
static void tab_says(const uint8_t *text, size_t n) {
  if (n < 6 || memcmp(text, "watch ", 6)) return;
  char url[1024];
  for (size_t i = 6; i < n;) {
    while (i < n && text[i] == ' ') i++;
    size_t j = i;
    while (j < n && text[j] != ' ') j++;
    if (j > i && j - i < sizeof url) { memcpy(url, text + i, j - i); url[j - i] = 0; watch_url(url); }
    i = j;
  }
}
// A loaded file whose bytes changed: "asset" with its URL and new SHA-256 (a touch that
// leaves the bytes as they were tells nothing).
static void check_assets(void) {
  for (int i = 0; i < asset_count; i++) {
    loaded *a = &assets[i];
    struct timespec t = mtime_of(a->path);
    if (same_time(t, a->mtime)) continue;
    a->mtime = t;
    char hex[65];
    if (!file_sha256(a->path, hex) || !strcmp(hex, a->hash)) continue;
    memcpy(a->hash, hex, sizeof hex);
    char msg[1200];
    snprintf(msg, sizeof msg, "{\"kind\":\"asset\",\"url\":\"%s\",\"hash\":\"%s\"}", a->url, a->hash);
    logf_("asset changed: %s", a->url);
    broadcast(msg);
  }
}

static void json_escape(const char *in, char *out, size_t cap) {
  size_t o = 0;
  for (; *in && o + 7 < cap; in++) {
    unsigned char ch = (unsigned char)*in;
    if (ch == '"' || ch == '\\') { out[o++] = '\\'; out[o++] = (char)ch; }
    else if (ch == '\n') { out[o++] = '\\'; out[o++] = 'n'; }
    else if (ch < 0x20) { o += (size_t)snprintf(out + o, cap - o, "\\u%04x", ch); }
    else out[o++] = (char)ch;
  }
  out[o] = 0;
}

static void build_finished(bool success) {
  static char escaped[REQ_MAX];
  double ms = monotonic_ms() - build_started;
  build_pid = 0;
  if (build_fd >= 0) { close(build_fd); build_fd = -1; }
  build_log[build_used] = 0;
  // A .wgsl the build wrote (WGSL generated from Coax) is a changed shader. Before #13410 the
  // times below were reset without telling the tabs.
  for (int i = 0; i < file_count; i++)
    if ((files[i].roles & ROLE_SCAN) && ends_with(files[i].path, ".wgsl") && !same_time(mtime_of(files[i].path), files[i].mtime)) note_wgsl(files[i].path);
  for (int i = 0; i < unit_count; i++) {
    unit *u = &units[i];
    if (!u->dirty || u->told || same_time(mtime_of(u->out), u->out_mtime)) continue;
    if (u->wgsl) note_wgsl(u->out);
    else tell_wasm(u);
  }
  note_failed(success, !build_targeted);
  // A page file saved while a whole build ran is shown: the tabs load the page again (#13441).
  for (int i = 0; !build_targeted && i < file_count; i++)
    if ((files[i].roles & ROLE_SCAN) && page_kind(files[i].path) && !same_time(mtime_of(files[i].path), files[i].mtime)) build_page = true;
  // An edit saved while the changed units built, or just after a tab was told, is the next
  // build's: a targeted build keeps every source's time from before the rescan, so only its
  // module outputs count as its own. Only a whole build writes sources (the realms' tables).
  typedef struct { char *path; struct timespec mtime; } seen;
  seen *kept = NULL; int kept_count = 0, kept_cap = 0;
  for (int i = 0; build_targeted && i < file_count; i++) {
    if (ends_with(files[i].path, ".wgsl")) continue;
    bool out = false;
    for (int k = 0; k < unit_count && !out; k++) out = !strcmp(units[k].out, files[i].path);
    if (out) continue;
    kept = grow(kept, &kept_cap, kept_count + 1, sizeof *kept);
    kept[kept_count++] = (seen){copy(files[i].path), files[i].mtime};
  }
  if (success && !build_targeted) {
    char msg[128];
    snprintf(msg, sizeof msg, "{\"kind\":\"wasm\",\"ms\":%.0f}", ms);
    logf_("engine rebuilt in %.0f ms", ms);
    broadcast(msg);
  } else if (!success) {
    logf_("build failed in %.0f ms:\n%s", ms, build_log);
    json_escape(build_log, escaped, sizeof escaped - 32);
    char msg[REQ_MAX + 64];
    snprintf(msg, sizeof msg, "{\"kind\":\"error\",\"text\":\"%s\"}", escaped);
    broadcast(msg);
  }
  tell_wgsl();
  if (build_page) broadcast("{\"kind\":\"page\"}");
  build_page = false;
  // What is watched again, at its times now: generated files the build wrote don't start
  // another build. An edit made in response to the completion message belongs to the next one.
  bool whole = !build_targeted;
  watch_all();
  for (int i = 0; i < kept_count; i++) {
    int at = find_file(kept[i].path);
    if (at >= 0) files[at].mtime = kept[i].mtime;
    free(kept[i].path);
  }
  free(kept);
  // A whole build wrote sources of its own (the realms' tables) before its modules began, so
  // their times are the build's. A listed source saved after the build began and after a
  // module that reads it began is an edit that module did not read (#13441): it gets time
  // 0, and the next look builds the modules that read it. Before this the edit was counted
  // as the build's own and lost until the next save.
  for (int k = 0; whole && k < unit_count; k++)
    for (int j = 0; j < units[k].nsrc; j++) {
      watched *f = &files[units[k].src[j]];
      if (later(f->mtime, build_wall) && later(f->mtime, units[k].started)) { logf_("saved during the build: %s", strrchr(f->path, '/') + 1); f->mtime = (struct timespec){0, 0}; }
    }
}

static void build_start(const char *cmd, bool targeted) {
  int p[2];
  build_used = 0; build_started = monotonic_ms(); build_targeted = targeted;
  clock_gettime(CLOCK_REALTIME, &build_wall);
  if (pipe(p)) goto failed;
  p[0] = private_fd(p[0]); p[1] = private_fd(p[1]);
  if (p[0] < 0 || p[1] < 0) goto close_failed;
  int flags = fcntl(p[0], F_GETFL);
  if (flags < 0 || fcntl(p[0], F_SETFL, flags | O_NONBLOCK)) goto close_failed;
  pid_t pid = fork();
  if (pid < 0) goto close_failed;
  if (pid == 0) {
    close(p[0]);
    if (dup2(p[1], STDOUT_FILENO) < 0 || dup2(p[1], STDERR_FILENO) < 0) _exit(127);
    if (p[1] > STDERR_FILENO) close(p[1]);
    signal(SIGPIPE, SIG_DFL);
    execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
    _exit(127);
  }
  close(p[1]); build_fd = p[0]; build_pid = pid;
  return;
close_failed: {
    int error = errno;
    if (p[0] >= 0) close(p[0]);
    if (p[1] >= 0) close(p[1]);
    errno = error;
  }
failed:
  build_used = (size_t)snprintf(build_log, sizeof build_log, "cannot start build: %s", strerror(errno));
  build_finished(false);
}

static void build_drain(void) {
  // Keep draining after the retained log fills, without letting a noisy
  // builder monopolise the HTTP loop. No fread/pclose waits on the child.
  char bytes[4096];
  for (int i = 0; build_fd >= 0 && i < 16; i++) {
    ssize_t n = read(build_fd, bytes, sizeof bytes);
    if (n > 0) {
      size_t keep = (size_t)n;
      if (keep > sizeof build_log - 1 - build_used) keep = sizeof build_log - 1 - build_used;
      memcpy(build_log + build_used, bytes, keep); build_used += keep;
    } else if (n < 0 && errno == EINTR) { i--; }
    else if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
    else { close(build_fd); build_fd = -1; }
  }
}

// The running targeted build has written and told every module it builds (no shader).
static bool units_all_told(void) {
  bool any = false;
  for (int i = 0; i < unit_count; i++) {
    if (!units[i].dirty) continue;
    if (units[i].wgsl || !units[i].told) return false;
    any = true;
  }
  return any;
}
// The build is done as far as the tabs and the watcher go; its tail runs on (see tail).
static void release_build(void) {
  build_drain();
  tails = grow(tails, &tail_cap, tail_count + 1, sizeof *tails);
  tails[tail_count++] = (tail){build_pid, build_fd};
  build_fd = -1;
  build_finished(true);
}

static void reap_children(void) {
  int status; pid_t pid;
  while ((pid = waitpid(-1, &status, WNOHANG)) > 0) {
    for (int i = 0; i < tail_count; i++) {
      if (tails[i].pid != pid) continue;
      if (tails[i].fd >= 0) close(tails[i].fd);
      tails[i] = tails[--tail_count];
      break;
    }
    if (pid == build_pid) {
      build_drain();
      build_finished(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    // Shaped responses and WebSocket tunnels have no completion message.
  }
}

