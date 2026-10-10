# Desktop & chat apps

**Scope:** `crates/{openagents-desktop, openagents-chat-app, openagents-chat, terminal-app}` and `bins/openagents-desktop-macos`. **Health grade: C.** Audit date 2026-10-10, snapshot `3168c986aa`.

This area is about 122k lines of Rust in four crates: openagents-desktop (54.0k), openagents-chat-app (40.4k), openagents-chat (25.9k) and terminal-app (1.6k). The macOS bundle scaffolding sits in `bins/openagents-desktop-macos`. All of it is under two weeks old (first commits 2026-09-29), and the four crates took 463 commits in the last 30 days. Testing is heavy (825 test functions). The production panic surface is small: about 64 unwraps and 35 expects outside test modules. Unsafe blocks are commented, `#[allow]` is rare, the updater's verification chain is careful, and doc comments explain intent.

The main problem is structure, not correctness. Code is piling into a few god files and god structs, and those are also the most-changed files in the repo: `shell.rs` (6,756 lines, 96 commits in 30 days), `chat.rs` (5,673 lines, a ~70-field `Panel`, 73 commits), `coder_tab.rs` (5,589 lines, 40-field `CoderTab`), `client.rs` (3,667 lines, 27-method `Coder` trait) and `coder_events.rs` (3,031 lines). The desktop and the phone each keep their own chat controller. The shipping desktop crate and binary also carry episode-deck demo scenes, the release acceptance gate, benchmarks and a fake host. Production code parses another crate's Rust source and the repo-root `deploy/` directory at compile time, and tests scrape sibling crates' source as text.

Reliability risks are moderate and specific. The async chat client runs blocking work (git subprocesses, filesystem reads) on the Tokio runtime, and `Client::collect` uses `block_in_place`, which panics on a current-thread runtime. The desktop worker routes requests to lanes through bool predicates backed by six `unreachable!` arms. The phone follows an open thread by polling every 300 ms. BYOK key zeroization in `seal_payer` is a dead store. `openagents-chat` has become a 23-module hub that pulls UI markdown types into the host daemon. None of these is a live data-loss or security hole, but together they are serious maintainability drag in the most active code in the monorepo.

## Measurements

| Metric | Value |
|---|---|
| LOC (tracked `.rs`) | desktop 54,049 (73 files), chat-app 40,359 (51), chat 25,871 (32), terminal-app 1,561 (3); total ~121,840 |
| Largest files | desktop `shell.rs` 6,756 (2,070 prod + 4,686 inline tests), desktop `chat.rs` 5,673, chat-app `coder_tab.rs` 5,589, chat `client.rs` 3,667, chat-app `gym.rs` 3,125, chat `coder_events.rs` 3,031, desktop `acceptance.rs` 2,779, desktop `update.rs` 2,692, desktop `route_map.rs` 2,106 |
| Longest functions | `route_map::paint` 361, `chat::Panel::action` 358, `coder_events::step` 358, `coder_tab::run_intent` 347, `chrome::sidebar` 314, `gym::cards_for` 310, `client::send` 269 |
| Structs with 25 or more fields | `chat::Panel` ~70 (69 by line count), `coder_tab::CoderTab` 40, `route_map::MapPage` 33, `coder_run::Run` 29, `model::Model` 28 |
| Tests | desktop 380, chat-app 254, chat 188, terminal-app 3 (1 ignored soak); 14 `#[ignore]` |
| Sleeps in tests | desktop 48, chat-app 15, chat 16 |
| unwrap / expect / panic (all code) | desktop 921/210/135, chat-app 486/39/37, chat 518/8/42 (mostly tests) |
| Outside test modules | 64 unwrap, 35 expect, 10 panic or unreachable |
| TODO/FIXME | 0 real |
| `#[allow]` | 13 (9 `too_many_arguments`) |
| `unsafe` | 42 in desktop (FFI; all but about 5 have SAFETY comments), 1 in chat (`seal_payer` zeroization) |
| `#[path]` redirects | 14 in desktop src (9 in `shell.rs`) |
| Detached thread spawns in production | ~30 |
| Dependents | openagents-chat 15 crates, chat-app 9, desktop 1 (dev-dep of `coder`, which is an optional dep of desktop, so the two form a cycle) |
| Churn, 30 days | desktop 190 commits, chat-app 155, chat 118, terminal-app 15; top files `shell.rs` 96, `chat.rs` 73, `coder_tab.rs` 51, `route_map/sources.json` 42 (106 KB, generated) |
| Poison-tolerant lock helpers | 13 copies |

## Strengths

