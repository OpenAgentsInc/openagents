# Coder orchestration crates

Scope: `crates/{coder-new, coder-delegate, coder-host, coder-host-wire, coder-access, coder-scheduler, coder-lease, coder-labor, coder-service}` (about 162.7k lines of Rust), plus overlap with `coder`, `coder-new` and `coder-one`. Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

**Health grade: C**

Line-level hygiene is good. Every crate uses the workspace lints, there are only 21 `#[allow]` attributes, 52 of 55 `unsafe` blocks carry SAFETY comments, almost every `unwrap` is in test code, the crates have about 1,190 tests, and the module docs explain intent. The problems are structure and lineage. The shipped `coder` binary is built from a crate named `coder-new`, which started as a "Ratatui mockup" on 2026-10-06 and has had 114 commits in four days, while the crate named `coder` still builds a second `coder` binary that holds `coder host`. The deprecated `coder-one` crate (132k lines, no Cargo dependents) cannot be deleted because the live `coder-delegate` crate reads 37 prompt and policy files from it via `include_str!`, and gym spawns a `coder-one` binary at runtime. Several files and functions are very large (`delegate.rs` at 4,443 lines, a 674-line `execute`, a 570-line `App::handle`). `coder-access` has become a hub with 21 dependents that also carries product data types. There are a few live correctness and security issues: unchecked PGID kills in the service launcher, ignored reassembly errors on the host's direct channel, and Linux/Windows self-update that trusts an unsigned checksum file. Helpers have been copied and have drifted (atomic writes, canonical JSON, key lookup, git spawning, wire constants, `build.rs`), and errors are mostly `String`, including in the paid-labor money path.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (git ls-files, .rs) | coder-new 46,026 (82 files); coder-delegate 38,232 (53); coder-host 30,574 (77); coder-access 22,750 (40); coder-lease 7,876 (23); coder-service 6,280 (14); coder-labor 5,847 (12); coder-scheduler 4,516 (8); coder-host-wire 582 (1). Total about 162,683 |
| Comparison | coder 262,296 (341 files, 887 commits); coder-one 132,534 (194 .rs, 395 non-.rs files, about 8.4 MB, no Cargo reverse deps, last change 2026-10-03); microluna 5,080 (deprecated, used only by coder-one) |
| Largest files | coder-delegate/src/delegate.rs 4,443 (3,380 non-test); coder-new/src/bundled_runtime.rs 3,210 (1,747 non-test); coder-access/src/protocol.rs 2,606; coder-new/src/provider.rs 2,428; coder-delegate/src/issue.rs 2,223; coder-access/src/host/mod.rs 2,213; coder-new/src/lib.rs 2,060; coder-access/src/studio.rs 2,032 |
| Longest functions | coder-access host/mod.rs `execute` ~674 (1097-1770); coder-new lib.rs `App::handle` ~570 (1348-1916); coder-new provider.rs `chat_with_plugins` ~395; coder-labor records.rs `ingest` ~391; coder-access protocol.rs `validate` ~351; coder-host cli.rs `serve` ~327 |
| Tests (`#[test]`/`#[tokio::test]`) | coder-new 419, coder-delegate 257, coder-host 181, coder-access 124, coder-lease 73, coder-scheduler 65, coder-service 51, coder-labor 15, coder-host-wire 4 |
| Non-test `unwrap` | coder-new ~60 (9 are `lock().unwrap()`); coder-delegate, coder-host, coder-service near zero. Production `.expect` ~50, mostly "infallible serialize" |
| TODO/FIXME | 0 |
| `#[allow]` | 21 total |
| `unsafe` blocks | 55, 52 with `// SAFETY:` |
| `Result<_, String>` signatures | coder-new 133, coder-delegate 69, coder-lease 42; coder-labor uses a crate-wide `Result<T, String>` |
| Public surface | pub fn: coder-delegate 554, coder-access 345, coder-new 320, coder-host 244. pub mod: coder-delegate 41, coder-new 39, coder-host 26, coder-access 20 |
| Path deps | coder-new 29 (including all of `coder`), coder-host 23, coder-delegate 12 |
| Reverse deps | coder-access 21, coder-host 12, coder-lease 11, coder-delegate 7, coder-scheduler 2, coder-new 1 (openagents-cli), coder-labor 1 |
| Process-global OnceLock/Mutex statics | 18 (coder-host 11) |
| Churn since 2026-09-10 | coder-new lib.rs 57 commits, ui.rs 53; coder-host serve/mod.rs 44, cli.rs 43; coder-access protocol.rs 40. coder-new: 114 commits, all 2026-10-06 to 2026-10-09 |

## Strengths

