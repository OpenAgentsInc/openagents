# Rust verification

Run the manual gate from a contributor machine or non-GitHub infrastructure:

```sh
./scripts/verify-rust.sh
```

[`rust-toolchain.toml`](../rust-toolchain.toml) pins Rust, Clippy, and rustfmt
to **1.97.1**. [`rustfmt.toml`](../rustfmt.toml) pins Rust and formatter style
editions to **2024**. Run the pinned formatter once for formatting-only
changes; do not mix a workspace reformat with behavioral fixes.

## Package policy and minimum versions

Workspace packages inherit edition 2024, `publish = false`, and Rust **1.95**
from the root manifest. Kev is the explicit exception: its standalone library
retains Rust **1.94** support, while its workspace test dependencies require
1.95. Every package inherits workspace Rust and Clippy lints, including Nostr
and the relay. Package-license metadata remains pending the owner decision
documented in [the dependency policy](dependencies.md).

Install 1.95.0 and 1.94.0 alongside the pinned gate toolchain to check those
minimums. The script compiles workspace targets with runtime features on
1.95.0 and Kev's standalone library on 1.94.0. These are minimum-compiler
checks, not a claim that all runtime tests ran on both compilers.

## Feature and infrastructure coverage

The gate checks formatting, strict Clippy, and tests for both default features
and `kev/serve,lev/serve,gym/tui,jev/blocking`. It then checks minimum compiler
versions, dependency policy, and disposable PostgreSQL acceptance. It stops
on the first failed command; later commands have not run when that happens.

The PostgreSQL script needs `initdb`, `pg_ctl`, `createdb`, `curl`, Python 3,
and ordinary shell tools on PATH. It creates disposable local databases;
never redirect its destructive-test environment to a production database.
Use a separate `CARGO_TARGET_DIR` per worktree. The acceptance script uses
that target directory for the relay binary it starts.

Optional coverage is explicit:

- `--skip-postgres` prints a skip and makes the run partial. It does not
  satisfy PostgreSQL release acceptance.
- `--with-metal` runs strict Kev Clippy and tests with `serve,metal` on a
  supported Apple host with the required Apple toolchain. A successful
  compile does not establish inference results for external weights.
- `--with-soak` runs the existing long-running relay soak against a disposable
  PostgreSQL cluster. Routine runs print that it was skipped.

FoundationModels tests require the Swift helper, an eligible Apple host, and
model availability. Build the helper with `./scripts/build-lev-bridge.sh` and
follow `docs/lev/` for model-backed verification. Tests that conditionally
return without an available model are not evidence of live inference.
Kev checkpoint conformance, model experiments, hosted Jev calls, and production
relay/worker proofs require their own documented inputs and records. The
routine gate does not download weights or authorize new paid measurements.

Shell orchestration and retained Python training/acceptance tooling are
infrastructure, not additional product implementation languages. Keep product
code in Rust, with the existing Swift FoundationModels bridge exception.
Preserve `docs/transcripts/` and do not add GitHub-billed automation.

## Current verification record

On 2026-09-20, the workspace passed strict all-target Clippy on Rust 1.97.1
with the runtime feature combination after enabling lint inheritance for
Nostr and the relay. The workspace also compiled all targets with those
features on Rust 1.95.0. Kev's standalone library compiled on Rust 1.94.0 and 1.94.1.

The disposable PostgreSQL acceptance script also passed with a separate
`CARGO_TARGET_DIR`, including store, gateway, multiprocess, import, release-load,
and binary deployment checks. The long-running soak and Metal checks were not
run for this package-policy change.

Formatting remains outstanding under #9402; the pinned `cargo fmt --all
--check` reports differences. This means the full manual gate does not yet
pass. The script is the entry point, not a substitute for completed results.
#9429 also retains the broader README/router and historical-plan review.
