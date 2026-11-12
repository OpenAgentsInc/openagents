// An account's life: freeze, merge, export and deletion (docs/auth/identity-and-methods.md#merging-accounts,
// docs/auth/data-and-backups.md#data-export-and-account-deletion, #13214). What may happen, and the
// words people see, are auth-life-policy.coax's; this file gathers its facts and moves its answers.
// The rows are 0149_auth_life.sql's, whose constraints refuse a purge sooner than 30 days and a
// merge into oneself whatever this file does.
//
//   GET  /api/auth/life                  the account's state: frozen or waiting to be deleted (and until when),
//                                        its ways in, agents, credit, realms and saves, the session's proof
//   POST /api/auth/freeze                (auth.c) any login, or a notice's link: every login ends, agents pause
//   POST /api/auth/unfreeze              a freeze lifts (L2-fresh; a fresh email login on an email-only account)
//   POST /api/auth/delete                frozen at once, purged 30 days on (L2-fresh, or a fresh email login)
//   POST /api/auth/delete/cancel         from any login of the account, at any level, until the purge
//   GET  /api/auth/export[?check=1]      everything we hold about you as JSON lines (L2; any login on an
//                                        email-only account); ?check=1 answers whether it may, not the file
//   POST /api/auth/merge/start           the account that stays asks (fresh proof): 10 minutes to prove the other
//   POST /api/auth/merge/prove?c=...     {proof}: the other account's key signed (purpose "merge"); or
//                                        POST /api/auth/email/start {email, merge: true} and /email/verify
//                                        when the other account has nothing stronger than its email
//   POST /api/auth/merge/confirm         {money}: both proofs fresh; the other account's credit moves only with money
//
// A frozen account's sessions work (it can log in to unfreeze, cancel a deletion or delete), but
// the gate lets them do only what al_frozen_route allows: no spending, no agents, no changes.
#include "auth-internal.h"
#include "auth-life-policy.generated.h"
#include "auth-policy.generated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

enum { AL_OK_ = 0, AL_RT_OTHER_ = 0, AL_RT_LOOK_ = 1, AL_RT_UNFREEZE_ = 2, AL_RT_STEP_UP_ = 3, AL_RT_DELETE_ = 4,
       AL_RT_CANCEL_ = 5, AL_RT_LOG_OUT_ = 6, AL_RT_EXPORT_ = 7 };
enum { V_OK = 0, V_LOG_IN = 1, V_HUMAN_ONLY = 2, V_STEP_UP = 3, V_FRESH_EMAIL = 4, V_NOT_FROZEN = 5, V_DELETING = 6,
       V_NOT_DELETING = 7, V_SAME = 8, V_OTHER_GONE = 9, V_OTHER_PROOF = 10, V_MONEY = 11, V_NOT_PROVED = 12,
       V_PROOF_OLD = 13, V_FROZEN = 14, V_GONE = 15 };

// The numbers above against auth-life-policy.coax's, at start.
bool auth_life_init(void) {
  return al_status(V_OK) == 200 && al_status(V_STEP_UP) == 401 && al_status(V_FRESH_EMAIL) == 401 && al_status(V_FROZEN) == 423
      && al_status(V_GONE) == 410 && al_status(V_OTHER_PROOF) == 403 && al_proof(1, 0, 0, false) == V_LOG_IN
      && al_proof(2, 0, 0, false) == V_HUMAN_ONLY && al_proof(3, 1, 0, false) == V_STEP_UP && al_proof(3, 0, -1, true) == V_FRESH_EMAIL
      && al_unfreeze(1, false, 0) == V_OK && al_unfreeze(0, false, 0) == V_NOT_FROZEN && al_unfreeze(1, true, 0) == V_DELETING
      && al_cancel(AP_PERSON, false, false) == V_NOT_DELETING && al_merge(0, true, AP_PERSON, false, true, 0, 0, false) == V_SAME
      && al_merge(0, false, AP_PERSON, true, true, 0, 0, false) == V_OTHER_GONE && al_merge(0, false, AP_PERSON, false, false, 0, 0, false) == V_NOT_PROVED
      && al_merge(0, false, AP_PERSON, false, true, -1, 0, false) == V_PROOF_OLD && al_merge(0, false, AP_PERSON, false, true, 0, 1, false) == V_MONEY
      && al_frozen_route(AL_RT_OTHER_, 1) == V_FROZEN && al_frozen_route(AL_RT_OTHER_, 2) == V_DELETING
      && al_frozen_route(AL_RT_LOOK_, 1) == V_OK && al_frozen_route(AL_RT_UNFREEZE_, 1) == V_OK && al_frozen_route(AL_RT_STEP_UP_, 1) == V_OK
      && al_frozen_route(AL_RT_DELETE_, 1) == V_OK && al_frozen_route(AL_RT_CANCEL_, 2) == V_OK && al_frozen_route(AL_RT_LOG_OUT_, 2) == V_OK
      && al_frozen_route(AL_RT_EXPORT_, 2) == V_OK && al_frozen_route(AL_RT_EXPORT_, 1) == V_FROZEN && al_merge_proof(false, 1) == V_OTHER_PROOF;
}

