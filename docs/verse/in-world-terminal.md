# In-world terminal

Status: research and specification, October 4, 2026. Nothing here is
implemented by this document. The owner asked whether the Coder terminal app,
which uses Ratatui, or a similar one can be the terminal of Verse zones:
"Say I'm in my workshop and want to pop open a terminal: I see a partial
overlay of the screen, can multiplex it, split it into different things,
share some with others." The owner also wants a demo soon: a terminal in the
app, typing into it, connected to one or more computers, possibly
multiplexed.

This page answers whether the existing code can do that, recommends a
rendering approach, specifies the overlay, multiplexing, sharing, input,
performance, and security design, lists what exists and what is missing with
effort estimates, and orders the work from a one-terminal demo to shared
terminals on in-world screens.

The [smart terminal](../terminal/smart-terminal.md) builds on this overlay:
blocks, natural-language requests as threads, and host-owned sessions.

Delivery update, October 5: the [workbench roadmap](../terminal/workbench-roadmap.md)
now governs the next slices. Today's target is the same real terminal
application in the desktop Grid's `T` overlay and a standalone install,
with shell blocks and request/proposal threads. The next pass connects it
to Everglade's existing studio resources. A desk monitor or floating window
is an entry point, not another terminal implementation. Physical placement
and sharing on world screens remain separate choices. The phased plan below
retains the earlier rendering research; the new roadmap owns delivery order.

**Correction, October 5, 2026.** The owner's starting point is OpenAgents
Terminal (`crates/openagents-terminal`, launched as `openagents terminal`),
not the Coder terminal app, and the look is the white ladder of
`coder_ui::theme::Intensity` on the near-black field, not amber. Where this
page says Coder terminal or amber, read OpenAgents Terminal and the white
ladder. Like `coder`, OpenAgents Terminal is a Ratatui program, so it runs in
a pane on a PTY and draws through `coder-vt` unchanged.

## The desktop demo

Implemented on October 5, 2026, in `crates/verse/src/terminal` (desktop
`terminal` feature; never in the web build). `T` opens the overlay over any
zone. The first pane runs the login shell. zsh integration uses temporary
startup files and preserves the user's dotfiles, editor, and history. While the
overlay has focus every key goes to the focused pane, `Esc` included, so
`vim` works; `` Ctrl+` ``, `Cmd+T`, or `Ctrl+B` then `Esc` gives focus back to
the world, and a click on a pane focuses it. `Ctrl+B` is a tmux-style
prefix: `%` and `"` split, the arrow keys move focus, `x` closes, `z` zooms,
`c`, `n`, and `p` open and switch tabs, and `o` opens a pane running
OpenAgents Terminal. The mouse wheel or `Shift+PageUp` scrolls back. Panes
run on PTYs of this computer through an in-process `coder-pty` host instead
of the resident host and NIP-TERM, which is the next phase. Hiding the
overlay keeps the sessions; quitting Verse ends their process groups.
`cargo run -p verse --example terminal_capture -- out.png` renders it.

`Ctrl+B`, then `a` opens Ask even over a full-screen program. At a zsh
prompt, `# ` selects Request before Enter; other input stays in Shell.
The request preview shows directory, Git status, and a scrubbed selected or
failed block. `Ctrl+D` removes its context; `PageUp` and `PageDown` scroll it.
Enter submits once and opens the same thread in an observing
`openagents terminal --thread ID --observe` split. Shell proposals remain
pending until Enter; commands that may change the computer require a second
Enter. The resulting command block returns to the same thread without
replaying an uncertain command. `Ctrl+B`, then `j` or `k` navigates blocks,
`y` copies, `d` collapses output, and `r` types a command for a new run.

The Grid overlay and the standalone `openagents-terminal` window mount the same
`terminal-core` application and `terminal-gfx` renderer. The core receives session,
request, clipboard, and link services through an injected interface; it has no
window, renderer, network, or Verse dependency. The standalone does not link the
Verse application or world. Native packages support macOS arm64; Linux is a
development platform. zsh supports request hooks; other shells retain normal PTY
input and can use the explicit Ask action. Hide/reopen preserves local panes;
exiting either app ends its local process groups. The chat-only `openagents terminal`
command remains separate.

Build the native package on the Mac with
`scripts/release/native-terminal.py build --commit origin/main`, setting an
absolute, reusable `CARGO_TARGET_DIR` outside the checkout. It bundles the GUI,
`openagents`, and `microcoder` from that commit under `OpenAgents Terminal.app`.
The separate `openagents-terminal/` release prefix does not change the existing
seven-platform TUI channel. Signing, notarization, publication, and both-surface
Mac verification remain owner steps in the
[handoff receipt](verification/2026-10-05-smart-terminal/README.md).

