// Email from the API service through Resend (svc-email-resend,
// docs/coaxial-services.md): from OpenAgents <no-reply@openagents.com>, a
// plain-text body with optional HTML, attachments, a List-Unsubscribe header
// and an idempotency key per message. A send is retried with backoff on a
// network error, a 429 or a 5xx. The log (email.sends, 0085_email.sql, and the
// service log) keeps the key's hash, counts, attempts and Resend's id: never an
// address, a subject, a body or the API key.
//
//   POST /api/admin/email   an admin (NIP-98 or session): the message as JSON
//                           (email.begin); 200 {"state":"sent","id"} or
//                           {"state":"already sent","id"}, 400 refused, 502
//                           Resend did not take it, 503 no key here.
//   email_send(...)         the same for the service's own mail (digests).
//
//   OA_RESEND_API_KEY   the key (Secret Manager openagents-resend-api-key)
//   OA_RESEND_URL       Resend's endpoint (tests point it at a fake)
//   OA_EMAIL_ALLOW      staging: the only addresses mail may go to
#include "api.h"

#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }

typedef struct { char *data; size_t n; } buf;
static size_t keep(char *p, size_t size, size_t count, void *u) {
  buf *b = u; size_t k = size * count;
  if (b->n + k > 65536) return 0;
  char *g = realloc(b->data, b->n + k + 1); if (!g) return 0;
  b->data = g; memcpy(b->data + b->n, p, k); b->n += k; b->data[b->n] = 0;
  return k;
}

// One POST to Resend: its HTTP status (0 unreached), Resend's id into id.
static long post_once(const char *key, const char *idem, const char *payload, char id[128]) {
  CURL *h = curl_easy_init(); if (!h) return 0;
  char auth[600], ik[400];
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", key);
  snprintf(ik, sizeof ik, "Idempotency-Key: %s", idem);
  struct curl_slist *headers = curl_slist_append(NULL, "Content-Type: application/json");
  headers = curl_slist_append(headers, auth); headers = curl_slist_append(headers, ik);
  buf b = {0};
  curl_easy_setopt(h, CURLOPT_URL, env_or("OA_RESEND_URL", "https://api.resend.com/emails"));
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, payload);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, keep); curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 20L); curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  id[0] = 0;
  const char *at = b.data ? strstr(b.data, "\"id\"") : NULL;
  if (at && (at = strchr(at + 4, '"'))) {
    size_t n = 0; at++;
    while (n < 127 && at[n] && at[n] != '"' && at[n] != '\\') { id[n] = at[n]; n++; }
    id[n] = 0;
  }
  free(b.data); curl_slist_free_all(headers); curl_easy_cleanup(h);
  return status;
}

// A message (JSON, as email.begin reads it): 0 sent now, 1 sent before (id is
// that send's), else an HTTP status for the caller, with err.
int email_send(PGconn *db, const char *message, char id[128], char *err, size_t errcap) {
  id[0] = 0; err[0] = 0;
  const char *key = getenv("OA_RESEND_API_KEY");
  if (!key || !*key) { snprintf(err, errcap, "email is not set up here"); return 503; }
  char ms[32]; snprintf(ms, sizeof ms, "%lld", (long long)api_now_ms());
  const char *v[3] = {message, env_or("OA_EMAIL_ALLOW", ""), ms};
  PGresult *r = PQexecParams(db, "SELECT x->>'state', x->>'error', x->>'key', x->>'id', (x->'payload')::text FROM email.begin($1::jsonb, $2, $3::bigint) x",
                             3, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    snprintf(err, errcap, "not a message"); PQclear(r); return 400;   // not JSON, mostly; the database's words could quote the body
  }
  const char *state = PQgetvalue(r, 0, 0);
  if (!strcmp(state, "refused")) { snprintf(err, errcap, "%s", PQgetvalue(r, 0, 1)); PQclear(r); return 400; }
  char hash[65]; snprintf(hash, sizeof hash, "%s", PQgetvalue(r, 0, 2));
  if (!strcmp(state, "sent")) {
    snprintf(id, 128, "%s", PQgetvalue(r, 0, 3)); PQclear(r);
    api_log("email: %.12s was sent before as %s", hash, id);
    return 1;
  }
  char *payload = strdup(PQgetvalue(r, 0, 4)); PQclear(r);
  if (!payload) { snprintf(err, errcap, "out of memory"); return 500; }
  long backoff = atol(env_or("OA_EMAIL_BACKOFF_MS", "500")), status = 0; int tries = 0;
  for (; tries < 4; tries++) {
    if (tries) { struct timespec t = {backoff / 1000, (backoff % 1000) * 1000000L}; nanosleep(&t, NULL); backoff *= 2; }
    status = post_once(key, hash, payload, id);   // the key's hash goes to Resend as its idempotency key too
    if (status && status != 429 && status < 500) { tries++; break; }
  }
  free(payload);
  bool ok = status >= 200 && status < 300 && id[0];
  char st[16], tr[16]; snprintf(st, sizeof st, "%ld", status); snprintf(tr, sizeof tr, "%d", tries);
  snprintf(ms, sizeof ms, "%lld", (long long)api_now_ms());
  const char *f[6] = {hash, ok ? "t" : "f", tr, st, ok ? id : NULL, ms};
  PQclear(PQexecParams(db, "SELECT email.finish($1, $2::boolean, $3::int, $4::int, $5, $6::bigint)", 6, NULL, f, NULL, NULL, 0));
  if (!ok) {
    api_log("email: %.12s not sent: Resend %ld after %d attempt(s)", hash, status, tries);
    snprintf(err, errcap, status ? "the mail service refused it (%ld)" : "the mail service did not answer", status);
    id[0] = 0;
    return 502;
  }
  api_log("email: %.12s sent as %s after %d attempt(s)", hash, id, tries);
  return 0;
}

static void answer(int fd, int status, const char *state, const char *id, const char *err) {
  char out[512];
  if (err) {
    char e[300]; size_t n = 0;
    for (const char *p = err; *p && n < sizeof e - 2; p++) if (*p != '"' && *p != '\\') e[n++] = *p;
    e[n] = 0;
    snprintf(out, sizeof out, "{\"error\":\"%s\"}", e);
  } else snprintf(out, sizeof out, "{\"state\":\"%s\",\"id\":\"%s\"}", state, id);
  api_json(fd, status, out);
}

bool email_route(const request *q) {
  if (strcmp(q->path, "/api/admin/email")) return false;
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST a message\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  char who[65];
  int s = admin_who(q, db, who);
  if (s) { api_json(q->fd, s, s == 403 ? "{\"error\":\"not an admin\"}" : "{\"error\":\"sign in\"}"); return true; }
  char *message = malloc(q->body_len + 1);
  if (!message) { api_json(q->fd, 500, "{\"error\":\"out of memory\"}"); return true; }
  memcpy(message, q->body, q->body_len); message[q->body_len] = 0;
  char id[128], err[300];
  int r = email_send(db, message, id, err, sizeof err);
  free(message);
  if (r == 0) answer(q->fd, 200, "sent", id, NULL);
  else if (r == 1) answer(q->fd, 200, "already sent", id, NULL);
  else answer(q->fd, r, NULL, NULL, err);
  return true;
}
