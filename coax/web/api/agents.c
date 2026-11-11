// Agents: accounts that act for a person, their grants and keys, and `openagents login` (GitHub
// #13211, docs/auth/identity-and-methods.md#agents). An agent is an auth.accounts row of kind
// 'agent' owned by a person (migration 0147). It proves who it is with its own Nostr key, signing
// each request with NIP-98, or with an oa_key_ API key (only its SHA-256 is kept). What it may do is
// its grant: scopes, spend caps in sats, the addresses it may call from and when it ends (90 days
// unless the owner picks another, at most a year), approved by its owner at L2.
//
// Every decision is agent-policy.coax (#authority); every message is
// openagents.account.v1alpha1 (proto/openagents/account/v1alpha1/agents.proto), read and written by
// agent-wire.coax; the owner's NIP-OA attestation is checked by agent-attest.coax. This file holds
// no rule of its own: it reads rows, asks those, writes rows and the audit log.
//
// The gate (auth.c) refuses an agent on every human, L2, L2-fresh and admin route, and calls
// agents_admit on every request an agent makes: a grant that ended, was revoked or paused, or an
// address it doesn't list, is refused before any route runs; agents_used writes the audit row.
//
// Routes. The protobuf ones take and answer application/x-protobuf; ?as=b64 takes and answers the
// same bytes in base64 (the Coax pages' fetch reads text), and refusals are then the words alone.
//   GET  /api/agents                   human     AgentList: your agents, their grants and keys
//   POST /api/agents                   L2        GrantRequest -> KeySecret: a new agent (an oa_key_
//                                                 when the request names no Nostr key)
//   POST /api/agents/request           NIP-98    GrantRequest signed by the agent's new key -> RequestTicket
//                                                 (its code, and /settings/agents#request=CODE for the owner)
//   POST /api/agents/request/poll      NIP-98    RequestTicket{code} by that key -> its state (and the agent account)
//   GET  /api/agents/request/CODE      human     RequestTicket with the GrantRequest, for the page
//   POST /api/agents/approve/CODE      L2        GrantRequest (the caps the person set; empty: as asked) -> Agent
//   POST /api/agents/deny/CODE         human
//   POST /api/agents/revoke/ACCT       human     the grant ends now, and the agent's API keys
//   POST /api/agents/revoke-all        human     every agent's
//   POST /api/agents/rotate/ACCT       L2        KeySecret: a new oa_key_; the agent's other ones end
//   POST /api/agents/grant/ACCT        L2        GrantRequest: change it (more than before: L2 within 5 minutes)
//   POST /api/agents/resume/ACCT       L2        after "log out everywhere" paused it
//   GET  /api/agent-keys/PUBKEY        public    the key's NIP-OA tag ["auth", owner, conditions, sig]
//   GET  /api/agent/me                 agent     Agent: its own grant
//   GET  /api/agent/owner              profile:read   OwnerProfile
//   POST /api/agent/spend              wallet:spend   SpendAsk -> SpendAnswer (each payment id counts once)
//   POST /api/auth/device/start {machine}     the CLI's new key, NIP-98 -> {id, code, expires_ms} (0098's table)
//   GET  /api/auth/device/ID                  that key -> waiting | approved | expired
//   POST /api/auth/device/look {code}         what /login/device shows: the computer's name
//   POST /api/auth/device/approve {code}      a person: the CLI's key joins the account as cli:<machine>
//
// Nothing secret is logged or audited: an oa_key_ is shown once, in the answer that made it.
#include "api.h"
#include "agent-wire.generated.h"
#include "agent-attest.generated.h"

#include <openssl/evp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// agent-policy.coax's numbers (agents_init checks them at start).
enum { AG_PROFILE_READ = 1, AG_WALLET_SPEND = 2 };
enum { AG_ACTIVE = 1, AG_EXPIRED = 2, AG_REVOKED = 3, AG_SUSPENDED = 4, AG_PENDING = 5 };
enum { AG_OK = 0, AG_NOT_ACTIVE = 10, AG_WRONG_HOST = 11, AG_NO_SCOPE = 12, AG_OVER_DAY = 15, AG_STEP_UP = 16,
       AG_BAD_WIRE = 17, AG_BAD_KEY = 18, AG_BAD_ATTESTATION = 19, AG_FRESH = 20 };
#define AG_MAX_BODY 4096
#define REQUEST_TTL "1 hour"

static coax_string cs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_string csn(const uint8_t *b, int64_t n) { return (coax_string){(const char *)b, n}; }
static void text_of(const uint8_t *b, int64_t n, char *out, size_t cap) { snprintf(out, cap, "%.*s", (int)n, (const char *)b); }
static void words_of(int verdict, char *out, size_t cap) { coax_string w = ag_words(verdict); snprintf(out, cap, "%.*s", (int)w.count, w.data); }

bool agents_init(void) {
  return ag_bit(AG_WALLET_SPEND) == 2 && ag_status(AG_NOT_ACTIVE) == 401 && ag_status(AG_NO_SCOPE) == 403 && ag_status(AG_FRESH) == 401
      && ag_status(AG_BAD_ATTESTATION) == 400 && ag_state(0, 1, 0, false, false) == AG_ACTIVE && ag_state(0, 1, 5, false, false) == AG_REVOKED
      && ag_agent(AG_SUSPENDED, 0, false) == AG_NOT_ACTIVE && ag_link(1, true) == AG_STEP_UP && ag_spend(2, 5, 5, 9, 5) == AG_OVER_DAY;
}

// ---------------------------------------------------------------- the wire
static bool b64(const request *q) { char v[8]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "b64"); }
// The request's message bytes (raw, or base64 under ?as=b64) into out: their count, or -1.
static int body_bytes(const request *q, unsigned char *out, size_t cap) {
  if (!b64(q)) {
    if (q->body_len > cap) return -1;
    memcpy(out, q->body, q->body_len);
    return (int)q->body_len;
  }
  size_t n = q->body_len;
  while (n && (q->body[n - 1] == '\n' || q->body[n - 1] == '\r' || q->body[n - 1] == ' ')) n--;
  if (n == 0) return 0;
  if (n % 4 || n / 4 * 3 > cap + 2) return -1;
  unsigned char tmp[AG_MAX_BODY + 4];
  int got = EVP_DecodeBlock(tmp, (const unsigned char *)q->body, (int)n);
  if (got < 0) return -1;
  if (q->body[n - 1] == '=') got--;
  if (q->body[n - 2] == '=') got--;
  if ((size_t)got > cap) return -1;
  memcpy(out, tmp, (size_t)got);
  return got;
}
static void send_bytes(const request *q, int status, const unsigned char *b, size_t n) {
  if (!b64(q)) { api_respond(q->fd, status, "application/x-protobuf", "Cache-Control: no-store\r\n", b, n); return; }
  char *t = malloc(4 * ((n + 2) / 3) + 1);
  if (!t) { api_json(q->fd, 500, "{\"error\":\"memory\"}"); return; }
  int k = EVP_EncodeBlock((unsigned char *)t, b, (int)n);
  api_respond(q->fd, status, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", t, (size_t)k);
  free(t);
}
static void say_no(const request *q, int status, const char *words) {
  if (b64(q)) api_respond(q->fd, status, "text/plain; charset=utf-8", NULL, words, strlen(words));
  else auth_refuse(q, status, words);
}
static void say_verdict(const request *q, int verdict) { char w[200]; words_of(verdict, w, sizeof w); say_no(q, (int)ag_status(verdict), w); }
static void say_done(const request *q, const char *json, const char *lines) {
  if (b64(q)) api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, lines, strlen(lines));
  else auth_answer(q, 200, NULL, json, lines);
}

