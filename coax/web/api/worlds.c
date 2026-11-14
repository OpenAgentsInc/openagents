// A world's document, read and edited by ID (GitHub #13412, the browser editor's E3;
// docs/editor/2026-11-12-browser-editor.md, docs/protocol/world-edit-v1.md). A world has a `dev`
// revision that edits change and a `live` revision a publish copies from it.
//
//   GET  /api/worlds/<id>?lane=dev|live          the lane's document and revisions (world.state.v1).
//        &engine=E&rules=R&content=C&wire=W      live only: the page's pins; it is given the live
//                                                revision its build reads, or 409 to reload
//   POST /api/worlds/<id>                        a world.edit.v1 batch made from the dev revision:
//                                                edits, undo, redo, original, publish-now or
//                                                publish-next. 409 on a stale revision; 422 with
//                                                the refusal in words when an edit may not stand
//        ?dry=1                                  the batch is carried out whole in a throwaway copy of
//                                                the world and nothing is saved: the answer is the
//                                                state as it stands with the batch as it would be taken
//   GET  /api/worlds/<id>/events?lane=L&after=N  the lane's saved events after revision N, a line each
//   GET  /api/worlds/<id>/card                   the world as a conversation shows it (world.card.v1): its name, the dev
//                                                revision and how many things it holds
//   GET  /api/worlds/<id>/content[?lane=dev]     the world's content tables (aethyr.content.v1): its own once a cell
//                                                was edited (dev: as they are edited; else as published), else the build's: what a
//                                                caller reads to aim an edit
//   GET  /api/worlds/<id>/assets[?lane=dev]      the world's imported assets and the limits (world.assets.v1): live's, or
//        &write=1                                dev's as they are edited. write=1: answered only to someone who may import
//   GET  /api/worlds/<id>/assets/pack[?lane=dev] the lane's import pack, a VPK1 file (none, 200: it has imported nothing)
//        ?live=SHA256                            the world's pack of that hash, whatever lane names it: what a tab asks for
//                                                after it is told of a new pack
//   POST /api/worlds/<id>/assets                 a file named before it is sent (world.asset.upload.v1): checked by name, size
//                                                and licence, then an upload is opened in the private bucket for exactly that
//                                                many bytes at PREFIX/worlds/<id>/files/SHA256 (drops.c), and its address is
//                                                answered; `kept` when the bucket has the file already
//   .../api/worlds/<id>/scripts...               the world's realm scripts: scripts.c (#13467). A publish here takes
//        POST /api/worlds/<id>?confirm=HASH,HASH    the hashes of the scripts the publisher confirms (409 lists them)
//   POST /api/worlds/<id>/assets/done            the same ask again, once the file is sent: the object is read back, its size and
//                                                SHA-256 are checked against its name (a wrong object is deleted), and it is
//                                                imported into dev's pack. The answer is the list with what it did, or 422 in words
//
// A realm with a board is a world of kind "realm" under its slug (#13422, migration 0185): its document is empty,
// /content is its board, its settings and its scripts as content tables, and a batch is switches of their `on` cells,
// carried out by the realm's own SQL (realm_post), and script rows, kept in the world's row (dev_tables) and held to
// scripts.c's rules (#13496). It has one lane, dev, and nothing to publish.
//
// This file moves facts in and answers out. The rules are Coax: who may write, the revision
// fence, the undo log and the dev and live rule are world-policy.coax's (#authority); whether an
// edit may stand is the world's own code, the code its page runs (Splice:
// metaverse/splice-bots/world-api.coax, linked as splice-world). Every byte this file answers is
// written by metaverse-engine/editor/world-edit.coax through world-policy.coax. SQL
// (migrations/0181_worlds.sql, 0183_world_assets.sql) stores the rows, holds the row lock for a write and keeps the
// outbox; it decides nothing. A saved edit is one outbox row, written in the transaction that
// changes the document (the domain event realm-authoring.world.edited.v1). A world is served only
// for a realm with a row in the authority table (metaverse-engine/editor/authority.coax, #13431):
// a write here is authoring, and nothing in this file reaches the character record or the ledger.
#include "api.h"
#include "world-policy.generated.h"
#include "splice-world.generated.h"

#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define WORLD_DOC 131072     // a document's bytes at most (metaverse/splice-bots/world.coax SPW_DOC)
#define WORLD_BATCH 65536    // a batch's (world-edit.coax WE_BATCH_BODY)
#define WORLD_TABLES 16384   // the content tables' bytes at most (metaverse/splice-bots/world.coax SPW_TABLES)
#define WORLD_EDIT 1400      // an edit's (WE_LINE)
#define WORLD_EVENTS 200     // events one read answers
#define WORLD_ASK 400        // an import's ask (asset-wire.coax: two lines)

static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_string csn(const char *s, size_t n) { return (coax_string){s, (int64_t)n}; }

static void refuse(int fd, int status, coax_string words, int edit) {
  char out[600];
  int64_t n = wr_refusal(words, edit, (coax_v_u8){(uint8_t *)out, sizeof out - 1});
  out[n < 0 ? 0 : n] = 0;
  api_json(fd, status, out);
}
static void refuse_verdict(int fd, int64_t verdict) { refuse(fd, (int)wr_status(verdict), wr_words(verdict), -1); }

// What a world's row holds. The pins are engine, rules, content, wire.
typedef struct {
  bool exists;
  char kind[24];
  long long dev_rev, done, steps, live_rev, live_from, prev_rev;
  int live_when;
  long long live_pins[4], prev_pins[4];
  char *dev_doc, *live_doc, *prev_doc;   // malloc'd; "" for none
  char *dev_tables, *live_tables, *prev_tables;   // the lane's content tables; "" for the ones the build shipped with
  char dev_pack[65], live_pack[65], prev_pack[65];   // the lane's import pack, by its SHA-256; "" for none (0183)
} world_row;

