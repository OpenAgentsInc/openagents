# Handoff: Grid multiplayer (epic #10590)

Written 2026-10-05 by the Devin session that ran slices 1 through 6 of the
Grid multiplayer epic. Read this before picking up #10586, #10587, #10588,
#10589, or #10626.

## Owner's standing instructions

- Merge your own PR when its checks pass (`command gh pr merge --squash
  --delete-branch N`) and move to the next issue. Never wait on the owner.
- Owner-only steps (a real phone, iOS or macOS hardware, real sats) go in
  `NEEDS_OWNER.md`; close the issue anyway and say what is left.
- The Grid renders through `verse-engine` (`crates/verse/src/grid_engine.rs`,
  `GridEngine::draw`), never through the legacy `verse::render::Renderer`.
  Non-Grid zones (Everglade, Ruins, Lagrange 1) still use the legacy renderer
  until they are migrated separately.
- Every slice ships a playable test mechanism and a retained receipt under
  `docs/verse/verification/` or `docs/desktop/verification/`.
- Don't touch the MMO-gap remediation work (`docs/audits/` MMO engine audit)
  that another agent owns.
- Rust only. Swift and Kotlin stay thin glue.

## What landed (all merged to main)

| Slice | Issue | PR | What you can run |
|---|---|---|---|
| Audit | — | `docs/audits/2026-10-05-grid-multiplayer-audit.md` | — |
| 1 Simulated players | #10581 | #10592 | `openagents verse walkers 20 --world verse-bare`; `openagents verse load` viewer receipt |
| 2 Pose lane, 5 Hz, 300 ms crowd | #10582 | #10594, #10595, #10597 | relay redeployed; `verse load` shows p95 age ~185 ms |
| 3 Display names over heads | #10583 | #10598, #10601 | `openagents verse name NAME`; `verse who` |
| 4 Shared zone presence through arches | #10584 | #10604, #10605 | walk through an arch on two clients |
| 5 Public chamber guests + RITUAL arch | #10585 | #10612 | `openagents chamber host --public`; desktop walks through the gray arch |
| Grid on the engine A–E | #10613–#10617 (#10606) | #10618, #10620, #10621, #10624, #10628 | `assets/verse/grid` pack; desktop, phone, and browser Grid draw through `GridEngine` |
| Chamber CLI | #10576, #10577 | #10578 | `openagents chamber host/status/snapshot/events/watch/move/jump/cast/respawn/...` |

Known pre-existing test failures on main, not caused by this work:
#10599 (`bare_presence_tests::bare_world_players_see_each_other_move`),
#10600 (`openagents-mobile` provider_keys copy), and
`coder-mobile terminal_live_tests::the_app_opens_a_shell_on_a_real_host_and_runs_commands`
(needs a live host; see
`docs/desktop/verification/2026-09-30-playable-grid/unrelated-failures.md`).

## In flight: #10586, the phone joins the chamber

Branch `devin/1791180782-phone-chamber` (based on `c9fefd2468`; main has
moved, rebase first). Uncommitted work tree, 14 files, about 630 lines.
Nothing is pushed yet. State:

Done on the branch:

- `crates/verse/src/imported/chamber_session.rs` (new): platform-neutral
  chamber `Session` — starts the bounded `worker::run` on a
  `chamber-worker` thread with `oneshot` stop, `consume()` drains updates
  without blocking, `step(scene, Held)` applies controls and prediction,
  `frame(pack, atlas, scene, size)` builds the engine `View`, dynamic
  instances, `UiBatch`, and `Lighting` from `chamber::remote_scene_predicted`,
  plus `cast`, `respawn`, `jump`, `target_nearest`, `stop() -> Stopped`.
  Three tests pass (`cargo test -p verse --features
  remote-chamber,imported-desktop --lib -- chamber_session ritual grid_engine`).
- `crates/verse/src/ritual.rs`: `ritual::connect(config, profile) -> Opened
  { client, runtime, pack, atlas, scene, dir }` behind `remote-chamber`.
- `crates/verse/src/grid_engine.rs`: `Content { prepared, statics, kind }`
  with `Content::grid()` and `Content::chamber(pack, dir, origin)`;
  `on_surface_with`, `from_metal_layer_with`, `from_android_window_with`
  take a `Content` so the same engine surface shows the Grid pack or a
  chamber pack.
- `crates/coder-mobile/src/chamber.rs` (new): `Play { stage: Connecting |
  Joined(Session) | Suspended | Failed, content, ... }`; `open` connects on a
  `chamber-connect` thread, `step(Held)`, `frame(size)`, `suspend`,
  `resume`. Two tests pass.