// ---------------------------------------------------------------- small helpers
static bool one(PGconn *db, const char *sql, int n, const char *const *v, char *out, size_t cap) {
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) >= 1;
  if (out && cap) { out[0] = 0; if (ok && !PQgetisnull(r, 0, 0)) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0)); }
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-life: %.60s: %s", sql, PQerrorMessage(db));
  PQclear(r);
  return ok;
}
static void say(const request *q, int verdict) {
  coax_string w = al_words(verdict);
  char words[240]; snprintf(words, sizeof words, "%.*s", (int)w.count, w.data);
  auth_refuse(q, (int)al_status(verdict), words);
}
static void utc(long long ms, char *out, size_t cap) {
  time_t t = (time_t)(ms / 1000); struct tm tm; gmtime_r(&t, &tm);
  strftime(out, cap, "%Y-%m-%d %H:%M UTC", &tm);
}
static char *body_of(const request *q) {
  char *b = malloc(q->body_len + 1);
  if (!b) return NULL;
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  return b;
}
static bool body_true(const char *body, const char *key) {
  char pat[40]; snprintf(pat, sizeof pat, "\"%s\":true", key);
  char flat[400]; size_t k = 0;
  for (const char *c = body; *c && k + 1 < sizeof flat; c++) if (*c != ' ' && *c != '\n' && *c != '\t') flat[k++] = *c;
  flat[k] = 0;
  return strstr(flat, pat) != NULL;
}

// The account's facts (auth.life_of).
typedef struct {
  bool found; char kind[16]; int why; long long frozen_ms, asked_ms, purge_ms, now_ms;
  int strong, emails, keys, agents, paused, worlds, saves, merged_from; long long credit_msat; bool deleted; char merged_into[40];
} life;
static life life_of(PGconn *db, const char *acct) {
  life l; memset(&l, 0, sizeof l);
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT kind, frozen_ms, frozen_why, delete_asked_ms, purge_ms, now_ms, strong, emails, keys, agents, "
                                 "agents_paused, credit_msat, worlds, saves, merged_from, deleted, merged_into FROM auth.life_of($1)", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    l.found = true;
    snprintf(l.kind, sizeof l.kind, "%s", PQgetvalue(r, 0, 0));
    l.frozen_ms = atoll(PQgetvalue(r, 0, 1));
    const char *w = PQgetvalue(r, 0, 2);
    l.why = !strcmp(w, "freeze") ? 1 : !strcmp(w, "delete") ? 2 : 0;
    l.asked_ms = atoll(PQgetvalue(r, 0, 3)); l.purge_ms = atoll(PQgetvalue(r, 0, 4)); l.now_ms = atoll(PQgetvalue(r, 0, 5));
    l.strong = atoi(PQgetvalue(r, 0, 6)); l.emails = atoi(PQgetvalue(r, 0, 7)); l.keys = atoi(PQgetvalue(r, 0, 8));
    l.agents = atoi(PQgetvalue(r, 0, 9)); l.paused = atoi(PQgetvalue(r, 0, 10)); l.credit_msat = atoll(PQgetvalue(r, 0, 11));
    l.worlds = atoi(PQgetvalue(r, 0, 12)); l.saves = atoi(PQgetvalue(r, 0, 13)); l.merged_from = atoi(PQgetvalue(r, 0, 14));
    l.deleted = !strcmp(PQgetvalue(r, 0, 15), "t");
    snprintf(l.merged_into, sizeof l.merged_into, "%s", PQgetvalue(r, 0, 16));
  } else if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth-life: life_of: %s", PQerrorMessage(db));
  PQclear(r);
  return l;
}
static int kind_of(const char *k) { return !strcmp(k, "person") ? AP_PERSON : !strcmp(k, "agent") ? AP_AGENT : !strcmp(k, "service") ? AP_SERVICE : AP_NOBODY; }

