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
//   GET  /api/worlds/<id>/assets/pack[?lane=dev] the lane's import pack, a VPK1 file (404: it has imported nothing)
//        ?live=SHA256                            the world's pack of that hash, whatever lane names it: what a tab asks for
//                                                after it is told of a new pack
//   POST /api/worlds/<id>/assets                 a file named before it is sent (world.asset.upload.v1): checked by name, size
//                                                and licence, then an upload is opened in the private bucket for exactly that
//                                                many bytes at PREFIX/worlds/<id>/files/SHA256 (drops.c), and its address is
//                                                answered; `kept` when the bucket has the file already
//   POST /api/worlds/<id>/assets/done            the same ask again, once the file is sent: the object is read back, its size and
//                                                SHA-256 are checked against its name (a wrong object is deleted), and it is
//                                                imported into dev's pack. The answer is the list with what it did, or 422 in words
//
// This file moves facts in and answers out. The rules are Coax: who may write, the revision
// fence, the undo log and the dev and live rule are world-policy.coax's (#authority); whether an
// edit may stand is the world's own code, the code its page runs (Splice:
// metaverse/splice/world-api.coax, linked as splice-world). Every byte this file answers is
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

#define WORLD_DOC 131072     // a document's bytes at most (metaverse/splice/world.coax SPW_DOC)
#define WORLD_BATCH 65536    // a batch's (world-edit.coax WE_BATCH_BODY)
#define WORLD_TABLES 16384   // the content tables' bytes at most (metaverse/splice/world.coax SPW_TABLES)
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

