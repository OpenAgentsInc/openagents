# Tests, CI, and verification

Scope: repo-wide (182 root-workspace packages, plus the excluded `crates/psionic` and `crates/openagents-mobile` workspaces). Covers test counts per crate, ignored and skipping tests, sleeps and network use in tests, snapshot and golden hygiene, test code in production builds, CI and gating, formal-methods claims in INVARIANTS.md, and fixture sprawl. Audit date 2026-10-10, snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`.

**Health grade: C**

The repo has a large, mostly well-built test base: 15,862 `#[test]`/`#[tokio::test]` in the root workspace (about 2.31M Rust LOC), 5,341 more in the excluded psionic workspace and 231 in the excluded mobile workspace. Only two root-workspace crates (coder-chat-web and plugin-outline) have no tests. Ignored tests almost always state why (153 of 155), fakes for security-sensitive seams are usually feature-gated, and snapshot helpers fail on a missing snapshot instead of writing one.

The weak part is deciding when the tests run. Nothing runs them automatically. There is no CI (GitHub Actions workflows have been deleted in 12 commits), the only git hook is opt-in and checks file size, and the full gate (`verify-rust.sh --release`) runs only at release and writes its records to a gitignored directory. The daily policy is `cargo test -p <edited crate>`, and `scripts/verify-changed.py` never adds the crates that depend on the edited one. At about 5,100 commits a month, with nostr having 58 dependents and coder 706 commits in 30 days, a break in a downstream crate stays on main until someone runs the release gate. The gate also cannot test the psionic and mobile workspaces, INVARIANTS.md cites test names that no longer exist, and a bounded-model artifact for relay admission has lost its checker. Test hygiene issues: 131 tests skip silently and report a pass, timing sleeps decide outcomes in large suites, helpers are copied between crates, 411 integration-test binaries slow the targeted loop, and `bench/` tracks 1.16 GB.

## Measurements

| Metric | Value |
|---|---|
| Root-workspace packages | 182 (`cargo metadata --no-deps`) |
| Rust LOC, root workspace | ~2,310,550; test LOC (tests/ dirs and `*tests.rs`) ~426,672 |
| Rust tests, root workspace | 15,862 `#[test]`/`#[tokio::test]` |
| Rust tests, excluded workspaces | psionic 5,341 (~1.23M LOC, 2,438 files); openagents-mobile 231 (29,230 LOC) |
| `#[ignore]` | 155 total; 2 bare (no reason); 9 for the Grid ball/blocks feature switched off 2026-10-01 |
| Crates with 0 tests | coder-chat-web (990 LOC), plugin-outline (205 LOC) |
| Lowest density (tests/kLOC, crates >3k LOC) | everglade-web 0.53, verse-zone-grove 1.46, commercial-spend 2.05, pylon 2.25, coder-labor 2.57, coder-ui 2.77 |
| Integration-test binaries (`crates/*/tests/*.rs`) | 411; openagents-cli 42, coder 35, nostr-relay 19, gateway 19 |
| Silent-skip tests (`eprintln!` "skip..." then return) | 131 |
| Sleeps in integration tests | 336 (gateway/tests/serve.rs: 16 in 6,328 lines); 26 of whole seconds, incl. 30s/60s/120s |
| Files using tokio paused time | 5 |
| Formal methods | 0 kani proofs (one leftover `cfg(kani)` check-cfg); 0 TLA/quint; proptest in 2 crates (nostr, pay-ledger); loom 0; cargo-fuzz 0 |
| INVARIANTS.md | 372 KB, 293 rows, 1,319 cited test identifiers, ~20 not found in tracked code |
| CI | No `.github/workflows` (only `.github/ISSUE_TEMPLATE`); 12 commits deleted workflows |
| Hooks | `.githooks/pre-commit`: large-file check only, enabled by hand via `core.hooksPath` |
| Snapshot/fixture bless variables | 6 conventions (UPDATE_SNAPSHOTS, OPENAGENTS_UI_BLESS, EXT_EVAL_BLESS, UPDATE_FIXTURES, UPDATE_CLASSIFY_FIXTURES, ignored "writes the fixture" tests) |
| Tracked `bench/` | 1,158,692 KB in 21,894 files (terminal-bench/traces: 873 MB, 14,744 files); 2,771 files added since 2026-10-05 |
| Tracked `__pycache__` files | 603 |
| Python test files | 25 `test_*.py`/`test-*.py`; ~8 not run by any gate phase |
| Dependents / 30-day commits | nostr 58 / 137; coder 15 / 706; pay-ledger 17 / 37; rust-native 17 / 58 |

