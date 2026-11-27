// POST /api/chat/route/v1 (docs/router/README.md): which computer, coding agent, model and
// effort would take a task. The body is a RouteRequest and the answer a RouteDecision
// (proto/openagents/router/v1alpha1/router.proto); router-wire.coax reads, decides and writes,
// and this file makes the one call to Google's Decisions API with OA_GEMINI_DECISIONS_KEY.
// Without the key, or when Google does not answer in eight seconds, the default runs and the
// decision says routing was unavailable. Admission is account chat's: a logged-in account that
// the team funds. It starts no agent.
#include "api.h"
#include "account-chat-wire.generated.h"
#include "router-wire.generated.h"
#include "cloud-tools-wire.generated.h"
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define ROUTER_HEADERS "Vary: Cookie, Authorization\r\nCache-Control: no-store\r\n"
#define ROUTER_GOOGLE 32768
#define ROUTER_DECISION 32768
#define ROUTER_URL "https://generativelanguage.googleapis.com/v1beta/decisions"
#define ROUTER_TIMEOUT_MS 8000L

static coax_string rs(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static long long now_ms(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (long long)t.tv_sec * 1000 + t.tv_nsec / 1000000; }

static bool record_decision(PGconn *db, const char *who, long status, long long elapsed, bool free_key) {
  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) { if (tx) PQclear(tx); return false; }
  PQclear(tx);

  long long cost = 0, up_cost = 0, msat_due = 0;
  PGresult *mr = PQexec(db, "SELECT per_call_micros, markup_bps FROM inference.models WHERE name = 'openagents/router-decision'");
  if (!mr || PQresultStatus(mr) != PGRES_TUPLES_OK) { if (mr) PQclear(mr); goto fail; }
  if (PQntuples(mr) == 1 && status == 200) {
    cost = atoll(PQgetvalue(mr, 0, 0));
    long long bps = atoll(PQgetvalue(mr, 0, 1));
    long long denom = 10000LL + (bps > 0 ? bps : 0);
    up_cost = (cost * 10000LL) / denom;
  }
  PQclear(mr);

  if (!free_key && cost > 0) {
    PGresult *rr = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd");
    if (!rr || PQresultStatus(rr) != PGRES_TUPLES_OK) { if (rr) PQclear(rr); goto fail; }
    if (PQntuples(rr) == 1) {
      long long rate = atoll(PQgetvalue(rr, 0, 0));
      if (rate > 0) msat_due = (cost * 100000LL + rate - 1) / rate;
    }
    PQclear(rr);
  }

  int ms_taken = elapsed < 0 ? 0 : (elapsed > INT_MAX ? INT_MAX : (int)elapsed);
  char st_s[16], cost_s[24], up_s[24], ms_s[16], msat_s[24];
  snprintf(st_s, sizeof st_s, "%ld", status);
  snprintf(cost_s, sizeof cost_s, "%lld", cost);
  snprintf(up_s, sizeof up_s, "%lld", up_cost);
  snprintf(ms_s, sizeof ms_s, "%d", ms_taken);
  snprintf(msat_s, sizeof msat_s, "%lld", msat_due);

  const char *dv[2] = {who, cost_s};
  PGresult *dr = PQexecParams(
      db,
      "INSERT INTO inference.account_day AS a (pubkey, day, n, usd_micros) "
      "VALUES ($1, current_date, 1, $2::bigint) "
      "ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1, usd_micros = a.usd_micros + EXCLUDED.usd_micros",
      2, NULL, dv, NULL, NULL, 0);
  if (!dr || PQresultStatus(dr) != PGRES_COMMAND_OK) { if (dr) PQclear(dr); goto fail; }
  PQclear(dr);

  const char *cv[6] = {who, st_s, cost_s, up_s, ms_s, msat_s};
  PGresult *cr = PQexecParams(
      db,
      "INSERT INTO inference.calls "
      "  (ms, pubkey, model, status, in_tokens, cached_in_tokens, out_tokens, usd_micros, upstream_usd_micros, ms_taken, msat, kind) "
      "VALUES "
      "  ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, $1, 'openagents/router-decision', $2::int, 0, 0, 0, $3::bigint, $4::bigint, $5::int, $6::bigint, 'decision')",
      6, NULL, cv, NULL, NULL, 0);
  if (!cr || PQresultStatus(cr) != PGRES_COMMAND_OK) { if (cr) PQclear(cr); goto fail; }
  PQclear(cr);

  if (!free_key && cost > 0) {
    const char *uv[2] = {who, msat_s};
    PGresult *ur = PQexecParams(db, "UPDATE inference.credit SET msat = msat - $2::bigint, updated = now() WHERE pubkey = $1", 2, NULL, uv, NULL, NULL, 0);
    if (!ur || PQresultStatus(ur) != PGRES_COMMAND_OK) { if (ur) PQclear(ur); goto fail; }
    PQclear(ur);
    if (msat_due > 0) {
      PGresult *sr = PQexecParams(
          db,
          "INSERT INTO auth.credit_spends AS c (account, hour, msat) "
          "SELECT a.id, date_trunc('hour', now()), $2::bigint "
          "  FROM auth.accounts a "
          "  LEFT JOIN auth.nostr_keys k ON k.account = a.id "
          " WHERE a.kind = 'person' AND (k.pubkey = $1 OR a.id = $1) "
          " GROUP BY a.id "
          "ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat",
          2, NULL, uv, NULL, NULL, 0);
      if (!sr || PQresultStatus(sr) != PGRES_COMMAND_OK) { if (sr) PQclear(sr); goto fail; }
      PQclear(sr);
    }
  }

  PGresult *cm = PQexec(db, "COMMIT");
  bool ok = cm && PQresultStatus(cm) == PGRES_COMMAND_OK;
  if (cm) PQclear(cm);
  if (!ok) goto fail;
  return true;
fail:
  PQclear(PQexec(db, "ROLLBACK"));
  return false;
}