- Production panic surface is small for this size (about 64 unwraps and 35 expects outside test modules), and most expects document an invariant ("valid composer metrics"). Mutex poisoning is tolerated on purpose rather than unwrapped.
- The updater (`crates/openagents-desktop/src/update.rs`) verifies an Ed25519 signature over the manifest's raw bytes before parsing, refuses downgrades, checks size and SHA-256, runs codesign and spctl and checks the Team ID, swaps the bundle atomically with rollback, and resumes downloads with ranges. Its `let _ =` cleanup calls are intentional and scoped.
- Clear process separation: the window never holds a host key and talks to the host only over a same-user control socket with read and write timeouts (`control.rs:422`). Dev and release builds use separate keychain items (`lib.rs` `RELEASE`).
- Unsafe FFI in the macOS, Windows and Linux platform code nearly always carries a SAFETY comment. Workspace lints deny `todo!`, `unimplemented!` and `dbg!`, and there are 0 real TODO/FIXME markers.
- High test density (825 tests), including copy guards (`oa_copy::scan_dir`), accessibility-tree checks, QR read-back tests, a version-lockstep test (`tests/version_lockstep.rs`) and a route-map snapshot pinned by `crates/coder/tests/route_map_sources.rs`.
- Module and item docs explain intent and cite issue numbers.
- The shared-logic split is mostly right: route_map layout and data live in chat-app and the desktop only paints them, and `coder_events` gives every surface one typed event stream.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| APP-01 | High | maintainability | Desktop chat `Panel` is a ~70-field god object in a 5.7k-line file; feedback dialog reuses the rename dialog's field | L |
| APP-02 | Medium | architecture | `shell.rs` is a 6.8k-line bin-only module with 9 `#[path]` redirects, mostly inline tests | M |
| APP-03 | Medium | architecture | Deck demo scenes, acceptance gate, benchmarks and fake host ship in the production desktop crate and binary | L |
| APP-04 | Medium | architecture | Desktop and chat-app read other crates' source and repo-root files via `include_str!` | M |
| APP-05 | Medium | concurrency | Async chat client runs blocking Coder calls on Tokio; `collect()` uses `block_in_place` | M |
| APP-06 | Medium | duplication | Two parallel 5.5k-line chat controllers: desktop `chat::Panel` and phone `CoderTab` | XL |
| APP-07 | Medium | error-handling | Desktop worker routes requests to lanes with bool predicates and six `unreachable!` arms | M |
| APP-08 | Medium | architecture | openagents-chat pulls UI markdown types (rust-native) into coder-host's host feature | L |
| APP-09 | Medium | security | BYOK key plaintext zeroization is a dead store the optimizer may remove | S |
| APP-10 | Low | maintainability | `coder_events::step` parses ATIF steps through untyped `serde_json::Value` | M |
| APP-11 | Low | performance | Phone follows an open thread by polling the newest page every 300 ms | M |
| APP-12 | Low | repo-hygiene | Generated 106 KB `route_map/sources.json` lives in `src/` and compiles into both apps | S |
| APP-13 | Low | duplication | Thirteen copies of the poison-tolerant Mutex lock idiom | S |
| APP-14 | Low | architecture | Process-global mutable theme read from 128 call sites | L |
| APP-15 | Low | error-handling | Host capability detection greps `coder host help` output for "keychain" | S |
| APP-16 | Low | duplication | Two separate banned-word lists for user-facing copy | S |
| APP-17 | Low | naming | terminal-app's binary is named `openagents-terminal`, the same as a different crate | S |
| APP-18 | Low | maintainability | Hand-rolled CLI parsing in both app binaries | S |
| APP-19 | Low | docs | Stale or misattached docs and crate descriptions | S |
| APP-20 | Low | build | Redundant and inconsistent cfg gates in the desktop binary | S |
| APP-21 | Low | duplication | Browser-opening code duplicated and spawned without reaping the child | S |
| APP-22 | Low | security | macOS window process gets allow-unsigned-executable-memory and network.server | S |
| APP-23 | Low | error-handling | `home()` falls back to `/` when HOME and USERPROFILE are unset | S |
| APP-24 | Low | repo-hygiene | Unused re-export shim and a borrowed mobile fixture in chat-app's `lib.rs` | S |
| APP-25 | Low | testing | Many sleep-based waits in UI and acceptance tests | M |

### APP-01 Desktop chat Panel is a ~70-field god object in a 5.7k-line file; the feedback dialog secretly reuses the rename dialog's field

**Severity:** High · **Category:** maintainability · **Effort:** L

**Locations:** [chat.rs:44](../../../../crates/openagents-desktop/src/chat.rs), [chat.rs:70](../../../../crates/openagents-desktop/src/chat.rs), [chat.rs:1714](../../../../crates/openagents-desktop/src/chat.rs), [chat.rs:3320](../../../../crates/openagents-desktop/src/chat.rs), [chat.rs:3830](../../../../crates/openagents-desktop/src/chat.rs)