## Strengths

- Large, real test base. Money and auth crates use scenario tests: pay-ledger has 123 tests across 17 integration files, including `tests/invariants.rs` with proptest; tenancy 239; coder-access 124; receipts 80; x402 76.
- Ignored tests are documented: 153 of 155 `#[ignore]` carry a reason ("calls the live judge", "requires a GPU...", "reaches the deployed decision worker..."), so live, GPU and quota-spending tests are opt-in.
- No integration test calls an external URL directly. A grep for `get`/`post`/`connect("https://...")` in tests, excluding loopback and `.invalid` hosts, found nothing. AGENTS.md forbids Cargo tests from reaching real hosts, and the desktop's `migrate::start` panics under `cfg(test)` if one does.
- Fakes for security-sensitive seams stay out of production builds: [oa-auth/src/lib.rs](../../../../crates/oa-auth/src/lib.rs) line 23 (`#[cfg(feature = "fake")] pub mod fake;`) and [coder-desk/src/lib.rs](../../../../crates/coder-desk/src/lib.rs) line 55 (`#[cfg(any(test, feature = "test-support"))]`). Every `features = ["fake"|"test-support"]` and tokio `test-util` is under `[dev-dependencies]`.
- Snapshot helpers fail on a missing or changed snapshot rather than recording one ([openagents-desktop/src/tests.rs](../../../../crates/openagents-desktop/src/tests.rs) 333-349, [openagents-deck/src/snapshot.rs](../../../../crates/openagents-deck/src/snapshot.rs) 53-70). The deck also rejects snapshots no slide uses ([openagents-deck/src/lib.rs](../../../../crates/openagents-deck/src/lib.rs) 160-180).
- The gate tooling is itself engineered and tested: [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) has named phases, `--print`/`--list`, retries on resource exhaustion, a preflight and per-phase records; [scripts/gate-record.py](../../../../scripts/gate-record.py) ties a pass to the tree digest it covered; `scripts/tests/test_gate_*.py` unit-test the scoping logic.
- Slow and live suites are fenced: soak, postgres and metal sit behind explicit flags (`--with-soak`, `--skip-postgres`, `--with-metal`), and lease classes (release-gate, bench) serialize heavy runs.
- A large-file guard ([scripts/dev/check-large-files.sh](../../../../scripts/dev/check-large-files.sh) plus allowlist, #11110) and a bucket path for bench output ([scripts/bench-artifacts.py](../../../../scripts/bench-artifacts.py)) landed on 2026-10-09 and start reversing the bench sprawl.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-TEST-01 | High | build | Nothing runs automatically: no CI, an opt-in size-only hook, and a release-only full gate with unshared results | M |
| X-TEST-02 | High | testing | Changed-crate scope never includes dependents; a Cargo.lock-only change runs no cargo phases | M |
| X-TEST-03 | High | testing | Gate cannot cover the excluded psionic and openagents-mobile workspaces and builds an invalid `-p` | M |
| X-TEST-04 | Medium | testing | At least 8 "Checked by" test names in INVARIANTS.md do not exist in code | S |
| X-TEST-05 | Medium | testing | Orphaned relay-admission bounded-model artifact with no generator or consumer | M |
| X-TEST-06 | Medium | testing | 131 tests print "skipped" and pass, including sandbox-enforcement tests | M |
| X-TEST-07 | Medium | testing | Wall-clock sleeps decide outcomes; only 5 files use tokio paused time | M |
| X-TEST-08 | Medium | performance | 411 separate integration-test binaries, each linking a large crate | M |
| X-TEST-09 | Medium | repo-hygiene | `bench/` tracks 1.16 GB in 21,894 files, incl. ~600 `__pycache__` files | L |
| X-TEST-10 | Medium | testing | Several Python test files run in no gate phase, incl. a pay-reconcile test | S |
| X-TEST-11 | Low | duplication | Polling and snapshot helpers copied across crates | M |
| X-TEST-12 | Low | maintainability | Test fakes compiled into production libraries without a feature gate | S |
| X-TEST-13 | Low | dead-code | 9 tests ignored for the switched-off Grid ball/blocks feature; 2 bare `#[ignore]` | S |
| X-TEST-14 | Low | testing | Little property or concurrency testing for money parsing, sealing, custody | M |
| X-TEST-15 | Low | testing | Two crates with zero tests; very low density in some UI/web and money crates | M |

### X-TEST-01 Nothing runs automatically: no CI, an opt-in hook that checks only file size, and a full gate that runs only at release with unshared results

Severity: High · Category: build · Effort: M

Locations:
- [.githooks/pre-commit](../../../../.githooks/pre-commit) (pre-commit:1-4)
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) (verify-rust.sh:137-163)
- [AGENTS.md](../../../../AGENTS.md) (AGENTS.md:57-66)
- [.gitignore](../../../../.gitignore) (.gitignore:6)