// The session's proof for a step: auth-policy.coax's step-up at L2 (or L2-fresh) on an account with
// a stronger way in; on an email-only account, a login by email within five minutes (fresh).
static int proof_of(const auth_principal *p, int strong, bool fresh) {
  int v = (int)ap_admit(fresh ? AP_L2_FRESH : AP_L2, p->kind, p->level, p->l2_age_ms, p->admin, false);
  if (v == AP_OK && p->limited) v = AP_LIMITED;
  return (int)al_proof(v, strong, p->l1_age_ms, fresh);
}
// Refuses a verdict; a step-up the page can act on goes through the shared answer (auth-recover.c).
static void refuse_proof(const request *q, PGconn *db, const auth_principal *p, int verdict, bool fresh) {
  if (verdict == V_STEP_UP) {
    coax_string w = al_words(verdict);
    char words[240]; snprintf(words, sizeof words, "%.*s", (int)w.count, w.data);
    auth_step_up_refusal(q, db, p, fresh ? 3 : 2, 401, words);
    return;
  }
  say(q, verdict);
}

// ---------------------------------------------------------------- the gate's check (auth.c)
static int route_kind(const char *method, const char *path) {
  bool get = !strcmp(method, "GET") || !strcmp(method, "HEAD");
  if (strncmp(path, "/api/auth/", 10)) return AL_RT_OTHER_;
  const char *op = path + 10;
  static const char *const looks[] = {"account", "life", "ways", "notices", "recovery", "sessions", "limits", NULL};
  if (get) {
    if (!strcmp(op, "export")) return AL_RT_EXPORT_;
    for (const char *const *o = looks; *o; o++) if (!strcmp(op, *o)) return AL_RT_LOOK_;
    return AL_RT_OTHER_;
  }
  if (strcmp(method, "POST")) return AL_RT_OTHER_;
  if (!strcmp(op, "freeze")) return AL_RT_LOOK_;   // freezing again changes nothing
  if (!strcmp(op, "unfreeze")) return AL_RT_UNFREEZE_;
  if (!strcmp(op, "step-up") || !strcmp(op, "proof/challenge")) return AL_RT_STEP_UP_;
  // Logging in proves who you are, as a step-up does: a frozen account logs in to unfreeze or cancel
  // (a new session is still held to these routes; email/start refuses linking a new way in).
  static const char *const logins[] = {"nostr/challenge", "nostr", "email/start", "email/verify", "email/link", "recovery-code/login", "refresh", NULL};
  for (const char *const *o = logins; *o; o++) if (!strcmp(op, *o)) return AL_RT_STEP_UP_;
  if (!strcmp(op, "delete")) return AL_RT_DELETE_;
  if (!strcmp(op, "delete/cancel") || !strcmp(op, "recovery/cancel")) return AL_RT_CANCEL_;
  if (!strcmp(op, "logout") || !strcmp(op, "logout-everywhere") || (!strncmp(op, "sessions/", 9) && strstr(op, "/remove"))) return AL_RT_LOG_OUT_;
  return AL_RT_OTHER_;
}
bool auth_life_frozen_refused(const request *q, const auth_principal *p) {
  if (!p || !p->frozen) return false;
  int v = (int)al_frozen_route(route_kind(q->method, q->path), p->frozen);
  if (v == V_OK) return false;
  api_log("auth: refused %s %s: the account is %s", q->method, q->path, p->frozen == 2 ? "waiting to be deleted" : "frozen");
  say(q, v);
  return true;
}

