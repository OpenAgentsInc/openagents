# coder-compositor

The Coder Wayland compositor for a CoderOS desktop, built on
[Smithay](https://github.com/Smithay/smithay). It lays the screen out with
the layout crate `coder-wm`, reads a bind table of the chords the Hyprland
session binds, and answers the
[desk protocol](../coder-desk/README.md) on a socket of its own, so the scripts
and tools that drive windows speak one contract to it. It runs Xwayland as
well, so an X11 program tiles beside the Wayland ones.

It has two backends that share the state, the layout, and the protocols. The
nested backend opens a window inside the session you are already in, and that
window is a compositor. The hardware backend takes the seat of the TTY it
starts on, drives the monitors through DRM and KMS, and reads the keyboard and
the pointer through `libinput`.

It is optional. Hyprland stays the default CoderOS desktop, and a host picks
this compositor with `coderos.desktop.compositor = "coder"`. It moved here
from the private Coder repository on 2026-09-28, by the owner's decision to
publish it, as part of the CoderOS move in
[the audit](../../docs/os/2026-09-28-coderos-audit.md).

## Which backend a run starts

`--backend winit` opens the nested window, and `--backend udev` takes the
hardware. With neither flag, a process that finds `WAYLAND_DISPLAY`,
`WAYLAND_SOCKET`, or `DISPLAY` in its environment is inside a session and
opens nested, and a process that finds none of them is on a TTY and takes the
hardware. `src/backend.rs` holds the rule. `os/bin/coder-compositor-session`
passes the flag it picked, from the grant's `backend` key,
`CODER_COMPOSITOR_BACKEND`, or the same rule.

## Run it nested

```sh
cargo run -p coder-compositor -- --backend winit
```

A window opens in your session. The log names the two sockets it announces:

```
the compositor listens on wayland-2, and its desk socket is /run/user/1000/coder-desk/2137004.sock
```

Open a client in it:

```sh
WAYLAND_DISPLAY=wayland-2 foot
```

The client tiles, takes the keyboard when the pointer is over it, and reads
`CODER_DESK_SOCKET` from its environment, so a `coder` in a tile
can ask the compositor about the screen it draws on.

An X11 program opens through the desk protocol, which starts Xwayland for
it:

```sh
CODER_DESK_SOCKET=/run/user/1000/coder-desk/2137004.sock coder-desk open -- xeyes
```

On a NixOS host the binary needs `libxkbcommon`, `libudev`, `libinput`,
`libseat`, and `libgbm` at link time, and those with the Wayland, EGL, and
Vulkan libraries at run time. `nix develop ./os#compositor` from the
repository root opens a shell that has them; a shell that does not have them
fails at the link with `unable to
find library -lxkbcommon` or `-linput`, or at startup with `Failed to
initialize an event loop` or a missing `libinput.so.10`. The package,
`nix build ./os#coder-compositor`, links and wraps all of them.

## Run it on a TTY

Log in on a TTY with no session running on it, and start the hardware
backend:

```sh
coder-compositor --backend udev
```

It opens a seat through `libseat`, which asks logind or seatd for the
devices, so the account needs no `video` or `input` group. It drives the
monitors of one graphics card: the card `CODER_COMPOSITOR_DRM_DEVICE` names,
such as `/dev/dri/card2`, or the card the firmware booted with. The log names
the seat, the card, and each monitor it turns on:

```
the seat is seat0, and the screens draw with renderD129
the monitor on DP-2 is on at 2560x1440 and 60 hertz
```

- **Screens.** Each connected monitor is a screen named for its port, such as
  `DP-2` or `HDMI-A-3`, which is the name the kernel and Hyprland give it. The
  screens sit left to right in the order they arrived, and each one shows a
  desk of its own.
- **Hotplug.** A monitor plugged in or pulled out starts or drops its screen
  without a restart, and a keyboard or a mouse plugged in reaches the seat.
  A screen that leaves keeps its desk's windows in the layout, and they come
  back when a screen shows that desk.
- **Leaving.** Ctrl+Alt with a function key switches the virtual terminal,
  and switching back resumes the screens. Super+Shift+E ends the compositor.
- **Another terminal.** A compositor started while another virtual terminal
  is active opens the card and waits, because logind gives DRM master to
  the session on the active terminal alone. The log says `waits for it`;
  switch to the compositor's terminal, and it resets the connectors, turns
  the monitors on, and logs `takes the screen`.
- **NVIDIA.** The proprietary driver needs explicit sync, the overlay planes
  left unused, and a heap reuse profile. The owner's NVIDIA workstation
  first ran it from a second TTY on 2026-09-17, which is the safe way to
  try it: a failure leaves tty1 alone. `CODER_COMPOSITOR_NO_SCANOUT=1` composites every frame, for a
  fullscreen client that draws torn or stale.

## Screens and scale

The layout places windows in logical pixels in one space every screen shares.
A screen's logical size is its mode divided by its scale, so a 2560 by 1440
monitor at 1.25 tiles 2048 by 1152. `src/screens.rs` holds the screens, their
places, and the desk each one shows. The layout crate holds screens of its
own, and this compositor does not read them yet.

`coder-desk scale <screen> <scale>` sets a screen's scale on either backend,
which is what `presentation-mode on` and `off` send. A scale is 0.5 through 4,
rounded to a 120th, and moved by up to a tenth to the nearest scale that
leaves a whole logical size. The compositor tells every surface its screen's
scale through `wp-fractional-scale`, so a client that reads it, such as
Chromium, draws at the screen's pixels through `wp-viewporter`; a client that
reads only `wl_output`, such as `foot`, draws at the next whole scale and the
compositor scales it down.

The monitor chords `os/modules/coderos/desktop.nix` binds work across the
screens:

| Chord | What it does |
| --- | --- |
| Ctrl+Alt+Tab, Ctrl+Alt+Shift+Tab | Gives the next or the previous screen the focus. |
| Super+Alt with an arrow | Gives the screen in that direction the focus. |
| Super+Shift+Alt with an arrow | Sends the focused window to the screen in that direction. |
| Super+Ctrl+Alt with an arrow | Sends the focused desk, with every window on it, to the screen in that direction, which then shows the desk it showed before on the screen the desk came from. |

A chord that moves the focus to another screen puts the pointer in its
middle, and a pointer that crosses onto another screen gives it the focus.
Super and a digit for a desk another screen shows moves the focus to that
screen, which is what Hyprland does.

The compositor draws the pointer on both backends: the cursor surface a
client sets, and otherwise the `default` image of the xcursor theme
`XCURSOR_THEME` and `XCURSOR_SIZE` name, or an arrow of its own when no theme
answers. The nested window hides the session's pointer over itself.

## What it answers

| Protocol | Version | What it carries |
| --- | --- | --- |
| `wl_compositor`, `wl_subcompositor`, `wl_shm` | 5, 1, 2 | Surfaces and shared-memory buffers. `wl_shm` names `xbgr8888` and `abgr8888` on top of the two every compositor names, because a screencopy client allocates in the format a frame asks for. |
| `xdg_wm_base` | 6 | Toplevels and popups, and a client's own fullscreen request. |
| `zxdg_decoration_manager_v1` | 1 | Answered server-side for every toplevel. The compositor draws the border. |
| `zwlr_layer_shell_v1` | 4 | Anchored surfaces. A layer surface's exclusive zone comes out of the area the layout tiles in, so a panel takes its pixels rather than drawing over a window. |
| `zwlr_screencopy_manager_v1` | 3 | One screen or a rectangle of it, into a shared-memory buffer. The hardware backend also offers a dmabuf for a copy of a whole screen, and draws the copy straight into it. |
| `wl_data_device_manager` | 3 | The clipboard and drag and drop. The clipboard follows the keyboard focus. |
| `zwp_primary_selection_device_manager_v1` | 1 | The middle-click selection, which follows the keyboard focus too. |
| `zwp_text_input_manager_v3` | 1 | The global an input method and a client that takes text agree on. |
| `zwp_virtual_keyboard_manager_v1` | 1 | A keyboard a client makes, which is how `wtype` types and how `os/bin/dictate-toggle` types a transcript. A key reaches the focused window under the keymap the client uploaded, and the next key from the real keyboard reads under the seat's layout again. A chord sent through it runs nothing: the bind table reads the seat's keyboard alone. |
| `ext_idle_notifier_v1` | 2 | One timer for each timeout a client asks about. Input resets them, folded to one report every 100 milliseconds. |
| `zwp_linux_dmabuf_v1` | 5 | The buffers a client that draws through Vulkan asks for, with the feedback that names the device it should allocate on. `wf-recorder` binds version 4 and disconnects without it. A commit that carries a dmabuf waits until the buffer is readable. |
| `wp_linux_drm_syncobj_manager_v1` | 1 | Explicit sync, on the hardware backend when the card supports `syncobj_eventfd`. A commit waits for the acquire point its client names. |
| `wp_fractional_scale_manager_v1`, `wp_viewporter` | 1, 1 | The scale of the screen a surface draws on, and the viewport a client draws a fractional buffer through. |
| `wl_output`, `zxdg_output_manager_v1` | 4, 3 | One output for each screen, with its scale and no transform: `nested-1` on the nested backend, and a port name such as `DP-2` on the hardware backend. The nested backend renders through a flip its window's framebuffer needs; that is how it draws and not what the screen is, so a recorder that read the transform off `wl_output` does not flip every frame. |
| `wl_seat` | 9 | One keyboard and one pointer, fed by every keyboard and pointing device `libinput` reports on the hardware backend. The keyboard loads the layout `XKB_DEFAULT_LAYOUT` and the four variables beside it name, and lights the keyboards' LEDs. |
| `xwayland_shell_v1` | 1 | The pairing of an X11 window with the surface Xwayland draws it on. Only the Xwayland server the compositor starts can bind it, so `wayland-info` in any other client does not list it. |

It also answers the desk protocol on
`$XDG_RUNTIME_DIR/coder-desk/<pid>.sock`: `list`, `screens`, `focused`,
`open`, `focus`, `place`, `raise`, `close`, `shape`, `scale`, `notice`,
`reload`, and the verbs that drive the session, `key`, `type`, `click`,
`move`, `press`, `release`, `drag`, `scroll`, and `shot`. One of them, `open` with a `path`, answers
`refused` with a code, listed below. The
socket directory holds one file per session, so the compositor sweeps the
files of sessions that have ended when it starts.

## How it is driven from a shell

The drive verbs feed the compositor's own input path, so a request over the
socket behaves as the keyboard and the mouse do:

- `key <chord>` presses the chord's modifiers and its key on the seat's
  keyboard. The bind filter reads it the way it reads a press from the
  keyboard, so `key super+t` opens a shell and `key ctrl+c` reaches the
  focused window. The chord names its key by what the key prints, and the
  keyboard layout the session loaded decides which keycode presses it, so
  a key the layout has not got is refused by name. Ctrl+Alt with a
  function key, which leaves this session for another virtual terminal, is
  refused: that request belongs to the person at the keyboard.
- `type <text>` presses one key per character, with Shift held for the
  characters that need it. A newline presses Return and a tab presses Tab.
- `click <x> <y> [button]` and `move <x> <y>` drive the pointer at a point
  in the space every screen shares. The pointer moves first, the window
  under the point in draw order takes the pointer focus, and the press
  goes to it. A point no screen holds is refused with the screens named.
- `press <button>` holds a pointer button down where the pointer is, and
  `release <button>` lets it go. The press runs the bind table's mouse rows
  first, so a press with Super held starts the drag that moves and resizes
  a window rather than reaching the client under the pointer.
- `press <key>` holds one key or one modifier down and `release <key>` lets
  it go, through the same bind filter a chord runs through, so a modifier
  held here is the modifier the next press and the next drag read.
- `drag <x1> <y1> <x2> <y2> [button]` is the press, the motion, and the
  release one verb makes, with `--modifier` held across all of it. The
  pointer steps between the two points rather than jumping: a request that
  names no `--steps` or `--ms` runs what a hand runs, because a grab that
  samples the motion can miss one jump.
- `scroll <dx> <dy>` sends a trackpad's smooth axis where the pointer is,
  and `--discrete` sends a wheel's notches instead.
- `shot <path>` writes a PNG of one screen, the screen the pointer is on
  when the request names none, and answers once the file is written. The
  nested backend reads the framebuffer it drew into its window, and the
  hardware backend draws the screen again into a buffer of its own,
  because the DRM compositor scans out the one it drew and keeps it.

```sh
coder-desk key super+t
coder-desk type 'echo hi'
coder-desk key return
coder-desk drag 300 300 700 500 left --modifier super
coder-desk scroll 0 -3 --discrete
coder-desk shot /tmp/screen.png
```

## How it lays out

- Dwindle tiling from `coder-wm`: nine desks, floats, fullscreen, maximize,
  and the split rule the Hyprland session uses. Gaps of 3 and 6 pixels and a
  one-pixel amber border, which is what
  `os/modules/coderos/desktop.nix` sets.
- The tiles fill the screen less every exclusive zone a layer surface
  holds. A window that fills the screen covers the zones, which is what
  Hyprland's `fullscreen 0` does.
- Focus follows the pointer, the way `follow_mouse = 1` does, and holds
  still while a drag is in progress.
- The stack, back to front: the tiles, then the floats in the order the
  layout raises them, then the pinned floats over everything. A tile that
  takes the focus is raised over the other tiles and put back under the
  floats, so the camera circle stays visible whatever the pointer moved
  onto.
  `src/stacking.rs` holds the order, and the log names it, back to front,
  each time it changes. A float goes over the other floats of its layer
  when it takes the focus, once; a `raise` of the float already in front
  moves nothing and logs nothing; and a `shape` changes the window it
  names and nothing else, so the desk you look at, the focus, and the
  order stay as they were, and a `shape` that changes nothing arranges
  nothing.
- The pointer reads the stack in the order the renderer draws it, and reads
  it again when the stack changes: a window that maps, closes, is raised,
  or is restacked under a pointer that holds still takes the pointer or
  gives it up then, and a press goes to the surface under the pointer at
  the press, not to the one the pointer found when it last moved.
- The chords `desktop.nix` binds for tiling, desks, floats, fullscreen,
  monitors, and the two `exec` rows on Super+Return and Super+T. The table is
  `src/binds.rs`; it moves to `crates/coder-binds`, the one table every
  surface reads, once that crate holds layout rows. The launcher rows come
  from that table already: each names the `coderos.desktop`
  option that gates it, the grant lists the options the host turned on,
  and the compositor answers those chords, Super+Shift+D for the deck and
  Super+C for the camera among them. A run
  with `CODER_COMPOSITOR_LAUNCHERS` unset answers every launcher.
- The two mouse rows of that table, the ones `desktop.nix` writes as
  `bindm` lines: Super with the left button drags a window and Super with
  the right button resizes it. The press
  takes the pointer until the button comes up, so the drag keeps its
  window when the pointer crosses another one and the client under the
  pointer gets no press, no motion, and no release. A float moves and
  resizes by the pixels the pointer moved; a tile resizes along the drag
  and does not move, because the layout places it. A window whose rule
  keeps its aspect ratio keeps it, and a resize stops at the floor of 64
  pixels and at the edge of the area the layout fills. `src/drag.rs` holds
  it.
- The window rules `desktop.nix` writes, read from
  [`crates/coder-binds`](../../crates/coder-binds/README.md) when a window
  maps and when its app-id or title changes: the camera circle and the
  recording HUD float pinned with no border, the Battle.net launcher floats
  in the middle of the screen, a game client tiles and is refused
  fullscreen, and the emulator floats. `src/rules.rs` applies them.
- A tile for every child agent of the main session that wants one, and a
  close chord that asks the session in a window before it closes it. Both
  are below.

## Xwayland

The deck, Zoom, the Android emulator, and Battle.net with World of Warcraft
are X11 programs on a CoderOS host, and the `game` tool sends its input to
the game with `xdotool` over XTEST, which only an X server answers. The
compositor runs that server through Smithay's `xwayland` module and is its
window manager. `src/xwayland.rs` is the module.

- **When it starts.** The first time the compositor starts a program: a
  chord's `exec` row, an `open` through the desk protocol, or an agent
  pane. The log says `Xwayland is starting on :1` and then `Xwayland is
  ready on :1`. On a TTY the display is the session's own number —
  `coder-compositor-session` names it in `CODER_COMPOSITOR_X11_DISPLAY`,
  and a name that is taken falls back to the first free display — so a
  host's sessions each hold their own whichever order they started in. A
  compositor nested in a window owns no TTY and takes the first free
  display from `:0`. `Xwayland` has to be on `PATH`; a host without it
  logs one line, and X11 programs have no server.
- **`DISPLAY`.** Every program the compositor starts after that reads the
  server's display as `DISPLAY`, beside `WAYLAND_DISPLAY` and
  `CODER_DESK_SOCKET`, and so does every program one of those starts. A
  program started while no server runs has `DISPLAY` taken out of its
  environment, because the value the session holds names another
  compositor's server.
- **Focus.** An X11 window that takes the keyboard takes the X server's
  input focus with it, and gives it back when the keyboard leaves. The
  X server hands a key to the window that holds that focus, which only a
  window manager sets, so a `WlSurface` focus reached Xwayland's surface
  and an X11 window read no key until it went fullscreen and covered the whole
  root window.
  `src/focus.rs` is the target the keyboard is given: a Wayland surface,
  or an X11 window, whose `enter` sets the focus the way its `WM_HINTS`
  and `WM_PROTOCOLS` ask. A window the client destroyed or has not mapped
  is skipped, because the X server answers a focus or a raise on it with
  `BadWindow`. One `BadWindow` the log still prints is Smithay's: its
  window manager reparents a window back to the root when it unmaps, and
  a client that destroys the window in the same breath, which Wine's
  transient login windows do, leaves that `ReparentWindow` request with
  no window. It is logged and harmless.
- **Layout.** An X11 window takes a tile, floats, and fills the screen the
  way an `xdg-shell` toplevel does, through the same chords and the same
  `shape` request. A window that asks for another size or place is
  answered with the rectangle the layout gives it. A window that sets
  override-redirect, such as a menu or a tooltip, places itself and stays
  out of the layout.
- **Class.** `list` reports an X11 window's app-id as the second string of
  its `WM_CLASS` pair, which is the class Hyprland 0.55 reports. The
  launchers match that string, so they focus an open window on either
  compositor.
- **Scale.** The compositor maps the Xwayland client through the largest
  scale a screen draws at, which is Hyprland's
  `xwayland:force_zero_scaling`: an X11 program's coordinates are the
  screen's pixels, so it draws sharp and smaller rather than being upscaled
  and blurred. A `scale` request changes the mapping with the screen.
- **A server that exits.** Its windows leave the layout, and the next
  program the compositor starts asks for a new server on a new display.

| Launcher | Class the client announces | What the launcher matches |
| --- | --- | --- |
| `os/bin/android-emulator` | `Emulator`, from the pair `qemu-system-x86_64`, `Emulator` | `class:Emulator` |
| `coder-zoom` | `zoom` | `class:zoom` |
| `coder-deck-open` | `coder-deck`, both halves of the pair | an app-id that starts with `coder-deck` |
| `coder-battlenet` | the class Wine sets, such as `battle.net.exe` | an app-id that matches its pattern |

The last three launchers belong to a host flake rather than to this
repository; the bind table keeps their rows so that a host that adds them
gets the chords.

The window rules an X11 window maps under are the rows of
[`crates/coder-binds`](../../crates/coder-binds/README.md), the same rows
`os/modules/coderos/desktop.nix` writes for Hyprland. `src/rules.rs` reads
the table when a window maps, with the rectangle the window asked for, and
again when its class or title changes, which is when a Wine window names
its game. The rules the launchers' windows match:

| Window | Rule |
| --- | --- |
| Class `battle.net.exe`, `Battle.net.exe`, or `steam_app_battlenet` | Floats in the middle of the screen at the size it asked for. |
| A class that starts with `wow`, `world of warcraft`, `sc2`, or `starcraft` in any case, a `steam_app_` class holding `wow` or `sc2`, or a title that starts with `World of Warcraft` or `StarCraft II` in any case | Tiles. A fullscreen or maximize request leaves it in its tile. |
| Class `Emulator` | Floats where it asked, at the size it asked for. The aspect ratio the rule keeps is recorded; the layout crate has no ratio to hold yet. |

The clipboard and the primary selection do not cross between an X11
program and a Wayland one: the compositor answers no selection request from
the X11 side.

## The close chord

Super+W and Super+Q used to close the focused window with nothing asked,
so a window running a live agent closed the same as an idle one and the
agent's work ended with the window. `src/closing.rs` is the same key with
one question in front of it.

The compositor knows the focused window's process, collects every process
under it, and asks the client:

```sh
coder activity --pid <window> --pid <session> --pid <each child>
```

The descendants matter: the window is a terminal emulator and the session
is its child, so the process the compositor holds is the terminal's and
the process that publishes the record is Coder's. What counts as work in
flight is decided once, by `coder activity` in
[`crates/coder/src/activity.rs`](../coder/src/activity.rs): a turn
streaming, or a delegation that has not reported.

A window with work in flight takes a notice saying what is running, and
closes on the press after it, within five seconds and on the same window.
Every other window closes on the first press: an idle session, a window
the client knows nothing about, a window with no process, and a client
that could not answer are one answer here. A key that cannot ask still
closes, because the question exists to save an agent's work rather than to
hold a desktop hostage.

The notice is the compositor's own, so no notification daemon is required
for it. The compositor draws no text yet, so a notice is a bar across the
top of the screen and its words are on the log. A session that runs a
notification daemon on the layer shell has one that draws words; this bar
is what a session running none shows. The desk protocol's `notice` verb
raises the same notice.

`os/bin/coder-close` is the same question on the Hyprland session.

## Hands

Hand tracking is not in this build. In the private repository a tracked hand
is another input device: the compositor reads the camera daemon's landmarks,
turns them into pointer motion, presses, desk switches, and Escape, and draws
the hand over every window. That needs `crates/coder-hands` and the camera
daemon, which move in #9874.

Until then `src/hands.rs` keeps the interface the rest of the compositor
calls, with tracking always off: the desk protocol's `status` verb answers
that hands are off, the overlay draws nothing, and Super+H, where the host
grants it, logs that this build has no tracking. The input paths a hand
drives, `input::pointer_to`, `input::button`, and `input::key_tap`, stay in
`src/input.rs`. #9874 replaces `src/hands.rs`, brings the overlay back, and
adds `coder-hands` to `Cargo.toml`.

## What it does not do yet

- Output hotplug and a second monitor on the hardware, checked by a hand at
  the console. The hardware backend first ran from tty2 on the owner's
  NVIDIA workstation on 2026-09-17: it opened the card, drove `DP-2`, turned
  explicit sync on, read every input device, and started Xwayland.
- The monitors of a second graphics card. The hardware backend drives one
  card, and a monitor on another card stays dark.
- A mode other than the monitor's preferred one, a transform, and a place
  other than left to right. Hyprland's configuration sets `preferred, auto,
  1`, which is what the compositor does.
- Latency and frame pacing on the hardware backend. The nested backend's
  tile input latency and frame pacing were measured on 2026-09-16; the same
  measurement has not run against a compositor on a TTY.
- The keyboard for a layer surface that asks for it, which a lock screen
  needs. `mako` and `slurp` read the pointer, which reaches them.
- `open` with a `path`, which shows a file in a pane. That request is
  refused, because the compositor has no file viewer.
- A `reload` that changes a bind. The compositor reads no file from the
  host while it runs, so the verb answers `done` and changes nothing. The
  bind table and
  the window rules are compiled in, and the keyboard layout is read from
  the environment at start, so a rebuild's change to those reaches the next
  login; the log says so on each `reload`, and the activation script in
  `os/modules/coderos/desktop.nix` sends one after every switch.
  `shape` takes `pin`, `aspect`, `border`, and `shadow`; a border is
  drawn at one pixel or not at all with square corners, and the aspect
  ratio and the shadow are recorded and not drawn, because the layout
  crate has no ratio to hold and the compositor draws no shadow.
- The screens the layout crate holds. This compositor lays its own out in `src/screens.rs`.
- The `overlay_cursor` flag of a screen copy. A copy carries the pointer
  whether the client asked for it or not.
- Hand tracking, which moves in #9874.

It has no popup grab, so a menu closes when its client closes it rather
than on a click outside.

## The loop

The nested backend polls: it reads the window's events, the desk socket,
and the clients, draws a frame every 16 milliseconds, and sleeps 4
milliseconds. It runs one `calloop` loop with no timeout beside the poll,
which the idle notifier's timers fire on and Xwayland's sources run on: the
server's readiness and the X11 events its window manager reads.

The hardware backend runs on `calloop` alone and sleeps until something
happens: the clients' socket, `libinput`, the seat, `udev`, each card's
vertical blanks, and a ping the desk socket's thread sends after it hands the
loop a request. Each screen draws after its vertical blank; a frame with
nothing new is not sent to the monitor, and the screen looks again one
refresh later.

A copy a screencopy client asked for is taken on its screen's next frame,
from the same elements the frame draws, so what the client saves is what
the monitor shows. The nested backend reads the copy out of the framebuffer
it drew and hands the pixels over after the swap, because mapping a read
makes the renderer's own context current and the swap needs the window's.
The hardware backend draws each copy into a texture of its own, or into the
client's dmabuf, because the DRM compositor keeps the framebuffer it scans
out.

## Tests

```sh
cargo test -p coder-wm -p coder-compositor
```

The desk protocol's server is `coder_desk::serve`, so this compositor
answers the protocol through the same module a second desk would, and its
tests live there: they bind a socket, answer from a fixture of tiles, and
read the answers back over a connection. The hardware backend binds the same
socket with `serve::bind_waking`, which pings its loop after each request.

The tests need no display and no graphics card. The backend tests pick a
backend from a list of arguments and an environment; the screen tests lay two
screens out, scale them, and run the monitor chords over the layout crate;
the connector tests run hotplug over a fake list of connectors shaped like
the owner's NVIDIA card; the desk server tests set a scale and put
it back through a `scale` request and answer a `reload` over a socket;
the render tests move rectangles onto a
scaled screen; the cursor tests pick a theme's image and draw the fallback
arrow; the layout
tests spawn and close tiles through `coder-wm` and check the pixels,
including the pixels an exclusive zone takes; the bind tests read every
chord out of the table, with the launcher rows a grant adds and leaves
out; the stacking tests put the tiles, the floats, and the pinned floats
in their order and hold that a raise of the float already in front moves
nothing; the input tests hold that a press on a pinned float over a tile
moves the pointer onto the float; the close tests run the chord's
decision over a fixture of what the client answered; the screencopy tests
check the rectangle a request names, its pixels on a scaled screen, and
where the renderer reads it for either backend; the
selection tests run the clipboard, the primary selection, and a drag
through their states; the idle tests fold a run of input into one
report; the rule tests put a window into the layout under each rule the
table holds and check that the camera circle and the HUD float pinned
on every desk; the Xwayland tests read the class the desk reports for
each launcher's client, check that a started program reads `DISPLAY`, and
hold that a mapped X11 window takes the focus and a destroyed one is
skipped.

A protocol needs a client to prove it, and the tests have no display, so the
checks against real clients are by hand. Open the compositor nested, then
against the socket it announces:

```sh
export WAYLAND_DISPLAY=wayland-2
foot                                 # a window to capture
grim screen.png                      # the whole screen
grim -g "0,0 100x60" region.png      # a rectangle of it
wf-recorder -f screen.mkv            # a recording, ended with Ctrl+C
echo hello | wl-copy && wl-paste     # the clipboard
echo hello | wl-copy --primary && wl-paste --primary
wtype hello                          # typed into the focused tile
mako & notify-send hello hi         # a notification on the top layer
swayidle -w timeout 3 'echo idle' resume 'echo back'
wayland-info                         # every global and its version
```

The X11 checks run against the display the log names, here `:1`:

```sh
coder-desk open -- xeyes             # starts Xwayland, then a tile
coder-desk list                      # the tile, with app_id XEyes
DISPLAY=:1 xdotool search --class XEyes
coder-desk open -- "xterm -class Wow.exe -fullscreen"   # tiles, not fullscreen
coder-desk open -- "xterm -class battle.net.exe"        # floats, centered
coder-desk open -- "xterm -class Emulator"              # floats
```