Evidence: `.github` contains only `ISSUE_TEMPLATE`. `git log --diff-filter=D -- .github/workflows/*` lists 12 deleting commits (489dfbbf35, 12dd399e71, c439925670, 48633e7192, 14e66f8439, 4ff25c0b7f, ...). `git grep -il 'github actions'` over `scripts/`, INVARIANTS.md and `docs/verification.md` returns nothing, so the invariant and check restored in 489dfbbf35/12dd399e71 are gone again. `.githooks/pre-commit` only execs `scripts/dev/check-large-files.sh` and says "Enable once per clone: git config core.hooksPath .githooks". AGENTS.md:59-66 (Velocity, owner 2026-10-01) makes `cargo test -p` on edited crates plus `cargo fmt` the default and rules out other crates' tests, clippy and the release gate. `.gitignore:6` ignores `/.coder/`, where verify-rust records are written.

Impact: Breakage in dependent crates, feature combinations or clippy lands on main and stays there until someone runs the release gate. The next agent inherits a red tree without knowing it.

Suggested action (keeps the owner's no-GitHub-Actions and velocity policy):
1. Add `scripts/dev/nightly-gate.sh`: create a clean worktree from `origin/main` and run `./scripts/verify-rust.sh --release --keep-going`.
2. Run it from a systemd timer on the coderos host, following the existing `deploy/` timer units.
3. Publish the result (commit, phases, pass/fail) to one pinned GitHub issue so agents can check whether main is red before starting.
4. Restore the no-workflows row in INVARIANTS.md, checked by `scripts/tests/test_gate_no_workflows.py` (picked up by the existing `test_gate_*.py` discovery in the gate-tooling phase).
5. Verify: break a dependent crate on a scratch worktree, run the nightly script, and confirm the pinned issue shows the failing phase; confirm `test_gate_no_workflows.py` fails when a dummy `.github/workflows/x.yml` is added.

### X-TEST-02 The changed-crate scope never includes dependent crates, and a Cargo.lock-only change runs no cargo phases

Severity: High · Category: testing · Effort: M

Locations:
- [scripts/verify-changed.py](../../../../scripts/verify-changed.py) (verify-changed.py:71-97)
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) (verify-rust.sh:146-153, 160-163, 268-272)

Evidence: `resolve()` adds only `package_of(root, parts[1])` for each `crates/<x>/` path and never adds reverse dependencies. `resolve(['Cargo.lock'])` returns `workspace=True, crates=[]`. In development mode, verify-rust.sh:151-152 then sets `WORKSPACE_SCOPE=0` and prints "development checks remain scoped. Select affected consumers with --crates"; `run_phase` (268-272) then reports every scoped cargo phase as "SCOPED OUT ... no affected crates".

Impact: A change to a widely used crate (nostr, rust-native, pay-ledger) passes the scoped check while its consumers break. A dependency bump runs zero cargo phases unless the agent picks consumers by hand.