// ---------------------------------------------------------------- freeze and unfreeze
void auth_life_froze(PGconn *db, const char *acct, bool by_link) {
  int paused = agents_pause_all(db, acct);
  char words[600];
  snprintf(words, sizeof words, "Your OpenAgents account was frozen%s. Every login on it ended%s, and nothing can be spent from it.\n\n"
           "To unfreeze it, log in and open %s/settings/data. Unfreezing asks for your key or passkey "
           "(or a new email code if you have neither). Agents stay paused until you resume each one.",
           by_link ? " from the link in a security notice" : "", paused ? " and its agents were paused" : "", auth_origin());
  auth_notice(db, acct, "freeze", "your account was frozen", words);
}
static void unfreeze(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  int v = (int)al_unfreeze(l.why, l.asked_ms > 0, proof_of(p, l.strong, true));
  if (v != V_OK) { auth_audit(db, p->account, p->session, "unfreeze", "refused", "{}"); refuse_proof(q, db, p, v, true); return; }
  const char *a[1] = {p->account};
  char ok[8];
  one(db, "SELECT auth.unfreeze($1)", 1, a, ok, sizeof ok);
  if (strcmp(ok, "t")) { say(q, V_NOT_FROZEN); return; }
  auth_audit(db, p->account, p->session, "unfreeze", "ok", "{}");
  auth_notice(db, p->account, "unfreeze", "your account was unfrozen",
              "Your OpenAgents account was unfrozen: it can log in and spend again. Its agents stay paused until you resume each one on Settings, Agents.");
  auth_answer(q, 200, NULL, "{\"state\":\"unfrozen\"}", "state unfrozen\n");
}

// ---------------------------------------------------------------- deletion
static void delete_ask(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  int v = (int)al_delete(kind_of(l.kind), l.asked_ms > 0, l.deleted, proof_of(p, l.strong, true));
  if (v != V_OK) { auth_audit(db, p->account, p->session, "delete.ask", "refused", "{}"); refuse_proof(q, db, p, v, true); return; }
  const char *a[1] = {p->account};
  char purge[24];
  if (!one(db, "SELECT auth.delete_ask($1)", 1, a, purge, sizeof purge) || !purge[0]) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  long long purge_ms = atoll(purge);
  agents_pause_all(db, p->account);
  char when[40], detail[120], words[900];
  utc(purge_ms, when, sizeof when);
  snprintf(detail, sizeof detail, "{\"purge_ms\":%lld}", purge_ms);
  auth_audit(db, p->account, p->session, "delete.ask", "ok", detail);
  snprintf(words, sizeof words, "Your OpenAgents account will be deleted on %s. Until then it is frozen: every login on it ended and its agents are paused.\n\n"
           "Changed your mind? Log in any way you can (email, key or passkey) and press CANCEL DELETION on %s/settings/delete before then.",
           when, auth_origin());
  auth_notice(db, p->account, "delete", "your account will be deleted", words);
  char json[200], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"deleting\",\"purge_ms\":%lld,\"purge_at\":\"%s\"}", purge_ms, when);
  snprintf(lines, sizeof lines, "state deleting\npurge_ms %lld\npurge_at %s\n", purge_ms, when);
  // Every session ended with it, this one too: its cookie goes.
  auth_answer(q, 200, "Set-Cookie: __Host-oa_session=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", json, lines);
}
static void delete_cancel(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  int v = (int)al_cancel(p->kind, l.asked_ms > 0, l.deleted);
  if (v != V_OK) { say(q, v); return; }
  const char *a[1] = {p->account};
  char ok[8];
  one(db, "SELECT auth.delete_cancel($1)", 1, a, ok, sizeof ok);
  if (strcmp(ok, "t")) { say(q, V_NOT_DELETING); return; }
  char detail[60]; snprintf(detail, sizeof detail, "{\"via\":\"%.16s\"}", p->via);
  auth_audit(db, p->account, p->session, "delete.cancel", "ok", detail);
  auth_notice(db, p->account, "delete.cancel", "your account won't be deleted",
              "The deletion of your OpenAgents account was cancelled. It works again. Its agents stay paused until you resume each one on Settings, Agents.");
  auth_answer(q, 200, NULL, "{\"state\":\"kept\"}", "state kept\n");
}