// ---- the world's own Coax, by kind. Splice is the first world; a kind this build has no code for is not served.
static bool kind_known(const char *kind) { return !strcmp(kind, "splice"); }
static bool kind_pins(const char *kind, long long pins[4]) {
  int64_t p[4] = {0};
  if (strcmp(kind, "splice") || !splice_world_pins((coax_v_s64){p, 4})) return false;
  for (int k = 0; k < 4; k++) pins[k] = p[k];
  return true;
}
// The document a world is before its first edit, into out (malloc'd by the caller, WORLD_DOC): its length, or -1.
static int64_t kind_empty(const char *kind, char *out) {
  if (strcmp(kind, "splice")) return -1;
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
  int64_t lane = wr_lane(cs(lane_s));
  int64_t join = wr_join(sent, mine[0], mine[1], mine[2], mine[3], w.exists && w.live_rev > 0, w.live_pins[0], w.live_pins[1], w.live_pins[2], w.live_pins[3],
                         w.exists && w.prev_rev > 0, w.prev_pins[0], w.prev_pins[1], w.prev_pins[2], w.prev_pins[3]);
  int64_t verdict = wr_read(w.exists && kind_known(w.kind) && wr_loads(cs(id)), lane, join);
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

static void world_post(const request *q, PGconn *db, const char *id) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]), admin = p && p->admin;
  int64_t head[3] = {0, 0, 0};
  bool batch_ok = q->body_len > 0 && q->body_len <= WORLD_BATCH && wr_batch(csn(q->body, q->body_len), (coax_v_s64){head, 3});
  PQclear(PQexec(db, "BEGIN"));
  world_row w;
  if (!row_read(db, id, true, &w)) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); return; }
  int64_t what = head[1];
  int64_t verdict = wr_write(auth_same_site(q), known, admin, w.exists && kind_known(w.kind) && wr_authored(cs(id), what), batch_ok, what, head[0], w.dev_rev, w.done, w.steps);
  if (verdict != 0) { rollback(db); refuse_verdict(q->fd, verdict); row_free(&w); return; }
  if (wr_publishes(what)) { world_publish(q, db, id, &w, what, head[0]); row_free(&w); return; }

  // the batch to carry out: the caller's own, or a stored step taken again (an undo its inverse, a redo its batch)
  char *text = NULL; size_t text_n = 0; bool fenced = true;
  int64_t step = wr_step(what, w.done);
  if (step >= 0) {
    char ns[24]; snprintf(ns, sizeof ns, "%lld", (long long)step);
    const char *v[2] = {id, ns};
    PGresult *r = PQexecParams(db, wr_backward(what) ? "SELECT inv FROM verse.world_steps WHERE world = $1 AND n = $2::int" : "SELECT did FROM verse.world_steps WHERE world = $1 AND n = $2::int", 2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { text = strdup(PQgetvalue(r, 0, 0)); text_n = strlen(text); }
    PQclear(r);
    fenced = false;
  } else { text = malloc(q->body_len + 1); if (text) { memcpy(text, q->body, q->body_len); text[q->body_len] = 0; text_n = q->body_len; } }
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
    long long rev = w.dev_rev + n[4], done = wr_done(what, w.done), steps = wr_steps(what, w.done, w.steps), now = (long long)api_now_ms();
    char rev_s[24], done_s[24], steps_s[24], at_s[24];
    snprintf(rev_s, sizeof rev_s, "%lld", rev); snprintf(done_s, sizeof done_s, "%lld", done); snprintf(steps_s, sizeof steps_s, "%lld", steps); snprintf(at_s, sizeof at_s, "%lld", w.done);
    const char *uv[5] = {id, out_doc, rev_s, done_s, steps_s};
    bool ok = run(db, "UPDATE verse.world_docs SET dev_doc = $2, dev_rev = $3::bigint, done = $4::int, steps = $5::int WHERE id = $1", 5, uv);
    // a batch that changed the content tables saves them with the document, in the same transaction
    const char *tv[2] = {id, out_tables};
    if (ok && n[6] > 0) ok = run(db, "UPDATE verse.world_docs SET dev_tables = $2 WHERE id = $1", 2, tv);
    if (ok && step < 0) {
      // a new step where the log stands: the steps an undo left ahead of it go
      const char *sv[4] = {id, at_s, did, inv};
      ok = run(db, "DELETE FROM verse.world_steps WHERE world = $1 AND n >= $2::int", 2, sv)
        && run(db, "INSERT INTO verse.world_steps (world, n, did, inv) VALUES ($1, $2::int, $3, $4)", 4, sv);
    }
    // one event a saved edit, in the transaction that changes the document
    for (int64_t i = 0; ok && i < n[4]; i++) {
      char edit[WORLD_EDIT + 1], event[WORLD_EDIT + 240], seq[24];
      int64_t en = wr_batch_edit(csn(did, (size_t)n[1]), i, (coax_v_u8){(uint8_t *)edit, WORLD_EDIT});
      ok = en > 0 && next_seq(db, seq);
      if (!ok) break;
      int64_t vn = wr_event(atoll(seq), cs(id), w.dev_rev + 1 + i, csn(edit, (size_t)en), now, (coax_v_u8){(uint8_t *)event, sizeof event - 1});
      ok = vn > 0;
      if (ok) { event[vn] = 0; ok = outbox(db, seq, wr_event_id(false), id, "dev", w.dev_rev + 1 + i, event, now); }
    }
    ok = ok && run(db, "COMMIT", 0, NULL);
    if (!ok) { rollback(db); refuse(q->fd, 500, cs("not saved"), -1); goto done; }
    long long here[4] = {0, 0, 0, 0};
    kind_pins(w.kind, here);
    answer_state(q, id, (int)wr_lane(cs("dev")), rev, here, &w, rev, w.live_rev, done, steps, "", 0, did, (size_t)n[1]);
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
  if (out && !strcmp(id, "splice") && wr_loads(cs(id)) && row_read(db, id, false, &w)) {
    const char *own = w.exists ? (dev ? w.dev_tables : w.live_tables) : "";
    n = splice_world_tables(cs(own ? own : ""), (coax_v_u8){(uint8_t *)out, WORLD_DOC});
    row_free(&w);
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
  if (!strcmp(id, "splice") && wr_loads(cs(id)) && row_read(db, id, false, &w)) {
    size_t dn = 0;
    char *doc = doc_of("splice", w.exists ? w.dev_doc : "", &dn);
    int64_t things = doc ? splice_world_things(csn(doc, dn), cs(w.exists && w.dev_tables ? w.dev_tables : "")) : -1;
    if (things >= 0) n = wr_card(cs(id), w.exists ? w.dev_rev : 0, things, splice_world_name(), (coax_v_u8){(uint8_t *)out, sizeof out});
    free(doc); row_free(&w);
  }
  if (n < 0) refuse_verdict(q->fd, wr_read(false, 0, 0));
  else api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", out, (size_t)n);
}

// ---- a world's imported assets (#13420; docs/protocol/world-assets-v1.md). The importer, its limits and every byte
// answered are Coax (metaverse/splice/world-assets.coax, the code the dev server runs); who may import and what makes a
// stored object the file that was named are world-policy.coax's. The bucket's part is drops.c's.

// Whoever asks may import into this world: the verdict (0: yes), by the rule a write is held to.
static int64_t asset_may(const request *q, const world_row *w, const char *id) {
  const auth_principal *p = auth_principal_of(q);
  bool known = p && (p->account[0] || p->pubkey[0]), admin = p && p->admin;
  return wr_asset_write(auth_same_site(q), known, admin, w->exists && kind_known(w->kind) && wr_loads(cs(id)) && wr_authored(cs(id), wr_asset_what()));
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
  int64_t verdict = api_query(q, "write", write, sizeof write) && write[0] == '1' ? asset_may(q, &w, id) : wr_read(w.exists && kind_known(w.kind) && wr_loads(cs(id)), wr_lane(cs("dev")), 0);
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
  if (wr_read(w.exists && kind_known(w.kind) && wr_loads(cs(id)), wr_lane(cs("dev")), 0) != 0) { refuse_verdict(q->fd, wr_read(false, 0, 0)); row_free(&w); return; }
  // by hash: the bytes a tab was told of; else the lane's as it now is
  char live[80] = "";
  bool by_hash = api_query(q, "live", live, sizeof live) && strlen(live) == 64 && strspn(live, "0123456789abcdef") == 64;
  const char *hash = by_hash ? live : asset_lane(q, &w);
  size_t n = 0;
  char *pack = *hash ? pack_read(db, id, hash, &n) : NULL;
  if (!pack || !n) refuse(q->fd, 404, cs("this world has imported nothing"), -1);
  else api_respond(q->fd, 200, "application/octet-stream", NULL, pack, n);
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

bool worlds_route(const request *q) {
  static const char root[] = "/api/worlds/";
  size_t rl = sizeof root - 1;
  if (strncmp(q->path, root, rl)) return false;
  bool get = !strcmp(q->method, "GET"), post = !strcmp(q->method, "POST");
  char id[40] = ""; const char *rest = q->path + rl; size_t k = 0;
  while (rest[k] && rest[k] != '/' && k < sizeof id - 1) { id[k] = rest[k]; k++; }
  id[k] = 0;
  const char *tail = rest + k;
  bool content = !strcmp(tail, "/content"), card = !strcmp(tail, "/card");
  bool assets = !strcmp(tail, "/assets"), pack = !strcmp(tail, "/assets/pack"), done = !strcmp(tail, "/assets/done");
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