Suggested action:
1. In verify-changed.py, build a reverse path-dependency map from `cargo metadata --format-version 1 --no-deps` and emit `dependents` (direct dependents by default).
2. Add `--with-dependents` to verify-rust.sh that runs `cargo check --all-targets -p <dependents>`, keeping the cost bounded.
3. For a Cargo.lock-only diff, map the package names changed in `git diff Cargo.lock` to the workspace crates that depend on them instead of producing an empty plan.
4. Verify: add cases to `scripts/tests/test_gate_scope.py` asserting that a dependent appears for a change to its dependency, and that a Cargo.lock change never yields an empty crate list.

### X-TEST-03 The gate cannot cover the excluded psionic and openagents-mobile workspaces, and builds an invalid -p for them

Severity: High · Category: testing · Effort: M

Locations:
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:1-11)
- [scripts/verify-changed.py](../../../../scripts/verify-changed.py) (verify-changed.py:60-79)
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) (verify-rust.sh:384-392)

Evidence: Root `Cargo.toml` excludes `crates/openagents-mobile` and `crates/psionic`. `resolve(['crates/psionic/crates/psionic-serve/src/x.rs', 'crates/openagents-mobile/src/a.rs'])` returns crates `['openagents-mobile', 'psionic']`. `crates/psionic/Cargo.toml` is a `[workspace]` root with no package name, so `-p psionic` is passed to a root workspace where no such package exists. openagents-mobile is a package but outside the root workspace, so `-p openagents-mobile` also fails. No cargo phase in verify-rust.sh uses `--manifest-path`.

Impact: Changes to psionic (about 5.3k tests) or mobile have no supported gate path; the scoped gate errors instead of testing them.

Suggested action:
1. In verify-changed.py, key each changed path to its nearest `Cargo.toml` containing `[workspace]`. For psionic, map `crates/psionic/crates/<x>/` to that sub-crate's package name.
2. Emit `workspaces: {path: [pkgs]}`.
3. In verify-rust.sh, loop the fmt, clippy and test phases per workspace with `--manifest-path <ws>/Cargo.toml`.
4. Verify: add a `test_gate_scope.py` case asserting that a psionic-serve path resolves to workspace `crates/psionic`, package `psionic-serve`; run the scoped gate on a one-line psionic change and confirm it executes tests rather than erroring.

### X-TEST-04 At least 8 'Checked by' test names in INVARIANTS.md do not exist in the code

Severity: Medium · Category: testing · Effort: S

Locations:
- [INVARIANTS.md](../../../../INVARIANTS.md)
- [crates/coder-computers/src/terminal/screen.rs](../../../../crates/coder-computers/src/terminal/screen.rs) (screen.rs:451)

Evidence: 8 cited names were spot-checked. Each appears once in INVARIANTS.md and zero times as `fn <name>` in tracked .rs/.py/.swift/.ts files outside `bench/`:
- `a_toggle_allows_and_removes_a_provider_and_keeps_the_settings_valid`
- `the_judge_answers_first_and_the_model_follows_its_opener`
- `the_primary_is_space_bunny_whenever_openrouter_is_reachable`
- `a_refusal_carries_its_code_and_wait`
- `a_tapped_suggestion_never_shows_again_even_after_a_relaunch` (found only in a bins/openagents-ios verification README)
- `a_wrong_answer_report_carries_the_exchange`
- `the_catalog_plugins_have_their_real_statuses`
- `a_build_without_the_live_service_refuses_clearly`, a rename: screen.rs:451 has `fn a_build_without_the_live_service_refuses_clearly_and_closes`.

The reviewer's full count was about 20; that total was not re-run.

Impact: The ledger claims coverage that no longer exists, so a policy can lose its only check without anyone noticing.

Suggested action:
1. Add `scripts/check-invariants.py`: extract backticked snake_case identifiers (containing `_`, at least 8 characters) from the Checked-by column; require each to exist as a test function name in a tracked source file; require cited paths to exist.
2. Run it in the gate-tooling phase through a `scripts/tests/test_gate_invariants.py` wrapper so existing `test_gate_*.py` discovery picks it up.
3. Fix current misses: update renamed citations such as screen.rs:451, and mark rows whose tests were deleted as unchecked.
4. Verify: the checker exits 0 on the fixed ledger and fails when a cited name is changed to a nonexistent one.

