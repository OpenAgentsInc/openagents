# Coder surfaces & terminal crates

Scope: `crates/{coder-computers, coder-compositor, coder-ui, coder-pty, coder-terminal, coder-vt, coder-mobile, coder-mobile-probe, coder-wm, coder-binds, coder-browser, coder-browser-web, coder-chat-web, coder-components-web, coder-demo-ui, coder-cloud-web, coderos-camera, coder-desk, coder-desk-cli, coder-engine-status}`. Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

**Health grade: C**

This area has 20 crates and about 129k lines of Rust. The lower layers are solid. coder-pty, coder-vt, coder-desk, coder-binds, coder-wm and coderos-camera have thorough module docs and few unwraps in production code (under 25 per crate outside tests, mostly invariant expects). Their unsafe blocks carry SAFETY comments, their inputs are bounded, and they have real tests (about 1,060 `#[test]` functions plus tokio tests). coder-binds has a drift test that keeps its Rust bind table equal to `os/modules/coderos/desktop.nix`.

The upper layers are weaker:

- **Dead Cloud web code is still built and shipped.** The 2026-10-08 Cloud web reset (cb11bee283) left two wasm crates (coder-cloud-web, coder-browser-web) and about 2,400 lines of coder-ui views with no consumer. The production Docker image still builds both wasm crates and copies them in, but nothing serves them.
- **One file is too large.** `coder-mobile/src/verse_app.rs` has 6,685 lines and is the area's most-changed file (76 changes in the last month). The desktop app depends on the "mobile" crate to get the Verse Grid.
- **coder-ui is a heavy dependency.** Ten Verse crates link all of coder-ui only to use one four-value enum.
- **The Everglade web terminal drops typing.** The live browser terminal workbench allows one request at a time and discards keystrokes typed while a request is in flight.
- **Code is duplicated across crates.** There are two syntax-highlight stacks, two markdown renderers, two spinners, about 22 `write_private` helpers, and about 33 `#[path]` includes of another crate's test support.
- **The Linux-only desktop crates have no automated tests.** No CI or nix check runs the tests for compositor, camera or desk.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (git ls-files `*.rs`) | ~128,990 total. coder-mobile 19,865; coder-computers 18,801; coder-ui 15,501; coder-compositor 14,935; coder-pty 13,327; coder-terminal 11,163; coder-vt 8,326; coder-desk 5,318; coder-demo-ui 2,884; coderos-camera 2,815; coder-browser 2,667; coder-binds 2,535; coder-browser-web 2,453; coder-wm 1,927; coder-components-web 1,813; coder-engine-status 1,184; coder-chat-web 990; coder-desk-cli 985; coder-cloud-web 845; coder-mobile-probe 652 |
| `#[test]` count | ~1,062 total. compositor 206, terminal 176, mobile 140, vt 101, pty 95, computers 92, desk 44, ui 43, wm 37, camera 34, binds 27, browser 19, desk-cli 16, engine-status 14, demo-ui 6, cloud-web 6, browser-web 2, components-web 2, mobile-probe 2, chat-web 0 |
| Largest files | coder-mobile/src/verse_app.rs 6,685 (`impl Scene` at lines 968-4201, inline tests at 4221-6685); coder-ui/src/catalog/settings.rs 4,069; coder-pty/src/host/mod.rs 2,752; coder-computers/src/live.rs 2,336; coder-pty/src/ext.rs 2,132; coder-browser-web/src/mount.rs 2,096; coder-ui/src/catalog/conversation.rs 2,037 |
| unwrap/expect, all code | mobile 1,072 (408 in verse_app.rs, almost all in tests), computers 467, pty 273, vt 140 |
| unwrap/expect, production code | Highest in any one file: 23, in coder-browser/src/workbench.rs, all `lock().unwrap()` |
| `#[allow]` | compositor 8 (dead_code on held Wayland globals); computers 6 (5 are dead_code on `#[path]` test modules) |
| TODO/FIXME | 0 real ones |
| Workspace lints | All 20 crates opt in (`[lints] workspace = true`) |
| 30-day churn | verse_app.rs 76, verse_ffi.rs 37, computers/live.rs 23 |
| No reverse dependency | compositor, mobile-probe, browser-web, chat-web, components-web, cloud-web, coderos-camera, desk-cli (bins or wasm cdylibs) |
| Most dependents | coder-ui 19, coder-pty 15, coder-vt 14 |

## Strengths

