# Local CI Policy

OpenAgents CI gates are local-first and executed through tracked git hooks and
scripts in this repository.

GitHub workflow automation is intentionally not used for CI in this repo.

## Install Hooks

From repository root:

```bash
./scripts/install-git-hooks.sh
```

This configures:

- `.githooks/pre-commit` -> `scripts/local-ci.sh changed`
- `.githooks/pre-push` -> no-op (checks intentionally disabled for now)

## Local CI Entrypoint

```bash
./scripts/local-ci.sh changed
```

Supported lanes:

- `changed` (default)
- `all`
- `docs`
- `proto`
- `runtime`
- `comms`
- `openclaw`

Examples:

```bash
./scripts/local-ci.sh runtime
./scripts/local-ci.sh comms
./scripts/local-ci.sh all
```

## Push Policy (Current)

Pre-push checks are intentionally disabled.

Run gates manually before pushing when needed:

```bash
./scripts/local-ci.sh changed
./scripts/local-ci.sh all
```

## Proto Contract Gate

`./scripts/local-ci.sh proto` enforces:

- `buf lint`
- `buf breaking --against '.git#branch=main,subdir=proto'` (or `origin/main`)
- `./scripts/verify-proto-generate.sh`

This lane is also invoked automatically by `changed` mode whenever `proto/`,
`buf.yaml`, `buf.gen.yaml`, or `scripts/verify-proto-generate.sh` changes.

## Proto Remediation

If proto CI fails:

1. Run `git fetch origin main` (required for `buf breaking` baseline).
2. Fix lint errors from `buf lint`.
3. If the change is intended and additive, keep field numbers stable and avoid
   renames/removals.
4. If a breaking change is truly required, move to a new package version
   namespace (for example `v2`) rather than mutating `v1`.
5. Re-run:
   - `./scripts/local-ci.sh proto`
   - `./scripts/local-ci.sh changed`

## Temporary Bypass (Use Sparingly)

```bash
OA_SKIP_LOCAL_CI=1 git commit -m "..."
```

If bypassing, run equivalent lane commands manually before merge.
