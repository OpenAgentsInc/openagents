# Verse engine

Scope: `crates/{verse, verse-core, verse-engine, verse-gfx, verse-pbr, verse-net, verse-host, verse-game, verse-bake, verse-content, verse-imported, verse-private, physics, world-tree, verse-water-spells, verse-lagrange}`. Audit date 2026-10-10, snapshot `3168c986aa`.

**Health grade: C**

The Verse engine area is 16 crates, 426 tracked `.rs` files and about 203K LOC. About 157K LOC is non-test, non-example code, and examples alone add 24.4K. No other part of the monorepo changes as fast: `crates/verse` had 589 commits in the two weeks before the snapshot, 99 of them to `crates/verse/src/app.rs`. The fundamentals hold up better than the size suggests. There are 1,371 `#[test]` functions, and production code rarely panics (most of the ~3,400 `unwrap()` calls are in tests). Network input has size limits (64 KiB wire cap; entity, argument and coordinate caps) and signature checks. The headless crates `verse-engine`, `physics` and `world-tree` have small, clean dependency sets. Both earlier audits are effectively closed: every V01–V28 item in [2026-10-04-verse-engine-audit.md](../../2026-10-04-verse-engine-audit.md) is marked Complete/Addressed, and grid slices #10581–#10590, #10552, #10553 and #10575 are all CLOSED. This audit therefore carries over no feature gaps and covers code health only.

Most of the problems are structural and come from a fast run of crate extractions. `crates/verse` is a god crate with about 60 dependencies, so `openagents-cli` compiles wgpu, gltf and resvg even though it mainly uses identity, MV and XP data. Nine of those declared dependencies are unused. There are two GPU renderer stacks, and their unsafe surface constructors are duplicated. Several functions are 700–1,100 lines long, and `App` has about 100 fields. Some layering is inverted: verse-net depends on verse-gfx/wgpu, and the "shared" water-spells crate depends on two zone crates. Re-export shims keep old module paths alive. Errors are mostly `String`, and nothing compile-checks the feature matrix. One live security/availability issue exists: anyone with a freshly generated key can fill the `verse-assets` broker's replay cache, which then refuses legitimate readers.

## Measurements

| Metric | Value |
|---|---|
| Crates / `.rs` files / LOC | 16 / 426 / 202,760 |
| Non-test, non-example LOC | ~157,168 |
| Examples | 79 files, 24,406 LOC (`crates/verse/examples`: 21,371 LOC, 62 programs, 39 declared, 23 auto-discovered) |
| LOC per crate | verse 72,307; verse-pbr 35,984; physics 22,324; verse-engine 16,385; verse-imported 11,517; verse-content 8,965; verse-water-spells 6,627; verse-net 6,470; verse-core 5,325; verse-lagrange 4,574; verse-bake 4,112; verse-gfx 2,685; verse-private 1,944; world-tree 1,790; verse-host 1,286; verse-game 465 |
| Largest files | verse/src/app.rs 5,913; verse-pbr/src/pbr/gpu.rs 5,586; verse/src/render.rs 3,608; verse-pbr/src/imported/mod.rs 3,554; verse/src/zones/runtime.rs 3,117; verse/src/panels/studio.rs 2,848; verse/src/session.rs 2,763; verse/src/runtime.rs 2,615 |
| Longest functions | `Photo::new` 1,104 (gpu.rs:1046); `Renderer::draw_frame` 1,002 (imported/mod.rs:1809); `App::frame` 723 (app.rs:4141); imported `build` 713 (imported/mod.rs:751); `encode_neon` 638 (gpu.rs:3566); `App::key` 380; `App::button` 337 |
| `App` struct | ~102 field lines (app.rs:585–742); `Options` has 44 pub fields |
| Tests | 1,371 `#[test]` (verse 465, verse-pbr 191, physics 176, verse-engine 174, verse-imported 63, verse-core 62, verse-net 59, verse-lagrange 47, verse-content 33, verse-gfx 31, verse-water-spells 27, verse-private 15, verse-bake 11, world-tree 10, verse-host 5, verse-game 2) |
| Test gaps | verse-content `remote_content.rs` and `collision.rs`: 0 in-crate tests; `zones/runtime.rs`: 2 tests for 3,117 LOC |
| unwrap / expect | 3,430 / 279, mostly in tests; fewer than ~150 production sites (hotspots: verse-content `compiler/characters.rs` 17, verse-imported `remote_window.rs` 17, physics `broadphase.rs` 12) |
| panic!/unreachable!/todo! | 66 total, about 1 in production (`physics/src/queries/snapshot.rs:106`) |
| `#[allow]` | 51 (36 `clippy::too_many_arguments`, 8 `dead_code`) |
| TODO/FIXME | 0 |
| `unsafe` lines | 44 (FFI surfaces, edr, test allocators, 2 test-only `env::set_var`) |
| Errors | ~1,016–1,035 `Result<_, String>`, ~473–476 `map_err(\|e\| e.to_string())`, 3–4 typed error enums |
| Public items | 4,558 `pub` fn/struct/enum/trait/const/type/mod |
| verse `Cargo.toml` | 20 non-default features, ~60 dependencies, 9 unused |
| `ZoneId::<Variant>` references | 327 across 25 files (zones/runtime.rs 93) |
| Repo weight | `assets/verse` 263 MB / 1,813 files, no LFS (largest PNG 14.1 MB); `bench/verse` 106 MB / 2,701 files (782 `.log`, 76 `source.patch`); git pack 8.20 GiB |
| Churn since 2026-09-26 | verse 589 commits, verse-pbr 76, verse-engine 69, physics 60; most extracted crates created 2026-10-03 to 2026-10-05 |
| Prior audits | V01–V28 all Complete/Addressed; grid issues #10581–#10590 all CLOSED |

## Strengths