static void row_free(world_row *w) { free(w->dev_doc); free(w->live_doc); free(w->prev_doc); free(w->dev_tables); free(w->live_tables); free(w->prev_tables); }
static bool row_read(PGconn *db, const char *id, bool lock, world_row *w) {
  memset(w, 0, sizeof *w);
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, lock
    ? "SELECT kind, dev_rev, dev_doc, done, steps, live_rev, live_from, live_doc, live_when, live_engine, live_rules, live_content, live_wire, prev_rev, prev_doc, prev_engine, prev_rules, prev_content, prev_wire, dev_tables, live_tables, prev_tables, dev_pack, live_pack, prev_pack FROM verse.world_docs WHERE id = $1 FOR UPDATE"
    : "SELECT kind, dev_rev, dev_doc, done, steps, live_rev, live_from, live_doc, live_when, live_engine, live_rules, live_content, live_wire, prev_rev, prev_doc, prev_engine, prev_rules, prev_content, prev_wire, dev_tables, live_tables, prev_tables, dev_pack, live_pack, prev_pack FROM verse.world_docs WHERE id = $1",
    1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("worlds: %s", PQerrorMessage(db)); PQclear(r); return false; }
  if (PQntuples(r) == 1) {
    w->exists = true;
    snprintf(w->kind, sizeof w->kind, "%s", PQgetvalue(r, 0, 0));
    w->dev_rev = atoll(PQgetvalue(r, 0, 1)); w->dev_doc = strdup(PQgetvalue(r, 0, 2));
    w->done = atoll(PQgetvalue(r, 0, 3)); w->steps = atoll(PQgetvalue(r, 0, 4));
    w->live_rev = atoll(PQgetvalue(r, 0, 5)); w->live_from = atoll(PQgetvalue(r, 0, 6)); w->live_doc = strdup(PQgetvalue(r, 0, 7));
    w->live_when = atoi(PQgetvalue(r, 0, 8));
    for (int k = 0; k < 4; k++) { w->live_pins[k] = atoll(PQgetvalue(r, 0, 9 + k)); w->prev_pins[k] = atoll(PQgetvalue(r, 0, 15 + k)); }
    w->prev_rev = atoll(PQgetvalue(r, 0, 13)); w->prev_doc = strdup(PQgetvalue(r, 0, 14));
    w->dev_tables = strdup(PQgetvalue(r, 0, 19)); w->live_tables = strdup(PQgetvalue(r, 0, 20)); w->prev_tables = strdup(PQgetvalue(r, 0, 21));
    snprintf(w->dev_pack, sizeof w->dev_pack, "%s", PQgetvalue(r, 0, 22)); snprintf(w->live_pack, sizeof w->live_pack, "%s", PQgetvalue(r, 0, 23)); snprintf(w->prev_pack, sizeof w->prev_pack, "%s", PQgetvalue(r, 0, 24));
  }
  PQclear(r);
  return true;
}
static bool run(PGconn *db, const char *sql, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK || PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("worlds: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static void rollback(PGconn *db) { PQclear(PQexec(db, "ROLLBACK")); }
void world_refuse(int fd, int status, const char *words) { refuse(fd, status, cs(words), -1); }
// A world's import pack of this hash: malloc'd, its length in *n. "" names no pack: an empty one, *n 0. NULL when it can't be read.
static char *pack_read(PGconn *db, const char *id, const char *hash, size_t *n) {
  *n = 0;
  if (!*hash) return calloc(1, 1);
  const char *v[2] = {id, hash};
  PGresult *r = PQexecParams(db, "SELECT bytes FROM verse.world_packs WHERE world = $1 AND hash = $2", 2, NULL, v, NULL, NULL, 1);
  char *out = NULL;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && (out = malloc((size_t)PQgetlength(r, 0, 0) + 1))) {
    *n = (size_t)PQgetlength(r, 0, 0);
    memcpy(out, PQgetvalue(r, 0, 0), *n); out[*n] = 0;
  } else api_log("worlds: %s: its pack is not read: %s", id, PQerrorMessage(db));
  PQclear(r);
  return out;
}

// ---- the world's own Coax, by kind. Splice is the first world; a realm (made, or hand-built with a board) is the
// second (#13422: metaverse/edit/realm-world.coax, in world-policy's unit). A kind this build has no code for is not served.
static bool kind_known(const char *kind) { return !strcmp(kind, "splice-bots") || !strcmp(kind, "realm"); }
// A realm's world has no build pins: its switches are the realm's rows, which every build reads.
static bool kind_pins(const char *kind, long long pins[4]) {
  int64_t p[4] = {0};
  if (!strcmp(kind, "realm")) { for (int k = 0; k < 4; k++) pins[k] = 0; return true; }
  if (strcmp(kind, "splice-bots") || !splice_world_pins((coax_v_s64){p, 4})) return false;
  for (int k = 0; k < 4; k++) pins[k] = p[k];
  return true;
}
// The document a world is before its first edit, into out (malloc'd by the caller, WORLD_DOC): its length, or -1.
static int64_t kind_empty(const char *kind, char *out) {
  if (!strcmp(kind, "realm")) return 0;   // a realm's world places nothing: its state is its tables (/content)
  if (strcmp(kind, "splice-bots")) return -1;
  return splice_world_empty((coax_v_u8){(uint8_t *)out, WORLD_DOC});
}
// The lane's document as stored, or the empty one: malloc'd, its length in *n; NULL when it can't be made.
static char *doc_of(const char *kind, const char *stored, size_t *n) {
  if (stored && *stored) { *n = strlen(stored); return strdup(stored); }
  char *out = malloc(WORLD_DOC + 1);
  int64_t made = out ? kind_empty(kind, out) : -1;
  if (made < 0) { free(out); return NULL; }
  out[made] = 0; *n = (size_t)made;
  return out;
}

// Who asks: the player key the realm routes know them by (X-Verse-Player; a NIP-98 caller's is its account's,
// auth.caller_key, as realms.c reads it), whether they are a site admin, and whether they are logged in or signed.
typedef struct { char key[160]; bool admin, signed_in, player; } world_asker;
static bool who_asks(const request *q, PGconn *db, world_asker *a) {
  memset(a, 0, sizeof *a);
  const auth_principal *p = auth_principal_of(q);
  if (!p) return true;
  a->admin = p->admin; a->signed_in = p->account[0] || p->pubkey[0];
  snprintf(a->key, sizeof a->key, "%s", p->player);
  if (p->credential == AUTH_NOSTR && p->pubkey[0] && p->event_id[0] && !access_caller_key(db, p->pubkey, a->key)) return false;
#ifdef OA_LOCAL_SERVE
  // The dev server's local admin (auth.c local_admin) sends no player key: it is its account key's, as a signed
  // caller's is, so the side and a CLI on this computer are one account (realms.c reads it the same way, #13493).
  if (!a->key[0] && p->pubkey[0] && !access_caller_key(db, p->pubkey, a->key)) return false;
#endif
  a->player = a->key[0] != 0;
  return true;
}
// The asker's role from the five API_REALM_ROLE_FACTS columns of row k from column at, in verse.edit_role's words
// ("" for none): world-policy.coax wr_edit_role.
static coax_string role_of(PGresult *r, int k, int at, bool site_admin) {
  bool f[5];
  for (int i = 0; i < 5; i++) f[i] = !strcmp(PQgetvalue(r, k, at + i), "t");
  return wr_edit_role(f[0], f[1], f[2], site_admin, f[3], f[4]);
}
// A realm's facts for the asker (migrations/0050, 0099): whether it is hidden from them, their role ("" for none) and
// its spec as its world reads it (malloc'd; NULL when it is no realm; 0190's realm_world_spec: what it places too).
// False on a database error.
static bool realm_facts(PGconn *db, const char *id, const world_asker *a, bool *hidden, char role[16], char **spec) {
  const char *v[3] = {id, a->key, a->admin ? "true" : "false"};
  PGresult *r = PQexecParams(db, "SELECT verse.realm_hidden($1, $2, $3::boolean), " API_REALM_ROLE_FACTS("$1", "$2") ", "
    "verse.realm_world_spec($1, $2, $3::boolean) FROM " API_REALM_ROLE_FROM("$1", "$2"), 3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  *hidden = false; role[0] = 0; *spec = NULL;
  if (ok) {
    *hidden = !strcmp(PQgetvalue(r, 0, 0), "t");
    coax_string w = role_of(r, 0, 1, a->admin);
    snprintf(role, 16, "%.*s", (int)w.count, w.data);
    if (!PQgetisnull(r, 0, 6)) *spec = strdup(PQgetvalue(r, 0, 6));
  } else api_log("worlds: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
// Whether a world is served to the asker: its kind has code here and a row of the authority table, and a realm is
// not hidden from them.
static bool world_shown(PGconn *db, const request *q, const char *id, const world_row *w) {
  if (!w->exists || !kind_known(w->kind) || !wr_loads(cs(id), cs(w->kind))) return false;
  if (!wr_at_once(cs(w->kind))) return true;
  world_asker a; bool hidden = true; char role[16]; char *spec = NULL;
  bool ok = who_asks(q, db, &a) && realm_facts(db, id, &a, &hidden, role, &spec);
  bool shown = ok && !hidden && spec;
  free(spec);
  return shown;
}
// A lane's content tables as text: the world's own, or the ones the build shipped with. malloc'd; NULL when they can't be made.
static char *tables_of(const char *kind, const char *stored) {
  char *out = strcmp(kind, "splice-bots") ? NULL : malloc(WORLD_DOC + 1);
  int64_t n = out ? splice_world_tables(cs(stored ? stored : ""), (coax_v_u8){(uint8_t *)out, WORLD_DOC}) : -1;
  if (n < 0) { free(out); return NULL; }
  out[n] = 0;
  return out;
}
// A realm's scripts document as its row keeps it (dev_tables; realm-world.coax): malloc'd, the empty one for none.
static char *realm_scripts_of(const char *stored) {
  coax_string none = wr_realm_scripts_none();
  return stored && *stored ? strdup(stored) : strndup(none.data, (size_t)none.count);
}
bool world_scripts_world(PGconn *db, const char *id, bool lock, char *tables[3]) {
  world_row w;
  if (tables) tables[0] = tables[1] = tables[2] = NULL;
  if (!wr_world_ok(cs(id)) || !row_read(db, id, lock, &w)) return false;
  // scripts run where a world's content tables have a scripts table: Splice's, and a realm's (#13496), whose one
  // lane is its dev: its row keeps the scripts table alone
  bool realm = w.exists && wr_at_once(cs(w.kind));
  bool ok = w.exists && (!strcmp(w.kind, "splice-bots") || realm) && wr_loads(cs(id), cs(w.kind));
  if (ok && tables && realm) tables[0] = realm_scripts_of(w.dev_tables);
  else if (ok && tables) {
    tables[0] = tables_of(w.kind, w.dev_tables);
    if (w.live_rev > 0) tables[1] = tables_of(w.kind, w.live_tables);
    if (w.prev_rev > 0) tables[2] = tables_of(w.kind, w.prev_tables);
  }
  row_free(&w);
  return ok;
}
// Who asks about a world's scripts (scripts.c, #13496): whether it is a world that runs scripts and is shown to them
// (a realm hidden from them is not), and their role in it in verse.edit_role's words ("" for none), the role worlds
// are written by. False on a database error.
bool world_scripts_asker(const request *q, PGconn *db, const char *id, bool *shown, char role[16]) {
  *shown = false; role[0] = 0;
  world_row w;
  if (!wr_world_ok(cs(id))) return true;
  if (!row_read(db, id, false, &w)) return false;
  bool runs = w.exists && (!strcmp(w.kind, "splice-bots") || wr_at_once(cs(w.kind))) && wr_loads(cs(id), cs(w.kind)), ok = true;
  if (runs) {
    world_asker a; bool hidden = true; char *spec = NULL;
    ok = who_asks(q, db, &a) && realm_facts(db, id, &a, &hidden, role, &spec);
    // a realm's world is shown to whoever its realm is not hidden from; Splice's to everyone
    *shown = ok && (!wr_at_once(cs(w.kind)) || (!hidden && spec));
    free(spec);
  }
  row_free(&w);
  return ok;
}

// pins: the build that reads the revision answered (engine, rules, content, wire), so an answer names what it touched (#13437).
static void answer_state(const request *q, const char *id, int lane, long long rev, const long long pins[4], const world_row *w, long long dev, long long live, long long done, long long steps,
                         const char *doc, size_t doc_n, const char *did, size_t did_n) {
  size_t cap = doc_n * 2 + did_n + 600;
  char *out = malloc(cap + 1);
  int64_t n = out ? wr_state(cs(id), lane, rev, pins[0], pins[1], pins[2], pins[3], dev, live, w->live_when, done, steps - done, csn(doc, doc_n), csn(did, did_n), (coax_v_u8){(uint8_t *)out, cap}) : -1;
  if (n < 0) { free(out); refuse(q->fd, 500, cs("not written"), -1); return; }
  out[n] = 0;
  api_json(q->fd, 200, out);
  free(out);
}

static void world_get(const request *q, PGconn *db, const char *id) {
  world_row w;
  if (!row_read(db, id, false, &w)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  char lane_s[16] = "", pin[4][24];
  api_query(q, "lane", lane_s, sizeof lane_s);
  static const char *pin_names[4] = {"engine", "rules", "content", "wire"};
  bool sent = true; long long mine[4] = {0};
  for (int k = 0; k < 4; k++) { if (!api_query(q, pin_names[k], pin[k], sizeof pin[k]) || !pin[k][0]) sent = false; else mine[k] = atoll(pin[k]); }
  int64_t lane = w.exists && wr_at_once(cs(w.kind)) ? wr_lane(cs("dev")) : wr_lane(cs(lane_s));   // a realm's world: one lane
  int64_t join = wr_join(sent, mine[0], mine[1], mine[2], mine[3], w.exists && w.live_rev > 0, w.live_pins[0], w.live_pins[1], w.live_pins[2], w.live_pins[3],
                         w.exists && w.prev_rev > 0, w.prev_pins[0], w.prev_pins[1], w.prev_pins[2], w.prev_pins[3]);
  int64_t verdict = wr_read(world_shown(db, q, id, &w), lane, join);
  if (verdict != 0) { refuse_verdict(q->fd, verdict); row_free(&w); return; }
  bool dev = wr_is_dev(lane), previous = !dev && wr_previous(join);
  size_t n = 0;
  char *doc = doc_of(w.kind, dev ? w.dev_doc : (previous ? w.prev_doc : w.live_doc), &n);
  if (!doc) { refuse(q->fd, 500, cs("not read"), -1); row_free(&w); return; }
  long long here[4] = {0, 0, 0, 0};
  kind_pins(w.kind, here);
  answer_state(q, id, (int)lane, dev ? w.dev_rev : (previous ? w.prev_rev : w.live_rev), dev ? here : (previous ? w.prev_pins : w.live_pins), &w, w.dev_rev, w.live_rev, w.done, w.steps, doc, n, "", 0);
  free(doc); row_free(&w);
}

static void world_events(const request *q, PGconn *db, const char *id) {
  char lane_s[16] = "", after[24] = "0", count[8];
  world_row w;
  if (!row_read(db, id, false, &w)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  bool hidden = w.exists && wr_at_once(cs(w.kind)) && !world_shown(db, q, id, &w);   // a realm hidden from the asker has none
  row_free(&w);
  if (hidden) { refuse_verdict(q->fd, wr_read(false, 0, 0)); return; }
  api_query(q, "lane", lane_s, sizeof lane_s); api_query(q, "after", after, sizeof after);
  int64_t lane = wr_lane(cs(lane_s));
  int64_t verdict = wr_read(true, lane, wr_join(false, 0, 0, 0, 0, true, 0, 0, 0, 0, false, 0, 0, 0, 0));   // any lane that is one
  if (verdict != 0) { refuse_verdict(q->fd, verdict); return; }
  for (const char *c = after; *c; c++) if (*c < '0' || *c > '9') { refuse(q->fd, 400, cs("after is a revision"), -1); return; }
  snprintf(count, sizeof count, "%d", WORLD_EVENTS);
  const char *v[4] = {id, wr_is_dev(lane) ? "dev" : "live", after[0] ? after : "0", count};
  PGresult *r = PQexecParams(db, "SELECT body FROM verse.world_outbox_since($1, $2, $3::bigint, $4::int)", 4, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("worlds: %s", PQerrorMessage(db)); PQclear(r); refuse(q->fd, 500, cs("not read"), -1); return; }
  size_t total = 1;
  for (int i = 0; i < PQntuples(r); i++) total += strlen(PQgetvalue(r, i, 0)) + 1;
  char *out = malloc(total); size_t n = 0;
  for (int i = 0; out && i < PQntuples(r); i++) { size_t l = strlen(PQgetvalue(r, i, 0)); memcpy(out + n, PQgetvalue(r, i, 0), l); n += l; out[n++] = '\n'; }
  PQclear(r);
  if (!out) { refuse(q->fd, 500, cs("not read"), -1); return; }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, n);
  free(out);
}

// The next number of the outbox, into seq.
static bool next_seq(PGconn *db, char seq[24]) {
  PGresult *r = PQexec(db, "SELECT nextval(pg_get_serial_sequence('verse.world_outbox', 'seq'))");
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) snprintf(seq, 24, "%s", PQgetvalue(r, 0, 0)); else api_log("worlds: %s", PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static bool outbox(PGconn *db, const char *seq, coax_string event_id, const char *id, const char *lane, long long rev, const char *body, long long now) {
  char rs[24], ms[24]; snprintf(rs, sizeof rs, "%lld", rev); snprintf(ms, sizeof ms, "%lld", now);
  char event[80]; snprintf(event, sizeof event, "%.*s", (int)event_id.count, event_id.data);
  const char *v[7] = {seq, event, id, lane, rs, body, ms};
  return run(db, "INSERT INTO verse.world_outbox (seq, event, world, lane, rev, body, at_ms) VALUES ($1::bigint, $2, $3, $4, $5::bigint, $6, $7::bigint)", 7, v);
}

// A publish: dev becomes live. The row is locked; the caller's transaction is open.
static void world_publish(const request *q, PGconn *db, const char *id, world_row *w, int64_t what, long long base) {
  long long pins[4];
  if (!kind_pins(w->kind, pins)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); return; }
  bool has_live = w->live_rev > 0;
  int64_t when = wr_when(what);
  int64_t v = wr_publish(when, base, w->dev_rev, w->live_from, pins[0], pins[1], pins[2], pins[3], has_live, w->live_pins[0], w->live_pins[1], w->live_pins[2], w->live_pins[3]);
  if (v != 0) { rollback(db); refuse(q->fd, (int)wr_publish_status(v), wr_publish_words(v), -1); return; }
  if (!strcmp(w->kind, "splice-bots")) {
    // the scripts dev's rows name: one that is new, or does more than live's, needs the publisher's confirmation (#13467)
    char *dev_t = tables_of(w->kind, w->dev_tables), *live_t = has_live ? tables_of(w->kind, w->live_tables) : NULL;
    bool ok = dev_t && (!has_live || live_t);
    if (!ok) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); }
    else if (!(ok = scripts_publish_ok(q, db, id, dev_t, live_t))) rollback(db);
    free(dev_t); free(live_t);
    if (!ok) return;
  }
  bool keeps = wr_keeps(has_live, pins[0], pins[1], pins[2], pins[3], w->live_pins[0], w->live_pins[1], w->live_pins[2], w->live_pins[3]);
  size_t n = 0;
  char *doc = doc_of(w->kind, w->dev_doc, &n);
  char seq[24], event[400];
  long long now = (long long)api_now_ms(), live = w->live_rev + 1;
  if (!doc || !next_seq(db, seq)) { free(doc); rollback(db); refuse(q->fd, 500, cs("not saved"), -1); return; }
  int64_t en = wr_published(atoll(seq), cs(id), live, w->dev_rev, when, pins[0], pins[1], pins[2], pins[3], now, (coax_v_u8){(uint8_t *)event, sizeof event - 1});
  if (en < 0) { free(doc); rollback(db); refuse(q->fd, 500, cs("not saved"), -1); return; }
  event[en] = 0;
  char rev_s[24], from_s[24], when_s[8], p[4][24];
  snprintf(rev_s, sizeof rev_s, "%lld", live); snprintf(from_s, sizeof from_s, "%lld", w->dev_rev); snprintf(when_s, sizeof when_s, "%d", (int)when);
  for (int k = 0; k < 4; k++) snprintf(p[k], sizeof p[k], "%lld", pins[k]);
  const char *sv[10] = {id, rev_s, from_s, doc, when_s, p[0], p[1], p[2], p[3], w->dev_tables ? w->dev_tables : ""};
  // the revision before a pin change stays for one release (world-live.coax); a publish on the same pins leaves it.
  // The lane's import pack goes with its document: live's is dev's from here on (0183)
  bool ok = (!keeps || run(db, "UPDATE verse.world_docs SET prev_rev = live_rev, prev_doc = live_doc, prev_engine = live_engine, prev_rules = live_rules, prev_content = live_content, prev_wire = live_wire, prev_tables = live_tables, prev_pack = live_pack WHERE id = $1", 1, sv))
    && run(db, "UPDATE verse.world_docs SET live_rev = $2::bigint, live_from = $3::bigint, live_doc = $4, live_when = $5::int, live_engine = $6::int, live_rules = $7::int, live_content = $8::int, live_wire = $9::int, live_tables = $10, live_pack = dev_pack WHERE id = $1", 10, sv)
    && outbox(db, seq, wr_event_id(true), id, "live", live, event, now)
    && run(db, "COMMIT", 0, NULL);
  if (!ok) { free(doc); rollback(db); refuse(q->fd, 500, cs("not saved"), -1); return; }
  w->live_when = (int)when;
  answer_state(q, id, (int)wr_lane(cs("live")), live, pins, w, w->dev_rev, live, w->done, w->steps, "", 0, "", 0);
  free(doc);
}

// The batch a write carries out: the caller's own, or a stored step taken again (an undo its inverse, a redo its
// batch), malloc'd; fenced is false for a stored step. NULL when it can't be read.
static char *batch_text(const request *q, PGconn *db, const char *id, int64_t what, const world_row *w, int64_t *step, size_t *n, bool *fenced) {
  char *text = NULL;
  *step = wr_step(what, w->done); *n = 0; *fenced = *step < 0;
  if (*step >= 0) {
    char ns[24]; snprintf(ns, sizeof ns, "%lld", (long long)*step);
    const char *v[2] = {id, ns};
    PGresult *r = PQexecParams(db, wr_backward(what) ? "SELECT inv FROM verse.world_steps WHERE world = $1 AND n = $2::int" : "SELECT did FROM verse.world_steps WHERE world = $1 AND n = $2::int", 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { text = strdup(PQgetvalue(r, 0, 0)); *n = strlen(text); }
    PQclear(r);
  } else if ((text = malloc(q->body_len + 1))) { memcpy(text, q->body, q->body_len); text[q->body_len] = 0; *n = q->body_len; }
  return text;
}
// The `at` a batch's events carry: now, or one millisecond after the last batch's when that is not
// earlier, so each step of a world has an `at` of its own and a reader finds a step where `at` changes
// (docs/protocol/world-edit-v1.md "Steps", #13485). The row is locked, so no other batch is between.
static long long step_at(PGconn *db, const char *id, long long rev) {
  long long now = (long long)api_now_ms();
  if (rev < 1) return now;
  char rs[24]; snprintf(rs, sizeof rs, "%lld", rev);
  const char *v[2] = {id, rs};
  PGresult *r = PQexecParams(db, "SELECT at_ms FROM verse.world_outbox WHERE world = $1 AND lane = 'dev' AND rev = $2::bigint", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { long long last = atoll(PQgetvalue(r, 0, 0)); if (now <= last) now = last + 1; }
  PQclear(r);
  return now;
}
// A batch carried out is saved: the dev revision and where the undo log stands, a new step where it stands (the steps
// an undo left ahead of it go), and one outbox event a saved edit, then COMMIT. The transaction is open; false when
// anything failed (the caller rolls back). *rev, *done and *steps come back as they now stand.
static bool batch_save(PGconn *db, const char *id, const world_row *w, int64_t what, int64_t step, const char *doc, const char *did, size_t did_n, const char *inv,
                       int64_t edits, long long *rev, long long *done, long long *steps) {
  *rev = w->dev_rev + edits; *done = wr_done(what, w->done); *steps = wr_steps(what, w->done, w->steps);
  long long now = step_at(db, id, w->dev_rev);
  char rev_s[24], done_s[24], steps_s[24], at_s[24];
  snprintf(rev_s, sizeof rev_s, "%lld", *rev); snprintf(done_s, sizeof done_s, "%lld", *done); snprintf(steps_s, sizeof steps_s, "%lld", *steps); snprintf(at_s, sizeof at_s, "%lld", w->done);
  const char *uv[5] = {id, doc, rev_s, done_s, steps_s};
  bool ok = run(db, "UPDATE verse.world_docs SET dev_doc = $2, dev_rev = $3::bigint, done = $4::int, steps = $5::int WHERE id = $1", 5, uv);
  if (ok && step < 0) {
    const char *sv[4] = {id, at_s, did, inv};
    ok = run(db, "DELETE FROM verse.world_steps WHERE world = $1 AND n >= $2::int", 2, sv)
      && run(db, "INSERT INTO verse.world_steps (world, n, did, inv) VALUES ($1, $2::int, $3, $4)", 4, sv);
  }
  // one event a saved edit, in the transaction that changes the document
  for (int64_t i = 0; ok && i < edits; i++) {
    char edit[WORLD_EDIT + 1], event[WORLD_EDIT + 240], seq[24];
    int64_t en = wr_batch_edit(csn(did, did_n), i, (coax_v_u8){(uint8_t *)edit, WORLD_EDIT});
    ok = en > 0 && next_seq(db, seq);
    if (!ok) break;
    int64_t vn = wr_event(atoll(seq), cs(id), w->dev_rev + 1 + i, csn(edit, (size_t)en), now, (coax_v_u8){(uint8_t *)event, sizeof event - 1});
    ok = vn > 0;
    if (ok) { event[vn] = 0; ok = outbox(db, seq, wr_event_id(false), id, "dev", w->dev_rev + 1 + i, event, now); }
  }
  return ok && run(db, "COMMIT", 0, NULL);
}

// A realm's world (#13422): the batch is the realm's switches and places, checked on its board by
// metaverse/edit/realm-world.coax and carried out one by one by the realm's own SQL (verse.edit_switch,
// verse.edit_settings, and for its characters' lore and quests verse.edit_lore and verse.edit_quest, #13526), whose rules and words are the ones the realm's page meets, and its places written by
// 0193's verse.realm_thing_add and verse.realm_thing_mark (their rules are realm-place-policy.coax's, asked in
// wr_realm_batch); a refusal takes the whole batch back. While it runs, verse.world_edit is on, so a made
// realm's own undo log takes no step: the world's is the one log (#13506). The row is locked; the transaction is open.
// given: a batch the server made (the owner's yes to proposal `pid`, #13515), carried out as the caller's own and
// answered {state: accepted}; NULL for the request's own batch.
static void realm_post(const request *q, PGconn *db, const char *id, world_row *w, int64_t what, const world_asker *a, const char *given, const char *pid) {
  int64_t step = -1; size_t text_n = 0; bool fenced = true;
  char *text = given ? strdup(given) : batch_text(q, db, id, what, w, &step, &text_n, &fenced);
  if (given && text) text_n = strlen(text);
  bool hidden = true; char role[16]; char *spec = NULL;
  char *did = malloc(WORLD_BATCH + 1), *inv = malloc(WORLD_BATCH + 1), *acts = malloc(WORLD_BATCH + 1), *scripts = malloc(WORLD_TABLES + 1);
  char *was = realm_scripts_of(w->dev_tables);
  char why[240]; int64_t n[7] = {0, 0, 0, 0, 0, -1, 0};
  if (!text || !did || !inv || !acts || !scripts || !was || !realm_facts(db, id, a, &hidden, role, &spec)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); goto done; }
  if (hidden || !spec) { rollback(db); refuse_verdict(q->fd, wr_read(false, 0, 0)); goto done; }
  int64_t status = wr_realm_batch(cs(spec), cs(was), csn(text, text_n), fenced, (coax_v_u8){(uint8_t *)did, WORLD_BATCH}, (coax_v_u8){(uint8_t *)inv, WORLD_BATCH},
                                  (coax_v_u8){(uint8_t *)acts, WORLD_BATCH}, (coax_v_u8){(uint8_t *)why, sizeof why}, (coax_v_u8){(uint8_t *)scripts, WORLD_TABLES}, (coax_v_s64){n, 7});
  if (status != 0) { rollback(db); refuse(q->fd, (int)wr_realm_status(status), n[3] > 0 ? csn(why, (size_t)n[3]) : wr_realm_words(status), (int)n[5]); goto done; }
  did[n[0]] = 0; inv[n[1]] = 0; acts[n[2]] = 0; scripts[n[6]] = 0;
  if (n[6] > 0) {
    // the batch changed the realm's script rows (#13496): a row it newly names must name a module the world stores that
    // is not switched off, and since what the batch leaves is what people get at once, a row that is new or does more
    // than the one it replaces needs its maker's yes by hash, as a publish does (script-policy.coax). An undo or a redo
    // takes the realm back to rows its maker said yes to, and is not asked again.
    char said[240];
    if (!scripts_edit_ok(db, id, was, scripts, said, sizeof said)) { rollback(db); refuse(q->fd, 422, cs(said), -1); goto done; }
    if (fenced && !scripts_publish_ok(q, db, id, scripts, was)) { rollback(db); goto done; }
  }
  // each switch and place by the realm's own SQL, in order: "switch KEY 0|1", "tool NAME 0|1", "place ID THING X Z YAW",
  // "spawn ID THING X Z YAW", "despawn ID", "lore WHO|TEXT" or "quest GIVER|ASK|STEP|STEP|STEP|REWARD" (realm-world.coax rw_batch). A thing placed must get the ID the batch as
  // taken gave it, or the realm changed since the board was read: the batch is refused and the page reads it again.
  if (!run(db, "SELECT set_config('verse.world_edit', 'on', true)", 0, NULL)) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); goto done; }
  int edit = 0;
  for (char *line = acts; *line; edit++) {
    char *end = strchr(line, '\n'); if (!end) break;
    *end = 0;
    if (!strncmp(line, "lore ", 5) || !strncmp(line, "quest ", 6)) {
      // "lore WHO|TEXT" or "quest GIVER|ASK|STEP|STEP|STEP|REWARD" (#13526): carried out by verse.edit_lore or
      // verse.edit_quest, which say who may; an empty TEXT forgets the lore and an empty ASK takes the quest away
      bool lore = line[0] == 'l';
      char *f[6] = {0}; int nf = 0;
      for (char *p = line + (lore ? 5 : 6); nf < 6; nf++) { f[nf] = p; char *bar = strchr(p, '|'); if (!bar) { nf++; break; } *bar = 0; p = bar + 1; }
      if (nf != (lore ? 2 : 6)) { rollback(db); refuse(q->fd, 500, cs("not read"), edit); goto done; }
      const char *lv[5] = {id, f[0], f[1], a->key, a->admin ? "true" : "false"};
      const char *qv[9] = {id, f[0], f[1], f[2], f[3], f[4], f[5], a->key, a->admin ? "true" : "false"};
      PGresult *r = lore
        ? PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'error', '') FROM verse.edit_lore($1, jsonb_build_object('who', $2::text, 'text', $3::text), $4, $5::boolean) x", 5, NULL, lv, NULL, NULL, 0)
        : PQexecParams(db, "SELECT (x->>'status')::int, coalesce(x->>'error', '') FROM verse.edit_quest($1, jsonb_build_object('giver', $2::text, 'ask', $3::text, "
                           "'steps', jsonb_build_array($4::text, $5::text, $6::text), 'reward', $7::text), $8, $9::boolean) x", 9, NULL, qv, NULL, NULL, 0);
      bool read = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
      int http = read ? atoi(PQgetvalue(r, 0, 0)) : 500;
      char said[200]; snprintf(said, sizeof said, "%s", read ? PQgetvalue(r, 0, 1) : "not saved");
      if (!read) api_log("worlds: %s", PQerrorMessage(db));
      PQclear(r);
      if (http != 200) { rollback(db); refuse(q->fd, http, cs(said[0] ? said : "not saved"), edit); goto done; }
      line = end + 1;
      continue;
    }
    char verb[8] = "", name[64] = "", on[2] = "", num[12] = "", xs[12] = "", zs[12] = "", ys[12] = "";
    bool place = !strncmp(line, "place ", 6) || !strncmp(line, "spawn ", 6), gone = !strncmp(line, "despawn ", 8);
    int got = place ? sscanf(line, "%7s %11s %63s %11s %11s %11s", verb, num, name, xs, zs, ys) : gone ? sscanf(line, "%7s %11s", verb, num) : sscanf(line, "%7s %63s %1s", verb, name, on);
    if (got != (place ? 6 : gone ? 2 : 3)) { rollback(db); refuse(q->fd, 500, cs("not read"), edit); goto done; }
    const char *v[8] = {id, name, !strcmp(on, "1") ? "true" : "false", a->key, a->admin ? "true" : "false"};
    const char *sql = !strcmp(verb, "switch")
      ? "SELECT (x->>'status')::int, coalesce(x->>'error', ''), '' FROM verse.edit_switch($1, jsonb_build_object('row', $2::text, 'on', $3::boolean), $4, $5::boolean) x"
      : "SELECT (x->>'status')::int, coalesce(x->>'error', ''), '' FROM verse.edit_settings($1, jsonb_build_object('tools', jsonb_build_object($2::text, $3::boolean)), $4, $5::boolean) x";
    int nv = 5;
    // a place's rules were realm-place-policy.coax's, in rw_batch; SQL only writes the row (0193), and answers the
    // thing's ID: 0 when the list has no such thing, which the batch, as taken, said it had
    if (!strcmp(verb, "place")) {
      const char *pv[6] = {id, name, xs, zs, ys, a->key};
      memcpy(v, pv, sizeof pv); nv = 6;
      sql = "SELECT CASE WHEN x > 0 THEN 200 ELSE 409 END, '', x::text FROM verse.realm_thing_add($1, $2, $3::int, $4::int, $5::int, $6) x";
    } else if (place || gone) {
      const char *dv[3] = {id, num, gone ? "true" : "false"};
      memcpy(v, dv, sizeof dv); nv = 3;
      sql = "SELECT CASE WHEN x > 0 THEN 200 ELSE 409 END, '', x::text FROM verse.realm_thing_mark($1, $2::int, $3::boolean) x";
    }
    PGresult *r = PQexecParams(db, sql, nv, NULL, v, NULL, NULL, 0);
    bool read = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
    int http = read ? atoi(PQgetvalue(r, 0, 0)) : 500;
    char said[200]; snprintf(said, sizeof said, "%s", read ? PQgetvalue(r, 0, 1) : "not saved");
    bool numbered = !read || !(place || gone) || !strcmp(PQgetvalue(r, 0, 2), num);
    if (!read) api_log("worlds: %s", PQerrorMessage(db));
    PQclear(r);
    if (http != 200 && !(place || gone)) { rollback(db); refuse(q->fd, http, cs(said[0] ? said : "not saved"), edit); goto done; }
    if (!numbered) { rollback(db); refuse(q->fd, 409, cs("the realm changed since: read it again"), edit); goto done; }
    line = end + 1;
  }
  if (pid) {
    // the proposal the batch carries out is settled in the same transaction: accepted once
    const char *pv[2] = {id, pid};
    PGresult *r = PQexecParams(db, "SELECT verse.realm_proposal_settle($1, $2::bigint)", 2, NULL, pv, NULL, NULL, 0);
    bool settled = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && atoi(PQgetvalue(r, 0, 0)) == 1;
    PQclear(r);
    if (!settled) { rollback(db); refuse(q->fd, 409, cs("that card is decided already"), -1); goto done; }
  }
  char dry[4] = "";
  if (!pid && api_query(q, "dry", dry, sizeof dry) && dry[0] == '1') {
    // a dry run: the realm's SQL carried it out in the transaction, which is taken back; nothing is saved
    rollback(db);
    long long none[4] = {0, 0, 0, 0};
    answer_state(q, id, (int)wr_lane(cs("dev")), w->dev_rev, none, w, w->dev_rev, w->live_rev, w->done, w->steps, "", 0, did, (size_t)n[0]);
    goto done;
  }
  long long rev, done_at, steps;
  const char *tv[2] = {id, scripts};
  bool kept = n[6] <= 0 || run(db, "UPDATE verse.world_docs SET dev_tables = $2 WHERE id = $1", 2, tv);
  if (!kept || !batch_save(db, id, w, what, step, "", did, (size_t)n[0], inv, n[4], &rev, &done_at, &steps)) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); goto done; }
  if (pid) { coax_string ok = wr_card_accepted(); api_respond(q->fd, 200, "application/json", NULL, ok.data, (size_t)ok.count); goto done; }
  long long none[4] = {0, 0, 0, 0};
  answer_state(q, id, (int)wr_lane(cs("dev")), rev, none, w, rev, w->live_rev, done_at, steps, "", 0, did, (size_t)n[0]);