**Evidence:** `chat.rs` is 5,673 lines. `pub struct Panel` (line 44) has 69 single-line field declarations. The field doc at line 70 reads "The open Give feedback dialog. It reuses the rename dialog's ...". `feedback_panel` calls `self.rename.as_ref().expect("an open feedback dialog")` and then `self.feedback.as_ref().expect(...)`. `self.rename` appears on 42 lines.

**Impact:** Every chat feature edit lands in one file and one struct, which causes merge conflicts in the most-churned area. Because of the implicit rename/feedback coupling, a change to rename state can panic the feedback dialog.

**Suggested action:**
1. Replace `rename` + `feedback` with one `dialog: Option<Dialog>`, where `enum Dialog { Rename { chat, field }, Feedback { field, state: FeedbackDialog } }`. This removes both expects.
2. Split `chat.rs` into `chat/{palette,dialogs,runs,transcript,changes}.rs`, with `Panel` holding the sub-structs.
3. Break `Panel::action` (358 lines) into one handler per Intent family.
4. Verify: the chat and shell tests pass unchanged.

### APP-02 shell.rs is a 6.8k-line bin-only module with 9 #[path] redirects and most of its length in inline tests

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:** [shell.rs:16](../../../../crates/openagents-desktop/src/shell.rs), [shell.rs:21](../../../../crates/openagents-desktop/src/shell.rs), [shell.rs:6132](../../../../crates/openagents-desktop/src/shell.rs), [shell.rs:6755](../../../../crates/openagents-desktop/src/shell.rs)

**Evidence:** 6,756 lines (2,070 production, 4,686 inline tests). `#[path]` at lines 16 (`settings_shell.rs`), 21 (`acceptance.rs`), and 6132, 6135, 6140, 6145, 6150, 6750, 6755 (seven test modules). There are 14 `#[path]` attributes in `crates/openagents-desktop/src` in total.

**Impact:** The module tree can't be read from the file layout. `DesktopApp` and its tests build only inside the bin target, which is the largest compile unit.

**Suggested action:**
1. Convert to `src/shell/mod.rs`, `src/shell/settings.rs`, `src/shell/tests/*.rs` and `src/acceptance/{mod,loop_,dead_task,persona}.rs`, and remove every `#[path]`.
2. Then consider moving `DesktopApp`, worker and platform into the lib behind the `app` feature.
3. Verify: `rg '#\[path' crates/openagents-desktop/src` returns nothing and the test count is unchanged.

### APP-03 Episode-deck demo scenes, the acceptance gate, benchmarks and the fake host ship inside the production desktop crate and binary

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:** [route_future.rs:1](../../../../crates/openagents-desktop/src/route_future.rs), [route_chat.rs:1](../../../../crates/openagents-desktop/src/route_chat.rs), [slide_embeds.rs:43](../../../../crates/openagents-desktop/src/slide_embeds.rs), [main.rs:53](../../../../crates/openagents-desktop/src/main.rs), [main.rs:123](../../../../crates/openagents-desktop/src/main.rs), [Cargo.toml:1](../../../../crates/openagents-desktop/Cargo.toml), [Cargo.toml:15](../../../../crates/openagents-desktop/Cargo.toml)

**Evidence:** Deck code is about 6.2k lines: `route_future` 1,160 ("the Episode 289 deck's third slide ... fed a synthetic, growing model"), `route_plugin` 810, `route_chat` 535, `route_live` 1,209, `slides` 1,312, `slide_embeds` 1,147. The acceptance files add 3,900 lines, `benchmark.rs` 617 and `fake.rs` 546. The `main.rs` USAGE lists `--fake-host`, `--fake-scan`, `--fake-phones`, `--chat-benchmark` and `--acceptance`. The default `app` feature pulls in `dep:openagents-deck`.

**Impact:** Demo and test-harness code adds to the shipping app's binary size, compile time and churn, and deck edits can cause regressions in the product app.

**Suggested action:**
1. Put the deck scenes (`route_future`, `route_plugin`, `route_chat`, `slides`, `slide_embeds`) behind a non-default `deck` feature, or move them into `openagents-deck`.
2. Put `acceptance*`, `benchmark` and `fake` behind a `harness` feature, or into a separate bin with `required-features`.
3. Make the release packaging scripts build without either feature.
4. Verify: a release build without the features compiles, and `strings` on the binary finds no `routes-future`.

