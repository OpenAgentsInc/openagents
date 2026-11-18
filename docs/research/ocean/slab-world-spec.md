# SLAB world: a simulation spec for the Metaverse Engine

Status: phases S1–S7 built, November 18, 2026, as
**SLAB Simulation** at `/worlds/slab` (the older `/realms/slab` moves
there). The hull floats on column buoyancy
with damping, moorings, wind and current (`metaverse/slab-sim.coax`); the
`prime` deck is built and can be walked with GameBuilder's character
controller; the seven sites, six sea states, OTEC, nuclear and hybrid power,
the `demo10` preset and the dispatcher run, with a motion and power panel and
a card for anything clicked. Cell A1 can be holed and the ballast pumped
against it; the section view (V) cuts the hull open and the underwater view
(U) shows the bottom, the cold-water pipes and the moorings; the flow
overlay (L) animates the plant's water and the power lines; the section draws
the hull's first bending mode 100 times exaggerated. One ledger,
`metaverse/hull.coax`, weighs the hull for the world, its cards, the section
and the construction timelapse (section 3.1). The `dome` and
`cluster` layouts (K) draw the Silone domes, their bridges and, with nuclear
power, the moored powership. The golden is still to be taken on staging. Measured numbers are in section 12. The spec was
revised the same day with sourced OTEC figures from [otec.md](otec.md).

This is the brief for a coding agent to build **SLAB**, an unlisted realm in
the OpenAgents Metaverse Engine that rebuilds Palmer Luckey's proposed
offshore industrial platform as a walkable, physically simulated world. It
gives every dimension, mass, rate and test the agent needs. It also says
which parts come from public sources and which are our own engineering
choices.

Read first:

- [palmer-luckey-post.md](palmer-luckey-post.md): the announcement and its
  concept image [slab.jpeg](slab.jpeg).
- [slab.md](slab.md): the research report, especially section 3
  (engineering), section 5 (law and location) and section 7 (data centers).
- [silone-dome.md](silone-dome.md): Ben Silone's floating dome concept and
  the video [silone-dome.mp4](silone-dome.mp4).
- [otec.md](otec.md): the OTEC assessment (full report:
  [OTEC_for_SLAB_Research_Report_2026-11-18.pdf](OTEC_for_SLAB_Research_Report_2026-11-18.pdf)).
  Sections 5 and 6 below take their sites, flows, output model, cooling and
  mining figures from it.
- [ocean-atomics-reactor.md](ocean-atomics-reactor.md): Ocean Atomics' 50 MWe
  marine reactor as shown in its video. Section 6.6's nuclear option takes
  its figures from there.
- Engine docs: [realm-host.md](../../metaverse/realm-host.md) (how a realm is
  registered), [water.md](../../metaverse/water.md) (the water physics, the
  spectral sea, the ocean clipmap and the water lab).

## 1. What is real and what is ours

SLAB has published **no** dimensions, materials, site, power system or
layout. This world is a reference design that is consistent with what has
been said and with basic naval architecture. Mark it as a reconstruction
inside the world (section 8).

| Item | Source |
|---|---|
| Seastead for heavy industry: metal processing, medical, desalination, refining, chemicals | Luckey's post |
| 1,000-year design life | Luckey's post |
| Rectangular concrete block with sheer walls, a city, towers, gardens and smokestacks on top | The AI-generated concept image (not a design) |
| Floating concrete dome of about 400 ft (122 m), airform and shotcrete, joinable modules, calm equatorial water (17 ft max wave) | Ben Silone's reply (not confirmed as SLAB) |
| Hull, mass, mooring, deck and load numbers (sections 3, 4, 6.4, 7) | **Ours**, chosen for this world and derived in this doc |
| OTEC as the power source | **Ours**, the owner's direction for this world |
| OTEC flows, output model, site temperatures and rankings, cooling, mining revenue (sections 5 and 6) | [otec.md](otec.md), which itself separates measured results, engineering models, developer claims and its own calculations. Where this spec still uses a planning value, it says so |
| OTEC at 120 MW net | **Unproven.** The largest verified net output is about 103 kW (Hawaii, 1990s), and no megawatt-scale plant has run commercially (otec.md section 3) |
| Nuclear option: a 50 MWe module of about 160 MWt in an 18 × 12.5 m compartment, for hulls 500 ft and up (section 6.6) | Ocean Atomics' explainer video, as read in [ocean-atomics-reactor.md](ocean-atomics-reactor.md). This is the company's claim, not verified |
| Module house load, compartment width, refuelling outage length, and where modules sit in the hull | **Ours**, planning values until the nuclear research arrives |

## 2. The realm

Register one row in `apps/openagents-web/realms.json`, then run
`bun coax/web/realms.ts` to write the generated files. Do not edit them by
hand. `bun coax/web/realms.ts --check` must pass.

```json
{ "id": "slab", "name": "SLAB SIMULATION", "kind": "open", "listed": false,
  "orbit": { "r": 0.5, "a": 3.6, "size": 0.02 },
  "realm": { "slug": "slab", "title": "SLAB Simulation",
    "description": "A reconstruction of SLAB, the floating platform for industry that Palmer Luckey proposed: a kilometre of concrete carrying a city on open ocean. Walk it, cut it open and watch it built.",
    "share": "Walk a reconstruction of SLAB, the floating platform for industry Palmer Luckey proposed.",
    "alt": "A kilometre-long concrete platform carrying a city and industrial stacks on open ocean",
    "style": "photoreal", "clock": "fixed:0.5", "module": "slab", "slot": "slab",
    "golden": { "lit": 0.3, "settle": 3000, "blocks": 6 }, "assets": [] } }
```

- **Unlisted.** With `listed: false` the realm stays off `/worlds` and the
  galaxy, but its page opens at `/worlds/slab`. The site moves every
  `/realms/...` address to `/worlds/...` for good, query kept, so
  `/realms/slab` opens it too. The row's `description` no longer says
  "unlisted" (the world is linked from a public post), and a `share` line
  gives the link preview its text.
- **Orbit.** Check that the orbit is clear of other rows. If it isn't, pick a
  clear one by the rules in realm-host.md.
- **No asset files.** Everything is generated from code, as in the water lab.
- **Golden.** The release walk opens every realm, so a new realm needs a
  golden. Follow the steps in realm-host.md ("The release walk").

Files, following the water lab's pattern (`waterlab.coax`, `waterlab.wgsl`,
`waterlab-face.c`):

| File | Holds |
|---|---|
| `apps/openagents-web/slab.coax` | The realm: state, input, HUD and frame loop |
| `apps/openagents-web/slab.wgsl` | Shaders: sea (start from `waterlab.wgsl`), hull, deck, buildings |
| `apps/openagents-web/slab-face.c` | Host face, as in `waterlab-face.c` |
| `metaverse/hull.coax` (new) | Floating-hull physics: column buoyancy, added mass, damping, moorings, flooding. Kept out of the realm so other worlds can reuse it |
| `metaverse/slab-plan.coax` (new) | The platform's dimensions, compartments, districts, building and plant layout as data |

Units are metres, seconds, kilograms and radians. **+Y is up**, matching
`metaverse/water.coax` and `metaverse-engine/physics/sim/rigid.coax`. The
world origin is the platform's centre at the calm waterline. +X runs along
the platform's length toward the bow (east), and +Z points north.

**The realm page.** `apps/openagents-web/realms/slab/index.md` is what a
visitor without WebGPU reads, as plain text. It leads with five stills and
the plain story (how big, why it floats, how it is built, where it could
go), then the feature notes. The stills are in
`apps/openagents-web/realms/slab/pictures/` and are served at
`/worlds/slab/pictures/NAME.jpg`. Retake them when the world's look
changes, with a dev server on PORT:

| Picture | Address after `http://127.0.0.1:PORT/worlds/slab` |
|---|---|
| `platform.jpg` | `?hud=0&t=40&yaw=0.802&pitch=0.30&dist=900&x=120&z=60&y=20` |
| `on-deck.jpg` | `?hud=0&t=40&mode=walk` |
| `temples.jpg` | `?hud=0&t=40`, then the key 1 (`--eval` a `keydown` on `window`) |
| `section.jpg` | `?hud=0&t=40&view=section` |
| `build.jpg` | `?hud=0&view=build&day=503.8` |

Each is `coax/web/tool.sh shot ADDRESS OUT.png 1600x900 6000`, then
`sips -Z 1280 -s format jpeg -s formatOptions 72 OUT.png --out NAME.jpg`.
The link-preview card `apps/openagents-web/og/realms/slab.jpg` is retaken
with `bun coax/web/og-render.ts /realms/slab` on a staged site
(docs/web/og-tags.md).

## 3. Layouts

The realm has three layouts, selected with `?layout=` and from the HUD.
The default is `prime`.

### 3.1 `prime`: the monolith (default)

This is a rectangular concrete platform read from the concept image. A
watertight cellular hull 50 m deep does all the floating. On it stands a
lighter structure that carries the city deck 93 m higher, to where the
picture's wall ends.

Every figure in this section comes from one ledger, `metaverse/hull.coax`.
The simulation, the hull's card, the section view and the construction
timelapse all read it, and checks fail if any of them gives a different
depth, concrete volume, mass or ballast. To change a figure, change the
ledger's shape (`HullShape`) and rerun its checks.

Each figure is marked **picture** (read from the concept image), **assumed**
(our planning value) or **follows** (arithmetic on the others). SLAB has
published none of them.

| Dimension | Value | From |
|---|---|---|
| Length L (X) | 1,000 m | Assumed |
| Beam B (Z) | 600 m | Assumed; the picture's fit gives about 550 m |
| Watertight hull depth | 50 m | Assumed |
| City deck over the keel D | 143 m | Follows from the draft and the picture's wall |
| Design draft T | 25 m | Assumed |
| Deck above calm water | 118 m | Picture |
| Hull's top above calm water | 25 m | Follows |
| Deck area | 600,000 m² (60 ha) | Follows |
| Corners | Sharp, 1.5 m radius in plan | Picture |
| Sea water, concrete | 1,025 and 2,500 kg/m³ | Standard values |
| Displacement at T | (1,000 × 600 − 1.9 m² of corners) × 25 × 1,025 = **15.375 Mt** | Follows |

**Following the picture (2026-11-18).** The owner's direction for the first
version is parity with the concept image, walls included. A camera fitted to
the picture's deck corners (about 5 px RMS, a 51° horizontal field) puts its
wall about 118 m over the water on a beam of about 550 m. The world therefore
puts the deck 118 m over the water and squares the corners, and keeps the
1,000 × 600 m plan and the 25 m draft. The picture's horizon sits higher than
its slab's perspective allows, so no single camera matches both; the world
matches the slab.

**Which hull (decided 2026-11-18).** A deck 143 m over the keel can be
carried two ways. Both were weighed at the 25 m draft with 2.5 Mt of
buildings and liquids:

| | A: one structural hull 143 m deep | B: a 50 m hull with a lighter structure on it |
|---|---|---|
| Cast in one piece | 2.41 + 0.45 (outer wall) + 1.25 (bulkheads) = 4.11 million m³, 10.3 Mt; 2.6 Mt of ballast left | — |
| Built from 100 × 50 m caissons, as the timelapse builds | 5.40 million m³, 13.5 Mt; no ballast left; floats at 26.0 m | 3.83 million m³, 9.57 Mt; **3.31 Mt of ballast left** |
| One caisson afloat with 3,000 t of tow ballast | 104,254 t, draws 20.9 m, GM −45 m: it capsizes | 58,931 t, draws 11.50 m, GM +2.48 m |
| Transverse GM of the finished body | positive | **+1,159 m** |

The world uses **B**. Nothing can cast a kilometre in one piece, and a
caisson 143 m tall neither floats upright nor passes a 16.5 m channel. B is
what the timelapse can build: it closes at the drawn draft with ballast to
spare and positive GM, and the bulkheads' upper 93 m, which would hold out
no water, become columns.

**The hull** (watertight, keel to 50 m). Reinforced and prestressed concrete.
It is 120 caissons of 100 × 50 × 50 m, cast in a yard, towed and joined
(all thicknesses assumed):

| Part of one caisson | Size | Volume |
|---|---|---|
| Bottom slab | 100 × 50 m, 1.5 m thick | 7,500 m³ |
| Top slab | 1.0 m thick | 5,000 m³ |
| Walls | 0.6 m all round and one 0.6 m web across the middle, 207.84 m² in plan, 47.5 m tall | 9,872 m³ |
| **One caisson** | | **22,372 m³ = 55,931 t** |

| The hull | | Volume |
|---|---|---|
| 120 caissons | 120 × 22,372.4 | 2,684,688 m³ |
| Sea-wall lining | The outer walls thickened from 0.6 m to 1.5 m from inside, at the berth: 2,818 m² in plan, 47.5 m tall | 133,876 m³ |
| **Hull concrete** | | **2,818,564 m³ = 7.05 Mt** |

