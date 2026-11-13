// A world's realm scripts (GitHub #13467; docs/protocol/world-scripts-v1.md,
// docs/research/2026-11-13-realm-scripts.md). A world names a script by its module's SHA-256 in a
// row of its content tables; these routes store the module behind the module check, serve it, compile
// a source into one, take reports and switch a module off.
//
//   GET  /api/worlds/<id>/scripts                  the world's stored scripts and the limits (world.scripts.v1)
//   GET  /api/worlds/<id>/scripts/<hash>           the module's bytes (application/wasm, named by its hash: immutable).
//                                                  404 when the world stores none; 410 when it is switched off
//   GET  /api/worlds/<id>/scripts/<hash>/source    the source the module was compiled from (text/plain), for who may
//                                                  store; 404 for a module that was uploaded
//   POST /api/worlds/<id>/scripts?name=NAME        body: a module's bytes. X-Script-Fields: the state record's fields as
//                                                  a JSON array of {name, type, at, size} (optional). The module is
//                                                  checked, its manifest written from the bytes and both stored; the
//                                                  answer is the manifest (realm.script.v1). 422: a refused module
//   POST /api/worlds/<id>/scripts/compile?name=NAME  body: a script's Coax source. The compile job (OA_SCRIPT_JOB) makes
//                                                  the module, which is then checked and stored as above, with its source.
//                                                  422 with the compiler's words; 503 when this server has no job
//   POST /api/worlds/<id>/scripts/<hash>/report    body: a short reason. Any account that is logged in
//   GET  /api/admin/scripts/reports                an admin: the open reports (world.script.reports.v1)
//   POST /api/admin/scripts/<hash>/off             an admin, body: why. The module is switched off everywhere and its
//                                                  open reports are marked removed
//   POST /api/admin/scripts/<hash>/on              an admin: switched back on
//   POST /api/admin/scripts/<hash>/keep            an admin: its open reports are marked kept
//
// This file moves facts in and answers out. The rules are script-policy.coax's (#authority): who may
// store, the limits, what an edit may name, what a publish asks about (applied by script-store.coax). The module check, the manifest
// and every byte answered are metaverse-engine/script's (host.coax, manifest.coax, store.coax). SQL
// (migrations/0186_world_scripts.sql) stores the rows and holds the world's row lock for a store; it
// decides nothing. Storing a script is authoring: nothing here reaches the character record or the
// ledger, and nothing here runs a script.
#include "api.h"
#include "script-store.generated.h"

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define SCRIPT_WORDS 600       // a refusal's words (store.coax SS_WORDS)
#define SCRIPT_SOURCE 65536    // a source's bytes (SS_SOURCE)
#define SCRIPT_MODULE 65536    // a module's (wasm.coax WM_BYTES)
#define SCRIPT_MANIFEST 8192   // a manifest's (manifest.coax SM_TEXT)
#define SCRIPT_JOB_MS 90000    // how long this service waits for the compile job, which has a shorter limit of its own
#define SCRIPT_DAY_MS 86400000LL

static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_string csn(const char *s, size_t n) { return (coax_string){s, (int64_t)n}; }

// What a request works in: the two slots a module is checked in (about a megabyte each), the world's
// rows and stored facts, and the answer. Static: this service answers one request at a time, and a
// child that compiles has its own copy.
static coax_t_SsWork work;
static coax_v_t_SsWork W(void) { return (coax_v_t_SsWork){&work, 1}; }

