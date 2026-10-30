# Verse water

Water is being ported from the previous Rust engine to Coax in the phases
of its specification, [verse/everglade/previous/water.md](../../verse/everglade/previous/water.md)
(copied from `0b401561a0^`; see its phased plan). Done: W1 (the physics),
W2 (the shader, on the Coax WebGPU renderer), W3 adapted to today's
Everglade (its river, swimming and floats) and, from W6, the ripple field
with your wake and splash, most of W5 (below) and W4 (the spectral sea,
in the water lab, below). The rest has a task each under `verse-water`
(#c278): W3's rest (townsfolk swimming; the ponds, the weir and the
rowboats are in), the rest of W6, and W7 to W11.

## W1: the physics (done)

[`verse/water.coax`](../../verse/water.coax) is a port of the previous
engine's `physics::water` at its W1 commit (`git show
b399fa2dbe:crates/physics/src/water/`), with the same formulas, constants
and tests. It covers what the specification's "Coupling with
`crates/physics`" section and its W1 status list:

- **Bodies.** `WaterBody`: pond, river, waterfall, ocean, pool, puddle or
  marsh; an outline (polygon, river `Course` with widths, or everywhere);
  a level (constant, or a profile along the course); a density (`FRESH`,
  `SALT`); up to eight Gerstner terms; an optional current.
- **Surface with deterministic time.** `WaveSet`: Gerstner waves whose
  frequencies snap to whole multiples of 2π/T, so each phase is an integer
  counter. `water_body_sample` at tick N equals stepping
  (`water_advance`) from tick 0 bit for bit, and the clock folds on the
  period. `water_seeded` draws terms from a SplitMix64 seed.
- **Point queries.** `WaterSet` grids a zone's bodies so `water_sample` and
  `water_bodies_overlapping` test only the bodies whose bounds cover a cell.
- **Flow grid.** `FlowGrid` (still, uniform, or `water_flow_river`:
  divergence-free potential flow round obstacles from the stream function
  by successive over-relaxation).
- **Buoyancy, drag, currents.** `water_submerged` (exact spherical caps,
  sliced capsules, clipped cuboids fitted to rough water) and
  `water_apply`: buoyancy at the centre of buoyancy, linear and quadratic
  drag relative to the water, angular damping. Bodies on still water count
  as supported, so they sleep; waves and currents keep them awake.
- **Ledger terms.** Each force is a `Push` under `water.buoyancy` or
  `water.drag`; `water_record` enters them into a momentum `Ledger`.

[`verse/rigid.coax`](../../verse/rigid.coax) holds the rigid bodies water
pushes on: the old `body.rs`, `World::step` without contacts, sleep with
support, and `ledger.rs`.

All fifteen of the old tests are ported as `#test` checks with the old
scenes, durations and tolerances (rigid.coax adds four of its own). Run
them with:

```sh
coax/bin/coax test --library verse/water.coax --steps 500000000
coax/bin/coax test --library verse/rigid.coax
```

### Differences from the Rust W1

- Coax has no growable arrays, traits, maps or serde yet. Polygons,
  courses, profiles, flow grids, a set's grid cells and pushes live in the
  caller's arrays and records hold views of them. `Water` (the trait) is a
  `WaterSet`; a single body is a set of one.
- A `WaterSet` whose storage is too small for the requested cell doubles
  the cell until it fits. Samples are the same at any cell size.
- Ledger terms are an enum (`Term`), not strings in a map.
- `WaveSet` refuses a period of 2^32 ticks or more, so the phase products
  are exact in `u64` (the Rust used `u128`).
- The rigid bodies have one collider each at their centre, all dynamic. No
  contacts, joints or kinematic bodies: W1's scenes need none.
- The old restore test round-tripped the world and water through JSON.
  Here it copies them and checks that the copy continues bit for bit.
- Not ported: the `water_buoyancy` timing example and the Water Lab's
  floats, which belonged to the old app.
- Coax has no `acos` or `atan` intrinsic, so rigid.coax defines `acos_d`
  and `atan_d` (checked against `cos` and `tan`).

## W2: the look (done)

The old W2 (`git show 03b7bbe2b0`: `assets/shaders/water.wgsl` v1 and
`crates/verse-world/src/water/{terms,presets,frame,shore}.rs`) on the
Coax WebGPU renderer (`coax/web/host.js`):