// One statement with text parameters; NULL (logged) unless it returned rows.
static PGresult *sql(PGconn *db, const char *what, const char *query, int n, const char *const *v) {
  PGresult *r = PQexecParams(db, query, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK || PQresultStatus(r) == PGRES_COMMAND_OK) return r;
  api_log("agents: %s: %s", what, PQerrorMessage(db));
  PQclear(r);
  return NULL;
}
static bool exec(PGconn *db, const char *query) { PGresult *r = PQexec(db, query); bool ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r); return ok; }
static void hex_of(const unsigned char *b, size_t n, char *out) {
  static const char d[] = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) { out[2 * i] = d[b[i] >> 4]; out[2 * i + 1] = d[b[i] & 15]; }
  out[2 * n] = 0;
}
static int unhex(const char *s, unsigned char *out, size_t cap) {
  size_t n = strlen(s) / 2;
  if (n > cap) return -1;
  for (size_t i = 0; i < n; i++) {
    int v = 0;
    for (int k = 0; k < 2; k++) { char c = s[2 * i + k]; v = v * 16 + (c >= 'a' ? c - 'a' + 10 : c - '0'); }
    out[i] = (unsigned char)v;
  }
  return (int)n;
}
// A request's addresses as Postgres's text[] literal (ag_host_ok let only digits, a-f, '.', ':' and '/' through).
static void hosts_literal(const coax_t_AwRequest *r, char *out, size_t cap) {
  size_t k = (size_t)snprintf(out, cap, "{");
  for (int64_t i = 0; i < r->nhosts && k < cap; i++)
    k += (size_t)snprintf(out + k, cap - k, "%s%.*s", i ? "," : "", (int)r->hosts.e[i].n, (const char *)r->hosts.e[i].b.e);
  if (k < cap) snprintf(out + k, cap - k, "}");
}
// The account and path id after a prefix: "acct_" + 26 base32, or false.
static bool acct_ok(const char *s) {
  if (strlen(s) != 31 || strncmp(s, "acct_", 5)) return false;
  for (const char *c = s + 5; *c; c++) if (!((*c >= 'a' && *c <= 'z') || (*c >= '2' && *c <= '7'))) return false;
  return true;
}

// ---------------------------------------------------------------- the gate's side (auth.c)
bool agents_key(PGconn *db, const char *digest, auth_principal *p) {
  const char *v[1] = {digest};
  PGresult *r = sql(db, "key", "UPDATE auth.agent_keys k SET last_used_at = now() FROM auth.accounts a "
                               "WHERE k.token_sha256 = $1 AND k.revoked_at IS NULL AND a.id = k.agent AND a.kind = 'agent' AND a.deleted_at IS NULL "
                               "RETURNING k.id, a.id, a.verse_account, a.team", 1, v);
  bool ok = r && PQntuples(r) == 1;
  if (ok) {
    memset(p, 0, sizeof *p);
    p->kind = AP_AGENT; p->credential = AUTH_BEARER; p->level = 1; p->l2_age_ms = -1;
    snprintf(p->key_id, sizeof p->key_id, "%s", PQgetvalue(r, 0, 0));
    snprintf(p->account, sizeof p->account, "%s", PQgetvalue(r, 0, 1));
    snprintf(p->verse_account, sizeof p->verse_account, "%s", PQgetvalue(r, 0, 2));
    p->team = !strcmp(PQgetvalue(r, 0, 3), "t");
    snprintf(p->via, sizeof p->via, "key");
  }
  if (r) PQclear(r);
  return ok;
}

// The agent's grant, decided now (agent-policy.coax): 0, or the refusal's status and words.
int agents_admit(const request *q, PGconn *db, auth_principal *p, char words[200]) {
  if (!db) { snprintf(words, 200, "The database isn't answering. Try again."); return 503; }
  const char *v[1] = {p->account};
  PGresult *r = sql(db, "facts", "SELECT grant_id, owner, scopes, per_day, per_payment, array_to_string(hosts, ' '), expires_ms, revoked_ms, "
                                 "suspended, now_ms FROM auth.agent_facts($1)", 1, v);
  int verdict = AG_NOT_ACTIVE;
  if (r && PQntuples(r) == 1) {
    int state = (int)ag_state(atoll(PQgetvalue(r, 0, 9)), atoll(PQgetvalue(r, 0, 6)), atoll(PQgetvalue(r, 0, 7)), false, !strcmp(PQgetvalue(r, 0, 8), "t"));
    char ip[128]; auth_visitor(q, ip);
    char hosts[700]; snprintf(hosts, sizeof hosts, "%s", PQgetvalue(r, 0, 5));
    int n = 0; bool matched = false;
    for (char *h = strtok(hosts, " "); h; h = strtok(NULL, " ")) { n++; if (ag_host_match(cs(ip), cs(h))) matched = true; }
    verdict = (int)ag_agent(state, n, matched);
    snprintf(p->grant, sizeof p->grant, "%s", PQgetvalue(r, 0, 0));
    snprintf(p->owner, sizeof p->owner, "%s", PQgetvalue(r, 0, 1));
    p->scopes = atoll(PQgetvalue(r, 0, 2));
    p->per_day = atoll(PQgetvalue(r, 0, 3));
    p->per_payment = atoll(PQgetvalue(r, 0, 4));
  }
  if (r) PQclear(r);
  if (verdict == AG_OK) return 0;
  if (p->owner[0]) {
    char d[120]; snprintf(d, sizeof d, "{\"why\":\"%s\"}", verdict == AG_WRONG_HOST ? "address" : "grant");
    auth_audit(db, p->owner, p->account, "agent.use", "refused", d);
  }
  memset(p->grant, 0, sizeof p->grant); p->scopes = 0;
  words_of(verdict, words, 200);
  return (int)ag_status(verdict);
}
bool agents_scoped(const auth_principal *p, int scope) { return ag_scope(p->scopes, scope) == AG_OK; }

void agents_used(const request *q, PGconn *db, const auth_principal *p) {
  if (!db || !p->grant[0]) return;
  char path[96]; size_t k = 0;   // the route, without anything but path characters
  for (const char *c = q->path; *c && k + 1 < sizeof path; c++)
    path[k++] = (*c >= 'a' && *c <= 'z') || (*c >= 'A' && *c <= 'Z') || (*c >= '0' && *c <= '9') || *c == '/' || *c == '_' || *c == '-' || *c == '.' ? *c : '?';
  path[k] = 0;
  char d[200]; snprintf(d, sizeof d, "{\"method\":\"%.7s\",\"path\":\"%s\",\"via\":\"%s\"}", q->method, path, p->via);
  auth_audit(db, p->owner, p->account, "agent.use", "ok", d);
  const char *v[1] = {p->grant};
  PGresult *r = sql(db, "used", "UPDATE auth.agent_grants SET last_used_at = now() WHERE id = $1", 1, v);
  if (r) PQclear(r);
}

int agents_pause_all(PGconn *db, const char *owner) {
  const char *v[1] = {owner};
  PGresult *r = sql(db, "pause", "UPDATE auth.agent_grants SET suspended_at = now() WHERE owner = $1 AND revoked_at IS NULL AND suspended_at IS NULL", 1, v);
  int n = r ? atoi(PQcmdTuples(r)) : 0;
  if (r) PQclear(r);
  if (n) auth_audit(db, owner, NULL, "agent.pause", "ok", "{\"all\":true}");
  return n;
}

