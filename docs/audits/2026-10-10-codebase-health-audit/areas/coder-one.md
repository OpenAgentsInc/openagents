# coder-one

**Scope:** `crates/coder-one` (132.5k lines, 194 `.rs` files). **Health grade: C.**
Audit date 2026-10-10, snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`.

`crates/coder-one` is a research and benchmark harness. It contains the Coder One issue-to-PR loop, the Microluna mini-handoff executor (`micro`, 16.6k lines), the tunable composition (`compose`, 8.1k lines), acceptance suites, about 30 `checks`, studies, contamination auditing, `ask`, and 24 CLI subcommands. No Cargo package depends on it. Gym runs the `coder-one ask` binary as a child process, and coder-delegate pulls a file from its `policies/` directory with `include_str!`. All 301 commits fall between 2026-09-22 and 2026-10-03. Microluna, the main executor, was deprecated on 2026-09-28. Most of the crate is therefore frozen benchmark evidence, sitting next to a few live surfaces: `ask`, the policy manifest, and the policies coder-delegate embeds.

Individual functions are well written. Production error handling is clean, every module has a design-level doc header, and there are 744 tests. The structure is weak. The main modules import each other in cycles, `Micro::suite_loop` is about 1,130 lines long, the CLI is hand-rolled with 276 flag match arms, and small helpers are copied many times. Binaries find their data through `CARGO_MANIFEST_DIR` paths at runtime. Two declared dependencies are unused, and 16 of the 68 policy manifests are never validated. The one security-relevant problem is in the `ask` allowlist, which does not refuse `rg --hostname-bin`. That lets an ask executor start any existing executable, though only with no arguments. The crate's size, coupling and sprawl of benchmark versions make it hard to change or retire safely.

## Measurements

| Metric | Value |
|---|---|
| Files / total LOC | 194 `.rs` / 132,534 |
| Production LOC (before each file's `#[cfg(test)]`) | ~95,317 |
| Dedicated test files (`tests.rs`, `*_tests.rs`) | 16,013 lines |
| Tests | 535 `#[test]` + 209 `#[tokio::test]`; 1 `#[ignore]`; 1 integration file (`tests/codex_login.rs`, 132 lines) |
| Largest files | micro.rs 5,035; micro/lean.rs 3,891; micro/tests.rs 3,620; compose.rs 2,906; policy.rs 2,656; compose/tests.rs 2,458; accept/mod.rs 2,443; handoff.rs 2,346; ask/mod.rs 2,192; episode.rs 2,048 |
| Module totals | micro 16,640; compose 8,106 |
| Longest functions (approx.) | `Micro::suite_loop` ~1,130; `compose::run` ~714; `study::pack::run` ~642; `ask::run` ~545; policy apply ~487; `episode::run_episode` ~482; `minitask::run` ~429; `handoff::run` ~405; main.rs solve ~371 |
| Production panics surface | 1 `unwrap`, 65 `expect`, 0 `panic!`, 0 `unsafe` |
| Other production patterns | 212 `let _ =` (28 in fire/report.rs, `writeln!`); 511 `unwrap_or_default` |
| TODO/FIXME/HACK | 0 |
| `#[allow]` | 61 (33 `too_many_lines`, 23 `too_many_arguments`, 9 `trivially_copy_pass_by_ref`, 4 `cast_precision_loss`) |
| Public API | 799 pub fns (2 unreferenced), 1,180 pub types/consts/statics |
| CLI | 24 `pub const USAGE`; 276 `"--flag" =>` arms; 19 copies of the exit-code block in main.rs |
| Environment | 29 distinct `CODER_ONE_*` vars in production code; `std::env::var` in 42 production files (29 `HOME` reads) |
| Processes | 60 `Command::new`; 35 raw `.output()`; 66 `supervise::` uses; 0 `tokio::process` |
| Scratch dirs | 63 `std::env::temp_dir()` sites in production code (`tempfile` is a dev-dependency only) |
| Data | 68 policy JSONs (52 validated via `policy::REFERENCE`); fixtures 2.6 MB / 274 files; prompts 152 KB |
| Bench coupling | 20 test references to `bench/terminal-bench/traces` (436 MB tracked in git) |
| Dependents | 0 Cargo dependents; coder-delegate `include_str!`s a policy file; gym shells out to the binary and re-declares 3 schema strings |
| Microluna coupling | `microluna::` referenced 256 times across 40 files |
| Git | 301 commits, 2026-09-22 to 2026-10-03, none since; top churn: micro/tests.rs 55, policy.rs 53, micro.rs 49, lib.rs 49 |