static void answer_out(int fd, int status) {
  coax_string s = wsp_out(W());
  if (!s.count) { world_refuse(fd, 500, "not written"); return; }
  api_respond(fd, status, "application/json", NULL, s.data, (size_t)s.count);
}
// A refusal by code, with the code's words or the ones given, and the limit it states.
static void refuse_code(int fd, int64_t code, const char *words, int64_t limit, long long left_ms) {
  if (!wsp_refuse(W(), code, cs(words ? words : ""), limit, left_ms)) { world_refuse(fd, 500, "not written"); return; }
  answer_out(fd, (int)wsp_status(code));
}
static bool run(PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK || PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("scripts: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static void rollback(PGconn *db) { PQclear(PQexec(db, "ROLLBACK")); }
// One number a query answers, or -1.
static long long ask_n(PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  long long out = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoll(PQgetvalue(r, 0, 0)) : -1;
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("scripts: %s", PQerrorMessage(db));
  PQclear(r);
  return out;
}
// One hit on a daily limit for `who`, in the count every instance shares (auth.limit_counts): how many
// in the window, this one counted, and the ms until the window lifts. A count that can't be read
// refuses nobody (0).
static long long day_hit(PGconn *db, const char *name, const char *who, long long *left_ms) {
  *left_ms = 0;
  const char *v[2] = {name, who};
  // the count in one statement (auth.limit_hit's, with count_it true): a window that has ended starts again
  PGresult *r = PQexecParams(db,
    "WITH t AS (SELECT projects.now_ms() AS now_ms) "
    "INSERT INTO auth.limit_counts AS c (name, key, started_ms, n) "
    "SELECT $1, left(encode(sha256(convert_to($2, 'UTF8')), 'hex'), 40), t.now_ms, 1 FROM t "
    "ON CONFLICT (name, key) DO UPDATE SET "
    "started_ms = CASE WHEN c.started_ms <= (SELECT now_ms FROM t) - 86400000 THEN (SELECT now_ms FROM t) ELSE c.started_ms END, "
    "n = CASE WHEN c.started_ms <= (SELECT now_ms FROM t) - 86400000 THEN 0 ELSE c.n END + 1 "
    "RETURNING c.started_ms, c.n, (SELECT now_ms FROM t)", 2, NULL, v, NULL, NULL, 0);
  long long n = 0;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    n = atoll(PQgetvalue(r, 0, 1));
    long long left = atoll(PQgetvalue(r, 0, 0)) + SCRIPT_DAY_MS - atoll(PQgetvalue(r, 0, 2));
    *left_ms = left > 0 ? left : 1;
  } else api_log("scripts: limit %s: %s", name, PQerrorMessage(db));
  PQclear(r);
  return n;
}

// Who asks: the account, else the key no account holds; "" for nobody.
static const char *who_of(const request *q) {
  const auth_principal *p = auth_principal_of(q);
  return !p ? "" : p->account[0] ? p->account : p->pubkey;
}
// Whether whoever asks may store or compile a script for this world: the verdict (0: yes).
static int64_t may_store(const request *q, PGconn *db, const char *id, bool exists) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]), admin = p && p->admin;
  int64_t role = 0;
  if (known && !admin && p->pubkey[0]) {
    const char *v[2] = {id, p->pubkey};
    PGresult *r = PQexecParams(db, "SELECT " API_REALM_ROLE_FACTS("$1", "$2") " FROM " API_REALM_ROLE_FROM("$1", "$2"), 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      bool f[5];
      for (int i = 0; i < 5; i++) f[i] = !strcmp(PQgetvalue(r, 0, i), "t");
      role = wsp_edit_role(f[0], f[1], f[2], false, f[3], f[4]);
    }
    PQclear(r);
  }
  return wsp_may(auth_same_site(q), known, admin, role, exists);
}
static void refuse_may(int fd, int64_t verdict) {
  coax_string s = wsp_may_words(verdict);
  char words[120]; snprintf(words, sizeof words, "%.*s", (int)s.count, s.data);
  world_refuse(fd, (int)wsp_may_status(verdict), words);
}

// The stored modules' facts into the work: each manifest, and whether its hash is switched off.
static bool facts_load(PGconn *db, const char *id) {
  wsp_have_clear(W());
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, "SELECT s.manifest, o.hash IS NOT NULL FROM verse.world_scripts s LEFT JOIN verse.script_off o ON o.hash = s.hash WHERE s.world = $1 ORDER BY s.hash", 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("scripts: %s", PQerrorMessage(db));
  for (int i = 0; ok && i < PQntuples(r); i++)
    if (!wsp_have_add(W(), cs(PQgetvalue(r, i, 0)), PQgetvalue(r, i, 1)[0] == 't')) api_log("scripts: %s: a stored manifest does not read", id);
  PQclear(r);
  return ok;
}

// ---- what worlds.c asks

