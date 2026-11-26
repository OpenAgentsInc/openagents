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
  // Meter and bill decision-model calls into inference.calls / inference.credit (#14069).
  if (called && who && *who) {
    PGconn *db = api_db_new();
    if (db) {
      char st_s[16], ms_s[24];
      snprintf(st_s, sizeof st_s, "%ld", status);
      snprintf(ms_s, sizeof ms_s, "%lld", elapsed);
      const char *v[4] = {who, st_s, ms_s, free_key ? "true" : "false"};
      PGresult *r = PQexecParams(db, "SELECT inference.decision_record($1, $2::int, $3::int, $4::bool)", 4, NULL, v, NULL, NULL, 0);
      if (!r || PQresultStatus(r) != PGRES_TUPLES_OK) api_log("router: decision not metered: %s", PQerrorMessage(db));
      if (r) PQclear(r);
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

bool router_route(const request *q) {
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