done:
  free(text); free(spec); free(did); free(inv); free(acts); free(scripts); free(was);
}

// A visitor's batch on a realm's world (#13515): each new thing kept as a proposal the owner decides (verse.realm_proposals,
// written by 0195's verse.realm_propose), never carried out; its rules are realm-place-policy.coax's (rw_propose). The row
// is locked; the transaction is open. 202 in words when kept.
static void realm_propose(const request *q, PGconn *db, const char *id, const world_asker *a, int64_t verdict) {
  bool hidden = true; char role[16]; char *spec = NULL;
  char *acts = malloc(WORLD_BATCH + 1); char why[240]; int64_t n[5] = {0, 0, 0, 0, -1};
  if (!acts || !realm_facts(db, id, a, &hidden, role, &spec)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); goto done; }
  if (hidden || !spec) { rollback(db); refuse_verdict(q->fd, wr_read(false, 0, 0)); goto done; }
  int64_t status = wr_realm_propose(cs(spec), csn(q->body, q->body_len), (coax_v_u8){(uint8_t *)acts, WORLD_BATCH}, (coax_v_u8){(uint8_t *)why, sizeof why}, (coax_v_s64){n, 5});
  if (status != 0) {
    rollback(db);
    // a batch that is not all new things is no proposal: the world's own refusal of a visitor
    if (n[2] > 0 && wr_place_status(n[3]) == 403 && !wr_proposal_words(n[3])) refuse_verdict(q->fd, verdict);
    else refuse(q->fd, n[2] > 0 ? (int)wr_place_status(n[3]) : (int)wr_realm_status(status), n[2] > 0 ? csn(why, (size_t)n[2]) : wr_realm_words(status), (int)n[4]);
    goto done;
  }
  acts[n[0]] = 0;
  int edit = 0;
  for (char *line = acts; *line; edit++) {
    char *end = strchr(line, '\n'); if (!end) break;
    *end = 0;
    char verb[8] = "", name[64] = "", xs[12] = "", zs[12] = "", ys[12] = "";
    if (sscanf(line, "%7s %63s %11s %11s %11s", verb, name, xs, zs, ys) != 5) { rollback(db); refuse(q->fd, 500, cs("not read"), edit); goto done; }
    const char *v[6] = {id, name, xs, zs, ys, a->key};
    PGresult *r = PQexecParams(db, "SELECT verse.realm_propose($1, $2, $3::int, $4::int, $5::int, $6)", 6, NULL, v, NULL, NULL, 0);
    bool kept = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && atoll(PQgetvalue(r, 0, 0)) > 0;
    bool read = PQresultStatus(r) == PGRES_TUPLES_OK;
    if (!read) api_log("worlds: propose: %s", PQerrorMessage(db));
    PQclear(r);
    if (!kept) { rollback(db); refuse(q->fd, read ? 401 : 500, cs(read ? "a visitor proposes as a player: open the realm's page first" : "not saved"), edit); goto done; }
    line = end + 1;
  }
  if (!run(db, "COMMIT", 0, NULL)) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); goto done; }
  refuse(q->fd, 202, wr_proposed_words(), -1);