- **Untrusted network input has limits and signature checks.** `net::parse` uses `parse_strict_bounded` with `MAX_WIRE` 64 KiB (verse-net/src/net.rs:33,91), and tungstenite caps message and frame sizes (net/relay.rs:142). MV decode calls `validate_id` and `validate_crypto` (mv.rs:530-531, 587-588). Profile kind-0 and room 39000 events are signature-checked (session.rs:1112-1121). `MAX_ENTITIES`, `MAX_BODIES`, `MAX_COMMAND_ARGS` and `MAX_COORD` caps are in mv.rs:25-41.
- **Production code rarely panics.** Fallible paths return `Result`. Examples: verse-host reads input with size limits and checks that key files are owner-only (verse-host/src/lib.rs:15-30), and the broker recovers poisoned locks with `unwrap_or_else(|e| e.into_inner())` (verse-private/src/broker.rs:94).
- **Headless layers have tight dependency sets.** verse-engine depends only on rtrb, bytemuck, glam, serde, serde_json, png and sha2. physics depends on glam, serde and sha2. world-tree depends on serde and sha2 (world-tree/src/lib.rs:21-23). verse-host links no renderer.
- **Parallelism is deterministic by design.** `physics::parallel` gives results bit-identical to serial, keeps order, re-raises worker panics, and has a SERIAL test switch (physics/src/parallel.rs:1-60).
- **The private-asset design is careful.** NIP-98 is checked against the exact URL, method and body digest. Registry names and digests are validated before any object path is built (verse-private/src/lib.rs:62-73). Signed URLs are never logged, and strangers get the same refusal as requests for missing assets.
- **Identity keys are handled safely.** Keys are created with `create_new` and mode 0600 (verse-net/src/identity.rs:167-172). Read-only commands use ephemeral in-memory keys (`load_or_ephemeral`, identity.rs:129).
- **Test investment is substantial.** There are 1,371 tests, allocation-free assertions for real-time audio using counting allocators, and retained bench receipts that record failures honestly. A `compile_error!` guard keeps dev-destruction out of web and phone builds (verse-zone-everglade/src/lib.rs:17).
- **Prior audits were followed through.** Every V01–V28 item has an explicit status, a remediation issue and retained evidence, and docs/verse/status.md keeps one current capability table.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| VE-01 | High | security | Anyone can fill the verse-assets broker replay cache, which then refuses every legitimate reader | S |
| VE-02 | Medium | architecture | `verse` is a god crate (~60 deps), so consumers that need little of it compile the renderer and every zone | L |
| VE-03 | Medium | build | Nine dependencies in verse/Cargo.toml are unused, two of them orphan optionals | S |
| VE-04 | Medium | duplication | Two GPU renderer stacks with duplicated unsafe surface constructors and vertex/lighting code | XL |
| VE-05 | Medium | maintainability | God functions (1,104 / 1,002 / 723 lines) and an `App` struct with ~100 fields | L |
| VE-06 | Medium | architecture | Inverted layering: net depends on graphics, shared spells on zones, core on the renderer | M |
| VE-07 | Medium | architecture | Closed `ZoneId` enum dispatch: 327 variant references in 25 files, no zone trait | L |
| VE-08 | Medium | error-handling | Browser chamber worker failures are swallowed, and `close()` reports `Closed` for a failed worker | S |
| VE-09 | Medium | build | Nothing compile-checks the verse feature/target matrix | S |
| VE-10 | Medium | repo-hygiene | ~370 MB of binary assets and bench receipts in-tree without LFS, with absolute home paths | M |
| VE-11 | Low | maintainability | Extraction leftovers: re-export shims, and admission tests in the wrong crate | M |
| VE-12 | Low | testing | A verse test sets `VERSE_HOME` under a false SAFETY claim while sibling `App` tests read the real home | S |
| VE-13 | Low | error-handling | Errors are strings throughout (~1,000 `Result<_, String>`, 3 typed error enums) | L |
| VE-14 | Low | testing | Content compiler unwraps optional skins in Result-returning functions; several compiler modules have no tests | S |
| VE-15 | Low | build | verse-host build script reruns on every `git add` and hardcodes a partial dependency list | S |
| VE-16 | Low | performance | `physics::parallel` spawns new OS threads in every simulation step | M |
| VE-17 | Low | security | Throwaway-key XP fixtures are public API in verse-net and used by production preview paths | S |
| VE-18 | Low | security | verse-game `Replay` decode/verify set no limits on untrusted replays | S |
| VE-19 | Low | duplication | Duplicated small helpers: hex encoders, 12 `write_png` copies, identical counting allocators | S |
| VE-20 | Low | maintainability | 62 example programs (21K LOC) in the engine crate, 23 undeclared, some of them operator tools | M |
| VE-21 | Low | error-handling | Identity key creation is not crash-safe and races between processes | S |
| VE-22 | Low | docs | Grid multiplayer audit is stale and has no per-slice status | S |
| VE-23 | Low | build | Dependency pins differ across Verse crates; root `[workspace.dependencies]` holds only iroh | S |

### VE-01 Anyone can fill the verse-assets broker replay cache, locking out every legitimate reader

**Severity:** High · **Category:** security · **Effort:** S

**Locations:** [broker.rs](../../../../crates/verse-private/src/broker.rs) (broker.rs:22-25, broker.rs:93-101, broker.rs:109-140)

**Evidence:** `first_use` refuses a request when `seen.len() >= MAX_SEEN` (10,000) and keeps IDs for `REPLAY_SECONDS` = 150 (lines 22-25, 93-101). `grant()` calls `first_use(&auth.event_id, now)` (line 122) as soon as `nostr::domain::parse_http_authorization` succeeds. That check only proves that *some* key signed the event for this URL, body and time. Reader admission (`manifest.admits(&auth.pubkey)`, line 138) runs only after `self.store.read(&object).await`, which is a GCS read. A freshly generated key can therefore sign 10,000 valid NIP-98 events within 150 s (~67 req/s), and every real reader then gets 403 until those entries expire. On top of that, `seen.retain` scans the whole cache (O(n)) under the mutex on every request.

**Impact:** An unauthenticated party can take the private asset broker offline (owner placements fail to load), and every request from a stranger costs a GCS manifest read before it is refused. Because the cache is per instance, Cloud Run scaling does not help: the flood hits whichever instance serves it.

**Suggested action:**
1. In `grant()`, parse the body, read and parse the manifest, and check `manifest.admits(&auth.pubkey)` and the sha256 match. Call `first_use` only after those pass, so only admitted readers occupy cache slots.
2. Replace refuse-when-full with eviction of the oldest entry: a `VecDeque<(u64, String)>` plus a `HashSet`, pruned by age from the front. This also removes the O(n) `retain`.
3. Optional: cache parsed manifests for a few seconds to cut down GCS reads.
4. Verify with a new broker.rs test: send 10,001 requests signed by strangers, then one from an admitted reader, and assert the reader is granted.