- coder-pty's host module ([host/mod.rs](../../../../crates/coder-pty/src/host/mod.rs), lines 1-90) is a good example of contract documentation. It spells out process ownership, output backpressure, the typist, authority, shares and lifetime. Nearly every unsafe block in `host/windows.rs` and `host/sys.rs` has a SAFETY comment (33 unsafe vs 32 SAFETY, and 16 vs 15).
- There is only one VT emulator: coder-vt, built on vte. No other crate implements `vte::Perform`.
- Production code rarely panics. Mutex poisoning is handled explicitly in coder-pty and coder-vt (`PoisonError::into_inner`). The FFI entry points in `coder-mobile/src/ffi.rs` and `verse_ffi.rs` wrap their work in `catch_unwind`. The Android bridge refuses calls made on the wrong thread (`android.rs:45` `main_thread`).
- The config/code drift test [coder-binds/tests/binds.rs](../../../../crates/coder-binds/tests/binds.rs) fails when a Hyprland bind or windowrule in `desktop.nix` differs from its Rust row.
- Browser HTML is escaped, and the escaping is tested. `coder-demo-ui/src/html.rs` uses `svg::escape`, with a test at line 119. coder-chat-web zeroizes drafts and turns off htmx eval and script tags.
- Manifests explain each dependency in a comment. Workspace lints deny `dbg!`, `todo!` and `unimplemented!`.
- The copy guard against machine talk (#11031) is enforced by tests in coder-ui and coder-mobile (`copy_guard_tests.rs`).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| CS-01 | High | dead-code | Dead Cloud web wasm crates and coder-ui views are still built into the production image | M |
| CS-02 | High | correctness | Browser terminal workbench drops keystrokes typed while a request is in flight | M |
| CS-03 | Medium | architecture | Ten Verse crates and terminal-gfx link all of coder-ui only for the `Intensity` enum | S |
| CS-04 | Medium | maintainability | verse_app.rs is a 6,685-line god file, and desktop depends on the "mobile" crate | L |
| CS-05 | Medium | maintainability | catalog/settings.rs is a 4,069-line stringly-typed fixture reducer in the shared UI library | L |
| CS-06 | Medium | duplication | Two syntax-highlight stacks and two markdown renderers; `../../../` asset includes | M |
| CS-07 | Medium | testing | Test support is shared through ~33 `#[path]` includes across 12 crates | S |
| CS-08 | Medium | duplication | `write_private` is reimplemented ~22 times; the coder-computers copy has a fixed tmp name and no directory fsync | M |
| CS-09 | Medium | build | Linux desktop crates have no automated test path, and smithay deps are not target-gated | M |
| CS-10 | Medium | policy | coder-terminal ports Apache-2.0 grok-build code but has no NOTICE | S |
| CS-11 | Low | build | Wasm build scripts hard-code per-agent target dirs and mix debug and release | S |
| CS-12 | Low | maintainability | coder-pty host/mod.rs and coder-computers live.rs each put several protocols in one module | M |
| CS-13 | Low | dead-code | coder-mobile-probe is a leftover feasibility prototype with unsafe FFI and no consumers | S |
| CS-14 | Low | duplication | Two spinner implementations and a re-export shim in coder-terminal | S |
| CS-15 | Low | docs | Stale docs and dead routes left behind by the web and mobile pivots | S |
| CS-16 | Low | error-handling | Camera fanout counts a dead sink as "dropped" | S |
| CS-17 | Low | maintainability | Compositor gap and border constants are copied from desktop.nix by hand, with no drift test | S |
| CS-18 | Low | concurrency | coder-browser panics on a poisoned mutex while sibling crates recover | S |

### CS-01 Dead Cloud web wasm crates and coder-ui views are still built into the production image after the /cloud/app reset

**Severity:** High · **Category:** dead-code · **Effort:** M

**Locations:**
- [openagents-web/README.md](../../../../crates/openagents-web/README.md) README.md:179
- [openagents-web/Dockerfile](../../../../crates/openagents-web/Dockerfile) Dockerfile:50-72, Dockerfile:92, Dockerfile:99
- [coder-browser-web/src/mount.rs](../../../../crates/coder-browser-web/src/mount.rs)
- [coder-ui/src/coordination.rs](../../../../crates/coder-ui/src/coordination.rs), [observation.rs](../../../../crates/coder-ui/src/observation.rs), [control.rs](../../../../crates/coder-ui/src/control.rs), [workspace.rs](../../../../crates/coder-ui/src/workspace.rs)
- [coder-chat-web/src/browser.rs](../../../../crates/coder-chat-web/src/browser.rs) browser.rs:153, browser.rs:226
- [coder-chat-web/README.md](../../../../crates/coder-chat-web/README.md) README.md:32-36

**Evidence:**
- README.md:179 says `--cloud-build DIRECTORY` is "Accepted and unused since the old Cloud pages left; the site images still pass it."
- Dockerfile:53-72 still compiles coder-cloud-web and coder-browser-web for wasm32 in release mode, runs wasm-bindgen on both, and checks their output with `test -s`.
- Dockerfile:92 copies `/build/cloud` into the image, and the CMD at line 99 passes `--cloud-build /srv/cloud`.
- Outside the two wasm crates, `git grep 'cloud-private|data-cloud-privacy-ready'` finds only coder-chat-web/src/browser.rs:153 and :226, its README, and a dated bench script (`bench/web/2026-10-08-web17/browser.py`).
- `coder_ui::coordination` (773 lines) and `coder_ui::observation` (589 lines) have no consumers outside coder-ui. `coder_ui::control` (798 lines) and `coder_ui::workspace` (208 lines) are used only by coder-browser-web/src/mount.rs.

**Impact:** Several thousand lines are maintained, built and shipped for nothing. Every production image build pays for two extra wasm release compiles. Readers are misled about which privacy boundary is live.

**Suggested action:**
1. Delete `crates/coder-cloud-web` and `crates/coder-browser-web`, or move them to backroom.
2. In the Dockerfile, remove both crates from the cargo build, wasm-bindgen and `test -s` lines (53-72) and from the COPY at line 92. Delete `scripts/build-coder-{cloud,browser}-web.sh`.
3. Drop `--cloud-build` from the openagents-web CLI and from the CMD at Dockerfile:99.
4. Delete `coder-ui/src/{coordination,observation,control,workspace}.rs` and their `pub mod` lines in `coder-ui/src/lib.rs`.
5. Remove the `#cloud-private` / `openagents-cloud-retired` branches from coder-chat-web/src/browser.rs, and the privacy paragraph at coder-chat-web/README.md:32-36.
6. Verify:
   - `cargo metadata --no-deps` succeeds.
   - `git grep -n 'cloud-private\|coder_cloud_web\|coder_browser_web' -- crates scripts` returns nothing.
   - A Docker build of openagents-web succeeds.

### CS-02 Browser terminal workbench allows one request in flight and drops keystrokes typed while it waits

**Severity:** High · **Category:** correctness · **Effort:** M

**Locations:**
- [coder-browser/src/workbench.rs](../../../../crates/coder-browser/src/workbench.rs) workbench.rs:40-62, workbench.rs:73-83, workbench.rs:238-241, workbench.rs:389-394
- [everglade-web/src/terminal.rs](../../../../crates/everglade-web/src/terminal.rs) terminal.rs:153-171, terminal.rs:274-293

**Evidence:**
- `IO::send` returns false whenever `self.outbound.is_some()` or `self.state != State::Ready`. On success it sets `state = State::Unknown`.
- `can_type()` requires `io.state == State::Ready`, so `Workbench::key` returns false while a request is in flight.
- `Remote::input` ignores the bool returned by `io.send(TermRequest::Input(input))`.
- In everglade-web/src/terminal.rs:162-170, the loop awaits `link.request(request)` before it polls `link.next()` again.
- The keydown handler calls `model.key(...)` and returns `true` (event consumed) whether or not the key was accepted.

**Impact:** In the Everglade web terminal, keys typed during a round trip are silently discarded. On a high-latency route, most fast typing is lost.

**Suggested action:**
1. Replace `outbound: Option<TermRequest>` with a bounded `VecDeque<TermRequest>`.
2. In `IO::send`, coalesce consecutive `Input` requests for the same terminal instead of refusing them: concatenate the bytes, capped at the frame size.
3. Gate `can_type()` only on typist, interactive and snapshot state, not on whether a request is in flight.
4. Make `dispatch()` pop the front of the queue.
5. In everglade-web/src/terminal.rs, select over `link.request` and `link.next` so output keeps flowing while a request is pending.
6. Verify with a new coder-browser test that types `a`, `b`, `c` before any result arrives and asserts that the dispatched Input bytes equal `"abc"`.

### CS-03 Ten Verse crates and terminal-gfx link all of coder-ui (syntect, two-face, pulldown-cmark, nostr, atif) only for the Intensity enum

**Severity:** Medium · **Category:** architecture · **Effort:** S

**Locations:**
- [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) Cargo.toml:93
- [verse-core/Cargo.toml](../../../../crates/verse-core/Cargo.toml) Cargo.toml:11
- [verse-gfx/Cargo.toml](../../../../crates/verse-gfx/Cargo.toml) Cargo.toml:15
- [verse-pbr/Cargo.toml](../../../../crates/verse-pbr/Cargo.toml) Cargo.toml:21
- [verse-net/Cargo.toml](../../../../crates/verse-net/Cargo.toml) Cargo.toml:16
- [verse-zone-everglade/Cargo.toml](../../../../crates/verse-zone-everglade/Cargo.toml) Cargo.toml:36
- [verse-zone-grove/Cargo.toml](../../../../crates/verse-zone-grove/Cargo.toml) Cargo.toml:10
- [verse-zone-lab/Cargo.toml](../../../../crates/verse-zone-lab/Cargo.toml) Cargo.toml:10
- [verse-gym/Cargo.toml](../../../../crates/verse-gym/Cargo.toml) Cargo.toml:10
- [terminal-gfx/Cargo.toml](../../../../crates/terminal-gfx/Cargo.toml) Cargo.toml:21
- [coder-ui/src/theme.rs](../../../../crates/coder-ui/src/theme.rs) theme.rs:3-60
- [coder-terminal/src/intensity.rs](../../../../crates/coder-terminal/src/intensity.rs)

**Evidence:**
- In each Verse crate, `git grep coder_ui::` finds only `coder_ui::theme::Intensity`. verse-gfx also uses `NEAR_BLACK`.
- verse-zone-grove declares coder-ui as a dependency but uses nothing from it.
- terminal-gfx also uses `coder_ui::coder_noir`, which is a re-export of `oa_tokens::noir`.
- coder-ui's `[dependencies]` pull in atif, nostr, syntect, two-face (syntect-fancy), pulldown-cmark, url and zeroize. theme.rs itself depends only on serde and `coder_noir`.

**Impact:** Verse, Everglade and terminal-gfx builds (including wasm and mobile) compile grammar, markdown and nostr stacks they never use. Any edit to a coder-ui fixture invalidates their build cache. The layering is also inverted: the world engine depends on an application's presentation crate.

**Suggested action:**
1. Move `Intensity`, `NEAR_BLACK` and `NEAR_BLACK_TINT` from coder-ui/src/theme.rs into oa-tokens. Re-export them from `coder_ui::theme` so existing callers keep compiling.
2. Point verse, verse-core, verse-gfx, verse-pbr, verse-net, verse-zone-everglade, verse-zone-lab, verse-gym and terminal-gfx at oa-tokens.
3. In terminal-gfx, use `oa_tokens::noir` instead of `coder_noir`.
4. Delete the coder-ui dependency from verse-zone-grove.
5. Verify:
   - `git grep -n coder_ui -- 'crates/verse*' crates/terminal-gfx` finds only doc comments.
   - One Verse crate still builds.

### CS-04 coder-mobile/src/verse_app.rs is a 6,685-line god file with the highest churn, and desktop depends on the 'mobile' crate for the Verse Grid

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [coder-mobile/src/verse_app.rs](../../../../crates/coder-mobile/src/verse_app.rs) verse_app.rs:968, verse_app.rs:4221
- [coder-mobile/src/lib.rs](../../../../crates/coder-mobile/src/lib.rs) lib.rs:1-2
- [coder-mobile/Cargo.toml](../../../../crates/coder-mobile/Cargo.toml) Cargo.toml:7
- [openagents-desktop/Cargo.toml](../../../../crates/openagents-desktop/Cargo.toml) Cargo.toml:35, Cargo.toml:117
- [openagents-desktop/src/grid.rs](../../../../crates/openagents-desktop/src/grid.rs) grid.rs:9

**Evidence:**
- The file has 6,685 lines. `impl Scene` starts at line 968. The test module starts at line 4221, after 7 `#[cfg(test)]` module includes.
- The file has had 76 commits since 2026-09-10.
- lib.rs:1 says "Coder's read-only mobile application", and Cargo.toml:7 says "Rust-owned read-only Coder mobile conversation state".
- openagents-desktop has an optional dependency on coder-mobile (Cargo.toml:117, enabled by a feature at line 35) so that grid.rs and grid_fixtures.rs can use `coder_mobile::verse_surface` and `BareGym`.

**Impact:** Merge conflicts and review load pile up in one file. Phone-only FFI code is mixed with world logic that the desktop also uses. The crate's name and docs no longer describe what it does.

**Suggested action:**
1. Extract a `verse-app` crate that holds Scene, Config, Request, verse_surface.rs, chamber.rs, computer_hud.rs and studio_panel.rs. Leave the reader (app.rs, render.rs, push.rs) and the FFI shims in coder-mobile.
2. Split `Scene::action` into per-domain submodules (`verse_app/{input,chamber,gym,hud}.rs`), and move the inline tests to `verse_app/tests.rs`.
3. Point openagents-desktop's optional dependency at verse-app.
4. Fix the "read-only" wording in Cargo.toml:7 and lib.rs:1.
5. Verify that the `#[test]` count is unchanged after the move.

### CS-05 coder-ui catalog/settings.rs: a 4,069-line stringly-typed fixture reducer with 4 tests, shipped in the shared UI library

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [coder-ui/src/catalog/settings.rs](../../../../crates/coder-ui/src/catalog/settings.rs) settings.rs:1, settings.rs:14-16, settings.rs:29, settings.rs:743, settings.rs:801, settings.rs:964

**Evidence:**
- Line 1 reads "Inert Coder settings, selection, and disclosure presentation fixtures."
- The file hard-codes `ROUTER_ENDPOINT`, `TYPESAFE_ENDPOINT` and `VERCEL_ENDPOINT` (lines 14-16) and keeps its own `PLUGINS: [Plugin; 8]` table (line 29).
- State is kept under string keys, for example `put(state, "endpoint", ...)` at lines 743 and 801. `fn owns(id: &str)` at line 964 matches string IDs.
- The file has 4 `#[test]` functions.
- Outside coder-ui, `coder_ui::catalog` is used only by coder-components-web and openagents-web.

**Impact:** A typo in a field or component ID fails silently. The provider and plugin tables can drift from the real ones. Fixture code is compiled into every crate that depends on coder-ui.

**Suggested action:**
1. Move `catalog/` (and `demo/`) into a `coder-ui-catalog` crate. Only coder-components-web, openagents-web and any demo consumers should depend on it.
2. Replace the string field and component IDs with enums that serde-rename to the existing wire strings.
3. Take the plugin and endpoint tables from the crate that owns them.
4. Add one reducer test per component.
5. Verify that the coder-ui dependents still build and that coder-ui no longer contains `catalog`.

### CS-06 Two syntax-highlight stacks and two markdown renderers; coder-ui reads another crate's assets through ../../../ paths

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-ui/src/components/syntax.rs](../../../../crates/coder-ui/src/components/syntax.rs) syntax.rs:12-28
- [coder-ui/src/components/markdown.rs](../../../../crates/coder-ui/src/components/markdown.rs) markdown.rs:1-3
- [coder-terminal/src/markdown.rs](../../../../crates/coder-terminal/src/markdown.rs)
- [code-highlight/src/grok/syntax.rs](../../../../crates/code-highlight/src/grok/syntax.rs) syntax.rs:96-99
- [code-highlight/src/grok/theme.rs](../../../../crates/code-highlight/src/grok/theme.rs) theme.rs:47-51

