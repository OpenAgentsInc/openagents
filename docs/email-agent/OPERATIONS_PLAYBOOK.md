# Email Lane Operator Playbook

## Daily Operations

1. Run lane health checks
- Validate ingestion/backfill/sync lanes report healthy status.
- Validate approval queue is not paused unless planned maintenance.
- Validate kill switch is disengaged unless active incident.

2. Review quality and send metrics
- Check quality gate trend and latest score drift.
- Check send success and retry rates.
- Check follow-up execution success and defer rates.

3. Review access audit and redaction events
- Confirm export/delete actions have audit entries.
- Confirm no secret leakage events in logs/diagnostics.

## SLO and SLA Targets

SLO targets (rolling 30d):
- Ingestion freshness: 99% of inbound items visible within 5 minutes.
- Draft readiness: 99% of generated drafts available within 2 minutes of retrieval completion.
- Approval auditability: 100% of sends have auditable approve/policy path.
- Send delivery determinism: 99.9% of approved sends end as exactly-one send or deterministic terminal failure.
- Follow-up scheduler reliability: 99% of due follow-ups executed or explicitly deferred by policy.
- Redaction safety: 100% of sensitive-key fields redacted in diagnostics exports.

SLA targets:
- P1 incident acknowledgement: <= 15 minutes.
- P2 incident acknowledgement: <= 1 hour.
- P3 incident acknowledgement: <= 4 business hours.

## Incident Runbooks

### OAuth Token Expired

Symptoms:
- Connect mailbox stage fails.
- E2E/token-expiry checks fail.

Actions:
1. Verify tenant secret scope version and token expiry timestamp.
2. Rotate/refresh OAuth lifecycle credentials.
3. Re-run sync preflight and e2e harness.
4. Post incident summary with root cause and preventative action.

### Gmail API Rate Limit

Symptoms:
- Sync provider returns rate_limit.
- Incremental sync stalls.

Actions:
1. Confirm backoff/retry policy is active.
2. Pause queue if lag exceeds target thresholds.
3. Lower polling pressure and resume once stabilized.
4. Validate cursor progression after recovery.

### Stale Sync Cursor

Symptoms:
- Rebootstrap required due to backward history cursor.

Actions:
1. Trigger deterministic rebootstrap sequence.
2. Verify no duplicate imports after rebootstrap.
3. Confirm trace continuity and cursor re-established.

### Send Permanent Failure

Symptoms:
- Send state reaches terminal permanent failure.

Actions:
1. Inspect send audit record and provider reason.
2. Route draft back to approval queue if human intervention needed.
3. Record failure class in incident notes and trend tracker.

### Quality Gate Regression

Symptoms:
- Quality gate script fails threshold.

Actions:
1. Block release/deployment.
2. Compare failing case dimensions vs previous baseline.
3. Patch prompt/policy/grounding inputs.
4. Re-run quality + release gates before unblocking.

## Troubleshooting Matrix

| Symptom | Likely Cause | Diagnostic Step | Remediation |
|---|---|---|---|
| Inbox stops updating | OAuth expiry or sync errors | Check connect/sync stage failures and token expiry | Refresh token lifecycle and rerun sync |
| Duplicate sends risk | Missing/invalid idempotency key | Inspect send record for dedupe conflict | Rebuild request fingerprint path; replay with stable key |
| Draft cannot send | Approval missing or queue blocked | Verify approval record, queue pause, kill switch | Record approval or clear queue controls with audit reason |
| Follow-up flood for recipient | Recipient daily limit too high | Inspect scheduler events for skipped/deferred counts | Tighten per-recipient daily limits and rerun tick |
| Secret seen in logs | Redaction gap | Inspect metadata/debug trace redaction output | Patch key pattern list and rotate affected credentials |

## Escalation Path

P1 (customer-visible outage, send path unavailable):
1. On-call operator owns mitigation within 15 minutes.
2. Escalate to lane maintainer if unresolved in 30 minutes.
3. Escalate to engineering lead if unresolved in 60 minutes.

P2 (degraded reliability, high retry/error rates):
1. On-call operator triages and mitigates within 1 hour.
2. Escalate to lane maintainer if unresolved in same shift.

P3 (non-critical defects or documentation gaps):
1. Log issue with reproduction and trace ID.
2. Schedule fix in next planned release window.

## Release/Change Control

Before closing any production change:
- Run `scripts/lint/email-agent-quality-gate.sh`.
- Run `scripts/lint/email-agent-release-gate.sh`.
- Confirm no unresolved P1/P2 incidents for the lane.
- Confirm operator notes and runbook updates are merged when behavior changes.
