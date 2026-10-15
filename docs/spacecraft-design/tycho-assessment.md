# Tycho Station: an engineering assessment

Tycho Station is the mobile shipyard in *The Expanse* novels and TV series.
This note treats it as a hypothetical future station and asks two
questions:

1. What would it take to build?
2. What could it build?

It reuses the rest of this folder:
- the method and margins in [design-method.md](design-method.md);
- the rocket equation in [astrodynamics.md](astrodynamics.md);
- the subsystem sizing in [subsystems.md](subsystems.md);
- the free-floating arm relation in [in-space-assembly.md](in-space-assembly.md);
- the phased plan to a 1,000 t ship by about 2042 in
  [cislunar-strategy.md](cislunar-strategy.md);
- the autonomy and radiation rules in
  [autonomy-and-environment.md](autonomy-and-environment.md).

Marks follow the [README](README.md):
- [V] verified with a cited source;
- [C] computed by us, with the method shown;
- [M] from memory, to verify before it becomes a code constant;
- [P] a proposed assumption or planning target.

**Fiction facts are the spec, not facts.** They come from the Expanse wiki
and secondary summaries of the novels, and are cited as fiction. Where the
fiction gives no number (ring cross-section, masses, power), the number is
ours and marked [P] or [C].

## 1. Summary

**Physically plausible:**
- **The geometry.** A 0.5 km sphere does hold about 65 million m³, so the
  fiction is self-consistent. Rings 700 m across can give a full 1 g at
  1.6 rpm, under the commonly cited 2 rpm comfort limit.
- **Counter-rotating rings.** They are the right engineering answer: zero net
  spin momentum, so the station can be turned.
- **The structure.** Steel or aluminium rings carrying their own shielding at
  1 g are a solved problem in principle: a ~100 kt hoop per ring.
- **Building it from asteroid material.** It is the only way it pencils out:
  the station is mostly unprocessed rock for shielding.

**Hard:**
- **Scale of industry.** About 10 million tonnes in total, mostly shielding.
  That is about 24,000 ISS masses.
- **Power.** One to two gigawatts of continuous power, with square
  kilometres of radiators.
- **Life support.** Closed for 15,000 people for decades, which no one has
  demonstrated beyond a few crew.
- **Autonomy.** A robot-run Belt industry that grows from a few thousand
  tonnes of seed equipment. Our cislunar plan reaches 1,000 t of ship per
  year by about 2042 [P]; Tycho needs roughly 50 to 100 times that in
  finished structure per year, plus a million tonnes or more of moved rock
  per year.

**Fiction:**
- **The Epstein drive.** Sustaining 1 g with a mass ratio of 2 needs about
  18 MW of jet power per kilogram of ship. The best real fusion-rocket
  concepts project about 1 kW/kg, more than four orders of magnitude less.
- **The station moving quickly.** With any real drive, moving a 10⁷-tonne
  station between asteroids takes years. Under real fusion propulsion its
  acceleration is micro-g, so the rings would simply keep spinning; the
  swivelling compartments matter only for the fictional drive.
- **Building a 2.5 km generation ship in a few years.** That is fiction too,
  and so is a reason to build one. A real Nauvoo would need Daedalus-class
  pulsed fusion and a multi-megatonne shield, and Tycho's stated scale gives
  build times of a decade or more.

## 2. The fictional spec (paraphrased)

From the Expanse wiki's Tycho Station page and the owner's notes [fiction]:

- **The sphere.** A central manufacturing sphere about half a kilometre
  across, with more than 65 million m³ inside.
- **The rings.** Two counter-rotating habitation rings, over 700 m across,
  housing about 15,000 workers and their families. Each ring compartment
  sits on a swivel, so when the rings stop the compartments can turn to take
  thrust gravity.
- **The waldoes.** About six very large construction manipulators on top of
  the sphere.
- **The drive housing.** A projection of about 50 m at the bottom holds a
  capital-ship-class fusion reactor and drive, so the whole station can move.
- **The ships.** It builds ships up to the largest size class in the
  tabletop RPG, and structures as large as the Nauvoo generation ship. The
  Nauvoo is about 2 km long in the books; one summary gives 2,460 × 950 ×
  910 m [fiction, Tor.com].
- **Later history.** It later relocates to Pallas and builds "Void cities".
- **The drive is beyond physics.** The setting's Epstein drive gives
  sustained high-g, high-exhaust-velocity thrust that no known physics
  supports. This note says so wherever it matters.

## 3. Geometry check

### The sphere

V = (4/3) π r³ with r = 250 m gives **65.4 million m³** [C], matching the
fiction's "more than 65 million m³". Its surface is 4π r² = **785,000 m²**
[C].

### Spin for gravity

For a ring of radius r spinning at ω, the floor acceleration is ω² r. With
r = 350 m [fiction, half of 700 m] and g₀ = 9.80665 m/s²:

| Target | ω (rad/s) | rpm | Rim speed | Coriolis at 1 m/s walk (2ωv) | As share of local gravity |
| --- | --- | --- | --- | --- | --- |
| 1 g | 0.167 | **1.60** | 58.6 m/s | 0.33 m/s² | 3.4% |
| 0.3 g (Belter-comfortable) | 0.092 | **0.88** | 32.1 m/s | 0.18 m/s² | 6.2% |

All [C].

**Head-to-foot gravity gradient.** For a 2 m person it is 2/350 = **0.6%**
[C] at any spin rate.

**Comfort limits.**
- Rotation below about 2 rpm is commonly cited as acceptable to most people,
  following Globus and Hall's comfort chart [V, as summarised in arXiv
  2408.00152].