**Evidence:**
- coder-ui's `syntax()` builds its own `two_face::syntax::extra_newlines()` set and adds a Swift grammar.
- It loads the theme with `include_bytes!("../../../code-highlight/assets/grok-build/grok-night.tmTheme")` and the Swift grammar with `include_str!` of `Swift.sublime-syntax`, both from inside the code-highlight crate.
- code-highlight's `syntax_set()` returns `SYNTAX_SET.get_or_init(load_syntax_set).clone()`, and `Palette::syntect` builds separate Night and Day instances.
- coder-ui/src/components/markdown.rs:1 says it was "reimplemented from the public terminal renderer". coder-terminal/src/markdown.rs is 1,682 lines.
- Both crates declare pulldown-cmark 0.13.4.

**Impact:** A process that links both crates holds several SyntaxSets in memory. Grammar and theme fixes reach only one of the two paths. The include paths break silently if code-highlight moves.

**Suggested action:**
1. Make coder-ui depend on code-highlight, and build components/syntax.rs on code-highlight's SyntaxSet.
2. Change code-highlight's `syntax_set()` to return `&'static SyntaxSet` instead of a clone.
3. Move the parse-to-blocks half of coder-terminal/src/markdown.rs into a renderer-neutral module that both renderers use.
4. Verify:
   - `git grep -n '\.\./\.\./\.\./code-highlight' crates` returns nothing.
   - The coder-terminal snapshot tests pass.

### CS-07 Test support is shared by #[path] includes of other crates' files (about 33 Rust sites across 12 crates), each needing #[allow(dead_code)]

**Severity:** Medium · **Category:** testing · **Effort:** S

**Locations:**
- [coder-computers/tests/connect_live.rs](../../../../crates/coder-computers/tests/connect_live.rs) connect_live.rs:10-12
- [coder-host/tests/serve.rs](../../../../crates/coder-host/tests/serve.rs)
- [coder/tests/host_cli.rs](../../../../crates/coder/tests/host_cli.rs)
- [openagents-cli/tests/agent_crew.rs](../../../../crates/openagents-cli/tests/agent_crew.rs)
- [terminal-remote/tests/remote.rs](../../../../crates/terminal-remote/tests/remote.rs)
- [verse/tests/studio_host.rs](../../../../crates/verse/tests/studio_host.rs)
- [coder-mobile/src/connection_tests.rs](../../../../crates/coder-mobile/src/connection_tests.rs)

**Evidence:**
- `git grep -l 'coder-control/src/tests/relay.rs'` matches 36 files: 33 Rust files and 3 verification docs.
- The Rust files are spread across coder-host (10), coder (7), coder-computers (5), coder-access (2), coder-connect (2), openagents-cli (2), terminal-remote (2), chat-load-bench, coder-mobile, gym-bridge and verse.
- `coder-ssh/tests/support/fake_ssh.rs` is referenced from 6 more files.
- Example, connect_live.rs:10-12: `#[path = "../../coder-control/src/tests/relay.rs"] #[allow(dead_code)] mod relay;`

