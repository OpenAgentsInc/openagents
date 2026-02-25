# Sync Docs

Current-state Khala sync documentation and runbooks (legacy lane during replacement program):
- `ROADMAP.md`: migration and implementation sequencing
- `RUNTIME_CODEX_CUTOVER_RUNBOOK.md`: rollout/cutover procedure
- `SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`: go/no-go gates, rollback triggers, and promotion evidence checklist
- `SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`: scope grants, rotation, and runtime enforcement contract
- `SPACETIME_RUNTIME_PUBLISH_MIRROR.md`: runtime authority fanout to Spacetime stream mapping and parity checks
- `SPACETIME_TOPIC_STREAM_CURSOR_CONTINUITY.md`: legacy topic cursor to stream cursor migration and stale/rebootstrap policy
- `SPACETIME_OBSERVABILITY_AND_ALERTS.md`: runtime/control metrics, alert thresholds, and incident-runbook links
- `SPACETIME_CLIENT_CRATE.md`: typed client API for negotiation, subscribe/resume, reducer calls, and reconnect helpers
- `SPACETIME_DESKTOP_CONNECTION_LIFECYCLE.md`: desktop reconnect/token-refresh lifecycle policy and UI health contract
- `SPACETIME_DESKTOP_APPLY_ENGINE.md`: deterministic `(stream_id, seq)` apply contract, duplicate suppression, and replay rewind behavior
- canonical contracts and invariants live in `docs/core/ARCHITECTURE.md` and `docs/protocol/OA_SYNC_WS_MAPPING.md`

Spacetime replacement authority plans:
- `docs/plans/spacetimedb-full-integration.md`
- `docs/plans/2026-02-25-spacetimedb-autopilot-primary-comms-integration-plan.md`
- `SPACETIME_ENVIRONMENT_MATRIX.md`

Historical sync snapshots and superseded notes are archived in backroom and are not canonical current-surface guidance.
