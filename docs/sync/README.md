# Sync Docs

Current-state Khala sync documentation and runbooks (legacy lane during replacement program):
- `ROADMAP.md`: migration and implementation sequencing
- `RUNTIME_CODEX_CUTOVER_RUNBOOK.md`: rollout/cutover procedure
- `SPACETIME_STAGING_CANARY_ROLLOUT.md`: cohorted staging canary flow, gate bundle, and promotion artifacts
- `SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`: go/no-go gates, rollback triggers, and promotion evidence checklist
- `SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`: scope grants, rotation, and runtime enforcement contract
- `SPACETIME_RUNTIME_PUBLISH_MIRROR.md`: runtime authority fanout to Spacetime stream mapping and parity checks
- `SPACETIME_SHADOW_PARITY_HARNESS.md`: dual-lane shadow parity harness, gate thresholds, mismatch reports, and resolution workflow
- `SPACETIME_CHAOS_DRILLS.md`: staged chaos scenarios, expected recovery behavior, and artifact capture workflow
- `SPACETIME_TOPIC_STREAM_CURSOR_CONTINUITY.md`: legacy topic cursor to stream cursor migration and stale/rebootstrap policy
- `SPACETIME_OBSERVABILITY_AND_ALERTS.md`: runtime/control metrics, alert thresholds, and incident-runbook links
- `SPACETIME_CLIENT_CRATE.md`: typed client API for negotiation, subscribe/resume, reducer calls, and reconnect helpers
- `SPACETIME_DESKTOP_CONNECTION_LIFECYCLE.md`: desktop reconnect/token-refresh lifecycle policy and UI health contract
- `SPACETIME_DESKTOP_APPLY_ENGINE.md`: deterministic `(stream_id, seq)` apply contract, duplicate suppression, and replay rewind behavior
- `SPACETIME_DESKTOP_CHECKPOINT_PERSISTENCE.md`: local checkpoint format, restart resume policy, stale clamp, and persistence/recovery guarantees
- `SPACETIME_DESKTOP_SYNC_HEALTH_UX.md`: Runtime Login sync-health indicators for replay progress, lag, token lease, and actionable recovery hints
- `SPACETIME_REPLAY_RESUME_TEST_EXPANSION.md`: cross-surface replay/resume/reconnect test matrix and unified verification script
- canonical contracts and invariants live in `docs/core/ARCHITECTURE.md` and `docs/protocol/OA_SYNC_WS_MAPPING.md`

Spacetime replacement authority plans:
- `docs/plans/spacetimedb-full-integration.md`
- `docs/plans/2026-02-25-spacetimedb-autopilot-primary-comms-integration-plan.md`
- `SPACETIME_ENVIRONMENT_MATRIX.md`

Historical sync snapshots and superseded notes are archived in backroom and are not canonical current-surface guidance.