**Impact:** Moving relay.rs breaks tests in a dozen crates. Every test binary recompiles the support code, and the `dead_code` allows hide drift.

**Suggested action:**
1. Create `crates/coder-test-support` (`publish = false`) that exports `relay` and `fake_ssh` as pub modules.
2. Add it as a dev-dependency of each crate that includes these files. chat-load-bench includes relay.rs from `src/relay.rs`, which is not test code, so it needs a normal dependency.
3. Replace each `#[path] mod relay;` with `use coder_test_support::relay;`, and drop the `#[allow(dead_code)]` attributes.
4. Verify that `git grep -n 'coder-control/src/tests/relay.rs' -- '*.rs'` returns nothing, and that the affected crates' tests pass.

### CS-08 write_private is reimplemented about 22 times; the coder-computers copy uses a fixed .tmp name and does not fsync the directory

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-computers/src/live.rs](../../../../crates/coder-computers/src/live.rs) live.rs:292-309
- [coder-cloud/src/workspace.rs](../../../../crates/coder-cloud/src/workspace.rs) workspace.rs:285
- [coder-host/src/serve/mod.rs](../../../../crates/coder-host/src/serve/mod.rs) mod.rs:1248
- [coder-sync/src/lib.rs](../../../../crates/coder-sync/src/lib.rs) lib.rs:118
- [model-access/src/store.rs](../../../../crates/model-access/src/store.rs) store.rs:148
- [openagents-login/src/lib.rs](../../../../crates/openagents-login/src/lib.rs) lib.rs:204
- [verse-net/src/identity.rs](../../../../crates/verse-net/src/identity.rs) identity.rs:159
- [wallet/src/config.rs](../../../../crates/wallet/src/config.rs) config.rs:344
- [spark-wallet/src/store.rs](../../../../crates/spark-wallet/src/store.rs) store.rs:139

