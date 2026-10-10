# Agent runtime, background, plugins

**Scope (AGT):** `crates/{microcoder, microcoder-loop, background, boat, boat-template, supervise, agent-fleet, voyager, atif, pylon, plugin, plugin-pdk, plugin-action-items, plugin-code-search, plugin-dependency-check, plugin-explain-error, plugin-outline, plugin-release-notes, plugin-repo-map, plugin-test-report}`
**Audit date:** 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`
**Health grade:** C

This area holds about 106k lines of Rust in 20 crates. Most of it is in microcoder (21.4k), background (15.6k), boat (14.3k, mostly generated), microcoder-loop (13.0k), pylon (10.7k) and voyager (9.8k). The code is careful work. Module docs are thorough, and every doc and script path these crates cite exists on disk. supervise, plugin, atif and boat are solid foundations. Once test modules are excluded, production code has few unwraps. The grade comes from structural problems and a few safety gaps in live paths. Pylon's mainnet daily spending ceiling fails open. Credential scrubbing for model-run commands is a weak suffix denylist that exists in several copies, and one copy can panic. Three ACP engine adapters are copy-pasted. Several functions are 400-740 lines long. background has unlocked state updates and runs git with no deadline. Small utilities are re-implemented across the workspace. Payment code in pylon has little test coverage.

## Measurements

| Metric | Value |
|---|---|
| Tracked `.rs` LOC by crate | microcoder 21,429 (30 files); background 15,586 (31); boat 14,311 (239, 211 generated in `src/models`); microcoder-loop 12,966 (17); pylon 10,654 (28); voyager 9,764 (20); supervise 4,309; atif 3,852; plugin 3,363; plugin-explain-error 1,850; plugin-dependency-check 1,836 (one file); agent-fleet 1,224; boat-template 877; plugin-release-notes 803; plugin-pdk 765; plugin-test-report 718; plugin-action-items 547; plugin-code-search 465; plugin-repo-map 354; plugin-outline 205. Total about 105.9k |
| Tests (`#[test]`/`#[tokio::test]`) | boat 222, microcoder 138, microcoder-loop 133, background 105, supervise 70, atif 60, voyager 43, plugin 31, pylon 21, agent-fleet 11, boat-template 5. The guest crates have only a few coarse host-side tests: plugin-dependency-check 4, plugin-release-notes 3, plugin-action-items 3 (see AGT-13) |
| unwrap/expect/panic in production code | voyager `ensemble.rs` 37 (35 of them poison `lock().expect`), background `judged_suite.rs` 18. Every other occurrence is in a test module |
| `let _ =` in production code | voyager 66, microcoder 51 (23 are ignored `host.append`/`host.result` transcript writes), pylon 28, background 26 |
| Longest functions | `microcoder-loop/src/run.rs:1153` `run` 738 lines; `microcoder/src/main.rs:356` `go` 450; `microcoder/src/show.rs:83` `event` 413; `voyager/src/ensemble.rs:1414` `sweep` 396; `background/src/plan.rs:424` `candidates` 327; `plugin-explain-error/src/lib.rs:1241` `diagnose` 310 |
| `#[allow]` | 30 in scope; pylon 12, of which 10 are `clippy::unwrap_used`/`expect_used` for a lint that is not enabled |
| TODO/FIXME | 4 |
| `unsafe` blocks / SAFETY comments | supervise 35/35, background 9/8, voyager 3/0 |
| Public items | background 365, microcoder-loop 350, pylon 257 |
| Dependent manifests | atif 29, supervise 18, boat 9, plugin-pdk 8, pylon 7, plugin 6, background 5, microcoder-loop 4, microcoder 1, agent-fleet 1, voyager 0 (binary only) |
| Commits, last 30 days | microcoder 149, microcoder-loop 35, background 27 |
| Workspace-wide duplication (reviewer counts) | `is_char_boundary` truncation helpers in 90 files; `fn now()`/`now_ms()` in 152; days-from-civil (`719468`) in 46; `killpg`/`process_group(0)` outside supervise in 12 crates |
| Dependency drift (`Cargo.lock`) | sha2 0.10.9 + 0.11.0; tungstenite 0.28/0.29/0.30; reqwest 0.12/0.13; thiserror 1 + 2. `[workspace.dependencies]` pins only the iroh crates |

## Strengths