// ---------------------------------------------------------------- an agent and its grant
// The rows of auth.agents_of as Agent messages, appended to an AgentList in list (n: its length).
static int list_rows(PGresult *r, unsigned char *list, size_t cap) {
  unsigned char *a = malloc(cap), *b = malloc(cap), *one = malloc(AG_MAX_BODY);
  if (!a || !b || !one) { free(a); free(b); free(one); return -1; }
  int n = 0; unsigned char *cur = a, *next = b;
  for (int i = 0; i < PQntuples(r); i++) {
    coax_t_AwAgent g; memset(&g, 0, sizeof g);
    g.account_n = (int64_t)snprintf((char *)g.account.e, sizeof g.account.e, "%s", PQgetvalue(r, i, 0));
    g.name_n = (int64_t)snprintf((char *)g.name.e, sizeof g.name.e, "%s", PQgetvalue(r, i, 1));
    g.grant_n = (int64_t)snprintf((char *)g.grant_id.e, sizeof g.grant_id.e, "%s", PQgetvalue(r, i, 2));
    g.mask = atoll(PQgetvalue(r, i, 3)); g.per_day = atoll(PQgetvalue(r, i, 4)); g.per_payment = atoll(PQgetvalue(r, i, 5));
    char hosts[700]; snprintf(hosts, sizeof hosts, "%s", PQgetvalue(r, i, 6));
    for (char *h = strtok(hosts, " "); h && g.nhosts < 8; h = strtok(NULL, " ")) {
      coax_t_AwText *t = &g.hosts.e[g.nhosts++];
      t->n = (int64_t)snprintf((char *)t->b.e, sizeof t->b.e, "%s", h);
    }
    g.created_ms = atoll(PQgetvalue(r, i, 7)); g.expires_ms = atoll(PQgetvalue(r, i, 8));
    g.state = ag_state(atoll(PQgetvalue(r, i, 13)), g.expires_ms, atoll(PQgetvalue(r, i, 9)), false, !strcmp(PQgetvalue(r, i, 10), "t"));
    g.spent = atoll(PQgetvalue(r, i, 11)); g.last_used_ms = atoll(PQgetvalue(r, i, 12));
    unsigned char rec[2048]; int rn = unhex(PQgetvalue(r, i, 14), rec, sizeof rec);
    if (rn > 0) {
      coax_t_AwRequest q = aw_request_read((coax_v_u8){rec, rn});
      if (q.owner_n == 64) {
        memcpy(g.att_owner.e, q.owner.e, 64); g.att_owner_n = 64;
        memcpy(g.att_conditions.e, q.conditions.e, (size_t)q.conditions_n); g.att_conditions_n = q.conditions_n;
        memcpy(g.att_sig.e, q.sig.e, (size_t)q.sig_n); g.att_sig_n = q.sig_n;
      }
    }
    char keys[4096]; snprintf(keys, sizeof keys, "%s", PQgetvalue(r, i, 15));
    char *save = NULL;
    for (char *line = strtok_r(keys, "\n", &save); line && g.nkeys < 16; line = strtok_r(NULL, "\n", &save)) {
      char id[80]; int api = 0; long long c = 0, u = 0, x = 0;
      if (sscanf(line, "%79s %d %lld %lld %lld", id, &api, &c, &u, &x) != 5) continue;
      coax_t_AwKey *k = &g.keys.e[g.nkeys++];
      k->id_n = (int64_t)snprintf((char *)k->id.e, sizeof k->id.e, "%s", id);
      k->api = api == 1; k->created_ms = c; k->last_used_ms = u; k->revoked_ms = x;
    }
    int64_t on = aw_agent_write(g, (coax_v_u8){one, AG_MAX_BODY});
    if (on < 0) continue;
    int64_t m = aw_list_append((coax_v_u8){cur, n}, (coax_v_u8){one, on}, (coax_v_u8){next, (int64_t)cap});
    if (m < 0) break;
    unsigned char *t = cur; cur = next; next = t; n = (int)m;
  }
  memcpy(list, cur, (size_t)n);
  free(a); free(b); free(one);
  return n;
}
static PGresult *agents_rows(PGconn *db, const char *owner, const char *one) {
  const char *v[2] = {owner, one};
  return sql(db, "agents", "SELECT account, name, grant_id, scopes, per_day, per_payment, hosts, created_ms, expires_ms, revoked_ms, suspended, "
                           "spent, last_used_ms, now_ms, record, keys FROM auth.agents_of($1, $2)", 2, v);
}
static void send_list(const request *q, PGconn *db, const char *owner) {
  PGresult *r = agents_rows(db, owner, NULL);
  if (!r) { say_no(q, 500, "That didn't work. Try again."); return; }
  size_t cap = 32 * AG_MAX_BODY;
  unsigned char *list = malloc(cap);
  int n = list ? list_rows(r, list, cap) : -1;
  PQclear(r);
  if (n < 0) say_no(q, 500, "That didn't work. Try again.");
  else send_bytes(q, 200, list, (size_t)n);
  free(list);
}
// One agent (the first row) as an Agent message.
static void send_one(const request *q, PGconn *db, const char *owner, const char *agent) {
  PGresult *r = agents_rows(db, owner, agent);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "No such agent."); return; }
  unsigned char list[AG_MAX_BODY * 2];
  int n = list_rows(r, list, sizeof list);
  PQclear(r);
  coax_t_AwAgent g = aw_list_at((coax_v_u8){list, n}, 0);
  unsigned char one[AG_MAX_BODY];
  int64_t m = g.error ? -1 : aw_agent_write(g, (coax_v_u8){one, sizeof one});
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, 200, one, (size_t)m);
}

// A request's attestation, when it has one, checked: the owner key is one of owner's own keys (not
// an agent's) and signed this agent key (agent-attest.coax). AG_OK, or AG_BAD_ATTESTATION.
static int attestation(PGconn *db, const char *owner, const coax_t_AwRequest *r) {
  if (r->owner_n == 0) return AG_OK;
  char ok[65], ak[65];
  text_of(r->owner.e, r->owner_n, ok, sizeof ok); text_of(r->pubkey.e, r->pubkey_n, ak, sizeof ak);
  if (aa_verify(cs(ok), cs(ak), csn(r->conditions.e, r->conditions_n), csn(r->sig.e, r->sig_n)) != 0) return AG_BAD_ATTESTATION;
  const char *v[2] = {ok, owner};
  PGresult *x = sql(db, "attest", "SELECT 1 FROM auth.nostr_keys WHERE pubkey = $1 AND account = $2", 2, v);
  bool mine = x && PQntuples(x) == 1;
  if (x) PQclear(x);
  return mine ? AG_OK : AG_BAD_ATTESTATION;
}

