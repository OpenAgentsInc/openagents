# SLAB world: a simulation spec for the Metaverse Engine

Status: spec, November 18, 2026. Nothing here is built yet.

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
- Engine docs: [realm-host.md](../../metaverse/realm-host.md) (how a realm is
  registered), [water.md](../../metaverse/water.md) (the water physics, the
  spectral sea, the ocean clipmap and the water lab).

## 1. What is real and what is ours

SLAB has published **no** dimensions, materials, site, power system or
layout. This world is a reference design that is consistent with what has
been said and with basic naval architecture. Mark it as a reconstruction
inside the world (section 10).

| Item | Source |
|---|---|
| Seastead for heavy industry: metal processing, medical, desalination, refining, chemicals | Luckey's post |
| 1,000-year design life | Luckey's post |
| Rectangular concrete block with sheer walls, a city, towers, gardens and smokestacks on top | The AI-generated concept image (not a design) |
| Floating concrete dome of about 400 ft (122 m), airform and shotcrete, joinable modules, calm equatorial water (17 ft max wave) | Ben Silone's reply (not confirmed as SLAB) |
| Every number in sections 3 to 6 | **Ours**, chosen for this world and derived in this doc |
| OTEC as the power source | **Ours**, the owner's direction for this world |

## 2. The realm

Register one row in `apps/openagents-web/realms.json`, then run
`bun coax/web/realms.ts` to write the generated files. Do not edit them by
hand. `bun coax/web/realms.ts --check` must pass.

