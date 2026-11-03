# Adaptive graphics quality

Status: v1 shipped in Everglade (`/worlds/everglade`) and Everglade 5E
(`/worlds/everglade-5e`), 2026-11-03.

Owner report: "Effortlessly get super laggy on the phone as soon as I destroy
something." This spec says what a destroy costs, how a Verse world picks its
graphics settings, and how it changes them while you play.

## What a destroy costs

Measured in headless Chrome on the M-series Mac (`coax/web/tool.sh
destroy-fps`, below), on Everglade 5E at the market, with three destroying
casts (Meteor Swarm, Mega Thunderbolt, Thunderbolt) at the nearest house.
These are the old desktop budgets, which `?quality=high` still uses:

| What | Idle | During the destroy |
| --- | ---: | ---: |
| Fire, smoke and glow puffs drawn a frame | 0 to 20 | 570 to 730 |
| Debris chunks alive (budget 260) | 0 | 225, then 260 |
| Bodies awake (stepped by the physics) | 0 | up to 200 |
| Splinters (budget 120) | 0 | 120 |
| Dust puffs (budget 56) | 0 | 56 |
| Wasm frame time, CPU throttled 6x | 3.1 to 3.4 ms | 4.6 to 5.1 ms |
| GPU frame span, 390x844 | 5.6 to 5.9 ms | 6.8 to 7.7 ms |

Before this change, a phone halved only the chunk and loose-piece budgets
(130 and 20). It still drew every puff, splinter and dust cloud, and it drew
at up to 1.5 device pixels per CSS pixel.

The cost is on the GPU first:

- Each chunk draws its piece's whole mesh, cut to its cell in the fragment
  shader (`everglade.wgsl` `LOOK_SHARD`). So 260 chunks draw 260 meshes.
- The puffs are soft, camera-facing, blended sprites. A blast close to the
  camera covers much of the screen several times over, and fill is what a
  phone's GPU has least of.
- Both costs scale with the pixels drawn. A phone at 1.5 device pixels per
  CSS pixel draws 2.25 times the pixels of the headless runs above.

On the CPU, the wasm frame grows about 45 % during a destroy. In a CPU
profile (4x throttle, same casts) the growth is spread out: the chunks'
physics and contacts, `demo_stand` (what you stand on) scanning every body
for rubble, the fire's flames, and the guards' response. No single function
dominates, so v1 targets the GPU costs and caps the debris, which is most of
the CPU growth that scales with the destroy.

## The strategy

A shared module, `aethyr/quality.coax`, owns the quality. A world keeps one
`Quality`, gives it the page's facts once, gives it every frame's length, and
reads its knobs. Worlds never make their own ad hoc quality checks.

### The ladder

The quality is a rung on a ladder of seven. Each rung down turns down one
kind of cost, costliest first:

| Rung | Tier shown | Pixels a CSS pixel | Debris | Splinters, dust | Puffs | Far trees |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 6 | HIGH | 3.0 | 100 % | 100 % | 100 % | 100 % |
| 5 | HIGH | 2.0 | 100 % | 100 % | 100 % | 100 % |
| 4 | MED | 1.5 | 100 % | 100 % | 100 % | 100 % |
| 3 | MED | 1.5 | 50 % | 50 % | 40 % | 100 % |
| 2 | MED | 1.0 | 40 % | 40 % | 30 % | 100 % |
| 1 | LOW | 1.0 | 30 % | 25 % | 20 % | 80 % |
| 0 | LOW | 0.75 | 30 % | 25 % | 15 % | 70 % |

- **Pixels.** The world tells the host how many pixels to draw at most
  (`coaxial_pixel_budget`); the host draws fewer and the browser scales the
  canvas up. On a phone this is the biggest saving.
- **Debris.** The share of the chunk budget (260) and the loose-piece budget
  (40), and of the chunks one break makes (24). A break still breaks, into
  fewer and bigger chunks. When the budget drops below what is there, the
  oldest rubble lying still goes at once (`demo_caps` in
  `verse/demolish.coax`).
- **Splinters and dust.** The share of the splinters (120) and dust (56)
  alive at once, and of how long a splinter lies (40 s).
- **Puffs.** The share of the fire, smoke and glow puffs (1,400) drawn a
  frame, with at least 160 kept. Past that, the frame drops evenly spaced
  puffs and makes the ones it keeps denser, up to 1.6 times
  (`sp_fx_thin` in `everglade-spells.coax`). A blast still looks like a blast.
- **Far trees.** The share of the kit trees' reach and count.

Spells and gameplay never change. Damage, physics, what breaks and what the
other players see stay the same; only how much of it is drawn changes.

### The start

At load the world picks a rung from the page's facts (`host.js` `pageFacts`:
`vw`, `vh`, `dpr`, `coarse`, `touch`, `cores`, `memory`):