The overlay also listens on a control socket,
`~/.openagents/verse/terminal.sock` (`VERSE_TERMINAL_SOCKET` overrides it;
mode `0600` in a `0700` directory), one JSON request a line and one JSON
reply a line (`crates/terminal-control/src/lib.rs`). Connections are read
on their own threads and the requests cross to the frame thread, where
`Overlay::tick` applies them, so a reply describes what the window now
shows. `openagents verse terminal status|open|hide|split|focus|close|send|
key|read|tab|zoom` is the client (`docs/cli/README.md`); `send` and `key`
go through `coder_vt::Terminal::paste` and `key`, the same encoding as the
keyboard, and nothing from the socket reaches NIP-MV, chat, or presence.

Since the parity pass of October 5, 2026, the overlay behaves like a normal
terminal:

- **Copy.** A drag selects, a double-click selects a word, and a
  triple-click selects the line, across the scrollback; Shift+click extends.
  Cmd+C copies (Ctrl+Shift+C off macOS) and Cmd+V pastes, bracketed when
  the program asked. The prefix, then `[`, enters copy mode (vi keys, `v`
  and `V` select, `y` copies); the prefix, then `/`, searches the
  scrollback, and `n` and `N` repeat the search.
- **Programs.** Mouse reporting (modes 9, 1000, 1002, and 1003 in the
  default, UTF-8, SGR, and urxvt encodings) reaches programs that ask, such
  as `vim`, `less`, and `htop`; Shift keeps the mouse for selection. Focus
  events, cursor shapes and blink, OSC 8 links (Cmd+click opens web, mail,
  and file links), and OSC 52 clipboard writes from the focused pane work.
  No program can read the clipboard, and a background pane cannot write
  it. A bell lights the pane's title bar.
- **Keys.** Option is Meta by default (the prefix, then `m`, toggles it),
  and F1 to F24, the keypad in both modes, and modified navigation keys
  send what xterm sends.
- **Text.** Wide characters take two cells, combining marks draw over
  their character, and characters Fira Mono's prebuilt set lacks are
  rasterized on demand from Fira Mono and this computer's fonts (CJK,
  symbols, and emoji as a grayscale mask in the white ladder). Box drawing,
  blocks, braille, and powerline separators are drawn as shapes.
- **Performance.** Each frame applies output for at most 3 ms and 256 KiB a
  pane, focused pane first, so a pane printing without pause cannot stall
  the world; the prefix, then `?`, shows frame, parse, and key-to-glyph
  numbers and logs them each second. The `terminal_stress` example measures
  the overlay over Everglade's town; the receipt is in
  [verification/2026-10-05-terminal-performance](verification/2026-10-05-terminal-performance/README.md).

## Summary

- The Coder terminal app (`crates/coder`, drawn with `crates/coder-terminal`)
  is a conversational agent shell, not a terminal emulator. It cannot show a
  shell, `vim`, or `htop`, so it is not the right base for an in-world
  terminal.
- The repository already has the pieces a terminal needs: host PTYs with
  replay, gaps, and per-operation rights (`crates/coder-pty`, NIP-TERM), a
  VT100 and xterm emulator (`crates/coder-vt`), a client session that follows
  one terminal on a linked host (`coder_computers::terminal::session`), and a
  phone terminal screen built on all three.
- Ratatui programs, including `coder` itself, run inside the PTY on the host
  and appear in Verse through `coder-vt`. Verse does not need Ratatui or GPUI
  to show them.
- **Recommendation:** draw terminals in Verse with a native wgpu glyph-grid
  renderer fed by `coder-vt`, and keep the session, multiplexer, and sharing
  logic in portable Rust crates. Do not adopt GPUI. Do not render Ratatui
  buffers inside Verse.
- **First demo:** mount the existing phone terminal view
  (`coder_computers::terminal::project::view`) as a desktop Verse panel in
  Everglade, connected to this computer's host with the same path as
  `openagents computer shell`. It needs no new renderer, protocol, or grant.

## What the existing terminal app is

`coder` is the agent's terminal user interface. Its interactive mode
(`interactive` in `crates/coder/src/main.rs`) takes raw mode and the
alternate screen through `coder_terminal::Guard`, builds
`ratatui::Terminal::new(CrosstermBackend::new(stdout()))`, and runs a draw
loop. `draw` in the same file lays out a scrollback of turns above a framed
composer (`coder_terminal::Composer`) and fills a `ratatui::buffer::Buffer`
through `frame.buffer_mut()`.

Whether its widgets could render elsewhere:

| Question | Answer | Evidence |
| --- | --- | --- |
| Do the widgets need a real TTY? | No. They write cells into a Ratatui `Buffer`. | The composer, frame, and overlay tests build `Buffer::empty(area)` with no backend (`crates/coder-terminal/src/composer.rs`, `hairline.rs`, `components/overlay.rs`). |
| Could Verse draw that buffer? | Yes, in principle. A `Buffer` is a grid of cells with symbols and styles that a glyph renderer can draw, as Ratatui's `TestBackend` does in memory. | Ratatui 0.30, pinned in `crates/coder-terminal/Cargo.toml`. |
| Does `coder-terminal` build for the browser or phones as is? | Not without a feature split. It depends on `crossterm` unconditionally (`keys.rs` maps `crossterm::event::KeyEvent`; `native.rs` imports it), and `code-highlight`'s `grok` feature builds `syntect` with the Oniguruma C library. | `crates/coder-terminal/Cargo.toml`, `crates/code-highlight/Cargo.toml`. |
| Is it a terminal emulator? | No. It draws its own transcript; it does not parse a program's escape sequences. | `crates/coder-terminal/src/lib.rs` module docs. |