- Older estimates put the upper limit as high as 3–4 rpm [M, Hall's summary
  of Graybiel and Hill and Schnitzer].
- The 1975 Stanford torus design held itself to 1 rpm [V, NASA SP-413].

**So a 700 m ring is comfortable even at 1 g;** the fiction's diameter is
well chosen. Belters, raised in low gravity in the setting, would get 0.3 g
at under 1 rpm [C].

**Rim speed matters for operations.** A ring moving at 59 m/s past a
non-rotating sphere needs either:
- docking at the hub, on the spin axis; or
- matched-speed transfer vehicles at the rim.

Our sizing below puts traffic through the hub [P].

### Why the rings counter-rotate

A spinning ring stores angular momentum L = I ω. Using the ring mass from
section 4 (about 2 Mt with shielding), one ring at 1 g holds:

L ≈ M r_mean v ≈ 1.99 × 10⁹ kg × 338 m × 58.6 m/s ≈ **3.9 × 10¹³ N·m·s** [C].

For scale, the Lagrange 2 factory's CMGs hold 10⁶ N·m·s [V, our code
`factory::CMG_CAPACITY`]. One ring is 39 million times that.

**Turning a single spinning ring.** Its axis turns only under a torque
τ = L Ω. Turning at 1° per minute (Ω = 2.9 × 10⁻⁴ rad/s) needs about
**1.1 × 10¹⁰ N·m** [C], which no thruster set on the station could supply
economically.

**What counter-rotation buys.** With two equal rings spinning opposite ways,
the station's net spin momentum is zero. The station turns like an ordinary
rigid body, and the two rings' gyroscopic torques cancel each other.

**What it doesn't remove:**
- The cancelling torques act *through the hub and bearings*. A 1°/min turn
  still puts about 10¹⁰ N·m of internal moment into the sphere's structure.
- So the station turns slowly, or the rings spin down first.
- Spinning one ring up against the other needs no external torque: the
  motors push the rings apart in angular momentum.

## 4. Mass budget

### Ring cross-section (our assumption)

The fiction gives only the diameter. We take each ring as a rectangular
section [P]:
- 60 m wide, along the spin axis;
- 24 m deep, radially, from r = 326 to 350 m;
- four 6 m decks.

That gives, per ring [C]:
- floor area 514,000 m²;
- pressurised volume 3.06 million m³;
- outer skin 357,000 m², from the 168 m section perimeter × 2,124 m mean
  circumference.

**For two rings:** 1.03 million m² of floor, which is **69 m² per person**
for 15,000 people, and 6.1 million m³, which is **408 m³ per person** [C].

**Comparisons:**
- The Stanford torus allowed about 67 m² of projected area per person [V,
  SP-413 ch. 4]. That includes farms, so our figure is the same order.
- The ISS's habitable volume is 388 m³ [V, NASA] for a crew of about seven.
  Tycho's rings give each person about as much volume as the whole ISS.

### Radiation shielding

The reference design point is the 1975 NASA/Stanford study. It sized the
torus shield at **4.5 t/m²** of raw lunar soil, for a dose under 0.5 rem/yr
(5 mSv/yr) [V, SP-413 ch. 4]. That is 450 g/cm² [C, 1 t/m² = 100 g/cm²],
about 1.7 m thick, and the torus shield massed 9.9 Mt [V, SP-413 ch. 4;
Wikipedia]. It was 95% of the settlement's mass.

Three cautions:
- **GCR dose falls slowly with depth.** One study found that about 180 g/cm²
  of regolith cut the GCR dose on Mars by only about 25% [V, arXiv
  2210.01061]. Secondary particles from the shield itself are the reason.
- **Thick shields are poorly modelled.** NASA notes that shields above about
  100 g/cm² can't currently be sized accurately because of
  transport-model uncertainty [V, NASA NTRS 20250006713].
- **The Belt is not more benign.** GCR at 2.8 AU is similar to, or slightly
  higher than, at 1 AU, because the Sun's shielding weakens with distance
  [M]. Solar particle events are weaker there, by distance.

**The design band.** For families who live there for life, we take
**5–10 t/m²** of regolith or water [P]. The low end is the SP-413 point;
the high end is margin for model uncertainty. Water and hydrogen-rich
material are better per tonne against GCR [M], and the Belt has water
(Ceres may be up to about 30% water by mass [V, secondary]).

NASA's career limit for astronauts is 600 mSv [V, NASA OCHMO brief].
Children raised on the station need far less, which argues for the high
end.

**Areas and masses** [C]:

| Item | Area | Areal mass | Mass |
| --- | --- | --- | --- |
| Two rings, full shield | 714,000 m² | 4.5 / 7 / 10 t/m² | 3.2 / 5.0 / 7.1 Mt |
| Sphere (a robot-heavy factory; crews on shifts) | 785,000 m² | 2 t/m² [P] | 1.6 Mt |
| Sphere, full habitat shield | 785,000 m² | 4.5–10 t/m² | 3.5–7.9 Mt |

### Ring structure: hoop stress

**Spin load.** A thin ring of mass λ per unit length, moving at rim speed v,
carries a hoop tension F = λ v² [C, from the centripetal force on a ring
element].

**Pressure load.** Air pressure adds about p A_section ≈ 101 kPa × 1,440 m²
= 1.5 × 10⁸ N [C].

**Two shield options:**
- **(a) Co-rotating.** The shield rides on the ring. This is what the
  fiction implies.
- **(b) Static.** A non-rotating shield shell surrounds each ring with a
  small gap, as SP-413 chose so that the shield adds no spin stress [V,
  SP-413].

