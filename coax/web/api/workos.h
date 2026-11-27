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

// WorkOS Pipes helpers shared with integrations.c and cloud agent tools (#14126).
const char *workos_pipes_base_url(void);
bool workos_pipes_allow_local(void);
bool workos_pipes_configured(void);
bool workos_lookup_identity(PGconn *db, const char *account, char uid_out[128], char org_out[128]);
void workos_api_request(const char *method, const char *url, const char *body, size_t body_len, void *http_res);
bool workos_pipes_github_token(PGconn *db, const char *account, char *token_out, size_t token_cap, char *err_out, size_t err_cap);