- supervise implements a real contract for supervising one subprocess: it owns the process group, bounds captured output, caps memory and reaps on drop. It has 70 tests, and all 35 of its unsafe blocks carry SAFETY comments ([lib.rs:1-60](../../../../crates/supervise/src/lib.rs)).
- The plugin host sandboxes guests carefully. It uses wasmtime fuel and epoch-interrupt cancellation, limits module size, and documents a fix for a macOS mach-port trap ([engine.rs:160-206](../../../../crates/plugin/src/engine.rs)). Guest build receipts pin the PDK source, the guest source and the wasm bytes, so editing a source without rebuilding fails [tests/guests.rs](../../../../crates/plugin/tests/guests.rs).
- boat's generated SDK has guards. A test asserts the digest of the pinned OpenAPI spec ([tests/boat.rs:401-407](../../../../crates/boat/tests/boat.rs)), the retry class of all 69 operations is tested, and recorded fixtures cover every operation.
- background deletes files defensively. It uses `symlink_metadata` everywhere and never follows links, re-checks each path under held locks right before deleting, moves files to a trash with a 24h window, and offers git-based undo ([run.rs:205-420](../../../../crates/background/src/run.rs)).
- Pylon does money arithmetic in u64/i64 msat with checked conversions and saturating adds, and it journals each mainnet payment before attempting it ([broker.rs:233-364](../../../../crates/pylon/src/broker.rs), [paid.rs:255-285](../../../../crates/pylon/src/paid.rs)).
- agent-fleet marks a panicked run as Failed through `Drop for Control`, which preserves the exactly-one-notice contract, and it recovers from mutex poisoning ([lib.rs:630, 782](../../../../crates/agent-fleet/src/lib.rs)).
- atif is a versioned schema crate. It writes ATIF-v1.8, reads every 1.x version and fsyncs each appended record. 29 crates use it as the shared trace format.
- The docs are unusually good. Every crate has a module-level design doc, and every `docs/` and `scripts/` path these crates cite exists on disk.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| AGT-01 | High | security | Pylon's mainnet daily spend ceiling fails open and is not safe across processes | S |
| AGT-02 | High | security | Env credential scrubbing is a weak, duplicated suffix denylist, and one copy can panic | M |
| AGT-03 | Medium | duplication | The OpenCode, Devin and Grok ACP `turn()` functions are copy-pasted | M |
| AGT-04 | Medium | maintainability | Very large functions in the agent loop and CLI; `run()` is 738 lines | L |
| AGT-05 | Medium | concurrency | background rule state uses an unlocked read-modify-write, resets silently when corrupt, and buffers audit records in one global | S |
| AGT-06 | Medium | reliability | background runs git, including a network fetch, with no deadline while holding the run lock | S |
| AGT-07 | Medium | architecture | The microcoder crate mixes the live adapter with research tooling | L |
| AGT-08 | Medium | policy | Keyword matching selects the code excerpts for the knowledge-conformance judgment | M |
| AGT-09 | Medium | testing | Pylon has the lowest test density in the area, on payment and market code | M |
| AGT-10 | Medium | build | `[workspace.dependencies]` is not used for common crates, so versions drift | M |
| AGT-11 | Low | architecture | voyager kills process groups by hand outside supervise, with no SAFETY comments | M |
| AGT-12 | Low | error-handling | Adapters handle transcript (ATIF) append failures inconsistently | S |
| AGT-13 | Low | testing | plugin-dependency-check has hand-written parsers but only coarse fixture tests | M |
| AGT-14 | Low | duplication | Small utilities are re-implemented across the workspace | M |
| AGT-15 | Low | performance | The plugin host builds a new wasmtime Engine and recompiles the module on every call | M |
| AGT-16 | Low | concurrency | voyager's ensemble is a 2,295-line module of shared Mutexes and `lock().expect` | L |
| AGT-17 | Low | docs | The Pylon README covers 8 of 19 modules, and pylon carries unused lint allows | S |
| AGT-18 | Low | error-handling | The Microcoder CLI parses integer limits as f64 and casts with `as` | S |
| AGT-19 | Low | maintainability | Confusing names and layering | S |

### AGT-01 Pylon mainnet daily spend ceiling fails open and is not safe across processes

**Severity:** High · **Category:** security · **Effort:** S

