// WorkOS authentication, webhook, AuthKit Actions, Events mirror, and lifecycle hooks (#14100).
#pragma once
#include <libpq-fe.h>
#include <stdbool.h>
#include "api.h"

// Handles /api/auth/workos/* routes (called from auth_api_route when op starts with "workos/").
bool workos_route(const request *q, PGconn *db, const char *op);

// Pure Coax/C GET /events cursor poller for jobs.c (EVERY[] job "workos-events").
// Returns malloc'd JSON summary or NULL on fatal error.
char *workos_poll_events(PGconn *db, const char *day);

// Revokes mirrored WorkOS sessions for acct on freeze, logout-everywhere, or delete request.
void workos_revoke_account_sessions(PGconn *db, const char *acct);

// Drains workos.pending_deletes (populated before 30-day auth.purge_due()) via DELETE /user_management/users/{id}.
int workos_process_pending_deletes(PGconn *db);