### APP-04 Desktop and chat-app code reads other crates' source files and repo-root files via include_str!

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:** [slide_embeds.rs:52](../../../../crates/openagents-desktop/src/slide_embeds.rs), [slide_embeds.rs:166](../../../../crates/openagents-desktop/src/slide_embeds.rs), [slide_embeds.rs:1078](../../../../crates/openagents-desktop/src/slide_embeds.rs), [eval_cards.rs:503](../../../../crates/openagents-chat-app/src/eval_cards.rs), [router.rs:1507](../../../../crates/openagents-chat/src/router.rs), [gym/tests.rs:1575](../../../../crates/openagents-chat-app/src/gym/tests.rs), [tests.rs:507](../../../../crates/openagents-desktop/src/tests.rs)

**Evidence:**
- `const DOWNLOAD_SOURCE: &str = include_str!("../../openagents-web/src/pages/download.rs")` is parsed at runtime with `constant(...).unwrap_or_default()`.
- `eval_cards.rs:503` does `include_str!("../../../deploy/eval-runner/catalog")` in production code.
- The router test reads `../../coder/src/cli_route/gate.rs` and looks for `pub const PHONE_COMMANDS`.
- `gym/tests.rs:1575` reads `ext-eval/src/author/stage.rs`.
- desktop `tests.rs` `include_str!`s `main.rs`, `shell.rs`, `worker.rs` and `mac.rs`.

**Impact:** The type checker can't see this coupling. A rename or reformat in openagents-web, coder or ext-eval breaks a test in another crate, and it blocks moving these crates. The production download slide relies on a test (`the_download_card_matches_the_page`, `slide_embeds.rs:1078`) rather than on types to catch drift.

**Suggested action:**
1. Move `CODER_VERSION`, `CODER_SH` and `CODER_PS1` into a small leaf crate (for example `release-facts`) that both openagents-web and openagents-desktop depend on.
2. Move `PHONE_COMMANDS` into `route-contract` and re-export it from `coder::cli_route::gate`.
3. Expose the eval-runner catalog as a const from a crate (or `build.rs`) that owns `deploy/eval-runner`.
4. Replace the source-grepping tests with type-level references.
5. Verify: `rg 'include_str!\("\.\./\.\./[^"]*\.rs"' crates/{openagents-desktop,openagents-chat-app,openagents-chat}` returns nothing.

### APP-05 Async chat client runs blocking Coder calls (git subprocesses, filesystem reads) on the Tokio runtime; collect() uses block_in_place

**Severity:** Medium · **Category:** concurrency · **Effort:** M

**Locations:** [client.rs:1106](../../../../crates/openagents-chat/src/client.rs), [client.rs:2363](../../../../crates/openagents-chat/src/client.rs), [client.rs:2073](../../../../crates/openagents-chat/src/client.rs), [client.rs:1992](../../../../crates/openagents-chat/src/client.rs), [local.rs:319](../../../../crates/coder/src/task/local.rs), [chat_client.rs:468](../../../../crates/coder/src/task/chat_client.rs)

**Evidence:** `run_coder` calls `self.coder.checkout(dir)` inline (`client.rs:2363`), and the real checkout runs `git rev-parse` twice (`local.rs:320` and `330`). `if self.coder.drafted(&draft)` (`client.rs:2073`) reads `~/.openagents/background` drafts from disk (`chat_client.rs:468`). `self.coder.effect(&argv)` runs inline at `client.rs:1992`. The client already uses `spawn_blocking` at 24 other sites. `collect()` does `tokio::task::block_in_place(|| crate::thread::collect(id, |c| handle.block_on(self.apply(c))))`, and `block_in_place` panics on a `current_thread` runtime.

**Impact:** A slow repo or disk stalls a runtime worker while a reply streams. A future caller on a current-thread runtime, including a default `#[tokio::test]`, panics in `collect()`. No such caller exists today.

**Suggested action:**
1. Wrap `checkout`, `drafted` and `effect` in `tokio::task::spawn_blocking`, as the other 24 call sites do.
2. Rewrite `thread::collect` to take an async page fetcher so `block_in_place` and `block_on` go away.
3. Verify: add a `#[tokio::test(flavor = "current_thread")]` that calls `collect`.

### APP-06 Two parallel 5.5k-line chat controllers: desktop chat::Panel and the phone's CoderTab

**Severity:** Medium · **Category:** duplication · **Effort:** XL

**Locations:** [chat.rs:44](../../../../crates/openagents-desktop/src/chat.rs), [coder_tab.rs:404](../../../../crates/openagents-chat-app/src/coder_tab.rs), [acceptance_persona.rs:140](../../../../crates/openagents-desktop/src/acceptance_persona.rs)

**Evidence:** `chat.rs` is 5,673 lines and `coder_tab.rs` is 5,589. chat-app's `lib.rs` describes the crate as "Chat application state shared by desktop and phone adapters", yet the desktop keeps its own `Panel` with followup, attention and seen state that parallels `CoderTab`'s.