### X-TEST-05 An orphaned formal-model artifact: the relay admission bounded-model results have no generator or consumer

Severity: Medium · Category: testing · Effort: M

Locations:
- [tests/fixtures/nip01/admission-state-model.json](../../../../tests/fixtures/nip01/admission-state-model.json)
- [tests/fixtures/README.md](../../../../tests/fixtures/README.md) (README.md:41)
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (Cargo.toml:126)

Evidence: Outside `bench/`, `git grep` for `admission-state-model` and `histories_checked` matches only the fixture itself (which records `"histories_checked": 111111`). The fixture was last touched in 9b5bb212f1 "Extract the public relay". `nip19/keys.json` is mentioned only in tests/fixtures/README.md:41 and has no code consumer. crates/openagents-mobile/Cargo.toml:126 holds `check-cfg = ['cfg(kani)']`, with no kani proofs anywhere.

Impact: The workspace contract says to run or document the checker and turn counterexamples into regression tests. The checker's output survives here but the checker does not.

Suggested action:
1. Either restore the checker as `crates/nostr-relay/tests/admission_model.rs`, enumerating the bounded action sequences, asserting the fixture's history count, and replaying each recorded counterexample as a named test;
2. or delete both orphaned fixtures and record the removal in tests/fixtures/README.md.
3. Remove the dead `cfg(kani)` check-cfg either way.
4. Verify: `git grep admission-state-model` shows either a consumer test that passes under `cargo test -p nostr-relay`, or no matches.

### X-TEST-06 131 tests print 'skipped' and return as passes, including sandbox-enforcement tests

Severity: Medium · Category: testing · Effort: M

Locations:
- [crates/coder-boundary/tests/boundary.rs](../../../../crates/coder-boundary/tests/boundary.rs) (boundary.rs:505-510)

Evidence: `git grep -i 'eprintln!(".*skip'` over `crates/*.rs`, excluding psionic, returns exactly 131 matches. boundary.rs:506-509: `if !backend() { eprintln!("skipped: no enforced boundary on this host"); return; }`. `git grep` for `OPENAGENTS_TEST_REQUIRE` and `REQUIRE_BOUNDARY` finds nothing.

Impact: On a host without an enforcing backend, the sandbox boundary tests report green.

Suggested action:
1. Add a `require_or_skip(cap, available)` helper that panics when `OPENAGENTS_TEST_REQUIRE` names the capability (or is `all`), and otherwise skips as today.
2. Replace the `eprintln!`/`return` blocks with it, starting with coder-boundary.
3. Export the variable in the `--release` gate on hosts known to have those capabilities.
4. Verify: with `OPENAGENTS_TEST_REQUIRE=boundary` on a host without the backend, `cargo test -p coder-boundary` fails; the 131-match grep count trends to zero.

### X-TEST-07 Wall-clock sleeps decide test outcomes; only 5 files use tokio paused time

Severity: Medium · Category: testing · Effort: M

Locations:
- [crates/gateway/tests/serve.rs](../../../../crates/gateway/tests/serve.rs) (serve.rs:2175-2176, serve.rs:5291)

Evidence: serve.rs:2175-2176: `// Let acme's first item dispatch before globex's call lands. tokio::time::sleep(Duration::from_millis(60))`. serve.rs:5291 sleeps 150ms between reading `next_cursor` and asserting the cursor has expired. Exactly 5 files under `crates/` (excluding psionic) use `start_paused` or `time::pause`. The reviewer counted 336 sleeps in integration tests, 26 of whole seconds (up to 120s); that total was not re-counted.

Impact: Timing-based ordering flakes on loaded shared hosts, and multi-second sleeps lengthen the targeted test loop.

Suggested action:
1. Replace the 60ms ordering sleep with a `Notify` or barrier that the fake backend signals when the first dispatch starts.
2. Make cursor TTL configurable or inject a clock so the expiry test advances time instead of sleeping.
3. Use `#[tokio::test(start_paused = true)]` where the code under test uses `tokio::time`.
4. Verify: run `cargo test -p gateway --test serve` under CPU load (e.g. parallel `stress`) 20 times with no failures, and compare wall time before and after.