**Contents** (people, furniture, soil, water, equipment): 0.5 t/m² of floor
[P], so 120 t per metre of ring.

**Allowables** (the factor of 2 is our crewed-structure assumption [P],
within the range in [subsystems.md](subsystems.md#structures)):
- AISI 4130 steel: yield 951 MPa, ρ 7,850 kg/m³ [V, Wikipedia UTS table],
  used at half yield.
- 2014-T6 aluminium: yield 414 MPa, ρ 2,800 [V, same], used at half yield.
- CNT composite: 1,200 MPa, ρ 1,300 [V, same], used at half.

**Method.** The required cross-section of hoop material is
A = F / (σ_allow − ρ v²), since the material also carries its own spin load.

| Gravity | Shield | F | Steel | Aluminium | CNT composite |
| --- | --- | --- | --- | --- | --- |
| 1 g | Co-rotating, 4.5 t/m² | 3.2 × 10⁹ N | 7.0 m², 55 t/m, **117 kt/ring** | 16 m², 45 t/m, 95 kt | 5.3 m², 7 t/m, 15 kt |
| 1 g | Static | 5.6 × 10⁸ N | 1.2 m², 21 kt/ring | 17 kt | 3 kt |
| 0.3 g | Co-rotating, 4.5 t/m² | 1.05 × 10⁹ N | 37 kt/ring | 31 kt | 5 kt |
| 0.3 g | Static | 2.7 × 10⁸ N | 10 kt/ring | 8 kt | 1 kt |

All [C].

**What the table shows:**
- **Ordinary metals work.** Steel and aluminium both close with large
  margins. The material's own spin stress, ρv², is only 27 MPa for steel at
  1 g [C]. At a 59 m/s rim speed, CNT is not needed. It matters only for
  much larger habitats (rim speeds of hundreds of m/s).
- **The shield dominates the hoop load.** A co-rotating shield costs about
  100 kt of extra steel per ring at 1 g. A static shield costs a second
  structure and a sliding gap at 59 m/s, which is a debris and seal problem.
  We baseline **co-rotating, at about 0.3–0.5 g** [P]. The spin load scales
  with g, and the setting's Belters prefer low gravity anyway.
- **Pressure skin.** It is thin. A 20 m-radius pressurised cell needs
  t = p r / σ ≈ 101,325 × 20 / 4.75 × 10⁸ ≈ **4.3 mm** of steel [C]. Frames,
  meteoroid layers, and leak margins make real walls much thicker, but they
  are not the mass driver.

### Habitat and life support

**Air.** The two rings' 6.1 million m³ hold about 7,300 t of air [C, at
1.2 kg/m³].

**Food.**
- A full bioregenerative diet needs on the order of 40–50 m² of crop area
  per person under good light [M].
- That is 0.6–0.75 km² of crops for 15,000 people, a large share of the
  rings' floor area.
- Electric lighting at about 300 W/m² [M] costs roughly 200 MW.

**Closure.**
- The ISS recovers about 98% of its water [M, NASA 2023].
- It does not close food or most of its oxygen-carbon loop; resupply does.
- A 15,000-person closed ecosystem has no precedent. BIOS-3 and Biosphere 2
  ran with a handful of people and had trouble keeping oxygen and carbon
  dioxide in balance [M].
- So the station should plan on open loops topped up from local ice and
  carbonaceous asteroids, not perfect closure [P]. This is exactly where
  being in the Belt helps.

### Total mass, order of magnitude

| Item | Mass |
| --- | --- |
| Ring shielding (5–10 t/m²) | 3.6–7.1 Mt |
| Sphere shielding (2–10 t/m²) | 1.6–7.9 Mt |
| Ring hoop and frame structure (steel, co-rotating, 0.3–1 g; ×2 for frames, decks, and bulkheads [P]) | 0.15–0.5 Mt |
| Sphere structure, factory plant, and stockpiles [P] | 0.3–1 Mt |
| Contents, soil, and water in the habitats | about 0.5 Mt |
| Power, radiators, drive, and waldoes [P] | 0.05–0.2 Mt |
| **Total** | **about 6–17 Mt, order 10⁷ t** |

The total is [C] from the rows, with the rows [C] or [P] as marked.

The Stanford torus, at 9.9 Mt of shield in a roughly 10 Mt settlement
[V, SP-413], lands in the same place. Like it, **Tycho is about 90% rock.**

## 5. Power and propulsion

### Power for people and industry

**The ISS benchmark.** It makes 75–90 kW [V, NASA] for about seven crew and
their experiments: about 12 kW per person [C].

**Habitat load** [P]: about 15 kW per person, mostly crop lighting, so
**about 225 MW**.

**Industry** [P]:
- Refining metal from asteroid ore by hydrogen reduction or molten-oxide
  electrolysis plausibly costs 5–20 MWh per tonne [M].
- At 100 kt per year of finished metal that is 1–2 TWh/yr, or **110–230 MW**
  continuous [C].
- Fabrication, forming, and robot work roughly double it.
- Moving and bagging 1–2 Mt per year of shielding rock adds little power.

**Total: about 0.5–1.5 GW** [P], sized from the above.

**Solar is viable in the Belt.** The flux at 2.77 AU (Ceres and Pallas) is
1,361 / 2.77² ≈ 177 W/m² [C]. With 30% cells, 1.5 GW needs about **28 km²**
of array [C]. That is large but thin. It is a real alternative to fusion for
the stationary load, and it removes a reactor from the critical path.

### Radiators

Nearly all of that power becomes heat. Using ε = 0.85 and two-sided panels
[C, from εσT⁴]:

| Reject temperature | Per side | Panel area per GW |
| --- | --- | --- |
| 293 K (habitat) | 355 W/m² | **1.4 km²** |
| 600 K (industry) | 6.2 kW/m² | 0.08 km² |
| 900 K (reactor or furnace loops) | 31.6 kW/m² | 0.016 km² |

**Reject heat as hot as possible.** The habitat's ~0.2 GW needs about
0.3 km² at room temperature [C], about a third of the sphere's whole
surface.
So:
- habitat heat should be heat-pumped up to higher temperature;
- industrial heat should be rejected from the furnace loops directly.

### Moving the station: Δv

**Between nearby coplanar asteroids.** A Hohmann transfer from Vesta's orbit
(2.36 AU) to Ceres's (2.77 AU) costs **0.75 + 0.72 = 1.47 km/s** and takes
**2.05 years** [C, from μ_Sun = 1.327 × 10²⁰ m³/s², circular coplanar
orbits].

**Inclination is the expensive part.** The circular speed at 2.77 AU is
17.9 km/s [C]. A plane change of Δi costs 2v sin(Δi/2):

| Δi | Δv |
| --- | --- |
| 10° | 3.1 km/s |
| 25° | 7.7 km/s |
| 35° | 10.8 km/s |

All [C].

**The move to Pallas.** Pallas's orbit is inclined about 35° [M]. Moving
Tycho there from a typical low-inclination orbit costs **roughly 8–11 km/s**
[C, depending on nodes]. That is far more than any other move in the
fiction.

### Real fusion propulsion against the fiction

**Real concepts:**
- **Direct Fusion Drive** (Princeton Plasma Physics Laboratory and Princeton
  Satellite Systems; not built). Projected:
  - specific impulse about 10⁴ s, possibly up to about 2 × 10⁴ s;
  - 5–10 N of thrust per MW of fusion power;
  - specific power about 0.75–1.25 kW/kg.

  [V, Cohen et al. 2017; arXiv 2009.12621; Wikipedia]
- **Project Daedalus** (a 1970s study of pulsed D–³He fusion). Exhaust about
  10,000 km/s, Isp about 10⁶ s. It would have used 50,000 t of propellant to
  send about 450 t to Barnard's Star, without stopping [V, Wikipedia; arXiv
  1005.3833].

