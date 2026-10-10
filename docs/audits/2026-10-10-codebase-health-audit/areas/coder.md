# coder crate

Scope: `crates/coder` [CD]: router, cli_route, program runtime, delegation, relay worker, customer/plugins, eval tooling, product_kb, efficiency, and `src/task` (180 files including sales/CRM and the workshop-agent subsystem). Snapshot commit `3168c986aa`, audited 2026-10-10.

**Health grade: C**

`crates/coder` has 262k lines (245k in `src`, 17k in `tests/`) and holds much more than "the Coder agent". It contains the chat router and its eval tooling, the program runtime, delegation doors, the NIP-CJ relay worker binary, the customer and plugin surfaces, a 39k-line sales/CRM pipeline (`task/sales`), a 26-module workshop-agent subsystem (`task/agent_*`), studio and studio simulation, disk-target maintenance, and CLI frontends that print to stdout. 61 workspace crates sit in its transitive dependency closure and 24 crates depend on it. Code inside individual functions is careful. Only about 260 unwrap/expect/panic sites are outside test modules, `unsafe` is rare and annotated, the sales store uses real file locks, private files get owner-only modes, `task/owner.rs` runs git through a hardened path, and there are about 98k lines of tests. The weaknesses are structural:

- 48 functions over 200 lines.
- `runtime.rs` alone has 5.4k production lines.
- 2,494 `pub fn` let dead modules go unnoticed.
- Host wiring is duplicated across two binaries and has drifted.
- Keyword lists route work mode and memory capture, which breaks the workspace semantic-routing rule.
- The jobs file drops records it does not understand.
- Git is spawned from PATH in more than 30 places.
- Metering failures are discarded silently.
- `build.rs` rebuilds the crate on every `git add`.

Function-level quality is roughly B. The crate's architecture and its role as a hub bring the grade down to C.

## Measurements

| Metric | Value |
|---|---|
| Tracked files | 403 |
| Rust LOC | 262,296 total (src 244,925; tests/ 17,371) |
| Test code | ~98k lines (~40.2k in `*_tests.rs`/tests files in src, ~41.1k inline `#[cfg(test)]`, 17.4k in tests/); production ~164k |
| Test functions | 1,931 (`#[test]`/`#[tokio::test]`); 29 `#[ignore]` (live judge/Devin/hosted) |
| Module sizes | task/ 180 files, 135,594 lines (task/sales 52 files, 39,123); cli_route 38,175 (34,034 are generated `tree.json`, 1.0 MB); router 12,011; bin 8,408 |
| Largest files | runtime.rs 9,576 (5,409 production); bin/coder-worker.rs 7,065 (3,933 production); task/autostart.rs 5,381; task/local.rs 4,087; task/agent_host.rs 3,975; delegate.rs 3,647 |
| Functions | 7,832; 198 over 100 lines; 48 over 200 lines |
| Longest functions | coder-worker `routed` 743; router_claim `record` 526; coder-worker `serve` 490; router/rubric `route` 483; sales `apply_with_evidence_root` 474 |
| Panic sites | unwrap 7,362 / expect 314 / panic! 211; ~260 outside test modules |
| Errors | 764 `Result<_, String>` signatures; 7 typed error enums; no thiserror/anyhow |
| Discards | 354-358 `let _ =` (agent_host.rs: 44) |
| `#[allow]` | 42 (23 `too_many_arguments`) |
| unsafe | ~20 real sites, 15 SAFETY comments |
| Public surface | 2,494 `pub fn` vs 283 `pub(crate)`/`pub(super)`; 1,272 pub types; 61 `pub mod` in lib.rs, 67 in task.rs |
| Globals / threads | 25 process-global statics; 29 detached `std::thread::spawn` in production |
| Bare `Command::new("git")` | 38 sites |
| Out-of-crate `include_str!` | 24 (docs/, deploy/, questions/, gym/, bench/) |
| Dependencies | 50+ direct; 61 workspace crates transitive; 24 transitive dependents; dev-deps include openagents-desktop and openagents-chat-app |
| Churn | 706 commits in last 30 days (887 since 2025-12-13); hottest: tree.json 145, bin/coder-worker.rs 81, task.rs 77, lib.rs 64, task/autostart.rs 62 |
| TODO/FIXME | 1 |

## Strengths