**Impact:** Behavior changes have to be made twice, and the two controllers drift apart.

**Suggested action:**
1. Extract the platform-neutral state (followups and starters, seen and attention marks, auto-start, submission bookkeeping) into chat-app types that both `Panel` and `CoderTab` hold.
2. Keep only the native field, focus and painting code in `Panel`.
3. Add a Panel-driven acceptance scenario so the shared behavior is exercised through the desktop.
4. Verify: the existing chat, shell and coder_tab tests pass, and the new scenario passes on both adapters.

### APP-07 Desktop worker sends requests to lanes with bool predicates and six unreachable! arms

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:** [worker.rs:90](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:95](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:253](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:308](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:355](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:424](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:511](../../../../crates/openagents-desktop/src/worker.rs), [worker.rs:719](../../../../crates/openagents-desktop/src/worker.rs)

**Evidence:** `fn coder(request: &Request) -> bool` and `fn local(...)` choose the lane, and there are 6 `unreachable!` sites. The unwrap at line 247 directly follows the `saved_history` insert, so it is safe in practice. `self.fake` branches appear in production lanes (lines 194, 280, 292, 317, 562, 784).

**Impact:** A new `Request` variant routed to the wrong lane panics that lane's thread at runtime instead of failing to compile. The fake-mode branches mix test behavior into production paths.

**Suggested action:**
1. Split `Request` into `enum Request { Host(HostRequest), Local(LocalRequest), Coder(CoderRequest) }`, so dispatch is an exhaustive match and each lane's `run` has no catch-all arm.
2. Replace `fake: bool` with a platform trait that has a real and a fake implementation.
3. Replace the unwrap at line 247 with `get_or_insert_with`.
4. Verify: `rg 'unreachable!' crates/openagents-desktop/src/worker.rs` returns nothing and the worker tests pass.

### APP-08 openagents-chat pulls UI markdown types (rust-native) into coder-host's host feature

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:** [lib.rs:1](../../../../crates/openagents-chat/src/lib.rs), [basic_chats.rs:17](../../../../crates/openagents-chat/src/basic_chats.rs), [basic_chats.rs:84](../../../../crates/openagents-chat/src/basic_chats.rs), [coder-host Cargo.toml:16](../../../../crates/coder-host/Cargo.toml), [coder-host Cargo.toml:39](../../../../crates/coder-host/Cargo.toml)

**Evidence:** `lib.rs` declares 23 modules. `basic_chats.rs:17` is `use rust_native::markdown::IncrementalMarkdown;` and line 84 is `Streaming(Vec<rust_native::markdown::Block>)`. coder-host's host feature includes `dep:openagents-chat` (Cargo.toml lines 16 and 39). openagents-chat has 15 dependent crates.

**Impact:** The host daemon links UI rendering crates, and edits to presentation helpers recompile many dependents.

**Suggested action:**
1. Store raw markdown text in `basic_chats` and build `IncrementalMarkdown` in the adapters.
2. Split out an `openagents-chat-core` crate (thread, cache, basic_link, service, migrate, remote, delegation) with no rust-native dependency, and point coder-host at it.
3. Verify: `cargo tree -p coder-host -e normal --features host | rg rust-native` returns nothing.

### APP-09 BYOK key plaintext zeroization is a dead store the optimizer may remove

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:** [basic_coder.rs:678](../../../../crates/openagents-chat/src/basic_coder.rs), [basic_coder.rs:685](../../../../crates/openagents-chat/src/basic_coder.rs)

**Evidence:** `let mut plaintext = keys.envelope_plaintext(); ... unsafe { plaintext.as_bytes_mut().fill(0) };` runs right before the drop, with no volatile write or compiler fence.

**Impact:** Provider API keys may stay in freed heap memory, and the code uses an `unsafe` block it does not need.

**Suggested action:**
1. Return `zeroize::Zeroizing<String>` from `model_access::Keys::envelope_plaintext`, or wrap the value at the call site.
2. Delete the `unsafe` block.
3. Verify: the `seal_payer` tests pass.

### APP-10 coder_events::step parses ATIF steps through untyped serde_json::Value

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:** [coder_events.rs:981](../../../../crates/openagents-chat/src/coder_events.rs), [atif document.rs:120](../../../../crates/atif/src/document.rs)

**Evidence:** `pub fn step(&mut self, step: &Value)` does `step["step_id"].as_u64().unwrap_or(0)` and `step["source"].as_str().unwrap_or("system")`, and falls back from `extra` to `extensions`. Its doc says it accepts both the ATIF document form and the log record form. `atif::document::Step` (`document.rs:120`) has `at`, `source`, `message` and so on, but no `step_id` or `extra`, so it does not cover what `step()` reads.

**Impact:** Schema drift silently falls back to defaults. The 358-line function is hard to change safely.

