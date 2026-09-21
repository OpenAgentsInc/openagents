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
It also runs the small Python artifact-acquisition regression suite before
Rust checks; this suite needs Python 3 and no model weights or network.
It also checks the delegation-result validator against completed, refused,
incomplete, and inconsistent execution records before the Rust checks.

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
Coder delegation tests need an enforceable filesystem boundary, `bwrap` on
Linux with user namespaces enabled. On a host without one, the cases in
`crates/coder/tests/program_run.rs` and
`crates/coder/tests/suite_questions.rs` that run or offer a `delegate` step
print `skipping:` and return, which is not evidence that delegation works.
Install `bubblewrap` to run them.
Kev checkpoint conformance, model experiments, hosted Jev calls, and production
relay/worker proofs require their own documented inputs and records. The
routine gate does not download weights or authorize new paid measurements.

Shell orchestration and retained Python training/acceptance tooling are
infrastructure, not additional product implementation languages. Keep product
code in Rust, with the existing Swift FoundationModels bridge exception.
Preserve `docs/transcripts/` and do not add GitHub-billed automation.

## Dependency policy

[`deny.toml`](../deny.toml) is the dependency policy: advisories, licenses,
sources, and bans over the resolved workspace graph with all features and
development dependencies. The manual command that enforces it is:

```sh
./scripts/check-dependencies.sh
```

It requires `cargo-deny` 0.20.2 on the pinned toolchain; install it with
`cargo +1.97.1 install cargo-deny --version 0.20.2 --locked`. The script
refuses when the `RUSTSEC-2024-0436` exception is overdue for review or the
resolved `paste` version differs from the reviewed one, then runs
`cargo deny --locked check advisories licenses sources bans`. If `cargo-deny`
is absent, `./scripts/verify-rust.sh` prints `SKIPPED: dependency policy`
and continues; that result is a partial gate, not a pass.
[The dependency policy](dependencies.md) records the exception's owner,
reason, and review date, the `paste` dependency paths, and the license
review.

## Current verification record

On 2026-09-20, the workspace passed strict all-target Clippy on Rust 1.97.1
with the runtime feature combination after enabling lint inheritance for
Nostr and the relay. The workspace also compiled all targets with those
features on Rust 1.95.0. Kev's standalone library compiled on Rust 1.94.0 and 1.94.1.

The disposable PostgreSQL acceptance script also passed with a separate
`CARGO_TARGET_DIR`, including store, gateway, multiprocess, import, release-load,
and binary deployment checks. The long-running soak and Metal checks were not
run for this package-policy change.

On 2026-09-20 at `fc385a13a` and the Jev fix after it, the whole gate passed
on a Linux host (Ubuntu, x86-64, Rust 1.97.1): formatting, both strict
Clippy runs, both test runs, the Rust 1.95.0 and 1.94.0 checks, the
dependency policy, and the PostgreSQL acceptance script including the
release-load proof. Debian and Ubuntu install the PostgreSQL server binaries
under `/usr/lib/postgresql/<version>/bin`, which is not on PATH by default;
prepend it before running the gate. Soak and Metal were not run.

## Apple serving matrix, 2026-09-20

The [Apple serving verification record](lev/measurements/2026-09-20-apple-serving-matrix.md)
records the actual #9426 commands on an Apple M5 Max running macOS 26.4.
Lev default and `serve` tests and strict Lev/Kev Clippy passed. Weighted Kev
Metal F32 conformance passed for all four retained variants, but the full
`serve,metal` test command failed in its CPU HTTP round-trip test at the
unchanged 10-second client deadline. bf16 remains unmeasured. This is a partial
matrix, not a successful full gate; #9426 stays open.

## Progress during the manual gate

Each manual-gate phase prints its name when it starts, an elapsed-time heartbeat
at least every 30 seconds while its command runs, and its elapsed time and exit
status when it finishes. Command output streams directly to the terminal. Cargo
tests use `--nocapture`, so test diagnostics appear while tests run rather than
only after a failure. The gate still stops at the first failing phase; a
heartbeat reports activity, not success or a timeout extension.

The phase runner forwards interrupt, termination, and hangup signals to the
command's process group and waits for the direct child. It does not add a test
timeout, change feature or device settings, or skip an assertion. A test that
prints no internal progress still produces the elapsed-time heartbeat.