// ---------------------------------------------------------------- the account's state
static void life_get(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  if (!l.found) { auth_refuse(q, 404, "No such account."); return; }
  int fresh = proof_of(p, l.strong, true), any = proof_of(p, l.strong, false);
  const char *why = l.why == 1 ? "freeze" : l.why == 2 ? "delete" : "";
  char when[40] = "";
  if (l.purge_ms > 0) utc(l.purge_ms, when, sizeof when);
  char json[1500], lines[1100];
  snprintf(json, sizeof json,
           "{\"account\":\"%s\",\"kind\":\"%s\",\"frozen\":\"%s\",\"frozen_ms\":%lld,\"delete_asked_ms\":%lld,\"purge_ms\":%lld,\"now_ms\":%lld,"
           "\"strong\":%d,\"emails\":%d,\"keys\":%d,\"agents\":%d,\"agents_paused\":%d,\"credit_msat\":%lld,\"worlds\":%d,\"saves\":%d,"
           "\"merged_from\":%d,\"purge_at\":\"%s\",\"level\":\"L%d\",\"limited\":%s,\"fresh\":%s,\"proved\":%s,\"purge_days\":30,\"backup_days\":35,\"backups_gone_days\":40}",
           p->account, l.kind, why, l.frozen_ms, l.asked_ms, l.purge_ms, l.now_ms, l.strong, l.emails, l.keys, l.agents, l.paused, l.credit_msat,
           l.worlds, l.saves, l.merged_from, when, p->level, p->limited ? "true" : "false", fresh == V_OK ? "true" : "false", any == V_OK ? "true" : "false");
  snprintf(lines, sizeof lines,
           "account %s\nkind %s\nfrozen %s\nfrozen_ms %lld\ndelete_asked_ms %lld\npurge_ms %lld\nnow_ms %lld\nstrong %d\nemails %d\nkeys %d\n"
           "agents %d\nagents_paused %d\ncredit_msat %lld\nworlds %d\nsaves %d\nmerged_from %d\npurge_at %s\nlevel %d\nlimited %d\nfresh %d\nproved %d\n",
           p->account, l.kind, why[0] ? why : "no", l.frozen_ms, l.asked_ms, l.purge_ms, l.now_ms, l.strong, l.emails, l.keys, l.agents, l.paused,
           l.credit_msat, l.worlds, l.saves, l.merged_from, when[0] ? when : "-", p->level, p->limited, fresh == V_OK, any == V_OK);
  auth_answer(q, 200, "Cache-Control: no-store\r\n", json, lines);
}

