/* Split from coax/web/serve.c (#13848): local /worlds, /scripts, /assets, and /files routes. */
static void world_url(char *url, size_t cap) {
  const char *at = url + 12; size_t k = strcspn(at, "/?");
  char id[64], rest[1100];
  if (k == 0 || k >= sizeof id) return;
  memcpy(id, at, k); id[k] = 0;
  coax_string named = site_world((coax_string){id, (int64_t)k});
  if (named.count == (int64_t)k && !memcmp(named.data, id, k)) return;
  snprintf(rest, sizeof rest, "%s", at + k);
  snprintf(url, cap, "/api/worlds/%.*s%s", (int)named.count, (const char *)named.data, rest);
}
// Whether the local tools answer this world: Splice Bots, and any world with a row under APP_DIR/build/.worlds.
static bool world_held_here(const char *url) {
  const char *at = url + 12; size_t k = strcspn(at, "/?");
  if (k == 11 && !strncmp(at, "splice-bots", 11)) return true;
  if (k == 0 || k > 64) return false;
  char file[PATH_MAX + 96]; struct stat st;
  snprintf(file, sizeof file, "%s/build/.worlds/%.*s.row", app_dir, (int)k, at);
  return stat(file, &st) == 0;
}
// A world's route on the dev server: the world's own tool answers it ("STATUS\n" then the body),
// and a write that was saved is told to every tab, which then asks for the world's new events.
static void worlds_local(client *c, const char *method, const char *url) {
  char dir[PATH_MAX + 32], body_file[PATH_MAX + 64], cmd[PATH_MAX * 3 + 2200];
  snprintf(dir, sizeof dir, "%s/build/.worlds", app_dir);
  for (const char *p = url; *p; p++)   // the URL goes into a shell command: only what a route's URL holds
    if (!((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || strchr("/?&=-_.", *p))) { respond(c->fd, 404, "Not Found", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  bool post = !strcmp(method, "POST");
  // A lane's events are the lines of its file, one a revision (world-local.coax): a tab asks for
  // them every second, so they are answered here, with no tool run: the lines after revision N.
  char id[40] = "", lane[8] = "live"; long after = 0;
  if (!post && sscanf(url, "/api/worlds/%39[a-z0-9-]/events?%*s", id) == 1 && strstr(url, "/events?")) {
    const char *q = strchr(url, '?') + 1, *l = strstr(q, "lane="), *a = strstr(q, "after=");
    if (l && !strncmp(l + 5, "dev", 3)) snprintf(lane, sizeof lane, "dev");
    if (a) after = strtol(a + 6, NULL, 10);
    char file[PATH_MAX + 96];
    snprintf(file, sizeof file, "%s/%s.%s.events", dir, id, lane);
    FILE *f = fopen(file, "rb");
    size_t cap = 1 << 20, n = 0; char *out = malloc(cap); long line = 0; int ch;
    while (f && out && (ch = fgetc(f)) != EOF) { if (line >= after && n < cap) out[n++] = (char)ch; if (ch == '\n') line++; }
    if (f) fclose(f);
    respond(c->fd, 200, "OK", "text/plain; charset=utf-8", out ? out : "", out ? n : 0);
    free(out); drop(c); return;
  }
  body_file[0] = 0;
  if (post) {
    const char *end = strstr(c->buf, "\r\n\r\n");
    size_t head = end ? (size_t)(end - c->buf) + 4 : c->n;
    mkdir(dir, 0755);
    snprintf(body_file, sizeof body_file, "%s/.request-%d", dir, (int)getpid());
    FILE *f = fopen(body_file, "wb");
    if (!f || fwrite(c->buf + head, 1, c->n - head, f) != c->n - head) { if (f) fclose(f); respond(c->fd, 500, "Error", "application/json", "{\"error\":\"not saved\"}", 21); drop(c); return; }
    fclose(f);
  }
  snprintf(cmd, sizeof cmd, "'%s/tool.sh' '%s/../../metaverse/splice-bots/tools/world-local.coax' '%s' %s '%s' %s%s%s 2>/dev/null", host_dir, host_dir, dir, post ? "POST" : "GET", url,
           post ? "'" : "", body_file, post ? "'" : "");
  FILE *run = popen(cmd, "r");
  size_t cap = 1 << 20, n = 0; char *out = malloc(cap); size_t k;
  while (run && out && n < cap && (k = fread(out + n, 1, cap - n, run)) > 0) n += k;
  int ended = run ? pclose(run) : -1;
  if (post) unlink(body_file);
  char *nl = out ? memchr(out, '\n', n) : NULL;
  int status = nl ? atoi(out) : 0;
  if (ended != 0 || status < 100) respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"the world's tool did not answer\"}", 42);
  else {
    size_t at = (size_t)(nl - out) + 1;
    respond(c->fd, status, status == 200 ? "OK" : "Refused", out[at] == '{' ? "application/json" : "text/plain; charset=utf-8", out + at, n - at);
    if (post && status == 200) {
      char msg[96]; size_t i = 0;
      for (const char *p = url + 12; *p && *p != '/' && *p != '?' && i < sizeof id - 1; p++) id[i++] = *p;
      snprintf(msg, sizeof msg, "{\"kind\":\"world\",\"id\":\"%s\"}", id);
      broadcast(msg);
    }
  }
  free(out);
  drop(c);
}
// A world's realm scripts on the dev server (#13485): GET .../scripts lists them (world.scripts.v1),
// GET .../scripts/HASH answers a module's bytes (the page's script host fetches them) and .../source its
// source, POST .../scripts?name=NAME stores a module (X-Script-Fields: its state fields), and POST
// .../scripts/compile?name=NAME compiles a source with the compile job (coax/web/script-job.sh, the
// API's OA_SCRIPT_JOB) and stores what it made. The world's script tool answers
// (metaverse/splice-bots/tools/script-local.coax: the API's checks, files under APP_DIR/build/.worlds).
static bool scripts_route(const char *url) {
  char id[40]; int end = 0;
  if (sscanf(url, "/api/worlds/%39[a-z0-9-]/scripts%n", id, &end) != 1 || end == 0) return false;
  return !url[end] || url[end] == '?' || url[end] == '/';
}
static void scripts_local(client *c, const char *method, const char *url) {
  char dir[PATH_MAX + 32], cmd[PATH_MAX * 4 + 3000], tmp[PATH_MAX], body[PATH_MAX + 32] = "", extra[PATH_MAX + 32] = "", src[PATH_MAX + 64] = "", out[PATH_MAX + 32] = "";
  char path[1024], name[64] = "", tail[1100] = "";
  snprintf(dir, sizeof dir, "%s/build/.worlds", app_dir);
  for (const char *p = url; *p; p++)   // the URL goes into a shell command: only what a route's URL holds
    if (!((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || strchr("/?&=-_.", *p))) { respond(c->fd, 404, "Not Found", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  bool post = !strcmp(method, "POST");
  if (!post && strcmp(method, "GET")) { respond(c->fd, 405, "Method Not Allowed", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  snprintf(path, sizeof path, "%.*s", (int)strcspn(url, "?"), url);
  const char *q = strstr(url, "name=");
  if (q) { size_t i = 0; for (q += 5; *q && *q != '&' && i < sizeof name - 1; q++) name[i++] = *q; name[i] = 0; }
  // a directory of the request's own, outside the app: the compile job may write only there (script-job.sh)
  const char *td = getenv("TMPDIR");
  snprintf(tmp, sizeof tmp, "%s/oa-script-local-XXXXXX", td && *td ? td : "/tmp");
  mkdir(dir, 0755);
  if (!mkdtemp(tmp)) { respond(c->fd, 500, "Error", "application/json", "{\"error\":\"not saved\"}", 21); drop(c); return; }
  if (post) {
    const char *end = strstr(c->buf, "\r\n\r\n");
    size_t head = end ? (size_t)(end - c->buf) + 4 : c->n;
    bool compile = strlen(path) > 8 && !strcmp(path + strlen(path) - 8, "/compile");
    snprintf(body, sizeof body, "%s/body", tmp);
    FILE *f = fopen(body, "wb");
    bool ok = f && fwrite(c->buf + head, 1, c->n - head, f) == c->n - head;
    if (f) fclose(f);
    if (ok && !compile) {
      // the module's state fields, from its header, for the tool
      for (const char *p = c->buf; end && (p = strstr(p, "\r\n")) && p < end; p += 2) {
        if (strncasecmp(p + 2, "X-Script-Fields:", 16)) continue;
        const char *v = p + 18; while (*v == ' ') v++;
        size_t n = strcspn(v, "\r");
        snprintf(extra, sizeof extra, "%s/fields", tmp);
        FILE *ff = fopen(extra, "wb");
        if (!ff || fwrite(v, 1, n, ff) != n) ok = false;
        if (ff) fclose(ff);
      }
    }
    if (ok && compile) {
      // the job makes the module; the tool checks and stores it, or answers the job's words
      bool named = name[0] && strspn(name, "abcdefghijklmnopqrstuvwxyz0123456789-") == strlen(name);
      snprintf(src, sizeof src, "%s/%s.coax", tmp, named ? name : "script");
      snprintf(out, sizeof out, "%s/out", tmp);
      snprintf(cmd, sizeof cmd, "cp '%s' '%s' && '%s/script-job.sh' '%s' '%s' '%s' > '%s/log' 2>&1", body, src, host_dir, src, named ? name : "script", out, tmp);
      int ran = named ? system(cmd) : -1;
      if (ran == 0) {
        snprintf(body, sizeof body, "%s/%s.wasm", out, name);
        snprintf(extra, sizeof extra, "%s/%s.json", out, name);
        snprintf(path, sizeof path, "%.*s/compiled", (int)(strlen(url) - strlen(strstr(url, "/compile"))), url);
      } else {
        snprintf(body, sizeof body, "%s/log", tmp);
        snprintf(path, sizeof path, "%.*s/refused", (int)(strlen(url) - strlen(strstr(url, "/compile"))), url);
        src[0] = 0; extra[0] = 0;
      }
      snprintf(tail, sizeof tail, "?name=%s", name);
    } else if (q) snprintf(tail, sizeof tail, "?name=%s", name);
    if (!ok) { respond(c->fd, 500, "Error", "application/json", "{\"error\":\"not saved\"}", 21); drop(c); return; }
  }
  // the files given: BODY, then EXTRA (or none), then SOURCE
  char files[PATH_MAX * 3 + 64] = "";
  if (post) snprintf(files, sizeof files, "'%s' '%s'%s%s%s", body, extra[0] ? extra : "/dev/null", src[0] ? " '" : "", src, src[0] ? "'" : "");
  snprintf(cmd, sizeof cmd, "'%s/tool.sh' '%s/../../metaverse/splice-bots/tools/script-local.coax' '%s' %s '%s%s' %s 2>/dev/null", host_dir, host_dir, dir, post ? "POST" : "GET", path, tail, files);
  FILE *run = popen(cmd, "r");
  size_t cap = (1u << 20), n = 0; char *res = malloc(cap); size_t k;
  while (run && res && n < cap && (k = fread(res + n, 1, cap - n, run)) > 0) n += k;
  int ended = run ? pclose(run) : -1;
  snprintf(cmd, sizeof cmd, "rm -rf '%s'", tmp);
  if (system(cmd) != 0) logf_("scripts: %s was not removed", tmp);
  char *nl = res ? memchr(res, '\n', n) : NULL;
  int status = nl ? atoi(res) : 0;
  if (ended != 0 || status < 100) respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"the world's script tool did not answer\"}", 49);
  else {
    size_t at = (size_t)(nl - res) + 1;
    const char *type = res[at] == '{' ? "application/json" : (!post && !strstr(path + 12, "/source") && strlen(path) > 64) ? "application/wasm" : "text/plain; charset=utf-8";
    respond(c->fd, status, status == 200 ? "OK" : "Refused", type, res + at, n - at);
  }
  free(res);
  drop(c);
}
// A world's imported assets on the dev server (#13420): GET .../assets lists them (world.assets.v1),
// GET .../assets/pack answers the import pack, and POST .../assets imports a file
// (world.asset.import.v1: a line, then the file's bytes). The world's asset tool answers
// ("STATUS\n" then the body). An import's body may be longer than a request this server keeps: what
// has not arrived is read here, straight into the body's file, up to ASSET_BODY_MAX (past that the
// answer is a refusal in words, and nothing is read). A saved import is told to every tab as the
// pack's URL and new SHA-256, the live channel's "asset" (#13410), so each tab that loaded the pack
// fetches it again by that hash; a tab that refuses the new pack keeps the one it has (#13441). An
// import a browser sends must come from this server's own page (Sec-Fetch-Site).
#define ASSET_BODY_MAX (4u << 20)
static bool assets_route(const char *url) {
  char id[40]; int end = 0;
  if (sscanf(url, "/api/worlds/%39[a-z0-9-]/assets%n", id, &end) != 1 || end == 0) return false;
  const char *rest = url + end;
  return !*rest || *rest == '?' || !strcmp(rest, "/pack") || !strncmp(rest, "/pack?", 6);
}
static void assets_local(client *c, const char *method, const char *url) {
  char dir[PATH_MAX + 32], body_file[PATH_MAX + 64], cmd[PATH_MAX * 3 + 2200], path[1024];
  snprintf(dir, sizeof dir, "%s/build/.worlds", app_dir);
  for (const char *p = url; *p; p++)   // the URL goes into a shell command: only what a route's URL holds
    if (!((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || strchr("/?&=-_.", *p))) { respond(c->fd, 404, "Not Found", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  bool post = !strcmp(method, "POST");
  if (!post && strcmp(method, "GET")) { respond(c->fd, 405, "Method Not Allowed", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  size_t plen = strcspn(url, "?");   // the tool is given the path: a query only names the bytes a tab wants (?live=HASH)
  if (plen >= sizeof path) plen = sizeof path - 1;
  memcpy(path, url, plen); path[plen] = 0;
  body_file[0] = 0;
  if (post) {
    // a browser's import comes from this server's own page (Sec-Fetch-Site, which a page can't set); a tool sends none
    bool told = false;
    for (const char *p = c->buf; (p = strstr(p, "\r\n")) && p[2] && p[2] != '\r'; p += 2) if (!strncasecmp(p + 2, "Sec-Fetch-Site:", 15)) told = true;
    if (told && !header_is(c->buf, "Sec-Fetch-Site", "same-origin")) {
      const char *no = "{\"error\":\"Only this computer's own page may import into a world.\"}";
      respond(c->fd, 403, "Forbidden", "application/json", no, strlen(no)); drop(c); return;
    }
    const char *end = strstr(c->buf, "\r\n\r\n");
    size_t head = end ? (size_t)(end - c->buf) + 4 : c->n, want = 0;
    for (const char *p = c->buf; end && p < end; p++)
      if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Content-Length:", 15)) want = (size_t)strtoul(p + 15, NULL, 10);
    if (want > ASSET_BODY_MAX) {
      const char *no = "{\"error\":\"That file is too large to import.\"}";
      respond(c->fd, 422, "Refused", "application/json", no, strlen(no)); drop(c); return;
    }
    mkdir(dir, 0755);
    snprintf(body_file, sizeof body_file, "%s/.import-%d", dir, (int)getpid());
    FILE *f = fopen(body_file, "wb");
    size_t got = c->n > head ? c->n - head : 0;
    if (got > want) got = want;
    bool ok = f && fwrite(c->buf + head, 1, got, f) == got;
    double until = monotonic_ms() + 15000;
    while (ok && got < want) {   // the rest of the body, as it arrives
      char part[65536];
      struct pollfd pf = {c->fd, POLLIN, 0};
      double left = until - monotonic_ms();
      if (left <= 0 || poll(&pf, 1, (int)left) <= 0) { ok = false; break; }
      ssize_t k = recv(c->fd, part, want - got < sizeof part ? want - got : sizeof part, 0);
      if (k < 0 && (errno == EAGAIN || errno == EINTR)) continue;
      if (k <= 0 || fwrite(part, 1, (size_t)k, f) != (size_t)k) { ok = false; break; }
      got += (size_t)k;
    }
    if (f) fclose(f);
    if (!ok) { unlink(body_file); respond(c->fd, 400, "Bad Request", "application/json", "{\"error\":\"the file did not arrive whole\"}", 39); drop(c); return; }
  }
  snprintf(cmd, sizeof cmd, "'%s/tool.sh' '%s/../../metaverse/splice-bots/tools/asset-local.coax' '%s' %s '%s' %s%s%s 2>/dev/null", host_dir, host_dir, dir, post ? "POST" : "GET", path,
           post ? "'" : "", body_file, post ? "'" : "");
  FILE *run = popen(cmd, "r");
  size_t cap = (2u << 20) + 64, n = 0; char *out = malloc(cap); size_t k;
  while (run && out && n < cap && (k = fread(out + n, 1, cap - n, run)) > 0) n += k;
  int ended = run ? pclose(run) : -1;
  if (post) unlink(body_file);
  char *nl = out ? memchr(out, '\n', n) : NULL;
  int status = nl ? atoi(out) : 0;
  if (ended != 0 || status < 100) respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"the world's asset tool did not answer\"}", 48);
  else {
    size_t at = (size_t)(nl - out) + 1;
    bool pack = !post && plen > 5 && !strcmp(path + plen - 5, "/pack");
    respond(c->fd, status, status == 200 ? "OK" : "Refused", status != 200 ? "application/json" : pack ? "application/octet-stream" : "text/plain; charset=utf-8", out + at, n - at);
    // "world.assets.v1 ID HASH ...": the pack as it now is, told to every tab
    char id[40], hash[80];
    if (post && status == 200 && n - at < cap - 1) {
      out[n] = 0;
      if (sscanf(out + at, "world.assets.v1 %39[a-z0-9-] %79[0-9a-f]", id, hash) == 2 && strlen(hash) == 64) {
        char msg[256];
        snprintf(msg, sizeof msg, "{\"kind\":\"asset\",\"url\":\"/api/worlds/%s/assets/pack\",\"hash\":\"%s\"}", id, hash);
        logf_("asset imported: /api/worlds/%s/assets/pack", id);
        broadcast(msg);
      }
    }
  }
  free(out);
  drop(c);
}
// Attachments on the dev server (#13468): GET .../files lists a world's and its project's
// (world.files.v1), GET .../files/FID answers one's bytes, POST .../files takes a world.file.*
// request (a file whole, a part of one, a link, an edit or a detach), and GET
// /api/projects/SLUG/files lists a project's own. The attachments tool answers
// ("STATUS TYPE\n" then the body). A body longer than a request this server keeps is read here,
// into the body's file, up to FILES_BODY_MAX. Nothing about a file is logged. A write a browser
// sends must come from this server's own page (Sec-Fetch-Site).
#define FILES_BODY_MAX ((25u << 20) + 2048)
static bool files_route(const char *url) {
  char id[48], fid[20]; int end = 0;
  if (sscanf(url, "/api/projects/%47[a-z0-9-]/files%n", id, &end) == 1 && end > 0) return !url[end] || url[end] == '?';
  end = 0;
  if (sscanf(url, "/api/worlds/%39[a-z0-9-]/files%n", id, &end) != 1 || end == 0) return false;
  const char *rest = url + end;
  return !*rest || *rest == '?' || (sscanf(rest, "/%16[0-9a-f]", fid) == 1 && strlen(fid) == 16 && (!rest[17] || rest[17] == '?'));
}
static void files_local(client *c, const char *method, const char *url) {
  char dir[PATH_MAX + 32], body_file[PATH_MAX + 64], cmd[PATH_MAX * 3 + 2200];
  snprintf(dir, sizeof dir, "%s/build/.worlds", app_dir);
  for (const char *p = url; *p; p++)   // the URL goes into a shell command: only what a route's URL holds
    if (!((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9') || strchr("/?&=-_.", *p))) { respond(c->fd, 404, "Not Found", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  bool post = !strcmp(method, "POST");
  if (!post && strcmp(method, "GET")) { respond(c->fd, 405, "Method Not Allowed", "application/json", "{\"error\":\"not found\"}", 21); drop(c); return; }
  body_file[0] = 0;
  if (post) {
    bool told = false;
    for (const char *p = c->buf; (p = strstr(p, "\r\n")) && p[2] && p[2] != '\r'; p += 2) if (!strncasecmp(p + 2, "Sec-Fetch-Site:", 15)) told = true;
    if (told && !header_is(c->buf, "Sec-Fetch-Site", "same-origin")) {
      const char *no = "{\"error\":\"Only this computer's own page may attach files.\"}";
      respond(c->fd, 403, "Forbidden", "application/json", no, strlen(no)); drop(c); return;
    }
    const char *end = strstr(c->buf, "\r\n\r\n");
    size_t head = end ? (size_t)(end - c->buf) + 4 : c->n, want = 0;
    for (const char *p = c->buf; end && p < end; p++)
      if ((p == c->buf || p[-1] == '\n') && !strncasecmp(p, "Content-Length:", 15)) want = (size_t)strtoul(p + 15, NULL, 10);
    if (want > FILES_BODY_MAX) {
      char no[160];
      int k = snprintf(no, sizeof no, "{\"error\":\"That file is %zu MB. A file can be 25 MB at most.\"}", (want + (1u << 20) - 1) >> 20);
      respond(c->fd, 422, "Refused", "application/json", no, (size_t)k); drop(c); return;
    }
    mkdir(dir, 0755);
    snprintf(body_file, sizeof body_file, "%s/.files-%d", dir, (int)getpid());
    FILE *f = fopen(body_file, "wb");
    size_t got = c->n > head ? c->n - head : 0;
    if (got > want) got = want;
    bool ok = f && fwrite(c->buf + head, 1, got, f) == got;
    double until = monotonic_ms() + 60000;
    while (ok && got < want) {
      char part[65536];
      struct pollfd pf = {c->fd, POLLIN, 0};
      double left = until - monotonic_ms();
      if (left <= 0 || poll(&pf, 1, (int)left) <= 0) { ok = false; break; }
      ssize_t k = recv(c->fd, part, want - got < sizeof part ? want - got : sizeof part, 0);
      if (k < 0 && (errno == EAGAIN || errno == EINTR)) continue;
      if (k <= 0 || fwrite(part, 1, (size_t)k, f) != (size_t)k) { ok = false; break; }
      got += (size_t)k;
    }
    if (f) fclose(f);
    if (!ok) { unlink(body_file); respond(c->fd, 400, "Bad Request", "application/json", "{\"error\":\"the file did not arrive whole\"}", 39); drop(c); return; }
  }
  snprintf(cmd, sizeof cmd, "'%s/tool.sh' '%s/../../metaverse-engine/tools/files-local.coax' '%s' %s '%s' %s%s%s 2>/dev/null", host_dir, host_dir, dir, post ? "POST" : "GET", url,
           post ? "'" : "", body_file, post ? "'" : "");
  FILE *run = popen(cmd, "r");
  size_t cap = (26u << 20) + 4096, n = 0; char *out = malloc(cap); size_t k;
  while (run && out && n < cap && (k = fread(out + n, 1, cap - n, run)) > 0) n += k;
  int ended = run ? pclose(run) : -1;
  if (post) unlink(body_file);
  char *nl = out ? memchr(out, '\n', n < 200 ? n : 200) : NULL;
  int status = nl ? atoi(out) : 0;
  char type[96] = "text/plain; charset=utf-8";
  const char *sp = nl ? memchr(out, ' ', (size_t)(nl - out)) : NULL;
  if (sp && (size_t)(nl - sp - 1) < sizeof type) { memcpy(type, sp + 1, (size_t)(nl - sp - 1)); type[nl - sp - 1] = 0; }
  if (ended != 0 || status < 100) respond(c->fd, 502, "Bad Gateway", "application/json", "{\"error\":\"the attachments tool did not answer\"}", 46);
  else {
    size_t at = (size_t)(nl - out) + 1;
    respond(c->fd, status, status == 200 ? "OK" : "Refused", type, out + at, n - at);
  }
  free(out);
  drop(c);
}
// Whether the request's body (Content-Length) is all in.