**Suggested action:**
1. In `crates/atif`, define a serde `#[serde(untagged)] enum WireStep { Document(...), Record(...) }` that covers `step_id` and `extra`/`extensions` as serialized on the wire.
2. Deserialize into it in `coder_events` and split `step()` into per-source handlers.
3. Verify: replay the `fixtures/coder-events` ndjson files and confirm the event output is unchanged.

### APP-11 Phone follows an open thread by polling the newest page every 300 ms on detached threads

**Severity:** Low · **Category:** performance · **Effort:** M

**Locations:** [host_threads.rs:70](../../../../crates/openagents-chat-app/src/host_threads.rs), [host_threads.rs:72](../../../../crates/openagents-chat-app/src/host_threads.rs), [host_threads.rs:792](../../../../crates/openagents-chat-app/src/host_threads.rs), [host_threads.rs:1123](../../../../crates/openagents-chat-app/src/host_threads.rs), [host_threads.rs:1197](../../../../crates/openagents-chat-app/src/host_threads.rs)

**Evidence:** `STREAMING` is 300 ms and `SETTLED` is 3 s. `follow` loops on `link.read(host, thread, None)` (the newest page, not the whole thread) and then sleeps. Each loop exits when `open.generation` changes (line 1126), so follow threads do not accumulate. The file has 6 `std::thread::spawn` sites.

**Impact:** About 3 relay round trips per second while a reply streams, which costs battery and host load.

**Suggested action:**
1. Add an `after` cursor or revision ETag to the NIP-HOST `thread.read`, so an unchanged poll returns no turns; alternatively, push activity notifications.
2. Consider one worker thread with a channel for the send, stop and run retries instead of separate spawns.
3. Verify: count relay requests during a streamed reply before and after the change.

### APP-12 Generated 106 KB route_map/sources.json lives in src/ and is compiled into both apps

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:** [sources.rs:19](../../../../crates/openagents-chat-app/src/route_map/sources.rs), [sources.json](../../../../crates/openagents-chat-app/src/route_map/sources.json)

**Evidence:** Per the reviewer, `include_str!("sources.json")` embeds a 106 KB file that took 42 commits in 30 days. Not re-measured.

**Impact:** Concurrent agents keep hitting merge conflicts in it.

**Suggested action:**
1. Mark it `linguist-generated` in `.gitattributes`.
2. Write it one route per line in a stable order.
3. Add a regen script for agents to run after a rebase.
4. Verify: `crates/coder/tests/route_map_sources.rs` still passes after a regen.

### APP-13 Thirteen copies of the poison-tolerant Mutex lock idiom

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:** [basic_coder.rs:869](../../../../crates/openagents-chat/src/basic_coder.rs), [host_threads.rs:535](../../../../crates/openagents-chat-app/src/host_threads.rs), [chats.rs:141](../../../../crates/openagents-chat-app/src/chats.rs), [grid/store.rs:33](../../../../crates/openagents-desktop/src/grid/store.rs)

**Evidence:** There are 13 `into_inner` poison-recovery sites across the three crates, and 8 files define their own `fn lock`.

**Impact:** The code is duplicated and the poison policy could drift, although today every copy behaves the same.

**Suggested action:**
1. Add one `LockExt::lock_unpoisoned` in a leaf crate.
2. Replace the local helpers with it.
3. Verify: `rg 'into_inner' crates/{openagents-desktop,openagents-chat-app,openagents-chat}/src` finds only the shared helper.

### APP-14 Process-global mutable theme read from 128 call sites

**Severity:** Low · **Category:** architecture · **Effort:** L

**Locations:** [visual.rs:401](../../../../crates/openagents-chat-app/src/visual.rs), [visual.rs:405](../../../../crates/openagents-chat-app/src/visual.rs), [visual.rs:412](../../../../crates/openagents-chat-app/src/visual.rs)

**Evidence:** `static LIGHT_ACTIVE: AtomicBool` and a thread_local `SCOPED` back `visual::current()`, which is called 128 times across desktop and chat-app. The design is deliberate and documented, and `scoped()` already handles test isolation.

**Impact:** Views depend on hidden global state, and per-window themes are impossible.

**Suggested action:**
1. Pass a `&Visual` into leaf view builders, one area at a time, starting with chat-app cards, `coder_run` and `gym`.
2. Verify: the `visual::current()` call count drops with each step and the snapshot and accessibility tests pass.

### APP-15 Host capability detection greps `coder host help` output for 'keychain'

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:** [migrate.rs:129](../../../../crates/openagents-desktop/src/migrate.rs)

**Evidence:** `text.contains("keychain")` runs on the lowercased stdout of `coder host help`. The doc comment says `false` leaves the earlier setup running, which is the safe default.