done:
  free(acts); free(spec);
}

// The owner's yes to a visitor's proposal `pid` on realm `slug` (POST /api/playtest/realms/SLUG/proposals/ID/accept,
// realms.c): the proposal's thing placed as the owner's own world edit, one step of the world's undo log, with the
// proposal settled in the same transaction (#13515). False when the realm has no world (the route answers 404).
bool world_realm_accept(const request *q, PGconn *db, const char *slug, const char *pid) {
  world_asker a;
  if (!who_asks(q, db, &a)) { refuse(q->fd, 500, cs("not read"), -1); return true; }
  PQclear(PQexec(db, "BEGIN"));
  world_row w;
  if (!row_read(db, slug, true, &w)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); return true; }
  bool hidden = false; char role[16] = ""; char *spec = NULL;
  if (!w.exists || !wr_at_once(cs(w.kind)) || !realm_facts(db, slug, &a, &hidden, role, &spec) || hidden || !spec) { rollback(db); row_free(&w); free(spec); return false; }
  bool shown = kind_known(w.kind) && wr_authored(cs(slug), cs(w.kind), wr_apply());
  int64_t verdict = wr_write(auth_same_site(q), wr_known(cs(w.kind), a.signed_in, a.player), wr_may(cs(slug), cs(w.kind), a.admin, cs(role)), shown, true, wr_apply(), w.dev_rev, w.dev_rev, w.done, w.steps);
  char *text = NULL;
  if (verdict != 0) { rollback(db); refuse_verdict(q->fd, verdict); goto done; }
  {
    const char *v[2] = {slug, pid};
    PGresult *r = PQexecParams(db, "SELECT state, change->>'thing', change->>'x', change->>'z', coalesce(change->>'yaw', '0') FROM verse.realm_proposals WHERE slug = $1 AND id = $2::bigint FOR UPDATE", 2, NULL, v, NULL, NULL, 0);
    bool read = PQresultStatus(r) == PGRES_TUPLES_OK;
    // no such card, or one decided already: the card route answers it in its own words (verse.edit_decide)
    if (!read || PQntuples(r) != 1 || strcmp(PQgetvalue(r, 0, 0), "waiting")) { PQclear(r); rollback(db); free(spec); row_free(&w); return false; }
    text = malloc(WORLD_BATCH + 1);
    coax_string made = text ? wr_realm_accept(cs(spec), w.dev_rev, cs(PQgetvalue(r, 0, 1)), cs(PQgetvalue(r, 0, 2)), cs(PQgetvalue(r, 0, 3)), cs(PQgetvalue(r, 0, 4)), (coax_v_u8){(uint8_t *)text, WORLD_BATCH}) : (coax_string){"", 0};
    PQclear(r);
    if (made.count == 0) { rollback(db); refuse(q->fd, 422, cs("the realm no longer places that kind of thing"), -1); goto done; }
    text[made.count] = 0;
  }
  realm_post(q, db, slug, &w, wr_apply(), &a, text, pid);
done:
  free(text); free(spec); row_free(&w);
  return true;
}