- Production code rarely panics. Of about 7.9k unwrap/expect/panic sites, about 260 are outside test modules, and many of those are lookups right after an insert, where the invariant holds ([task/sales.rs](../../../../crates/coder/src/task/sales.rs) sales.rs:1236). Poisoned mutexes are handled with `unwrap_or_else(PoisonError::into_inner)` throughout.
- Tests are extensive and close to the code: about 98k lines and 1,931 test functions. Live-model tests are `#[ignore]` with a stated reason (tests/router_eval.rs:741, tests/delegation.rs:305), and evals are fixture-driven (fixtures/chat-router, fixtures/nip-cj).
- `task/sales.rs` uses a real cross-process lock (`open_lock`/`take_lock` on `sales.lock`, sales.rs:523) and checks file identity again before persisting (`verify_same_file`, sales.rs:641). Other multi-process state should copy this pattern.
- `task/owner.rs:1021` `git` is a good model for running a subprocess safely. It uses a fixed, root-owned binary path (`GIT_PATHS`) and `env_clear`, disables `GIT_CONFIG_NOSYSTEM`/`GLOBAL`, and has a 10 s time limit and a 64 KiB output cap.
- Private state is handled deliberately. `crate::private` gives 0700/0600 on Unix and owner-only DACLs on Windows. `task.rs:1246` `replace_file` writes a temp file, fsyncs it, renames it and syncs the directory. `artifact.rs` uses `openat` with `O_NOFOLLOW`.
- Module docs are thorough and explain why the design is the way it is (runtime.rs's four NIP-PRG rules, delegate.rs's "what this is not"). No broken path references were found in comments.
- Policy decisions are encoded in types and tests. For example, autostart.rs:119-130 still reads the legacy `max_steps`/`wall_seconds` fields but ignores them, matching the no-step-limit decision, and a test at line 4053 checks that they are not written back.
- `unsafe` blocks are small and annotated (coder-worker.rs:2020 zeroes the decrypted BYOK envelope; task/cli.rs:444 calls `setsid` in `pre_exec`).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| CD-01 | high | policy | Keyword word-lists pick work mode and decide stored memories | M |
| CD-02 | high | build | build.rs reruns on every `git add` in the monorepo | S |
| CD-03 | high | architecture | God crate: sales, workshop agents, studio, relay worker, eval tooling in `coder` | XL |
| CD-04 | medium | duplication | Host serve wiring copy-pasted between `coder` and `openagents`, already drifted | M |
| CD-05 | medium | error-handling | Jobs file drops unknown records on write; one unknown field fails the file | S |
| CD-06 | medium | error-handling | Budget metering, journal and memory writes silently discarded | M |
| CD-07 | medium | security | 30+ `git` spawns resolve from PATH despite stated policy | M |
| CD-08 | medium | dead-code | doctor.rs, waves.rs, store.rs (1,808 lines) have no callers | S |
| CD-09 | medium | maintainability | bin/coder-worker.rs: 7,065 lines, ~740-line function | L |
| CD-10 | medium | maintainability | runtime.rs 5.4k production lines; ~48 functions over 200 lines | L |
| CD-11 | medium | architecture | ~2,500 `pub fn`, 61 `pub mod`: dead code invisible to rustc | M |
| CD-12 | medium | error-handling | ~764 `Result<_, String>` signatures | L |
| CD-13 | medium | duplication | Six+ atomic/private file-write implementations | S |
| CD-14 | medium | concurrency | In-process `static Mutex` claims machine-wide merge exclusion | M |
| CD-15 | medium | build | 24 `include_str!("../../..")` compile docs/ and other trees into behavior | M |
| CD-16 | medium | architecture | Library modules print to stdout/stderr and embed CLI frontends | M |
| CD-17 | low | testing | App-crate integration tests live in coder/tests | S |
| CD-18 | low | maintainability | Two unrelated "delegate" subsystems with clashing names | S |
| CD-19 | low | repo-hygiene | 1 MB pretty-printed tree.json is the most-churned file | S |
| CD-20 | low | docs | Unreferenced router fixtures and stale docs | S |
| CD-21 | low | duplication | Small helper duplication (unix_now, hex) | S |
| CD-22 | low | performance | Quota brake rewrites counts file on every admitted job | S |
| CD-23 | low | performance | Per-worktree remote-override cache never expires | S |

### CD-01 Keyword word-lists pick the agent's work mode and decide what becomes a stored memory

Severity: high. Category: policy. Effort: M.

Locations:
- [task/agent_host.rs](../../../../crates/coder/src/task/agent_host.rs): agent_host.rs:3768-3830, agent_host.rs:2137-2148
- [task/agent_memory.rs](../../../../crates/coder/src/task/agent_memory.rs): agent_memory.rs:409-441
- [task/agent_recall.rs](../../../../crates/coder/src/task/agent_recall.rs): agent_recall.rs:229-232

