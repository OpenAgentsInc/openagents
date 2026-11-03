# Battle.net and WoW in Verse OS

On coderos-4080, the installed Verse build is
`~/.local/share/verse-os/current`. It reuses the existing Battle.net prefix
at `~/Games/battlenet` and the host's installed Proton runner.

Start the Verse in a window from the desktop:

```sh
d="$HOME/.local/share/verse-os/current"
. "$d/env"
export LD_LIBRARY_PATH
VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json \
  "$d/verse-session" --dir "$d" --window --desk-size 2548x1428 \
    --state "$HOME/.local/state/verse" --run "$XDG_RUNTIME_DIR/verse-session"
```

F10 shows the desk flat; its terminal starts automatically. Super+G, or
`verse-battlenet` in that terminal, opens Battle.net. Select the installed
World of Warcraft Classic / Classic Era and Play. Super+F on the focused
game gives it the desk's full rectangle. F9 docks the desk beside the Grid;
F8 places its screen in the world.

Close a Battle.net/WoW instance on the old desktop before launching one on
the new desk. The launcher refuses a competing instance rather than starting
two Wine containers against one prefix. `verse-battlenet --check` checks the
session and installation without starting it.

From the Verse desk's terminal:

```sh
verse-game list
verse-game look /tmp/wow.png
verse-game press Tab
verse-game hold w 200
verse-game click 400 300 left
verse-game drag 80 0 right
verse-game stop
```

From another terminal or an agent, select the Verse socket explicitly:

```sh
d="$HOME/.local/share/verse-os/current"
"$d/verse-game" --socket "$XDG_RUNTIME_DIR/verse-desk.sock" look /tmp/wow.png
"$d/verse-game" --socket "$XDG_RUNTIME_DIR/verse-desk.sock" hold w 200
```

The default title is exactly `World of Warcraft`. `--title "Battle.net"`
controls the launcher. `list` reports handles and revisions; `--handle N`
and optionally `--revision N` narrow selection while still requiring the
expected title. Ambiguous or stale selection refuses.

`look` writes a PNG and a `.png.json` sidecar with the capture sequence,
session, target lifetime, desk rectangle and imported buffer size. `.ppm`
output skips PNG conversion. Clicks use desk pixels; drags use relative
pixels. Holds are 1–2000 ms. Keys include letters, digits, Tab, Enter,
Escape, Space, Shift, Ctrl, Alt, arrows and F1–F12.

Each action owns one short connection lease. Closing or killing the CLI,
losing focus, physical input, or expiry releases its owned input. `stop`
also works from a separate local connection. A long-running controller can
use [desk-control-v2](../protocol/desk-control-v2.md) directly; it must keep
the connection open while holding a key/button.

`--desk-size` sets both the compositor and the panel image, each side
64–4096 pixels. Match the Verse window's actual pixel dimensions to avoid
upscaling. The launcher now sets each installed WoW variant's saved window
resolution to that desk size before starting Battle.net, and the main
Battle.net/WoW surfaces use the whole desk automatically. Close a running
game before changing the desk size. The launcher restores borderless mode
after the host runner prepares its window settings. A fresh launch was
qualified at 2548×1428 and 100% render scale.

## Recorded waypoint navigation

The installed bundle includes `verse-navigate`, `verse-sense` and the
read-only `wow/VerseNavigation` addon. On this machine the addon is already
installed in Classic Era. For a new installation, copy that addon directory
to the game's `Interface/AddOns/VerseNavigation` before starting the client;
fully restart a client that was already running. Enable the addon and enter
the world. Its colour strip appears at the top-left of the game.

Capture the current public-API position/facing sensor:

```sh
d="$HOME/.local/share/verse-os/current"
"$d/verse-navigate" --mode observe --out /tmp/wow-observe-01
"$d/verse-sense" /tmp/wow-observe-01/observe-1.ppm
```

Each output directory must be new and absolute. The navigation tool defaults
to `$XDG_RUNTIME_DIR/verse-desk.sock`; `--socket PATH` overrides it. It records
images, capture identities, sensor samples, model distributions, actions and
the terminal outcome in that directory's `journal.jsonl`.

The qualified profile is Dun Morogh, map 1426. Coordinates are the sensor's
normalized integers from 0 to 65535. Select a nearby unobstructed waypoint
after inspecting a capture, then supply its coordinates:

```sh
"$d/verse-navigate" --mode elephant --map 1426 \
  --x 19670 --y 46120 --radius 12 --steps 40 --out /tmp/wow-route-01
```

That coordinate is an example from the recorded trial, not a destination
appropriate to every starting position. `--mode rules` explicitly selects
the geometric baseline. Elephant mode requires the existing Psionic service
on `http://127.0.0.1:8790/v1/systemone` with the original pinned Elephant
identity in `os/game-navigate.coax`. On coderos-4080, the existing GPU switch
at port 8791 loads it with `POST /use/elephant`; check `/status` until ready.
No CPU whole-model run, remote service or toy fallback is used.

Stop a running controller and release held input with:

```sh
touch /tmp/wow-route-01/stop
"$d/verse-game" --socket "$XDG_RUNTIME_DIR/verse-desk.sock" stop
```

Actions last at most 300 ms in this profile. No key is held during model
inference. Death, combat, a menu/chat focus, stale sensor, changed target,
no progress, invalid decision or action budget end stops further input.
The stop file ends the loop; the separate stop command releases held input
immediately. Killing the controller releases its lease too.

This provides feedback-controlled local waypoints. The learned model gets
semantic left/right/ahead facts rather than pixels. Route selection around
arbitrary obstacles, combat and quest planning are not implemented. See the
[protocol](../protocol/external-navigation-v1.md),
[work journal](verse-navigation.md) and
[measured trials](../verification/verse-navigation-2026-11-03/README.md).

The first accelerated path retains CPU desk composition and frame upload;
it is not zero-copy. Continuous turning across cursor edges and a
sustained frame-rate target are not yet qualified. Actual launch, movement,
camera and cancellation evidence is in the
[verification record](../verification/verse-os-gaming-2026-11-03.md).
