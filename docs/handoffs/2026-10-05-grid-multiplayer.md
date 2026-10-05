# Handoff: Grid multiplayer (epic #10590)

Written 2026-10-05 by the Devin session that ran slices 1 through 6 of the
Grid multiplayer epic, and updated the same day by the Claude Code session
that finished slice 6 (#10586). Read this before picking up #10587, #10588,
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
| 6 The phone joins the chamber | #10586 | pushed to `main` (`fcd0e10b82`, `8fb646cc8d`, `becedf0d40`) | `ritual.json` in the phone's zone cache directory; walk through the RITUAL arch |

Known pre-existing test failures on main, not caused by this work:
#10599 (`bare_presence_tests::bare_world_players_see_each_other_move`),
#10600 (`openagents-mobile` provider_keys copy),
#10633 (`verse_app::tests::zone_transition_clears_input_and_keeps_plaza_identity_out_of_ruins`), and
`coder-mobile terminal_live_tests::the_app_opens_a_shell_on_a_real_host_and_runs_commands`
(needs a live host; see
`docs/desktop/verification/2026-09-30-playable-grid/unrelated-failures.md`).

## Done: #10586, the phone joins the chamber

Finished by a Claude Code session from the Devin branch
`devin/1791180782-phone-chamber` (work-in-progress commit `34c9c24568`, left
in place), landed on `main` as `fcd0e10b82`, `8fb646cc8d`, and `becedf0d40`.

- `verse::imported::chamber_session::Session` is the one chamber client for
  the phone and the desktop: worker thread, replica, prediction, movement
  frame intervals (batched to `MAX_STEPS`), tracked input, and the engine
  frame (`frame` for a phone viewport, `frame_in` for the desktop's
  720-high overlay). It reports prediction and transport `Note`s to a
  recorder that calls `observe`.
- `imported::remote_window` (desktop) mounts the session; it keeps the
  keyboard and mouse, the giver and character panels, and the recorder.
- `imported::chamber_loopback::Loopback` (tests, or the `chamber-loopback`
  feature) is an in-process RITUAL chamber over memory streams, with
  `sever()` to stand in for a host that goes away and `start(true)` for a
  dead player.
- `coder-mobile/src/chamber.rs`: `Play::open(config, secret)` signs with the
  phone's world identity (`ritual::connect_as`); `Play::open_with` takes any
  connector. Each visit writes `chamber-frames.json` beside `ritual.json`.
- The bare Grid reads `ritual.json` from the zone cache directory
  (`verse_ffi::bare_config_with_gym`), so the OpenAgents app opens the arch.
- Lifecycle tests on the session and on `Play`: clean stop, host loss to
  `Stopped::Failed`, suspend then resume to the same life, respawn after
  death. Receipt:
  `docs/verse/verification/2026-10-05-phone-chamber/README.md`.
- Not done here: the 20-actor frame-time run on a phone. The Android
  emulator booted, but the build machine's disk filled during the
  OpenAgents Android release build. The steps are in `NEEDS_OWNER.md`,
  "The phone in the chamber (#10586)", with the iOS and physical-phone
  fight against a desktop player.
- `verse_app::tests::zone_transition_clears_input_and_keeps_plaza_identity_out_of_ruins`
  fails on `main` because Ruins has had a presence session since #10604;
  filed as #10633.

Gotchas met on this slice:

- The OpenAgents mobile workspace has its own `Cargo.lock`
  (`crates/openagents-mobile/Cargo.lock`). A dependency change in
  `coder-mobile` or `verse` needs
  `cargo metadata --manifest-path crates/openagents-mobile/Cargo.toml --offline`
  and a commit of that lockfile, or the locked phone builds refuse to start.
- Another agent edits `remote_window` movement batching; such changes now
  belong in `chamber_session.rs` (`send_movement_interval`) and its test.
- The phone path is the OpenAgents app (`crates/openagents-mobile`,
  `bins/openagents-android/host`, `bins/openagents-ios/host`), which mounts
  the bare Grid. Coder's Android app mounts the full Verse and has no arch.
- Disk on the shared Mac runs out under parallel builds; check `df -h`
  before an Android release build (it needs well over 10 GB).

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
  pose-age receipts from each client. The phone's chamber visits already
  write `chamber-frames.json`; the Grid itself has no phone frame-time file
  yet.
- Owner-planned, leave alone: #10552 REACH/NIP-HOST chamber discovery,
  #9832 mainnet LSPS4 test, #9806 iOS HDR.

## Environment notes

On the owner's Mac (Claude Code sessions):

- Commits go straight to `main`; rebase before each push and check that the
  pushed commits touch only your files.
- `CARGO_TARGET_DIR=$HOME/work/openagents-target-<slot>`; the rustc
  wrapper is kache. The Android SDK is at
  `/opt/homebrew/share/android-commandlinetools` (NDK 27.1.12297006,
  AVD `coder_mobile_api35`, arm64); start it with `-no-window -gpu
  swiftshader_indirect`. An iOS 26.5 simulator is available.

On the Devin build box:

- `PROTOC=/home/ubuntu/protoc/bin/protoc CARGO_TARGET_DIR=/home/ubuntu/target-oa`
  on every cargo command.
- Android: SDK at `/home/ubuntu/android-sdk`, NDK 27.1.12297006,
  `cargo-ndk` installed, AVD Pixel 6 API 35 (`emulator-5554`), Maven
  Central rate limits this IP (Gradle init script uses the Google mirror).
- iOS and macOS are unavailable here; use a macOS child session.