**Evidence:**
- Outside tests, `git grep 'fn write_private'` finds about 22 copies that write files. They are in coder-cloud, coder-computers, coder-host, coder-new, coder-sync, coder (2), gym, inference, microcoder, model-access, openagents-chat, openagents-cli (3), openagents-login, openagents-mobile, openagents-web, spark-wallet, verse-net, verse-private, verse-zone-everglade and wallet. The retail-cloud matches are an unrelated trait method and are excluded.
- The coder-computers copy writes to `path.with_extension("tmp")` with `.mode(0o600)`. The mode applies only when the file is newly created.
- It fsyncs the file but not the parent directory after the rename.
- Concurrent writers share the same tmp name.

**Impact:** Durability and permission fixes do not reach the other copies. Concurrent writers to the same record can collide on the tmp file. A leftover tmp file with mode 0644 is an edge case: an interrupted earlier write would itself have created the file as 0600.

**Suggested action:**
1. Add one `write_private(path, bytes)` to a small shared crate. It should:
   - create a uniquely named temp file in the same directory (`create_new`, mode 0600);
   - write and fsync it;
   - rename it over the target and fsync the parent directory;
   - set permissions to 0600 explicitly.
2. Replace the copies, starting with the ones that hold key material: coder-computers/src/live.rs, verse-net/src/identity.rs, wallet/src/config.rs and spark-wallet/src/store.rs.
3. Add a unix test that pre-creates `<path>.tmp` with mode 0644 and asserts that the final file is 0600.
4. Verify with `git grep -n 'fn write_private'`; only the shared implementation should remain.