**Impact:** If the help text is reworded, for example to add a note that mentions the keychain negatively, detection returns a false positive and migrates the setup to a host that loses its key.

**Suggested action:**
1. Add `coder host capabilities --json` and use it, keeping the grep only as a fallback for older binaries.
2. Extend the migrate tests with a help text that mentions the keychain negatively.
3. Verify: the new test fails against the grep alone and passes with the capabilities query.

### APP-16 Two separate banned-word lists for user-facing copy

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:** [words.rs:18](../../../../crates/openagents-desktop/src/words.rs), [oa-copy lib.rs:18](../../../../crates/oa-copy/src/lib.rs)

**Evidence:** desktop `words.rs` defines `pub const BANNED` (line 18), separate from oa-copy's `TERMS` and `PHRASES`.

**Impact:** The two copy policies drift apart.

**Suggested action:**
1. Fold the desktop's terms into an oa-copy profile.
2. Make `words::banned_in` a thin wrapper over it.
3. Verify: the desktop copy tests and `oa_copy::scan_dir` checks pass.

### APP-17 terminal-app's binary is named openagents-terminal, the same as a different crate, and both use the 'OpenAgents Terminal' description

**Severity:** Low · **Category:** naming · **Effort:** S

**Locations:** [terminal-app Cargo.toml:7](../../../../crates/terminal-app/Cargo.toml), [terminal-app Cargo.toml:10](../../../../crates/terminal-app/Cargo.toml), [openagents-terminal Cargo.toml:2](../../../../crates/openagents-terminal/Cargo.toml), [openagents-terminal Cargo.toml:11](../../../../crates/openagents-terminal/Cargo.toml)

**Evidence:** terminal-app has `[[bin]] name = "openagents-terminal"` and the description "OpenAgents Terminal: the shared smart terminal in a native window". The `openagents-terminal` package's description is "OpenAgents Terminal: a full-screen chat ...".

**Impact:** `--bin openagents-terminal` and `-p openagents-terminal` refer to different products.

**Suggested action:**
1. Rename terminal-app's bin to `openagents-terminal-window`.
2. Give the two crates distinct descriptions.
3. Update the release scripts and docs that reference the old bin name.
4. Verify: `cargo build --bin openagents-terminal-window` works and `rg 'bin openagents-terminal\b'` finds no stale references.

### APP-18 Hand-rolled CLI parsing in both app binaries

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:** [main.rs:103](../../../../crates/openagents-desktop/src/main.rs), [window.rs:14](../../../../crates/terminal-app/src/window.rs)

**Evidence:** The desktop parser is a manual match over about 20 flags, with a separate `USAGE` constant (`main.rs:53-78`).

**Impact:** The help text and the parser can drift apart.

**Suggested action:**
1. Switch both binaries to clap derive, with value parsers for the ranged flags.
2. Verify: `--help` output lists every flag and the existing flag tests pass.

### APP-19 Stale or misattached docs and crate descriptions

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:** [coder_events.rs:42](../../../../crates/openagents-chat/src/coder_events.rs), [desktop Cargo.toml:1](../../../../crates/openagents-desktop/Cargo.toml), [desktop Cargo.toml:15](../../../../crates/openagents-desktop/Cargo.toml), [chat-app lib.rs:2](../../../../crates/openagents-chat-app/src/lib.rs)

**Evidence:** `coder_events.rs:42-44` stacks two doc comments onto `STOPPED_AS_ASKED`. The owner-death doc belongs to `OWNER_ENDED_MESSAGE` (line 47), which has no doc. The desktop Cargo description says "OpenAgents for Mac: pair a phone by QR code, manage phones, and watch Coder". chat-app `lib.rs:2` says "No ... platform host dependency belongs here".

**Impact:** The docs mislead readers. Under the workspace rule, stale copy is a bug.

**Suggested action:**
1. Move the misplaced doc onto `OWNER_ENDED_MESSAGE`.
2. Rewrite the desktop Cargo description and header comment to match what the crate does now.
3. Clarify chat-app's dependency rule.

### APP-20 Redundant and inconsistent cfg gates in the desktop binary

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:** [main.rs:23](../../../../crates/openagents-desktop/src/main.rs), [main.rs:199](../../../../crates/openagents-desktop/src/main.rs)

**Evidence:** Per the reviewer, `feature = "app"` gates sit inside a bin that already has `required-features = ["app"]`. Not re-measured.

**Impact:** Conditional branches that can never be taken.

**Suggested action:**
1. Remove the redundant `feature = "app"` gates.
2. Use `target_os = "macos"` consistently.
3. Verify: the bin builds on macOS and in CI with unchanged behavior.