**Locations:** [paid.rs:239-247](../../../../crates/pylon/src/paid.rs), [paid.rs:255-285](../../../../crates/pylon/src/paid.rs), [cli.rs:455-470](../../../../crates/pylon/src/cli.rs)

**Evidence:** `spent_since` calls `std::fs::read_to_string(&self.journal).unwrap_or_default()` and then `filter_map(|line| serde_json::from_str::<Value>(line).ok())`. An unreadable journal therefore counts as 0 msat spent, and corrupt lines are skipped. The only serialization is an in-process `lock: Mutex<()>`. `cli.rs:466-470` builds `Granted::new(payer, grant, &home())` separately in each `pylon ask` process, so two concurrent CLI asks can both pass the check before either one appends. The journal is never pruned. One detail limits the damage: the journal line is appended before `inner.pay`, so a failed payment still counts against the ceiling.

**Impact:** On `Network::Bitcoin`, a read error, a corrupt line or two concurrent CLI invocations can let spending exceed the owner's `daily_msat` ceiling with real sats.

**Suggested action:**
1. Change `spent_since` to return `Result<u64, String>`. Refuse the payment on any read error except `NotFound`.
2. When a line does not parse, either refuse or count it pessimistically as `per_payment_msat`.
3. Hold an exclusive `flock` on `mainnet-payments.jsonl` (or a sibling `.lock` file) from the read, through the check, to the append.
4. Prune journal lines older than 24h.
5. Verify with these tests: a journal with mode 000 makes the payment refuse; a garbage line counts pessimistically; two `Granted` values on the same home cannot together exceed `daily_msat`.

### AGT-02 Env credential scrubbing is a weak, duplicated suffix denylist, and one copy can panic

**Severity:** High · **Category:** security · **Effort:** M

**Locations:** [microcoder-loop/src/env.rs:84-90](../../../../crates/microcoder-loop/src/env.rs), [microcoder-loop/src/env.rs:156-160](../../../../crates/microcoder-loop/src/env.rs), [acp-client/src/process.rs:378-387](../../../../crates/acp-client/src/process.rs), [coder-delegate/src/seal.rs:180-197](../../../../crates/coder-delegate/src/seal.rs)

**Evidence:** Both `Local::run` and the bounded run in microcoder-loop `env.rs` do `for (name, _) in std::env::vars() { if name.ends_with("_API_KEY") || name.ends_with("_TOKEN") || name.ends_with("_SECRET") { child.env_remove(name) } }`. That check is case-sensitive, while `acp_client::process::is_credential_name` upper-cases the name first. `is_withheld` in coder-delegate `seal.rs` adds a `NAMED` list on top. `std::env::vars()` panics when any variable name or value is not valid Unicode. Names such as `AWS_SECRET_ACCESS_KEY`, `*_PASSWORD` and `GOOGLE_APPLICATION_CREDENTIALS` pass every copy. The reviewer counted about 8 copies across the workspace; the verifier checked only these three, so the count is approximate.

**Impact:** Credentials can reach commands the model chooses to run. A single non-UTF-8 environment variable crashes a Microcoder step.

**Suggested action:**
1. Write one shared policy. Extend `acp_client::process::is_credential_name` with `seal.rs`'s `NAMED` list plus `_PASSWORD`, `_PASS`, `_PAT`, `_CREDENTIALS`, `*_ACCESS_KEY` and `AWS_SECRET_ACCESS_KEY`.
2. Replace every inline copy with a call to that function.
3. In `microcoder-loop/src/env.rs`, iterate `std::env::vars_os()`.
4. For model-run commands, prefer `env_clear()` plus an allowlist (`PATH`, `HOME`, `LANG`, `TERM`, `TMPDIR`).
5. Verify with a table test of names in acp-client and a test that runs `Local::run` with a non-UTF-8 env var.

### AGT-03 OpenCode/Devin/Grok ACP turn() functions are copy-pasted skeletons

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:** [opencode.rs:102-295](../../../../crates/microcoder/src/repository/opencode.rs), [devin.rs:609-800](../../../../crates/microcoder/src/repository/devin.rs), [grok.rs:231-478](../../../../crates/microcoder/src/repository/grok.rs), [devin.rs:294-303](../../../../crates/microcoder/src/repository/devin.rs), [lean_session.rs:68-79](../../../../crates/microcoder/src/repository/lean_session.rs)

