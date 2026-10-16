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

  Clip paths may use `$SYNC`, which defaults to `~/work/film-out/sync-v2`.

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

Cut 5's feature beats (1080p in `studio-1080/`, footage-v8, each 4–4.5 s
on a slow push toward the part of the screen that moves):
`macbook-plugins` (upper-left: the list, the ACP page),
`macbook-harness` (the input's right rail), `macbook-tray` (the input and
the agent row under it), `macbook-rewind` (the whole screen; not in cut 5).

### Multi-device sync

`four-apps-intro` places the Episode 290 shared-clock capture on two MacBooks
(terminal and native desktop), the display (web), and the iPhone. It uses the
same Grid, lighting, and camera crane as `suite-boot`, over eight seconds.
Render with `SYNC=~/work/film-out/episode-290-four-apps/intro` and
`render-all.sh --resolution=1920x1080 four-apps-intro`.

All five are rendered at 1080p from `sync-v2` into `~/work/film-out/studio-1080/`
(cut 3); 4K waits for the owner's approval.

| Spec | Length | In | Content |
|---|---|---|---|
| `suite-boot` | 6.5 s | boot clips | The trio boots on the Grid: the iPhone app opens from the home screen, the terminal starts Coder on the MacBook (in at -1.4 s), the website opens on the display (in at -2.2 s). The camera cranes down and pushes in. |
| `sync-terminal-web` | 7.5 s | 2.3 | MacBook in front, display behind. The terminal message is sent at 2.4 s; focus racks to the display at 3.5 s as it lands in the sidebar; the thread opens at 4.7 s. |
| `sync-phone` | 7.5 s | 14.0 | The whole iPhone in front: the reply is sent at 2.7 s; focus racks to the MacBook at 4.1 s as it lands in the terminal. |
| `sync-web-back` | 7.0 s | 26.0 | Display in front: typing at 2.4 s, sent at 3.8 s; focus racks to the MacBook at 5.4 s as the terminal takes it. |
| `trio-wide` | 5.5 s | 33.4 | All three live, a slow orbit, as the commit lands on every screen (3.0 s). Tighter than before (orbit 1.3 → 1.25 m) and the display brighter (1.6) so the screens read. |

All of them extend `sync-base.json`, which holds the background and light
rig. The trio layout is:

- MacBook at (-0.28, 0.02), rotated 16°.
- Display at (0.04, 0.42), rotated -3°.
- iPhone at (0.30, -0.14), standing.

`sync-web-back` has its own layout. In the original sync shots, the MacBook
shows the terminal; `four-apps-intro` adds the native desktop app on a second
MacBook (see `handoff-footage.md`).

## Commands

From an `openagents` checkout:

```sh
# 1080p review of the sync shots from the real shared-clock footage
SYNC=~/work/film-out/sync-v2 OUT=~/work/film-out/studio-1080 scripts/film/studio/render-all.sh --resolution=1920x1080 \
  sync-terminal-web sync-phone sync-web-back trio-wide
# suite-boot (boot-terminal, boot-web, boot-phone)
OUT=~/work/film-out/studio-1080 scripts/film/studio/render-all.sh --resolution=1920x1080 suite-boot
# The same with stand-ins (regenerate them with: python3 scripts/film/studio/standin.py --sync)
SYNC=~/work/film-out/studio/standin/sync OUT=~/work/film-out/studio-1080 \
  scripts/film/studio/render-all.sh --resolution=1920x1080 suite-boot
# Single-device 4K finals (the default size); add --resolution=1920x1080 for review
scripts/film/studio/render-all.sh macbook-chat macbook-terminal macbook-sync iphone-approve grid-plate
# One look-dev still at T seconds, without a full render
SYNC=~/work/film-out/sync-v2 /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
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

1. Re-render `macbook-sync` at 4K. Its last run failed, so the existing file
   is the 12:47 render.
2. Once the owner approves the 1080p review, render the multi-device shots at
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