### CS-09 No automated test path for the Linux desktop crates; smithay system-library deps are not gated by target

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [coder-compositor/Cargo.toml](../../../../crates/coder-compositor/Cargo.toml) Cargo.toml:43-66
- [os/pkgs/coder-compositor.nix](../../../../os/pkgs/coder-compositor.nix) coder-compositor.nix:82
- [os/pkgs/coder-desk.nix](../../../../os/pkgs/coder-desk.nix) coder-desk.nix:63
- [os/pkgs/coderos-camera.nix](../../../../os/pkgs/coderos-camera.nix) coderos-camera.nix:95
- [Cargo.toml](../../../../Cargo.toml) Cargo.toml:2-3

**Evidence:**
- `.github/` contains only ISSUE_TEMPLATE, so there are no CI workflows.
- All three nix packages set `doCheck = false` and build with `--ignore-rust-version`.
- smithay, with backend_udev, backend_libinput, backend_session_libseat and backend_gbm, is in plain `[dependencies]` rather than a Linux-only target section.
- The workspace sets `members = ["crates/coderbench","crates/*"]` and has no `default-members`.

**Impact:** Compositor, camera and desk regressions ship untested. Workspace-wide cargo commands are fragile on macOS.

**Suggested action:**
1. Either move smithay and the other Linux system dependencies under `[target.'cfg(target_os = "linux")'.dependencies]` with a stub main on other targets, or add a `default-members` list that excludes the Linux-only bins.
2. Add a Linux check that runs `cargo test -p coder-compositor -p coderos-camera -p coder-desk -p coder-desk-cli`. This can be a `nix flake check` entry, or `doCheck = true` with checkFlags that skip socket and spawn tests.
3. Verify that the check runs and fails on a deliberately broken test.

### CS-10 coder-terminal ports Apache-2.0 grok-build code but has no NOTICE, though coder-ui's NOTICE points to it

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [coder-terminal/src/markdown.rs](../../../../crates/coder-terminal/src/markdown.rs) markdown.rs:1-8
- [coder-terminal/src/grok_spinner.rs](../../../../crates/coder-terminal/src/grok_spinner.rs) grok_spinner.rs:1
- [coder-terminal/src/components/diff.rs](../../../../crates/coder-terminal/src/components/diff.rs), [turn.rs](../../../../crates/coder-terminal/src/components/turn.rs), [rail.rs](../../../../crates/coder-terminal/src/components/rail.rs), [run.rs](../../../../crates/coder-terminal/src/components/run.rs)
- [coder-ui/NOTICE](../../../../crates/coder-ui/NOTICE)

**Evidence:**
- Apart from `.rs` and snapshot files, `git ls-files crates/coder-terminal` lists only Cargo.toml. There is no NOTICE file.
- coder-ui/NOTICE says the attribution is "recorded in coder-terminal".
- `git grep -l grok-build crates/coder-terminal/src` lists markdown.rs, grok_spinner.rs and components/{diff,mod,rail,run,turn}.rs.

**Impact:** The crate with the largest Apache-2.0 port has no NOTICE file, and another crate's NOTICE refers to one that does not exist.

**Suggested action:**
1. Add `crates/coder-terminal/NOTICE`, modeled on `crates/code-highlight/NOTICE`. List each ported file with its upstream path and the Apache-2.0 attribution.
2. Verify that every file returned by `git grep -l grok-build crates/coder-terminal/src` appears in the NOTICE.

### CS-11 Wasm build scripts hard-code per-agent target dirs and build debug or release inconsistently

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [scripts/build-coder-browser-web.sh](../../../../scripts/build-coder-browser-web.sh) build-coder-browser-web.sh:11, :24
- [scripts/build-coder-cloud-web.sh](../../../../scripts/build-coder-cloud-web.sh) build-coder-cloud-web.sh:11
- [scripts/build-coder-components-web.sh](../../../../scripts/build-coder-components-web.sh) build-coder-components-web.sh:11, :24
- [scripts/build-coder-chat-web.sh](../../../../scripts/build-coder-chat-web.sh) build-coder-chat-web.sh:10, :18-21