### X-TEST-08 411 separate integration-test binaries, each linking a large crate

Severity: Medium · Category: performance · Effort: M

Locations:
- [crates/openagents-cli/tests](../../../../crates/openagents-cli/tests)
- [crates/coder/tests](../../../../crates/coder/tests)

Evidence: `ls crates/*/tests/*.rs | wc -l` = 411; openagents-cli has 42, coder 35.

Impact: Each file links its own executable, so `cargo test -p` on these crates pays link time and target-dir space many times over, which works against the fast targeted loop the Velocity policy relies on.

Suggested action:
1. Consolidate `tests/*.rs` into `tests/it/main.rs` with `mod` lines for openagents-cli, coder, gateway, nostr-relay and pay-ledger.
2. Keep re-exec and global-state tests (gym `locked_ledger`, coder-lease `dead_holder`) as separate targets.
3. Verify: measure `cargo test -p <crate> --no-run` time and target-dir size before and after; test counts reported by `cargo test -p <crate>` must not drop.

### X-TEST-09 bench/ holds 1.16 GB and 21,894 tracked files, including about 600 __pycache__ files

Severity: Medium · Category: repo-hygiene · Effort: L

Locations:
- [bench/terminal-bench/traces](../../../../bench/terminal-bench/traces)
- [scripts/dev/check-large-files.sh](../../../../scripts/dev/check-large-files.sh)
- [.gitignore](../../../../.gitignore) (.gitignore:10)

Evidence: `git ls-files bench | wc -l` = 21,894 of 38,827 tracked files; tracked bench files total 1,158,692 KB (terminal-bench/traces alone 873 MB in 14,744 files); 2,771 bench files added since 2026-10-05. 601 tracked `__pycache__` paths are under bench (603 repo-wide, including `scripts/tests/__pycache__`). `.gitignore` ignores `__pycache__` only under `/training/**` (line 10).

Impact: Higher clone, fetch and grep cost, and evidence directories that look like test fixtures.

Suggested action:
1. Add `__pycache__/` and `*.pyc` to the root `.gitignore` and `git rm -r --cached` the 603 files.
2. Move `bench/terminal-bench/traces` to the artifact bucket (via `scripts/bench-artifacts.py`) and keep manifests in git.
3. Add a cap on new bench files per commit to `check-large-files.sh`.
4. Verify: `git ls-files | grep -c __pycache__` is 0; `git ls-files bench | wc -l` drops by about 14.7k; a commit adding more than the cap of bench files is rejected by the hook.

### X-TEST-10 Several Python test files are not run by any gate phase, including a pay reconciliation timer test

Severity: Medium · Category: testing · Effort: S

Locations:
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) (verify-rust.sh:374-383)
- [scripts/tests/test_pay_reconcile_unit.py](../../../../scripts/tests/test_pay_reconcile_unit.py)
- [scripts/tests/test_boat_run_legacy.py](../../../../scripts/tests/test_boat_run_legacy.py)
- [scripts/test-coder-host.py](../../../../scripts/test-coder-host.py)
- [scripts/test-install-coder-hosted.py](../../../../scripts/test-install-coder-hosted.py)
- [scripts/test-wait-job-logs.py](../../../../scripts/test-wait-job-logs.py)
- [scripts/release/test_linux_terminal.py](../../../../scripts/release/test_linux_terminal.py)

Evidence: The gate discovers only `test_gate_*.py`, `test_verification_phase.py`, `test_check_coder_delegation_run.py`, `test_backup_media.py` and `test_fetch_kev_artifacts.py` (verify-rust.sh:374-383). None of the listed files is referenced outside itself (apart from a stray `.pyc`). The hyphenated `test-*.py` names cannot be discovered by unittest at all.

Impact: Install, host and pay-reconcile tests rot silently.

Suggested action:
1. Add a `scripts` phase: `python3 -B -m unittest discover -s scripts/tests -p 'test_*.py'`, plus `scripts/release`.
2. Rename `scripts/test-*.py` to `scripts/tests/test_*.py`.
3. Have verify-changed.py map `scripts/` and `deploy/` paths to that phase.
4. Verify: `./scripts/verify-rust.sh --print` lists the new phase, and the phase's run reports the test count of all 25 Python test files (or explicitly excludes any that need live hosts).

