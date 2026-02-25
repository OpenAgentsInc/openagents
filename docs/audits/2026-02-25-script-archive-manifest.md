# 2026-02-25 Script Archive Manifest

Status: completed
Date: 2026-02-25
Scope: prune obsolete top-level scripts in `scripts/`

## Backroom destination

- `/Users/christopherdavid/code/backroom/openagents-script-archive/2026-02-25-scripts-prune/scripts/`

## Archived from repo

1. `scripts/comms-security-replay-matrix.sh`
   - Reason: legacy Laravel + Elixir (`mix`) compatibility lane; no longer aligned to retained Rust runtime lanes.
2. `scripts/legacyparity-drift-report.sh`
   - Reason: depends on legacyparity intake/fixture paths that are no longer present in the repo.
3. `scripts/copy_web_bundle_to_clipboard.sh`
   - Reason: references removed `apps/web` and `apps/website` paths.
4. `scripts/copy-all-protos.sh`
   - Reason: ad-hoc clipboard helper with no active CI/deploy/runbook dependency.
5. `scripts/test-density-report.sh`
   - Reason: optional reporting helper removed from canonical Local CI policy.
6. `scripts/verify.sh`
   - Reason: redundant wrapper superseded by `scripts/local-ci.sh` lanes.

## Related cleanup

1. Removed retired lanes and references from `scripts/local-ci.sh`.
2. Updated `docs/core/LOCAL_CI.md` to match supported lanes after script retirement.