// ---------------------------------------------------------------- export
typedef struct { char *b; size_t n, cap; bool bad; } buf;
static void put(buf *o, const char *s, size_t n) {
  if (o->bad) return;
  if (o->n + n + 2 > o->cap) {
    size_t c = o->cap ? o->cap * 2 : 1 << 16;
    while (c < o->n + n + 2) c *= 2;
    if (c > (size_t)64 << 20) { o->bad = true; return; }   // 64 MB: far beyond any account today
    char *t = realloc(o->b, c);
    if (!t) { o->bad = true; return; }
    o->b = t; o->cap = c;
  }
  memcpy(o->b + o->n, s, n); o->n += n; o->b[o->n] = 0;
}
static void line(buf *o, const char *s) { put(o, s, strlen(s)); put(o, "\n", 1); }
static void lines_of(buf *o, PGconn *db, const char *sql, const char *acct) {
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, sql, 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { api_log("auth-life: export: %s", PQerrorMessage(db)); o->bad = true; }
  else for (int i = 0; i < PQntuples(r); i++) line(o, PQgetvalue(r, i, 0));
  PQclear(r);
}
static void export_file(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  int v = (int)al_export(kind_of(l.kind), l.why, proof_of(p, l.strong, false));
  if (v != V_OK) { refuse_proof(q, db, p, v, false); return; }
  char check[8] = "";
  if (api_query(q, "check", check, sizeof check) && !strcmp(check, "1")) { auth_answer(q, 200, NULL, "{\"state\":\"ready\"}", "state ready\n"); return; }
  buf o = {0};
  char head[600], e[400];
  snprintf(head, sizeof head, "{\"type\":\"export\",\"format\":\"openagents.export.v1\",\"account\":\"%s\",\"made_ms\":%lld,"
           "\"docs\":\"%s/docs/auth/data-and-backups#the-export\"}", p->account, l.now_ms, auth_origin());
  line(&o, head);
  const char *a[1] = {p->account};
  lines_of(&o, db, "SELECT jsonb_build_object('type', 'account', 'account', a.id, 'kind', a.kind, 'created_at', a.created_at, "
                   "'verse_account', a.verse_account, 'display_name', a.display_name, 'team', a.team, 'frozen_at', a.frozen_at, "
                   "'delete_asked_at', a.delete_asked_at, 'purge_at', a.purge_at)::text FROM auth.accounts a WHERE a.id = $1", p->account);
  // The emails themselves: each opened from its seal, here only.
  PGresult *r = PQexecParams(db, "SELECT id, sealed, created_at FROM auth.emails WHERE account = $1 AND sealed IS NOT NULL ORDER BY id", 1, NULL, a, NULL, NULL, 0);
  char ad[96]; snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
  for (int i = 0; PQresultStatus(r) == PGRES_TUPLES_OK && i < PQntuples(r); i++) {
    char email[300], row[800];
    if (!auth_unseal(ad, PQgetvalue(r, i, 1), email, sizeof email)) continue;
    auth_json_esc(email, e, sizeof e);
    snprintf(row, sizeof row, "{\"type\":\"email\",\"id\":%s,\"address\":\"%s\",\"added_at\":\"%s\"}", PQgetvalue(r, i, 0), e, PQgetvalue(r, i, 2));
    line(&o, row);
    memset(email, 0, sizeof email); memset(row, 0, sizeof row);
  }
  PQclear(r);
  // Agents: their proto/ message (openagents.account.v1alpha1.AgentList, agents.proto), base64.
  char *agents = agents_export_b64(db, p->account);
  if (agents) {
    static const char start[] = "{\"type\":\"agents\",\"message\":\"openagents.account.v1alpha1.AgentList\",\"base64\":\"";
    put(&o, start, sizeof start - 1);
    put(&o, agents, strlen(agents));
    line(&o, "\"}");
    free(agents);
  }
  lines_of(&o, db, "SELECT auth.export_rows($1)", p->account);
  lines_of(&o, db, "SELECT auth.export_audit($1)", p->account);
  if (o.bad) { free(o.b); auth_refuse(q, 500, "The export didn't finish. Try again."); return; }
  char headers[300];
  snprintf(headers, sizeof headers, "Content-Disposition: attachment; filename=\"openagents-%s.jsonl\"\r\nCache-Control: no-store\r\n", p->account);
  char detail[60]; snprintf(detail, sizeof detail, "{\"bytes\":%zu}", o.n);
  auth_audit(db, p->account, p->session, "export", "ok", detail);
  api_respond(q->fd, 200, "application/jsonl; charset=utf-8", headers, o.b, o.n);
  memset(o.b, 0, o.n);
  free(o.b);
}