The conclusion: porting the Ratatui widgets into Verse would show only
Coder's chat, and only after a dependency split. Running `coder` in a PTY
shows the same chat, unchanged, beside any other program. The emulator is
the reusable part, and the repository already has one.

## What already exists for remote terminals

| Piece | What it does | Where |
| --- | --- | --- |
| NIP-TERM | Open, attach (`interact` or `observe`), detach, input, resize, signal, and close a host PTY. Output has a per-terminal sequence number, a bounded replay buffer, explicit `gap` frames, idle expiry, and a host generation that makes old references `lost`. Input is live only. Content is private and never appears in a public event. | `nips/openagents/NIP-TERM.md` |
| Host half | Real PTYs as process groups (ConPTY and a job object on Windows), the replay `Ring`, per-attachment byte budgets, request deduplication, and `Config::observers_read` (default `false`) for read-only observers. Rights come through the `host::Rights` trait and frames leave through `host::FrameSink`. | `crates/coder-pty/src/host/mod.rs` (`Host::open`, `attach`, `input`, `resize`, `tick`) |
| Portable client state | Applies frames in order, detects gaps and duplicates. Builds without the `host` feature. | `crates/coder-pty/src/client.rs` (`TerminalState::apply`, `resume_after`) |
| Emulator | VT100 and xterm subset: SGR with 16, 256, and 24-bit color, the alternate screen, scroll regions, scrollback, bracketed paste, device replies, and xterm key and paste encoding. Ignores clipboard and other operating-system commands from output. Depends only on `vte` and `unicode-width`. | `crates/coder-vt/src/lib.rs` (`Terminal::feed`, `screen`, `scrollback`, `cursor`, `generation`, `key`, `paste`, `take_replies`) |
| Resident host binding | Runs each NIP-TERM operation after checking the device's standing and the right the operation needs, over a NIP-REACH direct channel or sealed `3188` artifacts. | `crates/coder-host/src/serve/terminal.rs` (`run`), `crates/coder-host/src/authority.rs` (`impl coder_pty::host::Rights for Grants`) |
| Rights | A closed set of seven host-wide rights. `terminal` opens and drives any terminal; `observe` reads, and reads terminals only when the host's policy allows. A grant has no per-terminal scope. | `crates/coder-access/src/rights.rs` (`Right`, `Rights::standard`, `Rights::pairing`), `crates/coder-access/src/protocol.rs` (`Grant`) |
| Client session | Opens a shell with NIP-HOST `terminal.open`, attaches in `interact` mode, orders frames (`Ordered`), reattaches after a lost frame or a new route, never queues input while the link is down, and reports `lost` after a host restart. Feeds a `coder-vt` emulator. | `crates/coder-computers/src/terminal/session.rs` (`Session::start`, `send`, `resize`, `close`, `leave`) |
| Terminal view | A Rust Native tree: header, one node per grid row (`TextRole::Terminal` runs), and an accessory key row (Esc, Tab, latching Ctrl, arrows, Ctrl-C, Paste). Colors map onto the amber ladder. | `crates/coder-computers/src/terminal/project.rs` (`view`), `screen.rs` (`Terminal::open`, `key`, `text`, `paste`, `packet`) |
| Command-line client | `openagents computer shell HOST` drives the same session from a real terminal; `Ctrl-]` detaches and the shell keeps running. The route (loopback, tailnet, direct, or relay) is the Computers supervisor's choice. | `crates/openagents-cli/src/terminal.rs`, `docs/cli/README.md` |

The NIP-TERM implementation status lists conformance tests for every
property above. A Verse terminal is one more client of this stack.

## What Verse has for drawing it

