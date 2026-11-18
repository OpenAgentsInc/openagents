# SLAB world: a simulation spec for the Metaverse Engine

Status: phases S1–S3, S6 and S7 built and most of S4, November 18, 2026, as
**SLAB Simulation** at `/realms/slab`. The hull floats on column buoyancy
with damping, moorings, wind and current (`metaverse/slab-sim.coax`); the
`prime` deck is built and can be walked with GameBuilder's character
controller; the five sites, six sea states, OTEC, nuclear and hybrid power,
the `demo10` preset and the dispatcher run, with a motion and power panel and
a card for anything clicked. Cell A1 can be holed and the ballast pumped
against it; the section view (V) cuts the hull open and the underwater view
(U) shows the bottom, the cold-water pipes and the moorings; the flow
overlay (L) animates the plant's water and the power lines; the section draws
the hull's first bending mode 100 times exaggerated. Still to build: the
powership (S4) and the dome layouts (S5); the golden is still to be taken on
staging. Measured numbers are in section 12. The spec was
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
    "description": "An unlisted reconstruction of SLAB, a proposed floating industrial platform: walk its deck, change the sea and the site, and watch the hull, the OTEC power plant, the data halls and the miners respond.",
    "alt": "A kilometre-long concrete platform carrying a city and industrial stacks on open ocean",
    "style": "painterly", "clock": "fixed:0.5", "module": "slab", "slot": "slab",
    "golden": { "lit": 0.3, "settle": 3000, "blocks": 6 }, "assets": [] } }