// ---------------------------------------------------------------- merge
static void merge_start(const request *q, PGconn *db, const auth_principal *p) {
  life l = life_of(db, p->account);
  int v = (int)al_merge_start(kind_of(l.kind), l.why, proof_of(p, l.strong, true));
  if (v != V_OK) { refuse_proof(q, db, p, v, true); return; }
  char id[33];
  if (!auth_random_hex(id, 16)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  const char *w[3] = {id, p->account, p->session};
  if (!p->session[0] || !one(db, "SELECT auth.merge_ask($1, $2, $3)", 3, w, NULL, 0)) { auth_refuse(q, 400, "Merge from a logged-in browser or app session."); return; }
  auth_audit(db, p->account, p->session, "merge.start", "ok", "{}");
  auth_answer(q, 200, NULL, "{\"state\":\"asked\",\"expires_in\":600}", "state asked\nexpires_in 600\n");
}
// The other account proved itself: what would move, for the confirmation.
static void proved(const request *q, PGconn *db, const char *survivor, const char *session, const char *other, const char *via) {
  const char *w[3] = {survivor, other, via};
  char state[16];
  one(db, "SELECT auth.merge_proved($1, $2, $3)", 3, w, state, sizeof state);
  if (!strcmp(state, "self")) { say(q, V_SAME); return; }
  if (strcmp(state, "ok")) { auth_refuse(q, 409, "Start the merge again: it lasts 10 minutes."); return; }
  life o = life_of(db, other);
  char detail[120]; snprintf(detail, sizeof detail, "{\"other\":\"%.39s\",\"via\":\"%s\"}", other, via);
  auth_audit(db, survivor, session, "merge.proved", "ok", detail);
  char json[600], lines[500];
  snprintf(json, sizeof json, "{\"state\":\"proved\",\"other\":\"%s\",\"emails\":%d,\"keys\":%d,\"agents\":%d,\"worlds\":%d,\"saves\":%d,\"credit_msat\":%lld,"
           "\"frozen\":%s,\"deleting\":%s}", other, o.emails, o.keys, o.agents, o.worlds, o.saves, o.credit_msat, o.why ? "true" : "false", o.asked_ms ? "true" : "false");
  snprintf(lines, sizeof lines, "state proved\nother %s\nemails %d\nkeys %d\nagents %d\nworlds %d\nsaves %d\ncredit_msat %lld\nfrozen %d\ndeleting %d\n",
           other, o.emails, o.keys, o.agents, o.worlds, o.saves, o.credit_msat, o.why != 0, o.asked_ms > 0);
  auth_answer(q, 200, "Set-Cookie: __Host-oa_pending=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", json, lines);
}
static void merge_prove(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  char pk[65];
  if (!auth_proof_key(q, db, p, "merge", body, pk)) return;
  const char *v[1] = {pk};
  char other[40];
  one(db, "SELECT account FROM auth.nostr_keys WHERE pubkey = $1", 1, v, other, sizeof other);
  if (!other[0]) { auth_refuse(q, 404, "No account holds that key. Sign with the other account's key."); return; }
  proved(q, db, p->account, p->session, other, "nostr");
}
void auth_life_merge_email(const request *q, PGconn *db, const char *survivor, const char *lookup) {
  const char *v[1] = {lookup};
  char other[40];
  one(db, "SELECT account FROM auth.emails WHERE lookup = $1", 1, v, other, sizeof other);
  if (!other[0]) { auth_refuse(q, 404, "No account has that email."); return; }
  life o = life_of(db, other);
  int verdict = (int)al_merge_proof(false, o.strong);
  if (verdict != V_OK) { auth_audit(db, survivor, NULL, "merge.proved", "refused", "{\"why\":\"has a key\"}"); say(q, verdict); return; }
  const auth_principal *p = auth_principal_of(q);
  proved(q, db, survivor, p ? p->session : "", other, "email");
}
static void merge_confirm(const request *q, PGconn *db, const auth_principal *p, const char *body) {
  life s = life_of(db, p->account);
  const char *w[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT id, other, proved_via, proof_age_ms FROM auth.merge_ready($1, $2)", 2, NULL, w, NULL, NULL, 0);
  bool got = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  char id[40] = "", other[40] = ""; long long age = -1;
  if (got) { snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 0)); snprintf(other, sizeof other, "%s", PQgetvalue(r, 0, 1)); age = atoll(PQgetvalue(r, 0, 3)); }
  PQclear(r);
  life o = got ? life_of(db, other) : (life){0};
  bool money = body_true(body, "money");
  int proof = (int)al_merge_start(kind_of(s.kind), s.why, proof_of(p, s.strong, true));
  // No proved other yet: nothing to judge it by, so the policy answers "not proved", not "gone".
  int v = (int)al_merge(proof, got && !strcmp(other, p->account), got ? kind_of(o.kind) : AP_PERSON,
                        o.deleted || o.merged_into[0] || o.asked_ms > 0 || o.why != 0, got, age, o.credit_msat, money);
  if (v != V_OK) {
    char detail[60]; snprintf(detail, sizeof detail, "{\"verdict\":%d}", v);
    auth_audit(db, p->account, p->session, "merge", "refused", detail);
    refuse_proof(q, db, p, v, true);
    return;
  }
  const char *m[3] = {p->account, other, money ? "t" : "f"};
  PQclear(PQexec(db, "BEGIN"));
  char result[2000];
  if (!one(db, "SELECT auth.merge($1, $2, $3::boolean)::text", 3, m, result, sizeof result) || !result[0]) {
    PQclear(PQexec(db, "ROLLBACK"));
    say(q, V_OTHER_GONE);
    return;
  }
  const char *d[1] = {id};
  one(db, "SELECT auth.merge_done($1)", 1, d, NULL, 0);
  PQclear(PQexec(db, "COMMIT"));
  char detail[200];
  snprintf(detail, sizeof detail, "{\"merged\":\"%.39s\",\"money\":%s}", other, money ? "true" : "false");
  auth_audit(db, p->account, p->session, "merge", "ok", detail);
  snprintf(detail, sizeof detail, "{\"into\":\"%.39s\"}", p->account);
  auth_audit(db, other, p->session, "merged", "ok", detail);
  auth_notice(db, p->account, "merge", "two accounts became one",
              "Two of your OpenAgents accounts were merged into this one: its emails, keys, agents, realms and saves are all here now, "
              "and the other account's history is kept with this one. If this wasn't you, freeze your account from the link below.");
  char json[2300], lines[200];
  snprintf(json, sizeof json, "{\"state\":\"merged\",\"merged\":\"%s\",\"result\":%s}", other, result);
  snprintf(lines, sizeof lines, "state merged\nmerged %s\n", other);
  auth_answer(q, 200, NULL, json, lines);
}