| Piece | Fit for a terminal | Where |
| --- | --- | --- |
| Rust Native panels | Paints a Rust Native view on the CPU with `rust-native-desktop` and hands the pixels to the renderer as an `OverlayImage`, drawn over the finished frame. Owns keyboard focus: while focused, every key goes to the panel and none reaches the character; a press outside returns focus to the world. Desktop only (`panels` feature). `rust-native-desktop` already lays out `TextRole::Terminal` text monospaced. | `crates/verse/src/panels.rs` (`Panel::key`, `Panel::press`), `crates/verse-gfx/src/overlay.rs` (`OverlayImage`, `MAX_EXTENT`), `crates/verse/src/app.rs` (`panel_key`), `crates/rust-native-desktop/src/layout.rs` |
| Screen-space UI batch | Fira Mono Medium rasterized once with `swash` into a single-channel atlas; `UiBatch::rect` and `UiBatch::text` draw colored quads and text in physical pixels. The font is monospaced, so cells align. The atlas carries only printable ASCII, Latin-1, and four extra characters; anything else draws as `?`. | `crates/verse-gfx/src/ui.rs` (`Atlas`, `drawable`, `UiBatch::text`) |
| In-world boards | Everglade's Task Wall, goal board, and one desk monitor per seat (0.84 m by 0.5 m) that already streams a seat's log tail. Text is stroke lettering built into meshes (`letters` through `doors::scene_label`), which suits a few short lines, not an 80 by 24 grid redrawn many times a second. | `crates/verse-zone-everglade/src/zones/everglade/boards.rs` (`monitor`, `screen`, `letters`), `crates/verse-zone-everglade/src/zones/everglade/layout.rs` (`DESKS`, `Board`) |
| Host connection | The Agent Studio talks to this computer's host over the same-user control socket (`openagents_connect::control`), a request and response protocol. It has no streaming terminal operation. | `crates/verse-zone-everglade/src/zones/everglade/studio/live.rs`, `crates/openagents-connect/src/control.rs` (`Op`) |
| Web build | `everglade-web` draws Everglade with WebGPU or WebGL2 and connects to no relay or studio host. | `crates/everglade-web/README.md` |
| Phones | Stations open Rust Native panels through the hosts' native mounting; the phone terminal screen already exists in both mobile libraries. | `docs/verse/everglade.md#the-workspace`, `crates/coder-computers/src/terminal/screen.rs` |

## Reference design: the multiplexer in the earlier repository

The earlier Coder repository on this machine (`~/work/coder`) built a
terminal multiplexer. It is reference material only: this repository
reimplements any idea it takes, copies no code, and says so in the commit
message. The ideas worth carrying over:

- **The session outlives every client.** A headless process (the "writer")
  owns each shell as a block under a PTY. Closing a pane detaches; ending a
  shell is a separate, confirmed close. NIP-TERM already has these
  semantics.
- **One driver per block.** The first client to type becomes the driver;
  another client's keys and resizes refuse with `not_driver` until the role
  moves with an explicit take and release. Viewers draw at the driver's size
  and pan to follow the cursor, and the title says who drives.
- **Per-device shares.** An allow list grants each device submit, drive,
  both, or neither, and the welcome message offers only the overlap.
- **Joining late.** A client that falls behind gets a fresh snapshot instead
  of an unbounded queue; history pages arrive newest first, capped.
- **Layout tree.** A dwindle tree (`Leaf` or `Branch { split, ratio, first,
  second }`) with spawn-by-split, geometric focus movement, ratio resize
  clamped to 0.1 through 0.9, toggle split axis, floating, fullscreen, and
  numbered workspaces. The tree lived only in memory. This repository's
  `crates/coder-wm` already holds a dwindle layout for the Coder compositor.
- **Chords, not a prefix.** One table of Super chords served the compositor,
  the game client, and the desktop app; there was no tmux-style prefix and no
  saved layout file.
- **Rendering.** The desktop app drew panes with GPUI; the game client drew
  them with wgpu: one instanced draw per pane over a shared glyph atlas, one
  instance per cell, dirty instance ranges so only changed cells are
  rewritten, and an atlas that uploads only new glyphs and grows by doubling
  without moving existing slots. Only visible panes take a snapshot each
  frame. Measured paint and emulation stayed under about 2 ms at the 99th
  percentile with 10 streaming panes.

The reference repository's own game client chose wgpu for in-world
terminals and kept GPUI for its desktop application, which supports the
recommendation below.

## Recommendation

Draw terminals in Verse with a native wgpu glyph-grid renderer that reads
`coder-vt`'s grid, and put the session, multiplexer, and sharing logic in
portable crates that Verse on desktop, web, and phones share.

| Option | Verdict | Why |
| --- | --- | --- |
| Native wgpu glyph grid in Verse | **Adopt.** | Verse already owns a wgpu renderer, a `swash` atlas, and overlay and in-world draw paths. A grid of colored cells is the simplest thing that renderer draws, and the same pass can target the screen or a texture on an in-world monitor. It builds for `wasm32-unknown-unknown` and the phone targets that Verse already ships. |
| Ratatui `Buffer` rendered by Verse | **Do not adopt for terminals.** | Ratatui lays out an application's own widgets; it does not emulate a terminal. Ratatui programs already reach Verse through the PTY. The `coder-terminal` crate would also need a `crossterm`-free and Oniguruma-free feature to build for the web. Revisit only if Verse wants Coder's chat widgets as native chrome rather than as a program in a pane. |
| GPUI | **Do not adopt.** | GPUI is an application framework that owns its window, event loop, and platform layer; Verse owns its own `winit` window and wgpu frame and draws the world underneath. GPUI has no supported browser or iOS and Android targets, which Verse needs. Adding it would mean a second renderer and text stack beside Verse's, and AGENTS.md directs shared UI contracts to Rust Native, not to another framework. |
| Rust Native panel with the phone terminal view | **Use for the first demo only.** | It works today with no new renderer. Its costs are a CPU raster and a full texture upload per revision, one Rust Native node per row under the 1,024-node view limit (`rust_native::view::MAX_NODES`), and no path onto an in-world surface. |