// A new agent for owner from an approved request (checked). When the request names no Nostr key,
// an oa_key_ is made: its id and secret in key_id and secret. The agent's account, or false (and
// *status, *words: 409 a key that is an account's already, 500).
static bool make_agent(PGconn *db, const char *owner, const coax_t_AwRequest *req, char agent[40], char key_id[9], char secret[160],
                       int *status, const char **words) {
  unsigned char rec[2048];
  int64_t rn = aw_request_write(*req, (coax_v_u8){rec, sizeof rec});
  *status = 500; *words = "That didn't work. Try again.";
  if (rn < 0) return false;
  char rechex[4100]; hex_of(rec, (size_t)rn, rechex);
  char name[100]; text_of(req->name.e, req->name_n, name, sizeof name);
  char pk[65]; text_of(req->pubkey.e, req->pubkey_n, pk, sizeof pk);
  char mask[24], day[24], pay[24], days[16], hosts[700];
  snprintf(mask, sizeof mask, "%lld", (long long)req->mask); snprintf(day, sizeof day, "%lld", (long long)req->per_day);
  snprintf(pay, sizeof pay, "%lld", (long long)req->per_payment); snprintf(days, sizeof days, "%lld", (long long)ag_days(req->days));
  hosts_literal(req, hosts, sizeof hosts);
  if (!exec(db, "BEGIN")) return false;
  // At most AG_MAX_AGENTS at once (agent-policy.coax); the owner's row is locked so two can't race past it.
  const char *v0[1] = {owner};
  PGresult *c = sql(db, "room", "SELECT (SELECT count(*) FROM auth.agent_grants WHERE owner = $1 AND revoked_at IS NULL) "
                                "FROM auth.accounts WHERE id = $1 FOR UPDATE", 1, v0);
  int room = c && PQntuples(c) == 1 ? (int)ag_room(atoll(PQgetvalue(c, 0, 0))) : AG_BAD_WIRE;
  if (c) PQclear(c);
  if (room != AG_OK) {
    exec(db, "ROLLBACK");
    static char words_room[200];
    words_of(room, words_room, sizeof words_room);
    *status = room == AG_BAD_WIRE ? 500 : (int)ag_status(room); *words = room == AG_BAD_WIRE ? *words : words_room;
    return false;
  }
  const char *v1[2] = {owner, name};
  PGresult *r = sql(db, "new agent", "SELECT auth.new_agent($1, $2)", 2, v1);
  bool ok = r && PQntuples(r) == 1;
  if (ok) snprintf(agent, 40, "%s", PQgetvalue(r, 0, 0));
  if (r) PQclear(r);
  if (ok && pk[0]) {
    const char *v2[2] = {agent, pk};
    r = PQexecParams(db, "INSERT INTO auth.nostr_keys (account, pubkey, label) VALUES ($1, $2, 'agent')", 2, NULL, v2, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_COMMAND_OK) {
      const char *st = PQresultErrorField(r, PG_DIAG_SQLSTATE);
      if (st && !strcmp(st, "23505")) { *status = 409; *words = "That key is an account's already."; }
      else api_log("agents: agent key: %s", PQerrorMessage(db));
      ok = false;
    }
    PQclear(r);
  }
  if (ok) {
    const char *v3[9] = {agent, owner, mask, day, pay, hosts, rechex, days, NULL};
    r = sql(db, "grant", "INSERT INTO auth.agent_grants (id, agent, owner, scopes, sats_per_day, sats_per_payment, hosts, record, expires_at) "
                         "VALUES ('grt_' || auth.random_b32(26), $1, $2, $3::int, $4::bigint, $5::bigint, $6::text[], decode($7, 'hex'), "
                         "now() + make_interval(days => $8::int))", 8, v3);
    ok = r != NULL;
    if (r) PQclear(r);
  }
  if (ok && !pk[0]) {
    char dig[65];
    ok = auth_new_key(key_id, secret, 160, dig);
    const char *v4[3] = {key_id, agent, dig};
    r = ok ? sql(db, "agent key", "INSERT INTO auth.agent_keys (id, agent, token_sha256) VALUES ($1, $2, $3)", 3, v4) : NULL;
    ok = r != NULL;
    if (r) PQclear(r);
  }
  if (!ok || !exec(db, "COMMIT")) { exec(db, "ROLLBACK"); if (secret) memset(secret, 0, 160); return false; }
  return true;
}
static void grant_detail(const coax_t_AwRequest *r, const char *extra, char *out, size_t cap) {
  snprintf(out, cap, "{\"scopes\":%lld,\"sats_per_day\":%lld,\"sats_per_payment\":%lld,\"days\":%lld,\"hosts\":%lld%s}",
           (long long)r->mask, (long long)r->per_day, (long long)r->per_payment, (long long)ag_days(r->days), (long long)r->nhosts, extra ? extra : "");
}

// POST /api/agents: the owner makes an agent directly (its key: the request's, or a new oa_key_).
static void create(const request *q, PGconn *db, const auth_principal *p) {
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  if (n < 0) { say_verdict(q, AG_BAD_WIRE); return; }
  coax_t_AwRequest req = aw_request_read((coax_v_u8){b, n});
  if (req.verdict != AG_OK) { say_verdict(q, (int)req.verdict); return; }
  int v = (int)ag_approve(req.mask, false, p->l2_age_ms);
  if (v == AG_OK) v = attestation(db, p->account, &req);
  if (v != AG_OK) { say_verdict(q, v); return; }
  char agent[40], key_id[9] = "", secret[160] = ""; int status; const char *words;
  if (!make_agent(db, p->account, &req, agent, key_id, secret, &status, &words)) { say_no(q, status, words); return; }
  char d[300], extra[80]; snprintf(extra, sizeof extra, ",\"agent\":\"%s\",\"key\":\"%s\"", agent, key_id[0] ? "api" : "nostr");
  grant_detail(&req, extra, d, sizeof d);
  auth_audit(db, p->account, p->session, "agent.grant", "ok", d);
  unsigned char out[512];
  int64_t m = aw_secret_write(cs(agent), cs(key_id), cs(secret), (coax_v_u8){out, sizeof out});
  memset(secret, 0, sizeof secret);
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, 200, out, (size_t)m);
  memset(out, 0, sizeof out);
}