static void world_post(const request *q, PGconn *db, const char *id) {
  world_asker a;
  if (!who_asks(q, db, &a)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  int64_t head[3] = {0, 0, 0};
  bool batch_ok = q->body_len > 0 && q->body_len <= WORLD_BATCH && wr_batch(csn(q->body, q->body_len), (coax_v_s64){head, 3});
  PQclear(PQexec(db, "BEGIN"));
  world_row w;
  if (!row_read(db, id, true, &w)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); return; }
  int64_t what = head[1];
  // who authors it: a site admin, and where the realm's row says owner-admins, its owner and its admins (verse.edit_role)
  bool hidden = false; char role[16] = ""; char *spec = NULL;
  if (w.exists && !realm_facts(db, id, &a, &hidden, role, &spec)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); row_free(&w); return; }
  free(spec);
  bool shown = w.exists && kind_known(w.kind) && !hidden && wr_authored(cs(id), cs(w.kind), what);
  int64_t verdict = wr_write(auth_same_site(q), wr_known(cs(w.kind), a.signed_in, a.player), wr_may(cs(id), cs(w.kind), a.admin, cs(role)), shown, batch_ok, what, head[0], w.dev_rev, w.done, w.steps);
  // a visitor's batch of new things on a realm's world is kept for the owner, where the owner lets visitors build (#13515)
  if (verdict != 0 && shown && batch_ok && wr_proposes(verdict, cs(w.kind), what)) { realm_propose(q, db, id, &a, verdict); row_free(&w); return; }
  if (verdict != 0) { rollback(db); refuse_verdict(q->fd, verdict); row_free(&w); return; }
  if (wr_at_once(cs(w.kind))) {
    if (wr_publishes(what)) { rollback(db); refuse(q->fd, 422, wr_at_once_words(), -1); }
    else realm_post(q, db, id, &w, what, &a, NULL, NULL);
    row_free(&w);
    return;
  }
  if (wr_publishes(what)) { world_publish(q, db, id, &w, what, head[0]); row_free(&w); return; }

  int64_t step; size_t text_n = 0; bool fenced;
  char *text = batch_text(q, db, id, what, &w, &step, &text_n, &fenced);
  size_t doc_n = 0;
  char *doc = doc_of(w.kind, w.dev_doc, &doc_n);
  char *out_doc = malloc(WORLD_DOC + 1), *did = malloc(WORLD_BATCH + 1), *inv = malloc(WORLD_BATCH + 1), *out_tables = malloc(WORLD_TABLES + 1);
  // the world's import pack: a row of the pieces table that draws a pack mesh is held to the pack the world really has (#13420)
  size_t pack_n = 0;
  char *pack = pack_read(db, id, w.dev_pack, &pack_n);
  char why[240]; int64_t n[7] = {0, 0, 0, 0, 0, -1, 0};
  if (!text || !doc || !out_doc || !did || !inv || !out_tables || !pack) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); goto done; }
  int64_t status = splice_world_batch_packed(csn(doc, doc_n), cs(w.dev_tables ? w.dev_tables : ""), csn(pack, pack_n), csn(text, text_n), fenced, (coax_v_u8){(uint8_t *)out_doc, WORLD_DOC},
                                      (coax_v_u8){(uint8_t *)out_tables, WORLD_TABLES}, (coax_v_u8){(uint8_t *)did, WORLD_BATCH},
                                      (coax_v_u8){(uint8_t *)inv, WORLD_BATCH}, (coax_v_u8){(uint8_t *)why, sizeof why}, (coax_v_s64){n, 7});
  if (status != 0) {
    rollback(db);
    int http = (int)splice_world_status(status);
    if (http >= 500) api_log("worlds: %s: status %d on the stored document", id, (int)status);
    refuse(q->fd, http, n[3] > 0 ? csn(why, (size_t)n[3]) : splice_world_words(status), (int)n[5]);
    goto done;
  }
  out_doc[n[0]] = 0; did[n[1]] = 0; inv[n[2]] = 0; out_tables[n[6]] = 0;
  if (n[6] > 0 && !strcmp(w.kind, "splice-bots")) {
    // the batch changed the content tables: a script row it newly names must name a module the world stores that is
    // not switched off (script-policy.coax, #13467)
    char *was = tables_of(w.kind, w.dev_tables), said[240];
    bool read = was != NULL, ok = read && scripts_edit_ok(db, id, was, out_tables, said, sizeof said);
    free(was);
    if (!ok) { rollback(db); refuse(q->fd, read ? 422 : 500, cs(read ? said : "not read"), -1); goto done; }
  }
  char dry[4] = "";
  if (api_query(q, "dry", dry, sizeof dry) && dry[0] == '1') {
    // a dry run: the world's own code carried it out in a copy; nothing is saved (#13436)
    rollback(db);
    long long here[4] = {0, 0, 0, 0};
    kind_pins(w.kind, here);
    answer_state(q, id, (int)wr_lane(cs("dev")), w.dev_rev, here, &w, w.dev_rev, w.live_rev, w.done, w.steps, "", 0, did, (size_t)n[1]);
    goto done;
  }
  {
    // a batch that changed the content tables saves them with the document, in the same transaction
    const char *tv[2] = {id, out_tables};
    long long rev, done_at, steps;
    bool ok = n[6] <= 0 || run(db, "UPDATE verse.world_docs SET dev_tables = $2 WHERE id = $1", 2, tv);
    if (!ok || !batch_save(db, id, &w, what, step, out_doc, did, (size_t)n[1], inv, n[4], &rev, &done_at, &steps)) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); goto done; }
    long long here[4] = {0, 0, 0, 0};
    kind_pins(w.kind, here);
    answer_state(q, id, (int)wr_lane(cs("dev")), rev, here, &w, rev, w.live_rev, done_at, steps, "", 0, did, (size_t)n[1]);
  }
done:
  free(text); free(doc); free(out_doc); free(did); free(inv); free(out_tables); free(pack); row_free(&w);
}

// The content tables a lane's revision is read with: the world's own once a cell was edited, else
// the ones the build shipped with. ?lane=dev is the tables as they are edited; without it, live's.
static void world_content(const request *q, PGconn *db, const char *id) {
  char *out = malloc(WORLD_DOC);
  char lane[8] = "";
  bool dev = api_query(q, "lane", lane, sizeof lane) && wr_is_dev(wr_lane(cs(lane)));
  world_row w;
  int64_t n = -1;
  if (out && !strcmp(id, "splice-bots") && wr_loads(cs(id), cs("splice-bots")) && row_read(db, id, false, &w)) {
    const char *own = w.exists ? (dev ? w.dev_tables : w.live_tables) : "";
    n = splice_world_tables(cs(own ? own : ""), (coax_v_u8){(uint8_t *)out, WORLD_DOC});
    row_free(&w);
  } else if (out && row_read(db, id, false, &w)) {
    // a realm's: its board and settings as they stand, both lanes (realm-world.coax), for whoever may see the realm
    world_asker a; bool hidden = true; char role[16]; char *spec = NULL;
    if (w.exists && wr_at_once(cs(w.kind)) && wr_loads(cs(id), cs(w.kind)) && who_asks(q, db, &a) && realm_facts(db, id, &a, &hidden, role, &spec) && !hidden && spec)
      n = wr_realm_tables(cs(spec), cs(w.dev_tables ? w.dev_tables : ""), (coax_v_u8){(uint8_t *)out, WORLD_DOC});
    free(spec); row_free(&w);
  }
  if (n < 0) refuse_verdict(q->fd, wr_read(false, 0, 0));
  else api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, (size_t)n);
  free(out);
}

// The world's card: the dev lane's revision and how many things its document holds, counted by the
// world's own code, in the card's bytes (world-policy.coax wr_card).
static void world_card(const request *q, PGconn *db, const char *id) {
  char out[400];
  world_row w;
  int64_t n = -1;
  if (!strcmp(id, "splice-bots") && wr_loads(cs(id), cs("splice-bots")) && row_read(db, id, false, &w)) {
    size_t dn = 0;
    char *doc = doc_of("splice-bots", w.exists ? w.dev_doc : "", &dn);
    int64_t things = doc ? splice_world_things(csn(doc, dn), cs(w.exists && w.dev_tables ? w.dev_tables : "")) : -1;
    if (things >= 0) n = wr_card(cs(id), w.exists ? w.dev_rev : 0, things, splice_world_name(), (coax_v_u8){(uint8_t *)out, sizeof out});
    free(doc); row_free(&w);
  }
  if (n < 0) refuse_verdict(q->fd, wr_read(false, 0, 0));
  else api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, (size_t)n);
}

// ---- a world's imported assets (#13420; docs/protocol/world-assets-v1.md). The importer, its limits and every byte
// answered are Coax (metaverse/splice-bots/world-assets.coax, the code the dev server runs); who may import and what makes a
// stored object the file that was named are world-policy.coax's. The bucket's part is drops.c's.

