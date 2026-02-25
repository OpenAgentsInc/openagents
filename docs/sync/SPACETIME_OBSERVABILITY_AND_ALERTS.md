# Spacetime Observability and Alert Thresholds

Status: active
Date: 2026-02-25

## Runtime Observability Endpoint

Runtime exposes Spacetime sync health at:

- `GET /internal/v1/spacetime/sync/metrics`

Primary signals:

1. `mirror.published_total`
2. `mirror.duplicate_suppressed_total`
3. `mirror.failed_total`
4. `delivery.max_replay_lag`
5. `delivery.dropped_messages_total`
6. `delivery.stale_cursor_events_total`
7. `auth_failures.unauthorized_total`
8. `auth_failures.forbidden_total`
9. `auth_failures.invalid_token_total`
10. `auth_failures.token_expired_total`
11. `auth_failures.token_not_yet_valid_total`
12. `auth_failures.token_revoked_total`

## Control-Plane Counters

Control service emits counters/audit events for sync token issuance paths:

1. `sync.token.issued`
2. `sync.token.issue.failed`
3. `sync.token.issue.policy_denied`
4. `sync.token.issue.auth_failed`
5. `sync.token.issue.policy_eval_failed`

## Alert Thresholds

Trigger investigation if any of the following holds for 5+ minutes:

1. `mirror.failed_total` increases continuously with no corresponding recoveries.
2. `delivery.max_replay_lag > 10_000` on retained streams.
3. `delivery.stale_cursor_events_total` spikes > 50 per 5 minutes.
4. `auth_failures.unauthorized_total` or `token_expired_total` jumps > 3x trailing hourly baseline.
5. `sync.token.issue.failed` error-rate > 2% of token issue attempts.

## Runbook Links

1. `docs/sync/SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`
2. `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
3. `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