// An edit, once the realm's own check has passed. was, now: the world's content tables before the
// batch and as it leaves them. True when it may be saved; else the words into why.
bool scripts_edit_ok(PGconn *db, const char *id, const char *was, const char *now, char *why, size_t cap) {
  why[0] = 0;
  if (wsp_scan(W(), 1, cs(was)) < 0) wsp_scan_none(W(), 1);   // rows that did not read named nothing: every row the batch leaves is new
  if (wsp_scan(W(), 0, cs(now)) >= 0 && facts_load(db, id) && wsp_edit(W()) == 0) return true;
  coax_string s = wsp_said(W());
  if (s.count) snprintf(why, cap, "%.*s", (int)s.count, s.data); else snprintf(why, cap, "not read");
  return false;
}
// A publish. dev, live: the lanes' content tables (live NULL when nothing is published). True when it
// may go on; else the refusal was answered (409: what the publisher confirms; 422: a row names a
// module the world has not stored).
bool scripts_publish_ok(const request *q, PGconn *db, const char *id, const char *dev, const char *live) {
  char confirm[600] = "";
  api_query(q, "confirm", confirm, sizeof confirm);
  if (wsp_scan(W(), 0, cs(dev)) < 0 || (live && wsp_scan(W(), 1, cs(live)) < 0) || !facts_load(db, id)) { world_refuse(q->fd, 500, "not read"); return false; }
  if (!live) wsp_scan_none(W(), 1);   // nothing is live: no rows
  int64_t status = wsp_publish(W(), cs(confirm));
  if (status == 0) return true;
  answer_out(q->fd, (int)status);
  return false;
}

// ---- reading

static void script_list(const request *q, PGconn *db, const char *id) {
  if (!world_scripts_world(db, id, false, NULL)) { world_refuse(q->fd, 404, "no such world"); return; }
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, "SELECT s.manifest, o.hash IS NOT NULL, coalesce(o.why, ''), s.source IS NOT NULL FROM verse.world_scripts s LEFT JOIN verse.script_off o ON o.hash = s.hash "
                                 "WHERE s.world = $1 ORDER BY s.hash", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("scripts: %s", PQerrorMessage(db)); PQclear(r); world_refuse(q->fd, 500, "not read"); return; }
  const char *job = getenv("OA_SCRIPT_JOB");
  bool ok = wsp_list_begin(W(), cs(id), job && *job);
  for (int i = 0; ok && i < PQntuples(r); i++)
    ok = wsp_list_add(W(), cs(PQgetvalue(r, i, 0)), PQgetvalue(r, i, 1)[0] == 't', cs(PQgetvalue(r, i, 2)), PQgetvalue(r, i, 3)[0] == 't', i == 0);
  PQclear(r);
  if (!ok || !wsp_list_end(W())) { world_refuse(q->fd, 500, "not written"); return; }
  answer_out(q->fd, 200);
}
static void script_get(const request *q, PGconn *db, const char *id, const char *hash) {
  const char *v[2] = {id, hash};
  PGresult *r = PQexecParams(db, "SELECT s.bytes, coalesce(o.why, ''), o.hash IS NOT NULL FROM verse.world_scripts s LEFT JOIN verse.script_off o ON o.hash = s.hash WHERE s.world = $1 AND s.hash = $2",
                             2, NULL, v, NULL, NULL, 1);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("scripts: %s", PQerrorMessage(db)); PQclear(r); world_refuse(q->fd, 500, "not read"); return; }
  bool stored = PQntuples(r) == 1, off = stored && PQgetvalue(r, 0, 2)[0] != 0;   // a binary bool: one byte, 1 for true
  int64_t code = wsp_get(stored, off);
  if (code != 0) {
    char words[400] = "";
    if (off) snprintf(words, sizeof words, "that script is switched off: %.*s", PQgetlength(r, 0, 1), PQgetvalue(r, 0, 1));
    PQclear(r);
    refuse_code(q->fd, code, words, 0, 0);
    return;
  }
  api_respond_immutable(q->fd, "application/wasm", PQgetvalue(r, 0, 0), (size_t)PQgetlength(r, 0, 0));
  PQclear(r);
}
static void script_source(const request *q, PGconn *db, const char *id, const char *hash) {
  int64_t verdict = may_store(q, db, id, world_scripts_world(db, id, false, NULL));
  if (verdict != 0) { refuse_may(q->fd, verdict); return; }
  const char *v[2] = {id, hash};
  PGresult *r = PQexecParams(db, "SELECT source FROM verse.world_scripts WHERE world = $1 AND hash = $2 AND source IS NOT NULL", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 0), (size_t)PQgetlength(r, 0, 0));
  else refuse_code(q->fd, wsp_get(false, false), "this world keeps no source for that script", 0, 0);
  PQclear(r);
}

// ---- storing

