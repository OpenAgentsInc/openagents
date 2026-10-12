# Studio handoff (#11432)

State of the Blender device studio for the V1 launch film as of
October 12, 2026, when work paused. The scripts live in
`scripts/film/studio/`. The shared contract is
[cinematic.md](cinematic.md).

## What exists

- **`devices.py`** builds three models at real size:
  - A MacBook Pro 14 or 16 (`macbook`).
  - An iPhone 17 Pro (`iphone`).
  - A Studio Display–style 27-inch 5K monitor on its tilting stand
    (`display`).

  Each screen plays a movie. `fit` is `cover` (the default) or `contain`.
  `contain` shows the whole clip with black bars and is used for web footage
  on the 16:9 display. A negative `in` delays a clip and holds its first
  frame until then, which suits boot clips.
- **`studio.py`** renders one shot spec. It handles:
  - A single device, using the older `device`, `screen` and `device_motion`
    fields.
  - Several devices, listed under `devices`. Each has its own `pose`,
    `screen` (`video`, `in`, `brightness`, `fit`) and optional `motion`.
    All of them run on the shot's one clock.

  Clip paths may use `$SYNC`, which defaults to `~/work/film-out/sync-v1`.

  Camera keys give either `pos` or `orbit`, plus `target`, `lens` and
  `fstop`. The keys are eased over the whole shot. A rack focus goes in
  `camera.focus_track` as `[{t, at, pull}]`. It runs in real shot time, so a
  pull lands exactly on a sync beat.

  The stage is the Verse Grid, built from the grid pack's numbers, under
  neutral white light. The front glass and the floor are light-linked away
  from the softboxes.

  A failed render now deletes its frames.
- **`render-all.sh`** renders shots by name.
  - Single-device shots take the newest footage, from `footage-v3`, then
    `footage-v2`, then `footage`.
  - Multi-device shots read `$SYNC`.
  - `--standin` falls back to the stand-in clips.
  - `--resolution=WxH` sets the output size.
  - `--preview` renders at quarter size.
- **`standin.py --sync [DIR]`** writes shared-clock stand-ins and a
  `beats.json` to
  `~/work/film-out/studio/standin/sync/`:
  - `terminal`, `desktop`, `web` and `phone` clips.
  - `boot-*` clips for each of the four screens.

## Shots

### Single-device

| Spec | Output | Footage | Status |
|---|---|---|---|
| `macbook-chat` | `~/work/film-out/studio/macbook-chat.mp4` | footage-v3 `web-chat` | 4K final. A coordinator v6 run re-rendered it with the fixed orbit. 1080p copy in `studio-1080/`. |
| `macbook-terminal` | `studio/macbook-terminal.mp4` | footage-v3 `terminal-coder` | 4K final, also in `studio-1080/` |
| `macbook-sync` | `studio/macbook-sync.mp4` | footage-v3 `web-sync` | 4K final from the 12:47 run. The later v6 re-render FAILED. 1080p copy in `studio-1080/`. |
| `iphone-approve` | `studio/iphone-approve.mp4` | footage-v2 `phone-app` (iOS app) | 4K final |
| `grid-plate` | `studio/grid-plate.mp4` | none (the empty Grid) | 4K final, 14 s, for the title and end cards |

### Multi-device sync

All five shots are 1080p review shots. Each has been previewed with
stand-ins only and not yet rendered from the real footage.

| Spec | Length | Content |
|---|---|---|
| `suite-boot` | 6.5 s | The trio boots: dark, then the app, then the home screen. The camera cranes down and pushes in. |
| `sync-terminal-web` | 7.5 s | MacBook in front, display behind. Focus racks to the display as the terminal's message lands there. |
| `sync-phone` | 7.5 s | iPhone in front replying. Focus racks to the MacBook terminal as the reply lands. |
| `sync-web-back` | 6.5 s | Display in front typing. Focus racks to the MacBook terminal as the reply lands. |
| `trio-wide` | 5.5 s | All three live, in a slow orbit. |

All of them extend `sync-base.json`, which holds the background and light
rig. The trio layout is:

- MacBook at (-0.28, 0.02), rotated 16°.
- Display at (0.04, 0.42), rotated -3°.
- iPhone at (0.30, -0.14), standing.

`sync-web-back` has its own layout.