**Propellant to move a 10¹⁰ kg station.** Propellant = M (e^(Δv/vₑ) − 1),
in Mt [C]:

| Δv | Chemical, Isp 450 s | Fusion, Isp 10⁴ s | Fusion, Isp 10⁵ s |
| --- | --- | --- | --- |
| 1.5 km/s | 4.1 | 0.15 | 0.015 |
| 5 km/s | 21 | 0.52 | 0.05 |
| 11 km/s | 111 | 1.2 | 0.11 |

At fusion Isp, propellant is a few percent of the station. A chemical move
of 1.5 km/s, burning 4 Mt of water-derived hydrogen and oxygen, is crude but
not absurd for a short hop.

**Thrust is limited by power.** F = 2 P_jet / vₑ [C], so a = F / M:

| Jet power | Isp | Thrust | Acceleration | Time at full thrust for 1.5 km/s | For 11 km/s |
| --- | --- | --- | --- | --- | --- |
| 1 GW | 10⁴ s | 20 kN | 2 × 10⁻⁶ m/s² | 23 yr | 171 yr |
| 10 GW | 10⁴ s | 204 kN | 2 × 10⁻⁵ m/s² | **2.3 yr** | 17 yr |
| 10 GW | 10⁵ s | 20 kN | 2 × 10⁻⁶ m/s² | 23 yr | 171 yr |

All [C].

**What the realistic version looks like:**
- **10–20 GW of fusion** (about 20 GW of fusion at 10 N/MW for 204 kN).
  At 1 kW/kg that is a 10–20 kt engine plant [C]. That is small against the
  station's mass, so the limit is fusion maturity, not mass.
- **Low Isp.** About 10⁴ s, because for fixed power lower Isp gives more
  thrust.
- **Trip times.** About 2–5 years for a nearby coplanar move, and
  **decades for the move to Pallas** [C]. The orbital geometry already sets
  a two-year floor for even the cheapest transfer.
- **Waste heat.** A 10 GW jet from a 30–50% efficient drive rejects
  10–20 GW of heat. Even at 900 K that is 0.2–0.3 km² of radiator [C].

**The Epstein drive's requirement.** Take the fiction's habit of three days
at 1 g, which is Δv ≈ 2,540 km/s [C].
- With a mass ratio of 2, that needs vₑ ≈ 3,670 km/s (Isp ≈ 3.7 × 10⁵ s)
  [C].
- The jet power per kilogram of ship is ½ a vₑ ≈ **18 MW/kg** [C]. That is
  about 15,000 times the DFD's projected 1.2 kW/kg, before any loss.
- Pushing Tycho at just 0.01 g with that drive would need **1.8 × 10¹⁵ W**,
  about a hundred times humanity's total primary power use [C against
  roughly 19 TW, M].
- Even at 99.9% efficiency it would shed 1.8 TW of waste heat.

**This is the cleanest line between plausible and fiction.**

### Thrust loads and the swivelling compartments

**With realistic propulsion,** the acceleration is around 10⁻⁵ m/s²
(10⁻⁶ g). The rings can keep spinning through a move. A thrust axis along
the spin axis puts no gyroscopic load on them, and the counter-rotating
pair has no net momentum to fight a turn. **The swivelling compartments are
then unnecessary;** they are a design feature for the fictional drive.