// ---------------------------------------------------------------- an agent asks its owner
static void ticket_send(const request *q, coax_t_AwTicket *t) {
  unsigned char out[AG_MAX_BODY];
  int64_t m = aw_ticket_write(*t, (coax_v_u8){out, sizeof out});
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, 200, out, (size_t)m);
}
static void ask(const request *q, PGconn *db, const auth_principal *p) {
  if (!p || p->credential != AUTH_NOSTR || !p->payload) { say_no(q, 401, "Sign this with the agent's new key (NIP-98, with the body)."); return; }
  if (p->account[0]) { say_no(q, 409, "That key is an account's already."); return; }
  if (auth_limited(q, db)) return;
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  if (n < 0 || n > 2048) { say_verdict(q, AG_BAD_WIRE); return; }
  coax_t_AwRequest req = aw_request_read((coax_v_u8){b, n});
  if (req.verdict != AG_OK) { say_verdict(q, (int)req.verdict); return; }
  char pk[65]; text_of(req.pubkey.e, req.pubkey_n, pk, sizeof pk);
  if (pk[0] && strcmp(pk, p->pubkey)) { say_verdict(q, AG_BAD_KEY); return; }
  memcpy(req.pubkey.e, p->pubkey, 64); req.pubkey_n = 64;
  if (req.owner_n) {
    char ok[65]; text_of(req.owner.e, req.owner_n, ok, sizeof ok);
    if (aa_verify(cs(ok), cs(p->pubkey), csn(req.conditions.e, req.conditions_n), csn(req.sig.e, req.sig_n)) != 0) { say_verdict(q, AG_BAD_ATTESTATION); return; }
  }
  unsigned char rec[2048];
  int64_t rn = aw_request_write(req, (coax_v_u8){rec, sizeof rec});
  if (rn < 0) { say_verdict(q, AG_BAD_WIRE); return; }
  char rechex[4100]; hex_of(rec, (size_t)rn, rechex);
  const char *v[2] = {p->pubkey, rechex};
  PGresult *r = sql(db, "ask", "INSERT INTO auth.agent_requests (code, pubkey, request, expires_at) VALUES (upper(auth.random_b32(8)), $1, "
                               "decode($2, 'hex'), now() + interval '" REQUEST_TTL "') RETURNING code, (extract(epoch FROM expires_at) * 1000)::bigint", 2, v);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 500, "That didn't work. Try again."); return; }
  coax_t_AwTicket t; memset(&t, 0, sizeof t);
  memcpy(t.code.e, PQgetvalue(r, 0, 0), 8); t.code_n = 8;
  t.url_n = (int64_t)snprintf((char *)t.url.e, sizeof t.url.e, "%s/settings/agents#request=%.8s", auth_origin(), PQgetvalue(r, 0, 0));
  t.expires_ms = atoll(PQgetvalue(r, 0, 1));
  t.state_n = (int64_t)snprintf((char *)t.state.e, sizeof t.state.e, "waiting");
  PQclear(r);
  ticket_send(q, &t);
}
// The code from a RequestTicket body, or from the path's tail.
static bool code_from(const char *s, char code[9]) {
  snprintf(code, 9, "%s", s);
  return strlen(s) == 8 && ag_code_ok(cs(code));
}
// A request's row by code: pubkey, request hex, state (waiting, approved, denied, expired), agent.
static PGresult *request_row(PGconn *db, const char *code) {
  const char *v[1] = {code};
  return sql(db, "request", "SELECT pubkey, encode(request, 'hex'), CASE WHEN result IS NOT NULL THEN result WHEN expires_at <= now() THEN 'expired' "
                            "ELSE 'waiting' END, coalesce(agent, ''), (extract(epoch FROM expires_at) * 1000)::bigint "
                            "FROM auth.agent_requests WHERE code = $1", 1, v);
}
static void ask_poll(const request *q, PGconn *db, const auth_principal *p) {
  if (!p || p->credential != AUTH_NOSTR) { say_no(q, 401, "Sign this with the agent's key (NIP-98)."); return; }
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  coax_t_AwTicket in = n < 0 ? (coax_t_AwTicket){.error = AG_BAD_WIRE} : aw_ticket_read((coax_v_u8){b, n});
  char code[9] = "", given[9] = "";
  if (!in.error && in.code_n == 8) { memcpy(given, in.code.e, 8); given[8] = 0; }
  if (!code_from(given, code)) { say_no(q, 404, "No such request."); return; }
  PGresult *r = request_row(db, code);
  if (!r || PQntuples(r) != 1 || strcmp(PQgetvalue(r, 0, 0), p->pubkey)) { if (r) PQclear(r); say_no(q, 404, "No such request."); return; }
  coax_t_AwTicket t; memset(&t, 0, sizeof t);
  memcpy(t.code.e, code, 8); t.code_n = 8;
  t.state_n = (int64_t)snprintf((char *)t.state.e, sizeof t.state.e, "%s", PQgetvalue(r, 0, 2));
  t.label_n = (int64_t)snprintf((char *)t.label.e, sizeof t.label.e, "%s", PQgetvalue(r, 0, 3));
  t.expires_ms = atoll(PQgetvalue(r, 0, 4));
  PQclear(r);
  ticket_send(q, &t);
}
static void ask_show(const request *q, PGconn *db, const char *code) {
  PGresult *r = request_row(db, code);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "That code isn't good. Ask the agent for a new one."); return; }
  coax_t_AwTicket t; memset(&t, 0, sizeof t);
  memcpy(t.code.e, code, 8); t.code_n = 8;
  t.state_n = (int64_t)snprintf((char *)t.state.e, sizeof t.state.e, "%s", PQgetvalue(r, 0, 2));
  t.expires_ms = atoll(PQgetvalue(r, 0, 4));
  unsigned char rec[2048]; int rn = unhex(PQgetvalue(r, 0, 1), rec, sizeof rec);
  PQclear(r);
  if (rn > 0) { t.request = aw_request_read((coax_v_u8){rec, rn}); t.has_request = t.request.verdict == AG_OK; }
  ticket_send(q, &t);
}
static void ask_answer(const request *q, PGconn *db, const auth_principal *p, const char *code, bool approve) {
  PGresult *r = request_row(db, code);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "That code isn't good. Ask the agent for a new one."); return; }
  char state[16], hex[4100]; snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 2)); snprintf(hex, sizeof hex, "%s", PQgetvalue(r, 0, 1));
  PQclear(r);
  if (!strcmp(state, "expired")) { say_no(q, 410, "That request expired. Ask the agent for a new one."); return; }
  if (strcmp(state, "waiting")) { say_no(q, 409, "That request was answered already."); return; }
  if (!approve) {
    const char *v[2] = {code, p->account};
    PGresult *x = sql(db, "deny", "UPDATE auth.agent_requests SET result = 'denied', owner = $2, done_at = now() WHERE code = $1 AND result IS NULL", 2, v);
    if (x) PQclear(x);
    auth_audit(db, p->account, p->session, "agent.deny", "ok", "{}");
    say_done(q, "{\"state\":\"denied\"}", "state denied\n");
    return;
  }
  unsigned char rec[2048]; int rn = unhex(hex, rec, sizeof rec);
  coax_t_AwRequest asked = aw_request_read((coax_v_u8){rec, rn > 0 ? rn : 0});
  // The person may set other caps, scopes, addresses, end or name; never another key.
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  if (n < 0) { say_verdict(q, AG_BAD_WIRE); return; }
  coax_t_AwRequest req = n == 0 ? asked : aw_request_read((coax_v_u8){b, n});
  if (req.verdict != AG_OK) { say_verdict(q, (int)req.verdict); return; }
  memcpy(req.pubkey.e, asked.pubkey.e, 64); req.pubkey_n = asked.pubkey_n;
  if (req.owner_n == 0 && asked.owner_n) {
    req.owner = asked.owner; req.owner_n = asked.owner_n; req.conditions = asked.conditions; req.conditions_n = asked.conditions_n;
    req.sig = asked.sig; req.sig_n = asked.sig_n;
  }
  int v = asked.verdict != AG_OK ? AG_BAD_WIRE : (int)ag_approve(req.mask, false, p->l2_age_ms);
  if (v == AG_OK) v = attestation(db, p->account, &req);
  if (v != AG_OK) { say_verdict(q, v); return; }
  // Claim the request first, so two approvals can't make two agents.
  const char *c[2] = {code, p->account};
  PGresult *x = sql(db, "claim", "UPDATE auth.agent_requests SET result = 'approved', owner = $2, done_at = now() "
                                 "WHERE code = $1 AND result IS NULL AND expires_at > now() RETURNING code", 2, c);
  bool claimed = x && PQntuples(x) == 1;
  if (x) PQclear(x);
  if (!claimed) { say_no(q, 409, "That request was answered already."); return; }
  char agent[40], key_id[9] = "", secret[160] = ""; int status; const char *words;
  if (!make_agent(db, p->account, &req, agent, key_id, secret, &status, &words)) {
    const char *u[1] = {code};
    x = sql(db, "unclaim", "UPDATE auth.agent_requests SET result = NULL, owner = NULL, done_at = NULL WHERE code = $1", 1, u);
    if (x) PQclear(x);
    say_no(q, status, words);
    return;
  }
  const char *s[2] = {code, agent};
  x = sql(db, "approved", "UPDATE auth.agent_requests SET agent = $2 WHERE code = $1", 2, s);
  if (x) PQclear(x);
  char d[300], extra[80]; snprintf(extra, sizeof extra, ",\"agent\":\"%s\",\"key\":\"nostr\"", agent);
  grant_detail(&req, extra, d, sizeof d);
  auth_audit(db, p->account, p->session, "agent.grant", "ok", d);
  send_one(q, db, p->account, agent);
}