// Whoever asks may import into this world: the verdict (0: yes), by the rule a write is held to.
static int64_t asset_may(const request *q, const world_row *w, const char *id) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]), admin = p && p->admin;
  // the importer is Splice's: a world of another kind imports nothing
  return wr_asset_write(auth_same_site(q), known, admin, w->exists && !strcmp(w->kind, "splice-bots") && wr_loads(cs(id), cs(w->kind)) && wr_authored(cs(id), cs(w->kind), wr_asset_what()));
}
// The lane a read of the assets names: dev's pack, or live's.
static const char *asset_lane(const request *q, const world_row *w) {
  char lane[8] = "";
  return api_query(q, "lane", lane, sizeof lane) && wr_is_dev(wr_lane(cs(lane))) ? w->dev_pack : w->live_pack;
}
static void asset_list(const request *q, PGconn *db, const char *id) {
  world_row w;
  if (!row_read(db, id, false, &w)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  char write[4] = "";
  int64_t verdict = api_query(q, "write", write, sizeof write) && write[0] == '1' ? asset_may(q, &w, id) : wr_read(w.exists && !strcmp(w.kind, "splice-bots") && wr_loads(cs(id), cs(w.kind)), wr_lane(cs("dev")), 0);
  if (verdict != 0) { refuse_verdict(q->fd, verdict); row_free(&w); return; }
  size_t pack_n = 0;
  char *pack = pack_read(db, id, asset_lane(q, &w), &pack_n);
  coax_t_SpAssetWork *k = calloc(1, sizeof *k);
  int64_t n = pack && k ? splice_world_assets((coax_v_t_SpAssetWork){k, 1}, cs(id), csn(pack, pack_n)) : -1;
  if (n < 0) refuse(q->fd, 500, cs("not read"), -1);
  else api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, k->answer.e, (size_t)n);
  free(k); free(pack); row_free(&w);
}
static void asset_pack(const request *q, PGconn *db, const char *id) {
  world_row w;
  if (!row_read(db, id, false, &w)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  if (wr_read(w.exists && !strcmp(w.kind, "splice-bots") && wr_loads(cs(id), cs(w.kind)), wr_lane(cs("dev")), 0) != 0) { refuse_verdict(q->fd, wr_read(false, 0, 0)); row_free(&w); return; }
  // by hash: the bytes a tab was told of; else the lane's as it now is
  char live[80] = "";
  bool by_hash = api_query(q, "live", live, sizeof live) && strlen(live) == 64 && strspn(live, "0123456789abcdef") == 64;
  const char *hash = by_hash ? live : asset_lane(q, &w);
  size_t n = 0;
  char *pack = *hash ? pack_read(db, id, hash, &n) : NULL;
  // a lane that has imported nothing has an empty pack (every page of the world asks for it: no error); a hash that
  // names no pack is one
  if ((!pack || !n) && by_hash) refuse(q->fd, 404, cs("no pack has that hash"), -1);
  else api_respond(q->fd, 200, "application/octet-stream", NULL, pack ? pack : "", pack ? n : 0);
  free(pack); row_free(&w);
}
// The bucket's part takes a second or more and this service answers one request at a time: a child does it, with a
// connection of its own, and answers (as drops.c does).
static bool asset_child(void) {
  signal(SIGCHLD, SIG_IGN);
  return fork() == 0;
}
// An import's ask, read and checked before anything is stored: who asks, the world, and the file by its name, size and
// licence. False when it answered a refusal. The file's bytes, hash and content type come back, and the object's name.
typedef struct { long long bytes; char hash[65], type[48], object[400]; char body[WORLD_ASK + 1]; size_t body_n; } asset_ask;
static bool asset_asked(const request *q, PGconn *db, const char *id, asset_ask *a) {
  world_row w;
  if (!row_read(db, id, false, &w)) { refuse(q->fd, 500, cs("not read"), -1); return false; }
  int64_t verdict = asset_may(q, &w, id);
  row_free(&w);
  if (verdict != 0) { refuse_verdict(q->fd, verdict); return false; }
  coax_t_SpAssetWork *k = calloc(1, sizeof *k);
  if (!k) { refuse(q->fd, 500, cs("not read"), -1); return false; }
  int64_t n[4] = {0, 0, 0, 0};
  memset(a, 0, sizeof *a);
  a->body_n = q->body_len > WORLD_ASK ? 0 : q->body_len;   // a body longer than an ask is none
  memcpy(a->body, q->body, a->body_n);
  int64_t status = splice_world_asset_ask((coax_v_t_SpAssetWork){k, 1}, csn(a->body, a->body_n), (coax_v_u8){(uint8_t *)a->hash, 64}, (coax_v_u8){(uint8_t *)a->type, sizeof a->type - 1}, (coax_v_s64){n, 4});
  if (status != 200) { refuse(q->fd, (int)status, csn((const char *)k->words.e, (size_t)k->words_n), -1); free(k); return false; }
  free(k);
  a->bytes = n[0];
  const char *prefix = getenv("OA_DROPS_PREFIX");
  snprintf(a->object, sizeof a->object, "%s/worlds/%s/files/%s", prefix && *prefix ? prefix : "dev", id, a->hash);
  return true;
}
// POST .../assets: the upload is opened for exactly the file's bytes and content type, or the bucket has the file.
static void asset_open(const request *q, PGconn *db, const char *id) {
  asset_ask a;
  if (!asset_asked(q, db, id, &a)) return;
  char from[256], bytes[24];
  if (!api_header(q, "Origin", from, sizeof from) || strcmp(from, auth_origin())) snprintf(from, sizeof from, "%s", auth_origin());
  snprintf(bytes, sizeof bytes, "%lld", a.bytes);
  if (!asset_child()) return;   // the parent goes on; the child answers
  long long stored = -1; char *head = NULL, session[2048] = "", out[2400];
  drops_inspect(a.object, &stored, &head);
  free(head);
  bool kept = wr_asset_kept(a.bytes, stored);
  if (!kept && stored >= 0) drops_delete_object(a.object);   // an object of another size under this name is not this file
  if (!kept && !drops_open_upload(a.object, a.type, bytes, from, session, sizeof session)) { refuse(q->fd, 503, cs("We could not open the upload. Try again."), -1); _exit(0); }
  int64_t n = splice_world_asset_answer(!kept, cs(a.hash), a.bytes, cs(session), (coax_v_u8){(uint8_t *)out, sizeof out});
  if (n < 0) refuse(q->fd, 500, cs("not written"), -1);
  else api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, out, (size_t)n);
  _exit(0);
}
// Whether any import of this world kept the file of this hash (its object then stays whatever a later import says).
static bool file_kept(PGconn *c, const char *id, const char *hash) {
  const char *v[2] = {id, hash};
  PGresult *r = PQexecParams(c, "SELECT 1 FROM verse.world_files WHERE world = $1 AND hash = $2", 2, NULL, v, NULL, NULL, 0);
  bool kept = PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) > 0;   // unsure: it stays
  PQclear(r);
  return kept;
}
// The new pack and the file's row saved, dev's lane pointed at the pack, and the packs no lane names any more deleted.
static bool asset_save(PGconn *c, const char *id, const world_row *w, const asset_ask *a, const coax_t_SpAssetWork *k) {
  char at[24], kind[8], lic[8], bytes[24], hash[65], name[130];
  snprintf(at, sizeof at, "%lld", (long long)api_now_ms()); snprintf(kind, sizeof kind, "%d", (int)k->kind); snprintf(lic, sizeof lic, "%lld", (long long)k->licence);
  snprintf(bytes, sizeof bytes, "%lld", a->bytes); snprintf(hash, sizeof hash, "%.64s", (const char *)k->pack_hash.e); snprintf(name, sizeof name, "%.*s", (int)k->file_n, (const char *)k->file.e);
  const char *pv[4] = {id, hash, (const char *)k->out.e, at};
  int pl[4] = {0, 0, (int)k->pack_n, 0}, pf[4] = {0, 0, 1, 0};
  PGresult *r = PQexecParams(c, "INSERT INTO verse.world_packs (world, hash, bytes, at_ms) VALUES ($1, $2, $3, $4::bigint) ON CONFLICT (world, hash) DO NOTHING", 4, NULL, pv, pl, pf, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  if (!ok) api_log("worlds: %s", PQerrorMessage(c));
  PQclear(r);
  const char *fv[7] = {id, a->hash, name, kind, lic, bytes, at};
  ok = ok && run(c, "UPDATE verse.world_docs SET dev_pack = $2 WHERE id = $1", 2, pv)
    && run(c, "INSERT INTO verse.world_files (world, hash, name, kind, licence, bytes, at_ms) VALUES ($1, $2, $3, $4::int, $5::int, $6::bigint, $7::bigint) "
              "ON CONFLICT (world, hash) DO UPDATE SET name = EXCLUDED.name, kind = EXCLUDED.kind, licence = EXCLUDED.licence, at_ms = EXCLUDED.at_ms", 7, fv);
  if (!ok) return false;
  const char *v[1] = {id};
  r = PQexecParams(c, "SELECT hash FROM verse.world_packs WHERE world = $1", 1, NULL, v, NULL, NULL, 0);
  ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  for (int i = 0; ok && i < PQntuples(r); i++) {
    const char *dv[2] = {id, PQgetvalue(r, i, 0)};
    if (!wr_pack_kept(cs(dv[1]), cs(hash), cs(w->live_pack), cs(w->prev_pack))) ok = run(c, "DELETE FROM verse.world_packs WHERE world = $1 AND hash = $2", 2, dv);
  }
  PQclear(r);
  return ok;
}
// POST .../assets/done: the object is read back and checked against its name, then imported into dev's pack.
static void asset_done(const request *q, PGconn *db, const char *id) {
  asset_ask a;
  if (!asset_asked(q, db, id, &a)) return;
  if (!asset_child()) return;
  long long stored = -1; char *head = NULL; size_t n = 0;
  drops_inspect(a.object, &stored, &head);
  free(head);
  unsigned char *data = stored == a.bytes ? drops_fetch(a.object, (size_t)a.bytes, &n) : NULL;
  bool named = data && (long long)n == a.bytes && splice_world_asset_named(csn((const char *)data, n), cs(a.hash));
  int64_t verdict = wr_asset_stored(a.bytes, data || stored != a.bytes ? stored : -1, named);
  if (verdict != 0) {
    if (wr_asset_deletes(verdict)) drops_delete_object(a.object);
    refuse(q->fd, 422, wr_asset_words(verdict), -1);
    free(data); _exit(0);
  }
  PGconn *c = api_db_new();
  coax_t_SpAssetWork *k = calloc(1, sizeof *k);
  world_row w; memset(&w, 0, sizeof w);
  if (!c || !k) { refuse(q->fd, 503, cs("no database"), -1); _exit(0); }
  PQclear(PQexec(c, "BEGIN"));
  size_t old_n = 0, doc_n = 0;
  char *old = row_read(c, id, true, &w) && w.exists ? pack_read(c, id, w.dev_pack, &old_n) : NULL;
  char *doc = old ? doc_of(w.kind, w.dev_doc, &doc_n) : NULL;
  if (!old || !doc) { rollback(c); refuse(q->fd, 500, cs("not read"), -1); _exit(0); }
  int64_t status = splice_world_asset_import((coax_v_t_SpAssetWork){k, 1}, cs(id), csn(old, old_n), csn(doc, doc_n), cs(w.dev_tables ? w.dev_tables : ""), csn(a.body, a.body_n), (coax_v_u8){data, (int64_t)n});
  if (status != 200) {
    // refused: the pack stays as it was, and a file no import kept leaves the bucket
    bool kept = file_kept(c, id, a.hash);
    rollback(c);
    if (!kept) drops_delete_object(a.object);
    refuse(q->fd, (int)status, csn((const char *)k->words.e, (size_t)k->words_n), -1);
    _exit(0);
  }
  bool ok = asset_save(c, id, &w, &a, k) && run(c, "COMMIT", 0, NULL);
  if (!ok) { rollback(c); refuse(q->fd, 500, cs("not saved"), -1); _exit(0); }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, k->answer.e, (size_t)k->answer_n);
  _exit(0);
}

