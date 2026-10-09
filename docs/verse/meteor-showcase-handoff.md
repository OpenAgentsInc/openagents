# Meteor Showcase handoff

This note records the state of the Meteor Showcase (issue #10926) when work
on it stopped on 2026-10-07.

October 8 coordination checkpoint: the remaining work is tracked in
[#10937](https://github.com/OpenAgentsInc/openagents/issues/10937)
(GPU-instanced debris, sleeping bodies, and merged rubble) and
[#10938](https://github.com/OpenAgentsInc/openagents/issues/10938)
(relighting destruction). Both remain open and unassigned after the
shared-usage checkpoint. For #10937, retain matching impact captures and
measure High-tier live p99 below 16.7 ms. Coordinate #10938 with bake B3
(#10907): destruction must remove floating baked shadows and `R` must
restore lighting. Recheck issue state and claims before resuming either
issue. Keep GPU compute and ray-tracing tests on `coderos-4080`.

## What landed

- `81c4443216` adds the zone (`verse --meteor-showcase`), Meteor Swarm's
  `Volley` (count, arc spread, circle width, and size), the eight-meteor
  `Volley::SHOWCASE` that the zone and Everglade's dev bar cast, the arcing
  meteor and smolder effects, the director's camera (`WorldRuntime::set_shot`),
  and the capture example `meteor_showcase_capture`.
- The follow-up commit that adds this note removes the automatic casts,
  caps the showcase's debris, adds per-system frame timing, and adds the
  live frame-budget run that writes `capture.json`.

Issue #10926 is closed.

## How to run it

- Play: `verse --meteor-showcase`, with `VERSE_QUALITY=high` for the high
  tier. Nothing casts on its own. Press `1`, click the ground between the
  houses, and wait out the 2.5-second cast; `R` rebuilds the houses. The
  spawn is within the non-dev 36 m range of the lot.
- Film: `cargo run --release -p verse --example meteor_showcase_capture --
  OUT_DIR --video PATH`. It stages a caster west of the houses
  (`WorldRuntime::stage_meteor_showcase`), renders 1920 by 1080 at 30 frames
  a second, and writes `establishing.png`, `impact.png`, `aftermath.png`,
  and `capture.json` to `OUT_DIR`.
- Restore: `--restore-at SECONDS` presses `R` then and writes
  `restored.png` a second later; the aftermath still comes half a second
  before it.
- Frame budget: add `--live --no-video --seconds 14`. The run plays at 60
  frames a second, one simulation step a frame, with the light still baking,
  and the player casts at 3 seconds.
- Set `VERSE_KIT_PACK` to the cached licensed kit pack
  (`~/.openagents/verse/zones-cache/<KIT_SHA256>.vtp`) to draw the licensed
  kit in place of the committed proxies.

## Measured frame times

These are live runs at 1920 by 1080 on the high tier, on this Mac. Each
frame time is the CPU's simulation step, dynamic mesh, and encoding, plus
the wait for the GPU, taken one after another. In the app, the CPU and GPU
overlap, so the app's real frame time is lower than this sum.

| Phase | Before p50 / p99 (ms) | After p50 / p99 (ms) |
| --- | --- | --- |
| Before the cast | 9.7 / 31.1 | 8.7 / 10.4 |
| Swarm (6 s) | 126.9 / 237.8 | 35.8 / 60.0 |
| After | 44.7 / 90.6 | 25.3 / 39.6 |

The cause was the rigid bodies. During the swarm, the town's physics step
took 92 ms at p50 and 202 ms at p99 for up to 1,666 live chunks. The
showcase kept every chunk for 10 minutes and broke each kit piece into up to
27 chunks. The second cost was the dynamic mesh: about 15 ms a frame to
pose 306,000 chunk vertices on the CPU and copy them into the frame's
figure. The GPU wait stayed between 5 and 13 ms. Sprites, bloom, shadows,
and draw calls (about 900) didn't dominate. The load-time light bake runs
off the main thread and didn't show in the frames before the cast.

The fix caps the showcase at 700 chunks and breaks kit pieces as the town
does, into up to eight chunks. Physics during the swarm fell to 15.5 ms at
p50 and 43.4 ms at p99, and the dynamic mesh to 7 to 9 ms.

## What remains

- The swarm still misses 60 frames a second. Physics is the top cost at
  about 16 ms at p50 and 43 ms at p99. Next steps: merge settled rubble into
  a static mesh and drop its bodies, and lower the chunk cap during the
  swarm's first seconds.
- The dynamic mesh poses every chunk vertex on the CPU every frame. Posing
  chunks on the GPU, as instances, would remove most of its 7 to 9 ms.
- After the swarm, frames stay at about 25 ms because the 700 resting
  chunks are still simulated and posed every frame.

## Temporal anti-aliasing (#10936)

`verse_pbr::pbr::taa` adds temporal anti-aliasing to the physical path on
the high tier, which draws the depth prepass it reprojects through. Each
frame draws with a Halton (2, 3) sub-pixel jitter (eight phases); culling
and the shadow cascades keep the steady camera, so cached cascades stay
cached. After the scene, the resolve pass rebuilds the current frame at
each pixel's center from its jittered 3 by 3 neighborhood, reprojects the
pixel into the last frame's history through the prepass depth and the last
camera (Catmull-Rom history reads, so the image does not soften as the
camera moves), clamps the history in YCoCg to the neighborhood's box
tightened to 1.5 standard deviations, and blends with inverse-luminance
weights, trusting the current frame more as the pixel moves faster. A
sharpening pass, bounded by the neighborhood, writes the result back into
the scene before bloom and the output transform. `VERSE_TAA=0` turns it
off. Objects that move on their own reproject by the camera only; the clamp
and the motion weight keep fast debris from trailing (no visible smear in
the impact frames).

`meteor_showcase_capture --orbit` measures it: a slow orbit round the
standing houses (`VERSE_ORBIT_SPEED`, rad/s), and the mean absolute second
temporal difference of luminance at edge pixels. Live, High, licensed kit:

| Orbit | Edge crawl off: mean / p99 | On: mean / p99 | Frame p50 off / on (ms) |
| --- | --- | --- | --- |
| 0.03 rad/s (under a pixel a frame) | 2.21 / 32.2 | 1.12 / 12.7 | 6.76 / 6.94 |
| 0.1 rad/s | 6.23 / 112 | 3.27 / 49.9 | 6.84 / 7.85 |

The GPU wait was 4.66 against 4.67 ms where the GPU held one clock; the
0.1 rad/s pair caught the GPU at its other clock (5.8 ms either way across
runs). The side-by-side
([captures/meteor-showcase/taa-orbit-side-by-side.mp4](captures/meteor-showcase/taa-orbit-side-by-side.mp4)
and [taa-orbit-zoom.jpg](captures/meteor-showcase/taa-orbit-zoom.jpg),
committed proxies, off left) shows the foliage and frame edges steady with
it on. Left: the medium tier, which has no depth prepass to reproject
through, and per-object motion vectors for chunks.

### Silhouettes, rejection, and the medium tier (October 9)

Most of the crawl that remained was on silhouettes: the roof's edge
against the sky, window frames, and tree outlines. A pixel there
reprojected with the roof's depth one frame and the sky's the next, as
the jitter moved its center across the edge. Each pixel now reprojects
with the nearest depth of its 3 by 3 neighborhood, so an edge moves with
what stands in front. The resolve also writes a depth history (clip w,
R32F). Where the history at a pixel's last place holds another surface,
such as a chunk that moved on its own or what it uncovered, the pixel
trusts the current frame in a tight box. This stands in for per-object
motion vectors for the debris. A still pixel's clamp box reaches 2.5
standard deviations instead of 1.5, so the jitter's shifting
neighborhood does not pull its history back and forth.

`meteor_showcase_capture --orbit --live` now casts nothing, so the
houses stand for the whole orbit. Earlier runs counted the swarm after
the third second. These are 8 s orbits, High, licensed kit, edge crawl
mean / p99 (the runs repeat to the digit):

| Orbit | Off | Shipped TAA | Now |
| --- | --- | --- | --- |
| 0.03 rad/s | 2.28 / 39.6 | 1.17 / 15.4 | 0.85 / 6.8 |
| 0.1 rad/s | 6.01 / 69.3 | 3.04 / 45.1 | 2.07 / 22.9 |

Sharpness (mean gradient of the middle of the frame) is unchanged: 3.21
shipped and 3.23 now at 0.03 rad/s. With TAA off it is 3.79, aliasing
included. Turning the sharpen off would cut the crawl further (0.74 /
5.1) but softens the image (3.03), so it stays at 0.2. The GPU wait
alternates between two clocks (4.66 and 6.16 ms) whatever the setting.
At the lower clock it was 4.68 ms with TAA against 4.66 off, under
0.1 ms for the extra depth taps. Maps of the crawl, with off, shipped,
and now from left to right:
[taa-crawl-maps.jpg](captures/meteor-showcase/taa-crawl-maps.jpg).
Impact frames show no new smear on debris:
[taa-rejection-impact-zoom.jpg](captures/meteor-showcase/taa-rejection-impact-zoom.jpg)
(shipped left). Both are from the committed proxies.

The medium tier keeps 4x MSAA and gets no TAA. It is the tier for phones
and the web and has no depth prepass. Adding one would draw every caster
a second time (about 1,500 draws here) and read and write two
full-size histories each frame. Those are the costs a tile-based GPU
feels most. On this Mac, medium's crawl matches High's with TAA off
(2.34 / 39.5 at 0.03 rad/s). Revisit this with a phone measurement if
crawl on medium becomes the complaint.

## Instanced chunks, merged rubble, and cheaper contacts (#10937)

- Broken chunks draw as GPU instances (`Town::set_instanced`, on in the
  showcase; `demolition::instanced::Herd`). Each chunk part is uploaded
  once in its own space; a frame writes only each live chunk's transform
  and its vertices' light, blended once at the chunk's center
  (`AmbientProbes::at`) and evaluated once per distinct normal. The pool
  still poses pieces that are damaged or loose but whole.
- Rubble at rest merges into one world-space mesh a material
  (`Town::settle_rubble`, `Herd::merged`) once 48 more chunks rest or any
  have rested 1.5 s, and is merged again when a merged chunk moves or the
  probes change. The renderer skips writing a set whose records and light
  are unchanged. Frozen chunks keep their static bodies, so later debris
  still lands on the pile and a blast still throws it; they no longer cost
  broadphase queries (below), and static bodies never cost solver time.
- Physics: only colliders that respond query the broadphase (pairs are
  visited in the exhaustive order, so contacts are unchanged), the solver
  finds each contact's warm start among its own pair's contacts instead of
  scanning all of them (quadratic with 3,700 contacts), and the yard sums
  impacts per pair through a map. Halving the solver's iterations was
  tried and dropped: a standing cottage's roof took damage from its own
  resting contacts.
- Draws rise during the swarm (about 4,300 at most, against 1,500) because
  most chunk shapes are unique; they fall again as the rubble merges.

Measured with `--live --settle-light`, three interleaved runs each, medians
of each run's p50 / p99 frame (sequential CPU and GPU, as above), on this
Mac with other builds running (load average 11 to 16):

| Phase | Before p50 / p99 (ms) | After p50 / p99 (ms) |
| --- | --- | --- |
| Before the cast | 6.5 / 13.2 | 6.6 / 12.8 |
| Swarm (6 s) | 19.2 / 34.6 | 14.6 / 21.9 |
| After | 14.8 / 23.8 | 10.2 / 18.4 |

The swarm's physics fell from 6.8 / 18.5 to 5.3 / 9.5 ms and the dynamic
mesh from 3.6 / 5.7 to 1.3 / 1.8 ms. The film's stills match the posed
path's to 59 to 63 dB PSNR
([captures/meteor-showcase/instanced-impact-pair.jpg](captures/meteor-showcase/instanced-impact-pair.jpg),
committed proxies, before left). The 16.7 ms p99 is not met yet: the GPU
wait alone reaches 12 ms at p99, and the impact frames' physics 6 to 10 ms.
Lowering the chunk cap early in the swarm would trade away visible debris
(the live cast peaks at about 500 chunks, under the 700 cap), so it is not
done.

### Physics over threads, shared chunk shapes, and app-like timing

The second pass at #10937 (October 9):

- Rigid-body detection splits over threads (`physics::parallel`): the
  broadphase queries, then the narrow phase, each pair's result back in
  order, so the contacts are the same as one thread finds.
- The contact solve splits by island: islands share no body that moves,
  so each thread sweeps whole islands and the result is the one-thread
  result bit for bit. A swarm's rubble is mostly one island of 1,000 to
  2,500 contacts, so an island of 1,024 or more contacts without joints
  solves by four slabs of its bodies along its longer side, the slabs at
  once and the contacts between slabs after them, each pass
  (`physics::contact::regions`). That order is fixed by the island, never
  by the machine's threads. Smaller islands keep the old order: a 384
  contact threshold changed which pieces a test strike broke.
- Carved blocks cut alike share one chunk shape
  (`carve::CellBody::key`): the body-frame triangles snap to a tenth of a
  millimeter, so every wall section of one model breaks into the same
  chunks and draws as instances of one mesh. Draws in the swarm fell from
  about 5,500 to 3,200.
- `meteor_showcase_capture` no longer reads every live frame's pixels back
  (only a still's, from a second draw), and `--pipelined` times frames as
  the app draws them: each frame's CPU work overlaps the last frame's GPU
  work (a frame latency of two). The sequential frame still adds the full
  GPU round trip to every frame.

Three interleaved runs each, `--live --settle-light`, High, licensed kit,
this Mac with other agents' builds running (load average 8 to 14),
medians of each run's p50 / p99 frame:

| Phase | Sequential before | Sequential after | Pipelined before | Pipelined after |
| --- | --- | --- | --- | --- |
| Before the cast | 6.6 / 9.7 | 6.7 / 9.7 | 2.6 / 4.5 | 2.6 / 4.5 |
| Swarm (6 s) | 17.9 / 26.1 | 14.4 / 21.8 | 11.8 / 18.8 | 8.9 / 16.1 |
| After | 10.8 / 17.2 | 10.9 / 14.1 | 4.8 / 8.7 | 5.2 / 7.4 |

The swarm's physics fell from 7.3 / 12.6 to 4.8 / 9.7 ms a frame (two
120 Hz steps): detection 3.5 / 4.8 to 1.8 / 2.3, the solve 3.4 / 7.8 to
2.6 / 6.4. As the app draws, the swarm holds p99 under 16.7 ms (runs of
15.3, 16.6, and 16.1 ms), with little room; the sequential sum does not.
The debris looks the same: the same chunk counts over time, both houses
down, the rubble spread alike
([parallel-physics-impact-pair.jpg](captures/meteor-showcase/parallel-physics-impact-pair.jpg)
and
[parallel-physics-aftermath-pair.jpg](captures/meteor-showcase/parallel-physics-aftermath-pair.jpg),
committed proxies, film frame 163 and the aftermath, before left); the
large islands' new solve order moves individual chunks, so the pairs
differ chunk by chunk (25.8 and 33.0 dB).

## Relighting what breaks (#10938)

The showcase's light is baked at load: each vertex's sky visibility and one
bounce of the low Sun, and a probe grid that lights the chunks. Before
#10938, broken walls left that light behind: a standing wall kept the shade
of the house that had stood beside it, and rubble took the probes' indoor
darkness. Now the zone calls `Everglade::relight_destruction` before its
bake, and `verse_pbr::pbr::relight` follows the scene's index edits: when
pieces are hidden, a worker thread traces every vertex and probe within 6 m
of a hidden triangle again with the hidden triangles passed through, and
delivers the light channel and probes. When `R` restores the houses, nothing
is hidden and the baked light returns exactly. The direct Sun's cascades
were already redrawn on destruction edits.

Measured on this Mac with the licensed kit (`--seconds 14 --restore-at 12`,
aftermath at 11.5 s as in the default film): the aftermath relight hid
18,460 triangles and recomputed 5,417 vertices and 66 probes in 124 ms on
its worker, off the frame. Frame times did not change. With
`VERSE_PHOTO_DEBUG=2` (diffuse ambient only), the aftermath shows the
floating shade gone from the standing wall
([captures/meteor-showcase/relight-aftermath-ambient.jpg](captures/meteor-showcase/relight-aftermath-ambient.jpg),
committed proxies, before above and after below). `--restore-at` writes
`restored.png` with the baked light back. The Everglade town relights the same way
since B3 (#10907), over its offline-baked layers.

## Known issues

- When the cast ends, the fire gathering over the caster's hands disappears
  in one frame, a small visible pop.
- With the 700-chunk cap, the oldest chunks disappear when the cap is
  reached, inside the fireballs.
- The film on the owner's Desktop was rendered before the debris cap, with
  finer chunks, so the live zone's rubble is coarser than in the film.
- A blast high on a wall draws its shockwave ring flat in midair, which
  reads as a streak.
- No zone loads the products of `verse-bake` yet, so the showcase uses the
  load-time CPU bake.

## Capture paths

- Film: `~/Desktop/meteor-swarm-v2.mp4` (12.5 s, 1920 by 1080, 30 frames a
  second, `libx264`, `yuv420p`, CRF 16).
- Stills: `~/Desktop/meteor-swarm-v2-establishing.png`,
  `~/Desktop/meteor-swarm-v2-impact.png`, and
  `~/Desktop/meteor-swarm-v2-aftermath.png`.
- Frame budgets:
  `~/.openagents/scratch/claude-code-d187dc74-0ea6-4000-9d57-3fe96d6cca18/live-before/capture.json`
  and `.../live-after/capture.json`.