// ---------------------------------------------------------------- the routes
bool auth_life_route(const request *q, PGconn *db, const char *op) {
  static const char *const gets[] = {"life", "export", NULL};
  static const char *const posts[] = {"unfreeze", "delete", "delete/cancel", "merge/start", "merge/prove", "merge/confirm", NULL};
  bool get = false, post = false;
  for (const char *const *o = gets; *o; o++) if (!strcmp(op, *o)) get = true;
  for (const char *const *o = posts; *o; o++) if (!strcmp(op, *o)) post = true;
  if (!get && !post) return false;
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  bool is_get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD");
  if (get && !is_get) { auth_refuse(q, 405, "GET this."); return true; }
  if (post && strcmp(q->method, "POST")) { auth_refuse(q, 405, "POST this."); return true; }
  if (post && !auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return true; }
  if (p->kind != AP_PERSON) { say(q, V_HUMAN_ONLY); return true; }
  if (!strcmp(op, "life")) { life_get(q, db, p); return true; }
  if (!strcmp(op, "export")) { export_file(q, db, p); return true; }
  char *body = body_of(q);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return true; }
  if (!strcmp(op, "unfreeze")) unfreeze(q, db, p);
  else if (!strcmp(op, "delete")) delete_ask(q, db, p);
  else if (!strcmp(op, "delete/cancel")) delete_cancel(q, db, p);
  else if (!strcmp(op, "merge/start")) merge_start(q, db, p);
  else if (!strcmp(op, "merge/prove")) merge_prove(q, db, p, body);
  else if (!strcmp(op, "merge/confirm")) merge_confirm(q, db, p, body);
  memset(body, 0, q->body_len);
  free(body);
  return true;
}