// ---------------------------------------------------------------- the owner changes an agent
static bool owns(PGconn *db, const char *owner, const char *agent) {
  const char *v[2] = {agent, owner};
  PGresult *r = sql(db, "owns", "SELECT 1 FROM auth.agent_grants WHERE agent = $1 AND owner = $2", 2, v);
  bool ok = r && PQntuples(r) == 1;
  if (r) PQclear(r);
  return ok;
}
static void revoke(const request *q, PGconn *db, const auth_principal *p, const char *agent) {
  const char *v[2] = {p->account, agent};
  PGresult *r = sql(db, "revoke", "WITH g AS (UPDATE auth.agent_grants SET revoked_at = now() WHERE owner = $1 AND ($2::text IS NULL OR agent = $2) "
                                  "AND revoked_at IS NULL RETURNING agent), k AS (UPDATE auth.agent_keys SET revoked_at = now() "
                                  "WHERE agent IN (SELECT agent FROM g) AND revoked_at IS NULL RETURNING 1) SELECT (SELECT count(*) FROM g)", 2, v);
  int n = r && PQntuples(r) == 1 ? atoi(PQgetvalue(r, 0, 0)) : -1;
  if (r) PQclear(r);
  if (n < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  if (agent && n == 0 && !owns(db, p->account, agent)) { say_no(q, 404, "No such agent."); return; }
  char d[80]; snprintf(d, sizeof d, "{\"revoked\":%d%s}", n, agent ? "" : ",\"all\":true");
  auth_audit(db, p->account, p->session, "agent.revoke", "ok", d);
  char json[64], lines[64]; snprintf(json, sizeof json, "{\"revoked\":%d}", n); snprintf(lines, sizeof lines, "revoked %d\n", n);
  say_done(q, json, lines);
}
// The agent's facts for the owner: grant id, scopes, caps, record hex, expires, revoked, suspended, now.
static PGresult *grant_row(PGconn *db, const char *owner, const char *agent) {
  const char *v[2] = {agent, owner};
  return sql(db, "grant row", "SELECT id, scopes, sats_per_day, sats_per_payment, encode(record, 'hex'), (extract(epoch FROM expires_at) * 1000)::bigint, "
                              "coalesce((extract(epoch FROM revoked_at) * 1000)::bigint, 0), suspended_at IS NOT NULL, (extract(epoch FROM now()) * 1000)::bigint "
                              "FROM auth.agent_grants WHERE agent = $1 AND owner = $2", 2, v);
}
static void rotate(const request *q, PGconn *db, const auth_principal *p, const char *agent) {
  PGresult *r = grant_row(db, p->account, agent);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "No such agent."); return; }
  bool revoked = atoll(PQgetvalue(r, 0, 6)) > 0;
  PQclear(r);
  if (revoked) { say_verdict(q, AG_NOT_ACTIVE); return; }
  char key_id[9], secret[160], dig[65];
  if (!auth_new_key(key_id, secret, sizeof secret, dig)) { say_no(q, 503, "No randomness here. Try again."); return; }
  const char *v[3] = {agent, key_id, dig};
  r = sql(db, "rotate", "WITH old AS (UPDATE auth.agent_keys SET revoked_at = now() WHERE agent = $1 AND revoked_at IS NULL RETURNING 1) "
                        "INSERT INTO auth.agent_keys (id, agent, token_sha256) SELECT $2, $1, $3 RETURNING (SELECT count(*) FROM old)", 3, v);
  bool ok = r && PQntuples(r) == 1;
  int ended = ok ? atoi(PQgetvalue(r, 0, 0)) : 0;
  if (r) PQclear(r);
  if (!ok) { memset(secret, 0, sizeof secret); say_no(q, 500, "That didn't work. Try again."); return; }
  char d[80]; snprintf(d, sizeof d, "{\"agent\":\"%s\",\"ended\":%d}", agent, ended);
  auth_audit(db, p->account, p->session, "agent.rotate", "ok", d);
  unsigned char out[512];
  int64_t m = aw_secret_write(cs(agent), cs(key_id), cs(secret), (coax_v_u8){out, sizeof out});
  memset(secret, 0, sizeof secret);
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, 200, out, (size_t)m);
  memset(out, 0, sizeof out);
}
// Every new address limit is one of the old ones (no limits at all is the widest).
static bool narrower(const coax_t_AwRequest *old, const coax_t_AwRequest *now) {
  if (old->nhosts == 0) return true;
  if (now->nhosts == 0) return false;
  for (int64_t i = 0; i < now->nhosts; i++) {
    bool found = false;
    for (int64_t j = 0; j < old->nhosts; j++)
      if (now->hosts.e[i].n == old->hosts.e[j].n && !memcmp(now->hosts.e[i].b.e, old->hosts.e[j].b.e, (size_t)now->hosts.e[i].n)) found = true;
    if (!found) return false;
  }
  return true;
}
static void change(const request *q, PGconn *db, const auth_principal *p, const char *agent) {
  PGresult *r = grant_row(db, p->account, agent);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "No such agent."); return; }
  long long old_mask = atoll(PQgetvalue(r, 0, 1)), old_day = atoll(PQgetvalue(r, 0, 2)), old_pay = atoll(PQgetvalue(r, 0, 3));
  long long old_end = atoll(PQgetvalue(r, 0, 5)), revoked = atoll(PQgetvalue(r, 0, 6)), now = atoll(PQgetvalue(r, 0, 8));
  unsigned char rec[2048]; int rn = unhex(PQgetvalue(r, 0, 4), rec, sizeof rec);
  PQclear(r);
  if (revoked > 0) { say_verdict(q, AG_NOT_ACTIVE); return; }
  coax_t_AwRequest old = aw_request_read((coax_v_u8){rec, rn > 0 ? rn : 0});
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  coax_t_AwRequest req = n <= 0 ? (coax_t_AwRequest){.verdict = AG_BAD_WIRE} : aw_request_read((coax_v_u8){b, n});
  if (req.verdict != AG_OK) { say_verdict(q, (int)req.verdict); return; }
  // The key and its attestation stay what was approved.
  req.pubkey = old.pubkey; req.pubkey_n = old.pubkey_n; req.owner = old.owner; req.owner_n = old.owner_n;
  req.conditions = old.conditions; req.conditions_n = old.conditions_n; req.sig = old.sig; req.sig_n = old.sig_n;
  long long new_end = (long long)ag_expires(now, req.days);
  bool widens = ag_widens(old_mask, req.mask, old_day, req.per_day, old_pay, req.per_payment, old_end, new_end, narrower(&old, &req));
  int v = (int)ag_approve(req.mask, widens, p->l2_age_ms);
  if (v != AG_OK) { say_verdict(q, v); return; }
  unsigned char out[2048];
  int64_t m = aw_request_write(req, (coax_v_u8){out, sizeof out});
  if (m < 0) { say_verdict(q, AG_BAD_WIRE); return; }
  char rechex[4100]; hex_of(out, (size_t)m, rechex);
  char mask[24], day[24], pay[24], days[16], hosts[700], name[100];
  snprintf(mask, sizeof mask, "%lld", (long long)req.mask); snprintf(day, sizeof day, "%lld", (long long)req.per_day);
  snprintf(pay, sizeof pay, "%lld", (long long)req.per_payment); snprintf(days, sizeof days, "%lld", (long long)ag_days(req.days));
  hosts_literal(&req, hosts, sizeof hosts); text_of(req.name.e, req.name_n, name, sizeof name);
  const char *w[9] = {agent, p->account, mask, day, pay, hosts, rechex, days, name};
  r = sql(db, "change", "WITH g AS (UPDATE auth.agent_grants SET scopes = $3::int, sats_per_day = $4::bigint, sats_per_payment = $5::bigint, "
                        "hosts = $6::text[], record = decode($7, 'hex'), approved_at = now(), expires_at = now() + make_interval(days => $8::int) "
                        "WHERE agent = $1 AND owner = $2 AND revoked_at IS NULL RETURNING agent) "
                        "UPDATE auth.accounts SET display_name = left($9, 80) WHERE id IN (SELECT agent FROM g) RETURNING id", 9, w);
  bool ok = r && PQntuples(r) == 1;
  if (r) PQclear(r);
  if (!ok) { say_no(q, 500, "That didn't work. Try again."); return; }
  char d[300], extra[80]; snprintf(extra, sizeof extra, ",\"agent\":\"%s\",\"wider\":%s", agent, widens ? "true" : "false");
  grant_detail(&req, extra, d, sizeof d);
  auth_audit(db, p->account, p->session, "agent.grant", "ok", d);
  send_one(q, db, p->account, agent);
}
static void resume(const request *q, PGconn *db, const auth_principal *p, const char *agent) {
  const char *v[2] = {agent, p->account};
  PGresult *r = sql(db, "resume", "UPDATE auth.agent_grants SET suspended_at = NULL WHERE agent = $1 AND owner = $2 AND revoked_at IS NULL RETURNING 1", 2, v);
  bool ok = r && PQntuples(r) == 1;
  if (r) PQclear(r);
  if (!ok) { say_no(q, 404, "No such agent."); return; }
  auth_audit(db, p->account, p->session, "agent.resume", "ok", "{}");
  send_one(q, db, p->account, agent);
}