### VE-02 `verse` is a god crate: ~60 deps, so consumers that need little of it compile the whole renderer and every zone

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:** [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:53-176), [verse/src/lib.rs](../../../../crates/verse/src/lib.rs) (lib.rs:10-91), [openagents-cli/Cargo.toml](../../../../crates/openagents-cli/Cargo.toml) (Cargo.toml:99), [chamber.rs](../../../../crates/openagents-cli/src/chamber.rs) (chamber.rs:226), [verse_town.rs](../../../../crates/openagents-cli/src/verse_town.rs) (verse_town.rs:20-21), [walkers.rs](../../../../crates/openagents-cli/src/walkers.rs) (walkers.rs:11-12)

**Evidence:** These dependencies are unconditional: wgpu, gltf, resvg, swash, png, verse-pbr, verse-imported, verse-gym, all zone crates, town-clock, townsfolk, gym-leaderboard, coder-access, coder-ui, rust-native, terminal-control and atif. Native builds add reqwest, tokio-tungstenite, rustls, gym-bridge, coder-lease and verse-private. openagents-cli's uses of `verse` (rg counts): identity 30, xp 19, mv 16, session 10, imported 7, zones 6, net 3, controller 2, blocklist 2, agent 1, loopback 1, terminal_control 1. Most of that is net/identity data. The CLI also uses `verse::zones::everglade::townsfolk`, `everglade_pack`, `verse::controller`, `verse::agent::Agent`, `verse::loopback::LoopbackRelay` and `verse::imported::original::generate`, so it cannot simply drop `verse` today. coder-mobile and openagents-mobile render the world (`verse::ui`, `verse::mesh`, `verse::world`, the imported-surface feature), so they do need the renderer. The crate also contains product features: panels/studio.rs, workshop.rs, replay.rs and the terminal overlay.

**Impact:** The CLI pays a large compile and link cost, and any edit to app.rs or render.rs invalidates the CLI build. Product code (studio, terminal, gym replay) is mixed in with engine code.

**Suggested action:**
1. In openagents-cli, import identity/mv/xp/blocklist/net directly from verse-net and remote_content from verse-content. Move the `session::{WORLD, BARE_WORLD, PUBLIC_RELAY, MAX_DISPLAY_NAME}` constants into verse-net.
2. Move what the CLI still needs from `verse` (controller, agent, loopback relay, everglade pack/townsfolk route helpers) into the crates that own them (verse-world, verse-core, verse-zone-everglade), then remove the CLI's `verse` dependency. Verify that `cargo tree -p openagents-cli -e normal -i wgpu` returns nothing.
3. Split studio/workshop/replay out of `verse` into feature-gated crates.
4. Leave the mobile apps on `verse`, since they render.

### VE-03 Nine dependencies declared in verse/Cargo.toml are unused (extraction leftovers), two of them orphan optionals

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:** [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:62, :126, :139-140, :145, :147, :168, :171, :172, :175-176)

**Evidence:** Searching crates/verse/src, examples and tests with rg for `<crate>::` / `use <crate>` finds 0 uses of reqwest, webpki_roots, swash, resvg, miniz_oxide, gltf, toml, memmap2 and coder_pty. `gltf` appears only as file-extension strings (grid_robot.rs:131, grid_pack.rs:342). `toml` appears only as string literals in examples/relevance/cases.rs:120,156, so examples do not use the crate either. rustls is used only in 4 example/test files. memmap2 and coder-pty are `optional = true`, but no feature names `dep:memmap2` or `dep:coder-pty`; `git log -S` traces them to 916329e292 ("Share the smart terminal between Grid and a native app"). The miniz_oxide comment ("Deflate for the zone pack's geometry") is out of date.

**Impact:** Slower builds (blocking reqwest with TLS, resvg, gltf), a larger dependency surface to audit, and misleading comments.

**Suggested action:**
1. Remove reqwest, webpki-roots, swash, resvg, miniz_oxide, gltf, toml, memmap2 and coder-pty from crates/verse/Cargo.toml.
2. Before moving rustls to `[dev-dependencies]`, check whether it is there to force the `ring` provider for tokio-tungstenite through feature unification. If it is, keep it and add a comment saying so.
3. Add `cargo machete` over `crates/verse*`, `crates/physics` and `crates/world-tree` to scripts/check-dependencies.sh.
4. Verify with `cargo check -p verse` (default), `--no-default-features --features xp-host`, and `--target wasm32-unknown-unknown --no-default-features --features browser-chamber`.

### VE-04 Two parallel GPU renderer stacks with duplicated unsafe surface constructors and vertex/lighting code

**Severity:** Medium · **Category:** duplication · **Effort:** XL

**Locations:** [render.rs](../../../../crates/verse/src/render.rs) (render.rs:1-9, render.rs:408-428, render.rs:444-470), [grid_engine.rs](../../../../crates/verse/src/grid_engine.rs) (grid_engine.rs:247-330), [pbr/instanced.rs](../../../../crates/verse-pbr/src/pbr/instanced.rs) (instanced.rs:49), [imported/mod.rs](../../../../crates/verse-pbr/src/imported/mod.rs) (mod.rs:41), [water/mod.rs](../../../../crates/verse-pbr/src/water/mod.rs) (mod.rs:78-82)

**Evidence:** render.rs and grid_engine.rs are separate stacks. render.rs is the legacy face/line renderer and switches to the Photo path in pbr/gpu.rs when a frame carries a Sky. grid_engine.rs uses `imported::Renderer` from the 3,554-line imported/mod.rs. grid_engine.rs:285 itself calls render.rs "the legacy renderer". Surface creation is duplicated: `from_metal_layer` at render.rs:408 and grid_engine.rs:247/263 contain the same null check and `create_surface_unsafe(CoreAnimationLayer)` block, and `from_android_window` at render.rs:444 and grid_engine.rs:291/307 build the Android handle the same way (only `android::backends()` is shared). There are two `struct GpuVertex` definitions (pbr/instanced.rs:49, imported/mod.rs:41). Water is already partly unified: water/water.wgsl (984 lines) is a shared core with front ends photo.wgsl (995) and imported.wgsl (170), and there is a GPU parity test (water/parity.rs).

**Impact:** Lighting, shadow and quality-tier changes have to be kept in parity across two renderers, and a fix to the unsafe platform surface code has to be made in two places.