**Evidence:** `return Turn::Ended(ended)` appears 10 times in grok, 7 in opencode and 6 in devin. `opencode.rs:178-191` shows the repeated arm `ended.error = Some(..); session.close(STOP_GRACE).await; return Turn::Ended(ended);`. `fn bounded` at `devin.rs:294` is byte-identical to `pub(crate) fn bounded` at `lean_session.rs:68`. All three files open the session the same way, use the same effect and result prologue, and record the same "The X session this turn runs in." step.

**Impact:** A fix to resume, model admission or error recording has to be made three times, in files that change often.

**Suggested action:**
1. Add `repository/acp_turn.rs` with an `AcpEngine` trait (name, effect kind, arguments, environment, admits, database) and shared `open_session` and `end(session, ended, why) -> Turn` helpers.
2. Port each adapter's `turn()` to the shared skeleton, one adapter at a time.
3. Delete `devin.rs::bounded` and use `lean_session::bounded`.
4. Verify that each adapter's recorded-turn tests still pass unchanged.

### AGT-04 God functions in the agent loop and CLI: run() is 738 lines

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:** [run.rs:1153-1890](../../../../crates/microcoder-loop/src/run.rs), [run.rs:1914-1915](../../../../crates/microcoder-loop/src/run.rs), [microcoder/src/main.rs:356](../../../../crates/microcoder/src/main.rs), [voyager/src/ensemble.rs:1414](../../../../crates/voyager/src/ensemble.rs)

**Evidence:** `run()` starts at `run.rs:1153` and its closing brace is at line 1890, about 738 lines. `gate()` carries `#[allow(clippy::too_many_arguments)]` at line 1914. `async fn go` at microcoder `main.rs:356` is 450 lines. `ensemble.rs` is 2,295 lines and `sweep` is 396 of them. The reviewer also measured `show::event` (413) and `plan::candidates` (327); the verifier did not re-measure those two.

**Impact:** The shared loop engine is hard to review, and its phases cannot be unit-tested on their own.

**Suggested action:**
1. Turn `run()` into a per-run struct with separate methods for route, acceptance, step, commands and finish.
2. Move `gate()`'s parameters into that struct and remove the `too_many_arguments` allow.
3. Move the tbench branch of `go()` into `tbench::episode`.
4. Verify that microcoder-loop's existing tests pass without modification.

### AGT-05 background rule state uses unlocked read-modify-write; corruption silently resets; audit pending buffer is global

**Severity:** Medium · **Category:** concurrency · **Effort:** S

**Locations:** [store.rs:259-285](../../../../crates/background/src/store.rs), [store.rs:287-317](../../../../crates/background/src/store.rs), [runner.rs:141-160](../../../../crates/background/src/runner.rs)

**Evidence:** `State::load` does `.ok().and_then(serde_json::from_slice(..).ok()).unwrap_or_default()`. `update` and `forget` load, mutate and then `let _ = write_atomic(...)` with no lock. At `runner.rs:146-151`, a process that is not the runner "still runs" a manual request, so two processes can write state at the same time. `static PENDING: Mutex<Vec<String>>` is not keyed by layout. When `write_all` fails, the whole joined buffer stays and is appended again later, so a partial write duplicates lines. The buffer has no size limit. Mixing across layouts only matters in processes that hold several Layouts, which is mostly tests. The lost updates and silent resets are the serious parts.

**Impact:** In a process that deletes files, rule schedules and runner ownership can be lost without notice, and audit records can be duplicated.

**Suggested action:**
1. Take a `flock` on `state.json.lock` inside `State::update` and `State::forget`.
2. When parsing fails, rename the file to `state.json.corrupt-<ts>` instead of falling back to defaults, and log it.
3. Key `PENDING` by `layout.runs()`, cap it at about 1,000 lines, and count dropped lines.
4. Verify with a test where two threads call `State::update` concurrently, and a test with a corrupt state file.

### AGT-06 background runs git (including network fetch) with no deadline while holding the run lock

**Severity:** Medium · **Category:** reliability · **Effort:** S

**Locations:** [git.rs:9-29](../../../../crates/background/src/git.rs), [engine.rs:419-432](../../../../crates/background/src/engine.rs), [engine.rs:482](../../../../crates/background/src/engine.rs), [run.rs:199](../../../../crates/background/src/run.rs)