// The modules no lane's rows name that were stored a day ago or more leave, to make room. The world's
// row is locked.
static void collect(PGconn *db, const char *id) {
  char *tables[3] = {NULL, NULL, NULL};
  if (!world_scripts_world(db, id, false, tables)) return;
  bool read = true;
  for (int lane = 0; lane < 3; lane++) {
    if (!tables[lane]) wsp_scan_none(W(), lane);   // the lane holds no revision: it names nothing
    else if (wsp_scan(W(), lane, cs(tables[lane])) < 0) read = false;   // unsure: everything stays
    free(tables[lane]);
  }
  if (!read) return;
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, "SELECT hash, at_ms FROM verse.world_scripts WHERE world = $1", 1, NULL, v, NULL, NULL, 0);
  long long now = (long long)api_now_ms();
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r); i++) {
    const char *dv[2] = {id, PQgetvalue(r, i, 0)};
    if (wsp_collect(wsp_named(W(), cs(dv[1])), now - atoll(PQgetvalue(r, i, 1)))) run(db, "DELETE FROM verse.world_scripts WHERE world = $1 AND hash = $2", 2, dv);
  }
  PQclear(r);
}
// The module the work last checked (wsp_manifest, wsp_hash) stored for the world, and the manifest
// answered. bytes: the module's. source: what it was compiled from (NULL: it was uploaded). today, left:
// the author's count for the day and when it lifts. compile: the limit a refusal states is the compiles'.
static void store_checked(const request *q, PGconn *db, const char *id, const char *who, const void *bytes, size_t n, const char *source, long long today, long long left, bool compile) {
  char hash[65], manifest[SCRIPT_MANIFEST + 1];
  coax_string h = wsp_hash(W()), m = wsp_manifest(W());
  if (h.count != 64 || m.count < 1 || m.count > SCRIPT_MANIFEST) { world_refuse(q->fd, 500, "not written"); return; }
  snprintf(hash, sizeof hash, "%.64s", h.data); memcpy(manifest, m.data, (size_t)m.count); manifest[m.count] = 0;
  const char *wv[1] = {id}, *hv[2] = {id, hash};
  PQclear(PQexec(db, "BEGIN"));
  if (ask_n(db, "SELECT count(*) FROM (SELECT 1 FROM verse.world_docs WHERE id = $1 FOR UPDATE) w", 1, wv) != 1) { rollback(db); world_refuse(q->fd, 404, "no such world"); return; }
  PGresult *r = PQexecParams(db, "SELECT why FROM verse.script_off WHERE hash = $1", 1, NULL, hv + 1, NULL, NULL, 0);
  bool off = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  char why[400] = "";
  if (off) snprintf(why, sizeof why, "that script is switched off: %s", PQgetvalue(r, 0, 0));
  PQclear(r);
  r = PQexecParams(db, "SELECT manifest FROM verse.world_scripts WHERE world = $1 AND hash = $2", 2, NULL, hv, NULL, NULL, 0);
  bool already = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (already) snprintf(manifest, sizeof manifest, "%s", PQgetvalue(r, 0, 0));   // the module is stored: its manifest stands
  PQclear(r);
  long long count = ask_n(db, "SELECT count(*) FROM verse.world_scripts WHERE world = $1", 1, wv);
  if (count < 0) { rollback(db); world_refuse(q->fd, 500, "not read"); return; }
  int64_t code = wsp_put(off, today, already, count);
  if (wsp_full(code)) {
    // the store is full: what no revision names and is a day old goes, and the world is asked again
    collect(db, id);
    count = ask_n(db, "SELECT count(*) FROM verse.world_scripts WHERE world = $1", 1, wv);
    code = wsp_put(off, today, already, count < 0 ? 0x7fffffff : count);
  }
  if (code != 0) { rollback(db); refuse_code(q->fd, code, off ? why : "", wsp_limit(code, compile, false), wsp_lifts(code) ? left : 0); return; }
  bool ok = true;
  if (!already) {
    char at[24]; snprintf(at, sizeof at, "%lld", (long long)api_now_ms());
    const char *iv[7] = {id, hash, bytes, manifest, who, source, at};
    int il[7] = {0, 0, (int)n, 0, 0, 0, 0}, ifm[7] = {0, 0, 1, 0, 0, 0, 0};
    PGresult *ir = PQexecParams(db, "INSERT INTO verse.world_scripts (world, hash, bytes, manifest, author, source, at_ms) VALUES ($1, $2, $3, $4, $5, $6, $7::bigint)", 7, NULL, iv, il, ifm, 0);
    ok = PQresultStatus(ir) == PGRES_COMMAND_OK;
    if (!ok) api_log("scripts: %s", PQerrorMessage(db));
    PQclear(ir);
  } else if (source) {
    // a module that was uploaded and is now compiled from a source keeps that source
    const char *sv[3] = {id, hash, source};
    ok = run(db, "UPDATE verse.world_scripts SET source = $3 WHERE world = $1 AND hash = $2 AND source IS NULL", 3, sv);
  }
  if (!ok || !run(db, "COMMIT", 0, NULL)) { rollback(db); world_refuse(q->fd, 500, "not saved"); return; }
  api_respond(q->fd, 200, "application/json", NULL, manifest, strlen(manifest));
}
// POST .../scripts?name=NAME: a module's bytes.
static void script_put(const request *q, PGconn *db, const char *id) {
  int64_t verdict = may_store(q, db, id, world_scripts_world(db, id, false, NULL));
  if (verdict != 0) { refuse_may(q->fd, verdict); return; }
  char name[80] = "", fields[4096] = "";
  api_query(q, "name", name, sizeof name);
  if (q->body_len < 8 || !wsp_name_ok(cs(name))) { refuse_code(q->fd, wsp_bad(), "", 0, 0); return; }
  if (!api_header(q, "X-Script-Fields", fields, sizeof fields)) fields[0] = 0;
  long long left = 0, today = day_hit(db, "script-upload", who_of(q), &left);
  int64_t status = wsp_module(W(), (coax_v_u8){(uint8_t *)q->body, (int64_t)q->body_len}, cs(name), cs(fields));
  if (status != 200) { answer_out(q->fd, (int)status); return; }
  store_checked(q, db, id, who_of(q), q->body, q->body_len, NULL, today, left, false);
}