// ---- attachments on a world and its project (#13468; docs/protocol/world-files-v1.md). Each scope's list is a row
// (migrations/0184_world_attachments.sql); the rules and every byte of a list are Coax (world-files.coax, and the
// shared store code the dev server runs, metaverse-engine/editor/file-store.coax). A file goes by direct upload to the
// private bucket at PREFIX/attachments/SHA256 (drops.c), is read back and scanned, and leaves when no list names it.
// Nothing about a file is logged.
#define FILES_ASK 2048
typedef struct { coax_t_FlWork *w; char world[40], project[56]; } files_ctx;
static coax_v_t_FlWork fw_of(files_ctx *x) { return (coax_v_t_FlWork){x->w, 1}; }
static void files_refuse_said(int fd, files_ctx *x) { coax_string s = wf_said(fw_of(x)); refuse(fd, (int)x->w->status, s, -1); }
static void files_object(const char *hash, char *out, size_t cap) {
  const char *prefix = getenv("OA_DROPS_PREFIX");
  snprintf(out, cap, "%s/attachments/%.64s", prefix && *prefix ? prefix : "dev", hash);
}
// A scope's stored list, malloc'd ("" for none), or NULL when it can't be read.
static char *files_list(PGconn *db, const char *key) {
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT list FROM verse.attachment_lists WHERE key = $1", 1, NULL, v, NULL, NULL, 0);
  char *out = NULL;
  if (PQresultStatus(r) == PGRES_TUPLES_OK) out = strdup(PQntuples(r) == 1 ? PQgetvalue(r, 0, 0) : "");
  else api_log("worlds: attachments not read: %s", PQerrorMessage(db));
  PQclear(r);
  return out;
}
// Whoever asks may see and change this project's attachments: the verdict (0: yes).
static int64_t files_may(const request *q, PGconn *db, const char *project) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]), admin = p && p->admin, member = false;
  if (project[0] && p && p->pubkey[0] && !admin) {
    const char *v[2] = {project, p->pubkey};
    PGresult *r = PQexecParams(db, "SELECT projects.role($1, $2) = 'member'", 2, NULL, v, NULL, NULL, 0);
    member = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
    PQclear(r);
  }
  return wf_may(auth_same_site(q), known, admin, member, project[0] != 0);
}
static const char *files_by(const request *q) { const auth_principal *p = auth_principal_of(q); return p && p->pubkey[0] ? p->pubkey : "account"; }
// The world's list and its project's read into x (world "" for a project's own). lock: in a transaction, the
// project's lists are held until it ends, so two writes never count the same room twice.
static bool files_load(PGconn *db, files_ctx *x, const char *world, const char *project, bool lock) {
  snprintf(x->world, sizeof x->world, "%s", world); snprintf(x->project, sizeof x->project, "%s", project);
  if (!(x->w = calloc(1, sizeof *x->w))) return false;
  char wkey[64], pkey[72];
  snprintf(wkey, sizeof wkey, "world:%s", world); snprintf(pkey, sizeof pkey, "project:%s", project);
  if (lock) {
    const char *v[1] = {project[0] ? pkey : wkey};
    if (!run(db, "SELECT pg_advisory_xact_lock(hashtext('attachments ' || $1))", 1, v)) return false;
  }
  char *wl = world[0] ? files_list(db, wkey) : strdup(""), *pl = project[0] ? files_list(db, pkey) : strdup("");
  long long other = 0, other_bytes = 0;
  bool ok = wl && pl;
  if (ok && project[0]) {
    const char *v[3] = {project, wkey, pkey};
    PGresult *r = PQexecParams(db, "SELECT coalesce(sum(count), 0), coalesce(sum(bytes), 0) FROM verse.attachment_lists WHERE project = $1 AND key <> $2 AND key <> $3", 3, NULL, v, NULL, NULL, 0);
    ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
    if (ok) { other = atoll(PQgetvalue(r, 0, 0)); other_bytes = atoll(PQgetvalue(r, 0, 1)); }
    PQclear(r);
  }
  ok = ok && wf_load(fw_of(x), cs(world), cs(project), cs(wl), cs(pl), other, other_bytes);
  free(wl); free(pl);
  return ok;
}
// Both scopes' lists as they now are, saved.
static bool files_save(PGconn *db, files_ctx *x) {
  static char out[180001];
  char key[72], count[16], bytes[24], at[24];
  snprintf(at, sizeof at, "%lld", (long long)api_now_ms());
  for (int scope = 0; scope < 2; scope++) {
    int64_t n = wf_stored(fw_of(x), scope, (coax_v_u8){(uint8_t *)out, sizeof out - 1});
    if (n < 0) continue;
    out[n] = 0;
    if (scope == 0) snprintf(key, sizeof key, "world:%s", x->world); else snprintf(key, sizeof key, "project:%s", x->project);
    snprintf(count, sizeof count, "%lld", (long long)wf_held(csn(out, (size_t)n), false)); snprintf(bytes, sizeof bytes, "%lld", (long long)wf_held(csn(out, (size_t)n), true));
    const char *v[6] = {key, x->project, out, count, bytes, at};
    if (!run(db, "INSERT INTO verse.attachment_lists (key, project, list, count, bytes, at_ms) VALUES ($1, $2, $3, $4::int, $5::bigint, $6::bigint) "
                 "ON CONFLICT (key) DO UPDATE SET project = EXCLUDED.project, list = EXCLUDED.list, count = EXCLUDED.count, bytes = EXCLUDED.bytes, at_ms = EXCLUDED.at_ms", 6, v)) return false;
  }
  return true;
}
// Whether any list of the project (or the world's own, for a world with none) still names the file of this hash.
static bool files_named(PGconn *db, files_ctx *x, const char *hash) {
  char wkey[64]; snprintf(wkey, sizeof wkey, "world:%s", x->world);
  const char *v[2] = {x->project, wkey};
  PGresult *r = PQexecParams(db, "SELECT list FROM verse.attachment_lists WHERE (project = $1 AND project <> '') OR key = $2", 2, NULL, v, NULL, NULL, 0);
  bool named = PQresultStatus(r) != PGRES_TUPLES_OK;   // unsure: it stays
  for (int i = 0; !named && i < PQntuples(r); i++) named = wf_names(cs(PQgetvalue(r, i, 0)), cs(hash));
  PQclear(r);
  return named;
}
static void files_answer(int fd, files_ctx *x) {
  coax_string s = wf_list_answer(fw_of(x));
  if (!s.count) refuse(fd, 500, cs("not written"), -1);
  else api_respond(fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", s.data, (size_t)s.count);
}
// GET .../files and GET /api/projects/SLUG/files: the list.
static void files_get(const request *q, PGconn *db, const char *world, const char *project) {
  int64_t verdict = files_may(q, db, project);
  if (verdict != 0) { refuse(q->fd, (int)wf_status(verdict), wf_words(verdict), -1); return; }
  files_ctx x = {0};
  if (!files_load(db, &x, world, project, false)) refuse(q->fd, 500, cs("not read"), -1);
  else files_answer(q->fd, &x);
  free(x.w);
}
// GET .../files/FID: its bytes (?at=N: a part of them), as the type its extension names, never stored by a browser.
static void files_raw(const request *q, PGconn *db, const char *world, const char *project, const char *fid) {
  int64_t verdict = files_may(q, db, project);
  if (verdict != 0) { refuse(q->fd, (int)wf_status(verdict), wf_words(verdict), -1); return; }
  files_ctx x = {0};
  if (!files_load(db, &x, world, project, false)) { refuse(q->fd, 500, cs("not read"), -1); free(x.w); return; }
  int64_t i = wf_file(fw_of(&x), cs(fid));
  if (i < 0) { refuse(q->fd, 404, cs("That attachment has no file here."), -1); free(x.w); return; }
  char object[200], hash[65], type[64], at_s[24] = "";
  coax_string h = wf_file_hash(fw_of(&x), i), t = wf_file_type(fw_of(&x), i);
  snprintf(hash, sizeof hash, "%.*s", (int)h.count, h.data); snprintf(type, sizeof type, "%.*s", (int)t.count, t.data);
  long long bytes = wf_file_bytes(fw_of(&x), i), at = 0, part = (long long)wf_read_part();
  free(x.w);
  if (api_query(q, "at", at_s, sizeof at_s)) { char *end; at = strtoll(at_s, &end, 10); if (*end || at < 0 || at > bytes) { refuse(q->fd, 400, cs("?at= is a byte of the file"), -1); return; } }
  files_object(hash, object, sizeof object);
  if (!asset_child()) return;
  size_t n = 0;
  unsigned char *data = drops_fetch(object, (size_t)bytes, &n);
  if (!data || (long long)n != bytes) refuse(q->fd, 404, cs("That attachment's file is not here."), -1);
  else {
    size_t from = at_s[0] ? (size_t)at : 0, to = at_s[0] && bytes - at > part ? (size_t)(at + part) : n;
    api_respond(q->fd, 200, type, "Cache-Control: private, no-store\r\nContent-Disposition: inline\r\n", data + from, to - from);
  }
  free(data);
  _exit(0);
}
// POST .../files: an upload asked for, or a link, an edit or a detach. POST .../files/done: the upload is done.
static void files_post(const request *q, PGconn *db, const char *world, const char *project, bool done) {
  int64_t verdict = files_may(q, db, project);
  if (verdict != 0) { refuse(q->fd, (int)wf_status(verdict), wf_words(verdict), -1); return; }
  coax_t_FwAsk ask; memset(&ask, 0, sizeof ask);
  char body[FILES_ASK + 1]; size_t bn = q->body_len > FILES_ASK ? 0 : q->body_len;
  memcpy(body, q->body, bn); body[bn] = 0;
  int64_t what = q->body_len > 0 && q->body_len <= FILES_ASK ? wf_ask((coax_v_t_FwAsk){&ask, 1}, csn(body, bn)) : (q->body_len > FILES_ASK ? 1 : 0);
  if (what == 1) { refuse(q->fd, 415, wf_whole_words(), -1); return; }
  if (what == 0 || (done && what != 2)) { refuse(q->fd, 400, cs("send a world.file.* request"), -1); return; }
  files_ctx x = {0};
  coax_v_t_FwAsk a = {&ask, 1};
  if (what == 2) {
    if (!files_load(db, &x, world, project, false)) { refuse(q->fd, 500, cs("not read"), -1); free(x.w); return; }
    if (wf_admit(fw_of(&x), a) != 200) { files_refuse_said(q->fd, &x); free(x.w); return; }
    free(x.w); x.w = NULL;
    char object[200], hash[65], type[64], bytes[24], from[256];
    coax_string h = wf_ask_hash(a), t = wf_ask_type(a);
    snprintf(hash, sizeof hash, "%.*s", (int)h.count, h.data); snprintf(type, sizeof type, "%.*s", (int)t.count, t.data);
    snprintf(bytes, sizeof bytes, "%lld", (long long)wf_ask_bytes(a));
    files_object(hash, object, sizeof object);
    if (!api_header(q, "Origin", from, sizeof from) || strcmp(from, auth_origin())) snprintf(from, sizeof from, "%s", auth_origin());
    if (!asset_child()) return;
    long long stored = -1; char *head = NULL;
    drops_inspect(object, &stored, &head);
    free(head);
    if (!done) {
      char session[2048] = "", out[2400];
      bool kept = stored == wf_ask_bytes(a);
      if (!kept && stored >= 0) drops_delete_object(object);
      if (!kept && !drops_open_upload(object, type, bytes, from, session, sizeof session)) { refuse(q->fd, 503, cs("We could not open the upload. Try again."), -1); _exit(0); }
      int64_t n = wf_upload_answer(!kept, cs(hash), wf_ask_bytes(a), cs(session), (coax_v_u8){(uint8_t *)out, sizeof out});
      if (n < 0) refuse(q->fd, 500, cs("not written"), -1);
      else api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, out, (size_t)n);
      _exit(0);
    }
    size_t n = 0;
    unsigned char *data = stored == wf_ask_bytes(a) ? drops_fetch(object, (size_t)stored, &n) : NULL;
    if (!data || (long long)n != wf_ask_bytes(a) || !splice_world_asset_named(csn((const char *)data, n), cs(hash))) {
      if (stored >= 0) drops_delete_object(object);
      refuse(q->fd, 422, cs(stored < 0 ? "The file did not arrive. Send it again." : "The stored file is not the one asked for. Send it again."), -1);
      free(data); _exit(0);
    }
    PGconn *c = api_db_new();
    if (!c) { refuse(q->fd, 503, cs("no database"), -1); _exit(0); }
    PQclear(PQexec(c, "BEGIN"));
    bool ok = files_load(c, &x, world, project, true);
    int64_t status = ok ? wf_attach(fw_of(&x), a, csn((const char *)data, n < 16 ? n : 16), (int64_t)api_now_ms(), cs(files_by(q))) : 500;
    if (status != 200) {
      rollback(c);
      if (!files_named(c, &x, hash)) drops_delete_object(object);
      if (ok) files_refuse_said(q->fd, &x); else refuse(q->fd, 500, cs("not read"), -1);
      _exit(0);
    }
    if (!files_save(c, &x) || !run(c, "COMMIT", 0, NULL)) { rollback(c); refuse(q->fd, 500, cs("not saved"), -1); _exit(0); }
    files_answer(q->fd, &x);
    _exit(0);
  }
  PQclear(PQexec(db, "BEGIN"));
  if (!files_load(db, &x, world, project, true)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); free(x.w); return; }
  if (wf_apply(fw_of(&x), a, (int64_t)api_now_ms(), cs(files_by(q))) != 200) { rollback(db); files_refuse_said(q->fd, &x); free(x.w); return; }
  if (!files_save(db, &x) || !run(db, "COMMIT", 0, NULL)) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); free(x.w); return; }
  coax_string freed = wf_freed(fw_of(&x));
  char hash[65] = "";
  if (freed.count == 64) snprintf(hash, sizeof hash, "%.64s", freed.data);
  if (hash[0] && !files_named(db, &x, hash)) {
    char object[200]; files_object(hash, object, sizeof object);
    drops_delete_object(object);
  }
  files_answer(q->fd, &x);
  free(x.w);
}
// /api/projects/SLUG/files: a project's own attachments, for its page.
bool world_files_project_route(const request *q) {
  static const char root[] = "/api/projects/";
  char slug[56] = ""; int end = 0;
  if (strncmp(q->path, root, sizeof root - 1)) return false;
  if (sscanf(q->path, "/api/projects/%55[a-z0-9-]/files%n", slug, &end) != 1 || q->path[end] || strcmp(q->method, "GET") || !wf_project_ok(cs(slug))) { refuse(q->fd, 404, cs("not found"), -1); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q->fd, 503, cs("no database"), -1); return true; }
  files_get(q, db, "", slug);
  return true;
}

// ---- what a hosted world turn opened (chat.c, POST /api/chat/complete with "attachments": {"world", "ids"}; the
// editor's Gemini turn, docs/protocol/world-turn-v1.md). Each attachment the model asked to open joins the request's
// last message: a line naming it, then its text or the file itself (an image or a PDF, which the model reads), up to
// WF_TURN_BYTES for the turn together; past that, or for a link, words in its place. The same access as the Files
// pane's. The worker reads the files itself (at most 8 MB, an admin's turn). Nothing about a file is logged.
#define TURN_OPENS 3
// One part joined to the last message: words, then (how WF_TURN_TEXT or WF_TURN_FILE) the bytes. The new body, or NULL.
static char *turn_join(PGconn *db, const char *body, const char *said, int how, const unsigned char *data, size_t n, const char *type) {
  char hows[4]; snprintf(hows, sizeof hows, "%d", how);
  const char *v[5] = {body, said, hows, data ? (const char *)data : "", type};
  int len[5] = {0, 0, 0, data ? (int)n : 0, 0}, fmt[5] = {0, 0, 0, 1, 0};
  PGresult *r = PQexecParams(db,
    "SELECT jsonb_set(b, ARRAY['messages', (jsonb_array_length(b->'messages') - 1)::text, 'content'], "
    "(CASE WHEN jsonb_typeof(b->'messages'->-1->'content') = 'array' THEN b->'messages'->-1->'content' "
    "ELSE jsonb_build_array(jsonb_build_object('type', 'text', 'text', coalesce(b->'messages'->-1->>'content', ''))) END) "
    "|| jsonb_build_array(jsonb_build_object('type', 'text', 'text', $2::text)) "
    "|| CASE $3 WHEN '1' THEN jsonb_build_array(jsonb_build_object('type', 'text', 'text', convert_from($4::bytea, 'UTF8'))) "
    "WHEN '2' THEN jsonb_build_array(jsonb_build_object('type', 'image_url', 'image_url', jsonb_build_object('url', "
    "'data:' || $5 || ';base64,' || translate(encode($4::bytea, 'base64'), E'\\n', '')))) ELSE '[]'::jsonb END)::text "
    "FROM (SELECT $1::jsonb b) s", 5, NULL, v, len, fmt, 0);
  char *out = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? strdup(PQgetvalue(r, 0, 0)) : NULL;
  PQclear(r);   // its error may quote the file: never logged
  return out;
}
#ifdef OA_LOCAL_SERVE
// The dev server's store (metaverse-engine/tools/files-local.coax), when serve.sh names it: a file under it, malloc'd.
static unsigned char *turn_local(const char *dir, const char *a, const char *b, const char *c, size_t cap, size_t *n) {
  char path[1200]; snprintf(path, sizeof path, "%s/%s%s%s", dir, a, b, c);
  FILE *f = fopen(path, "rb");
  unsigned char *out = f ? malloc(cap + 1) : NULL;
  *n = out ? fread(out, 1, cap + 1, f) : 0;
  if (f) fclose(f);
  if (out && *n > cap) { free(out); out = NULL; }
  if (out) out[*n] = 0;
  return out;
}
#endif
int world_files_turn(const request *q, PGconn *db, char **out, char *words, size_t cap) {
  *out = NULL;
  char *body = malloc(q->body_len + 1);
  if (!body) { snprintf(words, cap, "memory"); return 503; }
  memcpy(body, q->body, q->body_len); body[q->body_len] = 0;
  const char *v[1] = {body};
  PGresult *r = PQexecParams(db, "SELECT coalesce(b->'attachments'->>'world', ''), coalesce((SELECT string_agg(x, ' ') FROM "
                                 "jsonb_array_elements_text(CASE WHEN jsonb_typeof(b->'attachments'->'ids') = 'array' THEN b->'attachments'->'ids' ELSE '[]' END) x), ''), "
                                 "(b - 'attachments')::text, jsonb_array_length(b->'attachments'->'ids') FROM (SELECT $1::jsonb b) s", 1, NULL, v, NULL, NULL, 0);
  free(body);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); snprintf(words, cap, "not JSON"); return 400; }
  char world[40], ids[TURN_OPENS * 20 + 8];
  bool fits = strlen(PQgetvalue(r, 0, 0)) < sizeof world && strlen(PQgetvalue(r, 0, 1)) < sizeof ids && atoi(PQgetvalue(r, 0, 3)) <= TURN_OPENS;
  snprintf(world, sizeof world, "%s", PQgetvalue(r, 0, 0)); snprintf(ids, sizeof ids, "%s", PQgetvalue(r, 0, 1));
  char *b = strdup(PQgetvalue(r, 0, 2));
  PQclear(r);
  if (!fits || !wf_world_ok(cs(world)) || !b) { free(b); snprintf(words, cap, "attachments is {world, ids: at most %d}", TURN_OPENS); return 400; }
  { char was[sizeof world]; snprintf(was, sizeof was, "%s", world); coax_string named = wr_world_id(cs(was)); snprintf(world, sizeof world, "%.*s", (int)named.count, named.data); }   // an old world ID (world-alias.coax)
  coax_string ps = wf_project(cs(world));
  char project[56]; snprintf(project, sizeof project, "%.*s", (int)ps.count, ps.data);
  int64_t verdict = files_may(q, db, project);
  if (verdict != 0) { free(b); coax_string w = wf_words(verdict); snprintf(words, cap, "%.*s", (int)w.count, w.data); return (int)wf_status(verdict); }
  files_ctx x = {0};
  bool ok;
  const char *dir = NULL;