Evidence: `choose_mode` (agent_host.rs:3773) returns Terminal when the first word is in `terminal_first` (`["run","check","show",...]`). It returns Task when the first word is in `task_words` (`["fix","change","add",...]`), or when the text contains `" and {w} "` or starts with `"please {w} "`. Its doc comment says "A word list stands in for the spec's typed question until it has a measured threshold." agent_host.rs:2137 stores `agent_memory::proposed_preference(&queued.text)` as a Preference memory. `proposed_preference` (agent_memory.rs:413) fires when any of `"always "`, `"never "`, `"i prefer "`, `"from now on"` or `"in the future"` appears anywhere in the text. `remembered` routes on a `"remember"`/`"note that"` prefix.

Impact: "Never mind, show me the diff" is stored as an owner preference. "Can you add a test?" goes to Terminal mode, because "can" is in neither list. This breaks the binding workspace semantic-routing rule (CLAUDE.md, "Semantic Routing And Retrieval").

Suggested action:
1. Define one typed question set in the style of `questions/agent-steer.json`, loaded through `LazyLock` the way agent_steer.rs and agent_recall.rs already do. It returns `{mode: task|terminal, memory: none|remember|preference}` with a calibrated threshold.
2. Below the threshold, abstain: use Terminal mode and store no memory. Keep an explicit `Mode` passed by the owner as deterministic input.
3. Add a labeled fixture (including the two misroute examples above) and an eval that reports accuracy at the chosen threshold.
4. Delete `choose_mode`'s word lists, the `proposed_preference` and `remembered` keyword logic, and the tests that pin them.
5. Verify: run `rg '"always "|terminal_first|task_words' crates/coder/src` and expect no hits; the new eval passes on the fixture.

### CD-02 build.rs reruns on every `git add` anywhere in the monorepo and runs a full-repo `git status` each time

Severity: high. Category: build. Effort: S.

Locations:
- [build.rs](../../../../crates/coder/build.rs): build.rs:14-55

Evidence: build.rs watches `git rev-parse --git-path` for HEAD, `index`, `packed-refs` and the branch ref (lines 28-38). It then runs `git status --porcelain --untracked-files=no` (line 50) unless `CODER_BUILD_DIRTY` is set, and emits `CODER_GIT_COMMIT`/`CODER_GIT_TREE` as rustc-env. The doc at lines 6-9 says watching `index` is intentional.

Impact: Every stage, commit or checkout in the monorepo reruns the build script. That dirties the coder lib unit, and because cargo has no early cutoff, its 24 dependents rebuild too. Multi-agent work stages and commits constantly.

Suggested action:
1. Stop watching `index`. Watch only HEAD and the branch ref.
2. Take dirtiness only from `CODER_BUILD_DIRTY`, as set by `scripts/install-coder.sh`, and drop the `git status` call.
3. Better: move the stamp into a tiny `coder-version` crate that main.rs/about.rs read, so only that crate and the final binaries rebuild.
4. Verify: `git add` an unrelated file, then `cargo build -p coder -v` should show coder as `Fresh`.

### CD-03 God crate: sales CRM, workshop agents, studio, relay worker and eval tooling all live in `coder`

Severity: high. Category: architecture. Effort: XL.

Locations:
- [Cargo.toml](../../../../crates/coder/Cargo.toml): Cargo.toml:6
- [lib.rs](../../../../crates/coder/src/lib.rs)
- [task.rs](../../../../crates/coder/src/task.rs)
- [task/sales.rs](../../../../crates/coder/src/task/sales.rs): sales.rs:1-30
- [router_claim.rs](../../../../crates/coder/src/router_claim.rs): router_claim.rs:1-20

Evidence: The Cargo.toml:6 description says "The Coder agent: Classify routes, Generate answers, the terminal draws it." Meanwhile task/ holds sales (52 files, ~39k lines), 26 `agent_*` modules, studio/studio_sim, media, landing and spare. Eval-only modules (router_claim, router_eval, chat_goldens, eval_author) ship in the library.

Impact: An edit to sales or workshop-agent code recompiles 24 dependents, including Verse and web. Ownership boundaries are invisible, and the crate cannot be split into smaller compile units.