**Evidence:**
- The default target dirs are `$HOME/work/openagents-target-agent3` (browser-web, cloud-web) and `openagents-target-agent0` (components-web, chat-web). Six scripts embed `openagents-target-agent`.
- chat-web builds with `--release`. The other three scripts read `debug/*.wasm`.

**Impact:** Local dev builds depend on one machine's directory layout. Three of the four local builds differ in size and behavior from the release builds in the Dockerfile.

**Suggested action:**
1. Replace the scripts with one `scripts/build-coder-web.sh <crate> [--debug]` that defaults `CARGO_TARGET_DIR` to `$PWD/target` and builds release by default.
2. Delete the browser and cloud scripts as part of CS-01.
3. Verify that `git grep -n openagents-target-agent -- scripts` returns nothing.

### CS-12 coder-pty host/mod.rs (2,752 lines) and coder-computers live.rs (2,336) concentrate several protocols in one module

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [coder-pty/src/host/mod.rs](../../../../crates/coder-pty/src/host/mod.rs) mod.rs:1-90
- [coder-computers/src/live.rs](../../../../crates/coder-computers/src/live.rs) live.rs:1-36, live.rs:292

**Evidence:** The files have 2,752 and 2,336 lines. They had 17 and 23 commits respectively since 2026-09-10. host/mod.rs is well documented, and its tests start around line 2700, so nearly all 2,752 lines are production code.

**Impact:** The files are large and contain security-sensitive authority logic, which makes review harder.

**Suggested action:**
1. Split host/mod.rs into `typist`, `shares`, `attach` and `lifetime` submodules.
2. Split live.rs into `store`, `service` and `pump` submodules.
3. Treat both as pure moves, and verify that test counts are unchanged.

### CS-13 coder-mobile-probe is a retained feasibility prototype with Keychain/Keystore FFI and no consumers

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:**
- [coder-mobile-probe/src/lib.rs](../../../../crates/coder-mobile-probe/src/lib.rs) lib.rs:1-4
- [coder-mobile-probe/src/ios_keychain.rs](../../../../crates/coder-mobile-probe/src/ios_keychain.rs)
- [coder-mobile-probe/src/android.rs](../../../../crates/coder-mobile-probe/src/android.rs)
- [docs/coder/rust-native/adoption.md](../../../../docs/coder/rust-native/adoption.md) adoption.md:52-53

**Evidence:**
- Only the crate's own Cargo.toml names it.
- It has 2 commits and 16 `unsafe` occurrences (ios 10, android 4, ios_keychain 2).
- adoption.md:53 says to "Retain their historical feasibility evidence separately from the later iOS and Android readers."

**Impact:** Workspace builds compile unsafe platform code that has no product use.

**Suggested action:**
1. Move the crate to backroom. The doc deliberately keeps this evidence, so archive the crate rather than delete it.
2. Repoint rows 52-53 of adoption.md at the archive.
3. Verify that `cargo metadata --no-deps` no longer lists coder-mobile-probe.

### CS-14 Two spinner implementations with different frame sets, plus a compatibility re-export shim in coder-terminal

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [coder-terminal/src/spinner.rs](../../../../crates/coder-terminal/src/spinner.rs) spinner.rs:10-43
- [coder-terminal/src/grok_spinner.rs](../../../../crates/coder-terminal/src/grok_spinner.rs) grok_spinner.rs:28-42
- [coder-terminal/src/intensity.rs](../../../../crates/coder-terminal/src/intensity.rs)
- [openagents-terminal/src/draw.rs](../../../../crates/openagents-terminal/src/draw.rs) draw.rs:26-31

**Evidence:**
- spinner.rs has 10 braille `FRAMES` and `CYCLE = 500` ms.
- grok_spinner.rs has 8 `FRAMES`, `TICK = 1000/30` ms and `DIVISOR` 4. openagents-terminal uses this one.
- intensity.rs contains only `pub use coder_ui::theme::{Intensity, NEAR_BLACK, NEAR_BLACK_TINT};`, labeled "Compatibility imports".

**Impact:** The CLIs show different "working" indicators, and there are two APIs for one concept.

**Suggested action:**
1. Make spinner.rs a thin wrapper over grok_spinner, or move its callers to grok_spinner, and delete the 10-frame table.
2. Delete intensity.rs once `Intensity` has moved (CS-03).
3. Verify that `git grep -n 'spinner::FRAMES\|coder_terminal::intensity' crates` returns nothing.

### CS-15 Stale docs and dead routes left behind by the web and mobile pivots

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [coder-mobile/Cargo.toml](../../../../crates/coder-mobile/Cargo.toml) Cargo.toml:7
- [coder-mobile/src/lib.rs](../../../../crates/coder-mobile/src/lib.rs) lib.rs:1
- [coder-chat-web/README.md](../../../../crates/coder-chat-web/README.md) README.md:32-36
- [openagents-web/src/lib.rs](../../../../crates/openagents-web/src/lib.rs) lib.rs:245, lib.rs:471
- [openagents-web/src/tests.rs](../../../../crates/openagents-web/src/tests.rs) tests.rs:256
- [openagents-web/Dockerfile](../../../../crates/openagents-web/Dockerfile) Dockerfile:50-52