// ---- the compile job (metaverse-engine/tools/script-job.coax through OA_SCRIPT_JOB): a command that
// takes SOURCE NAME OUTDIR, writes OUTDIR/NAME.wasm and OUTDIR/NAME.json and exits 0, or says why not on
// its output and exits 1. It runs tools that are not a sandbox, under limits of its own. Nothing it
// writes is believed but the module's bytes, which are checked as any upload, and the name and state
// fields of its manifest.

// A file's bytes (at most cap) into out: how many, or -1.
static long file_read(const char *path, char *out, size_t cap) {
  int fd = open(path, O_RDONLY | O_NOFOLLOW);
  if (fd < 0) return -1;
  size_t n = 0; ssize_t k;
  while (n < cap && (k = read(fd, out + n, cap - n)) > 0) n += (size_t)k;
  char one;
  bool more = n == cap && read(fd, &one, 1) > 0;
  close(fd);
  return more ? -1 : (long)n;
}
// The job run on a source in a directory of its own: 0 with the module and the job's manifest read
// back, 1 with the job's words, 2 when it could not be run or ran past this service's wait.
static int job_run(const char *cmd, const char *name, const char *source, size_t source_n, char *wasm, long *wasm_n, char *made, long *made_n, char *words, size_t words_cap) {
  const char *tmp = getenv("TMPDIR");
  char dir[400], src[480], out[480], log[480], path[600];
  snprintf(dir, sizeof dir, "%s/oa-script-XXXXXX", tmp && *tmp ? tmp : "/tmp");
  snprintf(words, words_cap, "the compile job could not be run");
  if (!mkdtemp(dir)) return 2;
  snprintf(src, sizeof src, "%s/%s.coax", dir, name); snprintf(out, sizeof out, "%s/out", dir); snprintf(log, sizeof log, "%s/log", dir);
  int result = 2;
  int sfd = open(src, O_WRONLY | O_CREAT | O_EXCL, 0600);
  bool wrote = sfd >= 0 && write(sfd, source, source_n) == (ssize_t)source_n;
  if (sfd >= 0) close(sfd);
  pid_t pid = wrote ? fork() : -1;
  if (pid == 0) {
    setpgid(0, 0);
    int lfd = open(log, O_WRONLY | O_CREAT | O_TRUNC, 0600), nfd = open("/dev/null", O_RDONLY);
    if (lfd < 0 || nfd < 0) _exit(126);
    dup2(nfd, 0); dup2(lfd, 1); dup2(lfd, 2);
    for (int fd = 3; fd < 256; fd++) close(fd);   // the job holds no socket and no database connection of this service
    // the job inherits none of this service's environment: only where its tools are
    static const char *const keep[] = {"PATH", "HOME", "TMPDIR", "COAX", "COAXIAL_CLANG", "COAXIAL_RESOURCES", "COAXIAL_TOOLCHAIN", "OA_SCRIPT_JOB_SANDBOX"};
    static char vars[8][1200]; char *envp[9]; int en = 0;
    for (size_t i = 0; i < sizeof keep / sizeof keep[0]; i++) {
      const char *v = getenv(keep[i]);
      if (v && *v && snprintf(vars[en], sizeof vars[en], "%s=%s", keep[i], v) < (int)sizeof vars[en]) { envp[en] = vars[en]; en++; }
    }
    envp[en] = NULL;
    execle(cmd, cmd, src, name, out, (char *)NULL, envp);
    _exit(127);
  }
  if (pid > 0) {
    int status = 0; long long waited = 0; pid_t got = 0;
    while ((got = waitpid(pid, &status, WNOHANG)) == 0 && waited < SCRIPT_JOB_MS) { struct timespec ts = {0, 20000000}; nanosleep(&ts, NULL); waited += 20; }
    if (got == 0) { kill(-pid, SIGKILL); kill(pid, SIGKILL); waitpid(pid, &status, 0); snprintf(words, words_cap, "the compile job ran past its time limit"); }
    else if (got == pid && WIFEXITED(status) && WEXITSTATUS(status) == 0) {
      snprintf(path, sizeof path, "%s/%s.wasm", out, name); *wasm_n = file_read(path, wasm, SCRIPT_MODULE);
      snprintf(path, sizeof path, "%s/%s.json", out, name); *made_n = file_read(path, made, SCRIPT_MANIFEST);
      if (*wasm_n > 0 && *made_n > 0) result = 0; else snprintf(words, words_cap, "the compile job made no module");
    } else if (got == pid && WIFEXITED(status) && WEXITSTATUS(status) == 1) {
      long n = file_read(log, words, words_cap - 1);
      if (n < 0) n = (long)words_cap - 1;   // longer than a refusal's words: its start
      while (n > 0 && (words[n - 1] == '\n' || words[n - 1] == ' ')) n--;
      for (long i = 0; i < n; i++) if ((unsigned char)words[i] < 32 || (unsigned char)words[i] > 126) words[i] = ' ';
      words[n] = 0;
      if (!n) snprintf(words, words_cap, "the compile job refused the source");
      result = 1;
    }
    kill(-pid, SIGKILL);   // nothing the job started outlives it
  }
  // the directory goes: the files this service and the job's contract name, then the directory
  snprintf(path, sizeof path, "%s/%s.wasm", out, name); unlink(path);
  snprintf(path, sizeof path, "%s/%s.json", out, name); unlink(path);
  rmdir(out); unlink(src); unlink(log);
  if (rmdir(dir) != 0) api_log("scripts: the compile job left files in its directory");
  return result;
}
// POST .../scripts/compile?name=NAME: a script's source.
static void script_compile(const request *q, PGconn *db, const char *id) {
  int64_t verdict = may_store(q, db, id, world_scripts_world(db, id, false, NULL));
  if (verdict != 0) { refuse_may(q->fd, verdict); return; }
  const char *job = getenv("OA_SCRIPT_JOB");
  char name[80] = "";
  api_query(q, "name", name, sizeof name);
  bool runner = job && *job;
  long long left = 0, today = 0;
  int64_t code = wsp_compile(runner, wsp_name_ok(cs(name)) && !memchr(q->body, 0, q->body_len) ? (int64_t)q->body_len : 0, 0);
  if (code == 0) { today = day_hit(db, "script-compile", who_of(q), &left); code = wsp_compile(runner, (int64_t)q->body_len, today); }
  if (code != 0) { refuse_code(q->fd, code, "", wsp_limit(code, true, false), wsp_lifts(code) ? left : 0); return; }
  // the job takes seconds and this service answers one request at a time: a child runs it, with a
  // connection of its own, and answers (as drops.c does)
  signal(SIGCHLD, SIG_IGN);
  if (fork() != 0) return;
  signal(SIGCHLD, SIG_DFL);
  char *wasm = malloc(SCRIPT_MODULE), *made = malloc(SCRIPT_MANIFEST + 1), *source = malloc(q->body_len + 1), words[SCRIPT_WORDS];
  long wasm_n = 0, made_n = 0;
  if (!wasm || !made || !source) { world_refuse(q->fd, 500, "not read"); _exit(0); }
  memcpy(source, q->body, q->body_len); source[q->body_len] = 0;
  int ran = job_run(job, name, source, q->body_len, wasm, &wasm_n, made, &made_n, words, sizeof words);
  if (ran != 0) {
    if (ran == 2) api_log("scripts: %s", words);
    refuse_code(q->fd, wsp_source(), words, 0, 0);
    _exit(0);
  }
  int64_t status = wsp_compiled(W(), (coax_v_u8){(uint8_t *)wasm, (int64_t)wasm_n}, cs(name), csn(made, (size_t)made_n));
  if (status != 200) { answer_out(q->fd, (int)status); _exit(0); }
  PGconn *c = api_db_new();
  if (!c) { world_refuse(q->fd, 503, "no database"); _exit(0); }
  store_checked(q, c, id, who_of(q), wasm, (size_t)wasm_n, source, today, left, true);
  _exit(0);
}