Suggested action:
1. Extract `coder-sales` (task/sales/**). Move `open_lock`/`take_lock`/`verify_same_file` into a small shared fs crate that both sides use.
2. Extract `coder-agents` (task/agent_*).
3. Extract `coder-eval` (router_claim, router_eval, chat_goldens, eval_author), as a dev dependency or behind a feature.
4. Extract `coder-worker` (the relay worker binary and its logic; see CD-09).
5. Keep `pub use` re-exports in coder for one release so openagents-cli compiles unchanged.
6. Verify each step with `cargo check --workspace --all-targets`, and check that editing a sales file no longer rebuilds Verse/web (`cargo build -v`).

### CD-04 Host serve wiring is copy-pasted between `coder` and `openagents` binaries and has already drifted

Severity: medium. Category: duplication. Effort: M.

Locations:
- [coder/src/main.rs](../../../../crates/coder/src/main.rs): main.rs:355-418
- [openagents-cli/src/main.rs](../../../../crates/openagents-cli/src/main.rs): main.rs:565-626

Evidence: Both build the same `open` closure (Inbox, Autostart + `spawn_sweeper`, `agent_host::Agents`). Both make the same `set_local_coder`/runner/engines/result, `set_studio_sim` and `set_facts` calls. Only openagents-cli wires `coder_cloud::operator::Operator` + `coder_environment_operator::attach`, `background::JevJudge::from_env()` -> `set_judge`, and `set_services(background::HostServices::at)`. coder main.rs also re-scans argv with `windows(2).find(|pair| pair[0] == "--root")`.

Impact: `coder host serve` and `openagents host serve` behave differently: the coder one has no operator cloud, no background judge and no host services. Every new capability has to be added in two places.

Suggested action:
1. Add `coder::host::install(...)` in a new `src/host.rs`. It registers the shared `set_local_*`, studio_sim and facts hooks and builds the base opener.
2. Have openagents-cli inject the cloud, judge and services layers on top.
3. Take the root from coder-host's parsed options instead of scanning argv.
4. Alternatively, retire the `coder host` entrypoint in favor of `openagents host`.
5. Verify: one source of the set_* calls (`rg 'set_studio_sim' crates/` shows a single production call site).

### CD-05 Standing-jobs file drops records it does not understand on the next write, and one unknown field fails the whole file

Severity: medium. Category: error-handling. Effort: S.

Locations:
- [task/agent_jobs.rs](../../../../crates/coder/src/task/agent_jobs.rs): agent_jobs.rs:223-252, agent_jobs.rs:128-133, agent_jobs.rs:336-348

Evidence: `load` keeps only jobs where `job.schema == SCHEMA && job.v == 1 && job.requires.is_empty()`. `meter` and `edit` do load -> modify -> `save(&jobs)`, so they write back only the filtered jobs. `Job` is `#[serde(deny_unknown_fields)]`, so a single extra field makes `from_str::<Vec<Job>>` fail for the whole file. `save` writes with `std::fs::write` (umask mode), then `let _ = set_permissions(0o600)`, then renames, with no fsync.

Impact: An older binary sharing a root with a newer one deletes v2 jobs and jobs with `requires` on any metering tick. A field added in a newer version breaks every job operation. A crash between the write and the rename can leave a truncated file, and the file is briefly world-readable.

Suggested action:
1. Parse into `Vec<serde_json::Value>`. Keep records that do not parse or belong to another version as raw values, and write them back unchanged.
2. Drop `deny_unknown_fields` for reads.
3. Route `save` through `task.rs` `replace_file` (durable rename + directory sync) with a private-mode create (see CD-13).
4. Add a test that a v2 job and a job with an unknown field survive `meter()`.

### CD-06 Budget metering, journal writes and memory writes are silently discarded (~358 `let _ =` sites)

Severity: medium. Category: error-handling. Effort: M.

Locations:
- [task/agent_host.rs](../../../../crates/coder/src/task/agent_host.rs): agent_host.rs:3476, agent_host.rs:2137-2145, agent_host.rs:2648-2653
- [task/agent_jobs.rs](../../../../crates/coder/src/task/agent_jobs.rs): agent_jobs.rs:336-348

Evidence: agent_host.rs:3476 `let _ = Jobs::new(store.clone()).meter(&job, usd);`. agent_host.rs:2652 `let _ = studio.attach_remote_task(&task, &remote_task);`. agent_host.rs:2138 `let _ = memory.add(...)`. rg counts 358 `let _ =` in src, 44 of them in agent_host.rs.

Impact: If a jobs write fails, the spend is never recorded, so a job can exceed its budget with no trace. Failed journal appends lose audit entries without a sign. A failed attach leaves a remote task stranded.

Suggested action:
1. Add one host helper, `note(what, Result)`, that logs each failure and counts it, and have checkup report the count.
2. Replace `let _ =` on `store.append`, `memory.add`, `meter` and `attach_remote_task` in agent_host.rs, agent_coder.rs, agent_lifecycle.rs and issue_run.rs.
3. When `meter` fails, keep the unmetered USD and retry on the next sweep.
4. Leave channel `send` discards as they are.
5. Verify with a test that makes the jobs store unwritable, then confirms the failure is counted and the spend is metered after the store recovers.

### CD-07 30+ `git` spawns resolve `git` from PATH while owner.rs states git is never searched on PATH

Severity: medium. Category: security. Effort: M.

Locations:
- [task/owner.rs](../../../../crates/coder/src/task/owner.rs): owner.rs:63-76
- [task/local.rs](../../../../crates/coder/src/task/local.rs): local.rs:125-134
- [task/agent_host.rs](../../../../crates/coder/src/task/agent_host.rs): agent_host.rs:3662-3676
- [repo.rs](../../../../crates/coder/src/repo.rs): repo.rs:524-534
- [task/shadow.rs](../../../../crates/coder/src/task/shadow.rs): shadow.rs:195
- [runtime.rs](../../../../crates/coder/src/runtime.rs): runtime.rs:3444
- [task/issue_run.rs](../../../../crates/coder/src/task/issue_run.rs): issue_run.rs:883

Evidence: The `GIT_PATHS` doc at owner.rs:63 says "Like the boundary's `bwrap`, it is never searched for on `PATH`." Yet src has 38 `Command::new("git")` sites in 10 files. `local::git` falls back with `.unwrap_or("git")`. agent_host.rs `git_line` runs bare git with the inherited environment and no timeout, and it is used for remote dispatch reads (#10930).

Impact: Git helpers apply different trust rules and timeouts. A hung git (a credential helper or the network) can stall host threads. Behavior also differs under launchd's PATH.

Suggested action:
1. Create one `crate::git` builder: resolve through GIT_PATHS, set stdin to null, set `GIT_TERMINAL_PROMPT=0`, run with a time limit and output cap, and apply `remote_override_environment`.
2. Move every `Command::new("git")` and every per-file `fn git` helper onto it.
3. Add a CI check: `rg 'Command::new\("git"\)' crates/coder/src` (excluding tests) must return nothing.

### CD-08 Dead public modules: doctor.rs, waves.rs and store.rs (1,808 lines) have no callers

Severity: medium. Category: dead-code. Effort: S.

Locations:
- [lib.rs](../../../../crates/coder/src/lib.rs): lib.rs:66, lib.rs:100, lib.rs:107
- [doctor.rs](../../../../crates/coder/src/doctor.rs): doctor.rs:1-12
- [waves.rs](../../../../crates/coder/src/waves.rs): waves.rs:1-12
- [store.rs](../../../../crates/coder/src/store.rs): store.rs:1-12

Evidence: Searching crates/ and apps/ with rg for `coder::(store|doctor|waves)`, `crate::`/`super::` paths and grouped imports finds no references outside the modules themselves. The `waves` hits in source.rs are its own `Selection::waves` fn. Sizes from wc: doctor 535, waves 796, store 477, total 1,808.

Impact: About 1.8k lines are compiled and maintained for nothing. A second "doctor" makes it unclear which diagnostics are the real ones.

Suggested action:
1. Delete the three files and their `pub mod` lines in lib.rs.
2. Verify with `cargo check -p coder --all-targets` and check the dependents (`-p openagents-cli -p coder-labor`).

### CD-09 bin/coder-worker.rs is a 7,065-line binary with ~3.9k lines of service logic and a ~740-line function

Severity: medium. Category: maintainability. Effort: L.

Locations:
- [bin/coder-worker.rs](../../../../crates/coder/src/bin/coder-worker.rs): coder-worker.rs:586, coder-worker.rs:2052, coder-worker.rs:2800, coder-worker.rs:3934

Evidence: The file is 7,065 lines and `#[cfg(test)]` starts at 3934. `async fn routed` starts at 2800 and runs to about 3543. There are two `serve` fns, at 586 and 2052.

Impact: Other crates cannot reuse or unit-test the relay worker's routing, BYOK or failover logic. A function this large hides regressions.

Suggested action:
1. Move everything except argv parsing and `main` into `src/relay/worker/{config,connection,job,route}.rs`.
2. Split `routed` into gate setup, triage wait, door selection and stream merge, all taking a `RouteCtx` struct.
3. Move the tests along with their code.
4. Verify that `cargo test -p coder` passes with the same test count and that no function in the worker exceeds 200 lines.

### CD-10 Oversized files and functions: runtime.rs has 5.4k production lines; ~48 functions exceed 200 lines

Severity: medium. Category: maintainability. Effort: L.

Locations:
- [runtime.rs](../../../../crates/coder/src/runtime.rs): runtime.rs:5410
- [router_claim.rs](../../../../crates/coder/src/router_claim.rs): router_claim.rs:247-773
- [task/sales.rs](../../../../crates/coder/src/task/sales.rs): sales.rs:1124-1598
- [root Cargo.toml](../../../../Cargo.toml): Cargo.toml:37-40

Evidence: runtime.rs has 9,576 lines and `#[cfg(test)]` starts at 5410. The root `[workspace.lints.clippy]` exists but only denies `dbg_macro`/`todo`/`unimplemented`. Nothing limits function size.

Impact: Review and merge-conflict costs pile up in the files that many agents edit every day.

Suggested action:
1. Split runtime.rs into `runtime/{admit,steps,ask,budget,capture}.rs` and move its tests to `runtime/tests/`.
2. Split sales `apply_with_evidence_root` into one fn per Operation.
3. Enable `clippy::too_many_lines` (threshold 200) for coder through `[lints.clippy]`, with the current offenders allow-listed.
4. Verify that `cargo clippy -p coder` is clean and the allow-list only shrinks.

### CD-11 About 2,500 `pub fn` and 61 `pub mod`: no visibility discipline, so dead code is invisible to rustc

Severity: medium. Category: architecture. Effort: M.

Locations:
- [lib.rs](../../../../crates/coder/src/lib.rs)
- [task.rs](../../../../crates/coder/src/task.rs)

Evidence: lib.rs declares `pub mod` for nearly every module. The three dead modules in CD-08 never triggered a `dead_code` warning because they are `pub`.

Impact: Every internal helper is exposed to 24 dependents, and dead code builds up.

Suggested action:
1. Add `#![warn(unreachable_pub)]` to lib.rs.
2. Make modules that no other crate imports `pub(crate)`: classify, first, permit, profiles, select, evidence, codebase, repo, reattach, reconcile, turn, tracker.
3. Fix the resulting `dead_code` warnings by deleting code, not by adding allows.
4. Verify with `cargo check --workspace --all-targets`.

### CD-12 Stringly-typed errors: ~764 `Result<_, String>` signatures and module-wide String Result aliases

Severity: medium. Category: error-handling. Effort: L.

Locations:
- [task/sales.rs](../../../../crates/coder/src/task/sales.rs): sales.rs:45
- [customer.rs](../../../../crates/coder/src/customer.rs): customer.rs:22
- [task/autostart.rs](../../../../crates/coder/src/task/autostart.rs): autostart.rs:2101-2109

Evidence: Sales conflict and authorization outcomes are bare strings (for example "sales revision conflict"). autostart's `write_private` throws away the `io::Error` with `map_err(|_| ...)`.

Impact: The only way for callers to tell a retryable conflict from a permission denial is to match on the string. The underlying OS error is lost.

Suggested action:
1. Introduce `sales::Error {Conflict, Forbidden, Invalid, Io}` and `customer::Error`, keeping the existing Display strings the same.
2. Preserve `io::Error` context in the autostart and agent write helpers.
3. Verify with tests that match on the variants instead of on strings.

### CD-13 At least six separate atomic/private file-write implementations with different durability and permission behavior

Severity: medium. Category: duplication. Effort: S.

Locations:
- [task.rs](../../../../crates/coder/src/task.rs): task.rs:1244-1252
- [task/autostart.rs](../../../../crates/coder/src/task/autostart.rs): autostart.rs:2094-2111
- [task/agent.rs](../../../../crates/coder/src/task/agent.rs): agent.rs:1102-1117
- [relay/quota.rs](../../../../crates/coder/src/relay/quota.rs): quota.rs:341-346
- [task/agent_jobs.rs](../../../../crates/coder/src/task/agent_jobs.rs): agent_jobs.rs:241-252

Evidence: The agent_jobs `save` does `fs::write`, an ignored chmod, then rename. quota's `write_atomically` does `fs::write` + rename with no fsync and no mode. task.rs `replace_file` does a durable temp write + rename + directory sync.

Impact: Whether a file survives a crash and stays private depends on which helper the module happened to copy.

Suggested action:
1. Promote `replace_file` to `crate::private::replace(path, bytes)`: private-mode create, fsync, rename, directory sync.
2. Route every helper listed above through it.
3. Add a test that an existing 0644 target ends up 0600.

### CD-14 In-process `static Mutex` claims machine-wide merge exclusion

Severity: medium. Category: concurrency. Effort: M.

Locations:
- [task/studio_git.rs](../../../../crates/coder/src/task/studio_git.rs): studio_git.rs:92-94
- [task/issue_run.rs](../../../../crates/coder/src/task/issue_run.rs): issue_run.rs:1097-1100

Evidence: The comment at studio_git.rs:92 says "One merge at a time on this computer ... two merges never race on one checkout", but the lock is `static MERGING: Mutex<()>`, which only covers one process. issue_run.rs pairs its LANDING mutex with a cross-process `landing_lock`.

Impact: Despite the comment, a CLI process and the resident host can merge on the same checkout at the same time.

Suggested action:
1. Add a cross-process file lock (`<checkout>/.git/coder-merge.lock` via `task::open_lock`/`take_lock`) next to MERGING, as issue_run does.
2. At minimum, change the comment to say "in this process".
3. Verify with a two-process test that the second merge waits.

### CD-15 Production behavior is compiled from docs/ and other trees via 24 `include_str!("../../..")` paths

Severity: medium. Category: build. Effort: M.

Locations:
- [efficiency.rs](../../../../crates/coder/src/efficiency.rs): efficiency.rs:56-62
- [gym_kb.rs](../../../../crates/coder/src/gym_kb.rs): gym_kb.rs:180-186
- [task/sales/qualification.rs](../../../../crates/coder/src/task/sales/qualification.rs): qualification.rs:56-60

Evidence: rg finds 24 `include_str!` paths starting with `../../..` in crates/coder/src.

Impact: Editing a measurement doc or a deploy catalog silently changes the binary. The crate cannot be packaged on its own.

Suggested action:
1. Move runtime inputs into the crates that own them, or into `crates/coder/fixtures`, and expose them through explicit APIs.
2. Add a test that lists the allowed out-of-crate `include_str!` targets and fails when a new one appears.

### CD-16 Library modules print directly to stdout/stderr and embed CLI frontends

Severity: medium. Category: architecture. Effort: M.

Locations:
- [task/agent_interview.rs](../../../../crates/coder/src/task/agent_interview.rs): agent_interview.rs:1248-1760
- [task/autostart.rs](../../../../crates/coder/src/task/autostart.rs): autostart.rs:1392-1602
- [delegate_door.rs](../../../../crates/coder/src/delegate_door.rs): delegate_door.rs:1034

Evidence: rg finds about 197 `println!`/`eprintln!` in src, excluding bin/, main.rs and `*tests*` files.

Impact: Apps that embed the library cannot capture or route its diagnostics. Sweep errors go only to launchd's stderr.

Suggested action:
1. Move the autostart and agent_interview CLI fns into openagents-cli or behind a `cli` feature.
2. Replace sweep `eprintln!` with an injected diagnostic sink, or with host journal entries that doctor can show.
3. Verify that the rg count for library modules drops to near zero.

### CD-17 Integration tests for app crates live in coder/tests and pull app crates into coder's dev-deps

Severity: low. Category: testing. Effort: S.

Locations:
- [Cargo.toml](../../../../crates/coder/Cargo.toml): Cargo.toml:89-97
- [tests/desktop_local_coder.rs](../../../../crates/coder/tests/desktop_local_coder.rs): desktop_local_coder.rs:12-166
- [tests/route_map_sources.rs](../../../../crates/coder/tests/route_map_sources.rs): route_map_sources.rs:26-29

Evidence: `[dev-dependencies]` lists openagents-chat-app (test-support) and openagents-desktop (default-features=false). This is not a dependency cycle: chat-app does not depend on coder, and desktop's coder dependency is optional and not enabled here.

Impact: `cargo test -p coder` compiles the desktop and chat apps, and app behavior is tested from the wrong crate.

Suggested action:
1. Move desktop_local_coder.rs to `crates/openagents-desktop/tests`, and the route_map/eval_cards tests to `crates/openagents-chat-app/tests`.
2. Drop both dev-deps.
3. Measure `cargo test -p coder --no-run` time before and after.

### CD-18 Two unrelated 'delegate' subsystems with clashing type names (coder::delegate vs coder_delegate)

Severity: low. Category: maintainability. Effort: S.

Locations:
- [coder/src/delegate.rs](../../../../crates/coder/src/delegate.rs): delegate.rs:1-25
- [coder-delegate/src/delegate.rs](../../../../crates/coder-delegate/src/delegate.rs): delegate.rs:1-15

Evidence: Both define `Executor` and `Delegation` types, with different meanings.

Impact: Readers confuse the two, and imports need aliasing.

Suggested action:
1. Rename `coder::delegate` to `fanout` (`FanOut`, `CliExecutor`).
2. Add a note in lib.rs pointing to coder_delegate for the other concept.
3. Verify with `cargo check --workspace`.

### CD-19 1 MB pretty-printed generated tree.json is the most-churned file (145 commits in 30 days)

Severity: low. Category: repo-hygiene. Effort: S.

Locations:
- [cli_route/tree.json](../../../../crates/coder/src/cli_route/tree.json)
- [cli_route/tree.rs](../../../../crates/coder/src/cli_route/tree.rs): tree.rs:10-13

Evidence: The file is 1,003,349 bytes, and `git log --since=30.days` shows 145 commits touching it.

Impact: Concurrent agents keep hitting merge conflicts in it.

Suggested action:
1. Emit it compactly, one leaf per line.
2. Mark it `linguist-generated` in `.gitattributes`, with a note to regenerate on conflict.
3. Keep the drift test, and verify that it still passes after regeneration.

### CD-20 Unreferenced router fixtures and stale module/crate docs

Severity: low. Category: docs. Effort: S.

Locations:
- [fixtures/chat-router/routes-v1.json](../../../../crates/coder/fixtures/chat-router/routes-v1.json)
- [fixtures/chat-router/routes-v2.json](../../../../crates/coder/fixtures/chat-router/routes-v2.json)
- [task/autostart.rs](../../../../crates/coder/src/task/autostart.rs): autostart.rs:14-17, autostart.rs:119-124
- [Cargo.toml](../../../../crates/coder/Cargo.toml): Cargo.toml:6

Evidence: No Rust code loads routes-v1 or routes-v2. v2 is only named in doc comments (router/rubric.rs:17, router_eval.rs:7). autostart.rs:15-16 mentions "step and wall-clock limits", while autostart.rs:119-124 says "Coder runs have no step or time limit".

Impact: The fixtures are dead, and the module doc contradicts the no-limits decision.

Suggested action:
1. Delete routes-v1.json and routes-v2.json, and update the rubric.rs/router_eval.rs doc comments to cite the current fixture.
2. Fix autostart.rs:14-17 so it matches the no-limits decision.
3. Update the Cargo.toml description to match the crate's actual contents (or its contents after CD-03).

### CD-21 Small helper duplication: unix_now, hex, per-byte format!("{:02x}")

Severity: low. Category: duplication. Effort: S.

Locations:
- [task/agent_host.rs](../../../../crates/coder/src/task/agent_host.rs): agent_host.rs:3765
- [task/local.rs](../../../../crates/coder/src/task/local.rs): local.rs:1786

Evidence: agent_host.rs:3765 has `digest.iter().map(|b| format!("{b:02x}")).collect()`. local.rs uses `autostart::unix_now` as its clock.

Impact: Small drift between copies, and local.rs depends on autostart only for a clock.

Suggested action:
1. Add `crate::util::{unix_now, hex}`.
2. Replace the copies, then check with `rg '02x\}' crates/coder/src` that none remain.

### CD-22 Emergency quota brake rewrites its counts file synchronously on every admitted job, with no fsync

Severity: low. Category: performance. Effort: S.

Locations:
- [relay/quota.rs](../../../../crates/coder/src/relay/quota.rs): quota.rs:296-346

Evidence: `admit` calls `self.save()` after each admission, and `save` does `fs::write` + rename with no fsync. This only happens when a per-day limit is configured.

Impact: During an abuse emergency, every admission does a blocking write on the hot path, and the counts can still be lost in a crash.

Suggested action:
1. Debounce saves: set a dirty flag and flush at most once per second and on shutdown, using the shared durable write helper (CD-13).
2. Add a test that 1,000 admissions produce far fewer writes.

### CD-23 Per-worktree remote-override cache never expires in the resident host

Severity: low. Category: performance. Effort: S.

Locations:
- [task/local.rs](../../../../crates/coder/src/task/local.rs): local.rs:163-177

Evidence: `remote_overrides` caches results per directory in `static READ: OnceLock<Mutex<BTreeMap<PathBuf,...>>>` with no expiry. The doc says "Read once per directory."

Impact: The cache grows by one entry per task worktree path, and it serves a stale value if a remote URL changes while the host runs.

Suggested action:
1. Key the cache by the git common dir instead of the worktree path.
2. Add a TTL (`READY_EVERY`) or a small LRU cap.
3. Add a test that a changed remote is seen after the TTL.

## Refuted during verification

- **claim.rs `UNREADABLE` grows without bound** (part of a proposed process-lifetime caches finding). Rejected: `UNREADABLE` stores each repository name at most once (dedup check at claim.rs:389), so its size is bounded by the number of distinct repositories a process sees, not by request volume.
- **Partial: `UNREADABLE` and `here()` in CD-23.** These were dropped from CD-23. `UNREADABLE` is keyed by repository and bounded. `here()` holds its lock only to compute on a cache miss, and the cache lasts 15 s, so it is minor.
- **Partial: claim.rs:394 as a stringly-typed control-flow example (CD-12).** Dropped: that substring check only chooses a hint sentence inside a message and does not affect control flow.
- **Partial: dependency cycle via dev-deps (CD-17).** There is no real cycle: openagents-chat-app does not depend on coder, and openagents-desktop's coder dependency is optional and off in this configuration.
- **Partial: "`[workspace.lints.clippy]` is empty" (CD-10).** Corrected: it denies `dbg_macro`, `todo` and `unimplemented`, but has nothing on size.
