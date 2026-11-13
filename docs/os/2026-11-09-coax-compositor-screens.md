# Several monitors, turned monitors and the main screen in the Coax compositor (plan)

Written 2026-11-09. Status: steps 1 to 3 built (see Progress at the end); 4 to 6 are still plans. It
extends [2026-11-02-verse-os.md](2026-11-02-verse-os.md), whose display row
lists "more than one connector; hotplug" as missing.

## Why now

`coderos-4080` now has two monitors:

| Monitor | Connector | Card | Stands |
| --- | --- | --- | --- |
| 1440p, the main screen | `HDMI-A-3` | RTX 4080 (`card2`) | on the right, upright |
| 1080p | `HDMI-A-2` | the motherboard's Intel graphics (`card1`) | on the left, on its side |

The Rust `coder-compositor` on tty1 drives both since `~/coder` commits
`186d07677` (a monitor on a second card is drawn on the 4080 and copied
across) and `10ca12924` (`/etc/coderos/screens.json`: the order the screens
stand in and each one's quarter turn; the card that draws opens first, so
its monitor is the main screen). The Coax compositor on tty2 does none of
this, and it is the one Metaverse OS keeps.

## Where `os/compositor.coax` stands

- **One monitor.** `coax/platform/screen-native.c` (`cxscr_open`) takes the
  first `/dev/dri/card*` with a connector, unless `CODEROS_COMPOSITOR_CARD`
  names one, and the first connected connector on it. On this machine that
  is `card1`, the motherboard's port, so on tty2 today the main monitor is
  the one that stays dark.
- **Drawn on the CPU into dumb buffers.** This is what makes several cards
  easy: each card gets dumb buffers of its own, and no buffer crosses from
  one GPU to another. The Rust compositor needed Smithay's multi-GPU
  renderer for the same thing, and fell back to a CPU copy anyway.
- **The window policy already holds several screens.** `os/wm.coax` has
  `WmScreen` (name, position, size, scale, desk) and `desk_screen`, and the
  headless `--screens WxH,...` mode uses it. Only `--screen` (real hardware)
  is limited to one.
- **One canvas.** Every screen is drawn into one `CxFrame` of at most
  `CX_MAXW` x `CX_MAXH` (3840 x 2160). Two screens here fit (1080 + 2560 =
  3640 wide, 1920 high); a third would not.
- **The desk socket reports one screen.** The `screens` answer is a fixed
  row named `verse`.

## Steps

1. **The screen rules in Coax: `os/screens.coax`.** Read the same
   `/etc/coderos/screens.json` the Rust compositor reads (or the file
   `CODER_COMPOSITOR_SCREENS` names):

   ```json
   { "order": ["HDMI-A-2", "HDMI-A-3"], "rotate": { "HDMI-A-2": 90 } }
   ```

   Decide where each screen stands (left to right in `order`, tops aligned;
   a screen `order` leaves out stands to the right), which screen is the
   main one (the monitor on the card that draws: desk 1, the focus, new
   windows), and the arithmetic of a turn: 90, 180 and 270 degrees
   counter-clockwise, as a `wl_output` transform of the same number. The
   checks come first: a turn and its reverse give back the picture
   (`#forall` over small sizes), a quarter turn swaps width and height, a
   turn that is not a quarter stays upright, and the order rules match the
   Rust tests in `bins/coder-compositor/src/screens_tests.rs`.

2. **The C side drives every monitor on every card.** `screen-native.c`
   turns its one `scr_*` set into a list of heads: open each
   `/dev/dri/card*`, become DRM master on each, and give each connected
   connector a CRTC of its own, two dumb buffers and its own page-flip
   state. Name each head as the kernel does (`drmModeGetConnectorTypeName`
   plus the type id: `HDMI-A-2`), so the file above matches. A VT switch
   drops and takes master on every card. The C side only copies rows a
   frame hands it; what to draw and how it is turned stays in Coax.

3. **The compositor draws each screen on its own.** Replace the one canvas
   with one frame per head, each at its own size, so the 3840 limit becomes
   a per-screen limit. Composite each head in its turned orientation (the
   turn is Coax arithmetic on the copy). Keep damage per head. Let the
   pointer cross the screens and clamp it to their union. Announce one
   `wl_output` per head with its transform (globals past 8 are already the
   outputs of screens 2 and on). Answer `screens` on the desk socket with
   the real heads. Open the main card first.

4. **Nix.** The tty2 session passes the screens file and names the main
   card by a stable path (`/dev/dri/by-path/...`, not `card2`, whose number
   can change between boots). Move the `screens` option into
   `os/nix/modules`, since CoderOS becomes Metaverse OS; until then
   `coderos.desktop.screens` in `~/coder` writes the file both compositors
   read.

5. **Checks.** Headless: `--screens 1080x1920,2560x1440` with a turn, and a
   shot of each screen compared with a recorded picture. On the machine: a
   run on tty2 with a test window on each monitor, read by the owner.

6. **Later: monitors arriving and leaving** while the compositor runs (a
   DRM uevent, or a connector poll on a timer), and **the Metaverse session**:
   `verse/gpu/vk-display.c` takes the same rules, turns the picture in the
   display plane when `VkDisplayPlaneCapabilitiesKHR` supports the
   transform and in the shader when it does not, and draws on the 4080 with
   a copy to the Intel card's display.

## A quick fix that can land on its own

Name the 4080 in the tty2 session (`CODEROS_COMPOSITOR_CARD`), so tty2
shows the main monitor rather than the side one until step 3 lands.

## Known issue in the Rust compositor

A screenshot or screencopy of the turned screen comes out garbled, though
the monitor itself shows the picture upright (checked by the owner on
2026-11-09). Screen recording of that screen is affected until it is fixed.

## Progress

**2026-11-09: steps 1 to 3.**

- `os/screens.coax`: the arrangement file, the order, the turn and its
  arithmetic, with `#test` and `#forall` checks (run through
  `coax test os/compositor.coax`, which loads it).
- `coax/platform/screen-native.c`: `cxscr_open_all` takes every connected
  monitor on every card (the boot card first, or the cards
  `CODEROS_COMPOSITOR_CARD` names, split by `:`), each with its own CRTC,
  buffers and page flips (`page_flip_handler2`, so two monitors on one card
  are told apart). A card another program holds is passed over.
  `cxscr_open`, `cxscr_present` and the rest still work, as monitor 0.
- `os/compositor.coax`: `--screens-file` (default
  `/etc/coderos/screens.json`); each screen keeps its monitor, turn and own
  size; every monitor's rows are filled from the picture through
  `sc_walk` and shown with `cxscr_head_present`; `wl_output` sends the
  monitor's own mode with the turn as its transform; the desk socket's
  `screens` answer lists every screen; `--screens WxH@DEG` gives a headless
  screen a turned monitor, and the `panel N PATH` command writes what a
  monitor shows.
- Checked headless (`--screens 1920x1080@90,2560x1440`: a 3640 x 1920
  picture, the turned monitor's rows turned a quarter counter-clockwise),
  and on `coderos-4080`'s side monitor, which showed a terminal upright
  with `rotate = { "HDMI-A-2" = 90; }`, as the Rust compositor does with
  the same file (owner, 2026-11-09).

Not done in step 3: the screens still share one picture of at most
3840 x 2160 (enough for this machine's 3640 x 1920), rather than one frame
each; the pointer can wander into the part of the picture no screen
shows when the screens differ in height; and only one card has been
driven on hardware so far, because the Rust desktop holds the other.