## Architecture

```text
 host computer                                   Verse client
 ┌──────────────────────────┐   NIP-TERM over    ┌──────────────────────────────────┐
 │ coder host serve         │   NIP-REACH        │ terminal session                 │
 │  coder-pty Host          │ ◄────────────────► │  (coder_computers::terminal::    │
 │   PTY + process group    │   direct channel,  │   session::Session)              │
 │   replay Ring, seq, gap  │   or sealed 3188   │        │ frames in order         │
 │  Rights: NIP-HOST grants │   over a relay     │        ▼                         │
 │   rechecked per message  │                    │  coder-vt Terminal (grid)        │
 └──────────────────────────┘                    │        │                         │
                                                 │        ▼                         │
                                                 │  multiplexer: panes, layout,     │
                                                 │   focus, prefix key              │
                                                 │        │                         │
                                                 │        ▼                         │
                                                 │  glyph-grid pass ──► overlay     │
                                                 │                 └──► texture on  │
                                                 │                      a monitor   │
                                                 └──────────────────────────────────┘
```

The layers and their homes:

1. **Transport and authority** stay where they are: `coder-pty`,
   `coder-host`, `coder-access`, and `coder-reach`. A Verse terminal adds no
   authority of its own.
2. **Session** reuses `coder_computers::terminal::session::Session`. It
   needs a tokio runtime and `coder_host::client::Link`, so it runs on
   desktop and phones. The browser needs a different transport (see
   [Input and platforms](#input-and-platforms)).
3. **Multiplexer** is a new portable crate (working name `coder-mux`) with no
   renderer, network, or platform dependency: panes bound to terminal
   references, a layout tree, focus, the prefix-key state machine, and
   layout persistence. Verse, the phones, and `openagents computer shell`
   can share it.
4. **Renderer** is a new Verse module that draws a `coder-vt` grid in a
   rectangle, either into the frame (overlay) or into a texture (in-world
   screen).

## The overlay

A terminal opens as a partial-screen overlay over the running world. The
world keeps rendering and simulating behind it.

- **Opening.** A toggle chord opens the terminal overlay, or focuses it when
  it is open. Interacting with a terminal station (for example, a seat's
  desk monitor in the workshop) opens the overlay on that station's
  terminal. The default chord is `` Ctrl+` ``; it is configurable.
- **Placement.** The overlay docks to one edge (default: bottom, 45% of the
  window height) or floats as a window. Drag its edge to resize it; the grid
  recomputes rows and columns from the cell size and sends one NIP-TERM
  `resize` after the drag settles, not one per pixel.
- **Transparency.** The background opacity is adjustable (default 85%) so
  the world shows through. Text is always drawn opaque. A focused overlay is
  more opaque than an unfocused one.
- **Hiding.** The toggle chord hides a focused overlay. Hiding detaches
  nothing: the sessions stay attached and keep applying frames, so the
  overlay reopens at the current screen. Closing a pane detaches it; the
  shell keeps running on the host until it exits, is closed, or idles out.
- **Focus.** The overlay is either focused or not, and the HUD shows which.
  - While focused, every key goes to the terminal, including `Enter`,
    `Escape`, `W`, `A`, `S`, `D`, and the arrow keys. None reaches the
    character controller, chat, the hotbar, or spells. This follows the
    existing rule in `panels.rs` and `app.rs` (`panel_key`).
  - `Escape` belongs to the terminal, because programs such as `vim` need
    it. The ways back to the world are the toggle chord, the prefix key
    followed by `Escape`, or a click outside the overlay.
  - While unfocused, the overlay stays visible and live, and the world takes
    every key.
- **Mouse.** A click in a pane focuses it. The wheel scrolls that pane's
  scrollback, unless the program enabled mouse reporting, which a later
  phase adds to `coder-vt`.

## Multiplexing

The multiplexer is client-side state over host-side terminals. Every pane
is one NIP-TERM attachment; the host owns the shells, so a layout can close
and reopen on another device without ending a process.

- **Pane.** A pane binds a host key and a terminal reference
  (`{generation, terminal}`), an attachment mode (`interact` or `observe`),
  and the pane's `coder-vt` emulator.
- **Layout.** A tree of splits like the reference dwindle tree: a leaf is a
  pane; a branch has a split axis, a ratio clamped to 0.1 through 0.9, and
  two children. Operations: split horizontally or vertically (opening a new
  shell on the focused pane's host by default), close, move focus by
  direction, swap, resize, toggle the split axis, and zoom one pane to fill
  the overlay.
- **Tabs.** A tab is one layout tree. The tab strip shows each tab's name and
  which hosts its panes reach.
- **Several computers.** Each pane names its own host, so one tab can mix
  panes on the workshop's computer and a remote box. A new pane offers the
  hosts this device holds `terminal` on. The pane's title shows the host
  label and the route (loopback, tailnet, direct, or relay).
- **Attach and detach.** Detaching a pane or a whole layout keeps the shells
  running. Reattaching sets `after` to the last applied sequence number, and
  NIP-TERM replays or reports a gap. A pane whose host restarted shows
  `lost` and offers a new shell; it never presents a new process as the old
  one.
- **Persistence.** A saved layout records hosts, terminal references, the
  tree, and tab names, and nothing a terminal printed. Desktop keeps it in a
  private file in the Verse home. Restoring it reattaches each terminal that
  still exists and marks the rest `closed` or `lost`.
- **Prefix key.** `Ctrl+B` by default, configurable, followed by one key:

  | After the prefix | Action |
  | --- | --- |
  | `%` or `\|` | Split left and right. |
  | `"` or `-` | Split top and bottom. |
  | Arrow keys | Move focus. |
  | `Ctrl` with arrow keys | Resize the focused pane. |
  | `z` | Zoom or unzoom the focused pane. |
  | `c` | New tab. |
  | `n`, `p`, `0` to `9` | Next, previous, or numbered tab. |
  | `x` | Close the focused pane, after confirmation when a program is running. |
  | `d` | Detach the layout and hide the overlay. |
  | `[` | Scrollback mode. |
  | `s` | Share the focused pane (see [Sharing](#sharing-with-others)). |
  | `Escape` | Return focus to the world. |
  | The prefix again | Send the prefix byte to the program, so `tmux` inside a pane still works. |

  Super chords from `crates/coder-binds` can map to the same actions on
  CoderOS, as in the reference design.

## Sharing with others

Sharing has two modes, matching NIP-TERM's attachment modes:

- **Watch** (`observe`): the viewer sees output and cannot type, resize, or
  signal.
- **Drive** (`interact`): the viewer can type. One driver at a time, as in
  the reference design: input from anyone but the current driver refuses,
  and the role moves with an explicit take and release that the owner can
  override. Watchers draw at the driver's size.

### Grants

Today a viewer's device must be enrolled on the host. `terminal` lets it
open and drive every terminal on that host, and `observe` lets it read
terminals only when the host sets `observers_read`, which is host-wide and
off by default. Neither is the right grant for "show my build log to a
friend in the workshop."

Specify a narrower share as a NIP-TERM extension, decided by the host and
checked per message like every other right:

- A **terminal share** binds one terminal reference, one grantee device key,
  a mode (`observe` or `interact`), the first sequence number the grantee may
  read, and an expiry. The host signs it; revoking it ends the attachment
  with `detached` reason `revoked`.
- A share never lets the grantee open, close, or signal a terminal, or reach
  another terminal, and `interact` under a share still obeys the one-driver
  rule.
- Issuing a share requires `terminal` on that host. Delegation can only
  narrow, as `coder-access` already requires.
- The first readable sequence number defaults to the terminal's head when
  the share is issued, so a viewer does not receive scrollback written before
  the share. The host enforces this on `attach`, not the client.

### In-world screens others see

A shared terminal can appear on an in-world screen, such as a workshop desk
monitor, for every player standing nearby.

- The world carries only the **binding**: which screen shows a terminal and
  whose it is. It never carries terminal bytes. No terminal content enters
  NIP-MV events, the hosted social profile's snapshots, the chamber service,
  world chat, or any public event, as the NIP-TERM privacy section requires.
- Each player who sees the screen attaches to the terminal with their own
  device under their own share. A player without one sees a placeholder that
  names the owner and says the terminal is private.
- The binding is a world command under the existing authority rules: in a
  hosted social instance it goes through the authority's command path; on
  the owner's own studio it is local state.

### Disclosure on screen

A terminal shows whatever a program prints, including secrets.

- Sharing a pane shows who is watching and who drives in the pane's title,
  and a persistent marker on the overlay while any share is active.
- **Pause sharing** blanks the pane for every viewer without ending the
  share: the host stops delivering frames to share attachments and resumes
  them with a `gap`, so paused output is never sent.
- Starting a share warns once that everything printed from now on is
  visible to the grantee and cannot be recalled, as NIP-TERM states for
  revoked devices.
- Input echo is the program's: a password prompt that disables echo shows
  nothing to anyone. Keystrokes are never mirrored to viewers apart from the
  program's own output.

## Input and platforms

All key encoding goes through `coder_vt::Terminal::key` and `paste`, so
application cursor mode and bracketed paste are honored on every platform.
Printable text comes from the platform's text event (`winit`'s
`KeyEvent::text` and committed IME text on desktop), not from key codes, so
layouts and dead keys work. Input travels as NIP-TERM `input` requests of at
most 4,096 bytes and is never queued while the link is down; the pane shows
that typing is unavailable instead.

| Platform | Keyboard | Clipboard | Transport | Notes |
| --- | --- | --- | --- | --- |
| Desktop (macOS, Linux, Windows) | Full keyboard. `Cmd` on macOS maps to copy and paste, not to the terminal. | `arboard`, already a desktop dependency. | NIP-REACH channel or relay through `coder_computers::live`. | The demo platform. |
| Web (`everglade-web`) | Browser text events. The browser reserves some chords (for example, `Ctrl+W`, `Ctrl+T`, and `Ctrl+N`), so the prefix key and the accessory row cover them. | The asynchronous clipboard API, on user gesture only. | None today. A browser can use only WebSocket: either a host's WebSocket listener (`coder host serve --listen-websocket`) reached over a tailnet, or sealed `3188` artifacts over the relay at the lower relay rate (`RELAY_RATE` is 16 KiB/s in `session.rs`). The session needs a transport that builds for `wasm32`. | Later phase. `coder-vt` and `coder-pty`'s client half build without host code; the session and link do not. |
| Phones (iOS, Android) | The soft keyboard through the native host, which forwards text and named keys to Rust, plus the existing accessory row (Esc, Tab, latching Ctrl, arrows, Ctrl-C, Paste). | Native clipboard through the host, as `screen.rs` does now. | The same as desktop. | The phone terminal screen exists today. In Verse, a station opens it as a panel through native mounting; the GPU grid replaces it on in-world screens. Showing the soft keyboard pauses the world's touch controls. |

## Performance

- **Glyph atlas.** Extend Verse's atlas from a fixed character set to a
  growable one: rasterize glyphs on first use with `swash`, pack them, upload
  only new glyphs, and grow the texture without moving existing slots, as in
  the reference design. Terminals need box drawing (U+2500 through U+257F),
  block elements, braille, and common symbols, which the current atlas draws
  as `?`. Draw box-drawing and block characters as geometry so they join
  across cells at any size. Bold and italic need their own faces or
  synthetic styles. Wide characters take two cells, as `coder-vt` already
  records.
- **Draw path.** One instanced draw per visible pane: one instance per cell
  with background, foreground, glyph rectangle, and flags. At 200 by 60 that
  is 12,000 instances, well within one draw.
- **Damage regions.** `coder-vt` exposes only a whole-terminal
  `generation`. Add per-row dirty marks (set by print, erase, scroll, and
  resize; cleared when the renderer takes them) so the renderer rewrites only
  changed rows' instances. A pane whose generation did not change costs
  nothing to keep on screen.
- **Hidden panes.** A pane in a hidden tab or a closed overlay keeps applying
  frames to its emulator but builds no instances. An in-world screen outside
  the view frustum or beyond a distance builds no instances either.
- **In-world screens.** Render the grid into an offscreen texture only when
  it changes, and sample that texture on the monitor's face through the zone
  renderer's textured path. Cap in-world refresh (for example, 15 Hz) below
  the overlay's.
- **Scrollback.** `coder-vt` bounds scrollback by line count. Keep the
  default modest (for example, 5,000 lines per pane) and offer more only for
  focused panes. The host's replay ring, not the client, is the recovery
  path after a disconnect.
- **Output rate.** NIP-TERM's per-attachment `rate` already bounds a flood;
  the session asks 64 KiB/s over a direct channel. Emulation runs off the
  render thread on desktop, and the frame never waits on the socket, as the
  studio's live source does.

## Security

- **Rights are the host's.** The host checks `terminal`, `observe`, or a
  terminal share on every request and periodically on every attachment
  (`coder-host` `serve/terminal.rs`). Verse only avoids offering a control
  that cannot work; it decides nothing.
- **No keystroke leakage.** While a terminal has focus, key events go only to
  the terminal session. The world chat composer, NIP-MV gestures, the
  hosted-social command path, and Verse traces never receive them. A test
  types into a focused terminal and asserts that the chat buffer, controller
  input, and outgoing world commands are unchanged. Input is not logged.
- **Output is untrusted data.** `coder-vt` ignores operating-system commands
  other than the title, so a program cannot read or write the clipboard;
  keep it that way. Titles are bounded and drawn as text. A URL in output is
  not a link until the user acts on it explicitly.
- **No terminal content in the world.** Terminal bytes, titles, working
  directories, and command lines stay out of NIP-MV, presence, the chamber
  service, world chat, Verse replays, and logs. A capture made while a
  terminal is visible (`render::capture_with_overlay` composites overlays)
  leaves terminal panes out unless the user asks to include them.
- **Same-user scope.** The desktop demo opens the device store that
  `openagents computer` uses and holds the grant that store holds. It never
  enrolls a device or changes a grant on its own.
- **Tests stay off the owner's computers.** Automated tests use a scratch
  host under a temporary `HOME`, as `crates/verse`'s dev-dependency on
  `coder-host` already does for the studio.

## What exists and what is missing

Estimates are focused engineering days for one agent, including tests.

| Area | Exists | Missing | Estimate |
| --- | --- | --- | --- |
| Host PTYs, replay, rights | NIP-TERM host and resident binding, conformance tests | Nothing for the demo | 0 |
| Emulator | `coder-vt` | Per-row dirty marks; mouse reporting modes; OSC 8 hyperlinks shown as text | 2 |
| Client session | `coder_computers::terminal::session` | A Verse-owned tokio runtime and `Live` on desktop; a `wasm32` transport for the web | 1 (desktop), 5 to 8 (web) |
| Demo overlay | Rust Native panels, the phone terminal view, `rust-native-desktop`'s monospaced terminal text | A generic terminal panel beside the studio panel, wider docking, and key routing through `Terminal::key` | 2 to 3 |
| Glyph-grid renderer | `ui.rs` atlas and batches, `swash` | Growable atlas, box-drawing geometry, instanced cell pass, cursor and selection | 5 to 7 |
| Multiplexer | `coder-wm` dwindle layout as a reference | `coder-mux`: panes, tree, tabs, prefix keys, persistence, and its tests | 5 |
| Several computers | `coder_computers::live` supervises many hosts | Host picker in Verse, per-pane route display | 2 |
| Sharing | `observe` attachments, `observers_read` | Terminal share grant (NIP-TERM extension, `coder-access`, `coder-pty`, `coder-host`), one-driver rule, pause, viewer list | 8 to 10 |
| In-world screens | Desk monitors, zone textured path | Render-to-texture grid on a board, world binding command, placeholder for unauthorized viewers | 4 to 5 |
| Web | `everglade-web` | Transport, browser key handling, clipboard | 5 to 8 beyond the session work |
| Phones | Phone terminal screen | Opening it from a Verse station; later the GPU grid on in-world screens | 2 |

## Phased plan

Each phase ends in something the owner can run.

1. **Demo: one overlay terminal in Everglade (about 3 days).** On desktop,
   add a terminal panel to `verse::panels` that mounts
   `coder_computers::terminal::project::view` over a
   `coder_computers::terminal::session::Session`. It opens with the toggle
   chord or at a desk monitor in the workshop, connects to this computer's
   host through the `openagents computer` store (the same route as
   `openagents computer shell`), and opens the host's shell. Typing works,
   including `Enter`, `Escape`, arrows, `Ctrl` chords, and paste; the world
   takes no keys while it is focused. Running `coder` in it shows the Coder
   terminal app inside Verse. Prerequisite: this computer is linked once
   with `openagents computer link` and its grant includes `terminal`.
2. **Glyph-grid renderer (about 1 week).** Replace the panel's raster with
   the instanced cell pass, the growable atlas, and per-row damage. Adds
   transparency, resizing, and scrollback mode.
3. **Multiplexer (about 1 week).** `coder-mux` with splits, tabs, zoom, the
   prefix key, detach and reattach, and saved layouts. `openagents computer
   shell` can adopt it later for a terminal-native multiplexer over the
   same panes.
4. **Several computers (about 2 days).** Panes on any linked host, with the
   host and route in each title.
5. **Sharing (about 2 weeks).** Specify and implement the terminal share
   grant, watch and drive with one driver, pause, and the viewer list, on
   the owner's studio first.
6. **In-world screens (about 1 week).** A shared or private terminal on a
   workshop desk monitor, drawn to a texture, visible to authorized players
   nearby and a placeholder to others; then the binding in hosted social
   instances.
7. **Web and phones (about 2 weeks).** A browser transport and input for
   `everglade-web`; terminal stations on phones through native mounting.

## Open questions

- Should the desktop Verse process share the `openagents computer` device
  store with the command-line client, and is concurrent access to that store
  safe, or should Verse enroll its own device key once?
- Which key does a player's terminal share name: the player's Verse world
  key or a device key enrolled on the host? Phones already hold a separate
  world key for the Gym grant.
- Does the terminal share belong in NIP-TERM as an extension, or in NIP-HOST
  as a resource-scoped grant that other surfaces (sessions, tasks) can reuse?
- Should the one-driver rule be host-enforced for all attachments, including
  two devices of the same owner, or only for shares?
- Is `` Ctrl+` `` free of conflicts on the owner's keyboards and on CoderOS,
  and should the default prefix be `Ctrl+B` (tmux) or `Ctrl+A` (screen)?
- Should saved layouts live only on the client, or on the host so a layout
  follows the owner across devices?
- How should a Verse terminal treat a host's world-facing studio seats: is a
  seat's desk monitor the seat's own terminal, a terminal the owner opens,
  or both?
- What is the browser's transport: a host WebSocket listener that requires a
  tailnet, or relay artifacts at a lower rate, or a new relay-assisted
  direct route?