// ---- a report, and what an admin does with it

static void script_report(const request *q, PGconn *db, const char *id, const char *hash) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]);
  int64_t verdict = wsp_may_report(auth_same_site(q), known, world_scripts_world(db, id, false, NULL));
  if (verdict != 0) { refuse_may(q->fd, verdict); return; }
  char why[200] = "";
  size_t n = q->body_len < sizeof why ? q->body_len : sizeof why - 1;
  memcpy(why, q->body, n); why[n] = 0;
  while (n > 0 && (why[n - 1] == '\n' || why[n - 1] == '\r' || why[n - 1] == ' ')) why[--n] = 0;
  bool why_ok = q->body_len < sizeof why && wsp_why_ok(cs(why));
  const char *hv[2] = {id, hash}, *who = who_of(q);
  bool stored = ask_n(db, "SELECT count(*) FROM verse.world_scripts WHERE world = $1 AND hash = $2", 2, hv) == 1;
  long long left = 0, left_day = 0, today = 0;
  bool minute_ok = true;
  if (wsp_report(stored, why_ok, true, 0) == 0) { minute_ok = api_limit(db, "script-report", who, 6, &left); today = minute_ok ? day_hit(db, "script-report-day", who, &left_day) : 0; }
  int64_t code = wsp_report(stored, why_ok, minute_ok, today);
  if (code != 0) { refuse_code(q->fd, code, "", wsp_limit(code, false, true), wsp_lifts(code) ? (minute_ok ? left_day : left) : 0); return; }
  char at[24]; snprintf(at, sizeof at, "%lld", (long long)api_now_ms());
  const char *v[5] = {id, hash, who, why, at};
  if (!run(db, "INSERT INTO verse.script_reports (world, hash, reporter, why, at_ms) VALUES ($1, $2, $3, $4, $5::bigint)", 5, v) || !wsp_done(W(), cs("report"), cs(hash))) { world_refuse(q->fd, 500, "not saved"); return; }
  answer_out(q->fd, 200);
}

