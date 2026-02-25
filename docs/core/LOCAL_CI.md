# Local CI Policy

OpenAgents CI gates are local-first and executed through tracked git hooks and
scripts in this repository.

GitHub workflow automation is intentionally not used for CI in this repo.

## INV-12 Verification

Before releasing policy/invariant changes, verify no tracked workflow files exist:

```bash
git ls-files | rg '/\\.github/workflows/'
```

Expected result: no output.

## Install Hooks

From repository root:

```bash
./scripts/install-git-hooks.sh
```

This configures:

- `.githooks/pre-commit` -> `scripts/local-ci.sh changed`

## Local CI Entrypoint

```bash
./scripts/local-ci.sh changed
```

Supported lanes:

- `changed` (default)
- `all`
- `all-rust`
- `docs`
- `proto`
- `runtime`
- `runtime-history`
- `workspace-compile`
- `panic-surface`
- `allow-attrs`
- `clippy-rust`
- `cross-surface`
- `inbox-gmail`
- `test-triggers`

Examples:

```bash
./scripts/local-ci.sh runtime
./scripts/local-ci.sh runtime-history
./scripts/local-ci.sh workspace-compile
./scripts/local-ci.sh panic-surface
./scripts/local-ci.sh allow-attrs
./scripts/local-ci.sh clippy-rust
./scripts/local-ci.sh cross-surface
./scripts/local-ci.sh inbox-gmail
./scripts/local-ci.sh all-rust
./scripts/local-ci.sh all
```

`runtime` lane includes Hydra executable harnesses:

- `cargo run --manifest-path apps/runtime/Cargo.toml --bin vignette-neobank-pay-bolt11`
- `./scripts/vignette-hydra-mvp2.sh`
- `./scripts/vignette-hydra-mvp3.sh`
- expected artifact: `output/vignettes/local-ci/neobank-pay-bolt11/summary.json`

Changed-mode trigger note:

- `runtime` lane auto-runs for `apps/runtime/**`, `proto/**`, and `buf*.yaml` changes.
- `runtime-history` lane auto-runs for Rust runtime history-compat paths (`apps/runtime/src/**`, `apps/runtime/fixtures/history_compat/**`, `apps/runtime/Cargo.toml`, `Cargo.lock`) and enforces deterministic replay compatibility fixtures.
- `workspace-compile` lane auto-runs for Rust workspace paths and enforces `cargo check --workspace --all-targets`.
- `panic-surface` lane auto-runs for Rust workspace paths and enforces no-net-growth panic-surface policy against `docs/ci/panic-surface-baseline.env`.
- `allow-attrs` lane auto-runs for Rust workspace paths and enforces no-net-growth of unjustified `#[allow(...)]` usage in critical crates against `docs/ci/allow-attribute-baseline.env`.
- `clippy-rust` lane auto-runs for Rust workspace paths in `changed` mode when `OA_LOCAL_CI_ENABLE_CLIPPY=1` and runs phased clippy checks for critical crates.
- `cross-surface` lane auto-triggers for retained desktop/runtime harness paths and is opt-in in `changed` mode via `OA_LOCAL_CI_ENABLE_CROSS_SURFACE=1`.
- `inbox-gmail` lane auto-runs for Gmail inbox contract surfaces (`apps/openagents.com/service`, `apps/autopilot-desktop`, `apps/runtime/src/server*`) and executes deterministic non-live tests for inbox list/detail/actions + runtime comms ingest.

## Push Policy

Run Rust-first gates manually before pushing:

```bash
./scripts/local-ci.sh all-rust
```

Run additional gates manually before pushing when needed:

```bash
./scripts/local-ci.sh changed
./scripts/local-ci.sh workspace-compile
./scripts/local-ci.sh panic-surface
./scripts/local-ci.sh allow-attrs
OA_LOCAL_CI_ENABLE_CLIPPY=1 ./scripts/local-ci.sh changed
./scripts/local-ci.sh clippy-rust
./scripts/local-ci.sh inbox-gmail
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

Default `buf breaking` timeout in auto mode is `8s` to avoid slowing Rust issue throughput during migration.

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

## Panic-Surface No-Net-Growth Gate

`./scripts/local-ci.sh panic-surface` runs:

```bash
./scripts/panic-surface-gate.sh check
```

Policy:

- Counts production panic markers (`.unwrap(`, `.expect(`, `panic!(`) across `apps/**` + `crates/**`.
- Excludes test/example/bench/fixture paths.
- Fails if any metric exceeds baseline in `docs/ci/panic-surface-baseline.env`.

Refresh baseline only when intentional reductions are merged:

```bash
./scripts/panic-surface-gate.sh snapshot
```

## Allow-Attribute Policy

`./scripts/local-ci.sh allow-attrs` runs:

```bash
./scripts/allow-attribute-gate.sh check
```

Policy for critical crates (`apps/runtime`, `apps/openagents.com/service`, `apps/autopilot-desktop`, `crates/autopilot-core`, `crates/autopilot`):

- No-net-growth on **unjustified** `#[allow(...)]` suppressions is enforced against `docs/ci/allow-attribute-baseline.env`.
- Prefer refactoring over adding new suppressions.
- If an allow is unavoidable, annotate the immediately preceding line with `lint-allow-justified:` and a concrete reason.
- Keep the scope minimal to the specific item; avoid broad crate/module allows.

Refresh the baseline only after intentional debt reduction:

```bash
./scripts/allow-attribute-gate.sh snapshot
```

## Temporary Bypass (Use Sparingly)

```bash
OA_SKIP_LOCAL_CI=1 git commit -m "..."
```

If bypassing, run equivalent lane commands manually before merge.

Cross-surface harness opt-in in `changed` mode:

```bash
OA_LOCAL_CI_ENABLE_CROSS_SURFACE=1 ./scripts/local-ci.sh changed
```