static void decide(const request *q, const char *who, bool free_key) {
  coax_t_RwState *s = calloc(1, sizeof *s);
  char *google = malloc(ROUTER_GOOGLE), *out = malloc(ROUTER_DECISION);
  if (!s || !google || !out) { api_error(q->fd, 503, "Memory unavailable."); return; }
  int64_t n = rw_begin((coax_v_t_RwState){s, 1}, (coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){(uint8_t *)google, ROUTER_GOOGLE});
  if (n < 0) { api_error(q->fd, 400, "Invalid route request."); return; }
  long status = 0; char *answer = NULL; size_t count = 0; long long started = now_ms();
  const char *key = getenv("OA_GEMINI_DECISIONS_KEY"), *url = getenv("OA_ROUTER_DECISIONS_URL");
  bool called = false;
  if (n > 0 && key && *key && strlen(key) <= 256) {
    char header[300]; snprintf(header, sizeof header, "x-goog-api-key: %s", key);
    status = mesh_post_json(url && *url ? url : ROUTER_URL, google, (size_t)n, header, ROUTER_TIMEOUT_MS, &answer, &count);
    memset(header, 0, sizeof header);
    called = true;
  }
  long long elapsed = now_ms() - started;
  int64_t m = rw_finish((coax_v_t_RwState){s, 1}, (int64_t)status, (coax_string){answer ? answer : "", (int64_t)count},
    (int64_t)elapsed, (coax_v_u8){(uint8_t *)out, ROUTER_DECISION});
  if (m < 1) api_error(q->fd, 503, "The route could not be written.");
  else api_respond(q->fd, 200, "application/json", ROUTER_HEADERS, out, (size_t)m);
  // Meter and bill decision-model calls into inference.calls / inference.credit (#14069, #14111).
  if (called && who && *who) {
    PGconn *db = api_db_new();
    if (db) {
      if (!record_decision(db, who, status, elapsed, free_key)) api_log("router: decision not metered: %s", PQerrorMessage(db));
      PQfinish(db);
    }
  }
  free(answer);
}