The sea walls are lined after joining because a caisson cast with 1.5 m sea
walls cannot be towed: a corner one would draw 14.5 m and float top-heavy
(GM below zero). Every caisson is therefore the same on tow.

Joined, the caissons' walls pair into bulkheads every 100 m along the hull
and every 50 m across it, with the webs between them. They form **240
cells**, each 50 × 50 m in plan and 47.5 m clear inside, labelled by column
A–T (X, west to east) and row 1–12 (Z, south to north).

**The upper structure** (50 m to the deck at 143 m; all sizes assumed). It is
open inside: columns stand on the hull's bulkhead crossings, so their load
goes straight down the walls below. It is cast at the berth, after joining.

| Part | Size | Volume |
|---|---|---|
| Outer wall | 0.8 m thick, 92.4 m tall, around the edge | 236,307 m³ |
| Columns | 209 (11 × 19 crossings), 15 m² each, 89.4 m tall | 280,269 m³ |
| Girders | 2 m wide and 3 m deep under the deck, on every bulkhead line (22.4 km) | 131,604 m³ |
| Deck slab | 0.6 m thick | 360,000 m³ |
| **Upper structure** | | **1,008,180 m³ = 2.52 Mt** |

The columns carry the deck, the girders and the buildings at about 12 MPa at
their feet. That is a first-pass check that the sizes are plausible, not a
design.

**Mass budget at the design draft:**

| Item | Mass | From |
|---|---|---|
| Hull concrete | 7.05 Mt | 2,818,564 m³ |
| Upper structure | 2.52 Mt | 1,008,180 m³ |
| *All concrete* | *9.57 Mt* | *3,826,744 m³* |
| Topsides: buildings, plant, equipment | 2.0 Mt | Assumed |
| Liquids: process tanks, fuel, fresh water | 0.5 Mt | Assumed |
| Seawater ballast in the cells | 3.31 Mt | What is left: 15.375 − 9.567 − 2.5 |
| **Total** | **15.375 Mt** | The displacement at 25 m |

Each metre of draft carries 615,000 t. The concrete alone floats at 15.6 m.
Ballast is the trim variable: adding or removing topside load is balanced by
pumping ballast, so the draft stays at 25 m. The 3.31 Mt is 3.23 million m³
of sea water, 5.64 m deep over the cells' 572,241 m² of floor. Its surface is
7.14 m over the keel, 17.9 m under the sea's, and the section view draws it
there. Another 3.31 Mt of topsides would use it all; past that the draft
grows.

**Centre of gravity.** Moments about the keel:

| Item | Mass | Height of its centre | Moment |
|---|---|---|---|
| Hull concrete | 7.05 Mt | 22.6 m | 159 Mt·m |
| Upper structure | 2.52 Mt | 118.2 m | 298 Mt·m |
| Topsides | 2.0 Mt | 168 m: 25 m over the deck (assumed) | 336 Mt·m |
| Liquids | 0.5 Mt | 30 m (assumed) | 15 Mt·m |
| Ballast | 3.31 Mt | 4.3 m | 14 Mt·m |
| **Total** | **15.375 Mt** | **KG = 53.5 m** | 822 Mt·m |

**Hydrostatics** (rectangular-barge formulas, used as test targets in
section 9):

| Quantity | Formula | Value |
|---|---|---|
| KB | T/2 | 12.5 m |
| Transverse BM | B²/(12T) | 1,200 m |
| Longitudinal BM | L²/(12T) | 3,333 m |
| Transverse GM | KB + BM − KG | **1,159 m** |
| Longitudinal GM | | **3,292 m** |
| Heave natural period | 2π √(T(1+Cₐ)/g) | 10.0 s with Cₐ = 0; 14.2 s with Cₐ = 1 |
| Roll natural period | 2π k / √(g·GM), k = 0.35B (assumed) | 12.4 s; 14.1 s with Cₐ = 0.3 on rotation |
| Pitch natural period | k = 0.29L (assumed) | 10.1 s; 11.6 s with Cₐ = 0.3 |

Cₐ is the added-mass coefficient. It is a parameter (section 4.2), not a
measured value. The raised deck lifts KG from 25 m to 53.5 m, which costs
29 m of a GM over a thousand metres: the platform is still very stiff.

**The timelapse builds this hull.** `metaverse/slab-construction.coax` tows
the ledger's caisson (58,931 t with its tow ballast, 11.50 m of draft) and
joins the 120th on day 983. A lighter then makes 200 calls of 26,776 t over
800 days: first 2.86 Mt of concrete for the sea-wall lining and the upper
structure, cast at the berth while the deck rises from 50 m to 143 m, then
the 2.5 Mt of buildings and liquids. Ballast is pumped as the load comes
aboard. It ends on day 1,783 at the ledger's figures: 3,826,744 m³ of
concrete, 3.31 Mt of ballast, 15.375 Mt, 25.0 m of draft and the deck 118 m
over the water. The check "the timelapse ends on the world's hull" in that
file fails if any of these differs from the world's.

**Wave averaging.** At 1 km by 600 m the hull spans many wavelengths of an
equatorial sea: an 8 s wave is about 100 m long. Wave forces largely cancel
across the hull. This is why the world uses column buoyancy (section 4.1):
the cancellation then happens in the simulation instead of being assumed.

**Openings in the hull:**
- **Dock notch.** A sheltered dock cut into the south wall at columns J–K:
  100 m along the hull, 60 m deep into it, floor at −8 m. This is our
  addition, because the concept image shows no harbour.
- **Moonpools.** Four 12 m-diameter shafts through cells B3, B10, S3 and S10,
  open to the sea, carrying the OTEC cold-water pipes (6.7 m each, section
  6.1).

### 3.2 `dome`: one Silone dome

This is a single module from Ben Silone's concept: a cylindrical base ring
under a terraced dome.

| Dimension | Value |
|---|---|
| Diameter | 121.9 m (400 ft) |
| Submerged cylinder | Draft T = 12 m |
| Cylinder freeboard | 4 m |
| Dome | Spherical cap rising 30 m above the cylinder's top, built as 5 stepped terraces, each 6 m high and 6 m deep, with planters along every edge |
| Top | Glass lift lantern, 8 m diameter, 10 m tall |
| Displacement | π × 60.96² × 12 × 1,025 = **144,000 t** |

**Structure:**

| Part | Size | Volume |
|---|---|---|
| Cap shell | 0.4 m | 5,800 m³ |
| Base ring wall | 0.6 m | 3,700 m³ |
| Base slab | 1.0 m | 11,700 m³ |
| Floors | | 6,300 m³ |
| **Total** | | **27,500 m³ ≈ 69,000 t** |

Ballast and payload make up the remaining 75,000 t, with KG = 10 m.

**Compartments:** 8 equal sectors below the waterline, plus a central core
30 m in diameter.

**Hydrostatics:**

| Quantity | Formula | Value |
|---|---|---|
| BM | r²/(4T) | 77.4 m |
| GM | | ≈ 73 m |
| Heave natural period | | 6.9 s (Cₐ = 0) to 12.0 s (Cₐ = 2) |
| Roll natural period | k = 0.45r | ≈ 6.4 s (7.3 s with Cₐ = 0.3) |

The dome's roll period falls inside the band of common wave periods. The
world should show this as a finding rather than tune it away.

**Simulated (metaverse/slab-sim.coax, `sim_dome_shape`):** mass 0.1436 Mt,
GM 73.4 m; it floats at its 12 m draft within 0.12 m; it heaves more than
`prime` in the same sea; one flooded sector (cells below the waterline, 12 m
high) takes on about 18,000 t and heels it 3.4°.

**Drawn (`?layout=dome`, key K):** the base drum, five terraces stepped 6 m
in with planters along their edges and glass risers, and the lantern
(`slab_silone` in slab-plan.coax, `slab_silone_hit` in gpu/slab.coax). The
dome heaves, rolls and pitches with its simulated motion, and white water
rings its waterline. Its card gives the live heave, roll, pitch, drift and
peak line tension. The lantern is drawn 4 m in radius, as given above; the
atrium and interior are not drawn. Walk, section, underwater and flow belong
to `prime` only; the HUD says so if they're asked for here. With `nuclear` or
`hybrid` power the dome is drawn level (it still heaves and drifts) so that
the moored powership (6.6) does not swing with it.

**Interior,** following the render in silone-dome.md: a central three-storey
atrium 40 m across with a tree and ring galleries; offices, a gym and homes
in the terraces.

### 3.3 `cluster`: joined domes

Seven domes in a hexagon, one at the centre and six around it, with
centre-to-centre spacing of 130 m. Neighbouring domes are linked by 8 m-wide
bridges with a hinge at each end and a sliding joint in the middle. Each
dome is its own rigid body, so the bridges show the relative motion that
Silone's "easily joined" claim has to survive.

**Finding (simulated).** Each dome feels a Froude–Krylov horizontal wave
force on its columns as well as the vertical one. In a 5.2 m, 160 m swell
the domes tilt under 5°, but the bridge hinges swing 30–55° and the sliding
joints run past ±1.5 m into their stops. Bridges this short do not survive
that sea without much longer travel.

**Drawn (`?layout=cluster`):** the seven domes where the simulation has
them, and the twelve bridges between their seats, each an 8 m deck. The domes
are drawn level: their heave and drift show, their tilts (under 5°) do not.
Each bridge's card gives its live slide and hinge angle and the most so far,
and warns when the joint has run into its stops.

## 4. Physics

All forces are summed per tick on the engine's fixed clock, and nothing
reads frame time. The sea is the engine's deterministic water. Physics
reads only the gameplay band (cascade 0, `wsp_field`), never the
client-only cascades, so every client computes the same motion
(water.md, W10 "simulated and visual split").

### 4.1 Column buoyancy

Represent each hull's underwater part as vertical columns:

| Layout | Columns |
|---|---|
| `prime` | 10 m × 10 m grid: 100 × 60 = 6,000 columns |
| `dome` | Polar grid: 12 rings × 32 sectors = 384 columns, each sized to its area |

For each column, every tick:
1. Sample the sea height η at the column's centre, in world space after the
   hull's pose, using `water_sample` / `water_body_sample`.
2. The submerged length is s = clamp(η − z_bottom, 0, D_local).
3. The buoyant force is F = ρ g A s, acting at the column's centre at the
   middle of its submerged length.
4. Add drag relative to the water's velocity (`water_apply`'s drag terms,
   or the same formulas per column).

Sum the forces and moments into the rigid body. Record them in the
momentum ledger under `water.buoyancy` and `water.drag`, as `water_record`
does. The existing `water_submerged` handles clipped cuboids, so a column
can call it directly if that is cheaper than the closed form above.

**As built (2026-11-18).** `prime` stands on the 10 m grid, 6,000 columns, on
every device. Until that day the code used a 25 m grid of 960 columns, which
cannot see a wave shorter than 50 m; the `trade` sea's tile carries waves
down to 31 m. The finer grid costs less than the coarse one did. Because the
columns stand on a regular grid, the code adds them up as one weighted pass
over the sea's 64 × 64 tile: each texel's weight is the share of the columns
that read it. A check holds that sum to the sum over all 6,000 columns to one
part in 10⁹. The sea's sideways push is the same heights on the four walls.
One tick costs about 2 µs in a native release build, against 16 µs for the
25 m grid, and the frame times in headless Chrome did not change within the
machine's noise. The buoyancy is linearised about the design draft
(section 12), and the simulation keeps its own ledger of impulses
(`SimHull.imp`) rather than the engine's `water_record`.

### 4.2 The rigid body

Use the 6-degree-of-freedom body in `metaverse-engine/physics/sim/rigid.coax`.

**Mass and inertia** come from section 3: a box for `prime`, and a solid
cylinder with a shell dome for `dome`. Use radii of gyration
k_roll = 0.35B, k_pitch = 0.29L and k_yaw = 0.30√(L² + B²) for `prime`.

**Added mass.** Add Cₐ × displacement to the heave mass, plus matching
inertia in roll and pitch. Defaults:
- `prime`: Cₐ heave = 1.0 and Cₐ roll/pitch = 0.3.
- `dome`: Cₐ heave = 1.5 and Cₐ roll/pitch = 0.3.

Expose these on the debug panel. They stand in for frequency-dependent
radiation, which this world does not compute.

**Radiation damping.** Apply linear damping per degree of freedom, tuned to
a damping ratio of ζ = 0.08 in heave, roll and pitch at the natural periods
above. Without it, a free-decay test rings far too long.

**Sleep.** Never sleep a hull: waves keep it awake, as water.md describes
for bodies on moving water.

### 4.3 Moorings

The site's water is 1,500 m deep (section 5). The hull is held by taut
synthetic mooring lines:

| Layout | Mooring |
|---|---|
| `prime` | 16 lines, 4 per corner, fanned 30° apart, anchors 2,000 m from the fairleads |
| `dome` | 8 lines, 45° apart, anchors 1,800 m out |
| `cluster` | 8 lines per outer dome; the centre dome is held by its bridges only |

