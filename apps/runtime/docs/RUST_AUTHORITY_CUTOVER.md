# Rust Runtime Authority Cutover

Status: Added in OA-RUST-040.

This runbook defines how to cut authority writes to the Rust runtime and freeze legacy write paths with rollback controls.

## Preconditions

1. Shadow parity gate passes using `runtime-shadow-harness` (`decision=allow`).
2. No critical parity diffs in staged scenarios.
3. Runtime health/readiness and projector recovery checks are green.
4. DB role isolation verification passes:
   - `DB_URL=<postgres-url> apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh`

## Cutover Controls

Rust runtime:

- `RUNTIME_AUTHORITY_WRITE_MODE=rust_active` enables canonical Rust writes.
- `RUNTIME_AUTHORITY_WRITE_MODE=shadow_only|read_only` freezes Rust write endpoints (`503 write_path_frozen`).

Legacy runtime:

- `LEGACY_RUNTIME_WRITE_FREEZE=true` freezes legacy mutation endpoints (`410 write_path_frozen`).

## Staged Cutover Plan

1. Enable Rust authority in staging:
   - `RUNTIME_AUTHORITY_WRITE_MODE=rust_active`
   - `LEGACY_RUNTIME_WRITE_FREEZE=true`
2. Verify staging write path:
   - Create run/event/worker mutations through control-plane integration.
   - Confirm replay/receipt/projector artifacts update in Rust runtime.
3. Run shadow parity harness against staged traffic snapshots.
4. Promote same config to production in controlled cohort rollout.

## Verification Checklist

1. Rust write endpoints accept writes and update event/projection artifacts.
2. Legacy mutation routes return `410 write_path_frozen`.
3. Shadow parity reports remain within gate thresholds.
4. Error rates and drift metrics remain within SLO targets.

## Rollback

If incident criteria are hit:

1. Switch Rust runtime to freeze mode:
   - `RUNTIME_AUTHORITY_WRITE_MODE=shadow_only`
2. Disable legacy freeze:
   - `LEGACY_RUNTIME_WRITE_FREEZE=false`
3. Route writes back to legacy authority path.
4. Collect parity report and incident timeline artifacts before retry.

## Incident Criteria (Blockers)

1. Critical shadow parity diffs.
2. Sustained write-path error spike.
3. Unbounded drift or checkpoint regression.