- [`verse/water-look.coax`](../../verse/water-look.coax) is the CPU half:
  the seven presets (Jerlov absorption, in-scatter, foam, roughness, shore
  band, crest scattering), each wave's term and the angle constant from
  W1's integer phase counters in f64 so the GPU's f32 never sees a large
  phase, the f32 mirror of the surface the shader draws (checked against
  `water_body_sample` within a millimetre), shoaling, the wind sea, the 16
  analytic detail waves, the tiers (phone: 6 detail waves, glints fade at
  120 m, one foam octave; desktop: 16, 400 m, two), the exact Euclidean
  shore distance the shore foam reads, and the uniform block
  (`WATER_FLOATS` numbers, `W_*` rows).
- `apps/openagents-web/everglade.wgsl` (the water section) is the GPU
  half: the Gerstner mesh over baked nodes, Fresnel, the sky reflection
  blurred by roughness and pixel footprint, Beer-Lambert absorption over
  the baked depth, in-scatter, GGX glints, crest scattering, the shore,
  crest and white-water foam, flow-advected detail, ripples, and the view
  from under the surface (Snell's window). A host's sky, sun and shadows
  come in through four small hooks, as the old `water_host_*` functions.
- The renderer draws water last, in two halves over the scene with its
  depth: a multiplied transmittance, then the added light (`host.js`
  `waterPipelines`; a page names how many of its last draws are water with
  `coaxial_water_draws`). The page's target is gamma-encoded, so the
  transmittance half multiplies by the encoded transmittance (exact for a
  product) and the light half adds the difference it makes over a mid bed.
- Deterministic time: the water's clock is W1's tick (120 a second), and
  the shader's waves are the body's `WaveSet`, so what you see is what the
  physics and the swimmer read.

Not ported, and why: the Low tier's baked normal tile (the phone draws 6
analytic detail waves instead; no texture pipeline is needed), the
`capture` example and its golden PNGs (the realm's golden screenshot
covers it), the GLES path (the Coax renderer is WebGPU only), and
screen-space refraction (the bed comes through by absorption only).

```sh
coax/bin/coax test --library verse/water-look.coax --steps 500000000
```

## W3: Everglade's river (done, adapted)

The old W3 put ponds, a weir and rowboats in an older Everglade. Today's
Everglade has one 4 m river along x = 88, crossed by the river trail's
bridge, so W3 is that river (docs/verse/everglade.md, "The river"):

- `verse/everglade/plan.coax` carves its bed: a level that falls south and
  stays 0.2 m under the lowest ground beside it, a V 2.2 m deep at the
  middle, and banks that climb one in one to the ground (a ravine in the
  south woods). The bridge's deck spans the cut; you walk over it.
- [`verse/everglade/river.coax`](../../verse/everglade/river.coax) is one
  W1 river body on that level profile with a current from
  `water_flow_river` round the bridge's two piles, a gentle wind sea, and
  the nodes the shader draws (13 across, 209 along; the bed's depth and
  shore distance baked from the carved ground). It is its own library: the
  realm's face passes values between it, the page and the controller.