// /api/admin/scripts/...: the review queue and the switch, for a site admin.
bool scripts_admin_route(const request *q) {
  static const char root[] = "/api/admin/scripts/";
  if (strncmp(q->path, root, sizeof root - 1)) return false;
  const char *rest = q->path + sizeof root - 1;
  bool get = !strcmp(q->method, "GET"), post = !strcmp(q->method, "POST");
  PGconn *db = api_db();
  if (!db) { world_refuse(q->fd, 503, "no database"); return true; }
  char who[65];
  int denied = admin_who(q, db, who);
  if (denied) { world_refuse(q->fd, denied, denied == 403 ? "admins only" : "log in as an admin"); return true; }
  if (post && !auth_same_site(q)) { world_refuse(q->fd, 403, "cross-site"); return true; }
  if (get && !strcmp(rest, "reports")) {
    PGresult *r = PQexec(db, "SELECT id, world, hash, reporter, why, at_ms, status FROM verse.script_reports WHERE status = 'open' ORDER BY id LIMIT 200");
    if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("scripts: %s", PQerrorMessage(db)); PQclear(r); world_refuse(q->fd, 500, "not read"); return true; }
    int n = PQntuples(r);
    bool ok = wsp_reports_begin(W(), n);
    for (int i = 0; ok && i < n; i++)
      ok = wsp_reports_add(W(), atoll(PQgetvalue(r, i, 0)), cs(PQgetvalue(r, i, 1)), cs(PQgetvalue(r, i, 2)), cs(PQgetvalue(r, i, 3)), cs(PQgetvalue(r, i, 4)), atoll(PQgetvalue(r, i, 5)), cs(PQgetvalue(r, i, 6)), i == n - 1);
    PQclear(r);
    if (!ok || !wsp_reports_end(W())) { world_refuse(q->fd, 500, "not written"); return true; }
    answer_out(q->fd, 200);
    return true;
  }
  char hash[65] = "";
  const char *verb = "";
  if (strlen(rest) > 65 && rest[64] == '/') { memcpy(hash, rest, 64); hash[64] = 0; verb = rest + 65; }
  bool off = !strcmp(verb, "off"), on = !strcmp(verb, "on"), keep = !strcmp(verb, "keep");
  if (!post || !wsp_hash_ok(cs(hash)) || (!off && !on && !keep)) { world_refuse(q->fd, 404, "not found"); return true; }
  char why[200] = "", at[24];
  size_t n = q->body_len < sizeof why ? q->body_len : sizeof why - 1;
  memcpy(why, q->body, n); why[n] = 0;
  while (n > 0 && (why[n - 1] == '\n' || why[n - 1] == '\r' || why[n - 1] == ' ')) why[--n] = 0;
  if (off && (q->body_len >= sizeof why || !wsp_why_ok(cs(why)))) { refuse_code(q->fd, wsp_bad(), "say why in 1 to 160 plain letters, with no quote and no backslash", 0, 0); return true; }
  snprintf(at, sizeof at, "%lld", (long long)api_now_ms());
  const char *v[4] = {hash, why, at, who};
  bool ok;
  PQclear(PQexec(db, "BEGIN"));
  if (off) ok = run(db, "INSERT INTO verse.script_off (hash, why, at_ms, by) VALUES ($1, $2, $3::bigint, $4) ON CONFLICT (hash) DO UPDATE SET why = EXCLUDED.why, at_ms = EXCLUDED.at_ms, by = EXCLUDED.by", 4, v)
                 && run(db, "UPDATE verse.script_reports SET status = 'removed' WHERE hash = $1 AND status = 'open'", 1, v);
  else if (on) ok = run(db, "DELETE FROM verse.script_off WHERE hash = $1", 1, v);
  else ok = run(db, "UPDATE verse.script_reports SET status = 'kept' WHERE hash = $1 AND status = 'open'", 1, v);
  if (!ok || !run(db, "COMMIT", 0, NULL) || !wsp_done(W(), cs(verb), cs(hash))) { rollback(db); world_refuse(q->fd, 500, "not saved"); return true; }
  answer_out(q->fd, 200);
  return true;
}