**Evidence:**
- lib.rs:245 routes `/static/chat.js`, and lib.rs:471 includes the file, yet tests.rs:256 asserts that no page references it.
- The comment at Dockerfile:50-52 says the cloud wasm is "served from /cloud/assets/", but nothing serves it.
- The coder-chat-web README still points authors to `#cloud-private`.
- coder-mobile still calls itself "read-only".

**Impact:** Stale text points readers and agents at surfaces that no longer exist.

**Suggested action:**
1. Delete `static/chat.js`, its route and its `include_str`, plus the entry at tests.rs:1974.
2. Rewrite the README paragraph and the Dockerfile comment, and update the coder-mobile description.
3. Make these changes together with CS-01, which touches the same files.
4. Verify that `git grep -n 'chat.js\|/cloud/assets' crates/openagents-web` returns nothing.

### CS-16 Camera fanout counts a dead sink thread as 'dropped', so a crashed output looks like a slow one

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:**
- [coderos-camera/src/fanout.rs](../../../../crates/coderos-camera/src/fanout.rs) fanout.rs:76-86, fanout.rs:99-109

**Evidence:** `publish()` handles both send errors the same way: `Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => { lane.dropped += 1; }`. `LaneCount` reports only name, delivered and dropped.

**Impact:** A sink that panics shows up only as a rising drop count, which looks the same as a slow sink.

**Suggested action:**
1. Add `dead: bool` to `Lane` and `LaneCount`, set it on `Disconnected`, and report it in status.
2. Verify with a test whose sink panics on its first frame and assert that the lane is reported dead.

### CS-17 Compositor gap and border constants are hand-copied from desktop.nix without the drift test the bind table has

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [coder-compositor/src/layout.rs](../../../../crates/coder-compositor/src/layout.rs) layout.rs:17-24
- [os/modules/coderos/desktop.nix](../../../../os/modules/coderos/desktop.nix) desktop.nix:295-297
- [coder-binds/tests/binds.rs](../../../../crates/coder-binds/tests/binds.rs)

**Evidence:** layout.rs sets `GAP_INNER = 3`, `GAP_OUTER = 6` and `BORDER = 1`. desktop.nix sets `gaps_in = 3`, `gaps_out = 6` and `border_size = 1`. binds.rs does not check gaps or border (grep finds nothing).

**Impact:** The two sets of values can drift apart silently.

**Suggested action:**
1. Export the constants from a shared location.
2. Extend `crates/coder-binds/tests/binds.rs` to parse `gaps_in`, `gaps_out` and `border_size` from desktop.nix and compare them to those constants.
3. Verify by changing one value in desktop.nix and confirming that the test fails.

### CS-18 Inconsistent mutex-poison policy: coder-browser panics on poison while sibling crates recover

**Severity:** Low · **Category:** concurrency · **Effort:** S

**Locations:**
- [coder-browser/src/workbench.rs](../../../../crates/coder-browser/src/workbench.rs) workbench.rs:74, workbench.rs:236-269
- [coder-pty/src/host/mod.rs](../../../../crates/coder-pty/src/host/mod.rs)

**Evidence:**
- workbench.rs has 23 `lock().unwrap()` calls before its first `#[cfg(test)]` at line 589, and 31 in total.
- coder-pty's host/mod.rs uses `PoisonError::into_inner` at 8 sites.
- coder-browser has wasm32-specific dependencies, so it mainly runs on single-threaded wasm.

**Impact:** Minor on wasm, where a panic aborts anyway. On native targets, a poisoned lock would cause every later call to panic.

**Suggested action:**
1. Add a private `io()` helper that recovers from poison, or use `Rc<RefCell<IO>>`, since the type is wasm-first.
2. In `result()`, take the lock once instead of repeatedly.
3. Verify that `git grep -c 'lock().unwrap()' crates/coder-browser/src/workbench.rs` shows only test-code uses.

## Refuted during verification

- **"Verse scene creation runs on a 64 MiB-stack thread to work around ~1 MB values built on the stack."** Rejected. Commit 1990cb1dc3 already boxed the large values. `verse_ffi.rs:1020-1041` has a regression test (`the_world_build_fits_a_small_stack`) that builds both the bare and the full scene on a 512 KiB thread in release (16 MiB in debug). That is the bounding test the claim asked for. The 64 MiB thread is a documented safety margin: it reserves only virtual address space and is created once per Verse mount. Shrinking `CREATE_STACK_BYTES` is trivial follow-up work, not a finding.
- **"Device and world secret keys cross the FFI as plain JSON Strings and are never zeroized."** Rejected as a finding. The facts are correct (`secret_hex: String`, no zeroize in coder-mobile), but the secret arrives from the native shell as a JNI/C string that the platform already holds. Zeroizing the Rust copy would not remove the secret from process memory. The configs derive only `Deserialize` (app.rs:22), so there is no Debug leak. This would be defense in depth only.