- `crates/coder-mobile/src/verse_app.rs`: `Config::ritual`,
  `Scene::chamber`, RITUAL crossing opens `Play`, Grid presence session is
  dropped while in the chamber, sticks map to `Held`, look stick drives the
  chamber camera, HUD with Leave, four ability slots (`CHAMBER_SLOTS`:
  Bow, FireBolt, MagicMissile, Shield), connection state, Respawn;
  `leave_chamber()` calls `world.return_from_ritual()`. Test
  `the_ritual_arch_opens_the_chamber_and_a_refused_connection_returns_to_the_grid`
  passes.
- `crates/coder-mobile/src/verse_ffi.rs`: `engine_content(scene)` picks
  Grid or chamber content, `rendered_chamber_revision` reopens the surface
  when the chamber pack changes, chamber frames go through
  `GridEngine::draw`; while connecting the normal Grid keeps drawing.
- `cargo ndk -t x86_64 --platform 26 -- build --locked -p coder-mobile
  --lib` produced `libcoder_mobile.so` (Android x86_64 compiles).

Still to do before the PR:

1. `cargo test -p coder-mobile --lib` has one failure to settle:
   `verse_app::tests::zone_transition_clears_input_and_keeps_plaza_identity_out_of_ruins`
   asserts `scene.session.is_none()` after entering Ruins with a relay set.
   Check whether it also fails on current main (likely since #10604 gave
   zones a shared presence world); if so, file it like #10599 and move on,
   otherwise find which branch change starts the Ruins session.
2. Rewire `crates/verse/src/imported/remote_window.rs` (desktop) onto
   `chamber_session::Session` so desktop and phone share one transport and
   prediction path. Not started; desktop still has its own copy.
3. A 20-actor phone frame-time receipt: host a chamber with
   `openagents chamber host` and the 20-NPC battle pack, run the Android
   build on the emulator (`scripts/build-coder-android.sh package`, x86_64,
   `-gpu auto`), walk through the RITUAL arch, record frame times.
4. Worker lifecycle tests on the Rust side: clean stop, host disconnect ->
   `Stopped::Failed`, suspend then resume reconnects, respawn after death.
   `chamber.rs` covers the refused-connection and suspend/resume paths only.
5. `NEEDS_OWNER.md`: iOS and a physical phone fight with a desktop player.
6. `cargo fmt`, rebase on main, PR (fetch the template first), merge, then
   `scripts/project-status.sh 10586 done` and close #10586.

Gotchas met on this slice:

- The build box runs out of disk under parallel cargo runs
  (`/home/ubuntu/target-oa` is ~70 GB). Delete `debug/incremental` and the
  `x86_64-linux-android` target before an Android build; set
  `CARGO_INCREMENTAL=0`. A full disk once truncated `verse_app.rs` mid-write;
  check `wc -l` after any scripted edit.
- `verse_world::service::wire::Snapshot.hud` is `Option<verse_world::hud::Own>`.
- `Config` and `VerseHandle` literals exist in six test files; adding a field
  means patching all of them.

## Not started

- #10587 Grid in the browser (slice 7): the browser Grid already draws
  through the engine (#10624) but has no NIP-MV presence session; wire
  `verse::session` into the web client and add a `verse walkers` receipt
  viewed from the browser. #10626 (GLES 3.0 / WebGL pose block, texture
  arrays, shadow views) blocks a real-browser run; software WebGPU works.
- #10588 population cap, block and mute, world population reporting
  (slice 8): relay-side cap per world, client-side block/mute list in
  `verse::session`, `openagents verse who --count`.
- #10589 20-player soak across phone, desktop, and browser (slice 9): run
  `verse walkers 20` plus real clients for 10 minutes, retain frame-time and
  pose-age receipts from each client.
- Owner-planned, leave alone: #10552 REACH/NIP-HOST chamber discovery,
  #9832 mainnet LSPS4 test, #9806 iOS HDR.

## Environment notes

- `PROTOC=/home/ubuntu/protoc/bin/protoc CARGO_TARGET_DIR=/home/ubuntu/target-oa`
  on every cargo command.
- Android: SDK at `/home/ubuntu/android-sdk`, NDK 27.1.12297006,
  `cargo-ndk` installed, AVD Pixel 6 API 35 (`emulator-5554`), Maven
  Central rate limits this IP (Gradle init script uses the Google mirror).
- iOS and macOS are unavailable here; use a macOS child session.