- The swimmer (the old `physics::water::medium` and `verse-world`'s
  breath): ground, wading past 0.4 m, swimming past 1.25 m or off the
  bed, diving with the eye under; a swimmer settles at the float line, an
  idle diver rises at 0.6 m/s. Half speed wading and swimming (SRD 5.2.1),
  a sixth less each Exhaustion level; `verse/player.coax`'s walk and run
  speeds are unchanged on land (its new `pace` is 1 there). Breath: a
  minute (the Ranger's Constitution is +0), then an Exhaustion level
  every 6 s; the sixth puts you on the nearer bank. No jumping afloat; the
  current carries a swimmer. C dives.
- Floats: three ducks paddling round their stretch of the glade against
  the current and six lily pads on their stems, W1 rigid bodies on
  `water_apply`'s buoyancy and drag.
- Swimming across is allowed: a glade either side of the bridge opens the
  river from z = -40 to 12, so the far bank is reached by water as well as
  by the bridge. Up and down the river past it the forest closes in. The
  plan's reachability and slope checks say so.

- Ponds: four, carved into the woods and opened as clearings
  (`verse/everglade/plan.coax` `EG_POND`): the Lantern Pond (-77, -11),
  6 m in radius and 3 m deep; the Reed Pond (14, -90), 5 m and
  1.6 m; the Thinking Pond (28, 73), 5.5 m and 2.5 m; the Fern Pond
  (-79, 21), 5 m and 2 m. Each one's level sits 0.25 m under the lowest
  ground 5 m past its rim; its bed is a bowl to that depth and climbs
  0.7 in 1 past the rim to the ground. Each is a W1 pond body in the same
  `water_set` as the river (`river.coax`), so wading, swimming, diving,
  breath and the bank you come back to all work the same; the shader draws
  each as 8 wedges of a disc (`everglade.wgsl` `pond_vertex`) from the
  levels the page passes (`U.ponds`), risen by the rain as the river is
  (W9, `pond_row`). `#lantern-pond`, `#reed-pond`,
  `#thinking-pond` and `#fern-pond` put you on their banks. Each has a
  duck paddling round its middle and three lily pads at 0.6 of its rim,
  floats like the river's.

- The weir (Glade Run's, from the old W3): at z = -30, 19 m below the
  bridge, the level steps down 0.25 m over a sill of six stones 0.12 m
  under the water above it, into a plunge pool 2.5 m round whose middle is
  1.6 m deeper than the channel (`plan.coax` `EG_WEIR_*`, `EG_POOL_*`,
  `everglade_pool_deepen`). The river's level profile is a point every 2 m
  (a ground row, `RIVER_PROFILE`), so the step stays at the sill. The
  nodes carry white water down the drop and churning in the pool
  (`river_bake`). `#weir` puts you on the west bank above it. Its mist is
  W6's.

- Rowboats: one moored at each of the Lantern, Thinking and Fern ponds, a
  W1 rigid body (a 3.2 x 0.4 x 1.1 m hull of 220 kg, riding 6 cm under)
  on a slack line back to its mooring that keeps its bow along the shore
  (`river.coax` `FLOAT_BOAT`); drawn from boxes (`everglade.coax`
  `rowboat`). Boarding and rowing are W6's.

Not ported yet (task `verse-water-w3-rest`): townsfolk swimming. Everglade's
people stand at their places and don't walk yet; when they do, they wade
and swim by the same rules as you. Not
ported: underwater combat. You swim with the walk clip (there is no swim clip yet), and a
phone has no dive button yet.

## W5: the scene behind the water (done, in part)

A page whose WGSL declares `water_scene` (Everglade) has the scene's light
and depth copied after the scene and before the water (`host.js`
`waterCopyLayout`: the post pass's rgba16float light and the depth32float
depth, at `@group(3)`; the render pass ends, the two copies are made and a
pass that loads both goes on). Pages without it draw as before. With it, a
surface (the river, the fountain's pools) draws in one piece in the light
half over the copy, and the transmittance half only clears what it covers
(`water_whole`, everglade.wgsl); the fountain's falls, drawn over its pools,
keep the two halves.

- **Refraction.** What lies behind is read from the copy beside the pixel
  by the surface's slope, 0.16 m a metre of water up to 1.2 m (0.12 on a
  phone), never from something in front of the surface (its depth is
  checked).
- **Contact foam.** Where the water is much thinner along the view than
  the baked depth to the bed says, something stands in it: a broken white
  line round the bridge's piles, the fountain's column, a swimmer and the
  ducks.
- **Soft edges.** Water thinner than 6 cm against anything fades to what
  is behind it.
- **Reflection** (desktop): 20 growing steps along the ray reflected off a
  nearly calm surface, in screen space against the copied depth; where one
  finds the scene above the water it takes the sky's place in the
  reflection, fading at the screen's edges and past 12 m. This stands in
  for the old plan's planar reflection; a phone keeps the sky.

Measured headless on an Apple GPU (`frametime.ts`, `#river-wade`, the river
filling the view): desktop 1440x900 11.4 ms a frame before and after (the
surface now shades once, not in each half), a phone's canvas 3.2 to 3.8 ms.
Not done yet: soft particles (Everglade's water has no particles yet; W6's
spray will read the copy). `#river-wade` starts you in the river north of
the bridge, facing its piles.

## The painted preset (art-styles.md 12, as-painted-water)

`water_preset(WATER_PAINTED)` ("painted", the eighth preset in
[`verse/water-look.coax`](../../verse/water-look.coax)) sets `W_ABSORB + 3`
to 1, and the shader's `water_painted` (everglade.wgsl) draws it in place
of the optics: depth bands (shallow (0.30, 0.72, 0.74) under 0.4 m, mid
(0.10, 0.45, 0.55) to 1.5 m, deep (0.05, 0.22, 0.36) below, each edge soft
over 0.15 m, `water_paint_mid` and `water_paint_deep`), the painted sky in
Schlick's Fresnel with no roughness blur (on a calmer normal, the waves
bending it a third as much, and the sky never more than half), sparkles on
a 0.15 m grid (0.25 m on a phone) within 4° of the key's reflection,
twinkling, a 0.25 m white shore line with a broken inner edge, white water
round the bridge's piles, W6's ripples as white rings, and thin broken
streaks carried along the current. The bands take the land's light and its
shadows. The physics, the mesh and the swimmer are unchanged. Everglade's
river is drawn in it (eg-paint-river); Fairy Forest's stream had its own
copy in fairy.wgsl first.

Caustics (from W7): `water_caustics` lights what lies under the water at
rest (the river's bed and banks) with the ridges of two drifting noise
fields, faded in over the first 0.1 m and out with depth, in the key's
light and shadow.

## The fountain (verse-water-fountain)

The commons' fountain is lathed stone with its water drawn by the water
shader (everglade.coax `lathe`, everglade.wgsl `lathe_vertex`,
`fountain_vertex`):

- **Stone.** A profile of (radius, height) points turned about the axis,
  48 sectors round, in 8 wedge instances of 6 sectors by 28 segments so one
  stays inside the pack's 1,024 identity indices (`SHAPE_LATHE`, its own
  draw before the water). The basin: a dished floor, the inner wall, a
  rounded coping lip, the outer wall and a plinth, 6.6 m across. The column:
  a moulded base, the shaft, a collar, the upper bowl (floor, lip,
  underside) and a finial. `carved` shades it as coursed blocks (0.26 m by
  0.6 m, staggered), each its own tone, dark joints, streaked weathering,
  wet below the water and a mossy band at the waterline.
- **Water.** After the river's patches, 32 instances of the water draw: the
  basin's pool (8 wedges of rings and sectors) and the upper bowl's (4), in
  the painted look at 1.2 and 1.4 times their depth (the basin is 0.4 to
  0.6 m deep), the bands taken 85% of the way to a bright teal
  (`water_painted_teal`, (0.12, 0.70, 0.64)) with the sky at most 0.33 of it with a shore line half a river's width; 8 streams leaving the
  bowl's lip at 0.7 m/s and falling as thrown water does for 0.555 s to the
  pool 1.77 m out, each a tube that leaves the lip as a broad tongue; 4 arcs
  of the jet from the finial into the bowl; and 8 splash crowns where the
  streams strike. Where they strike the pool churns white, runs outward and
  rings spread (`fountain_pool_shade`). A stream is streaked, broken teal
  and white with the sky in its rim (`fountain_fall_shade`).
- **Caustics** on the basin's floor and the bowl's (`fountain_depth_at`
  feeds `water_depth_at`).
- **In your way** as a round body the plinth's 3.32 m across
  (everglade.coax `bodies`), not the plan's 6 m box.
- Two numbers ride in the water block's spare `W_GRID2` z and w: the ground
  under the fountain and 1 when it is drawn (everglade.coax `U_FOUNTAIN`).
  On a phone the pools and streams take the phone's coarser quads; the
  fountain adds 1,536 water triangles there.

## W6: ripples (done; the rest of W6 is not)

`water_look_ripples` puts 24 expanding rings in the uniform block (the old
`frame.rs` ripple field); the shader bends the normal by them and draws a
foam rim on young ones. In Everglade you splash where you come into the
water and leave a ring every 0.35 s you move through it, weaker diving.
Rowboats, wakes from other bodies and the rest of W6 are not started.

```sh
coax/bin/coax test --library verse/everglade/river.coax --steps 2000000000
```

## W4: the spectral sea (done)

[`verse/water-spectrum.coax`](../../verse/water-spectrum.coax) is a port of
the previous engine's `physics::water::{spectrum, fft}` and `verse-pbr`'s
baked tile (`git show 0b401561a0^:crates/physics/src/water/spectrum.rs`,
`fft.rs`, `spectrum_tests.rs`, `crates/verse-pbr/src/water/tile.rs`), with
the same formulas and constants:

- **Spectrum.** JONSWAP from wind speed and fetch with the TMA depth
  factor, the finite-depth dispersion relation, Hasselmann's
  frequency-dependent `cos^2s(theta/2)` spreading with Horvath's swell term
  and a standing share, the significant height.
- **Synthesis.** Each lattice mode's amplitude is the spectrum integrated
  over its cell (variance by Parseval, not a draw), a seeded phase, and a
  frequency snapped to a whole multiple of 2 PI over the loop, so its phase
  is an integer counter on W1's clock: a tick reproduces its tile bit for
  bit and the loop folds. A radix-2 inverse FFT (f32) makes height,
  choppy displacement, slopes and the Jacobian per tile.
- **Cascades by tier.** Up to three tiles 5.3 times smaller each split the
  wavenumbers into bands; cascade 0 is the gameplay band at 64^2
  (`wsp_field`: displacement and the water's velocity, sampled bilinearly,
  with `wsp_surface_height` inverting the displacement as W1 does).
- **The phone's baked tile.** `wsp_bake_tile`: one tick of a half-metre to
  8 m sea, slopes scaled to 0.085 RMS, with a mip chain whose third channel
  is the slope variance the averaging lost (Toksvig).
- **Whitecaps and the foam field.** `wsp_foam_step` makes foam where the
  Jacobian falls under a threshold set so the share of white water is
  Monahan's `3.84e-6 U^3.41` for the wind, and lets it decay.
- **Shallow water and surf.** `wsp_surf`: a wave's wavenumber over depth,
  its shoaling gain (energy flux kept) and how far it is past McCowan's
  0.78 breaking limit.

The old tests are ported as checks (the FFT against the direct sum, tile
variance against the spectrum's integral for four seas, loops and
reproduction, the published shape, sharpening crests, the gameplay band's
consistency), plus the surf and the baked tile. To fit the evaluator's
budget they use 64-texel tiles (the old moderate sea used 128), 8 ticks
(32) and an 8 by 8 band integral (12 by 12), so the short cascades' mean
over the loop is held to 6% (2%) and the whole surface to 4% (2%).

```sh
coax/bin/coax test --library verse/water-spectrum.coax --steps 2000000000
```

Not ported: the old `Field` cache (a caller keeps its own tile), the
`water_fft_cost` example (W11 measures the lab instead), and the
WaveSet's spectral mode in `verse/water.coax`: Everglade's 4 m river has
no fetch for a spectral sea, so it keeps its Gerstner waves.

### The water lab (/realms/waterlab)

Where the water system shows what Everglade has no place for: an unlisted
realm ([`waterlab.coax`](../../apps/openagents-web/waterlab.coax),
`waterlab-face.c`, `waterlab.wgsl`) with no assets, one traced pass. A sea
to the horizon on a 7 m/s wind (`?wind=`) breaks on a sloping beach, in the
painted look of the river (depth bands, the painted sky in Fresnel on a
calmer normal, sparkles, white paint for whitecaps, surf and the waterline)
under BotW's hazy teal air. Each frame the CPU synthesizes the tiles at the
water's tick and packs them into the uniform block as half floats: cascade
0's height, foam and slopes, and the finer cascade's slopes on a desktop or
the baked tile on a phone (`?tier=phone` forces it). Toward the beach the
shader shoals the band (`shoal_gain`), integrates the shallow wavenumber
across the beach for the surf's phase, and paints the crests past McCowan's
limit white. `?t=` holds a moment (goldens hold 12 s); `?yaw=`, `?pitch=`,
`?dist=` place the view.

Measured headless on this Mac (Apple GPU, `frametime.ts`): 0.71 ms GPU per
frame at 390x844, 1.59 ms at 1440x900 at device pixel ratio 2, with the
CPU's synthesis inside a 1.7 ms frame. W11 takes the phone numbers on real
devices.

## W7: under the water (done)

[`verse/water-under.coax`](../../verse/water-under.coax) ports the old
underwater module as numbers the shaders mirror, with its tests:

- **The waterline across the view.** `water_under_line` finds where the
  surface crosses the near plane per column, so a camera half in shows the
  world split, the top above and the bottom under, with a meniscus line
  on a desktop.
- **Submersion and fog.** `water_submersion` and `water_under_share` say how
  far under the eye is; `water_fog` is Beer–Lambert absorption per channel
  (reds go first) plus inscatter lit by the sun.
- **Snell's window.** `water_snell_angle` and `water_snell_transmit`: from
  below, the sky shows inside a 48.75° cone and the rest of the surface
  mirrors the water.
- **Tiers.** `water_under_tier`: a phone draws 48 motes and no shafts, waver or
  meniscus. A desktop draws 160 motes, 16 samples of sun shafts, a 0.004
  waver and the meniscus.
- **Sound, computed only.** `water_under_muffle` is the RBJ 450 Hz low-pass a
  submerged listener hears through, checked by its gain: 1 at 0 Hz, -3 dB at 450 Hz and under 0.012 at 4.5 kHz.
  Nothing plays it, and no test plays sound.

**Everglade.** `verse/everglade/river.coax` writes the eye's height over the
river into `W_CONTROL.w`. `water_under_view` in `everglade.wgsl` then fogs
every scene pixel and the sky under that line, with motes and, on a desktop,
shafts. Caustics (`water_caustics`) already light the submerged bed.
Holding C dives (`#river-wade` spawns in the river), and the camera comes
down to your head, looking level along the river, so the view goes under.
Everglade has no post pass, so the waver and meniscus are drawn only in the
lab.

**The lab.** Q dives the lab's camera and E brings it up (`?depth=` sets it).
The traced pass draws the split line, the waver, Snell's window, shafts,
motes and caustics on the sand.

Measured headless (`frametime.ts`, GPU p50). Everglade diving: 3.09 ms at
390x844 on the phone tier (3.93 ms on the surface), and 15.2 ms at 1440x900
at device pixel ratio 2 (12.1 ms on the surface; the shafts cost 3 ms).

```sh
coax/bin/coax test --library verse/water-under.coax --steps 2000000000
```

## W9: weather (done)

[`verse/water-weather.coax`](../../verse/water-weather.coax) ports the old
`physics::water::weather` with its tests. The weather is a pure function of
the zone's climate, its seed and the world tick (120 Hz from the page's
clock), so every visitor, a late joiner and a replay see the same sky with
no messages:

