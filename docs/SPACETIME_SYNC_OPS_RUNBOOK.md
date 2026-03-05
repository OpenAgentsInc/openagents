# Spacetime Sync Ops Runbook

This runbook is the operator entrypoint for Spacetime module publish/promote, schema/reducer contract validation, and active-connection handshake smoke checks.

## Scope

Applies to:
- `spacetime/modules/autopilot-sync/spacetimedb`
- `scripts/spacetime/verify-autopilot-sync-contract.sh`
- `scripts/spacetime/publish-promote.sh`
- `scripts/spacetime/maincloud-handshake-smoke.sh`

## Prerequisites

Required tools:
- `spacetime` CLI
- `jq`
- `git`
- `rg`

Required environment variables:
- `OA_SPACETIME_DEV_DATABASE`
- `OA_SPACETIME_STAGING_DATABASE`
- `OA_SPACETIME_PROD_DATABASE`

Optional environment variables:
- `OA_SPACETIME_DEV_SERVER` (default `maincloud`)
- `OA_SPACETIME_STAGING_SERVER` (default `maincloud`)
- `OA_SPACETIME_PROD_SERVER` (default `maincloud`)

## 1) Local Contract Check

Validates required tables/reducers from module source.

```bash
scripts/spacetime/verify-autopilot-sync-contract.sh
```

Expected pass output:
- `autopilot-sync contract verification passed`

Failure behavior:
- Exits non-zero.
- Prints missing table/reducer contract item.

## 2) Publish Module (Environment-Specific)

Publish to one environment:

```bash
scripts/spacetime/publish-promote.sh publish --env dev
scripts/spacetime/publish-promote.sh publish --env staging
scripts/spacetime/publish-promote.sh publish --env prod
```

Output artifacts:
- `output/spacetime/publish/<timestamp>/publish.log`
- `output/spacetime/publish/<timestamp>/pre-schema.json`
- `output/spacetime/publish/<timestamp>/post-schema.json`
- `output/spacetime/publish/<timestamp>/report.json`

Publish invariants:
- Local contract verification runs before publish.
- Post-publish schema must include required tables/reducers.
- Non-zero exit code on missing contract/table/reducer.

## 3) Promote Module (Source -> Target)

Promote from one environment to another:

```bash
scripts/spacetime/publish-promote.sh promote --from-env dev --to-env staging
scripts/spacetime/publish-promote.sh promote --from-env staging --to-env prod
```

Notes:
- Target schema drift is blocked by default.
- Use `--allow-target-drift` only for intentional/manual operations.

Output artifacts:
- `output/spacetime/publish/<timestamp>/source-schema.json`
- `output/spacetime/publish/<timestamp>/target-pre-schema.json`
- `output/spacetime/publish/<timestamp>/target-post-schema.json`
- `output/spacetime/publish/<timestamp>/promote-report.json`

## 4) Maincloud Handshake Smoke

Verifies `active_connection` lifecycle through two concurrent subscribe sessions:
- baseline count,
- count increase while subscriptions are open,
- count returns to baseline after disconnect.

```bash
scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_DEV_DATABASE"
```

Optional flags:
- `--server <name>`
- `--timeout <seconds>`
- `--sleep <seconds>`
- `--output-dir <path>`

Output artifacts:
- `output/spacetime/handshake/<timestamp>/baseline.txt`
- `output/spacetime/handshake/<timestamp>/during.txt`
- `output/spacetime/handshake/<timestamp>/final.txt`
- `output/spacetime/handshake/<timestamp>/sub1.log`
- `output/spacetime/handshake/<timestamp>/sub2.log`
- `output/spacetime/handshake/<timestamp>/SUMMARY.md`

Failure behavior:
- Exits non-zero when connected-client count does not increase during subscriptions.
- Exits non-zero when final connected-client count does not return to baseline.

## Exit Codes

- `0`: success
- `1`: operational/contract validation failure
- `2`: invalid usage or missing required args/tools/env

## Recommended Pre-Release Sequence

1. `scripts/spacetime/verify-autopilot-sync-contract.sh`
2. `scripts/spacetime/publish-promote.sh publish --env dev`
3. `scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_DEV_DATABASE"`
4. `scripts/spacetime/publish-promote.sh promote --from-env dev --to-env staging`
5. `scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_STAGING_DATABASE"`
