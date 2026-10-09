# openagents-desktop

**OpenAgents** for desktop opens into chat: a sidebar of your saved chats, a
transcript, and an editable composer, on a plain background. The live Grid
has its own **Verse** page, opened from the sidebar's footer, on macOS and
Linux. Chats are hosted OpenAgents conversations that persist
across restarts; a coding request runs Coder on this computer
([local Coder](../../docs/desktop/local-coder.md)). The same shared Rust
(`openagents-chat`, `openagents-chat-app`) drives the phones' chat.
**Phones and computers** opens the pairing and computer controls. Their design is
[Connect a computer by scanning a QR code](../../docs/coder/design/2026-09-29-auto-pairing.md);
the screens are `DSK-01` to `DSK-03` in the
[wireframe](../../docs/product/2026-09-28-app-wireframe.md).

## What is built

Status on `main` after the 2026-09-30 landings. Each row links its
verification record; owner-only checks are in the workspace `NEEDS_OWNER.md`.

| Feature | Platforms | Limits | Record |
| --- | --- | --- | --- |
| Chat: persisted chats, streaming hosted replies, stop and retry, Markdown and code, search, pin, archive, rename, command palette | macOS, Linux, Windows | Text only: attachments are off as of 2026-10-01 ([#10095](https://github.com/OpenAgentsInc/openagents/issues/10095)). The composer has no attach control, a pasted or dropped image is dropped, and a dropped document puts only its path in the message | [closeout](../../docs/desktop/verification/2026-09-30-basic-chat-closeout/verification.md) |
| Coder from chat: live events, stop, steer, queue, questions, approvals, saved Codex and Claude Code sessions, engine and usage, **What changed** diff pane | macOS, Linux, Windows | Saved-session browsing reads `.codex` and `.claude` on macOS and Linux only; the diff pane is read-only | [local Coder](../../docs/desktop/local-coder.md) |
| Gym and evals in chat: the phone's cards and sheets, the shared hosted runner, runs kept in an encrypted store | Cards: macOS, Linux, Windows. Runner and saved runs: macOS, Linux | The trainer key is the Grid's world key, which Windows does not have (`chat_gym` is not built there) | [Gym cards](../../docs/desktop/verification/2026-09-30-gym-cards/verification.md) |
| Settings: Appearance (Theme, Reduce motion), Text size, Keyboard shortcuts, Notifications, Phones and computers, Archived chats | macOS, Linux, Windows | Theme follows the system by default, or holds Coder Light or dark (#11028); every window surface paints from it, while the Verse's scenes keep their art. | [settings](../../docs/desktop/verification/2026-09-30-settings/README.md) |
| Theme tokens and reduced motion (system setting or the app's switch) | macOS, Linux, Windows | | [theme tokens](../../docs/desktop/verification/2026-09-30-theme-tokens/verification.md) |
| Screen readers through AccessKit (VoiceOver, Orca, Narrator) | macOS, Linux, Windows | Checked headlessly and with a macOS AX probe; a person's screen-reader pass is an owner check | [accessibility](../../docs/desktop/verification/2026-09-30-accessibility/verification.md) |
| Native menu bar from the command registry | macOS | Linux and Windows keep the in-window menus | [menus, notifications, update](../../docs/desktop/verification/2026-09-30-menus-notify-update/verification.md) |
| Coder notifications (question, approval, finished, failed) whose click opens the chat | Linux (portal, else notification server), macOS (notification center, asked on the first notice), Windows (toasts) | macOS needs the app bundle; Windows needs the MSI's AppUserModelID registration. A click after the app quit opens nothing on macOS or Windows | [Linux](../../docs/desktop/verification/2026-09-30-linux-integration/README.md), [Windows](../../docs/desktop/verification/2026-09-30-windows-notifications/verification.md), [release](../../docs/desktop/release.md#releasing-for-windows) |
| **Update ready** strip with **Restart to update** | macOS, Linux, Windows | A development build never checks | [menus, notifications, update](../../docs/desktop/verification/2026-09-30-menus-notify-update/verification.md) |
| Slide viewer: a deck opened from chat (`presentation.open`) or `--open-deck ID`, animated open and close, full screen | macOS, Linux, Windows | Decks open only in the desktop app; the phone and terminal say so | [captures](../../docs/desktop/verification/2026-09-30-slide-viewer/slides-viewer.png), [route](../../docs/coder/measurements/2026-09-30-presentation-route.md) |
| The Verse page (sidebar footer): the Grid's Watch and Play, loaded only while the page shows | macOS, Linux | Verse does not build on Windows | [playable Grid](../../docs/desktop/verification/2026-09-30-playable-grid/verification.md) |
| The Map page (sidebar footer, palette, Window menu): OpenAgents' routes, members, plugins, and gaps as one zoomable graph colored by kind, with details, a Gaps panel, and an outline | macOS, Linux, Windows | Built only while it shows; the plugin records are a committed snapshot | [design](../../docs/desktop/route-map.md), [verification](../../docs/desktop/verification/2026-10-01-route-map/verification.md) |

Packages, all at 1.0.0: the signed, notarized macOS `.dmg`; the Linux
AppImage and `.deb`
([record](../../docs/desktop/verification/2026-09-30-linux-release/README.md));
and the Windows MSI and `.zip`, released without an Authenticode signature
(owner, 2026-10-09), so Windows asks people to confirm the first run (**More
info**, **Run anyway**)
([record](../../docs/desktop/verification/2026-09-30-windows/verification.md)).
They are offered on [openagents.com/download](https://openagents.com/download)
once the 1.0.0 release run publishes them
([release day](../../docs/desktop/release.md#release-day-100-11092)).

## The shell and sidebar

The shell reimplements Zeron's inset content pane, grouped chat list, and
anchored footer ([port audit](../../docs/research/2026-09-29-comet-desktop-ui-port-audit.md),
[#9993](https://github.com/OpenAgentsInc/openagents/issues/9993)).
[`src/chrome.rs`](src/chrome.rs) owns the typed navigation actions; the
chat window ([`src/chat.rs`](src/chat.rs)) fills the sidebar from the saved
chat list (`openagents_chat_app::chat_list`). The shell projects existing
Rust Native elements; desktop layout stays in the adapter.

- Drag the sidebar seam to resize it from 224 to 400 points.
- Click the main header's sidebar button, or press Ctrl+B (Cmd+B on macOS),
  to hide or restore it. The window retains its width and selected chat.
- Click a section to hide or reveal its chats. The list scrolls independently
  while the header and footer stay in place. Tab brings a focused row into view;
  Enter or Space activates it.
- **New chat**, or Ctrl+N (Cmd+N on macOS), opens a new chat at the top of
  Recent.
- **Map**, beside Verse in the footer, opens the route map
  ([design](../../docs/desktop/route-map.md)).
- **Verse** opens the world view. **Phones and computers** opens the
  computer controls, and **Settings** its six pages. Opening a chat cancels any
  displayed pairing code.

The window requests 90% of the usable display area, with a 1200×840-point
fallback and a minimum of 760×540. A tiling desktop controls its placement.
Views can grow to 1.6× their size at 1200×840 points.

The foreground retains its pixels between interactions and repaints changed
controls. Screen-lock checks run on a separate background thread, once a
second, so dragging does not start a subprocess on the UI thread.

| Screen | What it shows |
| --- | --- |
| `DSK-01` Connect a phone | The QR code, **Scan with the OpenAgents app on your phone.**, and **Can't scan? Copy a code instead**. No checkbox: every phone that pairs gets full permission (every NIP-HOST right, a terminal included). |
| `DSK-02` Connected | **Kai's iPhone is connected.**, **Pick a project for Coder** with **Choose folder…**, whether Codex and Claude Code (and Grok Build, when installed) are signed in, and **Let my phone start Coder here**. |
| `DSK-03` Home | **Online. Your phone can reach this Mac.** or **Offline.**, the phones with **Remove**, Coder's recent tasks, and **Connect another phone**. |

The screens are Rust Native views ([`src/screens.rs`](src/screens.rs)),
drawn by [`rust-native-desktop`](../rust-native-desktop/). The window process
holds no host or owner secret key: it talks to the host only over the local control socket
([`src/control.rs`](src/control.rs), `openagents-connect`'s protocol). The
host, `coder host serve --keychain --iroh --control`, runs as the login agent
the app registers with `SMAppService`, and reads its keys from the keychain
itself. Upgrading an old-style setup runs the bundled `coder` as a child
process, `coder host adopt detect` and then `coder host adopt`
([`src/migrate.rs`](src/migrate.rs)), so the keychain items are written by
the same program that reads them and macOS never asks to allow it.

When a code shows, rotates, and is cancelled is [`src/codes.rs`](src/codes.rs);
`INVARIANTS.md` (Linking devices) states the rules.

## Play the Grid

On macOS and Linux, open **Verse** and choose **Play** to join the same
`verse-bare` world as the phone. **Watch** returns to the spectator. Opening the
app or the Verse page starts no player. Leaving the page ends Play and
preserves your chat drafts and sidebar state.

- W/S move forward and back; A/D and Q/E strafe. Space jumps once per press;
  Shift sprints. Diagonals, backpedaling, collisions, and shared bodies use the
  phone's controller and physics.
- Hold the right mouse button to look and turn; hold the left button to orbit.
  Both buttons move forward. The wheel zooms through first person and back.
  Escape closes a board or releases the mouse. Click the world to resume.
- Walk into the Gym and click its GYM, RESULTS, or EVALS board. Native controls
  show the shared board state, result provenance and caveats, attempt traces,
  replay controls, and published evals. Long boards have numbered pages.
  Movement pauses while a board is open. **Compare notes** remains off until
  you choose it; its explanation states what the agent publishes.
- The Gym accepts a separately granted connection through **Open Gym connection
  file**. Copy its **world public key** when creating that grant. Review a
  recipe's revision and budget before **Confirm and launch this recipe**.
  Walking, opening a board, and public result reads grant no execution rights.

Play owns a dedicated world key, separate from host, owner, pairing, and chat
keys: service `com.openagents.desktop.verse`, accounts `world-key` and
`gym-connection`, in the macOS login Keychain or Linux Secret Service. Key
creation and reads happen off the UI thread, only after Play. An unreadable
stored identity is never replaced; Play stays offline and explains why.

### The keychain (#10096)

- **Never at launch.** Opening the app, a chat, or the Verse page reads
  nothing from the keychain. The world key is read only when you choose
  **Play**, or when the chat's Gym starts its first hosted run (the chat Gym
  signs hosted runs with it and opens its saved runs under it, so earlier
  runs come back with that first hosted start).
- **At most one prompt.** A read or write the keychain denies, cancels, or
  can't answer is remembered until the app restarts: nothing asks again,
  Play stays offline, and the Gym says in one line that hosted runs need the
  key.
- **Dev builds keep their own items.** Only the packaged release is built
  with `OPENAGENTS_DESKTOP_RELEASE=1` (set by
  [`scripts/desktop/package-macos.sh`](../../scripts/desktop/package-macos.sh)
  and [`package-linux.sh`](../../scripts/desktop/package-linux.sh); see
  [`docs/desktop/release.md`](../../docs/desktop/release.md)). Every other
  build, `cargo build` and `cargo build --release` included, is a dev build
  (`openagents_desktop::RELEASE` is false): it keeps its world key and Gym
  connection under `com.openagents-dev.desktop.verse`, never reads or prompts
  for the signed app's `com.openagents.desktop.*` items, and never runs
  `coder host adopt`. A dev build outside an app bundle registers no login
  agent on macOS either, so nothing it starts reads the host's keys.
The public result cache and Compare notes preference live under
`~/.openagents/desktop/grid/`; these files contain no secret key or Gym grant.

[`coder_mobile::verse_surface`](../coder-mobile/src/verse_surface.rs) exposes
the phone's Rust scene without its touch or platform bridge. The desktop uses
that scene for signed spawn recovery, peer interpolation, the ball and blocks,
and board admission. New players use the phone's spawn; returning players may
restore only their own signed pose. Presence uses the phone's 3-second moving,
5-second idle, and 30-second state cadence, with the same 54-event/minute budget
and 1.5-second moving-body interval. Frame timing remains independent.

The interactive GPU viewport uses the window's device at full resolution,
without spectator dimming or blur. Its loop requests 60 Hz and bounds long
frame gaps. Focus loss, hidden windows, modals, and navigation release input
and pause or end the session. Reduce motion affects Watch; deliberate Play
still moves. If cursor capture fails, keyboard movement and wheel zoom remain
available. `--no-backdrop` disables Watch's background and still allows Play.
Windows shows an unsupported state: Verse does not build there, and Windows
chat parity ([#10027](https://github.com/OpenAgentsInc/openagents/issues/10027))
shipped without it.

`--fake-host` uses an offline player fixture and reads committed results;
it creates no world key and joins no relay as a player. See the
[verification record](../../docs/desktop/verification/2026-09-30-playable-grid/verification.md)
for captures, test commands, timing scope, and remaining native device checks.

## The Verse page

The **Verse** button in the sidebar's footer, beside the Local profile and
Settings, opens the Verse page. In Watch mode it shows the Grid, the
OpenAgents app's Verse world, seen live from above
([#9982](https://github.com/OpenAgentsInc/openagents/issues/9982),
[`src/backdrop.rs`](src/backdrop.rs)); **Play** joins it. The world exists
only on that page ([#10071](https://github.com/OpenAgentsInc/openagents/issues/10071),
[`src/grid.rs`](src/grid.rs)): chat and every other page have a plain
background, nothing of the world is loaded, connected, or drawn there, and
leaving the page closes the relay connection and releases the world's GPU
resources. Other players walk around the plaza,
the ball, the blocks, and the Gym as the phones draw them, and the camera
sways slowly over it. With nobody there the Grid is empty; nothing is
invented.

- **Watch is a spectator.** `verse::spectator` subscribes to the
  Grid's pose frames and entity states (`verse-bare` on
  `wss://relay.openagents.com`) and has no way to publish: no avatar, no
  presence, no input to the world, no chat, no name. If the relay asks for
  authentication it answers with a fresh key kept in memory for that one
  connection. Nobody sees the desktop or counts it.
- **The page is the world** ([#10116](https://github.com/OpenAgentsInc/openagents/issues/10116)).
  The world fills the whole content area, right of the sidebar and from
  the title bar to the bottom, and resizes with the window; Watch and Play
  draw it sharp, at the window's pixels, on the window's own `wgpu` device
  (`rust_native_desktop::backdrop`, which draws into the page's node by
  its key, `Scene::backdrop_rect`). The controls lie over it in small dark
  chips: **Watch**, **Play**, the status line, and **Full screen** at the
  top, an open board as a card in the middle (the wheel scrolls it), and
  the key hint, dim, at the bottom.
- **Full screen.** **Full screen**, Ctrl+Cmd+F (macOS's full-screen
  shortcut), or F11 hides the sidebar and title bar, lets the world cover
  the window, and puts the window in full screen; the window's own
  full-screen control on the Verse page does the same. Esc leaves once
  the world has no use for it: it first closes an open board, then
  releases a held mouse, and the next Esc leaves full screen. Leaving the
  page leaves full screen too. Play keeps the mouse in full screen.
- **Cost.** Nothing on any other page. On the Verse page, 30 frames a
  second while someone is in the Grid or the ball or a block moves, 10 while
  it is empty and settled, none while the window is hidden or minimized;
  after 20 seconds hidden the relay connection closes.
  With **Reduce motion** on, the camera stops and one still frame is drawn
  each time the window shows. Measured on an Apple M5 Max, release
  build, window at its default size, CPU of one core over 30 seconds: 7.2%
  with three players walking, 3.0% with the Grid empty, 0.03% minimized,
  and 0.03% for the window without a backdrop. Those figures were measured
  when the Grid sat behind every page; chat now costs what the window without
  a backdrop costs ([measurement](../../docs/desktop/verification/2026-10-01-chat-polish/verification.md)).
- **Choosing.** `--verse-relay URL` (or `OPENAGENTS_VERSE_RELAY`) watches
  another relay; `--no-backdrop` shows no world in Watch and keeps Play
  available. Windows has no Verse world yet: Verse does not build there.

## Run it

```sh
# An in-process host; open Phones and computers, then Connect another phone.
cargo run --release -p openagents-desktop -- --fake-host --fake-scan 8

# Five phones already connected; the window opens on Phones and computers.
cargo run --release -p openagents-desktop -- --fake-host --fake-phones 5

# The window against this Mac's host (its control socket).
cargo run --release -p openagents-desktop -- --no-login-agent

# Every screen as PNG files (without the Verse world).
cargo run -p openagents-desktop -- --capture /tmp/openagents-desktop

# The Verse page with simulated players: an in-process relay and three
# walkers, then the window watching that relay (open Verse in the sidebar).
cargo run -p verse --no-default-features --example grid_walkers -- 3
cargo run --release -p openagents-desktop -- --fake-host --verse-relay ws://127.0.0.1:PORT

# The Grid alone, as a PNG, watching a relay for ten seconds.
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

On Linux the host runs as the systemd user unit
`com.openagents.desktop.host.service`. From an AppImage the unit runs the
AppImage; from the .deb it runs `/usr/lib/openagents/coder`. From anywhere
else, such as a development build in `target/release`, it runs a copy of
`coder` and `microcoder` in `~/.openagents/host/bin/<sha256>/`, named by
their contents, so the next build never replaces the file under the running
host: quit the window and open it again to move the host onto a new build.
A settings change makes the host start itself again; the window shows the
chosen folder with **Saving…**, waits up to 30 seconds for it to answer, and
finishes a folder swap that failed partway the next time the host answers.

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

The app's window, captured with `screencapture -l` before the Grid moved to
its own page (#10071), against the in-process host with the Grid behind it,
three simulated players walking there
(`grid_walkers`): [connect](screenshots/dsk-01-connect.png),
[connected](screenshots/dsk-02-connected.png),
[home](screenshots/dsk-03-home.png),
[remove](screenshots/dsk-03-remove.png), and
[connect with the Grid empty](screenshots/dsk-01-empty-grid.png).
