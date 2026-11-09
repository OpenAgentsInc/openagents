# 10. Hold 60 frames per second

> **Status: partly implemented.** Drawing, fading and culling are in
> [`apps/openagents-web/everglade2.coax`](../../apps/openagents-web/everglade2.coax),
> [`apps/openagents-web/everglade2-cull.coax`](../../apps/openagents-web/everglade2-cull.coax)
> and [`apps/openagents-web/gpu/everglade2-cull.coax`](../../apps/openagents-web/gpu/everglade2-cull.coax).
> The graphics governor is Aethyr's [`aethyr/quality.coax`](../../aethyr/quality.coax).
> This chapter names functions and has no source listings yet.

This chapter answers the player's question: "Why is it smooth?"

Everglade II plants about 100,000 pieces and covers the land in grass. A frame
can't draw all of that at 60 frames per second, so the page draws only what
shows, as few times as it can, and lowers its resolution when frames run slow.
It does this without any piece moving, growing or popping in front of you.

## What this chapter teaches

- How to draw a forest in about 75 instanced draws.
- How to fade pieces with distance instead of swapping them.
- How the GPU leaves out pieces that nothing shows, without popping.
- How a governor spends the pixel budget.

## What exists today

- **Runs.** Each frame, the page picks the pieces in view, sorts them into one
  run for each kit piece and level of detail, and draws each run as one
  instanced draw. About 75 draws make a frame.
- **Levels of detail.** A tree draws whole to 32 m (`E2_LOD1`), two of its three
  crown cards to 52 m (`E2_LOD2`) and one card past that. Each change
  cross-fades over 8 m (`E2_LOD_BAND`): the nearer level drops a share of its
  pixels, and the farther level draws exactly those pixels.
- **Thinning.** Past 55% of a kit's reach (`E2_FAR_START`), the kit thins out.
  Each piece dithers out when the share left passes its own rank, a hash of
  where it stands (`e2_piece_shown`, `e2_piece_looks`). The edge is ragged,
  never a line.
- **Grass.** The GPU places one tuft per cell of a grid fixed to the land, in
  four nested tiers: 0.5 m cells to 28 m, 1 m to 40 m, 2 m to 52 m and 4 m to
  140 m ([`verse/everglade2/field.coax`](../../verse/everglade2/field.coax)).
  Each coarse tuft stands exactly where one of the four finer tufts under it
  stood.
