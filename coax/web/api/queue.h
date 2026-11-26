// Coax Web Postgres job queue, post-response defer(), and transactional afterCommit() (Issue #14079).
#pragma once
#include <libpq-fe.h>
#include <stdbool.h>
#include <stddef.h>

typedef void (*api_callback_fn)(void *ctx);

// Post-response deferred callbacks (mirrors Laravel defer() / InvokeDeferredCallbacks)
bool api_defer(api_callback_fn fn, void *ctx);
void api_defer_run_all(void);
void api_defer_clear(void);

// Transactional afterCommit() hooks (mirrors Laravel DB::afterCommit())
void db_tx_reset(void);
bool db_tx_after_commit(api_callback_fn fn, void *ctx);
void db_tx_on_commit(void);
void db_tx_on_rollback(void);

// Postgres queue operations (queue.jobs / queue.failed_jobs)
bool queue_enqueue(PGconn *db, const char *queue, const char *kind,
                   const char *payload_json, const char *idem_key, int max_attempts);
int queue_work_once(PGconn *db, const char *queue, const char *worker_id);