#ifdef OA_LOCAL_SERVE
  dir = getenv("OA_WORLD_FILES_DIR");
  if (dir && !*dir) dir = NULL;
#endif
  if (dir) {
#ifdef OA_LOCAL_SERVE
    size_t wn = 0, pn = 0;
    unsigned char *wl = turn_local(dir, world, ".files", "", 200000, &wn), *pl = project[0] ? turn_local(dir, "project.", project, ".files", 200000, &pn) : NULL;
    snprintf(x.world, sizeof x.world, "%s", world); snprintf(x.project, sizeof x.project, "%s", project);
    ok = (x.w = calloc(1, sizeof *x.w)) && wf_load(fw_of(&x), cs(world), cs(project), csn(wl ? (const char *)wl : "", wn), csn(pl ? (const char *)pl : "", pn), 0, 0);
    free(wl); free(pl);
#else
    ok = false;
#endif
  } else ok = files_load(db, &x, world, project, false);
  if (!ok) { free(b); free(x.w); snprintf(words, cap, "not read"); return 500; }
  long long left = wf_turn_bytes();
  char said[1400];
  for (char *id = strtok(ids, " "); id && b; id = strtok(NULL, " ")) {
    int64_t i = wf_find(fw_of(&x), cs(id));
    if (i < 0) { char *nb = turn_join(db, b, "That attachment is no longer on this world.", 0, NULL, 0, ""); free(b); b = nb; continue; }
    int64_t how = wf_turn_how(fw_of(&x), i), bytes = wf_file_bytes(fw_of(&x), i);
    int64_t sn = wf_turn_said(fw_of(&x), i, left, (coax_v_u8){(uint8_t *)said, sizeof said - 1});
    if (sn < 0) sn = 0;
    said[sn] = 0;
    unsigned char *data = NULL; size_t n = 0;
    if (how != 3 && bytes <= left) {
      coax_string h = wf_file_hash(fw_of(&x), i);
      char hash[65]; snprintf(hash, sizeof hash, "%.*s", (int)h.count, h.data);
#ifdef OA_LOCAL_SERVE
      if (dir) data = turn_local(dir, "files/", hash, "", (size_t)bytes, &n);
#endif
      if (!dir) { char object[200]; files_object(hash, object, sizeof object); data = drops_fetch(object, (size_t)bytes, &n); }
      if (!data || (long long)n != bytes) { free(data); data = NULL; snprintf(said + sn, sizeof said - (size_t)sn, " Its file could not be read."); }
      else left -= bytes;
    }
    coax_string t = wf_file_type(fw_of(&x), i);
    char type[64]; snprintf(type, sizeof type, "%.*s", (int)t.count, t.data);
    char *nb = turn_join(db, b, said, data ? (int)how : 0, data, n, type);
    if (!nb && data) nb = turn_join(db, b, "That attachment could not be read as text.", 0, NULL, 0, "");
    free(data); free(b); b = nb;
  }
  free(x.w);
  if (!b) { snprintf(words, cap, "the attachments were not joined to the request"); return 500; }
  *out = b;
  return 200;
}

// GET /api/worlds: the worlds the asker may edit, Splice first, one line each: "world ID NAME". Who may is the
// rule a write is held to (wr_may: a site admin, or a realm's owner and admins); someone logged out gets none.
// The site's side lists them under Worlds, each opening its editor, and `openagents realm list` shows the same
// rows as its "worlds" (apps/openagents-cli/realm.coax, #13493).
static void world_list(const request *q, PGconn *db) {
  world_asker a;
  if (!who_asks(q, db, &a)) { refuse(q->fd, 500, cs("not read"), -1); return; }
  char out[4096]; size_t n = 0;
  if (a.signed_in) {
    const char *v[2] = {a.key, a.admin ? "true" : "false"};
    PGresult *r = PQexecParams(db,
      "SELECT d.id, d.kind, coalesce(m.name, ''), " API_REALM_ROLE_FACTS("d.id", "$1") " FROM verse.world_docs d "
      "LEFT JOIN verse.made_realms m ON m.slug = d.id CROSS JOIN LATERAL " API_REALM_ROLE_FROM("d.id", "$1") " "
      "WHERE NOT verse.realm_hidden(d.id, $1, $2::boolean) ORDER BY d.id <> 'splice-bots', d.id LIMIT 64", 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("worlds: %s", PQerrorMessage(db)); PQclear(r); refuse(q->fd, 500, cs("not read"), -1); return; }
    for (int k = 0; k < PQntuples(r) && k < 24; k++) {
      const char *id = PQgetvalue(r, k, 0), *kind = PQgetvalue(r, k, 1);
      if (!kind_known(kind) || !wr_loads(cs(id), cs(kind)) || !wr_may(cs(id), cs(kind), a.admin, role_of(r, k, 3, a.admin))) continue;
      char name[64];
      coax_string sn = splice_world_name();
      if (!strcmp(kind, "splice-bots")) snprintf(name, sizeof name, "%.*s", (int)sn.count, sn.data);
      else snprintf(name, sizeof name, "%s", PQgetvalue(r, k, 2)[0] ? PQgetvalue(r, k, 2) : id);
      for (char *c = name; *c; c++) if ((unsigned char)*c < 32) *c = ' ';
      int w = snprintf(out + n, sizeof out - n, "world %s %s\n", id, name);
      if (w < 0 || (size_t)w >= sizeof out - n) break;
      n += (size_t)w;
    }
    PQclear(r);
  }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, n);
}

// GET /api/projects: the projects the asker is in, one line each: "project SLUG NAME". A private project lists
// them when their key is among its members (projects.role); a published project when they dropped it. Someone
// logged out gets none. The site's side lists them under Projects, each opening /projects/SLUG.
static void project_list(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  char out[4096]; size_t n = 0;
  if (p && p->pubkey[0]) {
    const char *v[1] = {p->pubkey};
    PGresult *r = PQexecParams(db,
      "SELECT slug, name FROM ("
      " SELECT d.slug, coalesce(nullif(d.doc->>'title', ''), d.slug) AS name FROM projects.docs d WHERE projects.role(d.slug, $1) = 'member'"
      " UNION SELECT j.slug, j.name FROM drops.projects j WHERE j.owner = $1 AND j.published_ms IS NOT NULL"
      ") x ORDER BY lower(name), slug LIMIT 24", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("projects: %s", PQerrorMessage(db)); PQclear(r); refuse(q->fd, 500, cs("not read"), -1); return; }
    for (int k = 0; k < PQntuples(r); k++) {
      char name[64];
      snprintf(name, sizeof name, "%s", PQgetvalue(r, k, 1));
      for (char *c = name; *c; c++) if ((unsigned char)*c < 32) *c = ' ';
      int w = snprintf(out + n, sizeof out - n, "project %s %s\n", PQgetvalue(r, k, 0), name);
      if (w < 0 || (size_t)w >= sizeof out - n) break;
      n += (size_t)w;
    }
    PQclear(r);
  }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, n);
}

bool worlds_route(const request *q) {
  if (!strcmp(q->path, "/api/projects")) {
    if (strcmp(q->method, "GET")) { refuse(q->fd, 404, cs("not found"), -1); return true; }
    PGconn *db = api_db();
    if (!db) { refuse(q->fd, 503, cs("no database"), -1); return true; }
    project_list(q, db);
    return true;
  }
  if (!strcmp(q->path, "/api/worlds")) {
    if (strcmp(q->method, "GET")) { refuse(q->fd, 404, cs("not found"), -1); return true; }
    PGconn *db = api_db();
    if (!db) { refuse(q->fd, 503, cs("no database"), -1); return true; }
    world_list(q, db);
    return true;
  }
  static const char root[] = "/api/worlds/";
  size_t rl = sizeof root - 1;
  if (strncmp(q->path, root, rl)) return false;
  bool get = !strcmp(q->method, "GET"), post = !strcmp(q->method, "POST");
  char id[40] = ""; const char *rest = q->path + rl; size_t k = 0;
  while (rest[k] && rest[k] != '/' && k < sizeof id - 1) { id[k] = rest[k]; k++; }
  id[k] = 0;
  // a renamed world's old ID is its new one, so a saved link, the CLI, an MCP tool or a cached page
  // that names it still works (world-alias.coax, #13486): /api/worlds/splice... is Splice Bots'
  char was[sizeof id]; snprintf(was, sizeof was, "%s", id);
  coax_string named = wr_world_id(cs(was));
  snprintf(id, sizeof id, "%.*s", (int)named.count, named.data);
  const char *tail = rest + k;
  bool content = !strcmp(tail, "/content"), card = !strcmp(tail, "/card");
  bool assets = !strcmp(tail, "/assets"), pack = !strcmp(tail, "/assets/pack"), done = !strcmp(tail, "/assets/done");
  // attachments (#13468): .../files, .../files/done, .../files/FID
  bool files = !strcmp(tail, "/files"), files_done = !strcmp(tail, "/files/done");
  char fid[20] = "";
  if (!strncmp(tail, "/files/", 7) && strlen(tail + 7) == 16) snprintf(fid, sizeof fid, "%s", tail + 7);
  if (files || files_done || fid[0]) {
    coax_string pj = wf_project(cs(id));
    char project[56]; snprintf(project, sizeof project, "%.*s", (int)pj.count, pj.data);
    if (!wf_world_ok(cs(id)) || (fid[0] && (!get || !wf_id_ok(cs(fid)))) || (files_done && !post) || (!get && !post)) { refuse(q->fd, 404, cs("not found"), -1); return true; }
    PGconn *fdb = api_db();
    if (!fdb) { refuse(q->fd, 503, cs("no database"), -1); return true; }
    if (fid[0]) files_raw(q, fdb, id, project, fid);
    else if (get) files_get(q, fdb, id, project);
    else files_post(q, fdb, id, project, files_done);
    return true;
  }
  if (!strncmp(tail, "/scripts", 8) && (get || post) && wr_world_ok(cs(id))) {
    // a world's realm scripts (#13467): scripts.c
    PGconn *sdb = api_db();
    if (!sdb) { refuse(q->fd, 503, cs("no database"), -1); return true; }
    if (scripts_route(q, sdb, id, tail)) return true;
  }
  if ((!get && !post) || !wr_world_ok(cs(id)) || (*tail && strcmp(tail, "/events") && !content && !card && !assets && !pack && !done) || (*tail && !get && !assets && !done) || (done && !post)) { refuse(q->fd, 404, cs("not found"), -1); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q->fd, 503, cs("no database"), -1); return true; }
  if (assets && get) asset_list(q, db, id);
  else if (assets) asset_open(q, db, id);
  else if (pack) asset_pack(q, db, id);
  else if (done) asset_done(q, db, id);
  else if (content) world_content(q, db, id);
  else if (card) world_card(q, db, id);
  else if (*tail) world_events(q, db, id);
  else if (get) world_get(q, db, id);
  else world_post(q, db, id);
  return true;
}
