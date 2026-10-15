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

## The CAD Lab rover in the swarm (#11644)

`verse::zones::meteor_rover` stands the rover built in CAD Lab (#11627) on
the lawn in front of the houses, from its v38 glTF and assembly
(`assets/verse/cad/rover.{glb,json}`):

- **Bodies:** every CAD solid is a rigid body, 51 in all, with its mass,
  centre and full inertia tensor (diagonalised; products kept in the
  principal frame) from its triangles and its material's density. They
  sum to the CAD mass, 87.94 kg.
- **Joints, 72:** the 12 CAD joints are pins (rocker and bogie pivots held
  level by soft servos, six axles, the camera pan, the lid hinge); every
  pair of touching solids in one rigid group is a bolted seam.
- **Pins** hold 0.15 × 0.6 σᵧ × πr² in shear (r from the CAD part: 36 mm
  shafts, 60 mm hubs, 40 mm mast, 12 mm lid pin) and that force × 4 cm in
  bending.
- **Bolted seams** take their bolts from the seam's length: under 30 mm
  2 × M3, to 80 mm 2 × M4, to 200 mm 4 × M4, to 350 mm 6 × M5, longer
  8 × M6. The bolts are class 8.8, 800 MPa over the tensile stress area
  (M3 5.03, M4 8.78, M5 14.2, M6 20.1 mm²), with shear at 0.6 of tension.
  A pattern of N bolts over a seam of length s holds N × Fv in shear, N × Ft
  in tension, N × Ft × s/4 in bending and N × Fv × s/2 in twist. Loads
  combine by (V/Vc + Mt/Mtc)² + (T/Tc + Mb/Mbc)² ≤ 1.
- **Design floor:** every joint carries at least 4 × its standing load
  (measured in a settling run).
- **Breaking:** the solver doesn't cap the joints, so the recorded loads are
  real. A joint breaks in the step its load reaches its capacity, whether
  from a blow, a landing, or a piece tumbling.
- **Loads:** real blast and impact physics; see "The meteor's blast" below.
- **Collisions:** pieces collide with the ground, the houses' debris near
  them (mirrored in as moving boxes; one-way), and each other once
  separate. Each piece held together by its joints is a collision group of
  its own. Parts that broke apart while overlapping, or that come to rest
  inside another piece after flying through it, collide only once clear.
  Parts faster than 40 m/s meet only the ground (a 100 m-deep slab, so
  nothing crosses it in a step).
- **Air drag:** every part flying through the air is slowed by
  ½ ρ C_d A v² (ρ 1.225 kg/m³, C_d 1.2, A its box's cross-section along its
  flight), which is what stops light fragments within tens of metres.
- **Effects:** sparks at a breaking joint and dust at a hard landing fire
  once each (`rover_sparks`, `rover_landing`). The old dust puffs were
  continuous emitters that piled up into a cloud over the rover.

`WorldRuntime::add_meteor_rover` and `stage_meteor_rover` set it up: the
third meteor lands on the rover and the sixth beside it. The rover is
native only (its glTF is built into the binary).

Film: `cargo run --release -p verse --example meteor_rover_capture --
OUT_DIR --video PATH` with `VERSE_QUALITY=high`. The scene is
deterministic, so each part plays it again from the start:

1. the swarm coming down, up to the strike;
2. the 0.35 s round the strike at 1/20 speed, low and 2.6 m from the
   rover, with the scene's fire, smoke and glow hidden so each piece can
   be seen leaving (on-screen note);
3. the dish and bay, the left wing, and the mast each followed in flight
   at half speed;
4. an overhead shot of where the pieces came to rest, labelled.

`capture.json` lists the meteor's numbers, what it struck, every joint's
capacity, peak load and when it broke, and each solid's impulse and how
far it ended up.

### The meteor's blast (2026-10-14)

The first version pushed the rover with a capped 25 m/s "gas" from an
18 kg rock at 32 m/s (9 kJ), while the same meteor flattens brick houses.
The rover now takes the load that meteor really carries.

**How big the meteor is.** The scene flattens the brick houses out to the
showcase's damage radius, 5.2 m (`BLAST` 4 m × size 1.3). Unreinforced
brick fails at 35–70 kPa side-on overpressure (Glasstone and Dolan, table
5.139). Taking 50 kPa at 5.2 m, the Kinney–Graham curve gives a scaled
distance Z = 3.79 m/kg^⅓, so the free-air TNT charge is
W = (5.2 / 3.79)³ = 2.58 kg (`blast::charge_for`). A burst on the ground is
reflected by it, so it equals a free-air charge 1.8 times its energy
(UFC 3-340-02): the meteor releases E = 2.58 × 4.184 MJ / 1.8 = 6.0 MJ. A
meteor that keeps that much energy to the ground is hypervelocity, not
falling at terminal speed. At 3 km/s (`IMPACT_SPEED`) it weighs
m = 2E/v² = 1.33 kg, an iron ball about 7 cm across.

**The blast load** (`zones::meteor_rover::blast`). For each solid and each
burst, at the solid's distance R, Z = R / W^⅓ gives (Kinney–Graham):

- the side-on peak Ps;
- the positive phase's duration td;
- its impulse Is;
- the shock's arrival time (its speed integrated outward);
- the reflected peak Pr = 2Ps(7P₀ + 4Ps)/(7P₀ + Ps);
- the dynamic pressure q = 2.5Ps²/(7P₀ + Ps).

The pulse is Friedlander, p(t) = P(1 − t/td)e^(−bt/td), with b fitted so
that it carries Is. The net pressure on a solid is the larger of two
terms. The first is the reflected-minus-side-on load on its face, which
fades as the wave clears round it in 3S/U (S the face's smaller half
side, U the shock's speed). The second is the wind's drag, C_d q(t) with
C_d 1.2. That pressure times the area the solid shows the burst (its box's
cross-section) is integrated through each physics step as a real impulse.
Nothing caps the resulting speed. A solid with another solid between it
and the burst (a ray test against every other box) gets half, because the
wave diffracts round small obstacles. The solid a burst goes off inside
(what the meteor struck, blown apart) shadows nothing.

**The direct strike.** A meteor whose flight meets the rover hits the
first solid on it in a perfectly inelastic collision. The solid takes the
impactor's momentum in proportion m/(m + mᵢ), and absorbs
½mᵢv² × m/(m + mᵢ) of its energy. Above 10 kJ/kg (`DISRUPTION`, the order
of the catastrophic-disruption threshold for metals struck at km/s) the
solid breaks into four fragments, split by two planes across its two
longest sides. The fragments share its mass, momentum and triangles, and
fly apart with 3 % of that energy (`SPREAD`). The energy the solid took
goes off as a burst where it hit; the rest goes off on the ground.

**Hand calculations.** These cover the solar wing, the dish (`antenna`)
and the avionics bay, from the rover at rest in the test pose
(`the_blast_loads_match_the_hand_calculations`). The meteor strikes the
9.65 kg deck, which takes 88 % of the energy: a 2.27 kg-TNT burst on the
deck and a 0.31 kg burst on the ground. The hand numbers use a triangular
pulse of the same peak and impulse:

| solid | burst | R (m) | Z | Ps (MPa) | Pr (MPa) | area × shade (m²) | hand (N s) | sim (N s) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| solar wing, 1.45 kg | deck | 0.34 | 0.26 | 11.4 | 87 | 0.028 | 11.3 | 11.3 |
| | ground | 0.69 | 1.01 | 0.99 | 5.4 | 0.069 × ½ | 10.5 | 10.1 |
| dish, 0.74 kg | deck | 0.19 | 0.20* | 16.1 | 124 | 0.081 | 124.7 | 113.0 |
| | ground | 0.72 | 1.06 | 0.90 | 4.8 | 0.049 × ½ | 6.6 | 6.0 |
| bay, 3.57 kg | deck | 0.24 | 0.20* | 16.1 | 124 | 0.059 | 79.1 | 68.2 |
| | ground | 0.57 | 0.83 | 1.49 | 9.0 | 0.103 × ½ | 19.3 | 18.6 |

\* Inside the fireball; the curves are held at Z = 0.2.

The test asserts agreement within 20 %; the largest gap is the bay's, at
14 %. The two bursts push from different points, so a solid's total is
their vector sum, not the sum of the table's numbers.

**The pulse, resolved.** The positive phase lasts 0.03–3 ms, far less
than the 1/240 s physics step. While a pulse is crossing the rover, each
step is therefore split into 200 substeps of 21 µs (`FINE`). Each
joint's load is averaged over 10 substeps (0.21 ms, `WINDOW`), about the
response time of the rover's parts (a 0.3 m aluminium panel's first
bending mode is near 1 kHz). Position correction is off during the
substeps: correcting a joint's drift within 21 µs would read as a load
hundreds of times its real one. The velocity constraints still hold every
joint and contact. Two checks guard this: a 1 kPa blast breaks nothing,
and the impulses still match the hand calculations.

**Do the wings tear off by blast alone?** With the deck not struck, the
scene's charge goes off on the ground at a distance from the deck. The
wing mount's load is the wing's inertia as the deck is thrown under it.
By hand, the bending at the mount is
M = m_wing × a_deck × r, where:

- m_wing is 1.45 kg;
- r is 0.15 m, the wing's centre from its mount;
- a_deck is the rover's total blast impulse ÷ its 87.9 kg ÷ the positive
  phase td.

The two 2 × M3 mounts hold 36 N m in bending each:

| burst from the deck | Z | td (ms) | rover impulse (N s) | a_deck (m/s²) | M by hand (N m, both mounts) | simulation |
| --- | --- | --- | --- | --- | --- | --- |
| 1.0 m | 0.73 | 0.30 | 372 | 13 900 | 3 000 | both wings off; mounts at 2.4–3.7 × |
| 2.4 m | 1.75 | 1.41 | 91 | 730 | 160 | mounts at 0.9–1.5 ×; a wing hangs on by one mount or its seam to the dish |
| 4.0 m | 2.92 | 2.25 | 28 | 140 | 31 | both hold, mounts at 0.07–0.2 × |

The hand figure is an upper bound. It treats the deck as rigid and loaded
evenly through td, and it ignores the wing's own push in the same
direction. The simulation is 2–5 times lower, but agrees on the outcome:
the blast alone tears the wings off when the meteor lands within about
1 m, it is the borderline case at 2.4 m, and they hold at 4 m
(`the_blast_alone_tears_the_wings_off_close_by_and_not_far`).

**The scene on 2026-10-14** (`capture.json`). The third meteor strikes the
deck at 4.95 s and shatters it into four fragments, which leave at about
360 m/s. Every seam to the deck goes with it: both wings, the dish and
the bay, the mast, the cassette, the hazard cameras, the rails, and the
rocker shafts.

With the pulse resolved, the blast's peaks then break joints across the
rover:

- the dish's 4 × M4 seam to the bay: 247 kN peak, 22 × its capacity;
- the lid hinge, at 39 ×;
- the camera pan, at 2.3 ×;
- every wheel axle;
- both bogie and rocker pivots;
- the batteries;
- the right wing's inner seams, at 1.8–1.9 × when it lands at 5.48 s.

In all, 35 of the 72 joints broke. The dish, the bay and the right wing
land 16–21 m away and the mast 12 m away. The wheels scatter 3–17 m. The
deck's fragments go out to 87 m.

Kept from before: the bolts, the pins, the 4× design floor, and joints that
break when their interaction ratio reaches 1. Parts that have once flown
faster than 40 m/s meet only the ground from then on: re-enabling their
collisions as they slow, wherever they have flown through other parts,
threw them. Each solid's inertia is floored at m × 1 cm², so a thin rail
spinning on a weld stays stable. Tests cover the blast
curves against Kinney–Graham, the hand calculations, momentum bookkeeping
(the rover's momentum equals the sum of every blast, strike and drag
impulse) and energy (the parts carry off less than half the
meteor's), a 1 kPa blast that breaks nothing, the blast alone tearing the wings off
at 1 m but not at 4 m, a direct hit that shatters what it strikes, and
determinism.

## Placed CAD bodies (#11680)

`everglade::cad_physics` now owns the CAD solid parser, full inertia
integration, body builder, bolt capacities, and Kinney–Graham/Friedlander
blast curves. `meteor_rover` re-exports the blast API and uses that builder;
its staged meteor flight and direct-hit fragmentation remain there.
Ordinary `cad_place` copies register in the town's existing `Site` world,
with the same standing-load margin (four times the measured load), fine
21 µs blast steps, and a ten-step load window. Connected solids exclude
internal contacts; separated pieces collide once their overlapping bounds
clear. Other copies, houses, debris, terrain, and force spells share contacts.

The native proof is `cad_physics_capture OUT_DIR`, built with
`--no-default-features --features capture,dev-destruction`. It places two
copies of the committed rover at `[-2.5,0,-28]` and `[2.5,0,-28]`, walks into
one, casts and ends Reverse Gravity, then casts a Mega Thunderbolt. It writes
an MP4, second-by-second PNGs, and per-frame body measurements. `--measure`
runs the same actions without a renderer. Saved placement ids, versions,
replacement, and removal work on native and browser clients; live physics
is shared between signed-in browser views through the account host (#11682);
standalone native file placements remain local.

The [two-browser check](captures/cad-sync/two-browsers.json) for #11682 uses
separate browser storage and a synthetic account service. A held keyboard
push moves the chassis 0.128 m; both clients receive Reverse Gravity and
blast debris. Reload restores the broken joints, replacement and removal
reach both views, and an old binding receives HTTP `409`. Server tests also
check account isolation and recovery of a blast accepted before a restart.
The [browser view](captures/cad-sync/rover.png) shows the placed rover beside
the character; its [geometry report](captures/cad-sync/rover.json) records
203,802 drawn vertices and their world bounds.

The [12-second native capture](captures/cad-physics/cad-physics.mp4) was
checked across all 144 frames. Each copy has 51 bodies and weighs 87.938 kg.
The push moves the first chassis 0.628 m; Reverse Gravity lifts it to 20.710 m.
Landing breaks six joints, and the blast raises that count to 57. Every body
position stays finite. The [measurements](captures/cad-physics/summary.json)
include the source GLB hash and one sample per second. Stills show the
[push](captures/cad-physics/push.png), [lift](captures/cad-physics/lift.png),
[blast](captures/cad-physics/blast.png), and
[loose parts](captures/cad-physics/aftermath.png).