The real shared-clock footage arrived at `~/work/film-out/sync-v1/` (`terminal`, `web`,
`phone`, `beats.json`) just before the pause. The in-points are already set
from its `beats.json` clip times:

- **`sync-terminal-web`**: in at 2.5 s. The message is sent 2.5 s into the
  shot, and the web sidebar shows it at 3.6 s, where the focus lands.
- **`sync-phone`**: in at 16.0 s. The phone sends at 2.8 s and the terminal
  shows the reply at 4.4 s, where the focus lands.
- **`sync-web-back`**: in at 28.8 s. Web typing starts at 1.5 s, the reply is
  sent at 2.9 s, and the terminal shows it at 5.0 s, where the focus lands.
- **`trio-wide`**: in at 16.5 s, so the phone reply lands mid-shot.

Check each of these against a render before approving. The real clips are
cut, so some beats sit right at a cut boundary.

`suite-boot` still points at `$SYNC/boot-desktop.mp4`, `boot-web.mp4` and
`boot-phone.mp4`. Those clips don't exist in `sync-v1` yet. There is no
`desktop.mp4` either; the MacBook shows the terminal in every sync shot.

## Commands

From an `openagents` checkout:

```sh
# 1080p review of the sync shots from the real shared-clock footage
OUT=~/work/film-out/studio-1080 scripts/film/studio/render-all.sh --resolution=1920x1080 \
  sync-terminal-web sync-phone sync-web-back trio-wide
# suite-boot, once sync-v1 has boot-desktop/boot-web/boot-phone (or rename in the spec)
OUT=~/work/film-out/studio-1080 scripts/film/studio/render-all.sh --resolution=1920x1080 suite-boot
# The same with stand-ins (regenerate them with: python3 scripts/film/studio/standin.py --sync)
SYNC=~/work/film-out/studio/standin/sync OUT=~/work/film-out/studio-1080 \
  scripts/film/studio/render-all.sh --resolution=1920x1080 suite-boot
# Single-device 4K finals (the default size); add --resolution=1920x1080 for review
scripts/film/studio/render-all.sh macbook-chat macbook-terminal macbook-sync iphone-approve grid-plate
# One look-dev still at T seconds, without a full render
SYNC=~/work/film-out/sync-v1 /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P scripts/film/studio/studio.py -- --shot scripts/film/studio/shots/sync-phone.json \
  --out /tmp/look/sync-phone.mp4 --still 3 --resolution 1920x1080 --samples 16
```

The output encoding is H.264, yuv420p, CRF 12 at 30 fps. The default is
40 samples with GPU denoise.

### Render times

These were measured on the M5 Max while it was loaded:

- About 8–10 s per frame at 4K.
- About 2–4 s per frame at 1080p.
- About 0.5–0.6 s per frame for a 1080 `--preview`.

A 4K render needs a few GB of scratch frames. Delete
`~/work/film-out/studio/*.frames` after a failed run. `studio.py` now cleans
up after itself, but a killed process leaves its frames behind.

## Left to do

1. Render the four sync shots at 1080p from `sync-v1`. Check that each beat
   happens on screen mid-shot, then nudge `in` and the `focus_track` times.
2. Get the boot clips recorded, then render `suite-boot`.
3. Re-render `macbook-sync` at 4K. Its last run failed, so the existing file
   is the 12:47 render.
4. Once the owner approves the 1080p review, render the multi-device shots at
   4K by dropping `--resolution`.

## Known weaknesses

- **Small screens in wide shots.** In the trio and wide shots, each screen is
  small. The text reads only in the two-device shots, and only on the device
  in focus.
- **DOF hides the Grid.** At shallow depth of field the Grid's thin lines
  blur away behind a close-up. The Grid reads mainly in wide shots and on the
  floor next to a device.
- **Dim display edges.** The display's aluminum edges and back are barely lit
  from the front, so the panel reads as a black slab with a bright stand.
- **Lights tuned per framing.** Light placement is tuned per framing. A new
  camera angle can bring a softbox into a silver deck or the stand, and the
  light then has to move. `"brightness"` in `sync-base.json` scales the whole
  rig.
- **Missing details.**
  - No key legends on the MacBook keyboard.
  - No "MacBook Pro" chin text.
  - No ports on the iPhone.
  - No camera in the display bezel.
- **Mid-move framing.** `macbook-terminal` passes through a framing with the
  screen pushed off to the right partway through its push-in.