### X-TEST-11 Polling and snapshot helpers are copied across crates instead of shared

Severity: Low · Category: duplication · Effort: M

Locations:
- [crates/eval-runner/tests/defaults.rs](../../../../crates/eval-runner/tests/defaults.rs) (defaults.rs:25)
- [crates/eval-runner/tests/hosted.rs](../../../../crates/eval-runner/tests/hosted.rs) (hosted.rs:118)
- [crates/eval-runner/tests/runner.rs](../../../../crates/eval-runner/tests/runner.rs) (runner.rs:23)
- [crates/openagents-desktop/src/tests.rs](../../../../crates/openagents-desktop/src/tests.rs)
- [crates/openagents-terminal/src/app_tests.rs](../../../../crates/openagents-terminal/src/app_tests.rs)
- [crates/openagents-deck/src/snapshot.rs](../../../../crates/openagents-deck/src/snapshot.rs)

Evidence: An identical `async fn until<T>(within: Duration, mut find: impl FnMut() -> Option<T>) -> Option<T>` appears in 3 eval-runner test files. Bless/update variables are spread across about 15 files in several crates (openagents-ui, ext-eval, microcoder, gateway, desktop, terminal, deck, coder-terminal), using 6 different conventions.

Impact: Helper fixes must be repeated in every copy, and there are several ways to regenerate golden files.

Suggested action:
1. Create `crates/test-support` (`publish = false`, dev-dependency only) with `until`/`wait_for` and `snapshot::check` under one bless variable.
2. Migrate the eval-runner copies and the desktop, terminal and deck snapshot helpers first.
3. Verify: `git grep -n 'async fn until<T>'` returns only the shared crate; one bless variable regenerates snapshots in each migrated crate.

### X-TEST-12 Test fakes are compiled into production libraries without a feature gate

Severity: Low · Category: maintainability · Effort: S

Locations:
- [crates/coder/src/claim.rs](../../../../crates/coder/src/claim.rs) (claim.rs:1365)
- [crates/coder/src/eval_author.rs](../../../../crates/coder/src/eval_author.rs) (eval_author.rs:795)
- [crates/codex-transport/src/lib.rs](../../../../crates/codex-transport/src/lib.rs) (lib.rs:22)
- [crates/retail-cloud/src/lib.rs](../../../../crates/retail-cloud/src/lib.rs) (lib.rs:40)
- [crates/coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) (gce.rs:1600-1605)
- [crates/ext-eval/src/door.rs](../../../../crates/ext-eval/src/door.rs) (door.rs:346-347)
- [crates/coder-delegate/src/delegate.rs](../../../../crates/coder-delegate/src/delegate.rs) (delegate.rs:3311-3312)

Evidence: Each location has a bare `pub mod fake` or `#[doc(hidden)] pub mod testing` with no cfg attribute; their doc comments say "for tests" or "Not for production use". retail-cloud's own docs (dispatch.rs:17, provision.rs:8) describe `crate::fake` as the simulation used by tests.

Impact: Test doubles ship in release builds, and production code can reach them.

Suggested action:
1. Gate each with `#[cfg(any(test, feature = "test-support"))]` and enable the feature through a self dev-dependency, as oa-auth and coder-desk already do.
2. If retail-cloud's fake-payment acceptance binary needs its fake, give it an explicit named feature.
3. Verify: `cargo check -p <crate>` (no features) compiles, and `cargo test -p <crate>` still passes for each crate.

### X-TEST-13 Nine tests are ignored for the Grid ball/blocks feature that was switched off, and two #[ignore] have no reason

Severity: Low · Category: dead-code · Effort: S