static bool router_has_credit(const char *who) {
  if (!who || !*who) return false;
  PGconn *db = api_db();
  if (!db) return false;
  const char *v[1] = {who};
  PGresult *r = PQexecParams(db, "SELECT coalesce((SELECT msat FROM inference.credit WHERE pubkey = $1), 0) > 0", 1, NULL, v, NULL, NULL, 0);
  bool ok = r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  if (r) PQclear(r);
  return ok;
}

static void placement_handle(const request *q) {
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST this"); return; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !act_owns(p->kind, rs(p->account), rs(p->account))) { api_error(q->fd, 401, "Log in first."); return; }
  if (!auth_same_site(q)) { api_error(q->fd, 403, "Cross-site write refused."); return; }
  coax_t_PlReq req;
  if (!pl_c_placement_req_read((coax_string){q->body, (int64_t)q->body_len}, (coax_v_t_PlReq){&req, 1})) {
    api_error(q->fd, 400, "Invalid placement request.");
    return;
  }
  char prompt[513] = {0}, project_id[41] = {0}, sz[17] = "small";
  if (req.prompt_n > 0) memcpy(prompt, req.prompt.e, (size_t)req.prompt_n);
  if (req.project_n > 0) memcpy(project_id, req.project_id.e, (size_t)req.project_n);
  if (req.size_n > 0) { memcpy(sz, req.size.e, (size_t)req.size_n); sz[req.size_n] = 0; }
  if (cp_c_sandbox_rate_hr(rs(sz)) < 0) { api_error(q->fd, 400, "Invalid sandbox size."); return; }

  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "Account storage unavailable."); return; }
  const char *who = p->pubkey[0] ? p->pubkey : p->account;
  long long credit_msat = 0, usd_per_btc = 100000;
  const char *cv[2] = {who, p->account};
  PGresult *cr = PQexecParams(db,
    "SELECT coalesce((SELECT msat FROM inference.credit WHERE pubkey = $1 OR pubkey = $2 ORDER BY msat DESC LIMIT 1), 0)",
    2, NULL, cv, NULL, NULL, 0);
  if (cr && PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1) credit_msat = atoll(PQgetvalue(cr, 0, 0));
  if (cr) PQclear(cr);
  PGresult *br = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd LIMIT 1");
  if (br && PQresultStatus(br) == PGRES_TUPLES_OK && PQntuples(br) == 1) {
    long long v = atoll(PQgetvalue(br, 0, 0));
    if (v > 0) usd_per_btc = v;
  }
  if (br) PQclear(br);

  bool free_acct = p->team || p->admin;
  bool sandbox_funded = cp_c_sandbox_admit(rs(sz), (int64_t)credit_msat, (int64_t)usd_per_btc, free_acct) == 200;

  bool has_station = false;
  coax_t_PlStation st = {0};
  char host_pk[65] = {0}, host_name[81] = {0};
  if (req.has_station_override) {
    has_station = true;
    st = req.station;
    const char *ev[1] = {p->account};
    PGresult *er = PQexecParams(db,
      "SELECT host_pubkey, display_name FROM chat.environment_connections_v1 WHERE account = $1 ORDER BY last_seen_ms DESC LIMIT 1",
      1, NULL, ev, NULL, NULL, 0);
    if (er && PQresultStatus(er) == PGRES_TUPLES_OK && PQntuples(er) == 1) {
      snprintf(host_pk, sizeof host_pk, "%s", PQgetvalue(er, 0, 0));
      snprintf(host_name, sizeof host_name, "%s", PQgetvalue(er, 0, 1));
    }
    if (er) PQclear(er);
  } else {
    long long now_wall = (long long)api_now_ms();
    char cut_s[32];
    snprintf(cut_s, sizeof cut_s, "%lld", now_wall - 120000LL);
    const char *ev[3] = {p->account, project_id, cut_s};
    PGresult *er = PQexecParams(db,
      "SELECT e.host_pubkey, e.display_name, (e.last_seen_ms >= $3::bigint) AS online, "
      "($2 = '' OR EXISTS(SELECT 1 FROM projects.environment_bindings_v1 b WHERE b.account = $1 AND b.project = $2 AND b.host_pubkey = e.host_pubkey) "
      " OR EXISTS(SELECT 1 FROM projects.private_v1 pr WHERE pr.account = $1 AND pr.id = $2 AND (pr.repo_canonical = '' OR position(pr.repo_canonical in lower(e.workspaces_json)) > 0))) AS repo_ok, "
      "(SELECT count(*) FROM chat.turns_v2 t WHERE t.account = $1 AND t.workspace = ('host:' || e.host_pubkey) AND t.state IN (1,2)) AS active_turns "
      "FROM chat.environment_connections_v1 e WHERE e.account = $1 "
      "ORDER BY (e.last_seen_ms >= $3::bigint) DESC, repo_ok DESC, e.last_seen_ms DESC LIMIT 1",
      3, NULL, ev, NULL, NULL, 0);
    if (er && PQresultStatus(er) == PGRES_TUPLES_OK && PQntuples(er) == 1) {
      has_station = true;
      snprintf(host_pk, sizeof host_pk, "%s", PQgetvalue(er, 0, 0));
      snprintf(host_name, sizeof host_name, "%s", PQgetvalue(er, 0, 1));
      st.online = PQgetvalue(er, 0, 2)[0] == 't';
      st.repo_matched = PQgetvalue(er, 0, 3)[0] == 't';
      st.keep_clear = false;
      st.active_turns = atoll(PQgetvalue(er, 0, 4));
      st.max_turns = 2;
      st.cpu_pct = 20;
      st.free_ram_mb = 8192;
      st.free_disk_mb = 20480;
    }
    if (er) PQclear(er);
  }

  int64_t work_kind = pl_c_classify_prompt(rs(prompt));
  uint8_t reason[256] = {0};
  int64_t tier = pl_c_decide_auto(work_kind, req.parallel_cloud, has_station,
                                  st.online, st.repo_matched, st.keep_clear,
                                  st.active_turns, st.max_turns, st.cpu_pct,
                                  st.free_ram_mb, st.free_disk_mb, sandbox_funded,
                                  (coax_v_u8){reason, sizeof reason - 1});
  uint8_t out[1024];
  int64_t rate_hr = cp_c_sandbox_rate_hr(rs(sz));
  int64_t n = pl_c_placement_res_write(tier,
                                       tier == 2 ? rs(host_pk) : rs(""),
                                       tier == 2 ? rs(host_name) : (tier == 3 ? rs("Cloud Sandbox") : rs("Cloud")),
                                       rs((const char *)reason), rate_hr, (int64_t)credit_msat,
                                       (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 503, "Could not encode placement decision."); return; }
  api_respond(q->fd, 200, "application/json", ROUTER_HEADERS, out, (size_t)n);
}

bool router_route(const request *q) {
  if (!strcmp(q->path, "/api/chat/placement/v1")) { placement_handle(q); return true; }
  if (strcmp(q->path, "/api/chat/route/v1")) return false;
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST this"); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !act_owns(p->kind, rs(p->account), rs(p->account))) { api_error(q->fd, 401, "Log in first."); return true; }
  const char *who = p->pubkey[0] ? p->pubkey : p->account;
  bool free_key = act_funded(p->kind, rs(p->account), p->team) || p->admin;
  if (!free_key && !router_has_credit(who)) {
    api_error(q->fd, 403, "Agent chat requires team beta access or OpenAgents Cloud credit."); return true;
  }
  // The decision waits on Google, so a child makes it: this process serves one request at a time.
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { decide(q, who, free_key); _exit(0); }
  if (pid < 0) api_error(q->fd, 503, "The route is unavailable.");
  return true;
}