**Line model:** tension T(x) = T₀ + k₁x + k₃x³, where x is the extension
beyond the line's resting length. Use T₀ = 4 MN and k₁ = 0.05 MN/m per line
for `prime`, and a quarter of each for `dome`. Choose k₃ so that a line
reaches its 40 MN breaking load at a 150 m offset.

Check `metaverse-engine/physics/sim/pbd.coax` for an existing distance
constraint. Use it if it gives this force law; otherwise apply the law as
an external force at each fairlead. Draw each line as a straight segment
from the fairlead to the anchor, fading out with depth.

Since November 18, 2026 the lines are not this polynomial. Each is a
catenary with a length, a weight and a stiffness, laid for the site's depth,
and it can break. Section 12, "The hull on its moorings", describes them.

### 4.4 Wind and current

- **Wind:** F = ½ ρ_air C_d A U², with ρ_air = 1.225 and C_d = 1.0. A is the
  projected area normal to the wind: hull freeboard plus building
  silhouettes. For `prime` side-on, A ≈ 1,000 m × 178 m: the drawn wall
  stands 118 m over the water and the buildings 60 m over that
  (`SIM_WINDAGE`).
  - At U = 20 m/s this gives ≈ 44 MN. The moorings must hold the offset
    under 5% of depth (75 m). The sixteen catenary lines hold it at 66 m,
    with the worst line at 16 MN (section 12, "The hull on its moorings").
  - Until November 18, 2026 the area was the 50 m hull's, 1,000 m × 85 m
    (21 MN at 20 m/s). The lines were not resized when it grew: section 12
    says what they hold and what they don't.
- **Current:** 0.5 m/s by default, from the east (the North Equatorial
  Current at `guam`), with quadratic drag on the submerged area, C_d = 1.0.

### 4.5 Flooding and ballast

- **Ballast.** Each cell holds a ballast level from 0 to 1, filled to the
  cell's height. Pumps move water at 10 m³/s per pump with 8 pumps, so
  re-trimming a large load takes minutes, not seconds. Ballast mass enters
  the body's mass and centre of gravity every tick.
- **Damage.** Holing a cell floods it to the outside waterline over 60
  seconds, with an orifice flow of 4 m² × √(2gΔh). That water joins the
  mass. Expected results (test targets):
  - `prime`, one corner cell flooded: about 64,000 t added, sinkage
    ≈ 0.10 m, heel ≈ 0.06°. The monolith barely notices.
  - `dome`, one sector flooded: about 18,000 t added, heel ≈ 3.4°.
    Clearly visible.

### 4.6 Hull bending (phase S7, optional)

Real structures this long flex as waves pass under them; Japan's Mega-Float
research measured this. Keep the hull rigid for the physics. Phase S7 adds a
visual-only bending mode: the deck's vertical deflection along X, computed as
the first beam mode, driven by the difference between column buoyancy and
weight along the length. Show it exaggerated 100× in the section view.

The hull girder is the 50 m watertight hull alone: its bottom and top slabs,
the 1.5 m sea walls and the lengthwise walls about their own neutral axis,
I = 1.04 × 10⁶ m⁴ (`hull_section_i`). The upper structure's columns and
jointed deck are taken to carry no bending. A gale flexes the ends about
8–15 cm, drawn 100× exaggerated.

### 4.7 What the hull does to the water

The hull is a wall 1,000 m long in 4,000 m of water, so the sea round it is
not the open sea. The models are the engine's (`docs/metaverse/water.md`,
"Large floating bodies") and each is simple and stated.

- **At the weather walls** the waves are sent back. The walls that meet the
  sea follow its bearing (section 5.3). In the opening sea the wind blows
  from the east-north-east (70°), so the east wall takes 99% of the sea's energy travelling
  toward it and the north wall 84%. A box 600 m in beam and 25 m in draft
  passes almost nothing under it at these wave lengths (R = 0.9995), so a wave
  that meets the east wall squarely stands twice as high there. Where a
  breaking crest meets a wall it throws a burst of spray. The wind carries the
  mist up the wall and along it. The white water at a wall's foot is where
  spray came down in the last 20 seconds, and the drift carries it off the
  corners.
- **The lee** is on the far side from the bearing: west and south-west of
  the hull in the opening sea. The sea there is the part
  that bends round the hull's ends, less than a third of the open sea close
  to the west wall, and nothing breaks in it. The hull's own motion reads the
  same water (`ww_height`).
- **The wake.** The current is 0.5 m/s. The hull's Froude number is 0.005 and
  the waves it could make are 0.16 m long, so it makes no bow wave and no
  Kelvin wake. It leaves slack water behind it, a little wider than the hull
  and widening downstream, with a ragged line of gathered foam along each
  edge. The lines swing slowly (about once in two hours).
