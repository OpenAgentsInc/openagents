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

static void decide(const request *q) {
  coax_t_RwState *s = calloc(1, sizeof *s);
  char *google = malloc(ROUTER_GOOGLE), *out = malloc(ROUTER_DECISION);
  if (!s || !google || !out) { api_error(q->fd, 503, "Memory unavailable."); return; }
  int64_t n = rw_begin((coax_v_t_RwState){s, 1}, (coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){(uint8_t *)google, ROUTER_GOOGLE});
  if (n < 0) { api_error(q->fd, 400, "Invalid route request."); return; }
  long status = 0; char *answer = NULL; size_t count = 0; long long started = now_ms();
  const char *key = getenv("OA_GEMINI_DECISIONS_KEY"), *url = getenv("OA_ROUTER_DECISIONS_URL");
  if (n > 0 && key && *key && strlen(key) <= 256) {
    char header[300]; snprintf(header, sizeof header, "x-goog-api-key: %s", key);
    status = mesh_post_json(url && *url ? url : ROUTER_URL, google, (size_t)n, header, ROUTER_TIMEOUT_MS, &answer, &count);
    memset(header, 0, sizeof header);
  }
  int64_t m = rw_finish((coax_v_t_RwState){s, 1}, (int64_t)status, (coax_string){answer ? answer : "", (int64_t)count},
    (int64_t)(now_ms() - started), (coax_v_u8){(uint8_t *)out, ROUTER_DECISION});
  if (m < 1) api_error(q->fd, 503, "The route could not be written.");
  else api_respond(q->fd, 200, "application/json", ROUTER_HEADERS, out, (size_t)m);
  free(answer);
}

bool router_route(const request *q) {
  if (strcmp(q->path, "/api/chat/route/v1")) return false;
  if (strcmp(q->method, "POST")) { api_error(q->fd, 405, "POST this"); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !act_owns(p->kind, rs(p->account), rs(p->account))) { api_error(q->fd, 401, "Log in first."); return true; }
  if (!act_funded(p->kind, rs(p->account), p->team)) { api_error(q->fd, 403, "Agent chat is currently available to the team beta."); return true; }
  // The decision waits on Google, so a child makes it: this process serves one request at a time.
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid == 0) { decide(q); _exit(0); }
  if (pid < 0) api_error(q->fd, 503, "The route is unavailable.");
  return true;
}