- Lint discipline: every in-scope crate sets `[lints] workspace = true` (dbg_macro, todo, unimplemented denied); only 21 `#[allow]` in ~163k lines, mostly `too_many_arguments`.
- Unsafe code is small and documented: 52 of 55 blocks have SAFETY comments (coder-host/src/control/windows.rs 14 of 14, coder-service/src/launcher.rs 5 of 5).
- Production code rarely panics: coder-delegate, coder-host, coder-access, coder-lease and coder-service have almost no `unwrap()` outside tests.
- Strict, bounded wire parsing in coder-host-wire: `#[serde(deny_unknown_fields)]`, `parse_strict_bounded`, `MAX_MESSAGE_BYTES` = 256 KiB on every decode (lib.rs:48, 108-120, 344-346).
- coder-lease is crash-safe: a file table under an exclusive `flock`, holder locks released by the kernel when the holder dies, atomic writes with fsync (table.rs:243-254).
- coder-service updates by trial run against a snapshot with automatic rollback, records every step durably, and its adoption of old setups is careful and rerunnable (adopt.rs:1-40).
- coder-scheduler is pure and deterministic, has no internal path deps, a reproducible simulator and 65 tests. It is the model for the other crates.
- Usage-limit parsing lives in one place: coder-delegate/src/limit.rs:32 re-exports `coder_engine_status::limit`.
- Module-level `//!` docs are thorough in every crate and explain intent, failure modes and history.
- Ported third-party code carries attribution (coder-new/LICENSE-APACHE-xai, NOTICE).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| CX-01 | High | correctness | Launcher recovery SIGKILLs a saved process group with no identity check (PGID reuse) | M |
| CX-02 | High | dead-code | Live coder-delegate compiles 37 prompt/policy files from deprecated coder-one, pinning ~137k lines of dead code | M |
| CX-03 | Medium | correctness | Host client ignores direct-channel reassembly errors and keeps a corrupt buffer | S |
| CX-04 | Medium | security | Linux/Windows self-update trusts only a same-bucket SHA256SUMS; CODER_BASE_URL not restricted to https | M |
| CX-05 | Medium | architecture | Product binary `coder` built from crate `coder-new`; crate `coder` builds a second `coder` binary; "coder-new" leaks into user paths | L |
| CX-06 | Medium | maintainability | coder-delegate/src/delegate.rs is a 4,443-line god module; 554 pub fns across 41 pub mods | L |
| CX-07 | Medium | architecture | coder-access is a 21-dependent hub that mixes product data types with access control | L |
| CX-08 | Medium | maintainability | ~95-variant `Operation` enum and a 674-line host `execute` with too_many_arguments | L |
| CX-09 | Medium | maintainability | coder-new `App` god object with a ~570-line event handler | L |
| CX-10 | Medium | architecture | coder-host wires callbacks through ~11 global statics whose setters ignore failure | M |
| CX-11 | Medium | duplication | Atomic/private file write helpers copied 20+ times with different durability | M |
| CX-12 | Medium | correctness | Git subprocesses do not clear inherited GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE | M |
| CX-13 | Medium | error-handling | String errors in the paid-labor money path; stop classification by a word in the reason | M |
| CX-14 | Medium | architecture | coder-new and coder-labor depend on all of `coder` for a few types | L |
| CX-15 | Medium | duplication | Three layers launch external agent CLIs with separate spawn/credential/env code | L |
| CX-16 | Low | duplication | Hand-rolled canonical-JSON digests in coder-scheduler and coder-delegate; misleading comment | S |
| CX-17 | Low | duplication | TypeSafe/Jev key lookup implemented twice | S |
| CX-18 | Low | duplication | Direct-channel wire schema strings defined in three crates | S |
| CX-19 | Low | docs | Stale "Coder One" and Microluna identity in the live delegate crate | S |
| CX-20 | Low | performance | coder-new builds a tokio runtime per call; nested mutexes with `lock().unwrap()` | M |
| CX-21 | Low | build | `build.rs` commit stamping copied between `coder` and `coder-new` and drifted | S |
| CX-22 | Low | maintainability | Paid-labor money amounts are raw i64/u64 msat with `as` casts | S |
| CX-23 | Low | repo-hygiene | Release binary always carries demo fixtures; stale mockup wording; missing READMEs | S |

### CX-01 Launcher recovery SIGKILLs a saved process group with no identity check (PGID reuse)

Severity: High · Category: correctness · Effort: M