**Evidence:** There are two private `git()` helpers. The one in `git.rs` sets `GIT_OPTIONAL_LOCKS=0` and a null stdin; the one at `engine.rs:419` sets neither. Both call `.output()` with no timeout. `engine.rs:482` runs `git fetch --quiet`. `run.rs:199` takes `let _lock = lock(env.layout)?` before `execute_after`. `background/Cargo.toml` does not depend on supervise.

**Impact:** A remote or credential helper that hangs blocks the single background runner, and cleanup stops.

**Suggested action:**
1. Add `supervise` as a dependency of background.
2. Replace both helpers with one `git::run(dir, args, deadline)` built on supervise's blocking runner. Use a 60s deadline for fetch and 10s otherwise, and always set `GIT_OPTIONAL_LOCKS=0` and a null stdin.
3. Verify with a test that puts a fake `git` that sleeps on `PATH` and checks that the call times out and the lock is released.

### AGT-07 microcoder crate mixes the live adapter with research tooling, and engine adapters are misnamed

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:** [tbench.rs](../../../../crates/microcoder/src/tbench.rs), [xpnet.rs](../../../../crates/microcoder/src/xpnet.rs), [kbstudy.rs](../../../../crates/microcoder/src/kbstudy.rs), [kbnet.rs](../../../../crates/microcoder/src/kbnet.rs), [main.rs:356](../../../../crates/microcoder/src/main.rs)

**Evidence:** Line counts are tbench 1,904; xpnet 1,188, `xpnet/eval.rs` 1,016 and `xpnet/tests.rs` 963; kbstudy 582; kbinput 212; kbnet 1,086. A search for `microcoder::(xpnet|tbench|kbstudy|kbinput|kbnet)` outside the crate finds only `openagents-cli/src/kb.rs:48`, which uses kbnet. The live repository adapter is what coder autostart and the chat app use.

**Impact:** Every consumer of the product adapter also builds the research code and its dependencies, and edits to this high-churn crate collide.

**Suggested action:**
1. Create a binary-only `microcoder-bench` crate and move tbench, kbstudy, kbinput and xpnet into it.
2. Keep kbnet somewhere openagents-cli can still reach it.
3. Leave `microcoder` as the repository-adapter library.
4. Verify with `cargo metadata` that `coder` no longer pulls in the moved dependencies, and that both crates' tests pass.

### AGT-08 Keyword matching chooses the code excerpts for the knowledge-conformance judgment (violates the semantic-retrieval policy)

**Severity:** Medium · **Category:** policy · **Effort:** M

**Locations:** [run.rs:938-985](../../../../crates/microcoder-loop/src/run.rs)

**Evidence:** `entry_words` splits the last part of the entry id and its tags into lowercase words of 3 or more characters. `excerpts` keeps ±25 lines around every line where `lower.contains(w)`, then cuts the result to `EXCERPT_CHARS = 10_000`. That text goes to Jev's CONTRADICTS (0.7) judgment. The workspace `CLAUDE.md` forbids ad hoc keyword matching for retrieval routing, and selecting evidence for a model judgment counts as retrieval.

**Impact:** Generic tags fill the excerpt budget, and relevant code that lacks the literal word is missed, so the contradiction checks are noisy.

**Suggested action:**
1. Select excerpts by embedding similarity over file chunks, using the knowledge crate's embedding path, within `EXCERPT_CHARS`.
2. Record the retrieval method in the judgment's `extra`.
3. Verify with a test showing that a tag such as `run` does not pull in every line that contains it.

### AGT-09 Pylon test density is the lowest in the area, on payment and market code

**Severity:** Medium · **Category:** testing · **Effort:** M

**Locations:** [broker.rs](../../../../crates/pylon/src/broker.rs), [market.rs](../../../../crates/pylon/src/market.rs), [provider.rs](../../../../crates/pylon/src/provider.rs), [tests/market.rs](../../../../crates/pylon/tests/market.rs)

**Evidence:** There are about 21 tests for 10.6k LOC across 22 source files. The untested code includes broker settlement (fee = price - compute, `Ceilings`) and provider admission. broker, market and provider have no in-file test modules.

**Impact:** A regression in a fee split or a ceiling can ship unnoticed in code that moves money.

**Suggested action:**
1. Add broker unit tests for: refusing overpayment, refusing compute > price, fee arithmetic at the boundaries, and refusing a second invoice under `Ceilings`.
2. Make a negative `sent_msat` in `spent_since` an error instead of 0, and test it.
3. Add tests for provider admission.
4. Verify that `cargo test -p pylon` covers each case.

