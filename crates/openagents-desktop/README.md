# openagents-desktop

**OpenAgents** for Mac: the desktop app a person installs so their phone can
connect to this computer by scanning a QR code. The design is
[Connect a computer by scanning a QR code](../../docs/coder/design/2026-09-29-auto-pairing.md);
the screens are `DSK-01` to `DSK-03` in the
[wireframe](../../docs/product/2026-09-28-app-wireframe.md).

| Screen | What it shows |
| --- | --- |
| `DSK-01` Connect a phone | The QR code, **Scan with the OpenAgents app on your phone.**, **Let this phone open a terminal on this Mac** (off), and **Can't scan? Copy a code instead**. |
| `DSK-02` Connected | **Kai's iPhone is connected.**, **Pick a project for Coder** with **Choose folder…**, whether Codex and Claude Code are signed in, and **Let my phone start Coder here**. |
| `DSK-03` Home | **Online. Your phone can reach this Mac.** or **Offline.**, the phones with **Remove**, Coder's recent tasks, and **Connect another phone**. |
| Adoption | **Use this Mac's existing Coder setup?** on a Mac set up the old way. |

The screens are Rust Native views ([`src/screens.rs`](src/screens.rs)),
drawn by [`rust-native-desktop`](../rust-native-desktop/). The window process
holds no secret key: it talks to the host only over the local control socket
([`src/control.rs`](src/control.rs), `openagents-connect`'s protocol). The
host, `coder host serve --keychain --iroh --control`, runs as the login agent
the app registers with `SMAppService`, and reads its keys from the keychain
itself. Adopting an old-style setup runs the bundled `coder` as a child
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
# The window against an in-process host; a phone "scans" after 8 seconds.
cargo run -p openagents-desktop -- --fake-host --fake-scan 8

# The window against this Mac's host (its control socket).
cargo run -p openagents-desktop -- --no-login-agent

# Every screen as PNG files (without the backdrop).
cargo run -p openagents-desktop -- --capture /tmp/openagents-desktop

# The backdrop with simulated players: an in-process relay and three
# walkers, then the window watching that relay.
cargo run -p verse --no-default-features --example grid_walkers -- 3
cargo run -p openagents-desktop -- --fake-host --verse-relay ws://127.0.0.1:PORT

# The backdrop alone, as a PNG, watching a relay for ten seconds.
cargo run -p verse --no-default-features --features capture \
  --example overlook_capture -- /tmp/grid.png 0 wss://relay.openagents.com 10
```

### The macOS app

```sh
bins/openagents-desktop-macos/bundle.sh
open target/release/OpenAgents.app
```

The script builds `openagents-desktop`, `coder`, and `microcoder` for this
Mac and assembles `OpenAgents.app` with the login agent's plist, signed ad
hoc. `SKIP_CODER=1` leaves the Coder binaries out. The signed, notarized
`.dmg` is `scripts/desktop/package-macos.sh` ([release](../../docs/desktop/release.md)).

On a Mac that already runs Coder from an earlier setup
(`~/.openagents/coder-access/`), the app does not register its own agent; it
asks whether to use that setup, and offers **Use it** only once the bundled
`coder` reads its keys from the keychain (`coder host help` names it). With
an older `coder` it still sees the setup, from the access store alone, and
leaves it running.

## Test it

```sh
cargo test -p openagents-desktop
UPDATE_SNAPSHOTS=1 cargo test -p openagents-desktop   # record a screen change
git diff crates/openagents-desktop/snapshots
```

## Screenshots

The app's window, captured with `screencapture -l`, against the in-process
host with the Grid behind it, three simulated players walking there
(`grid_walkers`): [connect](screenshots/dsk-01-connect.png),
[terminal allowed and code copied](screenshots/dsk-01-terminal-copied.png),
[connected](screenshots/dsk-02-connected.png),
[home](screenshots/dsk-03-home.png),
[remove](screenshots/dsk-03-remove.png), and
[connect with the Grid empty](screenshots/dsk-01-empty-grid.png). On a Mac
set up the old way, from before the backdrop:
[the question](screenshots/earlier-setup.png),
[home with Coder's real tasks](screenshots/dsk-03-earlier-setup.png).