- A phone (touched, or under 700 CSS pixels across) starts at MED (rung 3),
  or LOW (rung 1) with 4 cores or fewer or 3 GB or less.
- A computer starts at HIGH (rung 6), or MED with 2 cores or fewer or 2 GB or
  less.
- A number the browser doesn't give (Safari has no `deviceMemory`) counts for
  nothing.

### The governor

In AUTO, the governor moves the rung. It keeps a long average of the frame's
length (2 s time constant) and a short one (0.25 s):

- It ignores the first 3 s (shaders and the pack streaming in) and any frame
  over 0.25 s (a hidden tab, a stall).
- **Down one rung** when the long average is over budget (over 18.7 ms at
  60 FPS) for 1 s.
- **Down one rung at once** when the short average is in a burst (over 30 ms)
  for 0.25 s. This catches a destroy before the long average does. At most
  one step down every 0.5 s.
- **Up one rung** after 5 s of frames on time. A display at 60 Hz shows no
  headroom in its frame times, so the step up is a probe. If the world has to
  step down again within 10 s, that rung waits twice as long next time, up
  to 80 s. The quality settles instead of going up and down.
- A page that can't hold 60 FPS on rung 0 for 3 s gets a 30 FPS budget and
  climbs again from there. It goes back to 60 FPS once frames hold 60 for
  10 s.

### What the player sees and picks

- The tier is always on screen, small, at the bottom right: `GFX AUTO · MED`,
  or `GFX LOW` for one the player picked. In Everglade 5E the FPS counter
  sits to its left on the same line.
- A click or a tap on it picks the next: AUTO, HIGH, MED, LOW, then AUTO
  again. A picked tier holds its rung; the governor doesn't move it.
- The host keeps the pick in `localStorage` `verse.quality` (action 6), so it
  stays for the next visit. `?quality=low|med|high|auto` in the URL wins over
  the kept one and is kept in its place.
- There are no warnings or banners.

Golden frames hold the screen's tier, decided from its size and touch alone,
so a golden is the same on every machine. They draw the full pixels as
before (`coaxial_pixel_budget` returns 0) and don't show the tier.

## Where it lives

| File | What |
| --- | --- |
| `aethyr/quality.coax` | The ladder, the start rung, the governor, the label. Checks for each rule. |
| `verse/demolish.coax` | `demo_caps`: the debris, splinter and dust budgets. |
| `apps/openagents-web/everglade-quality.coax` | Everglade's side: facts, the per-frame call, the pixel budget, the control, a cost probe. |
| `apps/openagents-web/everglade-spells.coax` | The debris budgets each frame; `sp_fx_thin`. |
| `apps/openagents-web/everglade.coax` | The far trees; the control in the HUD and its taps. |
| `apps/openagents-web/everglade-face.c`, `everglade5e-face.c` | The governor each frame; `coaxial_pixel_budget`; `coaxial_quality_probe`. |
| `coax/web/host.js` | `auto` as a fourth tier, `cores`, `memory` and the kept tier in the page's facts, `globalThis.coaxial.quality(field)`. |
| `coax/web/destroy-fps.coax` | The measuring tool. |

A new world uses it the same way: `#load "../../aethyr/quality.coax"`, start
it from the facts, call `quality_frame` each frame, and read
`quality_knobs`.

## Measuring

`coax/web/tool.sh destroy-fps URL --eval JS` opens a page in headless Chrome
as a touch phone (`--size 390x844 --dpr 3`), optionally with a slower CPU
(`--cpu 6`), runs a script once the page is ready, and prints what it
returns. `--before JS` runs a script before the page's own (a timer around
`coaxial_frame`), `--profile OUT.json` writes Chrome's CPU profile of the
run, and `--shot OUT.png` takes a picture after it. In the page,
`globalThis.coaxial.quality(field)` reads the rung (0), the mode (1), the
puffs drawn (2), the chunks (3), the bodies awake (4), the splinters (5), the
dust (6), the chunk budget (7) and whether the 30 FPS budget is on (8).

Two limits apply to headless runs on the Mac:

- Chrome's device emulation draws one device pixel per CSS pixel whatever
  `--dpr` says, so the pixel knob above 1.0 shows no change there. Use a
  bigger `--size` at `--dpr 1` to draw a phone's pixel count.
- The GPU is shared with other work on the machine, so GPU timings vary by
  more than the differences being measured. The counts above are exact; the
  frame times are not.

## Next

- Use the WebGPU adapter's limits as a start hint (they arrive after the
  facts today).
- `demo_stand` scans all 300 bodies for rubble on every query; look them up
  in the chunks' hash instead.
- Shadows, grass density and the animation rate of far figures as rungs,
  once each has a measured cost.
- The other Verse worlds on the same module.