Locations: [launcher.rs:543](../../../../crates/coder-service/src/launcher.rs#L543), [launcher.rs:1086](../../../../crates/coder-service/src/launcher.rs#L1086), [launcher.rs:1095](../../../../crates/coder-service/src/launcher.rs#L1095), [launcher.rs:1101](../../../../crates/coder-service/src/launcher.rs#L1101)

Evidence: `recover()` calls `stop_orphan(group, ...)` for any saved `state.host_group` (launcher.rs:544-545). `stop_orphan` refuses only `group <= 1`, its own group, or a group that is not running, then sends `killpg(SIGTERM)` and later `killpg(SIGKILL)`. Its own doc says: "The group identifier could in principle be reused after that host exited." There is no start-time, boot-ID or lock check.

Impact: After a crash or reboot, every service start that finds a stale state file can kill an unrelated process group owned by the same user.

Suggested action:
1. Record the group leader's start time and the boot ID next to `host_group` in the state file.
2. In `stop_orphan`, skip the kill when either differs from the live process.
3. Preferably, have the host hold an `flock` on a lock file for its lifetime and treat "lock acquirable" as "host dead", so no PGID is trusted at all.
4. Verify: add a launcher test that starts an unrelated `sleep` process group, writes a state file with its PGID and a mismatched identity, runs `recover()`, and asserts the group is still alive.

### CX-02 Live coder-delegate compiles 37 prompt and policy files from the deprecated coder-one crate

Severity: High · Category: dead-code · Effort: M

Locations: [system.rs:107](../../../../crates/coder-delegate/src/system.rs#L107), [terminal.rs:202](../../../../crates/coder-delegate/src/terminal.rs#L202), [coder-one/Cargo.toml:17](../../../../crates/coder-one/Cargo.toml#L17), [microluna/src/lib.rs:3](../../../../crates/microluna/src/lib.rs#L3), [gym/src/runs_tui_ask.rs:199](../../../../crates/gym/src/runs_tui_ask.rs#L199)

Evidence: `include_str!`/`include_bytes!` of `../../coder-one/...`: system.rs 34, delegate.rs 2, terminal.rs 1 (37 total). coder-one is the only Cargo dependent of microluna (coder-one/Cargo.toml:17), whose doc says "**Deprecated.**". coder-one has no Cargo dependents and last changed 2026-10-03. However, gym's Runs pane spawns a `coder-one` binary as a child process (gym/src/runs_tui_ask.rs:199, 204, 861; gym/Cargo.toml:40 comment), so "no reverse deps" is true only at the Cargo level.

Impact: The deprecated stack (~137k lines with microluna) cannot be removed without breaking the live system prompt, and every `--workspace` build, test and clippy run compiles it.

Suggested action:
1. Move the referenced prompts and policies into `crates/coder-delegate/{prompts,policies}/` and update the 37 include paths.
2. Tag a `coder-one-final` commit and point `scripts/ember.sh` and `scripts/build-coder-one-linux.sh` at it.
3. Remove or feature-gate gym's coder-one Runs pane (`runs_tui_ask.rs`).
4. Add `crates/coder-one` and `crates/microluna` to the workspace `exclude` list, or move them to `backroom`.
5. Verify: `rg 'coder-one' crates --glob '!crates/coder-one/**'` returns no build-relevant hits, and `cargo build --workspace` passes.

### CX-03 Host client ignores direct-channel reassembly errors and keeps a corrupt buffer

Severity: Medium · Category: correctness · Effort: S

Locations: [client/link.rs:175](../../../../crates/coder-host/src/client/link.rs#L175), [coder-host-wire/src/lib.rs:90](../../../../crates/coder-host-wire/src/lib.rs#L90), [coder-host-wire/src/lib.rs:95](../../../../crates/coder-host-wire/src/lib.rs#L95), [serve/direct.rs:113](../../../../crates/coder-host/src/serve/direct.rs#L113)

Evidence: `Assembler::push` docs say "The caller closes the channel" on error (lib.rs:90-93). link.rs:175 does `let Ok(Some(message)) = assembler.push(&payload) else { continue; };`, which treats `Err` the same as "need more". `push` appends `rest` before matching the flag (lib.rs:99-104), so an unknown flag leaves bytes in the buffer, and on `TooLarge` the stale partial buffer is never cleared.

Impact: One malformed or oversized frame leaves the device link decoding garbage or failing every later message with `TooLarge`, silently. The peer is the authenticated host, so triggering it needs a host bug or a bad relay frame. The host side (serve/direct.rs:113) already closes correctly.

Suggested action:
1. In `Assembler::push`, check the flag before `extend_from_slice`, and clear `self.buffer` before returning any `Err`.
2. In link.rs:175, match the result; on `Err`, set a protocol-error close code and break, as serve/direct.rs:113 does.
3. Verify with wire tests: a `push` after `TooLarge` starts with an empty buffer; an unknown flag leaves the buffer empty.

### CX-04 Self-update on Linux and Windows trusts only a SHA256SUMS file from the same bucket

Severity: Medium · Category: security · Effort: M

Locations: [update.rs:230](../../../../crates/coder-new/src/update.rs#L230), [update.rs:743](../../../../crates/coder-new/src/update.rs#L743), [update.rs:986](../../../../crates/coder-new/src/update.rs#L986)

Evidence: `fetch_verified` fetches the sums file and the archive from the same `base_url` and only compares digests (update.rs:743-770). `signing_team` returns `None` unless `cfg!(target_os = "macos")` (986-989). A non-empty `CODER_BASE_URL` replaces `base_url` with no scheme check (230-240).

Impact: Write access to the release bucket is enough to ship code that standalone Linux and Windows installs auto-install. The env override requires local environment control, which already implies compromise, so it is the smaller part of the risk.

Suggested action:
1. Sign SHA256SUMS in `scripts/release/coder.sh` with an offline Ed25519/minisign key.
2. Embed the public key in coder-new and verify `SHA256SUMS-coder-<v>.sig` in `fetch_verified` on all platforms.
3. Reject a non-https `CODER_BASE_URL` unless it targets localhost.
4. Verify: tests that a bad signature and an `http://` non-localhost override are both refused.

### CX-05 The product binary `coder` is built from crate `coder-new`, while crate `coder` builds a second `coder` binary

Severity: Medium · Category: architecture · Effort: L

Locations: [scripts/release/coder.sh:103](../../../../scripts/release/coder.sh#L103), [coder/src/main.rs:354](../../../../crates/coder/src/main.rs#L354), [coder-new/src/main.rs:141](../../../../crates/coder-new/src/main.rs#L141), [plugins.rs:226](../../../../crates/coder-new/src/plugins.rs#L226), [account.rs:4](../../../../crates/coder-new/src/account.rs#L4), [jev_plugin.rs:411](../../../../crates/coder-new/src/jev_plugin.rs#L411), [scripts/desktop/dev-host.sh:211](../../../../scripts/desktop/dev-host.sh#L211)

Evidence: coder.sh:103-104 sets `product_package=coder-new`, `product_bin=coder-new`. crates/coder/src/main.rs:354 handles `host`. coder-new's state dir defaults to `<openagents>/coder-new` (main.rs:141). User-visible strings include "Settings file: ~/.openagents/coder-new/plugins.json." (plugins.rs:226), `~/.openagents/coder-new/account.json` (account.rs:4), "coder-new login" (account.rs:131), and the idempotency key prefix `coder-new-jev-` (jev_plugin.rs:411). dev-host.sh:211 builds both `-p coder --bin coder` and `-p coder-new --bin coder-new`. The first commit, 86a5b66346 (2026-10-06), is "Add a Ratatui mockup for the new Coder terminal"; 114 commits followed.

Impact: Two `coder` binaries, and a "new" name that leaks into user paths and help text. Developers must know which crate is the product. No correctness impact.

Suggested action:
1. Rename crate `coder-new` (for example to `coder-cli`) with its bin named `coder`.
2. Rename crate `coder` to `coder-agent`; move `coder host`/`task` dispatch into the shipped binary or the openagents companion.
3. Migrate `~/.openagents/coder-new` to `~/.openagents/coder` with a read fallback for the old path; fix the user-visible strings.
4. Update `scripts/release/coder.sh`, `scripts/desktop/dev-host.sh`, `scripts/coderdev` and their tests.
5. Verify: `rg -n 'coder-new' crates scripts` returns only the migration fallback; a release build produces one `coder` binary.

### CX-06 coder-delegate/src/delegate.rs is a 4,443-line god module

Severity: Medium · Category: maintainability · Effort: L

Locations: [delegate.rs:1](../../../../crates/coder-delegate/src/delegate.rs#L1), [delegate.rs:1949](../../../../crates/coder-delegate/src/delegate.rs#L1949), [delegate.rs:3042](../../../../crates/coder-delegate/src/delegate.rs#L3042), [lib.rs:1](../../../../crates/coder-delegate/src/lib.rs#L1)

Evidence: delegate.rs is 4,443 lines. It holds agent/mode/policy types, briefing, summary stream parsers, credential detection, `Executor`/`Cli`, `Delegation`, `Plan`, and `explore_then_delegate` with `#[allow(clippy::too_many_arguments)]`. The crate exposes 554 pub fns across 41 pub mods.

Impact: Any change to how one agent CLI is driven touches a shared 4.4k-line file, and the wide pub surface blocks refactoring.

Suggested action:
1. Split into `delegate/{agent,briefing,summary/{claude,codex,opencode},credential,executor,delegation,plan}.rs` with `pub use` re-exports at the old path.
2. Replace `explore_then_delegate`'s argument list with an `ExploreRequest` struct and drop the `#[allow]`.
3. Make modules with no external users `pub(crate)`.
4. Verify: `cargo test -p coder-delegate` and dependents build unchanged; no file in `delegate/` exceeds ~1,000 lines.

### CX-07 coder-access is a hub crate with 21 dependents that holds product data types

Severity: Medium · Category: architecture · Effort: L

Locations: [lib.rs:9](../../../../crates/coder-access/src/lib.rs#L9), [studio.rs:1](../../../../crates/coder-access/src/studio.rs#L1), [spend.rs:1](../../../../crates/coder-access/src/spend.rs#L1), [environment.rs:1](../../../../crates/coder-access/src/environment.rs#L1), [studio_intents.rs:1](../../../../crates/coder-access/src/studio_intents.rs#L1)

Evidence: lib.rs has 20 pub mods. studio.rs is 2,032 lines, spend.rs 1,199, studio_intents.rs 919, environment.rs 843. 21 other Cargo.toml files under `crates/` and `apps/` reference coder-access.

Impact: Product payload edits rebuild 21 crates, and security-critical enrollment/rights code shares a crate and review surface with fast-moving product types.

Suggested action:
1. Extract the data-type modules (studio, studio_intents, spend, environment, day_plan, crew, media, etc.) into `crates/coder-host-protocol`.
2. Re-export them from coder-access for one release.
3. Point data-only consumers (verse*, terminal-studio) at the new crate.
4. Verify: `cargo tree -i coder-access` shows fewer dependents; editing studio.rs no longer rebuilds access-only consumers.

### CX-08 ~95-variant Operation enum and a 674-line host `execute`

Severity: Medium · Category: maintainability · Effort: L

Locations: [protocol.rs:555](../../../../crates/coder-access/src/protocol.rs#L555), [host/mod.rs:1096](../../../../crates/coder-access/src/host/mod.rs#L1096), [serve/dispatch.rs:610](../../../../crates/coder-host/src/serve/dispatch.rs#L610)

Evidence: `pub enum Operation` spans protocol.rs:555-1019 (about 94-98 variants). host/mod.rs:1096 carries `#[allow(clippy::too_many_arguments)]` on `fn execute` (about 674 lines, 1097-1770); dispatch.rs:610 carries another on `fn publish`.

Impact: Adding an operation means editing several parallel matches across three files; a missed arm in rights or validation is easy to overlook.

Suggested action:
1. Group variants into domain sub-enums with a trait providing `name`, `validate` and `required_right`.
2. Replace `execute`'s parameters with an `ExecContext` struct.
3. Move each domain's arm bodies into `host/<domain>.rs`.
4. Verify: a test that iterates every operation and asserts it has a required right and validation; both `#[allow]` attributes removed.

### CX-09 coder-new `App` is a god object with a ~570-line event handler

Severity: Medium · Category: maintainability · Effort: L

Locations: [lib.rs:86](../../../../crates/coder-new/src/lib.rs#L86), [lib.rs:1348](../../../../crates/coder-new/src/lib.rs#L1348)

Evidence: `pub struct App` spans lib.rs:86-159 with dozens of pub fields; `pub fn handle` starts at 1348 and runs to about 1916. lib.rs had 57 commits since 2026-09-10, and coder-new 114 commits since 2026-10-06.

Impact: Concurrent agents collide on the same struct and match, and screen logic is hard to unit-test in isolation.

Suggested action:
1. Split per-screen state into sub-structs (`Composer`, `Pickers`, `AccountPanel`, `Disclosure`, `AgentsPanel`) that own their key handling.
2. Make `handle` a dispatcher to those sub-structs.
3. Make fields private, with getters for ui.rs.
4. Verify: existing coder-new tests pass; each sub-struct gets its own key-handling unit tests.

### CX-10 coder-host wires callbacks through ~11 global statics whose setters ignore failure

Severity: Medium · Category: architecture · Effort: M

Locations: [control/mod.rs:1148](../../../../crates/coder-host/src/control/mod.rs#L1148), [background.rs:19](../../../../crates/coder-host/src/background.rs#L19), [background.rs:32](../../../../crates/coder-host/src/background.rs#L32), [cli.rs:87](../../../../crates/coder-host/src/cli.rs#L87)

Evidence: 9 `let _ = X.set(..)` sites: `STUDIO_SIM` (cli.rs:87), `LOCAL_CODER`/`RUNNER`/`ENGINES`/`RESULT` (control/mod.rs:1148-1205), `FACTS`/`JUDGE`/`SERVICES`/`RUNNER` (background.rs:32-77). The doc at background.rs:35-36 says: "Without one, a judgment never holds."

Impact: Missed or double wiring silently degrades host behaviour, and tests cannot run two hosts with different hooks in one process.

Suggested action:
1. Introduce a `HostHooks` struct passed into `serve`/`Host::new`.
2. Delete the statics and `set_*` functions; have crates/coder build the struct.
3. Log a warning at startup for each hook left `None`.
4. Verify: `rg 'let _ = .*\.set\(' crates/coder-host` returns nothing; add a test that runs two hosts with different hooks in one process.

### CX-11 Atomic and private file write helpers are copied in 20+ places with different durability

Severity: Medium · Category: duplication · Effort: M

Locations: [coder-new/src/memory.rs:918](../../../../crates/coder-new/src/memory.rs#L918), [coder-new/src/update.rs:315](../../../../crates/coder-new/src/update.rs#L315), [coder-delegate/src/record.rs:484](../../../../crates/coder-delegate/src/record.rs#L484), [coder-lease/src/table.rs:243](../../../../crates/coder-lease/src/table.rs#L243), [coder-host/src/serve/mod.rs:1248](../../../../crates/coder-host/src/serve/mod.rs#L1248), [coder-new/src/plugin_store.rs:265](../../../../crates/coder-new/src/plugin_store.rs#L265)

Evidence: 21 `fn write_atomic`/`write_private` definitions across coder* crates (also coder-environment*, coder-history, coder-working-computer, coder-sync, coder-cloud). memory.rs:918 uses a fixed `path.with_extension("tmp")` and plain `fs::write` with no fsync; update.rs:315 uses `tmp.<pid>` and no fsync.

Impact: Uneven crash durability, and temp-file collisions between concurrent processes writing the same path.

Suggested action:
1. Provide one `write_atomic(path, bytes, Mode::{Private, Public})` in `crates/private-fs`: unique temp name, `O_NOFOLLOW`, fsync of file and parent directory, rename.
2. Replace the copies, starting with memory.rs:918 and update.rs:315.
3. Verify: `rg 'fn write_(atomic|private)' crates` lists only the shared helper; add a concurrency test with two writers to the same path.

### CX-12 Git subprocess spawns do not clear inherited GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE

Severity: Medium · Category: correctness · Effort: M

Locations: [coder-host/src/control/mod.rs:862](../../../../crates/coder-host/src/control/mod.rs#L862), [coder-boundary/src/source.rs:346](../../../../crates/coder-boundary/src/source.rs#L346), [coder-delegate/src/collect.rs:114](../../../../crates/coder-delegate/src/collect.rs#L114), [coder-new/src/memory.rs:332](../../../../crates/coder-new/src/memory.rs#L332), [coder/src/task/studio_git.rs:65](../../../../crates/coder/src/task/studio_git.rs#L65)

Evidence: coder-host `git()` only sets stdin to null (control/mod.rs:862-871). coder-boundary/source.rs:346-348 removes `GIT_DIR`/`GIT_WORK_TREE`/`GIT_COMMON_DIR`, and coder/src/task/studio_git.rs:65 has a separate scrub list, so two scrubbers already exist. Unscrubbed non-test `Command::new("git")` sites in scope: coder-delegate collect (x2), issue (x2), judge, git_fetch; coder-lease artifact/run (x2); coder-new main, memory.

Impact: Under a git hook, or a parent that exported `GIT_DIR`, worktree admission and changed-file collection can act on the wrong repository.

Suggested action:
1. Add one git command builder in coder-boundary that resolves the binary, sets `current_dir`, null stdin, `GIT_TERMINAL_PROMPT=0`, and removes `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`.
2. Migrate the sites above and fold studio_git.rs's list into it.
3. Verify: a test that sets a decoy `GIT_DIR` and asserts the builder operates on the intended repo; `rg 'Command::new\("git"\)'` in scope shows only the builder.

### CX-13 String errors in the paid-labor money path; stop classification by a word in the reason

Severity: Medium · Category: error-handling · Effort: M

Locations: [coder-labor/src/lib.rs:21](../../../../crates/coder-labor/src/lib.rs#L21), [coder-labor/src/paid.rs:1167](../../../../crates/coder-labor/src/paid.rs#L1167), [coder-delegate/src/adapter.rs:598](../../../../crates/coder-delegate/src/adapter.rs#L598), [coder-delegate/src/session.rs:857](../../../../crates/coder-delegate/src/session.rs#L857)

Evidence: coder-labor declares `pub type Result<T> = std::result::Result<T, String>`; paid.rs returns "invoice preparation or payment deadline requires manual reconciliation" as a plain `String`. adapter.rs:598 sets `stopped_by_deadline = reason.contains("deadline")`. session.rs:857 builds reasons as `"{by} proposed a stop"` from the submitter's name, so a proposer whose name contains "deadline" is misclassified.

Impact: Callers cannot tell "needs owner reconciliation" from a retryable failure in the money path, and deadline classification is fragile.

Suggested action:
1. Add `enum LaborError { NeedsReconciliation, Refused, Io, Wallet }` in coder-labor and branch on `NeedsReconciliation` in openagents-cli.
2. In coder-delegate, pass a `StopCause` enum to `Adapter::stop` and set `stopped_by_deadline` from it, not from text.
3. Verify: a test with a submitter named "deadline-bot" asserts `stopped_by_deadline == false`; a labor test asserts the reconciliation case returns the typed variant.

### CX-14 coder-new and coder-labor depend on the whole `coder` crate for a few types

Severity: Medium · Category: architecture · Effort: L

Locations: [coder-new/Cargo.toml:19](../../../../crates/coder-new/Cargo.toml#L19), [coder-labor/Cargo.toml:10](../../../../crates/coder-labor/Cargo.toml#L10), [coder-labor/src/paid.rs:6](../../../../crates/coder-labor/src/paid.rs#L6)

Evidence: Both Cargo.toml files declare `coder = { path = "../coder" }`; coder-labor imports `coder::task::sales`. coder is 262k lines.

Impact: Every edit to `coder` recompiles the shipped terminal and the paid-labor crate.

Suggested action:
1. Extract `crates/coder-task` (task store, intent, grant, execution, sales) and a narrow generate crate.
2. Re-export from `coder` to keep existing paths working.
3. Switch coder-new and coder-labor to the new crates.
4. Verify: `cargo tree -p coder-new` and `-p coder-labor` no longer list `coder`.

### CX-15 Three separate layers launch external agent CLIs

Severity: Medium · Category: duplication · Effort: L

Locations: [coder-delegate/src/delegate.rs:1949](../../../../crates/coder-delegate/src/delegate.rs#L1949), [coder-new/src/bundled_runtime.rs:550](../../../../crates/coder-new/src/bundled_runtime.rs#L550), [coder-new/src/claude_print.rs:1](../../../../crates/coder-new/src/claude_print.rs#L1), [coder/src/delegate.rs:1](../../../../crates/coder/src/delegate.rs#L1)

Evidence: coder-delegate has `Executor`/`Cli`; bundled_runtime.rs forwards `OPENROUTER_API_KEY`/`TYPESAFE_API_KEY` itself (550, 2320); claude_print.rs has its own `claude -p` bridge; crates/coder/src/delegate.rs has its own executor.

Impact: Fixes to process cleanup, environment scrubbing and credential precedence must be repeated in each layer.

Suggested action:
1. Make coder-delegate's `Executor`/`Cli` the single launcher, with an explicit credential `Policy { allow_login_file }`.
2. Route claude_print and bundled_runtime's non-ACP spawns through it; retire coder/src/delegate.rs's executor.
3. Verify: `rg 'Command::new' crates/coder-new/src/{claude_print,bundled_runtime}.rs` shows no direct agent-CLI spawns.

### CX-16 Hand-rolled canonical-JSON digests in coder-scheduler and coder-delegate

Severity: Low · Category: duplication · Effort: S

Locations: [coder-scheduler/src/catalog.rs:463](../../../../crates/coder-scheduler/src/catalog.rs#L463), [coder-delegate/src/policy.rs:498](../../../../crates/coder-delegate/src/policy.rs#L498), [coder-delegate/src/policy.rs:513](../../../../crates/coder-delegate/src/policy.rs#L513)

Evidence: catalog.rs:463 says "the same canonicalization the workspace's digests share" but defines its own `canonicalize` and returns `sha256:<hex>`; policy.rs:513 defines another `canonical` (sorted keys, no whitespace) and returns bare hex.

Impact: A misleading comment and two non-JCS implementations. A mismatch matters only if the digests are compared across subsystems, which nothing does today.

Suggested action:
1. Fix the catalog.rs comment now.
2. When a shared canonical-JSON crate exists, replace both implementations.
3. Verify: add a golden cross-crate test that both produce the same bytes for the same value.

### CX-17 TypeSafe/Jev key lookup implemented twice

Severity: Low · Category: duplication · Effort: S

Locations: [coder-delegate/src/credentials.rs:24](../../../../crates/coder-delegate/src/credentials.rs#L24), [coder-delegate/src/credentials.rs:71](../../../../crates/coder-delegate/src/credentials.rs#L71), [jev-hosted/src/lib.rs:66](../../../../crates/jev-hosted/src/lib.rs#L66)

Evidence: coder-delegate credentials.rs defines `JEV_KEY_VAR`/`JEV_FILE` and reads the env var, then `dir.join(JEV_FILE)` (71-77); jev-hosted defines `TYPESAFE_KEY_VAR`/`JEV_FILE`.

Impact: Key storage rules can diverge between delegate and the hosted resolver.

Suggested action:
1. Have `coder_delegate::credentials` call `jev_hosted::local_key` (coder-delegate already depends on jev-hosted) and delete its constant pair.
2. Verify: `rg 'JEV_FILE' crates` shows a single definition.

### CX-18 Direct-channel wire schema strings are defined in three crates

Severity: Low · Category: duplication · Effort: S

Locations: [coder-host-wire/src/lib.rs:33](../../../../crates/coder-host-wire/src/lib.rs#L33), [coder-access/src/cj.rs:44](../../../../crates/coder-access/src/cj.rs#L44), [openagents-connect/src/nearby.rs:74](../../../../crates/openagents-connect/src/nearby.rs#L74)

Evidence: `CALL`/`ANSWER` constants are duplicated in coder-host-wire and coder-access/cj.rs; openagents-connect duplicates `ANSWER`.

Impact: A schema bump could miss one copy. The strings are versioned literals, so silent change is unlikely.

Suggested action:
1. Re-export `coder_access::cj::{CALL, ANSWER}` from coder-host-wire (or the reverse) and use them in openagents-connect.
2. Verify: add an equality test until the copies are gone.

### CX-19 Stale "Coder One" and Microluna identity in the live delegate crate

Severity: Low · Category: docs · Effort: S

Locations: [credentials.rs:1](../../../../crates/coder-delegate/src/credentials.rs#L1), [agent.rs:150](../../../../crates/coder-delegate/src/agent.rs#L150), [Cargo.toml:1](../../../../crates/coder-delegate/Cargo.toml#L1)

Evidence: Many Coder One and microluna mentions remain in coder-delegate module docs and prompts.

Impact: Readers confuse live code with legacy code.

Suggested action:
1. After CX-02 moves the assets, rewrite module docs to name current callers.
2. Move coder-one-only modules back into coder-one (or drop them).
3. Verify: `rg -i 'coder one|microluna' crates/coder-delegate` returns only intentional history notes.

### CX-20 coder-new builds a tokio runtime per call and nests mutexes with `lock().unwrap()`

Severity: Low · Category: performance · Effort: M

Locations: [prompt_queue.rs:4](../../../../crates/coder-new/src/prompt_queue.rs#L4), [prompt_queue.rs:33](../../../../crates/coder-new/src/prompt_queue.rs#L33), [prompt_queue.rs:90](../../../../crates/coder-new/src/prompt_queue.rs#L90)

Evidence: 17 `new_current_thread()` sites in coder-new/src. `pub type Inbox = Arc<Mutex<Vec<Arc<Mutex<Option<String>>>>>>`, locked with `.lock().unwrap()` at prompt_queue.rs:33, 61, 90, 129.

Impact: Extra runtime setup per call, and a poisoned lock panics the UI thread.

Suggested action:
1. Create one runtime and pass its `Handle` into `App`.
2. Replace the per-slot `Mutex<Option<String>>` with a take-once primitive (for example a oneshot or `OnceLock`).
3. Use `unwrap_or_else(PoisonError::into_inner)` where a mutex remains.
4. Verify: `rg 'new_current_thread' crates/coder-new/src` returns one site.

### CX-21 `build.rs` commit stamping is copied between `coder` and `coder-new` and has drifted

Severity: Low · Category: build · Effort: S

Locations: [coder/build.rs:1](../../../../crates/coder/build.rs#L1), [coder-new/build.rs:87](../../../../crates/coder-new/build.rs#L87)

Evidence: coder/build.rs is 70 lines; coder-new/build.rs is 136 lines and has `is_object_name` validation (line 87) that coder lacks.

Impact: The two binaries can stamp commits inconsistently.

Suggested action:
1. Extract a `build-stamp` build-dependency crate and call it from both build.rs files.
2. Verify: both binaries report the same commit string for the same checkout.

### CX-22 Money amounts in paid labor are raw i64/u64 msat with `as` casts

Severity: Low · Category: maintainability · Effort: S

Locations: [paid.rs:101](../../../../crates/coder-labor/src/paid.rs#L101), [paid.rs:1192](../../../../crates/coder-labor/src/paid.rs#L1192)

Evidence: `terms.price_msat > i64::MAX as u64` and `self.worker.price_msat != terms.price_msat as i64` (101-104); `setup.worker.price_msat as u64` (1192).

Impact: A negative price that bypassed validation would wrap to a huge invoice amount.

Suggested action:
1. Use `u64::try_from` at 1192 and return a refusal on failure.
2. Introduce a non-negative msat newtype for `WorkerTerms.price_msat`.
3. Verify: a test with a negative stored price asserts a refusal, not an invoice.

### CX-23 Release binary always carries the demo fixtures; stale wording; missing READMEs

Severity: Low · Category: repo-hygiene · Effort: S

Locations: [lib.rs:58](../../../../crates/coder-new/src/lib.rs#L58), [Cargo.toml:28](../../../../crates/coder-new/Cargo.toml#L28), [lib.rs:1347](../../../../crates/coder-new/src/lib.rs#L1347)

Evidence: `DEMO_AVAILABLE = cfg!(debug_assertions)` (lib.rs:58), but `coder-demo-ui` is a non-optional dependency (Cargo.toml:28). lib.rs:1347 still says "Returns false when the preview should close." Several in-scope crates have no README.

Impact: Larger release binaries and leftover mockup wording.

Suggested action:
1. Put `coder-demo-ui` behind an optional `demo` feature, enabled by `scripts/coderdev`.
2. Fix the lib.rs:1347 comment.
3. Add short READMEs derived from each crate's lib.rs docs.
4. Verify: `cargo tree -p coder-new -e normal --release` (default features) does not list coder-demo-ui.

## Refuted during verification

- **session.rs:932 misreports a stop rule mentioning "deadline" as a host deadline.** The fallback at session.rs:931-935 runs only when `stop_by` is `None`, where reasons are fixed internal strings ("a stop rule fired", "the session was silent...", "the host deadline of N ms passed"); submitted stops set `stop_by`. The reachable variant (adapter.rs:598 via a submitter name) is kept in CX-13.
- **TypeSafe key looked up by four independent implementations.** coder/src/decision.rs:122-127 already calls `jev_hosted::local_key`, and model-access/store.rs only maps `Provider::TypeSafe` to a filename. Only coder-delegate and jev-hosted implement the lookup; kept as CX-17 (low).
