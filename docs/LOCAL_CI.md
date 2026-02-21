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
- `test-triggers`

Examples:

```bash
./scripts/local-ci.sh runtime
./scripts/local-ci.sh comms
./scripts/local-ci.sh all
```

Changed-mode trigger note:

- `comms` lane auto-runs for legacy Laravel/openagents.com surface paths, `apps/runtime/`, comms protocol docs, and comms matrix script changes.
- Rust control-service path `apps/openagents.com/service/` is intentionally excluded from automatic `comms` lane triggering to keep Rust migration iteration fast.

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
- `./scripts/verify-proto-generate.sh` (Rust-only proto generation verification; compatibility alias)

Rust generation command used by the alias:
- `./scripts/verify-rust-proto-crate.sh`

Rust proto verification modes:

- `OA_PROTO_VERIFY_MODE=fast` (default): single build snapshot + proto crate tests.
- `OA_PROTO_VERIFY_MODE=strict`: double-build deterministic snapshot check + proto crate tests.

`buf breaking` behavior is controlled with modes:

- `OA_BUF_BREAKING_MODE=auto` (default): run breaking check with timeout, but skip on transient remote/rate-limit errors so local dev is not blocked.
- `OA_BUF_BREAKING_MODE=strict`: fail on any breaking-check error (for release/hard gates).
- `OA_BUF_BREAKING_MODE=off`: skip breaking checks.

Optional timeout override:

```bash
OA_BUF_BREAKING_TIMEOUT=45s ./scripts/local-ci.sh proto
```

This lane is also invoked automatically by `changed` mode whenever `proto/`,
`buf.yaml`, `buf.gen.yaml`, `scripts/verify-proto-generate.sh`,
`scripts/verify-rust-proto-crate.sh`, or `crates/openagents-proto/` changes.

Breaking baseline override:

```bash
OA_BUF_BREAKING_AGAINST='.git#branch=origin/main,subdir=proto' ./scripts/local-ci.sh proto
```

Strict mode example:

```bash
OA_BUF_BREAKING_MODE=strict OA_BUF_BREAKING_AGAINST='.git#branch=origin/main,subdir=proto' ./scripts/local-ci.sh proto
```

Strict proto determinism example:

```bash
OA_PROTO_VERIFY_MODE=strict ./scripts/verify-rust-proto-crate.sh
```

## Proto Remediation

If proto CI fails:

1. Run `git fetch origin main` (required for strict `buf breaking` baseline checks).
2. Fix lint errors from `buf lint`.
3. If the change is intended and additive, keep field numbers stable and avoid
   renames/removals.
4. If a breaking change is truly required, move to a new package version
   namespace (for example `v2`) rather than mutating `v1`.
5. Re-run:
   - `./scripts/local-ci.sh proto`
   - `./scripts/local-ci.sh changed`

## Changed-Mode Trigger Tests

To validate changed-file lane routing logic:

```bash
./scripts/local-ci.sh test-triggers
```

## Temporary Bypass (Use Sparingly)

```bash
OA_SKIP_LOCAL_CI=1 git commit -m "..."
```

If bypassing, run equivalent lane commands manually before merge.