**Suggested action:**
1. Extract `verse_pbr::surface::{metal_layer_surface, android_window_surfaces}` to hold the unsafe `create_surface_unsafe` calls and the backend loop, and call it from both render.rs and grid_engine.rs.
2. Document in docs/verse that `imported::Renderer` is the target and render.rs is legacy, with a per-zone migration checklist. Delete the render.rs pipelines when the last zone has moved.
3. Shrink water/photo.wgsl by moving anything it shares with imported.wgsl into water.wgsl.
4. Verify each zone with the existing capture examples, and keep water/parity.rs green.

### VE-05 God functions and god structs: Photo::new 1,104 lines, draw_frame 1,002, App::frame 723; App has ~100 fields

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:** [pbr/gpu.rs](../../../../crates/verse-pbr/src/pbr/gpu.rs) (gpu.rs:1046, gpu.rs:3566), [imported/mod.rs](../../../../crates/verse-pbr/src/imported/mod.rs) (mod.rs:751, mod.rs:1809), [app.rs](../../../../crates/verse/src/app.rs) (app.rs:585-742, app.rs:4141)

**Evidence:** Function lengths re-measured by brace matching: `Photo::new` (gpu.rs:1046) 1,104 lines; `draw_frame` (imported/mod.rs:1809) 1,002; `App::frame` (app.rs:4141) 723; `build` (imported/mod.rs:751) 713; `encode_neon` (gpu.rs:3566) 638. `struct App` runs from app.rs:585 to about line 742 and has 102 top-level field lines. `#[allow(clippy::too_many_arguments)]` appears 36 times across the Verse crates.

**Impact:** Frame, input and pipeline changes are hard to review and test in isolation. In a file with this much churn, concurrent agents keep editing the same functions and colliding.

**Suggested action:**
1. Split `Photo::new` into per-pipeline-group `build_*_pipelines(device, &Capability)` functions.
2. Split `draw_frame` into per-pass methods on a `FramePass` context.
3. Group `App` fields into sub-structs (input, surface, zone services, HUD, net) and move the key/button/frame handlers into `crates/verse/src/app/` modules.
4. Keep every split behavior-preserving. Verify with byte-identical capture PNGs and the app.rs tests.
5. Enable `clippy::too_many_lines` (threshold ~300) for verse and verse-pbr so the functions do not grow back.

### VE-06 Inverted crate layering: net depends on graphics, shared spells on zones, core on the renderer

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:** [verse-net/Cargo.toml](../../../../crates/verse-net/Cargo.toml) (Cargo.toml:16, :22), [feed.rs](../../../../crates/verse-net/src/feed.rs) (feed.rs:401, feed.rs:440), [xp.rs](../../../../crates/verse-net/src/xp.rs) (xp.rs:25), [verse-gfx/Cargo.toml](../../../../crates/verse-gfx/Cargo.toml) (Cargo.toml:9-22), [verse-water-spells/Cargo.toml](../../../../crates/verse-water-spells/Cargo.toml) (Cargo.toml:9-18), [verse-water-spells/src/lib.rs](../../../../crates/verse-water-spells/src/lib.rs) (lib.rs:1-28), [verse-core/src/lib.rs](../../../../crates/verse-core/src/lib.rs) (lib.rs:17-23), [verse-imported/Cargo.toml](../../../../crates/verse-imported/Cargo.toml) (Cargo.toml:37)

**Evidence:** verse-net (described as "Nostr session plumbing") depends on verse-gfx and coder-ui. verse-gfx brings in wgpu, swash, paper-mono, and verse-world through the default `world-camera` feature. The only uses are `verse_gfx::ui::drawable` at feed.rs:401/440 and `coder_ui::theme::Intensity` at xp.rs:25. verse-water-spells ("Shared Verse water spell simulation...") depends on verse-zone-everglade and verse-zone-grove, contains terrain.rs and sea.rs, and its lib.rs doc describes the whole Water Lab (keys T/Y/U, the cove). verse-zone-water is just `pub use verse_water_spells::*;` plus coast. verse-core keeps `use verse_net::mv; use verse_pbr::mesh;` aliases labelled "The paths the moved modules were written against". verse-imported optionally depends on verse-zone-crypt.

**Impact:** Network-only builds pull in a GPU stack. The "shared" spells crate cannot be reused without specific zones, and a dependency cycle becomes likely as soon as a zone needs a spell.

**Suggested action:**
1. Move `drawable` and its tables into paper-mono (or a small glyph module that both verse-gfx and verse-net use). Replace `coder_ui::theme::Intensity` in xp.rs with a verse-net enum that is mapped at the UI edge, then drop verse-net's verse-gfx and coder-ui dependencies. Verify that `cargo tree -p verse-net -i wgpu` is empty.
2. Move terrain.rs and sea.rs into verse-zone-water, remove verse-water-spells' zone dependencies, and rewrite its lib.rs doc.
3. Replace verse-core's legacy path aliases with direct paths.

### VE-07 Closed ZoneId enum dispatch: 327 variant references across 25 files, no zone trait

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:** [zones/mod.rs](../../../../crates/verse/src/zones/mod.rs) (mod.rs:76-100), [zones/runtime.rs](../../../../crates/verse/src/zones/runtime.rs) (runtime.rs:895, runtime.rs:1764), [app.rs](../../../../crates/verse/src/app.rs), [runtime.rs](../../../../crates/verse/src/runtime.rs)

**Evidence:** `rg -o` counts 327 `ZoneId::<Variant>` occurrences in 25 files under crates/verse/src: zones/runtime.rs 93, zones/tests.rs 29, app.rs 29, everglade_tests.rs 27, runtime.rs 25. `apply_zone_intent` (zones/runtime.rs:895) is 277 lines and `zone_snapshot` (zones/runtime.rs:1764) is 312 lines. No `trait Zone*` exists in verse, verse-core or verse-world.

**Impact:** Every new zone means edits to app.rs, runtime.rs, zones/runtime.rs, the minimap and the HUD, which causes merge conflicts. A missed match arm silently falls into a `_ =>` default.

**Suggested action:**
1. Define `trait ZoneRuntime { tick, apply_intent, snapshot, hud, minimap }` in verse-core.
2. Implement it in each `verse-zone-*` crate, and have `zones::State` hold the active zone as `Box<dyn ZoneRuntime>`, keeping `ZoneId` as a registry key.
3. Migrate Coast first as the pilot.
4. Track progress with `rg -c 'ZoneId::' crates/verse/src`.