**With an Epstein-style 0.1–0.3 g burn** the rings must stop, and the
compartments swivel so floors face aft.

*Spinning down.* Each ring at 1 g holds about 3.4 × 10¹² J [C, ½ M v²
with M = 2 Mt]. Moving that between the rings through 10 MW motors takes
about 4 days [C], and the energy must be stored or dumped.

*Axial load.* The ring now carries its weight along the axis, like a wheel
loaded sideways between spokes. With six spokes [P] each span is 354 m.

At 0.1 g, the load per metre is q = 935 t/m × 0.98 m/s² ≈ 9.2 × 10⁵ N/m.

The bending moment is about q s² / 12 ≈ **9.6 × 10⁹ N·m** [C]. A section
24 m deep needs about 0.8 m² of steel flange at 475 MPa [C]. That is
manageable, and it is ten times worse at 1 g.

*Swivels.* Each compartment's swivel and its bearing carry the full floor
load in both orientations, and every pipe, duct, and cable crosses a
rotating joint. Joints that seal and swivel under load are the weak point.
We would replace them with fixed compartments and spin-only gravity [P].

*Fatigue.* Each spin-up and spin-down is one full hoop-stress cycle. Designed
at half yield, steel survives indefinitely for cycle counts in the
thousands [M; endurance limit roughly 40–50% of ultimate for steels]. Spin
cycles are not the fatigue driver. Pressure cycles of airlocks and
docking, and thermal cycling of radiators, are.

## 6. Construction

### Why it must be built in the Belt