// /api/worlds/<id>/scripts...: tail is what follows the world's ID. False when the path is not one of these.
bool scripts_route(const request *q, PGconn *db, const char *id, const char *tail) {
  if (strncmp(tail, "/scripts", 8) || (tail[8] && tail[8] != '/')) return false;
  bool get = !strcmp(q->method, "GET"), post = !strcmp(q->method, "POST");
  const char *rest = tail[8] ? tail + 9 : "";
  char hash[65] = "";
  const char *verb = "";
  if (strlen(rest) >= 64 && (rest[64] == 0 || rest[64] == '/')) { memcpy(hash, rest, 64); hash[64] = 0; verb = rest[64] ? rest + 65 : ""; }
  if (!*rest && get) script_list(q, db, id);
  else if (!*rest && post) script_put(q, db, id);
  else if (!strcmp(rest, "compile") && post) script_compile(q, db, id);
  else if (hash[0] && wsp_hash_ok(cs(hash)) && !*verb && get) script_get(q, db, id, hash);
  else if (hash[0] && wsp_hash_ok(cs(hash)) && !strcmp(verb, "source") && get) script_source(q, db, id, hash);
  else if (hash[0] && wsp_hash_ok(cs(hash)) && !strcmp(verb, "report") && post) script_report(q, db, id, hash);
  else world_refuse(q->fd, 404, "not found");
  return true;
}
