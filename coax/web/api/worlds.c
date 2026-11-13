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
//
// This file moves facts in and answers out. The rules are Coax: who may write, the revision
// fence, the undo log and the dev and live rule are world-policy.coax's (#authority); whether an
// edit may stand is the world's own code, the code its page runs (Splice:
// metaverse/splice/world-api.coax, linked as splice-world). Every byte this file answers is
// written by metaverse-engine/editor/world-edit.coax through world-policy.coax. SQL
// (migrations/0181_worlds.sql) stores the rows, holds the row lock for a write and keeps the
// outbox; it decides nothing. A saved edit is one outbox row, written in the transaction that
// changes the document (the domain event realm-authoring.world.edited.v1). A world is served only
// for a realm with a row in the authority table (metaverse-engine/editor/authority.coax, #13431):
// a write here is authoring, and nothing in this file reaches the character record or the ledger.
#include "api.h"
#include "world-policy.generated.h"
#include "splice-world.generated.h"

#include <stdlib.h>
#include <string.h>

#define WORLD_DOC 131072     // a document's bytes at most (metaverse/splice/world.coax SPW_DOC)
#define WORLD_BATCH 65536    // a batch's (world-edit.coax WE_BATCH_BODY)
#define WORLD_TABLES 16384   // the content tables' bytes at most (metaverse/splice/world.coax SPW_TABLES)
#define WORLD_EDIT 1400      // an edit's (WE_LINE)
#define WORLD_EVENTS 200     // events one read answers

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
} world_row;

static void row_free(world_row *w) { free(w->dev_doc); free(w->live_doc); free(w->prev_doc); free(w->dev_tables); free(w->live_tables); free(w->prev_tables); }
static bool row_read(PGconn *db, const char *id, bool lock, world_row *w) {
  memset(w, 0, sizeof *w);
  const char *v[1] = {id};
  PGresult *r = PQexecParams(db, lock
    ? "SELECT kind, dev_rev, dev_doc, done, steps, live_rev, live_from, live_doc, live_when, live_engine, live_rules, live_content, live_wire, prev_rev, prev_doc, prev_engine, prev_rules, prev_content, prev_wire, dev_tables, live_tables, prev_tables FROM verse.world_docs WHERE id = $1 FOR UPDATE"
    : "SELECT kind, dev_rev, dev_doc, done, steps, live_rev, live_from, live_doc, live_when, live_engine, live_rules, live_content, live_wire, prev_rev, prev_doc, prev_engine, prev_rules, prev_content, prev_wire, dev_tables, live_tables, prev_tables FROM verse.world_docs WHERE id = $1",
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
  // the revision before a pin change stays for one release (world-live.coax); a publish on the same pins leaves it
  bool ok = (!keeps || run(db, "UPDATE verse.world_docs SET prev_rev = live_rev, prev_doc = live_doc, prev_engine = live_engine, prev_rules = live_rules, prev_content = live_content, prev_wire = live_wire, prev_tables = live_tables WHERE id = $1", 1, sv))
    && run(db, "UPDATE verse.world_docs SET live_rev = $2::bigint, live_from = $3::bigint, live_doc = $4, live_when = $5::int, live_engine = $6::int, live_rules = $7::int, live_content = $8::int, live_wire = $9::int, live_tables = $10 WHERE id = $1", 10, sv)
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
  char why[240]; int64_t n[7] = {0, 0, 0, 0, 0, -1, 0};
  if (!text || !doc || !out_doc || !did || !inv || !out_tables) { rollback(db); refuse(q->fd, 500, cs("not read"), -1); goto done; }
  int64_t status = splice_world_batch(csn(doc, doc_n), cs(w.dev_tables ? w.dev_tables : ""), csn(text, text_n), fenced, (coax_v_u8){(uint8_t *)out_doc, WORLD_DOC},
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
  free(text); free(doc); free(out_doc); free(did); free(inv); free(out_tables); row_free(&w);
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
  if ((!get && !post) || !wr_world_ok(cs(id)) || (*tail && strcmp(tail, "/events") && !content && !card) || (*tail && !get)) { refuse(q->fd, 404, cs("not found"), -1); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q->fd, 503, cs("no database"), -1); return true; }
  if (content) world_content(q, db, id);
  else if (card) world_card(q, db, id);
  else if (*tail) world_events(q, db, id);
  else if (get) world_get(q, db, id);
  else world_post(q, db, id);
  return true;
}