// ---------------------------------------------------------------- what an agent asks
static void spend(const request *q, PGconn *db, const auth_principal *p) {
  unsigned char b[AG_MAX_BODY];
  int n = body_bytes(q, b, sizeof b);
  coax_t_AwSpendAsk ask = n < 0 ? (coax_t_AwSpendAsk){.verdict = AG_BAD_WIRE} : aw_spend_read((coax_v_u8){b, n});
  if (ask.verdict != AG_OK) { say_verdict(q, (int)ask.verdict); return; }
  char pid[80]; text_of(ask.payment_id.e, ask.payment_n, pid, sizeof pid);
  char sats[24]; snprintf(sats, sizeof sats, "%lld", (long long)ask.sats);
  if (!exec(db, "BEGIN")) { say_no(q, 500, "That didn't work. Try again."); return; }
  const char *v[2] = {p->grant, pid};
  // The grant's row is locked, so two payments at once can't both fit under the day's cap.
  PGresult *r = sql(db, "spend", "SELECT g.sats_per_day, g.sats_per_payment, auth.spent_24h(g.id), "
                                 "(SELECT sats FROM auth.agent_spends s WHERE s.grant_id = g.id AND s.payment_id = $2) "
                                 "FROM auth.agent_grants g WHERE g.id = $1 FOR UPDATE", 2, v);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); exec(db, "ROLLBACK"); say_no(q, 500, "That didn't work. Try again."); return; }
  long long day = atoll(PQgetvalue(r, 0, 0)), pay = atoll(PQgetvalue(r, 0, 1)), spent = atoll(PQgetvalue(r, 0, 2));
  bool seen = !PQgetisnull(r, 0, 3);
  long long seen_sats = seen ? atoll(PQgetvalue(r, 0, 3)) : 0;
  PQclear(r);
  int verdict;
  if (seen) verdict = seen_sats == ask.sats ? AG_OK : AG_BAD_WIRE;   // the same payment asked again counts once
  else {
    verdict = (int)ag_spend(p->scopes, ask.sats, pay, day, spent);
    if (verdict == AG_OK) {
      const char *w[3] = {p->grant, pid, sats};
      r = sql(db, "spent", "INSERT INTO auth.agent_spends (grant_id, payment_id, sats) VALUES ($1, $2, $3::bigint)", 3, w);
      if (!r) { exec(db, "ROLLBACK"); say_no(q, 500, "That didn't work. Try again."); return; }
      PQclear(r);
      spent += ask.sats;
    }
  }
  if (!exec(db, "COMMIT")) { exec(db, "ROLLBACK"); say_no(q, 500, "That didn't work. Try again."); return; }
  long long left = day - spent > 0 ? day - spent : 0;
  char d[160]; snprintf(d, sizeof d, "{\"sats\":%lld,\"again\":%s,\"left\":%lld}", (long long)ask.sats, seen ? "true" : "false", left);
  auth_audit(db, p->owner, p->account, "agent.spend", verdict == AG_OK ? "ok" : "refused", d);
  char words[200] = ""; if (verdict != AG_OK) words_of(verdict, words, sizeof words);
  unsigned char out[512];
  int64_t m = aw_answer_write(verdict == AG_OK, cs(words), left, (coax_v_u8){out, sizeof out});
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, verdict == AG_OK ? 200 : (int)ag_status(verdict), out, (size_t)m);
}
static void owner_profile(const request *q, PGconn *db, const auth_principal *p) {
  const char *v[1] = {p->owner};
  PGresult *r = sql(db, "owner", "SELECT id, coalesce(display_name, ''), kind FROM auth.accounts WHERE id = $1", 1, v);
  if (!r || PQntuples(r) != 1) { if (r) PQclear(r); say_no(q, 404, "No owner."); return; }
  unsigned char out[512];
  int64_t m = aw_owner_write(cs(PQgetvalue(r, 0, 0)), cs(PQgetvalue(r, 0, 1)), cs(PQgetvalue(r, 0, 2)), (coax_v_u8){out, sizeof out});
  PQclear(r);
  auth_audit(db, p->owner, p->account, "agent.owner", "ok", "{\"scope\":\"profile:read\"}");
  if (m < 0) { say_no(q, 500, "That didn't work. Try again."); return; }
  send_bytes(q, 200, out, (size_t)m);
}
// GET /api/agent-keys/PUBKEY: the NIP-OA tag its owner signed, published while its grant lasts.
static void published(const request *q, PGconn *db, const char *pk) {
  bool hex = strlen(pk) == 64;
  for (const char *c = pk; hex && *c; c++) hex = (*c >= '0' && *c <= '9') || (*c >= 'a' && *c <= 'f');
  if (!hex) { auth_refuse(q, 404, "No such agent key."); return; }
  const char *v[1] = {pk};
  PGresult *r = sql(db, "published", "SELECT encode(g.record, 'hex') FROM auth.nostr_keys k JOIN auth.agent_grants g ON g.agent = k.account "
                                     "WHERE k.pubkey = $1 AND k.label = 'agent' AND g.revoked_at IS NULL AND g.expires_at > now()", 1, v);
  unsigned char rec[2048]; int rn = r && PQntuples(r) == 1 ? unhex(PQgetvalue(r, 0, 0), rec, sizeof rec) : -1;
  if (r) PQclear(r);
  coax_t_AwRequest req = rn > 0 ? aw_request_read((coax_v_u8){rec, rn}) : (coax_t_AwRequest){.verdict = AG_BAD_WIRE};
  if (req.verdict != AG_OK || req.owner_n != 64) { auth_refuse(q, 404, "No attestation for that key."); return; }
  char json[800], lines[700];
  // The conditions read (noa_conditions): '&', '<', '>', '=', digits and letters, so no JSON escaping.
  snprintf(json, sizeof json, "{\"agent\":\"%s\",\"tag\":[\"auth\",\"%.64s\",\"%.*s\",\"%.128s\"]}", pk, (const char *)req.owner.e,
           (int)req.conditions_n, (const char *)req.conditions.e, (const char *)req.sig.e);
  snprintf(lines, sizeof lines, "agent %s\nowner %.64s\nconditions %.*s\nsig %.128s\n", pk, (const char *)req.owner.e,
           (int)req.conditions_n, (const char *)req.conditions.e, (const char *)req.sig.e);
  auth_answer(q, 200, "Cache-Control: public, max-age=60\r\n", json, lines);
}