Locations:
- [crates/verse/src/runtime.rs](../../../../crates/verse/src/runtime.rs)
- [crates/verse/src/session.rs](../../../../crates/verse/src/session.rs)
- [crates/verse/src/spectator.rs](../../../../crates/verse/src/spectator.rs)
- [crates/verse/src/zones/tests.rs](../../../../crates/verse/src/zones/tests.rs)
- [crates/coder-mobile/src/verse_app.rs](../../../../crates/coder-mobile/src/verse_app.rs)
- [crates/coder-mobile/src/verse_surface.rs](../../../../crates/coder-mobile/src/verse_surface.rs)
- [crates/coder-mobile/src/bare_bodies_tests.rs](../../../../crates/coder-mobile/src/bare_bodies_tests.rs)
- [crates/gym/tests/locked_ledger.rs](../../../../crates/gym/tests/locked_ledger.rs) (locked_ledger.rs:225-226)
- [crates/rust-native/src/layout/tests.rs](../../../../crates/rust-native/src/layout/tests.rs) (tests.rs:671-672)

Evidence: `git grep -c "Grid's ball"` gives 9 hits across those 7 files (feature switched off 2026-10-01). locked_ledger.rs:226 and layout/tests.rs:672 use a bare `#[ignore]`; their doc comments explain them (a child-process entry point and a benchmark), but the attributes carry no reason string.

Impact: The disabled feature's code has no running tests and will drift. The bare ignores are cosmetic.

Suggested action:
1. Park the ball/blocks code behind a `grid-ball` cargo feature that the `--release` clippy-features phase builds, or delete it with its tests.
2. Add reason strings to the two bare ignores (`#[ignore = "..."]`).
3. Verify: `git grep -c "Grid's ball"` is 0 or all hits sit under `cfg(feature = "grid-ball")`; a grep for `#[ignore]$` returns nothing.

### X-TEST-14 Little property or concurrency testing for money parsing, sealing and custody

Severity: Low · Category: testing · Effort: M

Locations:
- [crates/bitcoin-amount/src/tests.rs](../../../../crates/bitcoin-amount/src/tests.rs)
- [crates/oa-seal/src/lib.rs](../../../../crates/oa-seal/src/lib.rs)
- [crates/wallet/src/custody.rs](../../../../crates/wallet/src/custody.rs)

Evidence: Only crates/nostr and crates/pay-ledger declare proptest (apart from a plugin fixture Cargo.toml). crates/wallet/src/custody.rs has 0 `#[test]`, and crates/wallet/tests contains only testnet.rs. No loom or cargo-fuzz anywhere.

Impact: Edge cases in amount parsing and in custody's once-only and tamper properties are covered only by examples, or not at all.

Suggested action:
1. Add proptest round-trip and no-panic tests to bitcoin-amount parsing/formatting.
2. Add a byte-flip tamper proptest to oa-seal (any single flipped byte must fail to open).
3. Add unit tests in custody.rs for a wrong HMAC, file mode, and `Permit::take` succeeding exactly once across threads.
4. Verify: `cargo test -p bitcoin-amount -p oa-seal -p wallet` runs the new tests; deliberately breaking the once-only check makes the thread test fail.

### X-TEST-15 Two root-workspace crates have zero tests, and some UI and web crates have very low test density

Severity: Low · Category: testing · Effort: M

Locations:
- [crates/coder-chat-web](../../../../crates/coder-chat-web)
- [crates/plugin-outline](../../../../crates/plugin-outline)
- [crates/everglade-web](../../../../crates/everglade-web)
- [crates/pylon/src/market.rs](../../../../crates/pylon/src/market.rs)
- [crates/commercial-spend/src/refunds.rs](../../../../crates/commercial-spend/src/refunds.rs)

Evidence: coder-chat-web has 0 tests in 990 LOC, plugin-outline 0 in 205, everglade-web 2 in 3,809 (0.53/kLOC). Reviewer densities for pylon (2.25/kLOC) and commercial-spend (2.05/kLOC) were not re-measured.

Impact: Thin coverage on paid-market and refund code, where under the edited-crate-only policy a crate's own tests are its only check.

Suggested action:
1. Add unit tests to pylon `market.rs` and `provider.rs`: offer and price selection, refusals, serde round trips.
2. Add unit tests to commercial-spend `refunds.rs` and `statements.rs`.
3. Add one golden test to plugin-outline.
4. Verify: re-run the per-crate test count; no root-workspace crate reports 0 tests.