### AGT-10 [workspace.dependencies] is unused for common crates, so versions drift

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:** [Cargo.toml:15-22](../../../../Cargo.toml), [plugin/Cargo.toml:12](../../../../crates/plugin/Cargo.toml), [boat/Cargo.toml:4](../../../../crates/boat/Cargo.toml), [boat-template/Cargo.toml:4](../../../../crates/boat-template/Cargo.toml), [Cargo.lock](../../../../Cargo.lock)

**Evidence:** `[workspace.dependencies]` lists only iroh, iroh-relay and iroh-mdns-address-lookup. plugin uses `sha2 = "0.11.0"`, while background, boat, atif, microcoder-loop and voyager use `"0.10"`. `Cargo.lock` contains sha2 0.10.9 and 0.11.0, tungstenite 0.28/0.29/0.30, reqwest 0.12.28 and 0.13.5, and thiserror 1.0.69 and 2.0.20. boat and boat-template hardcode `edition = "2024"`. Some duplicates, such as tungstenite and reqwest 0.13, may come from transitive dependencies outside our control.

**Impact:** The same crates compile more than once, and each dependency bump touches many manifests.

**Suggested action:**
1. Add serde, serde_json, tokio, reqwest, sha2, tempfile, thiserror and libc to `[workspace.dependencies]`.
2. Switch the AGT crates to `workspace = true`.
3. Put plugin on sha2 0.10, or move every crate to 0.11.
4. Use `edition.workspace = true` in boat and boat-template.
5. Verify with `cargo tree -d` that there are fewer duplicates.

### AGT-11 voyager hand-rolls process-group kill outside supervise, without SAFETY comments

**Severity:** Low · **Category:** architecture · **Effort:** M

**Locations:** [relay.rs:108](../../../../crates/voyager/src/relay.rs), [relay.rs:154-166](../../../../crates/voyager/src/relay.rs), [server.rs:107](../../../../crates/voyager/src/server.rs), [server.rs:178-182](../../../../crates/voyager/src/server.rs)

**Evidence:** `.process_group(0)` appears at `relay.rs:108` and `server.rs:107`. `unsafe { libc::killpg(self.group, ...) }` appears at `relay.rs:154`, `relay.rs:165` and `server.rs:181`, none with a `SAFETY:` comment, although `server.rs:178` has an explanatory comment. `voyager/Cargo.toml:19` already depends on supervise. The severity is low because voyager is an experimental, binary-only crate with no dependents.

**Impact:** voyager's cleanup behavior can drift from the shared supervisor's, and its unsafe blocks are undocumented.

**Suggested action:**
1. Expose an owned process-group handle from supervise, with spawn and `terminate(grace)`.
2. Use it in `relay.rs` and `server.rs`, and delete the local `killpg` code.
3. Until then, add SAFETY comments to the three unsafe blocks.
4. Add a CI grep that fails on `killpg` outside `crates/supervise`.

### AGT-12 Transcript (ATIF) append failures are ignored inconsistently across adapters

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:** [repository.rs:935](../../../../crates/microcoder/src/repository.rs), [claude_sdk.rs:791](../../../../crates/microcoder/src/repository/claude_sdk.rs), [lean_session.rs:311](../../../../crates/microcoder/src/repository/lean_session.rs), [opencode.rs:182-191](../../../../crates/microcoder/src/repository/opencode.rs)

**Evidence:** `microcoder/src/repository*` has 23 `let _ = host.append/result` sites: repository.rs 9, claude_sdk 4, lean_session 3, recipe 2, coder_v1 2, and 1 each in devin, grok and opencode. By contrast, `opencode.rs:182-191` treats a failed append as fatal: it sets `ended.error`, closes the session and returns.

**Impact:** When storage fails, one engine stops while another keeps going with gaps in its trace.

**Suggested action:**
1. Add one `record(host, step)` helper in `repository.rs` with an explicit policy: fatal for session and result steps, counted for streaming steps.
2. Add a `transcript_gaps` count to `Ended`.
3. Replace all 23 sites with the helper.
4. Verify with a test host whose append fails, and check that every adapter reports the same outcome.

### AGT-13 plugin-dependency-check hand-writes TOML, lockfile and SPDX parsers in one 1,836-line file with only coarse fixture tests

