# Mobile terminal screen verification

Issue [#9733](https://github.com/OpenAgentsInc/openagents/issues/9733), part
of the linked-devices program
[#9736](https://github.com/OpenAgentsInc/openagents/issues/9736). Recorded on
2026-09-27 on one Mac (macOS 26.4, Apple silicon) with the pinned Rust
toolchain and Xcode's iOS 26.5 simulator runtime.

## What was built

- `crates/coder-vt`: the terminal emulator, with `vte` as its parser.
- `crates/coder-computers/src/terminal`: the model, the Rust Native view in
  the amber palette with the accessory key row, and the live session over
  NIP-HOST `terminal.open` and NIP-TERM attach, input, resize, and close.
- `crates/coder-mobile/src/terminal.rs` (now `crates/coder-computers/src/terminal/screen.rs`): the terminal screen behind the Host
  screen's **Terminal** control (the entry point #9732/#9734 added), and the
  `terminal_poll`, `terminal_resize`, `terminal_text`, `terminal_key`, and
  `terminal_paste` requests, answered with a smaller
  `coder.mobile.terminal.v1` packet so polling never resends the other views.
- `crates/coder-mobile/src/computer_hud.rs`: the world computer's HUD draws
  the terminal view on its **TERMINAL** page: each grid row on one line in
  the atlas's monospaced cells, runs in their own colors, box-drawing
  characters as lines; it asks for the grid its page fits
  (`terminal_resize`), keeps the page above the software keyboard, and adds a
  **KEYBOARD** control.
- `rust-native`: a `terminal` text role (one unwrapped monospaced row or run),
  drawn by the SwiftUI and Android renderers too.
- `coder-host`: `Link::generation` (relay routes carry the presence
  generation) and `Ordered::push_frames`, which returns each applied frame so
  an emulator can read its output.
- iOS: `TerminalKeyboard.swift` (a `UIKeyInput` keyboard target with
  hardware-key handling, and rotation only while the terminal page shows),
  terminal polling and clipboard reads in `MobileBridge` and `VerseScreen`,
  and the keyboard height sent to the HUD from `VerseSurface`.

## Rust checks

```sh
cargo test -p coder-vt                          # 42 passed
cargo test -p coder-computers --features ssh    # 53 unit, 3 edits, 2 live passed
cargo test -p coder-mobile                      # 82 passed; the two device-run fixtures are ignored
cargo test -p coder-host                        # unit, cj, end_to_end, headless, serve, websocket, wss passed
cargo test -p rust-native -p coder-terminal
cargo clippy -p coder-vt -p coder-computers -p coder-host -p coder-mobile \
  -p rust-native -p coder-terminal --all-targets -- -D warnings
cargo clippy -p coder-computers --features ssh --all-targets -- -D warnings
cargo clippy -p coder-computers --no-default-features --lib -- -D warnings
cargo clippy -p coder-host --no-default-features --lib -- -D warnings
```

The emulator tests cover each supported area, a typical shell and full-screen
session, and 20,000 steps of arbitrary bytes and sequence fragments across
resizes on grids from 1×1 to 24×80 that must keep the grid well formed.

`crates/coder-mobile/src/terminal_live_tests.rs` runs the application state
the way a native host drives it, against real shells on real PTYs:

- `the_app_opens_a_shell_on_a_real_host_and_runs_commands`: `coder host
  serve` (the command path, `coder_host::cli::run`) on the local NIP-42 relay
  fixture. The app enrolls by pasted invitation, opens the Host screen, taps
  **Terminal**, reports 24×60, and reads `terminal-42` from the grid after
  typing `echo terminal-$((6*7))`; `pwd` shows the admitted workspace; an
  unchanged screen is not resent; `stty size` reports `20 50` after a resize;
  the key row's ^C interrupts `sleep 30 && echo slept` within 20 seconds;
  latched Ctrl then `u` erases a typed line; a paste runs; **End terminal**
  reports `Terminal closed.`; **Back** closes the screen.
- `a_host_restart_leaves_the_terminal_lost`: after the host restarts with a
  new generation, the screen shows `Lost…`, keeps its output, offers **Open a
  new terminal**, and a new shell runs on the restarted host.
- `a_device_without_the_terminal_right_is_refused_clearly`: a device holding
  only `observe` and `operate` sees **Terminal** disabled with the reason
  naming the "Open terminals" right.

Session unit tests cover frame ordering with a held frame, a gap marked in the
output and counted, duplicates, exit, cursor-position replies sent back as
input, revocation, and the refusal-to-phase mapping. The HUD test checks rows
in monospaced cells, colored runs placed by column, box drawing, one resize
per grid change (smaller above a keyboard, wider on a wider surface), and the
**KEYBOARD** and key-row controls.

## iOS Simulator run

A dedicated simulator, **Coder Terminal 9733** (iPhone 17 Pro, iOS 26.5,
`AEA53266-689D-4FBC-A179-5E872D83DDB9`), created for this run; no shared
booted simulator was used. The app was built with
`scripts/build-coder-mobile.sh sim-build` (debug Rust library, the app
uninstalled before each run), and the ignored fixture
`serve_a_host_for_a_terminal_run` served a host with a real shell on this Mac:

```sh
CODER_TERMINAL_FIXTURE_DIR=/private/tmp/terminal-run \
  cargo test -p coder-mobile --lib serve_a_host_for_a_terminal_run -- --ignored --nocapture
```

`xcodebuild test -only-testing:CoderUITests/TerminalUITests` ran with
`TEST_RUNNER_CODER_TERMINAL_INVITATION` set to the fixture's
`invitation.txt` and `TEST_RUNNER_CODER_TERMINAL_FIXTURE_DIR` to its
directory. `testOpenAShellRunCommandsRotateAndSeeLostAfterRestart` passed in
45.1 seconds ([summary](2026-09-27-mobile-terminal/ui-test-summary.log)) with the terminal in the world computer's HUD: enrollment by pasted
invitation, **Open**, **Terminal**, `Connected directly`, **KEYBOARD** and
typing on the software keyboard, the Ctrl latch, ^C, **Up** recalling a
command, `stty size` growing to 89 columns in landscape, the fixture's host
restart shown as `Lost`, a new terminal after **Open a new terminal**, and
**Back** to the host screen. The same test passed twice earlier against a
SwiftUI panel drawing the same Rust view, before #9732 moved Computers into
the HUD.

In the same build, `ComputersUITests` (both tests, the synthetic HUD
fixture) passed.

Screenshots from the HUD run:

| Step | Evidence |
| --- | --- |
| Attached | [1-attached.png](2026-09-27-mobile-terminal/1-attached.png) |
| Command output | [2-command-output.png](2026-09-27-mobile-terminal/2-command-output.png) |
| Key row, ^C, history | [3-accessory-keys.png](2026-09-27-mobile-terminal/3-accessory-keys.png) |
| Landscape, 89 columns | [4-landscape.png](2026-09-27-mobile-terminal/4-landscape.png) |
| Lost after the host restarted | [5-lost-after-restart.png](2026-09-27-mobile-terminal/5-lost-after-restart.png) |
| New terminal after the restart | [6-new-terminal.png](2026-09-27-mobile-terminal/6-new-terminal.png) |

## Limits

- Simulator and one Mac only: no physical iPhone, no second machine, no
  tailnet route, and no relay-only terminal in the simulator. The relay
  terminal path is exercised only by the existing `coder-host` tests.
- The relay route asks for 16 KiB/s and sends each keystroke as a sealed
  artifact over a new relay connection, so typing is slow there.
- In landscape with the software keyboard up, the page fits only two rows.
- Android shares the Rust (`coder_mobile` answers the terminal requests
  through its JNI call) and its renderer draws the `terminal` role
  monospaced on one line, but it has no terminal keyboard or grid sizing.
  `scripts/build-coder-android.sh package` (arm64-v8a) built the APK; no
  emulator ran.
- The grid shows the visible screen only; scrollback is kept in the emulator
  but not drawn. There is no text selection or mouse reporting. The HUD
  atlas lacks glyphs outside Latin-1 other than the drawn box lines, so such
  characters show as `?`.
- The iOS host polls the terminal every 120 ms while its page shows.
- The TestFlight build is the coordinator's; nothing was uploaded.
