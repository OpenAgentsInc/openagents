# Rust verification

## Choose checks for the change

Documentation-only changes do not require the Rust verification gate, including
before a commit or push. Do not run the full gate for prose edits, documentation
moves, navigation changes, or documentation references in code comments.

For documentation reorganizations, check local links, referenced paths, and
preservation of retained artifacts. If moving a document requires updating an
embedded-document path such as `include_str!`, check only the affected consumer
and any relevant formatting. A documentation path update does not justify
workspace-wide Clippy, tests, compiler compatibility checks, or PostgreSQL
acceptance. Unrelated test failures do not block a documentation-only push.

## Development checks and release verification

Day-to-day issue work uses targeted checks on the pinned toolchain. Check the
changed behavior and the relevant consumers, then commit and push. A full
workspace run is not required for ordinary development, integration, issue
closure, or a push. Never stop independent issues to wait for release checks.
Fix a relevant failure; record an unrelated failure separately and keep moving.

```sh
./scripts/verify-rust.sh                         # changed-package fmt, Clippy, tests
./scripts/verify-rust.sh --crates coder,gym       # explicitly affected packages
./scripts/verify-rust.sh --phases tests --crates coder
cargo test -p coder task::                       # focused regression checks are valid
./scripts/verify-rust.sh --print                 # inspect the plan without running it
```

The default comparison is `origin/main`, including uncommitted changes. Use
`--changed=REF` to choose another base. Changes to a lockfile or workspace
configuration do **not** silently expand development checks to every package.
Select affected consumers with `--crates` and record any coverage deferred to
release. Select feature, PostgreSQL, or infrastructure phases only when relevant
to the change. Documentation-only work needs link and artifact checks instead.

Before a full release, explicitly request the full manual matrix:

```sh
./scripts/verify-rust.sh --release
```

This opt-in runs the standard workspace, feature, dependency, and PostgreSQL
checks. It is release preparation, never a prerequisite for taking the next
issue. Nothing runs automatically: there are no verification git hooks or
GitHub workflows. Use `--list` to see phases, `--keep-going` to collect independent
failures, and `--with-metal` or `--with-soak` for optional coverage. A previous
standard full run took 1003.7 seconds (about 17 minutes); actual time depends on
cache warmth, changed dependencies, and machine load. Targeted checks avoid
paying that cost for every issue.

The cancellation fixtures require a current Python runtime. On macOS,
Apple's system Python 3.9 fails their known-good runner; Python 3.13 passes
the same check. Put the installed Homebrew runtime first for the gate:

```sh
export PATH="/opt/homebrew/opt/python@3.13/libexec/bin:$PATH"
python3 --version
```

This selects the test runtime without changing the system interpreter.

`--changed` maps `crates/<name>/` paths to packages and maps the data
directories `coder` loads (`programs/`, `questions/`, `capabilities/`,
`sources/`) to it. In release mode only, workspace-wide files expand that
scope to the workspace. Feature flags narrow to selected packages. A change
with no affected crates scopes the Cargo phases out. Scoped runs record
`partial` coverage; that label does not mean their selected checks failed or
that development must wait for a full run.

Every run writes `.coder/verification/<run-id>/run.json` (override with
`--record-dir`, disable with `--no-record`): run ID, start and end UTC,
elapsed, the tree it covered (HEAD, dirty flag, diff digest), the phases
requested, each phase's command, exit, elapsed, attempts, and log path, the
skipped phases and why, and the result. A pass binds to that tree; it is not
a standing fact about "the gate." Reuse it only for the coverage it names.

The `preflight` phase runs first and fails fast on the environmental
prerequisites the later phases assume: a file-descriptor limit of at least
2048 (worktree fan-out tests exhaust less; the script first tries raising
the soft limit itself), `cargo`, `python3`, `git`, and `rustup` on PATH, and
free disk. Fix what it names and rerun; it does not skip or weaken a check.

When a phase fails and its log shows resource exhaustion — file-descriptor
pressure, address reuse, or `EAGAIN` — the gate retries it once and records
both attempts. `--no-retry` disables that. A retry triggered by a signature
is not a pass over a defect; the log names why it ran.

[`rust-toolchain.toml`](../rust-toolchain.toml) pins Rust, Clippy, and rustfmt
to **1.97.1**. [`rustfmt.toml`](../rustfmt.toml) pins Rust and formatter style
editions to **2024**. Run the pinned formatter once for formatting-only
changes; do not mix a workspace reformat with behavioral fixes.

## Package policy and compiler version

The workspace compiles on one compiler: **1.97.1**, the version
[`rust-toolchain.toml`](../rust-toolchain.toml) pins and the root manifest's
`rust-version` declares. Every package inherits edition 2024,
`publish = false`, the pinned Rust version, and the workspace Rust and
Clippy lints — including Kev, Laya, Nostr, and the relay, which carry no
per-crate overrides. Package-license metadata remains pending the owner
decision documented in [the dependency policy](dependencies.md).

## Feature and infrastructure coverage

The release gate checks formatting, strict Clippy, and tests for both default features
and `kev/serve,lev/serve,gym/tui,jev/blocking`. It then checks dependency
policy and disposable PostgreSQL acceptance. It stops
on the first failed command; later commands have not run when that happens.
It also runs the small Python artifact-acquisition regression suite before
Rust checks; this suite needs Python 3 and no model weights or network.
Backup collection regressions also run before Rust checks. They force a blob
rename during collection and verify that missing or corrupt bytes prevent
publication. The PostgreSQL phase separately tests backup and restore under
concurrent uploads and deletions.
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
Kev checkpoint conformance replays every committed variant on CPU for
hours, so `crates/kev/tests/conformance.rs` runs only under
`KEV_CONFORMANCE=1` — an operator sets it by hand, and nothing automatic
does. `KEV_VARIANT=<id>` narrows the battery to one variant. Model
experiments, hosted Jev calls, and production relay/worker proofs
require their own documented inputs and records. The routine gate does
not download weights or authorize new paid measurements.

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

## Progress during verification

Each manual-gate phase prints its name when it starts, an elapsed-time heartbeat
at least every 30 seconds while its command runs, and its elapsed time and exit
status when it finishes. Command output streams directly to the terminal and,
when a run record is being kept, is teed to that phase's log file. Cargo
tests use `--nocapture`, so test diagnostics appear while tests run rather than
only after a failure. The gate still stops at the first failing phase unless
`--keep-going` was passed; a heartbeat reports activity, not success or a
timeout extension.

The phase runner forwards interrupt, termination, and hangup signals to the
command's process group and waits for the direct child. It does not add a test
timeout, change feature or device settings, or skip an assertion. A test that
prints no internal progress still produces the elapsed-time heartbeat.