**Severity:** Low · **Category:** testing · **Effort:** M

**Locations:** [lib.rs:444-672](../../../../crates/plugin-dependency-check/src/lib.rs), [lib.rs:918-1506](../../../../crates/plugin-dependency-check/src/lib.rs), [lib.rs:1715-1830](../../../../crates/plugin-dependency-check/src/lib.rs)

**Evidence:** The `#[cfg(test)]` module at line 1715 has 4 tests: `a_mixed_project_is_flagged_against_its_policy`, `pnpm_yarn_and_go_files_are_read`, `requirements_and_licenses_read_their_usual_shapes` and `nothing_to_check_says_so`. Each runs `check()` through `MemoryHost` over a fixture tree. None calls the TOML or SPDX parsers directly with edge cases.

**Impact:** Parser edge cases are not pinned by tests: inline tables, multi-line arrays, quoted keys, and SPDX `WITH` and parentheses.

**Suggested action:**
1. Split `lib.rs` into `toml`, `cargo`, `npm`, `python`, `go` and `spdx` modules.
2. Add direct parser tests for quoted keys, inline tables, multi-line arrays, `MIT OR (Apache-2.0 AND BSD-3-Clause)` and `GPL-2.0 WITH Classpath-exception-2.0`.
3. Check whether the `toml` crate works on wasm32 and could replace the hand-written parser.
4. Rebuild the guest and refresh its build receipt so `plugin/tests/guests.rs` passes.

### AGT-14 Small utilities are re-implemented across the workspace: truncation, clocks, civil-date math

**Severity:** Low · **Category:** duplication · **Effort:** M

**Locations:** [devin.rs:294](../../../../crates/microcoder/src/repository/devin.rs), [lean_session.rs:68](../../../../crates/microcoder/src/repository/lean_session.rs)

**Evidence:** `devin.rs:294` and `lean_session.rs:68` contain identical `bounded` functions that truncate at a char boundary and append the same `…` suffix. Across the workspace, the reviewer reports 90 files with such helpers, 152 `now()` helpers and 46 files containing `719468`. The verifier did not re-measure the workspace-wide counts.

**Impact:** Fixes for edge cases, and choices such as whether to add an ellipsis, drift between the copies.

**Suggested action:**
1. Create a dependency-free `oa-std` crate with `text::bounded` and `time::{now_secs, now_ms, days_from_civil, rfc3339}`, and unit-test it.
2. Migrate the AGT-scope copies first.
3. Open a tracking issue for the rest of the workspace, and use a grep count to track progress.

### AGT-15 The plugin host rebuilds a wasmtime Engine and recompiles the module on every invocation

**Severity:** Low · **Category:** performance · **Effort:** M

**Locations:** [engine.rs:160-193](../../../../crates/plugin/src/engine.rs), [engine.rs:208-222](../../../../crates/plugin/src/engine.rs)

**Evidence:** `metered()` calls `wasmtime::Config::new()` and `Engine::new(&config)` on every call, and `run_guest` calls `Module::new(engine, wasm)`. The per-call engine is deliberate: cancellation calls `engine.increment_epoch()` from a per-call watcher (`interrupt_on_cancel`), and on a shared engine that would trap every concurrent guest. Caching alone is therefore not a correct fix. No cost has been measured.

**Impact:** Cranelift compiles the guest on every call.

**Suggested action:**
1. Measure `plugin/tests/guests.rs` timings first to decide whether this is worth doing.
2. If it is, use one process-wide `Engine` in a `OnceLock` and cache modules by digest.
3. Move cancellation to per-store epoch deadlines driven by a shared ticker, so cancelling one call does not trap the others.
4. Verify that the replay receipts are unchanged and that a concurrent-cancel test only stops the cancelled guest.

### AGT-16 voyager ensemble is a 2,295-line god module of shared Mutexes and lock().expect calls

**Severity:** Low · **Category:** concurrency · **Effort:** L

**Locations:** [ensemble.rs:313-348](../../../../crates/voyager/src/ensemble.rs), [ensemble.rs:395](../../../../crates/voyager/src/ensemble.rs), [ensemble.rs:1414](../../../../crates/voyager/src/ensemble.rs)

**Evidence:** The file is 2,295 lines long and has no test module. It has 35 `expect("... lock is not poisoned")` calls (37 expects in total), including one in the event hook at line 395: `server.lock().expect("the server lock is not poisoned")`. The severity is low because voyager is an experimental, binary-only crate with no dependents.

