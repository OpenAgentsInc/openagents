# Verse computer HUD and ordering work verification — September 27, 2026

This record covers
[#9732](https://github.com/OpenAgentsInc/openagents/issues/9732), the Verse
computer opening the linked-hosts Computers screens in the world HUD, and
[#9734](https://github.com/OpenAgentsInc/openagents/issues/9734), ordering and
following work on a linked host from the phone, within the
[linked-devices program, #9736](https://github.com/OpenAgentsInc/openagents/issues/9736).

## What changed

- **In-world screens.** Tapping the world computer's monitor opens a panel
  that Rust draws in the Verse HUD with the same atlas and amber ladder as
  the map and zone controls (`crates/coder-mobile/src/computer_hud.rs`). It
  lays out the Rust Native Computers view, hit-tests real touches on the
  world surface, scrolls long screens by drag, and captures every touch
  while open, so the world behind never moves. Tabs switch between
  **COMPUTERS**, **TERMINAL** (when one is open), and **CHATS**.
- **Chats as a secondary page.** **CHATS** shows the existing read-only
  reader, its `coder-pair:` pairing, and the world connection settings as
  before. Its **Computers** button returns to the HUD.
- **Native pieces only where needed.** The HUD asks the native host for the
  camera scanner (**SCAN QR**) or the keyboard (**PASTE** or **TYPE**) for
  the current input request. iOS forwards the reader worker's Computers and
  terminal views to the world, runs the HUD's commands, and exposes every
  laid-out control to accessibility as an element with the control's key.
- **Hosts, selection, enrollment.** Each host row has **Open**. The Host
  screen shows status and route (tailnet, LAN, relay), this device's rights,
  the workspaces the host shares, **Order work**, **Terminal**, **Access**,
  and its recent work. **Add a computer** enrolls by scanning or pasting a
  `coder-host:` invitation.
- **Ordering and following work.** NIP-HOST gains `workspace.list` (right
  `operate`, outcome `workspaces`). The order screen picks a listed
  workspace (or takes a typed name from an older host), asks for the prompt,
  and sends `task.create`. Activity shows each task's phase and revision;
  open tasks offer **Steer** (`task.steer`) and **Stop task** (`task.cancel`,
  confirmed) at the revision shown. The host's redacted summaries stream
  back into Activity.
- **Terminal entry point for #9733.** **Terminal** passes the shared
  authority check (online, `terminal` right), then `App` opens
  `crates/coder-mobile/src/terminal.rs::Terminal` for that host. Its Rust
  Native view is drawn on the HUD's **TERMINAL** page; taps return through
  `Request::TerminalActivate`. The placeholder names the host and offers
  **Back**; the NIP-TERM screen replaces `Terminal::view` and
  `Terminal::activate`.
- **Android.** The Android host keeps its native computer panel
  (`computer_hud` stays off in its Verse configuration). The shared Rust
  Computers screens, including Host, Order work, and Activity steer and stop,
  render there through its native renderer. The APK builds and its lint and
  JVM checks pass; no Android emulator run was made for this record.

## Evidence classes

Rust unit and integration tests ran on one macOS computer. The simulator
checks ran on a dedicated iPhone 17 Pro simulator (iOS 26.5) created for
this work. The live run used `coder host serve`'s command path
(`coder_host::cli::run`) with a real access store, two workspaces, and a
task owner that records revisions, over the synthetic authenticated NIP-42
relay on loopback. No physical device, tailnet, or production relay was
involved.

## Checks that ran

```sh
cargo test -p coder-access --no-default-features --test wire
cargo test -p coder-computers --features ssh -p coder-host
cargo test -p coder-mobile --lib
cargo clippy -p coder-computers --features ssh -p coder-access -p coder-host -p coder-mobile --all-targets -- -D warnings
scripts/build-coder-android.sh package
scripts/build-coder-android.sh check
```

Results: 4 NIP-HOST wire tests, 41 `coder-computers` unit tests plus its
SSH live and edit tests, every `coder-host` suite including the end-to-end
test, and 77 `coder-mobile` tests passed. New tests cover the HUD's layout,
tap resolution, drag scrolling, multi-touch and cancel refusal, input
commands, terminal page, invalid feeds, the scene's touch capture, the
order, steer, and stop flow over the synthetic service, and
`the_app_orders_steers_and_stops_a_task_on_a_real_host` against a served
host.

On the simulator, `ComputersUITests` (synthetic fixture: statuses, host
selection, ordering, steering, stopping, the terminal entry point, pasted
enrollment, the Chats page, and a masked owner key) and
`ComputersLiveUITests` with `TEST_RUNNER_CODER_LIVE_ORDER=1` against the
served host passed. The live run pasted the host's invitation, saw the host
online, selected it, ordered a task in `scratch`, followed it in Activity,
steered it, and stopped it. The host's task owner received each operation:

```text
Fix the flaky parser test | scratch | revision 1 | Queued | 0 steer(s)
Fix the flaky parser test | scratch | revision 2 | Queued | 1 steer(s)
Fix the flaky parser test | scratch | revision 3 | Cancelled | 1 steer(s)
```

The regression suites that open the computer's Chats page (reader, world
connection, pairing, Verse, Gym, production launch, owner key) passed.
The reader and world-connection suites ran on the build before the last
HUD change, which only resets scroll for a new notice.

Retained evidence is in
[`bins/coder-ios/verification/2026-09-27-computer-hud/`](../../../bins/coder-ios/verification/2026-09-27-computer-hud/):
screenshots of the live and synthetic runs, the fixture's log, and the UI
test results.

To repeat the live run:

```sh
CODER_COMPUTERS_FIXTURE_DIR=/private/tmp/computers-run CODER_COMPUTERS_REVOKE_AFTER=never \
  cargo test -p coder-mobile --lib serve_a_host_for_a_device_run -- --ignored --nocapture
# In another shell, with the invitation the fixture wrote:
TEST_RUNNER_CODER_LIVE_INVITATION="$(cat /private/tmp/computers-run/invitation.txt)" \
TEST_RUNNER_CODER_LIVE_ORDER=1 xcodebuild ... -only-testing:CoderUITests/ComputersLiveUITests test
```

## Not covered

Physical iPhone checks, a tailnet route, the production relay, and a
TestFlight build remain with the program issue and
[#9724](https://github.com/OpenAgentsInc/openagents/issues/9724). Tasks
ordered from the phone are recorded on the host; running them needs the
host's own execution policy
([#9735](https://github.com/OpenAgentsInc/openagents/issues/9735)).