**Lift from Earth is out of the question.** At the ISS's roughly
$150 billion [M, ESA's widely reported estimate] for 419,725 kg [V, NASA],
the station cost about **$357,000 per kg** [C]. Even launching 10⁷ t to
low Earth orbit at a hypothetical $100/kg [P] is $1 trillion before any
transfer to the Belt.

**The real US figures.** The audited US spending on the ISS was about
$75 billion through 2013, including Shuttle flights [V, NASA OIG IG-14-031].
The $150 billion is an all-partner estimate.

**What the Belt supplies:**
- **Shielding,** about 90% of the mass. It is raw or bagged regolith,
  rubble, or water ice, needing only excavation and containment.
- **Structure.** Metal comes from M-type and stony-iron asteroids. Psyche,
  the largest M-type, is about 226 km across and perhaps 30–60% metal [V,
  secondary]; even a 1 km metallic body holds over a billion tonnes [C, at
  about 4,000 kg/m³ × 0.52 km³].
- **Volatiles** (water, carbon, nitrogen) come from C-types and Ceres-like
  bodies, for life support top-up and propellant.
- **Leaving is cheap.** Escape speeds from small asteroids are metres per
  second, so moving material off a body is almost free [M].

**What Earth still supplies** [P]:
- electronics, sensors, bearings, seals, and reactor cores;
- 1–5% of the mass at the start, falling as the local industry learns.

### Bootstrap sequence

These follow on from [cislunar-strategy.md](cislunar-strategy.md). All
dates and sizes are [P].

| Phase | Dates | What happens | Gate to proceed |
| --- | --- | --- | --- |
| 0–4. Cislunar plan | 2027–2042 | LEO station, lunar port, L2 shipyard; ships to 1,000 t wet | Repeated autonomous assembly, cryogenic transfer, and heavy commissioning |
| 5. Belt pathfinders | 2040–2047 | Prospect a metal-rich and a water-rich body; demonstrate excavation, bagging, and small-scale metal extraction | A quantified product from real asteroid material |
| 6. Seed foundry | 2045–2052 | Deliver 2–5 kt of seed equipment, about 5–10 flights of the strategy's 1,000 t class ship on solar-electric propulsion | The foundry makes beams, plates, and its own spare parts |
| 7. Growth | 2050–2062 | Reinvest output so the plant grows to about 100 kt; on Earth-made "vitamins" only | Plant mass doubling every 1.5–3 years, with measured yield |
| 8. First ring and hub | 2060–2070 | Hub, bearings, one ring structure and its shield; crews of hundreds arrive once a shielded volume exists | Pressurised, shielded, spinning ring holds 0.3 g with positive margins |
| 9. Second ring and sphere | 2068–2080 | Counter-rotating partner; manufacturing sphere enclosed; population grows | Zero net momentum demonstrated; first ship built in the sphere |

**The seed ship.** A 1,000 t wet ship on solar-electric propulsion
(Isp 3,000 s [M, from subsystems.md]) with Δv 8 km/s has a mass ratio of
e^(8000/29,420) = 1.31 [C]. It can carry roughly half its mass as payload
[P].

**Growth arithmetic.** If installed plant produces its own mass in output
per year and half is reinvested [P], it doubles every ln 2 / 0.5 ≈
**1.4 years** [C]. Going from 5 kt to 100 kt is 4.3 doublings, about 6
years [C]. Real yields will be lower; the table allows 12.

**Overall.** The table gives an aggressive earliest completion of about
2080 [P]; 2080–2100 or later is more likely. The fiction places the station
centuries out.

### Industrial base at steady state

| Flow | Rate | How |
| --- | --- | --- |
| Shielding rock moved and bagged | 1–2 Mt/yr [P] | Shield in about 5 years. For comparison, a large terrestrial open-pit mine moves on the order of 10⁸ t/yr [M]; this is a modest mine |
| Metal refined and formed | 50–100 kt/yr [P] | Structure in about 5–10 years |
| Structure assembled | 140–270 t/day [C, from the metal rate] | About 1,500–3,000 times the ISS's average (420 t over roughly 13 years of assembly [M], about 32 t/yr) and 50–100 times our 2042 shipyard target |
| Robotic work cells | about 100 at roughly 0.1 t/h each [P] | Each cell joins, welds or latches, and inspects |

### Workforce and timeline

**During construction,** robots do nearly all the work. The ratio we assume
is **100–1,000 robots per human supervisor** [P]:
- humans approve stages;
- robots close every fast loop, as
  [autonomy-and-environment.md](autonomy-and-environment.md) requires;
- staff live in a small shielded core until the first ring is pressurised.

**In service,** the fiction's 15,000 people include families. Perhaps 5,000
are workers [P] supervising tens of thousands of machines, about 10:1 or
more [P].

### Cost, order of magnitude

**At the ISS's unit cost,** Tycho would cost 10¹⁰ kg × $357,000 ≈
$3.6 × 10¹⁵ [C], dozens of times world GDP. That is meaningless.

**A Belt-built estimate** [P]:
- the seed: 2–5 kt delivered at $10,000–50,000/kg;
- Earth-made vitamins: 50–100 kt over the build;
- operations and transport.

That gives a capital cost of order **$0.5–2 trillion**. The uncertainty is
an order of magnitude either way, and it depends entirely on robotic
industry that doesn't yet exist.

## 7. The shipyard

### The waldoes

**Mass, not weight, limits them.** In microgravity a manipulator lifts
nothing, but it must start and stop the load.
- Moving a 10 kt module at 5 cm/s gives a momentum of 5 × 10⁵ N·s [C].
- Stopping it in 100 s needs 5 kN [C], trivial for a large arm.
- Speed limits and momentum budgets set the payload rate, as in
  [in-space-assembly.md](in-space-assembly.md).

**Reach.** Six arms on top of a 500 m sphere can't reach a 2 km hull.
- They should be **walking manipulators** that move end over end between
  grapple fixtures, like Canadarm2 [V, CSA, in in-space-assembly.md].
- Or they should ride gantries along the build jig, with cooperative pairs
  sharing large loads [P].

**Flexibility.** An arm hundreds of metres long has first modes of order
0.01–0.1 Hz [M, scaling from Lagrange 1's 0.15 Hz array wings]. Its control
bandwidth must sit well below that, which makes motions slow (minutes per
placement).

**Base reaction is negligible.** Use the free-floating relation from
[in-space-assembly.md](in-space-assembly.md): a swing turns the station by
about −(m r² / I) Δθ.

Station inertia about a transverse axis [C]:
- rings, about 2 × ½ M r² ≈ 2.3 × 10¹⁴ kg·m²;
- plus the sphere shell, about 7 × 10¹³ kg·m²;
- total **I ≈ 3 × 10¹⁴ kg·m²**.

| Load | Radius | m r² / I | A 30° swing turns Tycho |
| --- | --- | --- | --- |
| 100 t module | 100 m | 3 × 10⁻⁶ | 0.0001° |
| 10 kt capital-ship section | 300 m | 3 × 10⁻³ | 0.09° |
| Lagrange 2: 20 t at 20 m, I = 5 × 10⁷ kg·m² | | 0.16 | almost 5° |

All [C].

**Tycho is the opposite regime to our Lagrange 2 factory.**
- **At Lagrange 2,** every arm move must be budgeted against the CMGs.
- **On Tycho,** arm reactions vanish into the station's inertia.

**What Tycho must budget instead:**
- **Structural flexibility** of the long assemblies.
- **Internal momentum exchange.** Arms on opposite sides should still move
  in opposed pairs to keep internal loads down.
- **Contact loads.** A 10 kt section at 5 cm/s carries only 12.5 kJ [C],
  but its 5 × 10⁵ N·s of momentum, stopped against a stiff latch in 0.1 s,
  is a 5 MN force [C]. So make-safe, capture envelopes, and compliant
  contact matter more than attitude.

### Dock and bay sizing

**Inside the sphere** [P]:
- a usable bay of roughly 400 m long and 150 m across;
- about 7 million m³, about a tenth of the sphere;
- this fits ships up to about 400 m.

**Larger hulls** go in an **external open jig** on the sphere's axis: a
truss cradle as long as the ship, shielded only where crews work [P].

**The Nauvoo** at 2,460 × 950 × 910 m [fiction] is bigger than Tycho itself,
which matches its depiction as built alongside, not inside.

**Docking.** Traffic docks at the non-rotating sphere or the hub on the
spin axis, with no rim docking [P].

## 8. What it could build

### Assumptions [P]

**Assembly rate.** Tycho assembles **75 kt/yr** of finished structure (the
middle of section 6), and moves 1.5 Mt/yr of shielding rock.

**The integration floor.** Small ships are limited by systems integration
and test, not tonnage: about 3–12 months each.

**Realistic propulsion** means a DFD-like fusion engine:
- Isp 10⁴ s (vₑ ≈ 98 km/s);
- ship-level specific power about 0.2 kW/kg;
- acceleration a = 2 P / (m vₑ) ≈ 4 × 10⁻³ m/s² [C].

**The Epstein column** gives what the fiction assumes, with no physical
basis.

**The Nauvoo's mission.** It needs a cruise of about 0.04 c, for example
12 light-years in 300 years [C]. Stopping as well as starting doubles the
Δv to 0.08 c.
- At vₑ = 10,000 km/s (Daedalus-class) the mass ratio is **e^2.4 ≈ 11** [C].
- At vₑ = 1,000 km/s it is **2.6 × 10¹⁰** [C], which is impossible.

**Generation ships need Daedalus-class pulsed fusion.** DFD-class engines
can't do the job.

### Ship classes

| Class | Length | Dry mass | Wet mass | Propulsion (realistic) | Realistic Δv and acceleration | Crew | Build time at 75 kt/yr |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Corvette (Rocinante-like) | 50 m | 0.3 kt | 0.9 kt | DFD-class fusion | 108 km/s at mass ratio 3; about 0.4 mg, so about 10 months to use it | 4–8 | 1.5 days of tonnage; **3–6 months** integration |
| Frigate | 150 m | 3 kt | 9 kt | DFD-class fusion | Same Δv; same acceleration | 30–60 | 2 weeks of tonnage; **6–12 months** |
| Bulk freighter or ice hauler | 300 m spine plus cargo | 10 kt | 30 kt, plus up to 100 kt of cargo | Fusion, or water-fed electric | 108 km/s empty; about 16 km/s with 100 kt of cargo | 20–50 | 7 weeks; **about 1 year** |
| Capital ship (Donnager-like) | 500 m | 50 kt | 150 kt | Fusion with GW-class reactors | About 100 km/s, very slow to build up | 500–2,000 | **8 months** of tonnage; 1.5–2 years |
| Nauvoo-scale generation ship | 2.5 km | 0.5 Mt structure plus 3–7 Mt shield (habitat zones only; the full hull at 5 t/m² would be 54 Mt) | 40–80 Mt (mass ratio 11) | Daedalus-class D–³He pulse | 0.08 c total over centuries; ³He from gas giants is its own industry | 5,000–10,000 | Structure **7 years**; shield 2–5 years in parallel; **propellant decades** |
| Void-city scale | multi-km | 50–100 Mt (Tycho ×5–10) | (station) | Station-keeping only; 10⁻⁶ g if moved | A few km/s over decades | 50,000–100,000 | **Decades**; a second Tycho-scale industry |

The table's dry and wet masses, crews, and lengths are [P] sizing choices.
The Δv, accelerations, and build times are [C] from them and the stated
rates.

**With an Epstein drive** (fiction): every row above gets thousands of km/s
and sustained 0.3–1 g, so a corvette crosses the Belt in days.

**Assembly is the same either way.** The yard's tonnage and integration
times don't depend on the drive, but the reactors and radiators the fiction
leaves out would dominate a real build. A real 1 g corvette of 900 t would
need about 1.6 × 10¹³ W of jet power (½ × 9.8 × 3.7 × 10⁶ × 9 × 10⁵) [C].
That is several times the world's present average electricity generation
(about 3 TW [M]), in one 50 m ship.

**The Nauvoo's mass.** At 2,460 × 950 × 910 m [fiction], its hull surface is
about 10.9 km² [C]. Shielding all of it at 5 t/m² would be 54 Mt [C]. A real
design shields only the inhabited drums, as in the table.

**Its build time.** At Tycho's rates the shield and structure take 5–10
years. That matches the fiction's sense of a long project, but only if the
yard does nothing else.

**Its propellant is the real limit.** It is tens of megatonnes of
deuterium and helium-3. ³He must be mined from a gas giant's atmosphere [M],
an industry larger than Tycho itself.

## 9. Biggest risks and open questions

1. **Fusion maturity.** No fusion rocket has flown or ignited; the DFD is a
   concept [V, Wikipedia]. Plan stationary power on Belt solar (28 km² for
   1.5 GW [C]) and treat the drive as an upgrade, not a dependency.
2. **Radiators.** Every GW needs 0.016–1.4 km² of panel depending on
   temperature [C]. Moving station heat to high temperature (heat pumps,
   liquid-droplet radiators [M]) is an enabling technology.
3. **Closed life support at 15,000 people.** There is no precedent above a
   handful of people in a closed system [M]. Plan for open loops topped up
   from local water and carbon, and staged population growth.
4. **Radiation.** Shield sizing above 100 g/cm² is model-limited [V, NASA].
   Children born on the station need an explicit dose standard; NASA's
   600 mSv career limit [V] is for adult astronauts. Water walls and a storm
   shelter for solar events are cheap insurance.
5. **Structural fatigue and spin cycles.** These are not limiting at
   half-yield design [M]. The swivelling compartments and the 59 m/s
   rim-to-static interfaces are. Hub bearings carrying about 2 Mt rings, and
   10¹⁰ N·m internal moments during any turn, need their own qualification.
6. **Feedstock logistics.** Prospecting must find metal and volatiles close
   to each other or accept hauling. A mapped body is not a reserve, as
   [cislunar-strategy.md](cislunar-strategy.md) says of lunar ice.
   Earth-made "vitamins" stay on the critical path until local electronics
   exist.
7. **Robotic industry at 10⁵ t/yr.** This is the heart of it. Nothing like
   it has flown; the most robotic assembly in orbit to date is in the tens
   of tonnes [M]. Every phase gate in section 6 is about this.
8. **Light time.** At 2–4 AU from Earth, light takes 15–30 minutes one way
   [C, 1 AU ≈ 499 s]. Earth can't supervise even at the stage level in real
   time, so the station is autonomous by necessity.

## 10. Mapping onto our work

**What we already model** in Lagrange 2 (`crates/verse-lagrange2`,
[docs/verse/lagrange-2.md](../verse/lagrange-2.md)):

| Tycho topic | Already modelled |
| --- | --- |
| Stage-checked assembly | `assembly`: the `assembly-plan.v1` document; simulate before, check after, step back or make safe; link loss, stuck latch, controller reset, solar-event and debris holds |
| Free-floating base reaction | The factory deck (`factory::FACTORY_MASS` = 800 t) turns as the momentum relation predicts; mirrored arms cancel; CMG saturation is a named failure |
| Mass properties from CAD | `cad_feed` reads `cad-mass-feed.v1`; the 54 t sample tug's parts come from it |
| Ship document checks | `spacecraft.v1`: mass budget, thrust line, latch loads, assembly tree |
| Burns | Momentum-exact against the exhaust, rocket equation, gimbal tracking, the keel's flexible mode |
| Arm fidelity | MuJoCo 3.3.7 oracle for the scaled arm |

**What a "Tycho" Verse zone would add:**

- **Rotating frames.**
  - Walkers and dropped objects in a ring feel Coriolis (2ωv) and the
    gradient. A test can check a dropped ball's drift against the
    rotating-frame solution.
  - Spin-up and spin-down go through ring motors, with energy and momentum
    ledgers.
- **Counter-rotating rings on bearings.**
  - Zero net momentum.
  - The internal gyroscopic moment when the station turns, which is a named
    failure if the bearings' rating is exceeded.
- **The inertia regime flips.** Arm reactions become negligible, and
  flexible-structure dynamics and collision energy become the checks. The
  same tests apply with different thresholds.
- **A heliocentric orbit model.** Two-body Sun plus asteroid, with
  low-thrust trajectories in place of the Earth–Moon CR3BP. It would check
  the Hohmann and plane-change numbers above.
- **Logistics and ISRU documents.** Typed flows of rock, metal, and
  volatiles; plant growth; Earth vitamins. These are checked like the
  spacecraft document, with a refusal when a schedule outruns its
  feedstock.
- **Scale.** Kilometre geometry and 10¹⁰ kg masses need care with
  floating-point precision: local frames per ring and per bay.
- **Candidate tests:**
  - `ring_hoop_margin_is_positive_at_rated_spin`
  - `counter_rotating_rings_cancel_momentum`
  - `ring_coriolis_matches_two_omega_v`
  - `waldo_swing_turns_tycho_as_predicted`
  - `station_move_obeys_the_rocket_equation`
  - `spin_down_energy_balances`

Order of work, if the owner wants the zone [P]:
1. The rotating ring as a physics frame.
2. The pair on bearings.
3. A single waldo on the sphere, reusing the Lagrange 2 arm and sequencer.
4. The heliocentric move.

## Sources

**Real world:**
- NASA, [Space Station Facts and Figures](https://www.nasa.gov/international-space-station/space-station-facts-and-figures/):
  ISS mass 419,725 kg, habitable volume 388 m³, 75–90 kW.
- NASA OIG, [IG-14-031](https://oig.nasa.gov/docs/IG-14-031.pdf), and
  [SpacePolicyOnline summary](https://spacepolicyonline.com/news/nasa-ig-iss-cost-u-s-75-billion-so-far-estimates-of-future-costs-overly-optimistic/):
  US ISS cost about $75 billion through 2013.
- CBC, [ISS background](https://www.cbc.ca/news2/background/space/iss.html):
  the ESA estimate of about $150 billion overall.
- Johnson and Holbrow (eds.), *Space Settlements: A Design Study*, NASA
  SP-413 (1977), [chapter 4](https://nss.org/settlement/nasa/75SummerStudy/Chapt4.html):
  4.5 t/m² shield, 0.5 rem/yr, 9.9 Mt shield, about 150 kt structure, 1 rpm,
  0.95 g, 67 m² per person.
- Wikipedia, [Stanford torus](https://en.wikipedia.org/wiki/Stanford_torus):
  1.8 km, 10,000 people, 10 Mt, 1.7 m of lunar soil.
- Wikipedia, [Direct Fusion Drive](https://en.wikipedia.org/wiki/Direct_Fusion_Drive);
  Cohen et al. 2017, [PPPL](https://w3.pppl.gov/ppst/docs/Cohen2017ibis.pdf);
  [A Titan mission using the DFD](https://arxiv.org/pdf/2009.12621).
- Wikipedia, [Project Daedalus](https://en.wikipedia.org/wiki/Project_Daedalus);
  [Project Icarus](https://arxiv.org/pdf/1005.3833).
- Wikipedia, [Ultimate tensile strength](https://en.wikipedia.org/wiki/Ultimate_tensile_strength):
  steel, aluminium, and CNT values.
- [Radiation protection and shielding materials for crewed missions on the
  surface of Mars](https://arxiv.org/pdf/2210.01061);
  NASA, [lunar radiation presentation](https://ntrs.nasa.gov/api/citations/20250006713/downloads/nasem_presentation%20v07012025%20vf.pdf).
- NASA OCHMO, [Radiation protection technical brief](https://www.nasa.gov/wp-content/uploads/2023/03/radiation-protection-technical-brief-ochmo.pdf):
  600 mSv career limit.
- [Design Limits on Large Space Stations](https://arxiv.org/pdf/2408.00152):
  the summary of the Globus and Hall comfort chart, 2 rpm.
- Wikipedia, [16 Psyche](https://en.wikipedia.org/wiki/16_Psyche) and
  [M-type asteroid](https://en.wikipedia.org/wiki/M-type_asteroid).

**Fiction (the spec):**
- The Expanse Wiki, [Tycho Station](https://expanse.fandom.com/wiki/Tycho_Station).
- Tor.com, [9 spaceships that function as their own societies](https://tor.com/2019/07/17/9-spaceships-that-function-as-their-own-societies),
  for the Nauvoo's dimensions.
- The owner's notes from the wiki: the waldoes, the 50 m drive projection,
  the Titanic class, Pallas, and the Void cities.
