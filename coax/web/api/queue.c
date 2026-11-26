// Coax Web Postgres job queue, post-response defer(), and transactional afterCommit() (Issue #14079).
#include "queue.h"
#include "api.h"
#include "queue-policy.generated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CB_SLOTS_MAX 32

typedef struct {
  api_callback_fn fn;
  void *ctx;
} cb_slot;

static cb_slot g_defer[CB_SLOTS_MAX];
static int g_defer_n;

static cb_slot g_after_commit[CB_SLOTS_MAX];
static int g_after_commit_n;

bool api_defer(api_callback_fn fn, void *ctx) {
  if (!fn || g_defer_n >= CB_SLOTS_MAX) return false;
  g_defer[g_defer_n++] = (cb_slot){fn, ctx};
  return true;
}

void api_defer_run_all(void) {
  int n = g_defer_n;
  g_defer_n = 0;
  for (int i = 0; i < n; i++) {
    if (g_defer[i].fn) g_defer[i].fn(g_defer[i].ctx);
  }
}

void api_defer_clear(void) {
  g_defer_n = 0;
}

void db_tx_reset(void) {
  g_after_commit_n = 0;
}

bool db_tx_after_commit(api_callback_fn fn, void *ctx) {
  if (!fn || g_after_commit_n >= CB_SLOTS_MAX) return false;
  g_after_commit[g_after_commit_n++] = (cb_slot){fn, ctx};
  return true;
}

void db_tx_on_commit(void) {
  int n = g_after_commit_n;
  g_after_commit_n = 0;
  for (int i = 0; i < n; i++) {
    if (g_after_commit[i].fn) g_after_commit[i].fn(g_after_commit[i].ctx);
  }
}

void db_tx_on_rollback(void) {
  g_after_commit_n = 0;
}

bool queue_enqueue(PGconn *db, const char *queue, const char *kind,
                   const char *payload_json, const char *idem_key, int max_attempts) {
  if (!db || !queue || !kind || !payload_json) return false;
  if (!qp_enqueue_valid((int64_t)strlen(queue), (int64_t)strlen(payload_json), max_attempts)) return false;
  char max_s[16];
  snprintf(max_s, sizeof max_s, "%d", max_attempts);
  const char *v[5] = {queue, kind, payload_json, (idem_key && *idem_key) ? idem_key : NULL, max_s};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO queue.jobs (queue, kind, payload, idempotency_key, max_attempts) "
      "VALUES ($1, $2, $3::jsonb, $4, $5::smallint) ON CONFLICT (idempotency_key) DO NOTHING",
      5, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  return ok;
}

// Claims and processes one available job via FOR UPDATE SKIP LOCKED.
// Returns 1 if a job was processed, 0 if queue was empty, -1 on DB error.
int queue_work_once(PGconn *db, const char *queue, const char *worker_id) {
  if (!db || !queue || !worker_id) return -1;
  const char *v[2] = {queue, worker_id};
  PGresult *r = PQexecParams(
      db,
      "WITH next_job AS ("
      "  SELECT id FROM queue.jobs "
      "  WHERE queue = $1 AND available_at <= now() "
      "    AND (reserved_at IS NULL OR reserved_at < now() - interval '90 seconds') "
      "  ORDER BY available_at ASC, id ASC "
      "  FOR UPDATE SKIP LOCKED LIMIT 1"
      ") "
      "UPDATE queue.jobs j SET reserved_at = now(), reserved_by = $2, attempts = j.attempts + 1 "
      "FROM next_job WHERE j.id = next_job.id "
      "RETURNING j.id, j.kind, j.payload::text, j.attempts, j.max_attempts",
      2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { PQclear(r); return -1; }
  if (PQntuples(r) == 0) { PQclear(r); return 0; }

  char id[32], kind[128];
  snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 0));
  snprintf(kind, sizeof kind, "%s", PQgetvalue(r, 0, 1));
  char *payload = strdup(PQgetvalue(r, 0, 2));
  int attempts = atoi(PQgetvalue(r, 0, 3));
  int max_attempts = atoi(PQgetvalue(r, 0, 4));
  PQclear(r);

  bool ok = false;
  char err[256] = "unknown job kind";
  if (payload && !strcmp(kind, "mail.send")) {
    char mid[128];
    int rc = email_send(db, payload, mid, err, sizeof err);
    ok = (rc == 0 || rc == 1);
  }

  if (ok) {
    const char *d[1] = {id};
    PQclear(PQexecParams(db, "DELETE FROM queue.jobs WHERE id = $1::bigint", 1, NULL, d, NULL, NULL, 0));
  } else {
    int64_t act = qp_next_action(attempts, max_attempts, false, false, false);
    if (act == 2) { // QP_ACT_DEAD -> move to failed_jobs
      const char *f[4] = {queue, kind, payload ? payload : "{}", err};
      PQclear(PQexecParams(
          db,
          "INSERT INTO queue.failed_jobs (queue, kind, payload, error_text) VALUES ($1, $2, $3::jsonb, $4)",
          4, NULL, f, NULL, NULL, 0));
      const char *d[1] = {id};
      PQclear(PQexecParams(db, "DELETE FROM queue.jobs WHERE id = $1::bigint", 1, NULL, d, NULL, NULL, 0));
    } else {
      int64_t delay_s = qp_retry_delay_sec(attempts);
      if (delay_s < 1) delay_s = 5;
      char ds[32]; snprintf(ds, sizeof ds, "%lld", (long long)delay_s);
      const char *u[2] = {id, ds};
      PQclear(PQexecParams(
          db,
          "UPDATE queue.jobs SET reserved_at = NULL, reserved_by = NULL, "
          "available_at = now() + ($2 || ' seconds')::interval WHERE id = $1::bigint",
          2, NULL, u, NULL, NULL, 0));
    }
  }
  free(payload);
  return 1;
}