```

- **Unlisted.** With `listed: false` the realm stays off `/realms` and the
  galaxy, but its page opens at `/realms/slab`.
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

## 3. Layouts

The realm has three layouts, selected with `?layout=` and from the HUD.
The default is `prime`.

### 3.1 `prime`: the monolith (default)

This is a rectangular, cellular concrete hull read from the concept image.

| Dimension | Value |
|---|---|
| Length L (X) | 1,000 m |
| Beam B (Z) | 600 m |
| Hull depth D | 143 m (was 50 m; see the note below) |
| Design draft T | 25 m |
| Freeboard (deck above calm water) | 118 m (was 25 m) |
| Deck area | 600,000 m² (60 ha) |
| Corners | Sharp, 1.5 m radius in plan (was 40 m) |
| Displacement at T | 1,000 × 600 × 25 × 1,025 kg/m³ = **15.4 Mt** |

**Following the picture (2026-11-18).** The owner's direction for the first
version is parity with the concept image, walls included, over this spec. A
camera fitted to the picture's deck corners (about 5 px RMS, a 51° horizontal
field) puts its wall about 118 m over the water on a beam of about 550 m. The
world therefore raises the deck to 118 m and squares the corners, and keeps
the 1,000 × 600 m plan, the 25 m draft and the mass budget below. The tables
in this section are still the 50 m hull's; the hull-bending section modulus
(section 4.6) and the wind area use the 143 m depth. The picture's horizon
sits higher than its slab's perspective allows, so no single camera matches
both; the world matches the slab.

**Structure.** Reinforced and prestressed concrete at 2,500 kg/m³:

| Part | Size | Volume |
|---|---|---|
| Bottom slab | 1.5 m thick | 900,000 m³ |
| Deck slab | 1.0 m thick | 600,000 m³ |
| Outer walls | 1.5 m thick, 50 m tall | 240,000 m³ |
| Longitudinal bulkheads | 11, every 50 m, 0.6 m thick | 330,000 m³ |
| Transverse bulkheads | 19, every 50 m, 0.6 m thick | 342,000 m³ |
| **Total** | | **2.41 million m³ ≈ 6.0 Mt** |

The bulkheads form **240 cells**, each 50 × 50 × 50 m, labelled by column
A–T (X, west to east) and row 1–12 (Z, south to north).

**Mass budget at the design draft:**

| Item | Mass |
|---|---|
| Hull concrete | 6.0 Mt |
| Topsides: buildings, plant, equipment | 2.0 Mt |
| Liquids: process tanks, fuel, fresh water | 0.5 Mt |
| Seawater ballast in the lower cells | 6.9 Mt |
| **Total** | **15.4 Mt** |

Ballast is the trim variable. Adding or removing topside load is balanced by
pumping ballast, so the draft stays at 25 m. The vertical centre of gravity
is KG = 25 m above the keel; the ballast sits low and the topsides sit high.

**Hydrostatics** (rectangular-barge formulas, used as test targets in
section 9):

| Quantity | Formula | Value |
|---|---|---|
| KB | T/2 | 12.5 m |
| Transverse BM | B²/(12T) | 1,200 m |
| Longitudinal BM | L²/(12T) | 3,333 m |
| Transverse GM | KB + BM − KG | ≈ 1,188 m |
| Longitudinal GM | | ≈ 3,321 m |
| Heave natural period | 2π √(T(1+Cₐ)/g) | 10.0 s with Cₐ = 0; 14.2 s with Cₐ = 1 |
| Roll natural period | 2π k / √(g·GM), k = 0.35B | ≈ 12.2 s |
| Pitch natural period | k = 0.29L | ≈ 10.1 s |

Cₐ is the added-mass coefficient. It is a parameter (section 4.2), not a
measured value.

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
high) takes on about 18,000 t and heels it 3.4°. The dome physics is built;
its rendering is not yet.

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

### 4.4 Wind and current

- **Wind:** F = ½ ρ_air C_d A U², with ρ_air = 1.225 and C_d = 1.0. A is the
  projected area normal to the wind: hull freeboard plus building
  silhouettes. For `prime` side-on, A ≈ 1,000 m × 85 m.
  - At U = 20 m/s this gives ≈ 21 MN. The moorings must hold the offset
    under 5% of depth (75 m).
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

With the 143 m hull (section 3.1) the section is far stiffer (I ≈ 9.7e6 m⁴
per the simulation): a gale flexes the ends about 1–1.5 cm, so the world
draws it 1,000× exaggerated instead.

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
| `california` | 9.7–15.4 °C | 0 in winter, at most 19 MW in September |

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
| `winter` | 4.0 m | 13.7 m/s | `california` |
| `gale` | 10 m | 21.6 m/s | `california` extreme |
| `hurricane` | 14 m | 25.6 m/s | The storm case at `guam` (a typhoon) and `puerto-rico`. The cost models in otec.md assume hurricane-free moorings, so this is where the world tests what they leave out |

Check the spectral sea's working range: the water lab's default wind is
7–8 m/s. If `gale` and `hurricane` break the tiles (patch size, cascades or
foam), extend `metaverse/water-spectrum.coax` with its own checks. Do not
clamp silently.

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

The data hall district (section 7) holds 4 halls of 25 MW IT each, 100 MW
IT in total. PUE is 1.20 by default, the report's baseline, giving **120 MW**.

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
phase 4).

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
| `otec` (default) | OTEC only, as above |
| `nuclear` | 4 nuclear modules; no OTEC power plant. The deep-water intake is kept only if seawater cooling is on |
| `hybrid` | 2 nuclear modules plus OTEC at `?otec=` scale |

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
  scene. The HUD says why.

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

The cranes are gone and the dock notch (section 3.1) stays uncut: the
picture shows neither.
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
  view, and the 1.5 km view.

**Opening view.** The world opens on the concept image's camera (yaw 0.802,
pitch 0.438, 1,164 m out, aimed at the waterline) in a winter sea, in June
at 17:51 off Guam: the sun at about 297°, 10° up, lights the long north
wall as the picture does. The golden-hour grade lifts the exposure and adds
sepia. The spray stands about nine times the significant height up the
walls, every 95 m.

**Views:**

| View | Key | Shows |
|---|---|---|
| Section | V | Cuts the hull on a plane through the player, showing the cells, ballast levels (blue), flooded cells (red), the waterline and the cold-water pipes |
| Underwater | U | Uses the engine's underwater rendering (water.md W7) to show the hull bottom, moorings and pipes |
| Flow overlay | L | Warm, cold and discharge flows, and power lines |

**HUD panels**, which keep the engine's existing HUD look:

| Panel | Shows |
|---|---|
| Motion | Heave, roll and pitch, as now and as a 60 s trace; deck vertical acceleration at the player (with comfort bands of 0.05 g and 0.1 g); draft; trim; GM; mooring offset; peak line tension |
| Systems | Surface, deep and plant ΔT (each marked sourced or planning); OTEC gross, parasitic and net output; each load; data-hall state and backup; mining hashrate, containers on, revenue per MWh and power budget; desalination output; the cooling-flow ratio; the "engineering models" line from section 6; with nuclear, each module's state (running or refuelling), its MWe and its condenser flow |
| World | Site, sea state, wind, current, time of day, layout |

**Controls** in the world panel:
- Site, sea state, layout and time of day.
- Power source (`otec`, `nuclear`, `hybrid`), module count and "refuel a
  module".
- OTEC scale, the `demo10` preset, seawater cooling, and "trip a pipe".
- Mining inputs and the halving toggle.
- Ballast transfer (select a cell and fill or empty it).
- "Hole a cell", "repair all" and "reset".

**Time.** The world runs a day in 24 minutes, sun included. A world year of
6 hours drives each site's seasonal swing in surface temperature (section
5.1). `?t=` freezes a moment, as in the water lab; `?month=` holds a month.

**On-screen label.** The bottom-left corner always shows: "Reconstruction:
SLAB's real design is unpublished. Dimensions and systems are this world's
assumptions."

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
| 9 | Determinism | Two runs from the same seed give identical hull poses bit for bit at tick 10,000; a late joiner at tick 10,000 gets the same pose |
| 10 | Ledger | Over 600 s in `trade`, the momentum ledger balances: buoyancy, drag, mooring, wind and gravity sum to the body's change in momentum |
| 11 | Mining revenue | With the default inputs, R = $79.1 ± 0.5/MWh and the power budget is $30.7 ± 0.5/MWh. The halving toggle gives 50.3% ± 0.5% of R |
| 12 | Pipe trip | Tripping one pipe cuts net output by 25% ± 2%. The data halls stay up on backup through the 60 s generator start, with no gap |
| 13 | Nuclear dispatch | With `?power=nuclear` at `california`, net is 192 MW in every month and mining runs 29 containers. "Refuel a module" gives 144 MW net, with mining at 0 and industry shedding 19 MW. `hybrid` at `guam` gives 191–212 MW over the year |
| 14 | Module figures | Each module reports 50 MWe gross, 48 MW net, 31.25% efficiency and 2.68 ± 0.05 m³/s of condenser flow. In `dome`, choosing `nuclear` adds the separate powership, not modules inside the dome |

**Performance targets** (measure with `frametime.ts`, as water.md does):
- Physics (6,000 columns) under 1.5 ms of CPU a tick on desktop. On phone,
  use a 20 m grid (1,500 columns) under 1.5 ms.
- Desktop at 1440×900, DPR 2: GPU under 12 ms.
- Phone tier at 390×844: GPU under 8 ms.

## 10. Build phases

| Phase | Delivers | Done when |
|---|---|---|
| S1 Shell | Realm row, page, sea from the water lab, static `prime` hull and deck slab, fly camera, golden | `/realms/slab` opens, the release walk passes, the golden is committed |
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
- Frequency-domain hydrodynamics: diffraction, radiation and second-order
  drift.
- Concrete fatigue, cracking, or the 1,000-year lifespan claim itself.
- Process simulation for industry, hospital operations, a market, or
  multiplayer economics.
- Construction of the domes (the airform), until there is a reason to
  animate it.

## 12. Results

Measured on November 18, 2026, from the checks in `metaverse/slab-sim.coax`,
`metaverse/slab-plan.coax` and `apps/openagents-web/slab.coax`. The targets
in sections 3–9 are unchanged.

| Quantity | Target | Measured |
|---|---|---|
| Displacement | 15.4 Mt | 15.34 Mt (the corners are rounded) |
| Transverse and longitudinal GM | ≈ 1,188 m and ≈ 3,321 m | 1,187.5 m and 3,320.8 m |
| Heave period (Cₐ = 1) | 14.2 s | 14.2 s; free decay rings at it within 15%, ζ within 0.03 of 0.08 |
| Roll and pitch periods | 12.2 s and 10.1 s without added inertia | 12.2 s and 10.1 s; with Cₐ 0.3 on rotation the body rings at 13.9 s and 11.5 s |
| Calm equilibrium (test 1) | design draft, no heel or trim | heave under 0.25 m, heel and trim under 0.01° |
| Short sea (test 4) | averaged away | a 1.5 m, 100 m wave moves the hull under 0.15 m |
| 20 m/s beam wind (test 6) | within 5% of depth | 20.8 MN of wind, offset under 75 m, the peak line under its breaking load |
| OTEC net at ΔT 24, 20 and 12 °C (test 7) | 120, 67.6 and 0 MW | the same within 0.5 MW; Guam 95–116 MW over the year |
| Dispatch (tests 8 and 13) | sheds mining, then industry, then desalination | as targeted; nuclear 192 MW with 29 MW of miners, 144 MW refuelling, hybrid at Guam 191–212 MW |
| Pipe trip (test 12) | net down a quarter | 0.75 of net |
| Cell A1 holed on `prime` (test 5) | about 64,000 t, sinkage ≈ 0.10 m, heel ≈ 0.06° | 64,000–66,000 m³ in, sinkage and heel within 0.03 of the targets; pumping half the floodwater to the opposite corner takes heel and trim back under 0.005° |
| Hull bending in `gale` (S7) | visible ×100 in the section | the first mode's ends flex 0.10–0.15 m over a minute of the gale (EI = 3.4 × 10¹⁶ N·m²), 10–15 m as drawn |
| `demo10` preset | 10 MW net at the site's best | 10 MW at its best; IT 3.45, utility 4.0 and mining 1.0 MW leave a 1.55 MW margin |
| Frame time | 60 FPS | 60 FPS, p95 16.7 ms, at 1440×900 (dpr 1 and 2) and on a 390×844 phone, from `realm-perf` on an Apple M-series GPU; the section and underwater views were checked by eye, not timed |

**Departures from the spec, and why:**
- The sea's heights are scaled to Pierson–Moskowitz (Hs = 0.21 U²/g), so each
  sea state shows the Hs in section 5: the water lab's fetch-limited spectrum
  at 300 km of fetch grows past a fully developed sea (3.4 m at 8.4 m/s).
- Spray up the windward walls reaches about four times Hs (a standing wave's
  crest at a vertical wall is about twice the highest wave's height,
  2 × 1.86 Hs, and the wind lifts the spray above it), capped at 24 m.
- The arrival view and the default hour follow the concept image: January at
  17:18, with the sun low in the west-south-west, seen from off the
  north-east corner.
- With the default `otec` power at Guam, the dispatcher sheds industry, so
  the stacks don't smoke. The concept image's smoke appears with
  `?power=hybrid` or `nuclear`.

- A holed cell's 4 m² orifice takes about 24 minutes to fill 64,000 m³, so
  the spec's 60 s is not consistent with its orifice law. The world keeps the
  orifice law and runs the flood and the pumps 24 times faster than real
  time, so the cell fills in about a minute.
- Ballast is drawn as one level across the inner bottom (−12.2 m, 6.9 Mt);
  the pumps are modelled as moving half the floodwater's mass from the
  damaged corner to the opposite one, which balances its moments.
- The flow overlay's key is L, since O already returns to the orbit. Its
  dashes move about ten times faster than the water, so the flow reads.
- The underwater key is U, since Q and E already zoom and climb. The section
  cuts through the orbit's point, the walker, or where the fly camera looks,
  and cuts the sea away in front of it.
- Under water, red light falls to 1/e in 14 m and blue in about 80 m (clear
  tropical ocean), so the 1,000 m pipes and the moorings fade within a
  hundred metres. The mooring lines are drawn 1.2 m wide.