- **GPU culling (#13050).** `apps/openagents-web/gpu/everglade2-cull.coax` tests each picked
  piece's box against the view and a depth pyramid in two phases
  (`coax/web/instance-cull.js`). It needs the adapter's
  `indirect-first-instance` feature. Without it, or with `?cull=0`, the page
  draws every piece it picked.
- **The governor.** `e2_quality_frame` feeds each frame's time to the
  governor, and `e2_pixel_budget` tells the host how many pixels to draw. It
  steps down a ladder of twelve rungs when frames run slow and back up when
  there's room. The start comes from the device's class: a weak phone at
  LOW, a phone and a tablet at MED, a laptop with a built-in GPU at MED's
  top, a computer at HIGH. When the scene first draws, the governor times
  1.5 s of frames and, if they run slow, steps down several rungs at once.
  The rung it settles at is kept for your next visit.
- **The knobs.** Each rung sets how much the frame draws: the pixels, the
  grass's reach, the scenery's reach, the shadow tiles, the post pass and the
  other players drawn (`quality_rung_knobs`, `e2_knobs_frame`).
- **The GFX control.** Left of the counter, `GFX AUTO` (or the tier you
  picked) shows the graphics. Tap it to pick the next: HIGH, MED, LOW, AUTO.
- **The counter.** A frame-rate counter in the bottom-right corner shows frames
  a second, averaged over 0.5 s.

## The method

### Fade, never swap

A piece stands in one place at one size at every distance. With distance, it
only fades out in a screen dither. A tree's levels keep the same crown cards in
the same places, so a tree keeps its shape as it thins.

### Cull without popping

The cull works in two phases:

1. Test each piece against last frame's depth pyramid, and draw what passes.
2. Build the pyramid from the depth just drawn. Retest what phase 1 hid, and
   draw what is no longer hidden.

A piece is left out only when depth that this frame really drew is nearer than
all of its box. So culling never makes a piece pop. The image with the cull on
is the image with it off, pixel for pixel.

### Spend the cheapest loss first

A rung down never costs more than the one above, and each changes at least one
thing drawn. Resolution goes first, from 3 pixels a CSS pixel to 1.5. Then the
post pass's extras, the far shadow tiles, the grass's reach, the other players
and the scenery's reach, and last the resolution again, down to 0.75.

Reaches never jump. When a rung changes the grass's or the scenery's reach,
the frame eases toward it at 0.34 a second, and each tuft and piece fades
through its own dither as the edge passes it. The same is true when you walk
out of the studio, which trims the grass, scenery, shadows and post below HIGH
while you make your character: the run-out takes about 3 s, and the world fills
back in over it.

At HIGH every knob is 1, and each shader runs its original code. The desktop
picture is the same, pixel for pixel.

## A worked example: how many trees you see at 128 m

Trees reach 165 m (`E2_KIT_FAR`), and thinning starts at 55% of that:

```text
start = 0.55 × 165 = 90.75 m
span  = 0.45 × 165 = 74.25 m
t     = (128 − 90.75) / 74.25 = 0.502
share = 1 − t² (3 − 2t) = 1 − 0.252 × 1.996 ≈ 0.50
```

At 128 m, about half the trees still draw. Which half depends on each tree's
rank, so the same trees stay as you walk, and a tree's own fade takes 10% of
the share (`E2_FAR_BAND`) to go from whole to gone.

The cull's effect, measured by
[`coax/web/everglade2-cull-check.coax`](../../coax/web/everglade2-cull-check.coax)
at 2400 by 1400 on an Apple Metal 3 Mac (2026-11-08):

| View | Pieces picked | Hidden by the cull | GPU off | GPU on |
| --- | ---: | ---: | ---: | ---: |
| The glade | 4229 | 2602 | 8.6 ms | 7.6 ms |
| The woods (`?at=200`) | 4555 | 1953 | 11.0 ms | 9.2 ms |
| The clearing (`?at=470`) | 4160 | 888 | 8.5 ms | 8.2 ms |

The cull helps most in the woods, where trunks hide most of what is behind
them. The receipt is
[`docs/verification/gpu-cull-everglade2-13050/`](../../docs/verification/gpu-cull-everglade2-13050/).

## The checks

In `apps/openagents-web/everglade2.coax`:

- `#test "a piece keeps its place and size at every distance and only fades"`.
- `#test "the woods' pieces never move, grow or swap as you walk, and come back
  the same"`.
- `#test "the catalog's kit rows fill their (kit, level) buckets"`.
- `#test "the frame rate is the frames over the last half second"`.
- `#test "e2 quality: each rung down draws no more: the grass's and scenery's
  reach, the shadow tiles, the post pass and the crowd"`.
- `#test "e2 quality: the studio trims what stands behind its panes below
  HIGH, and the run-out eases it back with nothing popping"`.
- `#test "e2 quality: the shadow knob: no tiles, the nearest only, or the full
  set"`.
- `#test "e2 quality: the GFX control beside the frame rate, a finger's size; a
  tap picks the next and the host keeps it"`.

In `verse/everglade2/field.coax`:

- `#test "a tier's tuft is one of the nearer tier's, in the same spot"`.
- `#test "the grass thins smoothly past the near ground, and tiers hand over
  with nothing changing"`.
- `#test "the grass's reach: every distance shrinks with it, and the finest
  tier hands over to the next with nothing moving"`.

In `apps/openagents-web/everglade2-cull.coax`:

- `#test "the cull's matrix projects every point as the kit stage's
  fivelab_project does"`.
- `#test "the cull's runs are the frame's kit draws, each with its kit piece, and
  nothing else"`.

In `aethyr/quality.coax`:

- `#test "quality: a phone starts no higher than MED, a weak one LOW, a computer
  HIGH"`.
- `#test "quality: each rung down costs no more than the one above it"`.
- `#test "quality: the page's facts give the device; a phone held sideways or a
  tablet never starts at HIGH"`.
- `#test "quality: the device's class: a weak phone, a phone, a tablet, a
  laptop's built-in GPU, a computer; each starts at its rung"`.
- `#test "quality: the measured start steps down as many rungs as the frames
  need, at once, and never up"`.
- `#test "quality: the rung AUTO settles at is kept for the next visit, for its
  class and its ladder only"`.

[Aethyr chapter 16](../../aethyr/book/ch016-budgets-that-refuse.md) builds the
budgets that refuse work a frame can't pay for.

## Mistakes

- **The glade's grass.** Character Create's glade grass was about 35 million
  vertices a frame and cost about 10 ms of GPU time. In a studio that is fine.
  In a world that also draws a forest, it isn't. Everglade II drops it and
  covers the glade with blade grass. Headless GPU time at the glade fell from
  31 ms to about 16 ms.
- **Grass that changed as you walked.** An earlier grass (`59acb8e8ec`) shrank
  the tufts that a coarser tier dropped and widened the one it kept. Tufts
  changed size in front of you. Now a coarse tuft is one of the finer tufts,
  with the same turn, height and width, and the others only dither out. The
  field tests assert that no tuft moves, widens or is replaced, and the
  page's walk test asserts the same for every kit piece.

## What comes next

[Chapter 11](ch011-one-address.md) gives the world one address.
