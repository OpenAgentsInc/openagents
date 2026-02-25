# Sync Docs

Current canonical Spacetime sync documentation and runbooks:

- `ROADMAP.md`: active sync sequencing and closure checklist
- `RUNTIME_CODEX_CUTOVER_RUNBOOK.md`: rollout/cutover/rollback procedure
- `SPACETIME_ENVIRONMENT_MATRIX.md`: environment and ownership matrix
- `SPACETIME_GCLOUD_DEPLOYMENT_CONSIDERATIONS.md`: GCP deployment state, gaps, and readiness checklist
- `SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`: control-issued claim scope and refresh policy
- `SPACETIME_RUNTIME_PUBLISH_MIRROR.md`: runtime mirror and stream mapping rules
- `SPACETIME_PARITY_HARNESS.md`: active replay/resume parity harness
- `SPACETIME_REPLAY_RESUME_TEST_EXPANSION.md`: cross-surface replay/resume test matrix
- `SPACETIME_CHAOS_DRILLS.md`: staged chaos drills and promotion gate policy
- `SPACETIME_OBSERVABILITY_AND_ALERTS.md`: metric and alert thresholds
- `SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`: acceptance/rollback gates
- `SPACETIME_STAGING_CANARY_ROLLOUT.md`: staging rollout sequencing
- `SPACETIME_PRODUCTION_PHASED_ROLLOUT.md`: production rollout sequencing
- `SPACETIME_CUTOVER_STATE_ANNOUNCEMENT.md`: operator-facing cutover evidence publication

Canonical contracts and invariants:

- `docs/core/ARCHITECTURE.md`
- `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
- `docs/plans/rust-migration-invariant-gates.md`

Token issuance notes:

- Primary endpoint: `POST /api/spacetime/token`
- Compatibility aliases: `POST /api/sync/token`, `POST /api/v1/spacetime/token`, `POST /api/v1/sync/token`

Archived/superseded sync docs:

- `docs/sync/archived/2026-02-25-spacetime-shadow-parity-harness.md`

Historical Elixir-era runtime sync docs remain under `apps/runtime/docs/archived/` and are not canonical retained-surface guidance.