### APP-21 Browser-opening code duplicated and spawned without reaping the child

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:** [chat.rs:4356](../../../../crates/openagents-desktop/src/chat.rs), [update.rs:1462](../../../../crates/openagents-desktop/src/update.rs)

**Evidence:** Per the reviewer, `open`/`xdg-open`/`rundll32` are spawned with `let _ = spawn()` in two files. Not re-measured.

**Impact:** Short-lived zombie processes and duplicated platform logic.

**Suggested action:**
1. Add one `open_url` helper that waits on the child off-thread.
2. Replace both call sites with it.

### APP-22 macOS window process gets allow-unsigned-executable-memory and network.server

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:** [OpenAgents.entitlements:19](../../../../bins/openagents-desktop-macos/OpenAgents.entitlements), [OpenAgents.entitlements:23](../../../../bins/openagents-desktop-macos/OpenAgents.entitlements), [host.entitlements](../../../../bins/openagents-desktop-macos/host.entitlements)

**Evidence:** The two entitlement files differ only in the line 4 comment. Both grant `allow-unsigned-executable-memory` and `network.server`. The desktop's `Cargo.toml:64-65` shows the window links `coder` (the `coder::task::local` lane), so the window may legitimately run plugins.

**Impact:** The W^X relaxation may be broader than the UI process needs.

**Suggested action:**
1. Determine whether the window process runs wasmtime through the coder lane.
2. If it does not, drop the two keys from `OpenAgents.entitlements`.
3. Either way, generate both files from one template in `bundle.sh`.
4. Verify: `codesign -d --entitlements -` on the built window binary shows the intended set, and the app launches and runs a Coder task.

### APP-23 home() falls back to `/` when HOME and USERPROFILE are unset

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:** [main.rs:175](../../../../crates/openagents-desktop/src/main.rs)

**Evidence:** `.map_or_else(|| PathBuf::from("/"), PathBuf::from)`.

**Impact:** In a stripped environment the app reads and writes state under `/`.

**Suggested action:**
1. Use `std::env::home_dir`.
2. Exit with a clear error when it returns `None`.
3. Verify: `env -i` launch prints the error instead of touching `/`.

### APP-24 Leftover noise in chat-app's lib.rs: unused re-export shim and a fixture borrowed from the mobile crate

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:** [lib.rs:3](../../../../crates/openagents-chat-app/src/lib.rs), [lib.rs:34](../../../../crates/openagents-chat-app/src/lib.rs)

**Evidence:** `pub use openagents_chat::{basic_chats, basic_coder, router};` has no users of `openagents_chat_app::{basic_chats,basic_coder,router}` outside the crate. Line 34 has `include_str!("../../openagents-mobile/fixtures/gym-report.json")`.

**Impact:** Minor confusion, and the fixture couples chat-app to the mobile crate.

**Suggested action:**
1. Use `openagents_chat::` paths directly and delete the `pub` re-export.
2. Copy the fixture into `crates/openagents-chat-app/fixtures/`.
3. Verify: the workspace builds and the chat-app tests pass.

### APP-25 Many sleep-based waits in UI and acceptance tests

**Severity:** Low · **Category:** testing · **Effort:** M

**Locations:** [shell.rs:2144](../../../../crates/openagents-desktop/src/shell.rs), [host_threads.rs:1551](../../../../crates/openagents-chat-app/src/host_threads.rs)

**Evidence:** Per the reviewer, tests contain thread or tokio sleeps: desktop 48, chat-app 15, chat 16. Not re-counted.

**Impact:** Tests can flake on loaded hosts.

**Suggested action:**
1. Add a `wait_until(deadline, predicate)` helper and replace fixed sleeps with it.
2. Use `tokio::time::pause` in the async tests.
3. Verify: the sleep count drops and the suites pass repeatedly under load (for example, with `--test-threads` raised).

## Refuted during verification

- **Phone follow threads pile up on repeated taps / re-read the whole thread** (partial). `follow()` reads only the newest page (`link.read(host, thread, None)`), and each loop returns when `open.generation` changes (`host_threads.rs:1126`), so follow threads do not accumulate. APP-11 keeps only the polling cost, at low severity.
- **Use `atif::document::Step` directly in `coder_events`** (partial). That struct lacks `step_id` and `extra`/`extensions`, and `step()` accepts both the document and the log record form, so a drop-in typed struct would not work. APP-10 keeps the finding with a corrected action.
- **Count corrections.** `Panel` has 69 single-line fields (not 71), `self.rename` appears on 42 lines (not 58), there are 14 `#[path]` attributes (not 15), and openagents-chat declares 23 modules (not 24).
- **Download slide "silently shows an empty version"** (APP-04). This is overstated: `slide_embeds.rs:1078` has a test that asserts the version starts with 1.0.0 and that the sh command is well-formed.