**Impact:** A panic in one leg poisons the shared locks, and the poisoning cascades into aborts.

**Suggested action:**
1. Replace the expects with `unwrap_or_else(PoisonError::into_inner)`, as agent-fleet and `background/store.rs:297` already do.
2. Merge the shared state into one `Mutex<World>` or an actor.
3. Split the file into `ensemble/{setup,legs,quest,sweep}.rs`.
4. Verify that voyager's 43 tests still pass, and add a test that a panic in one leg does not abort the sweep.

### AGT-17 Pylon README describes 8 of 19 modules; dead clippy::unwrap_used allows

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:** [README.md:1-22](../../../../crates/pylon/README.md), [Cargo.toml:7](../../../../crates/pylon/Cargo.toml), [inflight.rs:100](../../../../crates/pylon/src/inflight.rs), [check.rs:534](../../../../crates/pylon/src/check.rs), [lease.rs:96](../../../../crates/pylon/src/lease.rs)

**Evidence:** The README table lists provider, client, field, pool, engine, job, relay and identity, and both the README and the Cargo.toml description call the jobs "free NIP-CJ" jobs. `src` also contains paid, market, broker, league, lease, share, route, check, inflight, cli and fixture. Workspace clippy lints deny only `dbg_macro`, `todo` and `unimplemented`. pylon nevertheless has 12 `#[allow]`s, 10 of them `unwrap_used`/`expect_used`: 5 in src test modules and 5 file-level in `tests/` and the field tests.

**Impact:** Readers miss that the crate moves real money, and the allows suggest a lint is enforced when it is not.

**Suggested action:**
1. Add the paid, broker and market modules to the README with a Payments section, and correct the Cargo.toml description.
2. Either enable `clippy::unwrap_used` for pylon through `[lints]`, or delete the allows.
3. Verify that `cargo clippy -p pylon` is clean under whichever choice you make.

### AGT-18 Microcoder CLI parses integer limits as f64 and casts with `as`

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:** [main.rs:213-219](../../../../crates/microcoder/src/main.rs), [main.rs:258-261](../../../../crates/microcoder/src/main.rs)

**Evidence:** `--max-steps` is parsed as `number(value()?)? as usize`. `--command-seconds` and `--test-seconds` are cast `as u64`, and `--adversarial` and `--oracle-steps` `as usize`. A negative value saturates to 0 and a fraction is truncated, with no error either way.

**Impact:** Mistyped limits are accepted without any warning.

**Suggested action:**
1. Parse integer flags with `str::parse::<u64>` (or `usize`).
2. For flags that really are floats, reject negative values and NaN.
3. Add parse tests for `-3`, `2.9` and `abc`, each of which should fail with an error message.

### AGT-19 Naming and layering confusion: 'background' vs 'Background agents', Devin-namespaced meta helper, stale local path

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:** [background/Cargo.toml:7](../../../../crates/background/Cargo.toml), [agent-fleet/Cargo.toml:7](../../../../crates/agent-fleet/Cargo.toml), [opencode.rs:144](../../../../crates/microcoder/src/repository/opencode.rs), [grok.rs:321](../../../../crates/microcoder/src/repository/grok.rs), [acp-client/src/lib.rs:86](../../../../crates/acp-client/src/lib.rs), [atif/src/lib.rs:10](../../../../crates/atif/src/lib.rs)

**Evidence:** background's description is "Background processes: durable rules...", and agent-fleet's is "Background agents: ...". `opencode.rs:144` and `grok.rs:321` call `acp_client::devin::engine_meta(coder_history::engine::MARK)`. `acp-client/src/lib.rs:86` hardcodes `devin::engine_meta("openagents-coder-engine")`. `atif/src/lib.rs:10` cites the local path `~/work/coder`.

**Impact:** Contributors look in the wrong crate, and engines that are not Devin depend on the Devin module.

**Suggested action:**
1. Move `engine_meta` to the `acp_client` root and re-export it from `devin`.
2. Replace the string literal with a shared const.
3. Retitle one of the two crate descriptions so the two crates are clearly different.
4. Replace `~/work/coder` with a reference to a repo or RFC.
5. Verify with `rg 'devin::engine_meta'` that only Devin code still uses the devin path.
