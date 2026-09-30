# openagents-desktop

**OpenAgents** for desktop opens into a chat shell with a sidebar and the live
Grid behind its main pane. The sidebar currently contains sample chats;
selecting or creating one changes only the window's presentation state.
**Phones and computers** opens the existing pairing and computer controls.
Their design is
[Connect a computer by scanning a QR code](../../docs/coder/design/2026-09-29-auto-pairing.md);
the screens are `DSK-01` to `DSK-03` in the
[wireframe](../../docs/product/2026-09-28-app-wireframe.md).

## The shell and sidebar

The shell reimplements Zeron's inset content pane, grouped chat list, and
anchored footer ([port audit](../../docs/research/2026-09-29-comet-desktop-ui-port-audit.md),
[#9993](https://github.com/OpenAgentsInc/openagents/issues/9993)).
[`src/chrome.rs`](src/chrome.rs) owns its bounded sample data and typed
navigation actions. It projects existing Rust Native elements; desktop layout
stays in the adapter.

- Drag the sidebar seam to resize it from 224 to 400 points.
- Click the main header's sidebar button, or press Ctrl+B (Cmd+B on macOS),
  to hide or restore it. The window retains its width and selected chat.
- Click a section to hide or reveal its chats. The list scrolls independently
  while the header and footer stay in place. Tab brings a focused row into view;
  Enter or Space activates it.
- **New chat**, or Ctrl+N (Cmd+N on macOS), adds a sample chat for this window.
  Closing the window discards sample navigation and chats.
- **The Grid** opens the world view. **Phones and computers** and **Settings**
  lead to the existing computer controls. Opening a chat cancels any displayed
  pairing code.

The window requests 90% of the usable display area, with a 1200×840-point
fallback and a minimum of 760×540. A tiling desktop controls its placement.
Views can grow to 1.6× their size at 1200×840 points. The shell
retains the current painter and backdrop; it does not implement a transcript
or editable composer yet.

The foreground retains its pixels between interactions and repaints changed
controls. Screen-lock checks run on a separate background thread, once a
second, so dragging does not start a subprocess on the UI thread.

| Screen | What it shows |
| --- | --- |
| `DSK-01` Connect a phone | The QR code, **Scan with the OpenAgents app on your phone.**, and **Can't scan? Copy a code instead**. No checkbox: every phone that pairs gets full permission (every NIP-HOST right, a terminal included). |
| `DSK-02` Connected | **Kai's iPhone is connected.**, **Pick a project for Coder** with **Choose folder…**, whether Codex and Claude Code are signed in, and **Let my phone start Coder here**. |
| `DSK-03` Home | **Online. Your phone can reach this Mac.** or **Offline.**, the phones with **Remove**, Coder's recent tasks, and **Connect another phone**. |

The screens are Rust Native views ([`src/screens.rs`](src/screens.rs)),
drawn by [`rust-native-desktop`](../rust-native-desktop/). The window process
holds no secret key: it talks to the host only over the local control socket
([`src/control.rs`](src/control.rs), `openagents-connect`'s protocol). The
host, `coder host serve --keychain --iroh --control`, runs as the login agent
the app registers with `SMAppService`, and reads its keys from the keychain
itself. Upgrading an old-style setup runs the bundled `coder` as a child
process, `coder host adopt detect` and then `coder host adopt`
([`src/migrate.rs`](src/migrate.rs)), so the keychain items are written by
the same program that reads them and macOS never asks to allow it.

When a code shows, rotates, and is cancelled is [`src/codes.rs`](src/codes.rs);
`INVARIANTS.md` (Linking devices) states the rules.

## The backdrop

Behind every screen is the Grid, the OpenAgents app's Verse world, seen live
from above ([#9982](https://github.com/OpenAgentsInc/openagents/issues/9982),
[`src/backdrop.rs`](src/backdrop.rs)). Other players walk around the plaza,
the ball, the blocks, and the Gym as the phones draw them, and the camera
sways slowly over it. With nobody there the Grid is empty; nothing is
invented.

- **A spectator, never a player.** `verse::spectator` subscribes to the
  Grid's pose frames and entity states (`verse-bare` on
  `wss://relay.openagents.com`) and has no way to publish: no avatar, no
  presence, no input to the world, no chat, no name. If the relay asks for
  authentication it answers with a fresh key kept in memory for that one
  connection. Nobody sees the desktop or counts it.
- **Behind the views.** The world is drawn by the Verse renderer on the
  window's own `wgpu` device, at half the window's pixels, then blurred a
  little and covered by the window's black at 55 percent
  (`rust_native_desktop::backdrop`). The views are painted over it
  unchanged, so the QR code's white square and every word keep full
  contrast. Tests read the code back from a frame over an all-white,
  a finely lined, and a noisy backdrop, and from the committed window
  capture.
- **Cost.** 30 frames a second while someone is in the Grid or the ball or
  a block moves, 10 while it is empty and settled, none while the window is
  hidden or minimized; after 20 seconds hidden the relay connection closes.
  With **Reduce motion** on, the camera stops and one still frame is drawn
  each time the window shows. Measured on an Apple M5 Max, release
  build, window at its default size, CPU of one core over 30 seconds: 7.2%
  with three players walking, 3.0% with the Grid empty, 0.03% minimized,
  and 0.03% for the window without a backdrop.
- **Choosing.** `--verse-relay URL` (or `OPENAGENTS_VERSE_RELAY`) watches
  another relay; `--no-backdrop` shows the plain black window. Windows has
  no backdrop yet: Verse does not build there.

## Run it

```sh
# An in-process host; open Phones and computers, then Connect another phone.
cargo run --release -p openagents-desktop -- --fake-host --fake-scan 8

# The window against this Mac's host (its control socket).
cargo run --release -p openagents-desktop -- --no-login-agent

# Every screen as PNG files (without the backdrop).
cargo run -p openagents-desktop -- --capture /tmp/openagents-desktop

# The backdrop with simulated players: an in-process relay and three
# walkers, then the window watching that relay.
cargo run -p verse --no-default-features --example grid_walkers -- 3
cargo run --release -p openagents-desktop -- --fake-host --verse-relay ws://127.0.0.1:PORT

# The backdrop alone, as a PNG, watching a relay for ten seconds.
cargo run -p verse --no-default-features --features capture \
  --example overlook_capture -- /tmp/grid.png 0 wss://relay.openagents.com 10
```

### The macOS app

```sh
bins/openagents-desktop-macos/bundle.sh
open target/release/OpenAgents.app
```

The app's version is the phone app's, in lockstep: `MARKETING_VERSION` in
[`bins/openagents-ios/host/project.yml`](../../bins/openagents-ios/host/project.yml)
is the source of truth, and this crate's `version` must equal it
([`tests/version_lockstep.rs`](tests/version_lockstep.rs); see
[release](../../docs/desktop/release.md#version)). 1.0.0 is the first such
release; 0.1.0 installs update to it.

The script builds `openagents-desktop`, `coder`, and `microcoder` for this
Mac and assembles `OpenAgents.app` with the login agent's plist, signed ad
hoc. `SKIP_CODER=1` leaves the Coder binaries out. The signed, notarized
`.dmg` is `scripts/desktop/package-macos.sh` ([release](../../docs/desktop/release.md)).

There is one flow. On a computer that already runs Coder from an earlier
setup (`~/.openagents/coder-access/`, a launchd agent or systemd unit that
runs `coder host serve`), the app upgrades it silently on launch, with no
question: the keys move into the keychain (each read back before its file
goes), the old agent and any other agent serving a host stop and are
removed, `service.json` becomes `service.adopted.json`, and the app
registers its own agent on the same state, so every paired phone keeps
working. If a safety check refuses, or the bundled `coder` doesn't read the
keychain, the earlier setup keeps running as it was, the reason goes to the
log, and the normal screens show one quiet line. On a Linux desktop with no
Secret Service the keys go to `~/.openagents/host-keys` instead. Tasks the
owner archived never show on the home screen.

## Test it

```sh
cargo test -p openagents-desktop
cargo test --release -p openagents-desktop --bin openagents-desktop \
  shell_paint_benchmark -- --ignored --nocapture
UPDATE_SNAPSHOTS=1 cargo test -p openagents-desktop   # record a screen change
git diff crates/openagents-desktop/snapshots
```

The timing test uses the real shell with warm font caches. It reports layout
and foreground paint times for hover, scrolling, and sidebar resizing at 1×
and 2× scale. It excludes GPU rendering and display latency.

## Screenshots

The app's window, captured with `screencapture -l`, against the in-process
host with the Grid behind it, three simulated players walking there
(`grid_walkers`): [connect](screenshots/dsk-01-connect.png),
[connected](screenshots/dsk-02-connected.png),
[home](screenshots/dsk-03-home.png),
[remove](screenshots/dsk-03-remove.png), and
[connect with the Grid empty](screenshots/dsk-01-empty-grid.png).