### VE-08 Browser chamber worker failures are swallowed; close() reports Closed for a failed worker

**Severity:** Medium · **Category:** error-handling · **Effort:** S

**Locations:** [chamber_session.rs](../../../../crates/verse-imported/src/imported/chamber_session.rs) (chamber_session.rs:248-270, chamber_session.rs:379-399), [worker.rs](../../../../crates/verse-world/src/service/worker.rs) (worker.rs:134-160)

**Evidence:** On wasm, `start_browser` spawns `async move { let _ = worker::run(client, cursor, ...).await; }`, which throws away the Result. `close()` aborts the task, and because `self.thread` is None on wasm, it returns `Stopped::Closed`. `alive()` only checks `self.output.is_closed()`. The worker's `Update` enum (worker.rs:134) has no terminal-error variant that could carry the reason instead. The native path does map `Some(Ok(Err(error)))` to `Stopped::Failed(error)`.

**Impact:** In the browser (/grid, the everglade-web chamber), authentication refusals, digest mismatches and transport errors disappear. The UI cannot show a reason, and acceptance runs cannot tell a failure from a clean exit.

**Suggested action:**
1. On wasm, store a `futures::channel::oneshot::Receiver<Result<(), String>>` on `Session`, and have the spawned task send `worker::run`'s result into it.
2. In `close()`, and in a new `poll_stopped()` that the UI can call once `alive()` is false, map `Err` to `Stopped::Failed` on wasm as well, through one mapping function shared with the native path.
3. Add a test that uses the `chamber-loopback` feature with a gateway that refuses, and asserts `Stopped::Failed`.