- **The discharge.** The ocean thermal plant's mixed water leaves 60 m down,
  about 8 °C colder than the surface water at Guam (the plant's cold and warm
  flows mixed, a third of the plant's temperature difference plus 0.7 °C). It
  sinks about 100 m more to its
  own level within 105 m of the outlets (the stratification, N = 0.015 /s, is
  an assumption) and drifts off with the current. From above it cannot be
  seen through 160 m of water, so only the flow overlay draws it there. Under
  the sea it is drawn as a pale stream from each of four outlets. Its water
  is clear in life: the colour is drawn so the path shows.
- **The moonpools** surge like pistons. The water column in each has a
  natural period of 10.7 s. The waves' pressure 25 m down drives it and the
  loss at the mouth holds it.

Left out: the waves the hull's own motion makes, the wind's shadow behind the
hull, spray landing on the deck.

## 5. Sites and sea states

### 5.1 Sites (`?site=`)

Sites change the sea, the water temperatures for OTEC (section 6), the sky
and the label in the HUD. The OTEC report ranks Guam first among U.S. sites,
so `guam` is the default.

| Site | Where | OTEC rank (otec.md §6) | Surface water over the year | Storms |
|---|---|---|---|---|
| `guam` (default) | West of Guam, about 13.4°N, steep western bathymetry | 1st U.S. site | **27.7–29.2 °C** (historical monthly values, sourced) | Typhoons |
| `puerto-rico` | Off Yabucoa, south-east Puerto Rico, about 18.0°N; 1,045 m depth reached by about 4.7 km of pipe | 2nd | 26.5–29.0 °C (planning; the report says winter minima need verifying) | Hurricanes, earthquakes |
| `kona` | West of Hawaiʻi Island at NELHA, about 19.7°N | 3rd; the best research base | 24.0–27.0 °C (planning; the report says the winter margin is narrow) | Rare hurricanes |
| `equator` | Equatorial Pacific near Baker and Howland Islands (U.S. Minor Outlying Islands), about 0.2°N | Not in the report's atlas | 28.0–29.5 °C (planning) | None: Silone's hurricane-free belt |
| `california` | 3.5 nautical miles off California (the "three miles" framing), San Clemente Basin | Excluded for OTEC | **15.2 °C in February to 20.9 °C in September** (NOAA climatology, sourced) | Winter storms |
| `texas` | The Gulf of Mexico, 40 nautical miles off the Port Aransas jetties (27.55°N, 96.37°W): the berth the timelapse builds at | Not in the report's atlas | **20.5 °C in February to 29.9 °C in August** (NASA JPL MUR monthly means, 2015–2024, sourced) | Hurricanes, June to November |
| `fjord` | Yrkefjorden, Norway (59.42°N, 5.78°E): a sheltered fjord, a place to join a hull and not to run one | None | **5.2 °C in March to 16.3 °C in August** (the same source) | Sheltered |

**Each site is a real place on a chart** (`metaverse/slab-where.coax`, on the
engine's chart module, `docs/metaverse/chart.md`). The coast is Natural Earth
1:10m and the depths are NOAA ETOPO1, both public domain. The world reads the
water's depth, the distance to land and the legal zone from the chart, at the
site's own place or wherever the visitor moves the platform:

| Site | Water depth | Nearest land | The state's line |
|---|---|---|---|
| `guam` | 1,408 m | 2.3 nautical miles | 3 nautical miles |
| `puerto-rico` | 1,440 m | 2.6 | 9 |
| `kona` | 1,725 m | 3.2 | 3 |
| `equator` | 1,448 m | 2.9 (Baker Island, a marine monument) | none: federal waters |
| `california` | 607 m | 3.5 | 3 |
| `texas` | 89 m | 39.8 (40 from the jetties) | 9 |
| `fjord` | 134 m | 0.5 | not drawn: Norwegian internal waters |

Past the state's line the territorial sea ends at 12 nautical miles, the
contiguous zone at 24 and the exclusive economic zone at 200.

**No cold water, no ocean thermal plant.** The cold-water pipes hang 1,000 m
(section 6.1). Where the water is shallower than that (`SIM_COLD_DEPTH`),
the world hangs no pipes and the plant makes nothing, and says so:
`california` (607 m), `texas` (89 m) and `fjord` (134 m). Before the sites
had real depths, `california` showed up to 19 MW in September; the surface
is warm enough then, but there is no cold water under it.

**Temperatures at the plant.** OTEC works from the temperatures at the plant's
inlets, not the sea surface (otec.md §6). The world uses:
- Deep water at 1,000 m: 5.0 °C at every site. This is the report's
  illustrative value; label it "planning value" in the HUD.
- Cold-pipe warming: 0.5 °C on the way up the pipe (a parameter, `?warming=`).
- Plant ΔT = surface temperature − (5.0 + 0.5) °C.

**Seasons.** Surface temperature follows a sine through the site's range
over the world's year (section 8). It is coldest in February, and every
site except `equator` is in the northern hemisphere. This gives:

| Site | Plant ΔT over the year | Net output over the year (section 6.1) |
|---|---|---|
| `guam` | 22.2–23.7 °C | 95–116 MW |
| `puerto-rico` | 21.0–23.5 °C | 80–113 MW |
| `kona` | 18.5–21.5 °C | 50–86 MW |
| `equator` | 22.5–24.0 °C | 99–120 MW |
| `california` | 9.7–15.4 °C | 0: the water is 607 m deep, too shallow for the pipes |
| `texas` | 15.0–24.4 °C | 0: the water is 89 m deep |
| `fjord` | −0.3–10.8 °C | 0: too cold, and 134 m deep |

The HUD labels each site's temperatures "sourced" or "planning value", as
in the first table. When better site data arrives, change only this section
and section 9's test 7.

### 5.2 Sea states (`?sea=`)

The engine's spectral sea is driven by wind speed. Map each sea state to
the wind that gives its significant wave height under Pierson–Moskowitz,
Hs ≈ 0.21U²/g:

| Sea | Hs | Wind U | Default for |
|---|---|---|---|
| `calm` | 0.5 m | 4.8 m/s | — |
| `trade` | 1.5 m | 8.4 m/s | `guam`, `puerto-rico`, `kona`, `equator` |
| `equator-max` | 2.8 m (a 5.2 m / 17 ft maximum wave) | 11.4 m/s | Silone's design case |
| `rough` | 4.0 m | 13.7 m/s | `california`, and the opening view |
| `gale` | 10 m | 21.6 m/s | `california` extreme |
| `hurricane` | 14 m | 25.6 m/s | The storm case at `guam` (a typhoon) and `puerto-rico`. The cost models in otec.md assume hurricane-free moorings, so this is where the world tests what they leave out |

**The opening sea.** The world opens on `rough` at Guam in June, for parity
with the concept image's breaking sea. That is not Guam's ordinary sea.
Guam's default is `trade` (1.5 m), and choosing Guam from the site button or
with `?site=guam` sets it. This sea was named `winter` until November 18,
2026. A 4 m sea is California's winter, but it is no season at Guam, so it
is now `rough`: 4 m is the top of the World Meteorological Organization's
"rough" band. `?sea=winter` still opens it.

Check the spectral sea's working range: the water lab's default wind is
7–8 m/s. If `gale` and `hurricane` break the tiles (patch size, cascades or
foam), extend `metaverse/water-spectrum.coax` with its own checks. Do not
clamp silently.

### 5.3 Where the sea comes from (`?winddir=`)

The wind and the waves come from one bearing, in degrees clockwise from
north, as a forecast gives it: a south-westerly comes from 225. The world's
+x is east and +z is north. The bearing turns the spectral sea (its waves
travel the other way), the wind and wave forces on the hull, which walls meet
the sea and which lie in its lee, the foam's drift, and the words on the
dock's card and in the "What would it take?" panel. `?winddir=DEGREES` sets
it at any site and in any sea; without it each site has its normal bearing
(`apps/openagents-web/slab-bearing.coax`, `slab_site_from`):

| Site | Bearing | From | Source (the direction is the source's; the round number is ours) |
|---|---|---|---|
| `guam` | 70° | east-north-east | The trade winds, easterly to east-north-easterly most of the year (NOAA National Weather Service Guam climate summaries) |
| `puerto-rico` | 80° | east | The Caribbean's easterly trades (NWS San Juan climate) |
| `kona` | 60° | north-east | Hawaii's north-easterly trades; Kona's own shore lies in the island's lee of them, which the world does not model (Western Regional Climate Center, Hawaii) |
| `equator` | 100° | east | The south-east trades reach the line as easterlies (standard trade-wind climatology) |
| `california` | 300° | west-north-west | The Southern California Bight's prevailing north-westerlies and its winter swell from the west and north-west (NOAA climatology) |
| `texas` | 135° | south-east | The western Gulf's prevailing south-easterlies (NWS Corpus Christi climate) |
| `fjord` | 225° | south-west | Western Norway's prevailing south-westerlies; the fjord's sides turn them, which the world does not model |

**The opening uses Guam's trades, 70°.** The opening camera stands
north-east of the hull and looks at its north and east walls, so the sea
meets the walls the visitor sees: the east wall squarely and the north wall
aslant (a cosine of 0.34). In Guam's wet season, July to October, the
monsoon trough brings surges and swell from the south-west:
`?winddir=225` shows that sea. Then the south and west walls take it, the
opening view looks at the hull's lee, and the dock's basin, in the south
wall, is exposed.

A change of bearing (or of wind) builds the sea again and starts the walls'
water afresh, as a world opened in that sea would have it.

The sea surface must reach the horizon with detail around the hull. The
ocean clipmap already covers 3 km in detail and runs a skirt to 40 km, so
the 1 km hull fits.

## 6. Systems on board

Every system is a small, deterministic model ticking on the fixed clock,
with its numbers shown on the HUD's systems panel. Figures follow
[otec.md](otec.md). Each section cites the part of the report it uses, and
those are models and calculations, not measured plant results. The systems
panel always shows one line: "No OTEC plant has delivered more than about
0.1 MW net. These figures are engineering models."

### 6.1 OTEC power plant

Closed-cycle ammonia plant along the west end (columns A–C), with 4
cold-water pipes:

| Parameter | Value at the design ΔT of 24 °C | Source |
|---|---|---|
| Gross output | 171.4 MW | otec.md §2: 120 MW net with pumping at 30% of gross |
| Parasitic load (seawater and working-fluid pumps) | 51.4 MW, held fixed when ΔT changes | otec.md §2 |
| **Net output** | **120 MW** | |
| Cold-water flow | 282 m³/s | otec.md §4, scaled from a peer-reviewed 100 MW-net design (235 m³/s per 100 MW) |
| Warm-water flow | 564 m³/s, through intakes on the west wall at −20 m | Same scaling (470 m³/s per 100 MW) |
| Cold-water pipes | 4 pipes, each 6.7 m in diameter and 1,000 m long, hanging from the moonpools. This keeps flow at 2 m/s and matches the report's single 13.4 m equivalent pipe | otec.md §4 |
| Discharge | Mixed outflow at −60 m, through 4 diffusers below the hull | Ours. otec.md §9 notes that no depth is universally safe |

**Output model** (otec.md §2):

gross = 171.4 × (ΔT/24)² MW
net = max(0, gross − 51.4) MW

Here ΔT is the plant's inlet ΔT from section 5.1. This reproduces the
report's check that a 20 °C ΔT leaves 56.3% of design net output.

| ΔT | 24 °C | 23 °C | 22 °C | 21 °C | 20 °C | 19 °C | 16 °C | 12 °C |
|---|---|---|---|---|---|---|---|---|
| Net output | 120.0 MW | 106.0 MW | 92.6 MW | 79.8 MW | 67.6 MW | 56.0 MW | 24.8 MW | 0 |

**Pipe trip.** A control on the world panel trips one cold-water pipe. Cold
flow drops by a quarter, and net output drops in proportion until the pipe
is restored. The report notes that several turbines sharing one intake
don't protect against losing that intake (§7). The data halls' backup
(section 6.2) is what carries them through.

**Drawing.** Draw the pipes in the section and underwater views. When the
flow overlay is on, draw the water flows as animated arrows.

### 6.2 AI data center

The data hall district (section 7) holds 2 halls of 50 MW IT each, 100 MW
IT in total: the two halls the deck draws and the card names. (This spec
first said four halls of 25 MW; the total is unchanged.) PUE is 1.20 by default, the report's baseline, giving **120 MW**.

**Seawater cooling,** following otec.md §7:
- Turn it on with `?swac=1` or from the world panel.
- A split flow of about 3 m³/s is taken from the cold intake *before* the
  OTEC condenser and goes to the halls' heat exchangers. A closed fresh-water
  loop keeps seawater out of the halls.
- PUE drops to 1.10, so the campus draws **110 MW**. That saves 10 MW, or
  87.6 GWh a year.
- To reject 120 MW at a 10 °C rise takes 2.93 m³/s, about 1.0% of the cold
  flow. Show this ratio on the panel.
- Taking that share before the condenser costs the plant about 1% of its net
  output. Apply it.

**Backup.** Each hall has 15 minutes of batteries. There is also a 40 MW
generator plant that starts within 60 seconds, enough for the critical IT
load. This is ours, following the report's point that critical loads need
independent backup. A hall drops out only when both its share of OTEC
power and the backup are gone.

The world runs this (`sim_backup` in `metaverse/slab-sim.coax`). When the
plant gives the halls less than they draw, the batteries carry the gap at
once and the generators take up to 40 MW of it after 60 seconds. Fifteen
minutes of the world's clock after the halls went short, the batteries are
spent: what the plant and the generators cannot carry goes dark, and the
halls' lights dim by that share. "What it is doing now" says which of the
three states the halls are in, the battery minutes left and the megawatts
dark. With OTEC alone off Guam the gap is under 40 MW, so the generators
carry it and nothing goes dark; off California the plant gives almost
nothing, and 80 MW of the 120 go dark. When the power is back the halls are
whole at once: the recharge is not modelled.

Halls are lit when running.

### 6.3 Bitcoin mining yard

**Layout:** 50 immersion-cooled 40 ft containers of 1 MW each (12.2 × 2.4 ×
2.9 m), in 5 rows of 10 on the east deck (section 7). Facility PUE is 1.05.

**Hashrate:** at a fleet efficiency of 15 J/TH (a parameter, `?jth=`),
50 MW is 3.17 EH/s.

**Revenue,** using the formula in otec.md §8:

R = 6B(S + f)(1 − p) / (e · u · H)

R is in $ per facility MWh. Defaults are Luxor's November 16, 2026
snapshot:

| Symbol | Meaning | Default |
|---|---|---|
| B | Bitcoin price | $65,118 |
| H | Network hashrate | 967 EH/s |
| f | Fees | 0.0229 BTC/block |
| S | Block subsidy | 3.125 BTC |
| p | Pool fee | 2% |
| e | Fleet efficiency | 15 J/TH |
| u | Facility PUE | 1.05 |

These give **$79/MWh**. After hardware ($40.27/MWh) and other operating
costs ($8/MWh), only **$30.68/MWh** is left to pay for power.

The panel shows hashrate, power, revenue per MWh and that power budget,
labelled with the snapshot's date. All inputs can be edited. A "halving"
toggle halves S, which cuts revenue to 50.3% at these inputs.

**Dispatch order:**
1. Data center.
2. Desalination.
3. Industry.
4. Mining, which takes the remaining power, in 1 MW steps.

When OTEC output falls, miners shut off first, container by container. Their
lights go out so you can see it happen. The report's conclusion (§8) is the
design: mining is a flexible buyer of leftover power, not the anchor
customer.

### 6.4 Desalination and industry

| Load | Size | Power |
|---|---|---|
| Reverse-osmosis desalination | 100,000 m³/day | 3.5 kWh/m³ ≈ 15 MW |
| Industry (metal processing, refining and chemical units, as named in the post) | Placeholder | Total of 20 MW, 3 units with smokestacks |

The industry units have no process simulation in this world.

### 6.5 Power balance

Net OTEC output must cover all loads; there is no grid.

| Load | Power |
|---|---|
| Data center | 120 MW (110 MW with seawater cooling) |
| Desalination | 15 MW |
| Industry | 20 MW |
| Hotel load (lighting, housing, pumps) | 8 MW |
| **Fixed loads** | **163 MW** (153 MW with seawater cooling) |
| Mining | Takes whatever is left |

Even at its best (120 MW net), a 120 MW-net plant can't carry these loads,
and at `guam` it delivers 95–116 MW over the year. The dispatcher sheds
mining, then industry, then desalination, and the panel goes amber. This is
intentional: it matches the report's point that the 100 MW IT illustration
needs more than 120 MW of delivered capacity *plus* resilience (otec.md §10,
phase 4). It is no longer the state the world opens in (section 6.7): open
it with `?power=otec`.

**`?otec=` scale** (0.05 to 3.0) multiplies the plant: gross, parasitic
load, flows and pipe count. Show the extra pipes the larger plant needs.
`?otec=1.75` at `guam` covers the fixed loads all year (95.3 × 1.75 = 167 MW in the coldest month, with 7 pipes).

**`?plant=demo10` preset:** the report's 10 MW commercial demonstration
(otec.md §10). It sets the scale so that net output is 10 MW at the site's
best ΔT, and replaces the campus with the report's example allocation:

| Use | Load |
|---|---|
| 3 MW IT at PUE 1.15 | 3.45 MW |
| Utility, industry and water | 4.00 MW |
| Mining | Up to 1.00 MW, curtailable |
| Uncommitted margin | 1.55 MW |

This preset shows what the report actually recommends building first.

### 6.6 Nuclear option (`?power=`)

The power source can be switched with `?power=` or from the world panel:

| Option | Plant |
|---|---|
| `otec` | OTEC only, as above. The default for the `dome` and `cluster` layouts |
| `nuclear` | 4 nuclear modules; no OTEC power plant. The deep-water intake is kept only if seawater cooling is on |
| `hybrid` | 2 nuclear modules plus OTEC at `?otec=` scale. The default for `prime` (section 6.7) |

The modules follow Ocean Atomics' video. Label the module "Ocean
Atomics-type module, company's stated figures" on the systems panel.

**Per module:**

| Parameter | Value | Source |
|---|---|---|
| Electrical output | 50 MWe gross | The video |
| Thermal power | 160 MWt (31% efficient) | The video |
| House load (pumps and controls) | 2 MWe, so **48 MW net** | Ours, a planning value |
| Compartment | 18 m long, 12.5 m high, 14 m wide | The video, except the width, which is ours |
| Turbine room | 25 m long, aft of the compartment | Ours. The video says only "to the turbine room aft" |
| Condenser seawater | 110 MW of waste heat ÷ (4.1 MJ/m³·K × 10 K) = **2.7 m³/s**, through hull-side intakes, discharged at −30 m | Derived from the video's figures |
| Availability | Runs at full power except during a refuelling outage | Ours. The video says only "years between refuellings" |

**Placement in `prime`.** The four modules sit below deck in cells D3, D10,
G3 and G10, inside the industry district. Each 50 m cell holds the 18 m
compartment and its 25 m turbine room.
- **On deck above each module:** a 30 m fenced exclusion zone with a
  hatch. Draw the compartment, containment, shielding tank and turbine in
  the section view, following the video's diagram.
- **Hull minimum.** The video says modules need a hull of at least 500 ft
  (152 m). The `dome` and `cluster` layouts are smaller (122 m), so for
  them, `nuclear` and `hybrid` add a separate 160 m powership. It is moored
  300 m away and connected by a power cable, like the video's "Powership"
  scene. The HUD says why. **Drawn:** a 160 × 32 m hull drawing 8 m, its
  bridge aft, four turbine halls with their stacks, and the cable from its
  side to the west dome's rim (`slab_powership`). The cable is drawn above
  the water so it can be seen and clicked; a real one would hang below the
  surface. The ship is drawn at rest: its own motion is not simulated.

**Power:**

| Option | Net power | Against fixed loads |
|---|---|---|
| `nuclear` | 4 × 48 = 192 MW at every site and in every month, including `california` | Covers the 163 MW of fixed loads, leaving 29 MW for mining |
| `hybrid` at `guam` | 96 MW nuclear + 95–116 MW OTEC = 191–212 MW | Covers them with room to spare |

**Refuelling outage.** A "refuel a module" control takes one module
offline. With `nuclear`, net output drops to 144 MW, so mining goes to zero
and industry sheds 19 MW. This is the case for adding a fifth module, set
with `?modules=` (1–6).

**What the world does not model:** reactor physics, accidents, emergency
cooling, fuel handling or radiation. A module is a steady power source
with an outage switch.

### 6.7 The opening power state

Decided November 18, 2026, for
[issue 13683](https://github.com/OpenAgentsInc/openagents/issues/13683).

**`prime` opens on `hybrid`:** the report's 120 MW ocean thermal plant plus
two modules. At `guam` that is 96 MW from the modules and 95–116 MW from
OTEC, 191–212 MW over the year against 163 MW of fixed loads. Every fixed
load runs in every month, 28 to 48 of the 50 mining containers run on what
is left, and the industry's stacks smoke as the concept image's do.

Why not the other two options that cover the loads:

| Option | Why not the opening state |
|---|---|
| `?otec=1.75` (167–203 MW at `guam`, 7 pipes) | It makes the panel calm by scaling up the least proven part. No OTEC plant has delivered more than about 0.1 MW net, and the report's own illustration stops at 120 MW. Opening on a 210 MW plant would hide the report's main finding behind a larger number |
| `nuclear` (192 MW) | It removes OTEC, which is the owner's direction for this world and most of what the deck, the pipes and the flow overlay show |

`hybrid` keeps the plant at the size the report studies and says what fills
the gap. Both sources are still unproven at this size: the OTEC figures are
engineering models, and the module's figures are the company's own. The
panel says both.

**The shortfall stays reachable.** It is the lesson, not the first
impression:

- `?power=otec` opens the same city on the 120 MW plant alone: 110.5 MW in
  June against 163 MW. Mining, industry and desalination switch off, the
  stacks stop smoking and the data halls go to backup.
- The power control steps to it (hybrid, the 10 MW demonstration, OTEC,
  nuclear).
- `?site=california` shows the other half: in February OTEC gives nothing
  and the two modules' 96 MW can't carry the city either.

**The domes open on `otec`,** as before. They are under the 152 m hull a
module needs, so `hybrid` there adds a powership and draws the dome level
beside it. A dome should open rolling on the swell, alone. A power picked
in the address or with the control is kept when the layout changes; the
opening power is applied only while nothing has been picked
(`sim_opening_power` in `metaverse/slab-sim.coax`).

**Amber means a fixed load is short.** The panel's loads line used to turn
amber whenever fewer than 50 mining containers ran. Mining is the flexible
buyer of what is left (section 6.3), so 43 of 50 is the design working. The
line now turns amber only when the data halls are on backup or desalination
or industry is shed (`sim_fixed_short`). `SimPower.shed` keeps its meaning
for the dispatch tests.

## 7. Deck layout (`prime`)

The deck is laid out after the concept image: industry at one end, towers
in the middle, gardens at the other. Coordinates are X from −500 to 500 and
Z from −300 to 300.

| District | X | Z | Contents |
|---|---|---|---|
| OTEC plant | −480 to −95 | 170 to 265, and the west end | Three turbine halls along the north wall, the power block, 4 moonpools with their pipes, ammonia tanks |
| Industry | −480 to −232 | −270 to 140 | Process units, a desalination plant, a tank farm, and 8 stacks of 70–95 m (the picture's row of chimneys; the tallest smoke) |
| Towers | −290 to 205 | −165 to 40 | 23 glass towers of 85–205 m, each with its own tone; those of 150 m or more step back to 0.7 for their top fifth |
| Canal and parterre | −90 to 230 | 40 to 280 | Five temple fronts along the canal's south bank, a canal pool lined with cypresses, a domed rotunda with a portico, and two parterres with a long pool |
| Gardens and temples | 230 to 500 | −165 to 300 | The great temple (a full peristyle of columns, steps and pediments), two smaller temples, a round tholos, cypress groves, and a fountain plaza |
| Data halls | 40 to 195 | −270 to −196 | 2 halls of 26 and 32 m, cooling, tanks and two chimneys |
| Mining yard | 160 to 345 | −280 to −243 | 5 rows of containers |
| Medical | 262 to 335 | −240 to −170 | One hospital block |
| Civic | 348 to 468 | −268 to −172 | A colonnaded hall |

The cranes are gone: the picture shows none. Their card, which nothing
could open, went with them, and the plan keeps the crane's kind number
(`SLAB_KIND_CRANE`) and the shader its paint. The dock notch (section 3.1)
is cut (#13686): a basin 100 m long and 60 m in, its floor 8 m under the
water, in the sheltered south wall, on any hull at least 400 m long and
300 m wide. Its name is pinned on the wall from the sea side and opens a
card that says how you get on board: boats come into the basin and lifts in
its walls go up to the deck. The lifts are our assumption. The card says
whether the basin is sheltered in the sea as it is now: it is exposed when
the sea comes from within 70° of south, and sheltered in the picture's sea,
which comes from the east-north-east. The one data
hall that stood over the basin moves back from its quay. The basin takes
48,000 m³ out of the hull's displacement at 25 m of draft (about 49,200 t
of lift, 0.3%), which `metaverse/hull.coax`'s ledger does not count yet.
Temples and the tholos are traced as distance fields in the shader
(`slab_temple_sd`, `slab_tholos_sd`), and cypresses as cones, so the
colonnades have real columns.

**Buildings** are generated by code from simple masses. Painted Village's
architecture kit is the model for this. No imported models. The budget is
under 400 draw groups on desktop and under 150 on phone, using instancing
for repeated containers, tanks and trees.

**Streets:** 20 m wide on a 100 m grid, aligned with the hull's cells. This
keeps building weight over the bulkheads.

## 8. Play

**Arrival.** The camera starts 1.5 km out, low over the water, and pushes
in toward the north-east corner (the concept image's view). It then hands
control to the player on the gardens promenade.

**Moving around:**
- **Walk.** Use the existing character controller (as `fivelab` worlds do).
  The character stands and walks in the deck's local frame: its position is
  stored relative to the hull and drawn through the hull's pose, so the deck
  can move under it. Walking is 1.4 m/s; running is 5 m/s.
- **Fly.** F toggles a free camera, with a maximum of 60 m/s and an altitude
  cap of 2,000 m.
- **Travel.** Number keys 1 to 9 jump to stations: the temples, OTEC plant,
  data halls, mining yard, towers, fountain, the bow, the concept image's
  view, and the 1.5 km view. Each station's camera has its subject in view:
  the plant is seen from the sea to its west, and the data halls and the
  mining yard from the north wall, so no tower stands in front of them.

**Opening view.** The plain address opens on the whole platform: the concept
image's bearing (yaw 0.802), pitch 0.42, 1,330 m out, aimed 100 m over the
waterline, so the skyline and both near walls are in view at 1440 × 900 and
at 390 × 844 (`TOUR_OPEN` in `slab-tour.coax`). The concept image's own
camera (yaw 0.802, pitch 0.438, 1,164 m out, aimed at the waterline) crops
the towers as the picture does; it is the tour's first stop and the station
"The view in Luckey's picture". The world opens in the `rough` sea
(4 m; section 5.2 says why, and that Guam's own sea is `trade`), in June
at 17:24 off Guam: the sun at about 291°, 13° up. From the opening camera
(which looks south-west) the sun stands to the left and a little ahead: it
lights the long north wall at a glancing angle, leaves the east end wall in
shade so the block reads as a solid, and lays shadows about four times as
long as things are tall across the deck toward the camera's right. June is
the month because the sun then sets furthest north, which is what lights the
north wall; an earlier hour lifts the sun and shortens the shadows, and a
later one (the first choice was 17:51, 7° up) puts the whole garden in the
towers' shadow. The painterly style's golden-hour grade lifts the exposure
and adds sepia; the photoreal style exposes for the deck's pale concrete and
warms the camera's white balance under a low sun (photoreal.md). The spray at the walls is computed from the waves that meet them
(section 4.7), so in this 4 m sea it is small against a 118 m wall: bursts a
few metres high, about three every two seconds round the hull. The picture's
plumes, about 36 m, need a sea between the `gale` and the `hurricane`
(section 12).

**Views:**

| View | Key | Shows |
|---|---|---|
| Section | V | Cuts the platform on a plane through the player, showing the 50 m hull's cells, the ballast's level (blue, at the ledger's depth), flooded cells (red), the waterline, the open upper structure with its columns and girders, and the cold-water pipes |
| Underwater | U | Uses the engine's underwater rendering (water.md W7) to show the hull bottom, moorings and pipes |
| Flow overlay | L | Warm, cold and discharge flows, and power lines |

**The HUD** is drawn by the browser from the components kit in the mono skin,
so it looks like `/chat`: sentence case, the kit's 14 px body size as its
smallest text, and 44 px targets for a finger. It is a bar at the top with a
sheet under it (at the foot of a phone), over the full-window scene. A menu
lists its choices; no control steps through values unseen.

| In the bar | Holds |
|---|---|
| Tour, Explore, Build it | The ways to be in the world. Tour is the guided tour below. Build it is the construction timelapse, and its sheet holds the day, the step under way, play and pause, a place in the film, and the first tow, the first join and fitting out |
| Look | Orbit, walk or fly (not offered to a finger, which orbits and pinches); closer and farther; a list of the nine stations |
| See inside | Cut it open, go underwater, water and power; "What it is doing now" holds the sea, the hull's motion and the power balance in plain words |
| Change | Where it floats, the sea, the hour and the month, stop the clock, the power source, sea-water cooling, the shape; "Break something" holds the pipe trip, the refuelling, the holed cell and the halving |
| Help | How to get around, the keys, and links to this spec and the reports |

The keys that stepped through the sites, seas, powers and layouts (N, M, P,
K) now open Change. `?hud=0` and H hide the HUD; the canvas-drawn panels
remain only as the fallback when the page has no pane.

The HUD is three parts. `components/kit/scene.coax` is the kit's scene set
(bar, sheet, card, caption, tags, the tag that says where a number is
from), with its rules in `mono_scene_css`. `apps/openagents-web/scene-pane.coax`
is the pane any world can use, and `scene-hud.coax` is how a world says what
its HUD holds. `apps/openagents-web/slab-hud.coax` is SLAB's own content; its
first lines say how another file adds a control, a menu entry or a panel.

**Time.** The world runs a day in 24 minutes, sun included. A world year of
6 hours drives each site's seasonal swing in surface temperature (section
5.1). `?t=` freezes a moment, as in the water lab; `?month=` holds a month.

**On-screen label.** The bottom-left corner always shows: "A reconstruction.
SLAB's real design is unpublished." Help opens with the full sentence: the
sizes and the systems are our assumptions, worked out from the picture.

**The tour.** A first visit by the plain address opens on a guided tour of
seven stops (`apps/openagents-web/slab-tour.coax`). Each stop is one
sentence and one number with a comparison, and the tag that says where the
number is from. The camera flies a track of
`metaverse-engine/cam/track.coax` to each stop. A stop moves on by itself
after about fifteen seconds; Next, Back and "Leave the tour" are always
there, a tap on the scene or the right arrow goes on, the left arrow goes
back, and Escape leaves. Pressing anything inside a stop makes it wait.

| Stop | Shows | Its number |
|---|---|---|
| 1 | Luckey's concept picture, fading to the world from the camera fitted to it. The caption says in full that this is our reconstruction | 1,000 years, the life he claims (from Luckey) |
| 2 | The platform from the north, with Shell's Prelude (488 × 74 m), a 400 m container ship and a 100 m tower drawn beside it as outlines, then taken away | 118 m of wall over the sea: a 35-storey building |
| 3 | The section view, with its parts named | 15.4 million tonnes: about 150 of the largest aircraft carriers |
| 4 | The sea from under the hull in the survey view (section 12), the mooring lines and their anchors drawn in from the hull's own rig. The sea is drawn clearer than it is, and the caption says so | The depth the hull's moorings are laid in (1,500 m): about 4 Empire State Buildings |
| 5 | The flow view from above | The megawatts made now: homes at 1.2 kW each |
| 6 | The construction timelapse, the whole film in twenty seconds | About 5 years; the Channel Tunnel took six |
| 7 | The platform far off, the chart of the place beside the caption (a press on it moves the platform) and a menu of the seven sites | How far the place is from the nearest land, and whether the wall would show from the beach (from a source: the chart) |

It ends on "Your turn", with Explore, "Watch it built" and "See the tour
again". The pane keeps one flag in the viewer's browser (`localStorage`,
read and written inside try/catch), so a returning visitor lands in
Explore; without storage the tour simply opens again. A link that places
the view, holds a moment, opens a cutaway, another layout or the build view
goes straight there. `?tour=0` never opens the tour, `?tour=1` to `?tour=8`
opens at that stop, and `&hold=SECONDS` holds the stop at that second, for
a picture. On a tall screen the caption covers the lower half, so the
tour's camera tips down and its subject stands in the upper half.

The first-screen card of `apps/openagents-web/slab-intro.coax` was a stopgap
for the tour. The canvas HUD still draws it when the page has no pane; the
pane never shows it.

**Names in the scene** (`apps/openagents-web/slab-labels.coax`). The pane
draws names over the canvas in the kit's body size:

- The nine districts' names over the deck while the camera is more than
  400 m over it, fading out by 220 m. A district that holds a station is a
  button that takes the camera there, and so are the fountain and the bow.
- In the section view: the deck, the open structure and its 93 m, the hull
  and its 50 m, the waterline, the ballast and its depth, a cell, a
  bulkhead, the keel, and "Empty" on empty cells. The figures are the
  ledger's.
- In the flow view: each flow and each load with its megawatts.

No two names overlap: each takes its own place or the nearest clear row
above or below it, and one with no clear place is left out.

**Cards** (`apps/openagents-web/slab-cards.coax`). A click or a tap on
anything opens its card, and every card has one pattern:

1. What it is, in one plain sentence.
2. Why it is here.
3. Three numbers. Each has a comparison a person can picture and a tag:
   From Luckey, From a source or Our assumption.
4. What it is doing now: live figures from the world, each tagged, and a
   plain sentence where nothing about the thing is simulated.
5. "The maths": the formulas and the working, shut until asked for.
6. Two ways on: a view that shows the thing (the cut, the underwater view,
   the water and power lines, a station, or the Change sheet) and this
   spec's section as a link.

A comparison is worked in the code from the number and the measure it
names (a home's 1.2 kW, a storey's 3.4 m, a football pitch, an Olympic
pool's 2,500 m³, a 100,000 t aircraft carrier), and a check holds each
one's arithmetic. The hull's figures are the ledger's (section 3.1), for
the hull the settings give. A term is explained where it first appears:
OTEC on the plant's cards, PUE on the data halls', GM in the hull's maths.

The sea's card says what the spray does, honestly: the highest spray the
chosen sea throws up the wall (section 12), that the picture's 36 m plumes
need waves near 11 m, and the lee's figures. The data halls' card says
which of the backup's states the halls are in (section 6.2).

`?card=` opens the world on a card: `sea`, `hull`, `dock`, `pipe`,
`plant`, `moonpool`, `ammonia`, `industry`, `stack`, `tank`, `water`,
`data`, `cooling`, `tower`, `mining`, `medical`, `grove`, `temple`,
`round-temple`, `hall`, `rotunda`, `canal`, `pool`, `fountain`, `statues`,
and with `?layout=` `dome`, `bridge`, `ship` and `cable`. A card asked for
by the address is a deep link, so no tour starts over it.

The canvas's own card (`slab_card` in `slab.coax`) remains for a page with
no pane, and is still in the old words.

**Narrow screens and touch.** At 390 px the bar wraps to two rows and a
sheet opens at the foot of the screen, so the platform stays in view above
it. Under 760 px the sheet, a card and the tour's caption stand in a
column over the foot's line, however many lines that line takes, so none
of them covers another. A card that opens shuts the sheet. One finger drags as the mouse does; two fingers pinch to zoom as the
wheel does (flying has no zoom), and lifting after a pinch is not a tap.

## 9. Tests

Write tests as `#test` checks next to the code, run with
`coax/bin/coax test --library <file>`, as in water.md. Tolerances allow for
the column approximation.

| # | Test | Pass condition |
|---|---|---|
| 1 | Calm equilibrium | `prime` settles at T = 25.0 m ± 1% (`dome`: 12.0 m ± 1%), zero heel and trim ± 0.01° |
| 2 | Heave free decay | Release 1 m above equilibrium in calm water: the measured period is within 15% of section 3's period at the configured Cₐ, and the decay matches ζ = 0.08 ± 0.03 |
| 3 | Roll free decay | Release at 0.5° (`prime`) or 3° (`dome`): period within 15% of section 3 |
| 4 | Wave averaging | In `trade`, `prime`'s significant heave is less than 10% of the sea's Hs, and `dome`'s is greater than `prime`'s. Record the values; do not tune to pass |
| 5 | Damage | Holing cell A1 on `prime` gives 0.10 m ± 0.03 sinkage and 0.06° ± 0.03 heel; one `dome` sector gives 3.4° ± 0.7 heel. Both stay afloat |
| 6 | Mooring | Steady 20 m/s beam wind with no waves on `prime`: offset under 75 m and peak line tension under 40 MN |
| 7 | OTEC model | Net output at every ΔT in section 6.1's table matches to ± 0.5 MW. At ΔT 20 °C, net is 56.3% ± 0.2% of design (otec.md §2). Each site's yearly range matches section 5.1's table to ± 1 MW. `california` gives 0 in February |
| 8 | Dispatch | At `guam` with `?otec=1`, shedding goes mining, then industry, then desalination. The data center draws OTEC power whenever net output is at least its load, and switches to backup otherwise. With `?otec=1.75` nothing sheds in any month. With `?plant=demo10`, the loads match section 6.5's table |
| 9 | Determinism | Two runs from the same start give identical hull poses bit for bit at tick 10,000; the second is handed to a late joiner at tick 5,000, who gets the same pose |
| 10 | Ledger | Over 600 s in a `trade` sea, the momentum ledger balances: buoyancy, gravity, damping, drag, mooring, wind, current and the sea's push sum to the body's change in momentum |
| 11 | Mining revenue | With the default inputs, R = $79.1 ± 0.5/MWh and the power budget is $30.7 ± 0.5/MWh. The halving toggle gives 50.3% ± 0.5% of R |
| 12 | Pipe trip | Tripping one pipe cuts net output by 25% ± 2%. The data halls stay up on backup through the 60 s generator start, with no gap |
| 13 | Nuclear dispatch | With `?power=nuclear` at `california`, net is 192 MW in every month and mining runs 29 containers. "Refuel a module" gives 144 MW net, with mining at 0 and industry shedding 19 MW. `hybrid` at `guam` gives 191–212 MW over the year |
| 14 | Module figures | Each module reports 50 MWe gross, 48 MW net, 31.25% efficiency and 2.68 ± 0.05 m³/s of condenser flow. In `dome`, choosing `nuclear` adds the separate powership, not modules inside the dome |

**Performance targets** (measure with `frametime.ts`, as water.md does):
- Physics (6,000 columns) under 1.5 ms of CPU a tick on desktop and on
  phone. The phone uses the same 10 m grid: the weighted pass (section 4.1)
  costs the same for any grid, so the 20 m phone grid this spec first asked
  for is not needed.
- Desktop at 1440×900, DPR 2: GPU under 12 ms.
- Phone tier at 390×844: GPU under 8 ms.

## 10. Build phases

| Phase | Delivers | Done when |
|---|---|---|
| S1 Shell | Realm row, page, sea from the water lab, static `prime` hull and deck slab, fly camera, golden | `/worlds/slab` opens, the release walk passes, the golden is committed |
| S2 Hull physics | `metaverse/hull.coax`: columns, rigid body, added mass, damping, moorings, wind and current; motion panel; site and sea selectors | Tests 1–4, 6, 9 and 10 pass |
| S3 Deck | `metaverse/slab-plan.coax` districts and generated buildings; walking in the deck frame; travel stations | Walk the deck in `gale` without the character sliding off or jittering; draw budgets hold |
| S4 Systems | OTEC, data halls and backup, seawater cooling, mining and revenue, desalination, industry, dispatch, `demo10` preset, nuclear and hybrid options with module placement and the powership, systems panel, flow overlay, pipes | Tests 7, 8, 11–14 pass; `california` shows OTEC at zero in winter and nuclear at full power |
| S5 Domes | `dome` and `cluster` layouts, the Silone interior, bridges | Test 3 (dome) passes; bridges show relative motion in `equator-max` |
| S6 Events and views | Ballast transfer, damage, section view, underwater view | Test 5 passes |
| S7 Bending | Visual-only hull bending mode | Visible in the section view in `gale`, with no physics change |

**Each phase:**
1. Commit to `main` with an `Agent:` line, as the pre-push check requires
   (see `docs/metaverse/green-main-pipeline.md`).
2. Add the phase's tests.
3. Update this spec's status line.
4. Record measured numbers (periods, offsets, frame times) in a results
   section at the end of this file. Keep the targets above unchanged, so
   results and targets can be compared.

## 11. Out of scope

- Real SLAB data. If SLAB publishes real dimensions, a site or a power
  system, update section 1 and the affected numbers. Keep this world's
  version history.
- Frequency-domain hydrodynamics: diffraction and radiation, and any
  second-order drift beyond the wall estimate in section 12.
- Concrete fatigue, cracking, or the 1,000-year lifespan claim itself.
- Process simulation for industry, hospital operations, a market, or
  multiplayer economics.
- Construction of the domes (the airform), until there is a reason to
  animate it.

## 12. Results

### What would it take? (#13686)

One panel under **Change** (`apps/openagents-web/slab-panel.coax`), drawn
with the components kit in the mono skin. Its settings and their limits are
`metaverse/slab-whatif.coax`; what they do to the world is
`apps/openagents-web/slab-change.coax`. Every hull figure is
`metaverse/hull.coax`'s ledger.

| Part | Controls (range) |
|---|---|
| The hull | Buildings and equipment (0–6 Mt), length (100–2,000 m), width (50–1,000 m), depth keel to deck (30–150 m), sea walls and bottom slab (0.5–3 m), the draft the ballast holds (2–60 m) |
| Building it | Concrete in place ($100–$1,000 a cubic metre), casting yards (1–8), each yard's pour (1,000–6,000 m³ a day) |
| Where | The chart (press it to move the platform), distance from shore (0–250 nautical miles) |
| Weather | Wind (0–40 m/s), month. It also says which way the sea comes from, which walls face it and which side is sheltered |
| Power | Ocean thermal plant (30–240 MW), reactor modules (1–8), and each tenant on or off with its size: data halls, desalination, industry, miners |
| Design life | 25–1,000 years |

- Each slider shows its two ends in words under its line and says why it
  stops at each (`k_slider_ends` in the kit, `sh_limits` in the HUD's text).
- Each figure is tagged "From Luckey", "From a source" or "Our assumption".
- The reasons a setting can't work lead the panel, for example "The hull is
  wider than the Corpus Christi channel (162 m), so it leaves as 120
  caissons and is joined at sea." and "No ocean thermal power here: under
  607 m of water there is no room for the pipes."
- Presets: the picture, the Gulf Coast pilot (100 × 50 m at 5 m draft, off
  Texas), joined in a Norwegian fjord, three miles off California, Guam on
  ocean thermal power, one Silone dome. **Reset to the picture** returns.
- Every state has an address. A press writes the settings that differ from
  the picture's into the address bar: `?preset=`, `?weight=`, `?long=`,
  `?wide=`, `?deep=`, `?wall=`, `?floor=`, `?draft=`, `?price=`, `?yards=`,
  `?pour=`, `?lat=`, `?lon=`, `?shore=`, `?data=`, `?desal=`, `?industry=`,
  `?mining=` (0 is off), `?life=`, beside `?site=`, `?power=`, `?layout=`,
  `?wind=`, `?month=`, `?otec=` and `?modules=`.

What the world does with them, each checked in `slab.coax`:

| Setting | The world |
|---|---|
| The picture's hull with 6 Mt on it | draws 27 m, two more than the picture: it carries far more than its buildings |
| A 400 × 300 m hull with 6 Mt on it (`?long=400&wide=300&weight=6`) | weighs 9.6 Mt, floats 6.1 Mt, and goes down until its deck is awash |
| The pilot (`?preset=pilot`) | 23,100 t of concrete, ballasted to 5 m; it leaves the 162 m wide, 16.5 m deep Corpus Christi channel in one piece |
| The picture's hull | 162 m is narrower than its 600 m, so it leaves as 120 caissons |
| California (`?preset=california`) | 607 m of water: no pipes, and the ocean thermal plant makes 0 MW |
| The concrete alone, at $500 a cubic metre | $1.9 billion, 1,276 pour-days in one yard |

A resized hull is drawn at its size with the picture's buildings scaled to
its deck. Walking, the cut-open view and the flow lines are for the
picture's hull only and say so. The hull's motion is still the picture
hull's: a smaller hull would move more than the world shows, and the panel
says that too.

**The chart.** `metaverse-engine/world/chart.coax` is the engine's chart:
coast, heights, distance to land, level lines, a view and a skyline
(`docs/metaverse/chart.md` has its API). `apps/openagents-web/slab-chart.coax`
draws it for this world: the coast, the 200 m and 1,000 m depth lines, the
state's line and the 12, 24 and 200 mile lines, the platform, and off Texas
the construction route (the casting yard at Corpus Christi, the Port Aransas
jetties, the berth). Freight loads at Ingleside too. `chart_draw_in(s, out, n, view)` draws it
into any rectangle and `chart_route_tags` names the route, for the tour's
last stop and the construction drawing (#13682).

**The place.** The nearest land is drawn when it is in sight: its shore on
the sea at the chart's distance and bearing, and its ridge over the horizon
as high and as wide as the chart's heights make it from the eye
(`slab-place.coax`, `slab_land` and `slab_shore` in the shader). The ridge's
small rises are noise. Only the nearest shore stands at its real distance:
where land is all round, as in the fjord, the rest stands on the horizon. The
state's line and the 12 mile line are dashed on the water where they pass
within 16 km, each with its name.

**The timelapse builds the picture's platform at its Gulf berth**, the
`texas` site, 40 nautical miles off the Port Aransas jetties. Entering the
build view says so, and leaving it puts the world off Texas, where the
build ended. For the construction work (#13682): the chart puts that berth
in 89 m of water, not the 40 m `BUILD_SITE_DEPTH` assumes; 40 m of water is
about 22 nautical miles out. Either the berth moves in or the depth
changes.

Measured on November 18, 2026, from the checks in `metaverse/slab-sim.coax`,
`metaverse/slab-plan.coax` and `apps/openagents-web/slab.coax`. The targets
in sections 3–9 are unchanged.

| Quantity | Target | Measured |
|---|---|---|
| Displacement | 15.375 Mt | 15.375 Mt; the simulation's mass is the ledger's to the kilogram |
| Hull ledger (section 3.1) | closes at 25 m with ballast left and positive GM | 3,826,744 m³ of concrete (9.57 Mt), 3.31 Mt of ballast, KG 53.5 m; the hull's card, the section's ballast line and the simulation give the same figures (checked) |
| Timelapse and world | the same hull | the timelapse ends at 143 m of deck over the keel, 25.0 m of draft, 3,826,744 m³ of concrete, 3.31 Mt of ballast and 15.375 Mt, the ledger's figures (checked) |
| Transverse and longitudinal GM | 1,159 m and 3,292 m | 1,159.0 m and 3,292.3 m |
| Heave period (Cₐ = 1) | 14.2 s | 14.2 s; free decay rings at it within 15%, ζ within 0.03 of 0.08 |
| Roll and pitch periods | 12.4 s and 10.1 s without added inertia | 12.4 s and 10.1 s; with Cₐ 0.3 on rotation the body rings at 14.1 s and 11.6 s |
| Calm equilibrium (test 1) | design draft, no heel or trim | heave under 0.25 m, heel and trim under 0.01° |
| Short sea (test 4) | averaged away | a 1.5 m, 100 m wave moves the hull under 0.15 m |
| 20 m/s beam wind (test 6) | within 5% of depth | 43.6 MN of wind on the drawn wall's 178 m wind height, 66 m of offset, 16.1 MN in the peak line of a 40 MN breaking load. (On the earlier 85 m wind height and polynomial line: 20.8 MN, 69 m and 8.0 MN.) |
| Buoyancy grid (section 4.1) | 6,000 columns on 10 m, under 1.5 ms a tick | 6,000 columns; about 2 µs a tick in a native release build (16 µs on the earlier 25 m grid) before the hull turned and hung on catenaries. With them a tick is 25 µs for a hull square to the tile (the sixteen lines' solutions), 41 µs turned (each column reads the tile) and 54 µs on the weighted sea with the drift, and the three weighted tiles cost 75 µs a frame. Headless Chrome, 10 s on the `trade` sea, frames drawn before and after: 3,295 and 3,058 at 1440 × 900, 12,726 and 12,531 at 390 × 844 as a phone, with other builds loading the machine; not timed on a real phone |
| Determinism (test 9) | the same pose bit for bit at tick 10,000 | two runs on a moving 1.5 m sea agree in every bit, the second through a late joiner at tick 5,000; at tick 10,000 the hull has moved 2.91 m east and 3.41 m north and turned 0.06° |
| Momentum ledger (test 10) | balances over 600 s in `trade` | up: lift +9.04968 × 10¹³ N·s, weight −9.04970 × 10¹³, heave damping +9.5 × 10⁶, which sum to the hull's −1.085 × 10⁸ within 15 N·s. North: wind +2.991 × 10⁹, moorings −3.240 × 10⁹, drag −1.72 × 10⁸, the sea's push +8.9 × 10⁵, the current on the turned hull +3 × 10⁵, which sum to the hull's −4.19 × 10⁸. East, with the current's −1.153 × 10⁹, likewise. About the vertical: wind −9.60 × 10¹⁰ N·m·s, moorings +8.75 × 10¹⁰, the sea's push −1.28 × 10¹⁰, drag −4 × 10⁸, which sum to the hull's −2.18 × 10¹⁰ |
| OTEC net at ΔT 24, 20 and 12 °C (test 7) | 120, 67.6 and 0 MW | the same within 0.5 MW; Guam 95–116 MW over the year |
| Dispatch (tests 8 and 13) | sheds mining, then industry, then desalination | as targeted; nuclear 192 MW with 29 MW of miners, 144 MW refuelling, hybrid at Guam 191–212 MW |
| Pipe trip (test 12) | net down a quarter | 0.75 of net |
| Cell A1 holed on `prime` (test 5) | about 64,000 t, sinkage ≈ 0.10 m, heel ≈ 0.06° | 64,000–66,000 m³ in, sinkage and heel within 0.03 of the targets; pumping half the floodwater to the opposite corner takes heel and trim back under 0.005° |
| Hull bending in `gale` (S7) | visible ×100 in the section | the first mode's ends flex 0.08–0.15 m over a minute of the gale (EI = 3.6 × 10¹⁶ N·m², the 50 m hull girder), 8–15 m as drawn |
| `demo10` preset | 10 MW net at the site's best | 10 MW at its best; IT 3.45, utility 4.0 and mining 1.0 MW leave a 1.55 MW margin |
| Frame time | 60 FPS | 60 FPS, p95 16.7 ms, at 1440×900 (dpr 1 and 2) and on a 390×844 phone, from `realm-perf` on an Apple M-series GPU; the section and underwater views were checked by eye, not timed |

**Departures from the spec, and why:**
- The sea's heights are scaled to Pierson–Moskowitz (Hs = 0.21 U²/g), so each
  sea state shows the Hs in section 5: the water lab's fetch-limited spectrum
  at 300 km of fetch grows past a fully developed sea (3.4 m at 8.4 m/s).
- The spray is not painted to match the picture. It is thrown where and when
  a breaking crest meets a wall (section 4.7), and the table below is what
  that gives. The picture's plumes stand about 36 m up the wall. The model
  reaches that only between the `gale` and the `hurricane` (a significant
  height of about 11 m). In deep water nothing makes the waves plunge at the
  wall as they do on a shallow breakwater, so a 4 m sea throws spray about
  12 m at most.

**The water at the walls, measured** (ten minutes of each sea on the `prime`
hull; the wind sea in the hull's own wave band; `coax/bin/coax test
metaverse/water-wall.coax`):

| Sea | Bursts in 10 min | Highest run-up | Highest spray | Fastest jet |
|---|---|---|---|---|
| `calm` (4.8 m/s) | 0 | 0.9 m | none | none |
| `trade` (8.4 m/s) | 48 | 3.0 m | 4.9 m | 6.1 m/s |
| `equator-max` (11.4 m/s) | 337 | 5.2 m | 8.6 m | 8.4 m/s |
| `rough` (13.7 m/s) | 950 | 7.5 m | 12.4 m | 10.1 m/s |
| `gale` (21.6 m/s) | 4,061 | 17.9 m | 28.7 m | 15.0 m/s |
| `hurricane` (25.6 m/s) | 6,446 | 30.1 m | 44.1 m | 17.0 m/s |

Run-up is about 2.0 to 2.4 times the significant height and the spray's top
about 3.2 to 3.5 times. The spray's top is the heaviest drops' height; mist
goes higher on the wind that turns up the wall.

**The lee, measured** (root-mean-square height against the open sea's, in
the `rough` sea and the `gale`): 0.24 to 0.31 at 60 m off the west wall;
about 2.0 to 2.2 at the east wall, where the standing wave is; 0.46 to 0.59
in the dock notch in the south wall (section 3.1; the model's notch is the
drawn one, 50 m either side of the middle and 60 m in). The south wall is only 20° off the waves' path,
so the notch sees about half the open sea. A notch in the west wall would see
under a third.
- The arrival view and the default hour follow the concept image: January at
  17:18, with the sun low in the west-south-west, seen from off the
  north-east corner.
- With `otec` power at Guam, the dispatcher sheds industry, so the stacks
  don't smoke. The world therefore opens on `hybrid` (section 6.7): supply
  206.5 MW in June against 163 MW, every fixed load carried, 43 of 50
  mining containers on, and the stacks smoking as in the concept image.

- Column buoyancy is linearised about the design draft: each column adds the
  sea's rise over it times its area. Columns do not clip at the keel or the
  hull's top, so a sea that would wash over the 25 m hull top is not modelled.
- Test 9 shows that the same hull and the same sea give the same step in
  every bit. The world steps the hull on each frame's sea tile, so two
  browsers at different frame rates see the sea at different instants and do
  not hold the same pose. Nothing shares a hull between browsers yet.
- The wind area is still the 50 m hull's (section 4.4).
- A holed cell's 4 m² orifice takes about 24 minutes to fill 64,000 m³, so
  the spec's 60 s is not consistent with its orifice law. The world keeps the
  orifice law and runs the flood and the pumps 24 times faster than real
  time, so the cell fills in about a minute.
- Ballast is drawn as one level across the inner bottom (−17.9 m, 3.31 Mt);
  the pumps are modelled as moving half the floodwater's mass from the
  damaged corner to the opposite one, which balances its moments.
- The flow overlay's key is L, since O already returns to the orbit. Its
  dashes move about ten times faster than the water, so the flow reads.
- The underwater key is U, since Q and E already zoom and climb. The section
  cuts through the orbit's point, the walker, or where the fly camera looks,
  and cuts the sea away in front of it.
- Under water a diver sees about 23 m at Guam (Jerlov type IA), and the hull
  is 1,000 m long. The underwater view therefore opens as a survey: the water
  is drawn about 64 times clearer than it is, with a fill light, and the foot
  line says so. "Show the water as it is" under See inside draws the real
  water. The mooring lines are drawn 1.2 m wide.

### The sea's look, the weather and the view under the sea

Measured on November 18, 2026, from the checks in
`metaverse-engine/render/shade/water-optics.coax`, `metaverse/water-sea.coax`
and `apps/openagents-web/slab-sea.coax`, and from pictures of the six sea
states. Issue 13688 asked for these. `docs/metaverse/water.md` ("The open sea
by its optics") has the method and its sources.

| Quantity | Measured |
|---|---|
| The sea's colour | the sky mirrored by Fresnel's mean share, plus the water's own reflectance by Jerlov type (IA at Guam, coastal 3 on the Texas shelf, coastal 5 in the fjord); no tint. The site's water type is our reading of Jerlov's map |
| Slopes drawn | three cascades on a desktop (patches of 1,245, 235 and 44 m in the 4 m sea), one and a baked tile on a phone; the rest of Cox and Munk's mean square slope is the glint's roughness |
| The hull's sea | cascade 0 is the same bit for bit with one, two or three cascades drawn (checked), so the heights the hull samples are the heights drawn |
| Whitecap cover | Monahan's, from the wind: 0.08% at 4.8 m/s, 0.6% at 8.4, 2.9% at 13.7, 14% at 21.6 and 24% at 25.6 m/s; broken at 2 to 6 m on the long waves' crests |
| Streaks and spindrift | streaks from 13.9 m/s, full at 20.8 m/s; spindrift from 20.8 m/s (the Beaufort scale's descriptions) |
| Weather by the sea | clear to `rough`; rain in `gale`; a storm in `hurricane` with a cloud deck, rain, lightning and about 7 km of visibility (our planning value). Overcast in a strong breeze in the site's storm season |
| A diver's sight | 24 m at Kona, 23 m at Guam, 16 m off California, 10 m on the Texas shelf, 7 m in the fjord (3.9 beam attenuation lengths; the beam attenuation is our planning ratio to K_d) |
| The view under the sea | survey view 1,500 m of sight, stated on screen; the hull's bottom, the cold-water pipes, the mooring lines, Snell's window and the bed at Texas (89 m) and the fjord (134 m) seen in pictures |
| Frame time, GPU p50 (p95) | 1440×900: opening view 2.34 (3.33) ms, was 2.06 (2.25); hurricane 3.95 (6.73) ms, was 2.20 (3.74); under the sea 1.37 (2.02) ms. 390×844 phone tier: 0.62 (2.39) ms, was 0.40 (0.44). Headless Chrome on an Apple M-series GPU, on a machine busy with other builds |

**Not done, or not physics.**

- In the default style the sky the sea mirrors is the realm's painted sky.
  With `?style=photoreal` it is the physical sky, and the weather dims that
  frame's sun, sky and air too. In both the sea's optics still work between a
  square and a square root of the display's colours, not yet in the photoreal
  frame's own light and film response.
- Foam is where the sea's own Jacobian breaks, but its lace, the streaks'
  places and the spindrift are drawn from noise, not simulated.
- Rain does not flatten the sea or ring its surface, and the cloud deck is a
  texture with no shadow of its own on the water.
- Under the sea the hull casts its shadow on the bed, not in the water: the
  sun's shafts do not stop under the hull.

### The hull on its moorings

Measured on November 18, 2026, from the checks in
`metaverse-engine/physics/moor.coax`, `metaverse/slab-moor.coax`,
`metaverse/slab-drift.coax`, `metaverse/water-hull.coax` and
`metaverse/slab-station.coax` (H1 to H5, and tests 9 and 10 run on the
moorings). Issue 13688 asked for these.

**What the hull's step now does.**

- **Yaw.** The hull turns about the vertical. Wind and current that meet a
  wall at an angle push nearer the end they reach first (Rayleigh's
  free-streamline plate: the centre of pressure sits ¾ L cos α / (4 + π sin α)
  from the middle), and each mooring line pulls on its own fairlead, so
  unequal tensions turn the hull too. Its inertia is the ledger's mass at a
  radius of gyration of 0.30 of the diagonal, plus the water a flat strip as
  deep as the draft carries (5% more).
- **Waves by their length.** `water-hull.coax` makes three tiles each tick
  from the spectral sea's gameplay band, with every wave weighted by its own
  wavenumber k against the 25 m draft T: the lift at the keel (e^−kT), the
  head summed over the wall ((1 − e^−kT)/k) and the part the hull turns back
  (√(1 − e^−2kT), Wiegel's power-transmission estimate). A 400 m swell keeps
  68% of its lift and a 60 m chop 7%. The hull sums the tiles where it
  floats: the lift over its 6,000 columns, the push round its waterline in
  10 m pieces. Crests and troughs still cancel along the hull in the sum
  itself.
- **Slow drift.** A wall that turns back a wave of amplitude a takes a mean
  push of ½ ρ g a² for each metre (Maruo), and in an irregular sea the push
  follows the square of the wave envelope (Newman's approximation). Each
  metre of weather wall takes ρ g × (the turned-back height)² × cos² of the
  angle between the waves and its normal, and the step keeps the slow part
  (a 10 s low-pass).
- **Moorings.** Each line is an elastic catenary on a flat seabed (Irvine;
  Jonkman, NREL/TP-500-41958), solved for the pull at its fairlead. The
  tension, the anchor's uplift, the length on the seabed and the drawn shape
  come from the one solution. In 400 m of water or more the lines are taut
  polyester legs from the keel (5.9 N/m in water and a stiffness of 15 times
  the breaking load, for a 40 MN rope), cut to give 4 MN at rest, with the
  anchors as far out in proportion as 2,000 m is at 1,500 m. In shallower
  water they are 220 mm chains from the waterline (8.2 kN/m in water), long
  enough that no anchor is pulled upward at the breaking load. A line parts
  at its breaking load and stays parted.
- **Added mass and damping.** These are stand-ins and nothing more. They
  differ by motion: heave keeps the spec's Cₐ = 1, roll and pitch 0.3, and
  surge and sway 0.1 (a flat strip as deep as the draft gives 4% end on and
  7% broadside). Yaw adds the water that strip carries along the hull's
  length and beam, 5% of its own inertia. Heave, roll and pitch are damped at
  ζ = 0.08. Surge, sway and yaw have the cross-flow drag of each pair of
  faces on the hull's speed through the current (C_d = 1) and a linear term
  of 20 MN for each m/s, about ζ = 0.08 on the moorings.
- **The ledger.** `SimHull.imp` names the drift, the current's push on a
  turned hull's long walls and the five moments about the vertical beside
  the forces it had, and test 10 runs on both seas. A hull square to the
  sea's tile sums its 6,000 columns as weights on the tile; a turned hull
  reads the tile under each column.

| Quantity | Measured |
|---|---|
| Lines at 1,500 m (`prime`) | 16 taut legs of 2,471 m, anchors 2,000 m out, each anchor pulled upward 2 MN at rest |
| Lines at California, 607 m | 16 taut legs of 974 m, anchors 789 m out |
| Lines off Texas, 89 m | 16 chains of 1,076 m, anchors 1,058 m out, 796 m of each on the seabed at rest, no uplift |
| 20 m/s beam wind, no waves (test 6 and H4) | 43.6 MN: 66 m off its place at 1,500 m with the worst line at 16.2 MN; 14 m off Texas, worst line 16.2 MN, 490 m of each chain still on the seabed |
| 25.6 m/s wind from the south-west, no waves (H1) | 80 MN: 108 m off its place and turned 1.1°, the worst line at 24 MN. The wind's own moment is 1.9 GN·m toward broadside; the unequal lines turn it the other way with 5.6 GN·m until the two balance. Mirrored, it turns the other way by as much. End on, it doesn't turn |
| `gale`, as the page runs it (H3) | the hull turns back a sea 8.0 m high of the gale's 10 m. Drift 17 MN on top of the wind's 46 MN: 72 m off its place where wind and current alone leave it 51 m, and the worst line at 18.0 MN instead of 13.7 |
| `hurricane`, as the page runs it (H5) | wind 65 MN and drift 33 MN: 113 m off its place, turned 1.5°, the worst line at 26.4 MN of 40. One line lost: it comes to rest 146 m off and turned 3.1° with the worst line at 30.5 MN, swinging out to 163 m and 33.5 MN on the way. Two lost: 190 m, 7.3°, 35.7 MN at rest. Three lost: a fourth is over 40 MN at rest and parts on its own |
| `hurricane` square on the long wall (H5; not a sea the page runs) | wind 72 MN and drift 60 MN: the worst of the sixteen lines is at 44 MN at rest, over its breaking load |
| A line lost in the gale (H5) | the hull comes to rest 95 m off its place instead of 72 m, turned 2.2° instead of 0.9°, with another line the most loaded at 21.0 MN |
| Heave on the gale's sea (H2) | less with each wave faded by its length than with the same sea lifting the hull unfaded; both are centimetres |
| Determinism (test 9) | two runs of 10,000 ticks in `winter`, with a line lost at tick 4,000, end in the same hull bit for bit, and a hull handed over at tick 10,000 steps on the same |
| Ledger (test 10) | over 600 s of `trade` from rest, each step's change in velocity is that step's summed force over the mass it moves, and the yaw and heave ledgers close to one part in a million |

**On the page.** `apps/openagents-web/slab-lines.coax` runs this in the
world. Each frame it makes the three weighted tiles once, puts the hull at
rest when the place, the layout, the wind or the depth changes
(`sim_hull_settle`), and then steps it. The hull's pose carries its turn, so
the camera, the walker, the drawing and the cards follow the turned hull.

- **See inside**, **What holds it in place** says the layout the depth gives
  and shows the lines holding, the offset, the turn, the
  hardest-pulled line against its breaking load, the wind's push, the waves'
  slow push, and the hardest upward pull on an anchor or the chain still on
  the seabed. **Lose a line** breaks the most loaded line and **Fit new
  lines** puts them back. A line that reaches its breaking load parts on its
  own and the HUD says so. `?lose=N` opens the world with N lines broken.
- The hull's card gives the same layout, the turn, both pushes and the lines
  broken, and says what is not modelled.
- Under the sea each line is drawn as four straight pieces through points of
  the mooring model's own curve (`sim_line_packed`): the anchor, where the
  line leaves the seabed, two points up the hanging part and the fairlead.
  They are the last 132 floats of the uniforms (the seabed's height, then 8
  floats a line, in the world's frame), which brings the block to 4,083 of
  the 4,096 vec4s a uniform block may hold on every device. A broken line
  lies on the seabed from its anchor.
- **What the hull samples.** The hull feels the spectral sea's waves at its
  place, each weighted by its length against the draft, in the sea the hull
  changes round itself. `metaverse/slab-shelter.coax` takes the wall and lee
  models' sea (`wall_water`, the page's `ww_height`) and hands it to the
  force model as a gain under each buoyancy column and at each piece of
  waterline. Under the hull a wave's pressure fades from the weather side to
  the lee side. At a weather wall the wave and what the wall sends back
  stand in step, and at a lee wall the sea is calmer. A cluster's domes
  shelter each other the same way. The gains are laid again when the wall
  and lee models are (a new wind, or the hull has moved 8 m or turned). The
  slow drift is not scaled: it already counts the waves the hull turns back.
  These are stand-ins for a diffraction analysis: one reflection coefficient
  for the whole sea and a planning model of the lee.

| On the page | Measured |
|---|---|
| A line lost in the gale | at the break the worst of the fifteen is at 16.4 MN. Two minutes on the hull is 3 m further off and turned 0.3° more, with the worst line at 16.9 MN. It comes to rest 22 m further off, turned 2.1°, at 21.0 MN. 15 million tonnes on these lines swing in about 20 minutes, so the move takes minutes, and the HUD says so |
| The hull's step (native, release) | 25 µs square to the sea's tile, 41 µs turned, 54 µs on the weighted tiles with drift; the tiles themselves 75 µs a frame |
| The gale with the hull in its own shelter, against the open sea (ten minutes, native) | heave 0.08 m rms against 0.11 m, roll 0.040° against 0.055°, pitch 0.033° against 0.042°. Each wave's push 203 MN rms along the hull and 94 MN across it, against 140 and 69 MN: the east and north walls stand in 1.97 and 1.68 times the open sea, the west wall in 0.32. The worst line 19.2 MN against 19.0 MN |
| The shelter's cost | 5 µs more a step (native, release); the gains are laid only when the wall and lee models are |

**Departures, and what this does not model:**
- The sixteen 40 MN lines are the spec's, sized for a 20 m/s wind on a 25 m
  wall. They pass test 6 on the drawn wall, but they have no storm margin.
  In the page's hurricane the worst line is at two thirds of its breaking
  load and the hull is 113 m off its place, 7.5% of the depth. Square on the
  long wall the same storm would part them. A mooring designed to offshore
  practice keeps its worst line under 60% of breaking in its design storm
  with every line intact, which here needs about twice the lines or twice
  their strength. That sizing is open. The numbers are as the model gives
  them; nothing was tuned.
- The drift force is an estimate from the wall's draft and the waves the
  hull turns back. It leaves out the hull's own motion, the waves the hull
  sends out and any change to the sea round the hull.
- A flat plate of `prime`'s size carries far more water in heave than
  Cₐ = 1: about 6.8 times the hull's mass (π/8 × B/T, less a quarter for its
  ends), and about 1.7 and 2.5 times in roll and pitch. With those the heave
  period would be about 28 s, not 14.2 s. Sections 3 and 4.2 keep the spec's
  values, so the periods on the hull's card are the spec's, not a
  hydrodynamic result.
- The lines are in equilibrium at every instant: they have no inertia or
  drag of their own, the seabed is flat and has no friction, and a rope's
  stiffness doesn't change with load.
- The hull is rigid and its added mass doesn't couple its motions (no Munk
  moment).


### The construction, simulated

Measured on November 18, 2026, from the checks in
`metaverse/slab-build.coax`, `metaverse/slab-build-view.coax` and
`metaverse-engine/physics/afloat.coax`. Issue 13682 asked for these (its
phases 0 to 2). The method, with every number marked sourced or assumed, is
in [slab-construction-method.md](slab-construction-method.md).

`slab-build.coax` is a seeded simulation of the build in three-hour steps:
six casting cells sharing one concrete supply, pairs of caissons towed from
Ingleside by tugs on towlines, joins at sea that wait for a calm, the
sea-wall lining, the upper structure rising bay by bay, barges and lifts
for the city, storm stand-downs and the final ballast. It reads the hull
from `hull.coax`. The timelapse on the page still draws the earlier
schedule in `slab-construction.coax`; drawing this simulation is the
issue's later phases.

| Result (seed 1, default settings) | Value |
| --- | --- |
| Days from the first pour to commissioned | 1,510 (4.1 years) |
| Placement days at 3,000 m³ a day | 1,275.58 for this hull's 3,826,744 m³; 804 for the feasibility note's 2.41 million m³ |
| Tows and joints at sea | 60 pairs of caissons, 59 joints |
| Days on which weather held some work | 314 |
| Storm stand-downs | 2, one a hurricane (6.8 m, 35.7 m/s on day 1,298) |
| A tow | 13 hours; towline 1.65 MN at worst against 2.94 MN; 4.2 m under the keel and 36 m to the channel edge at least |
| Hull-girder stress in the design storm | 9.4 MPa head on and 3.6 MPa sideways, against 10 MPa of prestress |
| Row-by-row assembly | fails: 19.1 MPa sideways |
| Least GM of anything afloat | 2.09 m (a dry caisson); the joined hull's least is 143 m |
| The end state | 15,374,950 t at 24.9999 m, level: the ledger's hull |

**The moorings need storm lines.** The sixteen lines hold the growing hull
in working weather (the finished hull moves 10.9 m, worst line 11.8 MN).
The design storm pushes the finished hull with 211 MN beam on, 139 MN of it
wind on the 143 m wall, and a line breaks at 114 MN. The simulation makes
storm lines fast at each storm warning: 20 more chains of the same kind for
the finished hull, square to its sides. With them the default run's
hurricane loads the worst chain to 16 MN. Without them a line breaks and
the run stops. This agrees with "about twice the lines" above.

**What is placed and not integrated:** moves inside the yard, a pair's
approach from stand-by to its joint, and tugs and barges on passage. The
method note lists the other limits.