## Strengths

- Production error handling is clean. Code before `#[cfg(test)]` has 1 `unwrap` (study/cli.rs:105, guarded by a peek) and no `panic!`. All 65 `expect`s are on compile-time regexes or `include_str!` JSON (for example departures/mod.rs:198-209, 510). Keep this standard.
- The tests are thorough: 744 test functions, about 37k lines of test code, and named regression tests per issue. For example, policy.rs:1489 `reference_manifests_are_complete_and_valid` checks the digest and on-disk equality for 52 manifests.
- Every module opens with a design-level `//!` header that names the issue it implements (#9531, #9584, #9655, ...). All 18 `docs/*.md` paths cited in the code exist.
- Most child-process work goes through the shared `supervise` crate with wall-clock limits and output caps (66 uses, e.g. accept/runner.rs:475-497). Filesystem confinement goes through `coder-boundary`.
- The policy manifest design is sound. It uses a canonical-JSON SHA-256 digest that ignores name/note/search, a `SEARCHABLE` allowlist of fields a study may vary, and protected constants for isolation, effect policy and acceptance (policy.rs:1-60). Every env override is recorded in the resolution.
- Shared regexes are compiled once with `OnceLock` (departures/mod.rs:500-512; 42 `OnceLock`/`LazyLock` uses). The prompt-capture server binds only to `127.0.0.1:0` and never records headers (capture.rs:83).
- There is almost no dead code: only 2 of 799 pub fns are unreferenced, and there are no TODO/FIXME markers.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| C1-01 | medium | security | `ask` allowlist lets `rg --hostname-bin` run any existing executable (no arguments) | S |
| C1-02 | medium | architecture | Cyclic dependencies among the main modules | XL |
| C1-03 | medium | dead-code | Deprecated Microluna code is most of the crate and compiles in every workspace build | XL |
| C1-04 | medium | maintainability | `Micro::suite_loop` ~1,130 lines; 33 `too_many_lines` allows | L |
| C1-05 | medium | testing | 16 policy manifests, including benchmark-profile arms, never validated | S |
| C1-06 | medium | build | Runtime code hard-codes source-checkout paths via `CARGO_MANIFEST_DIR` | M |
| C1-07 | medium | architecture | coder-delegate embeds a coder-one file without a crate dependency | S |
| C1-08 | medium | docs | INVARIANTS.md cites an invariant test in the wrong crate | S |
| C1-09 | medium | maintainability | ~29 `CODER_ONE_*` env switches outside the manifest digest | M |
| C1-10 | medium | concurrency | Blocking `std::process` in async code on a current-thread runtime, no timeouts; issue-eval needs GNU `timeout` | M |
| C1-11 | low | security | `gym runs` uses a denylist while other gym families use allowlists | S |
| C1-12 | low | maintainability | `micro::Policy` has 30 fields (15 bools); 68 versioned manifests | L |
| C1-13 | low | duplication | Hand-rolled argv parsing in 24 subcommands, copied dispatch boilerplate | M |
| C1-14 | low | duplication | Small helpers copied across modules (`clip` x9, sha256/hex x10, `read_json` x14) | M |
| C1-15 | low | build | Two unused dependencies and a duplicate `coder-boundary` entry | S |
| C1-16 | low | error-handling | Hand-made `temp_dir` scratch directories with best-effort cleanup | M |
| C1-17 | low | naming | Two import paths for the same modules (re-export shim) | M |
| C1-18 | low | duplication | check-grades schema string declared three times | S |
| C1-19 | low | testing | Tests needing bench traces or a sandbox return early and pass silently | M |
| C1-20 | low | maintainability | God files: micro.rs, lean.rs, compose.rs, policy.rs | M |
| C1-21 | low | dead-code | Two unreferenced public functions and no crate README | S |

### C1-01 `ask` allowlist lets `rg --hostname-bin` run any existing executable, though only without arguments

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:** [allow.rs:1-11](../../../../crates/coder-one/src/ask/allow.rs), [allow.rs:91-99](../../../../crates/coder-one/src/ask/allow.rs)

**Evidence:** allow.rs:91-99 refuses only those `rg` args that start with `--pre`, equal `-z`, start with `--search-zip`, or are bundled short flags containing `z`. ripgrep 15.1.0, the version installed here, documents `--hostname-bin=COMMAND` ("the flag's value should correspond to an executable") and runs it when the hyperlink format uses `{host}`. Neither `--hostname-bin` nor `--hyperlink-format` is refused. The module header (allow.rs:3-5) says the allowlist "narrows what the executor may run at all".

**Impact:** An ask executor can start any executable already on disk. ripgrep calls it with no arguments, so this path cannot reach `gym runs rank` or other commands that need arguments. The read-only filesystem boundary still applies. The allowlist does not keep its stated promise, and the denylist has to be updated with every rg release.

**Suggested action:**
1. Now: refuse `--hostname-bin*` and `--hyperlink-format*` in allow.rs:91-99.
2. Replace the rg denylist with an allowlist of safe flags (`-n`, `-i`, `-l`, `-c`, `-w`, `-F`, `-e`, `-g`, `-t`, `--json`, `-A`/`-B`/`-C`, `--max-count`, `--files`, `--glob`) and refuse everything else.
3. Verify: add tests asserting refusal of `rg --hostname-bin=/bin/sh --hyperlink-format=x a .` and of an unknown long flag. Run `cargo test -p coder-one ask::allow`.

### C1-02 Cyclic dependencies among the main modules

**Severity:** medium · **Category:** architecture · **Effort:** XL

**Locations:** [policy.rs](../../../../crates/coder-one/src/policy.rs), [episode.rs](../../../../crates/coder-one/src/episode.rs), [handoff.rs](../../../../crates/coder-one/src/handoff.rs), [micro.rs](../../../../crates/coder-one/src/micro.rs), [checks/readiness.rs](../../../../crates/coder-one/src/checks/readiness.rs)

**Evidence:** Re-measured non-test `crate::` imports: policy.rs imports compose, handoff and micro. episode.rs imports compose, handoff, micro and policy. handoff.rs imports checks, episode and policy. The checks submodules (readiness.rs, reproduced.rs, oracle/*, contract/executed.rs) import `crate::micro` or `crate::compose`. This produces cycles such as episode <-> handoff and policy -> micro -> policy.

**Impact:** No subsystem can be extracted or frozen on its own. This blocks separating the deprecated Microluna parts from the live `ask` and policy surfaces.

**Suggested action:**
1. Move the data types that policy.rs and `checks/*` take from micro/compose (for example `micro::Policy` and the compose policy structs) into a leaf `spec` module with no upward imports.
2. Document the layers (spec -> checks/accept -> micro/compose executors -> episode/handoff/CLI) in `src/lib.rs`.
3. Add a unit test that scans `crate::` imports per layer and fails on any upward import.
4. Do this as part of the bench-crate split (C1-03), not as a standalone refactor.

### C1-03 Deprecated Microluna code is most of the crate and still compiles in every workspace build

**Severity:** medium · **Category:** dead-code · **Effort:** XL

**Locations:** [micro.rs](../../../../crates/coder-one/src/micro.rs), [micro/lean.rs](../../../../crates/coder-one/src/micro/lean.rs), [compose.rs](../../../../crates/coder-one/src/compose.rs), [Cargo.toml:16](../../../../crates/coder-one/Cargo.toml), [AGENTS.md:602-609](../../../../AGENTS.md), [root Cargo.toml:2-3](../../../../Cargo.toml)

**Evidence:** AGENTS.md:602-609 says microluna was "deprecated on 2026-09-28 ... stays in the workspace only for Coder One's retained Terminal-Bench policies ... Don't build new work on it". The workspace members are `crates/*` with no `default-members`, so every workspace build compiles coder-one and microluna. micro.rs (5,035), lean.rs (3,891) and compose.rs (2,906) account for much of this. The last coder-one commit is 530b207410 (2026-10-03). The issue flow already lives in `coder_delegate::issue` (AGENTS.md:610-618), so coder-one's own live surface is mainly `ask` (run by gym as a child process) plus the policies coder-delegate embeds.

**Impact:** Every workspace build and test compiles and runs about 25k lines of frozen benchmark machinery plus microluna, only to keep the `coder-one ask` binary and the policy files available.

**Suggested action:**
1. Record the decision in AGENTS.md.
2. Pick one route: (a) move `ask` into its own small crate or binary (or into gym) and leave coder-one as a frozen bench crate; or (b) put micro/compose/study/handoff/minitask/contamination/fire behind a `bench` Cargo feature that is off by default.
3. Add `default-members` to the root Cargo.toml so `cargo build`/`cargo test` at the root skip the frozen crates.
4. Verify: `cargo tree -p <live crate> -i microluna` returns nothing, and a root `cargo build` does not compile microluna.

### C1-04 `Micro::suite_loop` is about 1,130 lines; 33 functions carry `#[allow(clippy::too_many_lines)]`

**Severity:** medium · **Category:** maintainability · **Effort:** L

**Locations:** [micro.rs:2614-3748](../../../../crates/coder-one/src/micro.rs), [compose.rs:1446](../../../../crates/coder-one/src/compose.rs), [policy.rs:872](../../../../crates/coder-one/src/policy.rs), [ask/mod.rs:388](../../../../crates/coder-one/src/ask/mod.rs)

**Evidence:** micro.rs:2614 has `#[allow(clippy::too_many_lines)]` and `async fn suite_loop` starts at 2615. The next fn, `early_session`, starts at 3749, so suite_loop is about 1,134 lines. rg finds 33 `clippy::too_many_lines` and 61 `allow(` across src.

**Impact:** The core control flow of every benchmarked policy can only be tested end to end, and changes to it are hard to review.

**Suggested action:**
1. First settle C1-03. If the code stays frozen, leave it as is.
2. If it stays live, split `suite_loop` into `define_suite` / `run_round` / `choose_move` / `final_guard`, each taking a `SuiteCtx` struct. Do the same for `compose::run` and policy apply.
3. Remove each `#[allow]` once its function is under the limit.
4. Verify: `cargo clippy -p coder-one` passes with the allow removed, and the existing micro tests stay green.

### C1-05 16 policy manifests, including benchmark-profile arms, are never validated by a test

**Severity:** medium · **Category:** testing · **Effort:** S

**Locations:** [policy.rs:1152](../../../../crates/coder-one/src/policy.rs), [policy.rs:1489-1501](../../../../crates/coder-one/src/policy.rs), [agents.json:4347-4353](../../../../bench/terminal-bench/profiles/agents.json)

**Evidence:** `REFERENCE` holds 52 file names, but `policies/` has 68 files. Missing: microluna-v19, v19-fire, v19-fire-oracle, v19-fire-profile, v19-long, v19-xlong, v20, v4, v5, v6, tunable-luna-pack, tunable-luna-v2, tunable-v2, v3, v6, v7. agents.json:4353 launches `crates/coder-one/policies/microluna-v19.json`, and agents.json:2406 launches `tunable-v7.json`. `reference_manifests_are_complete_and_valid` iterates only over `REFERENCE`.

**Impact:** A schema change can silently break manifests that benchmark profiles launch.

**Suggested action:**
1. Add a test in policy.rs that reads every `*.json` under `reference_dir()`.
2. Assert that each file appears either in `REFERENCE` or in an explicit `LEGACY` list.
3. Assert that each file parses and passes `validate()`, or appears in an explicit expected-failure list for legacy files.
4. Verify: adding a stray JSON to `policies/` makes the test fail.

### C1-06 Runtime code hard-codes source-checkout paths via `env!("CARGO_MANIFEST_DIR")`

**Severity:** medium · **Category:** build · **Effort:** M

**Locations:** [policy.rs:1357-1361](../../../../crates/coder-one/src/policy.rs), [handoff.rs:1858-1863](../../../../crates/coder-one/src/handoff.rs), [issue_eval/mod.rs:40-44](../../../../crates/coder-one/src/issue_eval/mod.rs), [support/cli.rs:46](../../../../crates/coder-one/src/support/cli.rs)

**Evidence:** `reference_dir()` is `concat!(env!("CARGO_MANIFEST_DIR"), "/policies")`. `compare_path()` joins `../../bench/terminal-bench/handoff/minitask-patterns.json`. `default_set_dir()` joins `issues-eval`. support/cli.rs:46 defaults traces to `CARGO_MANIFEST_DIR/../../bench/terminal-bench/traces`. All four are in non-test code.

**Impact:** When a coder-one binary runs anywhere other than the checkout it was built in (gym launches `coder-one ask`), it looks for its data under the builder's absolute path.

**Suggested action:**
1. Add one `repo_root()` helper that resolves from a `--repo` flag or `CODER_ONE_REPO`, then by walking up to the workspace Cargo.toml, and falls back to `CARGO_MANIFEST_DIR`.
2. Route every production call site through it.
3. Print the resolved root in `coder-one doctor`.
4. Verify: `rg 'CARGO_MANIFEST_DIR' crates/coder-one/src` shows only `repo_root()` and test code.

### C1-07 coder-delegate embeds a file from coder-one's directory without a crate dependency

**Severity:** medium · **Category:** architecture · **Effort:** S

**Locations:** [coder-delegate/src/terminal.rs:202](../../../../crates/coder-delegate/src/terminal.rs), [AGENTS.md:610-618](../../../../AGENTS.md)

**Evidence:** terminal.rs:202 reads `pub const POLICY: &str = include_str!("../../coder-one/policies/jevprobe2-opus-lean-low-5m.json");`. coder-delegate was split out so that Coder builds without Microluna.

**Impact:** Archiving or moving `coder-one/policies` breaks the live Coder terminal build, and Cargo does not track this dependency.

**Suggested action:**
1. Move the JSON into `crates/coder-delegate/policies/`.
2. Have coder-one's `REFERENCE` `include_str!` it from there.
3. Verify: `rg 'coder-one/' crates/coder-delegate/src` returns nothing, and both crates build.

### C1-08 INVARIANTS.md cites an invariant test in crates/coder-one that lives in coder-delegate

**Severity:** medium · **Category:** docs · **Effort:** S

**Locations:** [INVARIANTS.md:33](../../../../INVARIANTS.md), [coder-delegate/src/adapter.rs:981](../../../../crates/coder-delegate/src/adapter.rs)

**Evidence:** INVARIANTS.md says `every_launch_marks_its_session_as_the_engines` is "in `crates/coder-one`". The only definition is in crates/coder-delegate/src/adapter.rs:981.

**Impact:** Anyone checking the engine-session-origin invariant looks in the wrong crate. Retiring coder-one could also wrongly look like it drops the test.

**Suggested action:**
1. Change the citation in INVARIANTS.md:33 to `crates/coder-delegate`.
2. Add a script that checks every test name INVARIANTS.md cites exists in the crate it names.
3. Verify: the script passes on the corrected file and fails if the crate name is reverted.

### C1-09 Env-var configuration outside the manifest digest (about 29 `CODER_ONE_*` switches)

**Severity:** medium · **Category:** maintainability · **Effort:** M

**Locations:** [policy.rs:872](../../../../crates/coder-one/src/policy.rs), [micro/lean.rs:3413](../../../../crates/coder-one/src/micro/lean.rs)

**Evidence:** lean.rs:3413 reads `std::env::var_os(super::checkpoint::ENV)` in the middle of the loop to decide whether to capture evidence. rg finds 33 distinct `CODER_ONE_*` names in src, tests included. The reviewer counted 29 in production code.

**Impact:** Run behaviour depends on ambient environment that the manifest digest does not record.

**Suggested action:**
1. Put all `CODER_ONE_*` names in one `env.rs` table.
2. Read them through that table only, and have `Resolution::record()` record every one that is set.
3. Verify: `rg '"CODER_ONE_' crates/coder-one/src` outside env.rs and tests returns nothing, and a test asserts that a set variable appears in the recorded resolution.

### C1-10 Blocking `std::process` calls inside async code on a current-thread runtime, without timeouts; issue-eval needs GNU `timeout`

**Severity:** medium · **Category:** concurrency · **Effort:** M

**Locations:** [micro.rs:144-153](../../../../crates/coder-one/src/micro.rs), [micro.rs:1735-1748](../../../../crates/coder-one/src/micro.rs), [issue_eval/grade.rs:398-403](../../../../crates/coder-one/src/issue_eval/grade.rs), [Cargo.toml:25](../../../../crates/coder-one/Cargo.toml)

**Evidence:** tokio is declared with only `[macros, rt, time]`, and `main` uses `flavor = "current_thread"`. `workspace_diff` and `git_signature` run `std::process::Command::new("git").output()` with no timeout. grade.rs:398 runs `Command::new("timeout")`, which stock macOS does not ship.

**Impact:** A hung `git` call freezes the single-threaded runtime and all its timers. The issue-eval grader fails on macOS.

**Suggested action:**
1. Route the git calls through `supervise::Job` with `Limits::within(wall)`.
2. Replace the `timeout` shell-out in grade.rs with `supervise`.
3. Verify: `rg 'Command::new\("timeout"\)' crates/coder-one/src` returns nothing, and the issue-eval grade tests pass on macOS.

### C1-11 `gym runs` uses a denylist while the other gym families use allowlists

**Severity:** low · **Category:** security · **Effort:** S

**Locations:** [allow.rs:113-140](../../../../crates/coder-one/src/ask/allow.rs), [allow.rs:15-20](../../../../crates/coder-one/src/ask/allow.rs)

**Evidence:** `terminal-bench` is checked against `TERMINAL_BENCH_READS` (allow.rs:16). `runs` accepts any subcommand except: `rank` without `--rank`, `--record`, `fingerprint(s)`/`moves` without `--no-jev` or `--cached`, and `mark`/`unmark` (allow.rs:115-139).

**Impact:** A future `gym runs` subcommand that spends Jev requests would be allowed in a read-only ask by default. The filesystem boundary still blocks writes.

**Suggested action:**
1. Add a `RUNS_READS` const in allow.rs listing the read-only `gym runs` subcommands, and check `next` against it the same way as `TERMINAL_BENCH_READS`.
2. Add a test in crates/gym that fails when a `runs` subcommand is in neither the coder-one read list nor an explicit write/spend list.
3. Verify: an unknown `gym runs foo` is refused by the allowlist test.

### C1-12 `micro::Policy` has 30 fields, 15 of them booleans, plus 68 versioned manifest files

**Severity:** low · **Category:** maintainability · **Effort:** L

**Locations:** [micro.rs:215](../../../../crates/coder-one/src/micro.rs), [policies/](../../../../crates/coder-one/policies)

**Evidence:** `pub struct Policy` at micro.rs:215 has 30 pub fields, 15 of them `bool`. `policies/` holds 68 JSON files.

**Impact:** The type can represent invalid flag combinations, and readers cannot tell which manifests matter.

**Suggested action:**
1. Move superseded manifests (not referenced by `bench/terminal-bench/profiles/agents.json` and not in `REFERENCE`) into `policies/archive/` with a README.
2. Group the booleans into sub-structs only if Microluna stays live.
3. Verify: the C1-05 all-files test still passes after the move.

### C1-13 Hand-rolled argv parsing in 24 subcommands with copied dispatch boilerplate

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:** [main.rs:60-88](../../../../crates/coder-one/src/main.rs), [main.rs:90-130](../../../../crates/coder-one/src/main.rs), [main.rs:299](../../../../crates/coder-one/src/main.rs)

**Evidence:** main.rs has 19 copies of `ExitCode::from(u8::try_from(code).unwrap_or(1))`, and src has 276 `"--flag" =>` arms. `contamination` exits 2 on error (main.rs:127, 299) while the other subcommands exit 1. USAGE (main.rs:60-87) has no `accept` line, but main.rs:102 dispatches `accept`.

**Impact:** Exit codes are inconsistent, and the help text has drifted from what the CLI actually parses.

**Suggested action:**
1. Collapse the 19 blocks into `fn exit(r: Result<i32, String>, fail: u8) -> ExitCode`.
2. Add `accept` to USAGE.
3. Add a test that every dispatched subcommand name appears in USAGE.
4. Moving to clap derive is optional and only worthwhile if the crate stays live.

### C1-14 Small helpers copied across modules (`clip` x9, sha256/hex x10, `read_json` x14)

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:** [checks/behavior.rs:855](../../../../crates/coder-one/src/checks/behavior.rs), [micro.rs:1724](../../../../crates/coder-one/src/micro.rs), [accept/mod.rs:1116](../../../../crates/coder-one/src/accept/mod.rs), [issue_eval/mod.rs:232](../../../../crates/coder-one/src/issue_eval/mod.rs), [policy.rs:1363](../../../../crates/coder-one/src/policy.rs), [episode.rs:1421](../../../../crates/coder-one/src/episode.rs), [contamination/lexicon.rs:262](../../../../crates/coder-one/src/contamination/lexicon.rs), [component/extract.rs:434](../../../../crates/coder-one/src/component/extract.rs)

**Evidence:** There are 9 `fn clip` and 14 `fn read_json` definitions. Hash helpers: `sha256` in micro.rs:1724, accept/mod.rs:1116 and issue_eval/mod.rs:232; `sha256_hex` in lexicon.rs:262 and extract.rs:434; `hex` in policy.rs:1363 and episode.rs:1421. Despite its name, checks/behavior.rs:855 `fn hex` runs `Sha256::digest`.

**Impact:** A fix to one copy does not reach the others, and a function called `hex` that computes SHA-256 is easy to misuse.

**Suggested action:**
1. Add `src/util.rs` with `sha256_hex`, `hex`, `clip` and `read_json<T>`.
2. Replace the copies with calls to it.
3. Rename behavior.rs `hex` to `sha256_hex`.
4. Verify: `rg 'fn (clip|read_json|sha256|sha256_hex|hex)\b' crates/coder-one/src` finds only util.rs.

### C1-15 Two unused dependencies in Cargo.toml (`plugin`, `coder-history`) and a duplicate `coder-boundary`

**Severity:** low · **Category:** build · **Effort:** S

**Locations:** [Cargo.toml:13](../../../../crates/coder-one/Cargo.toml), [Cargo.toml:17](../../../../crates/coder-one/Cargo.toml), [Cargo.toml:27-28](../../../../crates/coder-one/Cargo.toml)

**Evidence:** `rg 'plugin::|coder_history'` over src/ and tests/ finds nothing. `coder-boundary` is listed under both `[dependencies]` and `[dev-dependencies]`.

**Impact:** The crate has extra compile edges, and the declared dependencies misrepresent what it uses.

**Suggested action:**
1. Remove `coder-history` and `plugin` from `[dependencies]`.
2. Remove `coder-boundary` from `[dev-dependencies]`.
3. Verify: `cargo test -p coder-one` still builds and passes.

### C1-16 Hand-made `temp_dir` scratch directories with best-effort cleanup

**Severity:** low · **Category:** error-handling · **Effort:** M

**Locations:** [issue_eval/grade.rs:393-404](../../../../crates/coder-one/src/issue_eval/grade.rs), [compose/best_of.rs:303](../../../../crates/coder-one/src/compose/best_of.rs)

**Evidence:** src has 72 `std::env::temp_dir()` uses, tests included. grade.rs creates and removes its scratch directory with `let _ =`. best_of.rs:303 names directories `coder-one-best-of-{stamp}-{i}` and does not check for collisions.

**Impact:** Directories leak on early return, and parallel runs can collide.

**Suggested action:**
1. Move `tempfile` to `[dependencies]`.
2. Use `tempfile::Builder` for scratch directories, and call `keep()` where a directory is kept as evidence.
3. Verify: `rg 'temp_dir\(\)' crates/coder-one/src` count drops to the deliberately retained sites.

### C1-17 Two import paths for the same modules (re-export shim from the coder-delegate split)

**Severity:** low · **Category:** naming · **Effort:** M

**Locations:** [lib.rs:55-59](../../../../crates/coder-one/src/lib.rs)

**Evidence:** lib.rs:55-59 re-exports 31 modules with `pub use coder_delegate::{action, adapter, ..., usage}`.

**Impact:** It is unclear which crate owns the code being edited.

**Suggested action:**
1. Rewrite `crate::<reexported>::` paths to `coder_delegate::`.
2. Remove the shim.
3. Verify: `cargo build -p coder-one` succeeds with lib.rs:55-59 deleted.

### C1-18 check-grades schema string declared three times, in coder-one and gym

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:** [review_rule.rs:65](../../../../crates/coder-one/src/review_rule.rs), [grade/mod.rs:50](../../../../crates/coder-one/src/grade/mod.rs), [gym/src/runs_card.rs:54-60](../../../../crates/gym/src/runs_card.rs)

**Evidence:** The string `"openagents.coder-one.check-grades.v1"` appears in review_rule.rs:65 (`GRADES_SCHEMA`), grade/mod.rs:50 (`SCHEMA`) and gym runs_card.rs:60. `executed-command.v1` appears in gym runs_card.rs:54.

**Impact:** A version bump can update one copy and miss the others.

**Suggested action:**
1. In review_rule.rs, use `pub use crate::grade::SCHEMA as GRADES_SCHEMA;`.
2. Move the schemas gym reads into a shared crate or into coder-delegate, and import them in gym.
3. Verify: `rg 'check-grades.v1' crates` finds one definition.

### C1-19 Tests that read bench traces or need a sandbox return early and pass silently

**Severity:** low · **Category:** testing · **Effort:** M

**Locations:** [checks/recover.rs:409-420](../../../../crates/coder-one/src/checks/recover.rs)

**Evidence:** recover.rs:412-417 contains `let Ok(all) = recover_tree(..) else { return; }; if all.is_empty() { return; }`. The traces tree has 14,744 tracked files, about 436 MB.

**Impact:** On hosts without the traces or a sandbox, these tests pass without checking anything.

**Suggested action:**
1. Copy the trace files these tests need into fixtures, or
2. Turn each silent return into an explicit skip that fails when `CODER_ONE_REQUIRE_FIXTURES=1`.
3. Verify: running with `CODER_ONE_REQUIRE_FIXTURES=1` and the traces missing fails loudly.

### C1-20 God files: micro.rs 5,035 lines, lean.rs 3,891, compose.rs 2,906, policy.rs 2,656

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:** [micro.rs](../../../../crates/coder-one/src/micro.rs), [micro/lean.rs](../../../../crates/coder-one/src/micro/lean.rs), [compose.rs](../../../../crates/coder-one/src/compose.rs), [policy.rs](../../../../crates/coder-one/src/policy.rs)

**Evidence:** `wc -l` confirms all four sizes.

**Impact:** The files are hard to navigate, and changes pile up merge conflicts in them.

**Suggested action:**
1. Split policy.rs into `policy/{manifest,validate,overrides,reference}.rs`, since policy is a live surface.
2. Replace `use super::*` with explicit imports in the micro submodules.
3. Verify: `cargo test -p coder-one policy` passes and no file in `policy/` exceeds ~1,000 lines.

### C1-21 Two unreferenced public functions and no crate README

**Severity:** low · **Category:** dead-code · **Effort:** S

**Locations:** [review_rule.rs:222-226](../../../../crates/coder-one/src/review_rule.rs), [accept/verify.rs:71-79](../../../../crates/coder-one/src/accept/verify.rs)

**Evidence:** An rg search across crates/ finds no caller of `checked_kind` or `test_questions`. Only `test_questions_with` is used. `crates/coder-one/README.md` does not exist.

**Impact:** A small amount of dead public surface, and no entry document for the crate.

**Suggested action:**
1. Delete `checked_kind` and `test_questions`.
2. Add a README that marks which subcommands are live (`ask`) and which are frozen.
3. Verify: `cargo build -p coder-one` succeeds.