### VE-09 No automated compile check of the verse feature/target matrix

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:** [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:9-51), [openagents-cli/Cargo.toml](../../../../crates/openagents-cli/Cargo.toml) (Cargo.toml:99), [coder-mobile/Cargo.toml](../../../../crates/coder-mobile/Cargo.toml) (Cargo.toml:25, :64), [openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (Cargo.toml:45), [runtime-contract.json](../../../verse/runtime-contract.json) (runtime-contract.json:93-99), [verse-runtime-contract.py](../../../../scripts/verse-runtime-contract.py) (verse-runtime-contract.py:1-40)

**Evidence:** Besides `default`, verse defines 20 features: desktop, pylon-relay, model-host, xp-host, replay-host, capture, crypt, crypt-fight, panels, terminal, studio-host, imported-desktop, imported-surface, native-audio, hosted-social, remote-chamber, chamber-loopback, web, browser-chamber and dev-destruction. Each consumer uses a different combination: the CLI uses `xp-host,remote-chamber,studio-host`; coder-mobile uses `xp-host,studio-host,imported-surface,remote-chamber` (plus `chamber-loopback` in dev); openagents-mobile uses `xp-host`; everglade-web uses wasm32 `browser-chamber`. There is no `.github/workflows` directory. scripts/verse-runtime-contract.py only records manifests and constants into runtime-contract.json as a docs drift check. Nothing runs `cargo check` for each combination.

**Impact:** A change that compiles with the default desktop build can break the phone, browser or CLI build. Nobody finds out until someone next builds that app.

**Suggested action:**
1. Add scripts/check-verse-features.sh that runs `cargo check -p verse` for:
   - default;
   - `--no-default-features --features xp-host`;
   - `--no-default-features --features xp-host,remote-chamber,studio-host`;
   - `--no-default-features --features xp-host,studio-host,imported-surface,remote-chamber`;
   - `--target wasm32-unknown-unknown --no-default-features --features browser-chamber`;
   - plus `-p physics --target wasm32-unknown-unknown`.
2. Derive the combinations from runtime-contract.json so they follow the consumers.
3. Call the script from the release gate (scripts/gate-record.py).

### VE-10 Large binary assets and bench receipts committed without LFS (~370 MB in-tree), with absolute home paths

**Severity:** Medium · **Category:** repo-hygiene · **Effort:** M

**Locations:** [assets/verse](../../../../assets/verse), [bench/verse](../../../../bench/verse), [browser-host-Cargo.toml](../../../../bench/verse/2026-10-05/platform-clients/browser-host-Cargo.toml) (browser-host-Cargo.toml:6-8)

**Evidence:** `git ls-files assets/verse` lists 1,813 files, ~263 MB on disk. `git ls-files bench/verse` lists 2,701 files, ~106 MB, including 782 `.log` files. There is no `.gitattributes`. browser-host-Cargo.toml lines 6-8 use `path="/home/christopherdavid/work/openagents-verse-audit/crates/..."`.

**Impact:** Every clone and every fresh worktree pays for this history, and the workspace's multi-agent flow creates a lot of worktrees. The retained receipts contain absolute paths, so they cannot be rebuilt on another machine.

**Suggested action:**
1. Move large textures and `.glb`/`.vtp` packs to Git LFS, or to the digest-verified bucket pattern verse-private already uses.
2. Gzip bench logs, and cap raw profiles above 256 KB.
3. In bench `Cargo.toml` receipts, replace absolute paths with a documented `${REPO}` placeholder.
4. Add a size check, in scripts/check-dependencies.sh or a pre-commit hook, that refuses new non-LFS files over 5 MB.

### VE-11 Extraction leftovers: compatibility re-export shims, and admission tests in the wrong crate

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:** [verse/src/lib.rs](../../../../crates/verse/src/lib.rs) (lib.rs:10-90, lib.rs:68), [verse-zone-water/src/lib.rs](../../../../crates/verse-zone-water/src/lib.rs) (lib.rs:1-5), [examples/verse_host.rs](../../../../crates/verse/examples/verse_host.rs) (verse_host.rs:1-7), [verse-imported remote_content.rs](../../../../crates/verse-imported/src/imported/remote_content.rs) (remote_content.rs:1-127), [verse-content remote_content.rs](../../../../crates/verse-content/src/remote_content.rs) (remote_content.rs:1-241)

**Evidence:** verse/src/lib.rs re-exports modules from verse-gfx, net, pbr, core, gym and imported under their old paths. `pub use verse_gfx::profiling;` (line 68) has no users outside the crate (rg `verse::profiling`). verse-zone-water is `pub use verse_water_spells::*;` "to preserve public paths". examples/verse_host.rs is a "Compatibility entry point" that wraps `verse_host::run`. verse-content/src/remote_content.rs holds 241 LOC of content admission and has no `#[test]`. Its 3 tests live in verse-imported/src/imported/remote_content.rs, which is a `pub use verse_content::remote_content::*;` shim. Those tests need `verse_content::compiler::original::generate` and verse_world types, and verse-content already has both (verse-world is a normal dependency and `compiler` is a feature).

**Impact:** Every re-exported module is reachable under two names, which hides the real dependency graph. `cargo test -p verse-content` does not exercise the admission code.

**Suggested action:**
1. Move the 3 tests into verse-content/src/remote_content.rs under `#[cfg(all(test, feature = "compiler"))]`. Delete the verse-imported shim and repoint callers of `verse::imported::remote_content` (such as openagents-cli/src/chamber.rs) to `verse_content::remote_content`.
2. Delete the unused `profiling` re-export. Then codemod external `verse::{identity,mv,xp,blocklist,net}` uses to `verse_net::` and remove those re-exports.
3. Delete examples/verse_host.rs and point docs at `cargo run -p verse-host`.
4. Verify with `cargo test -p verse-content --features compiler` and a workspace `cargo check`.

### VE-12 verse test mutates VERSE_HOME with a false SAFETY claim while sibling App tests read the real home

**Severity:** Low · **Category:** testing · **Effort:** S

**Locations:** [app.rs](../../../../crates/verse/src/app.rs) (app.rs:5525, app.rs:5658-5660, app.rs:5715, app.rs:1036-1043, app.rs:1114-1122), [identity.rs](../../../../crates/verse-net/src/identity.rs) (identity.rs:75-81)

**Evidence:** app.rs:5659-5660 says `// SAFETY: tests in this module run on the test harness's threads only.` and then calls `unsafe { std::env::set_var("VERSE_HOME", &home) }`, with `remove_var` at 5715. The test harness runs tests concurrently. The other test helper that constructs `App` (app.rs:5525) reads `identity::home()`, which falls back to `~/.openagents/verse` (identity.rs:75-81), for the profile key and `zones-cache`. Depending on timing it sees either the temp home or the owner's real home. The comment at 1119-1120, "Tests never read the real home", is therefore wrong for the key and the zone cache. `struct App` is private (app.rs:585) and is only built by `run` and these two tests, so the `cfg!(test)` switches do cover every test caller.

**Impact:** The concurrent env mutation is a data race between tests (the reason Rust 2024 made `set_var` unsafe), and tests can write profile keys and caches into the developer's real `~/.openagents/verse`.

**Suggested action:**
1. Add `home: Option<PathBuf>` to `Options` (default `None` means `identity::home()`), and have `App::new` use it for `load_or_create`, the zone cache and private assets.
2. Set it to a tempdir in both tests and delete the two unsafe env calls.
3. At the same time, either keep the `cfg!(test)` switches or turn them into `Options` fields.
4. Verify by running `cargo test -p verse app::` with `HOME` pointed at an empty directory, and assert that nothing is created there.

### VE-13 Error handling is stringly typed across the engine (~1,000 `Result<_, String>`, 3 typed error enums)

**Severity:** Low · **Category:** error-handling · **Effort:** L

**Locations:** [verse-content remote_content.rs](../../../../crates/verse-content/src/remote_content.rs) (remote_content.rs:6-30), [chamber_session.rs](../../../../crates/verse-imported/src/imported/chamber_session.rs) (chamber_session.rs:242), [identity.rs](../../../../crates/verse-net/src/identity.rs) (identity.rs:89-118)

**Evidence:** Re-measured over the 16 crates: 1,016 `Result<..., String>` matches, 473 `map_err(|e| e.to_string())` and 3 `pub enum *Error`. Admission refusals (for example "Outfit model is missing a chamber animation state"), identity I/O errors and chamber errors are all flattened to `String`.

**Impact:** Callers cannot tell refusal classes apart (content mismatch, I/O, bad input) when deciding whether to retry or what to show the user.

**Suggested action:**
1. Add typed errors only at boundaries where callers branch: `verse_content::AdmissionError` (MissingModel, MissingState, Digest, Io) and `verse_net::IdentityError`. Give each a `Display` impl so existing `String` callers can migrate with `.to_string()`.
2. Leave internal `String` errors as they are.
3. Track progress with `rg -c 'Result<.*, String>' crates/verse-content crates/verse-net`.

### VE-14 Content compiler unwraps optional skins in Result-returning functions; several compiler modules have no tests

**Severity:** Low · **Category:** testing · **Effort:** S

**Locations:** [characters.rs](../../../../crates/verse-content/src/compiler/characters.rs) (characters.rs:79, :423-425, :496, :509, :622, :1008), [effects.rs](../../../../crates/verse-content/src/compiler/effects.rs), [worlds.rs](../../../../crates/verse-content/src/compiler/worlds.rs), [collision.rs](../../../../crates/verse-content/src/collision.rs)

**Evidence:** `pub fn compose(...) -> Result<(), String>` begins with `target.skin.as_ref().unwrap()` and `source.skin.as_ref().unwrap()` (lines 424-425), and later calls `mapping[old].unwrap()` (line 496). There are 8 production `skin.as_ref().unwrap()` sites: lines 79, 424, 425, 509, 533, 713, 924 and 957 (the ones from 1459 on are in tests). The same condition is handled with `ok_or(...)?` at lines 622, 1008, 1200 and 1279. `#[test]` counts: effects.rs 0, worlds.rs 0, collision.rs 0, and characters.rs 4 for 1,605 LOC.

**Impact:** A malformed or unskinned glTF makes the verse-content compiler panic instead of returning an error. The inputs are mostly repo-owned assets, so only authoring workflows are exposed.

**Suggested action:**
1. Replace each production `skin.as_ref().unwrap()` with `ok_or("<fn>: model has no skin")?`, or introduce a `SkinnedModel` newtype produced by `import_bytes`.
2. Add tests: `compose` with an unskinned source returns `Err`, and collision.rs round-trips a box mesh.
3. Verify with `cargo test -p verse-content --features compiler`.

### VE-15 verse-host build script reruns on every `git add` anywhere and hardcodes a partial dependency list

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:** [verse-host/build.rs](../../../../crates/verse-host/build.rs) (build.rs:12-35, build.rs:45-58)

**Evidence:** Lines 31-35 emit `cargo:rerun-if-changed` for git's `index` and `packed-refs`, so staging any file in the monorepo reruns the build script. The watched paths (12-27) and the `git status --porcelain` scope (45-57) list only verse-world, verse-engine, physics and verse-content. Other transitive dependencies such as nostr and paper-mono are missing. Each run spawns up to 7 git processes.

**Impact:** Every agent's build loop gets spurious rebuilds, and the `-modified` marker misses changes in transitive crates that are not on the list.

**Suggested action:**
1. Drop the `index` and `packed-refs` watches, keeping HEAD and the current ref.
2. Either compute the dirty scope from `cargo metadata` path dependencies, or record only `VERSE_SOURCE_REVISION` when it is set and "unrecorded" otherwise.
3. Verify that `git add README.md && cargo build -p verse-host` does not recompile verse-host.

### VE-16 physics::parallel spawns fresh OS threads inside every simulation step

**Severity:** Low · **Category:** performance · **Effort:** M

**Locations:** [parallel.rs](../../../../crates/physics/src/parallel.rs) (parallel.rs:38-65), [contact.rs](../../../../crates/physics/src/contact.rs) (contact.rs:567), [collision.rs](../../../../crates/physics/src/collision.rs) (collision.rs:794, collision.rs:821)

**Evidence:** Each call to `each` runs `std::thread::scope(... scope.spawn(move || f(part)) ...)`, creating up to MOST-1 = 7 new threads. It is called on every step from contact.rs:567, and from collision.rs:794/821 through `map` (64 and 128 items per thread).

**Impact:** Thread creation eats into the frame budget exactly when scenes are heaviest, and mobile suffers most. Nobody has measured the actual cost yet.

**Suggested action:**
1. Keep the ordered-split semantics, but run the parts on a persistent pool owned by `World` (parked threads with a job channel, or a rayon `ThreadPool::scope`) sized `min(cores, MOST)`.
2. Benchmark step p95 before and after with a dense-contact fixture.
3. Keep the SERIAL bit-identity tests passing.

### VE-17 Throwaway-key XP fixtures are public API in verse-net and used by production preview paths

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:** [xp.rs](../../../../crates/verse-net/src/xp.rs) (xp.rs:1437), [xp/fixture.rs](../../../../crates/verse-net/src/xp/fixture.rs) (fixture.rs:1-20), [coder-mobile verse_app.rs](../../../../crates/coder-mobile/src/verse_app.rs) (verse_app.rs:921-945), [openagents-mobile trainer.rs](../../../../crates/openagents-mobile/src/trainer.rs) (trainer.rs:500-508)

**Evidence:** `pub mod fixture;` is unconditional (xp.rs:1437). It exposes `signer(n)`, which derives secret keys from small integers, with the comment "Never use these keys for anything real". Production code uses it as well as tests and examples. coder-mobile's `xp_preview`/`playtest_preview` (verse_app.rs:922-942) and openagents-mobile's trainer.rs:503-504 build preview ledgers from these keys, trusting only the throwaway referee in a scoped `XpTrust`.

**Impact:** Deterministic signing keys ship as public API in phone binaries. A future caller could add a fixture referee to a real trust set.

**Suggested action:**
1. Do not simply gate the module with `cfg(test)`, because that breaks both mobile apps. Put it behind a `preview-fixtures` feature that coder-mobile, openagents-mobile and dev-dependencies/examples enable.
2. Rename the preview entry points (for example `preview::playtest_events`) so the intent is explicit.
3. Add a unit test asserting that `openagents_trust()` and `playtest_trust()` never contain any fixture signer's pubkey.

### VE-18 verse-game Replay decode/verify are unbounded for untrusted replays

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:** [verse-game/src/lib.rs](../../../../crates/verse-game/src/lib.rs) (lib.rs:289-330, lib.rs:333-358)

**Evidence:** `decode` accepts any `ticks: u32`, any number of inputs, and a `game` line of any length. It checks that inputs are ordered but not that each input tick is in `1..=ticks`. `verify` loops `while game.tick() < self.ticks && game.outcome().is_none()`. An input at tick 0 is never matched, which also stops every later input from being consumed, so the loop runs through all ticks before refusing. Today only tests call decode/verify (verse-game lib.rs:439-445, bunny-web zone.rs:270-274), so the problem is latent.

**Impact:** The envelope is designed for replays from untrusted sources. Once they are verified, a replay with `ticks = u32::MAX` makes the verifier step about 4 billion times.

**Suggested action:**
1. Add `MAX_REPLAY_TICKS` (or hz × max seconds) and `MAX_INPUTS`.
2. In `decode`, reject ticks over the cap, input ticks of 0 or greater than `ticks`, and a game id longer than 64 bytes.
3. Add refusal tests next to `a_replay_round_trips_and_verifies_and_refuses_tampering`.

### VE-19 Duplicated small helpers: hex encoders, 12 write_png copies, identical test counting allocators

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:** [verse-game lib.rs](../../../../crates/verse-game/src/lib.rs) (lib.rs:228), [verse-bake layers.rs](../../../../crates/verse-bake/src/layers.rs) (layers.rs:406), [verse-bake scene.rs](../../../../crates/verse-bake/src/scene.rs) (scene.rs:160), [baked_layers.rs](../../../../crates/verse-pbr/src/pbr/baked_layers.rs) (baked_layers.rs:294), [world-tree lib.rs](../../../../crates/world-tree/src/lib.rs) (lib.rs:670), [verse-private lib.rs](../../../../crates/verse-private/src/lib.rs) (lib.rs:82), [verse-engine audio.rs](../../../../crates/verse-engine/src/audio.rs) (audio.rs:907-913), [audio_native.rs](../../../../crates/verse/src/audio_native.rs) (audio_native.rs:549-555)

**Evidence:** `fn hex(bytes: &[u8]) -> String` is defined in verse-game, verse-bake (layers.rs and scene.rs), verse-pbr baked_layers.rs and world-tree. verse-private has `sha256_hex`, and verse-world, which is out of scope, has 3 more. 12 files in crates/verse/examples define `fn write_png`. The test-only tracking `GlobalAlloc` modules in verse-engine audio.rs `rt_tests` and verse audio_native.rs tests begin identically.

**Impact:** Minor risk that the copies drift, and extra noise.

**Suggested action:**
1. Export one `pub fn hex(&[u8]) -> String` from verse-engine and use it in verse-game, verse-bake and verse-pbr. world-tree keeps its own because it has no verse-engine dependency.
2. Add crates/verse/examples/common/png.rs with a single `write_png`.
3. Move the counting allocator into `verse_engine::test_alloc` behind `cfg(any(test, feature = "test-support"))`.

### VE-20 Example sprawl: 62 example entries (21K LOC) in the engine crate, 23 undeclared, including operational tools

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:** [crates/verse/examples](../../../../crates/verse/examples), [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:200-330), [verse_migrate.rs](../../../../crates/verse/examples/verse_migrate.rs)

**Evidence:** `ls crates/verse/examples` shows 62 entries, Cargo.toml has 39 `[[example]]` blocks, and the `.rs` files total 21,371 LOC. Undeclared examples are auto-discovered with no `required-features`, so `cargo test -p verse` builds them with default features. Operator tools (verse_migrate, verse_load, frame_profile) live as examples.

**Impact:** `cargo test -p verse` compiles dozens of binaries, and production tooling such as save migration is not treated as a real binary.

**Suggested action:**
1. Promote verse_migrate and verse_load to `[[bin]]`s in verse-host, or to `openagents verse` CLI subcommands, and give them tests.
2. Declare the remaining examples with `required-features = ["capture"]`, or set `autoexamples = false` and declare each one.
3. Fold the one-off `everglade_*`/`meteor_*` capture examples into one table-driven `verse_capture --scene NAME`.
4. Verify by timing `cargo test -p verse --no-run` before and after.

### VE-21 Identity key creation is not crash-safe and races between processes

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:** [identity.rs](../../../../crates/verse-net/src/identity.rs) (identity.rs:89-118, identity.rs:157-176)

**Evidence:** On `NotFound`, `load_or_create` calls `write_private`, which opens the file with `create_new(true)` and calls `writeln!`, with no fsync and no temp-file-plus-rename. If two processes start together, the loser gets "cannot create ..." instead of re-reading the winner's key. A process that reads between create and write sees an empty file and fails with "holds an invalid key". A crash mid-write leaves a truncated key that fails permanently. The secret hex is also parsed twice (`RelaySigner::from_secret_hex`, then a manual `from_str_radix` loop at 103-111), and that loop is duplicated again in `load_or_ephemeral`.

**Impact:** Rarely, first launch fails, or a profile stays unusable until someone deletes the file by hand.

**Suggested action:**
1. Write the key to `<profile>.key.tmp-<pid>` with mode 0600, call `sync_all`, then `hard_link` it to the final name. On EEXIST, re-read the winner's key.
2. Factor out one hex-to-SecretKey helper and use it in both `load_or_create` and `load_or_ephemeral`.
3. Tests: two threads calling `load_or_create` concurrently get the same pubkey, and an empty or truncated file produces an error that names the path.

### VE-22 Grid multiplayer audit is stale and carries no per-slice status, unlike the engine audit

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:** [2026-10-05-grid-multiplayer-audit.md](../../2026-10-05-grid-multiplayer-audit.md) (lines 19-32, 139-152)

**Evidence:** Its "What exists" table still says "The Grid on the web | Does not exist", says Everglade shares "Nothing" from the arch, and says name tags have "No display name", even though the slice issues it links are closed. The doc has no Status lines. By contrast, [2026-10-04-verse-engine-audit.md](../../2026-10-04-verse-engine-audit.md) has Status/Remediation/Implemented links for each V-item (the V03/V04/V07 remediation issues #10575/#10580/#10596 are all CLOSED).

**Impact:** Agents routing work may re-plan slices that are already finished, or cite gaps that no longer exist.

**Suggested action:**
1. Add a "Status (2026-10-10)" line under each G-gap, and a Status column in the slices table naming the closing issue and bench receipt.
2. Label "What exists" as the 2026-10-05 baseline and link docs/verse/status.md for the current state.

### VE-23 Inconsistent dependency pins across Verse crates; root [workspace.dependencies] holds only iroh

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:** [verse-private/Cargo.toml](../../../../crates/verse-private/Cargo.toml) (Cargo.toml:28), [verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:62), [verse-content/Cargo.toml](../../../../crates/verse-content/Cargo.toml) (Cargo.toml:26), [root Cargo.toml](../../../../Cargo.toml) (Cargo.toml:15-22)

**Evidence:** verse-private pins `sha2 = "0.11.0"` while the other Verse crates use `"0.10"`; Cargo.lock has both 0.10.9 and 0.11.0. gltf is pinned `=1.4.1` in verse but uses a caret `1.4.1` in verse-content. `glam = "0.30.10"` is repeated in 22 crate manifests. Root `[workspace.dependencies]` lists only iroh entries.

**Impact:** Bumping a version (glam, wgpu) means editing many manifests in lockstep, and mixed exact and caret pins can resolve differently when the lockfile is regenerated.

**Suggested action:**
1. Add glam, wgpu, gltf, sha2, serde, serde_json, png and secp256k1 to root `[workspace.dependencies]` with one pin each, and switch the Verse crates to `{ workspace = true, features = [...] }`.
2. Remove gltf from verse entirely, since it is unused (VE-03).
3. Verify with `cargo tree -d -p verse` and the VE-09 feature matrix.

## Refuted during verification

- **"Three parallel GPU renderers."** There are two. The Photo path is reached through render.rs. Water already shares a 984-line core shader and has a GPU parity test (VE-04).
- **"Other crates construct `App`" (VE-12).** `App` is private and is only built by `run` and two in-module tests. The texture_gpu `COPY_SRC` difference under `cfg(test)` is harmless: it only adds a usage flag in verse-pbr's own tests.
- **"Point openagents-cli and the mobile apps at verse-net instead of verse" (VE-02).** That would not compile. The CLI also uses zones, controller, agent, loopback and `imported::original`, and both mobile apps render through `verse`.
- **"Gate the XP fixtures with `cfg(any(test, feature = "test-support"))`" (VE-17).** Both mobile apps use the fixtures in production preview mode, so this would break them.
- **"sha2 0.11 in verse-private causes a duplicate compile" (VE-23).** nostr, openagents-cli, coder-host and others already pull in sha2 0.11, so the pin adds no extra compile.
- **"verse-imported declares gltf" (VE-23).** It does not.
- **"toml is used by examples" (VE-03).** It appears only as string literals there. Remove the dependency instead of moving it to dev-dependencies.
- **"The CLI could just drop the verse dependency today."** It cannot until the moves in VE-02 step 2 are done.