// ---------------------------------------------------------------- `openagents login` (0098's table)
#define RESULT "SELECT (x->>'status')::int, (x->'body')::text, x->>'lines' FROM "
static void result(const request *q, PGconn *db, const char *query, int n, const char *const *v) {
  PGresult *r = sql(db, "device", query, n, v);
  if (!r || PQntuples(r) != 1) {
    if (r) PQclear(r);
    auth_refuse(q, 400, "That didn't read.");
    return;
  }
  int status = atoi(PQgetvalue(r, 0, 0));
  char lines_q[8]; bool lines = api_query(q, "as", lines_q, sizeof lines_q) && !strcmp(lines_q, "lines");
  if (lines && !PQgetisnull(r, 0, 2)) api_respond(q->fd, status, "text/plain; charset=utf-8", NULL, PQgetvalue(r, 0, 2), strlen(PQgetvalue(r, 0, 2)));
  else api_json(q->fd, status, PQgetvalue(r, 0, 1));
  PQclear(r);
}
static bool json_body(const request *q, char *out, size_t cap) {
  if (q->body_len == 0 || q->body_len >= cap) return false;
  memcpy(out, q->body, q->body_len); out[q->body_len] = 0;
  return true;
}
static void device(const request *q, PGconn *db, const auth_principal *p, const char *op) {
  bool post = !strcmp(q->method, "POST");
  char body[4096];
  if (!strcmp(op, "start") || (!post && strlen(op) == 16)) {   // the CLI's new key, its own signature only
    if (!p || p->credential != AUTH_NOSTR) { auth_refuse(q, 401, "Sign it with the new key (NIP-98)."); return; }
    if (p->kind == AP_AGENT) { auth_refuse(q, 403, "An agent's key can't log in a computer."); return; }
    if (!post) { const char *v[2] = {p->pubkey, op}; result(q, db, RESULT "auth.device_poll($1, $2) x", 2, v); return; }
    if (auth_limited(q, db)) return;
    if (!json_body(q, body, sizeof body)) { auth_refuse(q, 400, "Name the machine."); return; }
    const char *v[3] = {p->pubkey, body, "600000"};
    result(q, db, RESULT "auth.device_start($1, $2::jsonb, $3::bigint) x", 3, v);
    return;
  }
  if (!strcmp(op, "look") && post) {
    if (auth_limited(q, db)) return;
    if (!json_body(q, body, sizeof body)) { auth_refuse(q, 400, "Send the code."); return; }
    const char *v[1] = {body};
    result(q, db, RESULT "auth.device_look($1::jsonb) x", 1, v);
    return;
  }
  if (!strcmp(op, "approve") && post) {
    if (!p || p->kind != AP_PERSON || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return; }
    if (!auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return; }
    if (!json_body(q, body, sizeof body)) { auth_refuse(q, 400, "Send the code."); return; }
    const char *a[1] = {p->account};
    PGresult *r = sql(db, "strong", "SELECT auth.has_strong($1)", 1, a);
    bool strong = !r || PQntuples(r) != 1 || !strcmp(PQgetvalue(r, 0, 0), "t");   // unknown: as if it had one
    if (r) PQclear(r);
    int v = (int)ag_link(p->level, strong);
    if (v != AG_OK) { char w[200]; words_of(v, w, sizeof w); auth_refuse(q, (int)ag_status(v), w); return; }
    const char *w[2] = {p->account, body};
    r = sql(db, "link", "SELECT status, coalesce(machine, '') FROM auth.device_link($1, $2::jsonb->>'code')", 2, w);
    if (!r || PQntuples(r) != 1) { if (r) PQclear(r); auth_refuse(q, 400, "That didn't read."); return; }
    int status = atoi(PQgetvalue(r, 0, 0));
    char machine[80]; snprintf(machine, sizeof machine, "%s", PQgetvalue(r, 0, 1));
    PQclear(r);
    if (status != 200) {
      auth_refuse(q, status, status == 404 ? "That code isn't good." : status == 410 ? "That code expired: run openagents login again."
                                                                     : "That computer was approved already, or its key is another account's.");
      return;
    }
    auth_audit(db, p->account, p->session, "device.link", "ok", "{\"label\":\"cli\"}");
    char me[160], json[400], lines[300];
    size_t k = 0;
    for (const char *c = machine; *c && k + 1 < sizeof me; c++) if (*c != '"' && *c != '\\' && (unsigned char)*c >= 0x20) me[k++] = *c;
    me[k] = 0;
    snprintf(json, sizeof json, "{\"account\":\"%s\",\"machine\":\"%s\",\"approved\":true}", p->account, me);
    snprintf(lines, sizeof lines, "approved %s\n", me);
    auth_answer(q, 200, NULL, json, lines);
    return;
  }
  auth_refuse(q, 405, "Not this method.");
}

// ---------------------------------------------------------------- the routes
bool agents_route(const request *q) {
  bool agents = !strncmp(q->path, "/api/agents", 11) && (q->path[11] == 0 || q->path[11] == '/');
  bool agent = !strncmp(q->path, "/api/agent/", 11), keys = !strncmp(q->path, "/api/agent-keys/", 16);
  bool dev = !strncmp(q->path, "/api/auth/device/", 17);
  if (!agents && !agent && !keys && !dev) return false;
  PGconn *db = api_db();
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  const auth_principal *p = auth_principal_of(q);
  bool get = !strcmp(q->method, "GET") || !strcmp(q->method, "HEAD"), post = !strcmp(q->method, "POST");
  if (dev) { device(q, db, p, q->path + 17); return true; }
  if (keys) { if (!get) auth_refuse(q, 405, "GET this."); else published(q, db, q->path + 16); return true; }
  if (agent) {
    // The gate admitted only an agent with an active grant here (scope routes: with the scope).
    if (!p || p->kind != AP_AGENT || !p->grant[0]) { auth_refuse(q, 403, "Only an agent may ask this."); return true; }
    const char *op = q->path + 11;
    if (!strcmp(op, "me") && get) send_one(q, db, p->owner, p->account);
    else if (!strcmp(op, "owner") && get) owner_profile(q, db, p);
    else if (!strcmp(op, "spend") && post) spend(q, db, p);
    else auth_refuse(q, 404, "Not found.");
    return true;
  }
  const char *op = q->path[11] ? q->path + 12 : "";
  if (!strcmp(op, "request") && post) { ask(q, db, p); return true; }
  if (!strcmp(op, "request/poll") && post) { ask_poll(q, db, p); return true; }
  // The rest are the owner's: the gate let only a person through (L2 where the table says).
  if (!p || p->kind != AP_PERSON || !p->account[0]) { auth_refuse(q, 403, "Only a person may do this."); return true; }
  if (post && !auth_same_site(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  char code[9], id[40];
  if (!*op && get) send_list(q, db, p->account);
  else if (!*op && post) create(q, db, p);
  else if (!strncmp(op, "request/", 8) && get && code_from(op + 8, code)) ask_show(q, db, code);
  else if (!strncmp(op, "approve/", 8) && post && code_from(op + 8, code)) ask_answer(q, db, p, code, true);
  else if (!strncmp(op, "deny/", 5) && post && code_from(op + 5, code)) ask_answer(q, db, p, code, false);
  else if (!strcmp(op, "revoke-all") && post) revoke(q, db, p, NULL);
  else if (!strncmp(op, "revoke/", 7) && post && acct_ok(op + 7)) revoke(q, db, p, op + 7);
  else if (!strncmp(op, "rotate/", 7) && post && acct_ok(op + 7)) { snprintf(id, sizeof id, "%s", op + 7); rotate(q, db, p, id); }
  else if (!strncmp(op, "grant/", 6) && post && acct_ok(op + 6)) { snprintf(id, sizeof id, "%s", op + 6); change(q, db, p, id); }
  else if (!strncmp(op, "resume/", 7) && post && acct_ok(op + 7)) { snprintf(id, sizeof id, "%s", op + 7); resume(q, db, p, id); }
  else say_no(q, 404, "Not found.");
  return true;
}