- **The schedule.** Ten-minute blocks draw clear, overcast, fog, rain or a
  storm from the climate's weights (`weather_temperate` for Everglade,
  mostly clear; `weather_coastal`, wetter and windier), blend over 60 s, and
  vary the rain and the wind with seeded smooth noise. A storm flashes with
  lightning.
- **SRD effects.** `weather_heavy` (Heavy Precipitation) and
  `weather_strong_wind` (9 m/s and over; it disperses the fog).
- **Spell weather.** `WeatherOverlay`s for Create Water's rain, Sleet Storm,
  Fog Cloud, Call Lightning's cloud and Storm of Vengeance. Each is a local
  area with a start tick and a duration that never changes the schedule.
- **The ground and rising water.** `weather_ground` integrates the wetness
  and the puddles over the last hour on a 30 s grid fixed to the tick.
  `weather_rise` is the mean rain over the last 30 minutes times 0.15 m, and
  `weather_flow_gain` (up to 1.6) speeds the current for it.

**Everglade.** `weather_frame` in `everglade.coax` samples the schedule each
frame and writes `u.weather` (three rows after the lamps). It also integrates
the ground and the rise once a second.

- **Under cloud.** The key light dims, the ambient dims, and the painted sky
  and haze go blue-grey. Fog and rain thicken the haze.
- **In `everglade.wgsl`.** Rain falls as wind-tilted streaks on cylinders
  round the eye (2 layers on a phone, 4 on a desktop). Drops ring the river
  and the puddles (`weather_rain_slope`). Wet surfaces and characters darken
  and take a painted sheen. Puddles fill the ground's hollows and mirror the
  grey sky, never the sun's disc. Lightning flashes the sky and the ground.
- **The river.** `everglade_river_weather` raises the river by the rise, on
  the surface and in the swimmer's rules, and speeds its current by the gain.
- **Pinning it.** `?weather=clear|overcast|fog|rain|storm` pins the state.
  A still page (a golden) keeps a calm sky unless it is pinned, so the
  goldens don't change with the schedule.

Not ported: the old splash-back and eave drips, and a swimmer staying wet
after leaving the water.

Measured headless (`frametime.ts`, GPU p50). In a storm: 4.76 ms at 390x844
on the phone tier (3.60 ms clear), and 12.5 ms at 1440x900 at device pixel
ratio 2 (8.7 ms clear).

```sh
coax/bin/coax test --library verse/water-weather.coax --steps 2000000000
```