```json
{ "id": "slab", "name": "SLAB", "kind": "open", "listed": false,
  "orbit": { "r": 0.5, "a": 3.6, "size": 0.02 },
  "realm": { "slug": "slab", "title": "SLAB",
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
| Hull depth D | 50 m |
| Design draft T | 25 m |
| Freeboard (deck above calm water) | 25 m |
| Deck area | 600,000 m² (60 ha) |
| Corners | Rounded, 40 m radius in plan |
| Displacement at T | 1,000 × 600 × 25 × 1,025 kg/m³ = **15.4 Mt** |

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
  open to the sea, carrying the OTEC cold-water pipes (section 6.1).

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
| Roll natural period | k = 0.45r | ≈ 6.4 s |

The dome's roll period falls inside the band of common wave periods. The
world should show this as a finding rather than tune it away.

**Interior,** following the render in silone-dome.md: a central three-storey
atrium 40 m across with a tree and ring galleries; offices, a gym and homes
in the terraces.

### 3.3 `cluster`: joined domes

Seven domes in a hexagon, one at the centre and six around it, with
centre-to-centre spacing of 130 m. Neighbouring domes are linked by 8 m-wide
bridges with a hinge at each end and a sliding joint in the middle. Each
dome is its own rigid body, so the bridges show the relative motion that
Silone's "easily joined" claim has to survive.

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
- **Current:** 0.5 m/s by default, from the east (the equatorial current),
  with quadratic drag on the submerged area, C_d = 1.0.

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

## 5. Sites and sea states

### 5.1 Sites (`?site=`)

Sites change the sea, the water temperatures for OTEC (section 6), the sky
and the label in the HUD.

| Site | Where | Surface water | Water at 1,000 m | ΔT | Storms |
|---|---|---|---|---|---|
| `equator` (default) | Equatorial Pacific near Baker and Howland Islands (U.S. Minor Outlying Islands), about 0.2°N 176.5°W | 28.5 °C | 4.5 °C | **24 °C** | None (the hurricane-free belt) |
| `kona` | West of Hawaiʻi Island, about 19.7°N | 26.0 °C | 4.8 °C | 21 °C | Rare hurricanes |
| `california` | 3.5 nautical miles off California (the "three miles" framing), about 34°N | 15.0 °C | 4.3 °C | 11 °C | Winter storms |

These temperatures are planning values chosen to show the contrast between
sites. Replace them with the OTEC research's sourced figures when they
arrive (that research is the follow-up prompts set). In the HUD, label them
"planning values".

### 5.2 Sea states (`?sea=`)

The engine's spectral sea is driven by wind speed. Map each sea state to
the wind that gives its significant wave height under Pierson–Moskowitz,
Hs ≈ 0.21U²/g:

| Sea | Hs | Wind U | Default for |
|---|---|---|---|
| `calm` | 0.5 m | 4.8 m/s | — |
| `trade` | 1.5 m | 8.4 m/s | `equator`, `kona` |
| `equator-max` | 2.8 m (a 5.2 m / 17 ft maximum wave) | 11.4 m/s | Silone's design case |
| `winter` | 4.0 m | 13.7 m/s | `california` |
| `gale` | 10 m | 21.6 m/s | `california` extreme |
| `hurricane` | 14 m | 25.6 m/s | Stress test only |

Check the spectral sea's working range: the water lab's default wind is
7–8 m/s. If `gale` and `hurricane` break the tiles (patch size, cascades or
foam), extend `metaverse/water-spectrum.coax` with its own checks. Do not
clamp silently.

The sea surface must reach the horizon with detail around the hull. The
ocean clipmap already covers 3 km in detail and runs a skirt to 40 km, so
the 1 km hull fits.

## 6. Systems on board

Every system is a small, deterministic model ticking on the fixed clock,
with its numbers shown on the HUD's systems panel. These are planning
figures for this world, not engineering quotes.

### 6.1 OTEC power plant

Closed-cycle ammonia plant along the west end (columns A–C), with 4
cold-water pipes:

| Parameter | Value at design ΔT 22 °C |
|---|---|
| Gross output | 170 MW |
| Parasitic load (seawater and working-fluid pumps) | 50 MW, fixed |
| **Net output** | **120 MW** |
| Cold-water pipes | 4, each 8 m in diameter and 1,000 m long, hanging from the moonpools |
| Cold-water flow | 450 m³/s in total (≈ 2.2 m/s in each pipe) |
| Warm-water flow | 500 m³/s, through intakes on the west wall at −20 m |
| Discharge | Mixed outflow at −60 m, through 4 diffusers below the hull |

**Output model:** gross = 170 × (ΔT/22)² MW, and net = gross − 50 MW, with
a floor of 0. This gives:

| ΔT | 24 °C | 22 °C | 20 °C | 16 °C | 12 °C |
|---|---|---|---|---|---|
| Net output | ≈ 152 MW | 120 MW | 90 MW | 40 MW | ≈ 0 MW |

So the `california` site produces almost nothing, which is the point that
site makes. Add a seasonal swing of ±1 °C to ΔT over the world's year
(section 8). Draw the pipes in the section and underwater views. Draw the
water flows as animated arrows when the flow overlay is on.

### 6.2 AI data center

The data hall district (section 7) holds 4 halls of 25 MW IT each, 100 MW
IT in total, at PUE 1.15.

**Cooling:** the plant's cold effluent (≈ 9 °C after the OTEC condenser)
feeds the halls' heat exchangers through a closed fresh-water loop.
- To reject 115 MW at a 10 °C rise needs about 2.8 m³/s of seawater, under
  1% of the plant's cold flow.
- Show this ratio on the panel.

Halls are lit when running. A hall drops out if its share of power is cut.

### 6.3 Bitcoin mining yard

**Layout:** 50 immersion-cooled 40 ft containers of 1 MW each (12.2 × 2.4 ×
2.9 m), in 5 rows of 10 on the east deck (section 7).

**Hashrate:** at a fleet efficiency of 15 J/TH (a parameter, `?jth=`),
50 MW is 3.3 EH/s.

**Dispatch order:**
1. Data center.
2. Desalination.
3. Industry.
4. Mining, which takes the remaining power, in 1 MW steps.

When OTEC output falls, miners shut off first, container by container. Their
lights go out so you can see it happen.

The panel shows hashrate and power. Revenue appears only if the
viewer enters a BTC price and network hashrate; nothing is baked in.

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
| Data center | 115 MW |
| Desalination | 15 MW |
| Industry | 20 MW |
| Hotel load (lighting, housing, pumps) | 8 MW |
| **Fixed loads** | **158 MW** |
| Mining | Takes whatever is left |

At the design ΔT of 22 °C (120 MW net) the fixed loads already exceed
supply. The dispatcher then sheds mining, then industry, and the panel goes
amber. At the `equator` site's ΔT of 24 °C (152 MW) the shortfall is small.
This is intentional: the world shows that a 100 MW AI campus needs a larger
OTEC plant. Add a `?otec=` scale (1.0 to 3.0) that multiplies the plant
(gross, parasitic load and flows), and show the extra pipes it would need.

## 7. Deck layout (`prime`)

The deck is laid out after the concept image: industry at one end, towers
in the middle, gardens at the other. Coordinates are X from −500 to 500 and
Z from −300 to 300.

| District | X | Z | Contents |
|---|---|---|---|
| OTEC plant | −500 to −350 | −300 to 300 | Turbine halls, heat exchangers, 4 moonpools, ammonia tanks |
| Industry | −350 to −150 | −300 to 300 | 3 process units, 6 smokestacks 60 m tall (the concept image's stacks), tank farm, desalination plant |
| Data halls | −150 to 50 | −300 to 0 | 4 halls of 120 × 60 m, 3 storeys (18 m), with cooling yards |
| Towers | −150 to 150 | 0 to 300 | 12 towers of 80–180 m, offices and housing |
| Dock | −50 to 50 | −300 to −240 | The dock notch from section 3.1, with cranes |
| Mining yard | 150 to 300 | −300 to −100 | 50 containers |
| Medical campus | 150 to 350 | −100 to 300 | Hospital and research blocks, 4–6 storeys |
| Gardens and civic | 300 to 500 | −300 to 300 | Classical civic buildings, groves, a fountain plaza (as in the concept image) |
| Perimeter | Whole edge | | A 20 m promenade inside a 1.5 m parapet |

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
- **Travel.** Number keys 1 to 9 jump to stations: dock, OTEC plant, data
  halls, mining yard, towers, gardens, the bow, under the hull, and the
  1.5 km view.

**Views:**

| View | Key | Shows |
|---|---|---|
| Section | V | Cuts the hull on a plane through the player, showing the cells, ballast levels (blue), flooded cells (red), the waterline and the cold-water pipes |
| Underwater | Q / E | Uses the engine's underwater rendering (water.md W7) to show the hull bottom, moorings and pipes |
| Flow overlay | O | Warm, cold and discharge flows, and power lines |

**HUD panels**, which keep the engine's existing HUD look:

| Panel | Shows |
|---|---|
| Motion | Heave, roll and pitch, as now and as a 60 s trace; deck vertical acceleration at the player (with comfort bands of 0.05 g and 0.1 g); draft; trim; GM; mooring offset; peak line tension |
| Systems | ΔT; OTEC gross, parasitic and net output; each load; mining hashrate and containers on; data-hall state; desalination output; the cooling-flow ratio |
| World | Site, sea state, wind, current, time of day, layout |

**Controls** in the world panel: site, sea state, layout, time of day, the
OTEC scale, the mining efficiency, ballast transfer (select a cell and fill
or empty it), "hole a cell", "repair all" and "reset".

**Time.** The world runs a day in 24 minutes, sun included. A world year of
6 hours drives the seasonal ±1 °C swing in ΔT. `?t=` freezes a moment, as
in the water lab.

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
| 7 | OTEC model | Net output at ΔT 24, 22, 20, 16 and 12 °C matches section 6.1's table to ± 1 MW |
| 8 | Dispatch | At `equator` with `?otec=1`, the order of shedding is mining, then industry; the data center never sheds while net is at least 115 MW |
| 9 | Determinism | Two runs from the same seed give identical hull poses bit for bit at tick 10,000; a late joiner at tick 10,000 gets the same pose |
| 10 | Ledger | Over 600 s in `trade`, the momentum ledger balances: buoyancy, drag, mooring, wind and gravity sum to the body's change in momentum |

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
| S4 Systems | OTEC, data halls, mining, desalination, industry, dispatch, systems panel, flow overlay, pipes | Tests 7 and 8 pass; `california` shows the plant near zero |
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
