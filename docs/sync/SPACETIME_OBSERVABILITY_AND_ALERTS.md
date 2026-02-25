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
4. `mirror.outbox_depth`
5. `mirror.auth_failures_total`
6. `mirror.rate_limited_failures_total`
7. `mirror.network_failures_total`
8. `mirror.validation_failures_total`
9. `mirror.unknown_failures_total`
10. `delivery.max_replay_lag`
11. `delivery.dropped_messages_total`
12. `delivery.stale_cursor_events_total`
13. `auth_failures.unauthorized_total`
14. `auth_failures.forbidden_total`
15. `auth_failures.invalid_token_total`
16. `auth_failures.token_expired_total`
17. `auth_failures.token_not_yet_valid_total`
18. `auth_failures.token_revoked_total`

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
2. `mirror.outbox_depth` trends upward for > 10 minutes.
3. `delivery.max_replay_lag > 10_000` on retained streams.
4. `delivery.stale_cursor_events_total` spikes > 50 per 5 minutes.
5. `auth_failures.unauthorized_total` or `token_expired_total` jumps > 3x trailing hourly baseline.
6. `sync.token.issue.failed` error-rate > 2% of token issue attempts.

## Runbook Links

1. `docs/sync/SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`
2. `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
3. `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
