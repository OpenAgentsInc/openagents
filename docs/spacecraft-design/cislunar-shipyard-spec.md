# The largest cislunar shipyard that works with today's technology

Engineering spec and decision document. Planning date: October 14, 2026.

**The owner's ask:** using today's technology, chart the largest space
shipyard that makes sense with current physics and propulsion. Decide whether
Earth–Moon L2 is the right place for it. Say what its specs are and what is
needed to build it.

This note builds on the rest of the folder:

- the phased plan in [cislunar-strategy.md](cislunar-strategy.md);
- the far-future upper bound in [tycho-assessment.md](tycho-assessment.md);
- the orbit facts in [astrodynamics.md](astrodynamics.md);
- the sizing rules in [subsystems.md](subsystems.md);
- the arm physics in [in-space-assembly.md](in-space-assembly.md);
- the autonomy and radiation rules in
  [autonomy-and-environment.md](autonomy-and-environment.md);
- the simulation in [sim-mapping.md](sim-mapping.md) and
  `crates/verse-lagrange2`.

Marks follow the [README](README.md):

- [V] verified against the cited source while writing;
- [C] computed by us, with the method shown;
- [M] from memory or from secondary reporting only, to verify before it
  becomes a code constant;
- [P] a planning target or assumption.

The calculations were run as short scripts while writing. Their inputs are
stated beside each result, so anyone can repeat them.

## 1. The decision in one page

**Location: build the biggest yard in low Earth orbit, not at L2.**

- **Where:** a low-inclination LEO yard at about 28.5–33° and 450–500 km
  [P].
- **Its partners:**
  - a co-orbiting cryogenic depot a few kilometres away;
  - a smaller departure port and propellant depot in the L2 southern
    near-rectilinear halo orbit (NRHO).
- **L2 comes later.** The L2 halo becomes the heavy yard's second home only
  when propellant made on the Moon is cheaper there than propellant lifted
  from Earth. With regolith oxygen alone it isn't (section 2.4). With lunar
  water it can be.

**Why:**

1. **Delivered mass.** One Starship flight puts about 100 t in LEO [V]. The
   same 100 t at an L2 halo or the NRHO costs about 2.7–5.5 launches,
   counting the tankers [C, section 2.2].
   - A 2,000 t yard costs about 20 launches to build in LEO, and about
     55–110 at L2.
   - Every tonne of ship and propellant afterwards carries the same
     multiplier.
2. **The departure Δv advantage doesn't pay when the propellant comes from
   Earth.**
   - A Mars ship leaving from L2 needs about 0.75 km/s instead of 3.6 km/s
     [C].
   - But the same ship built and fuelled in LEO costs about 16–24 launches,
     and staged through L2 on Earth propellant it costs about 25–46 [C,
     section 2.3].
3. **Crew.** LEO sits inside the magnetosphere.
   - ISS crews receive about 0.4–1.0 mSv/day [V]. Deep space delivered
     about 1.8 mSv/day inside Curiosity's cruise vehicle [V].
   - At L2, a six-month tour would use more than half of NASA's 600 mSv
     career limit [V, C]. In LEO it uses 12–30% (72–180 mSv), and about
     12% at the lower dose rate we plan for a 30° orbit [C, P].
   - Rescue from LEO takes hours. From L2 it takes days.
4. **The owner's Earth-view preference is met.** A LEO yard never sits in
   front of the Moon. It crosses the lunar disc for a fraction of a second
   from any given site, like the ISS.

**Why L2 still matters:**

- Departures from the NRHO or the L2 halo cost 2–3 km/s less than from LEO
  for Mars, Belt and cycler trajectories [C].
- The lunar surface is 2.5–2.8 km/s away from there, against 5.9 km/s from
  LEO [M].
- So the port goes there first, and the yard follows once lunar fuel flows.

### The headline specs (LEO Yard, full build) [P unless marked]

| Item | Value |
| --- | --- |
| Yard dry mass | About 1,600–2,500 t, 4–6 times the ISS's 420 t [V, C]; plus a 300–600 t depot |
| Crew | 16 permanent on 6-month tours, rising to 24–32 during surges |
| Pressurised volume | About 4,000 m³ (the ISS has about 916 m³ pressurised [M] and 388 m³ habitable [V]) |
| Power | 1.6 MW average; about 3.2 MW of arrays at beginning of life [C] |
| Radiators | About 3,200 m² [C] |
| Assembly | Two heavy bays, each 250 m × 40 m, plus one precision bay |
| Largest ship | About 600 t dry and 2,400–3,000 t wet, about 250 m long |
| Depot | 2,000–6,000 t of liquid oxygen and methane |
| Throughput | About 5 kt/yr of hardware and 15–20 kt/yr of propellant handled, at about 200 program launches a year |
| Output | About 3 crewed Mars transfer vehicles of 2,400 t wet a year, plus tugs, telescopes and depot modules |
| Capital cost | About $20–60 billion |
| Operating cost | About $6–24 billion a year, mostly launch |
| First heavy ship | 2035–2037 |
| Full production rate | 2037–2040 |

**Top three enabling technologies:**

1. Starship-class full reuse at 100 t or more, at a sustained cadence of
   hundreds of flights a year.
2. Cryogenic propellant transfer, gauging and long-duration storage at the
   scale of thousands of tonnes.
3. Robotic berthing and joining of 100 t-class modules, with fluid and power
   connections made in orbit.

## 2. Location trade

### 2.1 What "today's technology" means here

| Area | Baseline | Status, with source |
| --- | --- | --- |
| Heavy lift | Starship Block 3 | About 100 t to LEO, 1,600 t ship propellant, Raptor Vacuum Isp 380 s [V, [Wikipedia, SpaceX Starship](https://en.wikipedia.org/wiki/SpaceX_Starship)]; first V3 flight (Flight 12) on May 22, 2026, booster lost [V, [NSS](https://nss.org/?p=119535), [Pulse 2.0](https://pulse2.com/spacex-starship-flight-12-marks-first-successful-v3-test-flight-and-major-advancement-for-mars-program/amp/)]. Reusable 100 t is a target, not yet demonstrated [M] |
| Backups | Falcon Heavy, New Glenn | Flight-proven heavy lift at tens of tonnes [M] |
| Price | Starship price per flight | Infobox: $100 million expendable [V, Wikipedia]. Targets of about $10 million a launch, or about $100/kg, are repeated in secondary reporting only [M, e.g. [ASPI](https://www.aspistrategist.org.au/the-starship-revolution-in-space/)] |
| Propellant transfer | Ship-to-ship transfer | Not yet flown; "NET late 2026" [V, [Wikipedia](https://en.wikipedia.org/wiki/Starship_Propellant_Transfer_Demonstration)]; "ten-ish" tanker flights per lunar lander mission per a SpaceX official [V, same] |
| Cryogenic storage | LOXSAT (Eta Space, Rocket Lab) | Liquid oxygen storage and transfer demo, 9 months of data planned; launch listed for 2026 but not confirmed as flown [V, [Rocket Lab](https://rocketlabcorp.com/missions/launches/loxsat/), [Gunter's Space Page](https://space.skyrocket.de/doc_sdat/loxsat.htm)] |
| Solar arrays | iROSA | More than 20 kW per wing; 8 wings give more than 160 kW; 6 deployed as of January 2025 [V, [Redwire](https://ir.redwirespace.com/news-events/press-releases/detail/152/redwire-successfully-delivers-fourth-pair-of-roll-out-solar)] |
| Solar-electric propulsion | Gateway PPE | Up to 60 kW, three 12 kW AEPS Hall thrusters plus four 6 kW BHT-6000 [V, [NASA](https://www.nasa.gov/centers-and-facilities/glenn/nasa-starts-up-gateways-power-system-for-first-time), [NTRS](https://ntrs.nasa.gov/api/citations/20220007037/downloads/SP2022_438_CASSADY.pdf)]; AEPS ran at 4.2–12.5 kW in 2019 [V, [GlobeNewswire](https://www.globenewswire.com/news-release/2019/11/08/1944195/0/en/Advanced-Electric-Propulsion-Thruster-for-NASA-s-Gateway-Achieves-Full-Power-Demonstration.html)] |
| Gateway itself | Lunar Gateway | Paused in March 2026; the PPE and HALO have no firm launch [V, [Wikipedia](https://en.wikipedia.org/wiki/Lunar_Gateway)] |
| Fission | Kilopower / FSP | KRUSTY ran 28 h at full power in 2018, as a 1 kWe-class unit [V, [Wikipedia](https://en.wikipedia.org/wiki/Kilopower)]. NASA now asks for a 100 kWe lunar reactor ready to launch by late calendar 2029 [V, [ANS](https://www.ans.org/news/2026-01-14/article-7674/nasa-doe-solidify-collaboration-on-a-lunar-surface-reactor/)] |
| Inflatables | Sierra Space LIFE | Full-scale unit about 283 m³ (10,000 ft³), burst at 77 psi against a 60.8 psi requirement [V, [SatNow](https://satnow.com/news/details/1291-sierra-space-announces-pioneering-full-scale-burst-test-of-expandable-space-station-module), [The Space Bucket](https://thespacebucket.com/full-scale-life-habitat-testing-is-underway/)]; BEAM has flown on the ISS since 2016 [M] |
| Life support | ISS ECLSS | 98% water recovery [V, [Space.com](https://space.com/astronaut-pee-iss-water-recycling-98-percent-milestone)]; about 50% of oxygen recovered from CO₂, with 75% the minimum and 90% the goal for long missions [V, older, [ECS abstract](https://ecs.confex.com/ecs/236/webprogram/Paper127652.html)] |
| Lunar oxygen | Blue Alchemist / Air Pioneer | Molten regolith electrolysis has made oxygen from simulant; critical design review in September 2025; vacuum-chamber demo planned for 2026; nothing yet on the Moon [V, [Blue Origin](https://www.blueorigin.com/news/blue-alchemist-hits-major-milestone-toward-permanent-sustainable-lunar-infrastructure), [Gigazine](https://gigazine.net/gsc_news/en/20260412-blue-origin-moon-dust-to-fresh-air)] |
| Robotics | Canadarm2, Dextre | 17 m, 7 joints, walks end over end; Dextre has two 3.51 m arms [V, see [in-space-assembly.md](in-space-assembly.md)]. OSAM-1 was cancelled in 2024 [V, same] |

**Out of scope:** fusion, nuclear thermal propulsion, space elevators,
mass drivers, unproven materials, and lunar water ice as a production
resource. Section 6 covers what changes if any of them arrive.

### 2.2 Getting mass there from Earth

**The method [C].** One Starship carries a payload P from LEO to the
destination and returns to Earth by aerocapture. Its Δv budget is:

- outbound Δv;
- plus about 0.4 km/s to lower its perigee into the atmosphere [M] (1.3 km/s
  from low lunar orbit, 1.5 km/s from GEO [M]).

**The assumptions:**

- ship dry mass 120–150 t [M];
- tank 1,600 t [V];
- effective Isp 375 s (vₑ = 3.68 km/s), a blend of vacuum and sea-level
  Raptors [M];
- each tanker delivers 100–150 t of propellant to LEO [P];
- aerocapture from lunar return speed is assumed to work, which is not yet
  demonstrated [M].

We solved for the smallest LEO fill that closes the round trip, then counted
the tankers needed to supply it.

| Destination | Outbound Δv from LEO | Launches per 100 t delivered | Cost per delivered tonne at $50–100M a launch [P] | At $10–20M a launch [P] |
| --- | --- | --- | --- | --- |
| LEO, 28.5–33° | 0 | **1.0** | $0.5–1.0M | $0.1–0.2M |
| LEO, 51.6° | 0 | about 1.1–1.2 (payload loss to a higher inclination [M]) | $0.6–1.2M | $0.1–0.25M |
| L2 halo | about 3.45 km/s [M] | **2.7–5.3** [C] | $1.4–5.3M | $0.3–1.1M |
| NRHO | about 3.55 km/s [M]; CAPSTONE needed only 70–150 m/s after injection [V, search summary of [Advanced Space](https://advancedspace.com/?p=2210)] | 2.8–5.5 [C] | $1.4–5.5M | $0.3–1.1M |
| DRO | about 3.5 km/s [M] | 2.7–5.4 [C] | $1.4–5.4M | $0.3–1.1M |
| L1 | about 3.8 km/s [M] | 3.0–6.0 [C] | $1.5–6.0M | $0.3–1.2M |
| Low lunar orbit | about 4.0 km/s [M] | 3.7–7.8 [C] | $1.9–7.8M | $0.4–1.6M |
| GEO | about 4.2 km/s from 28.5° [M] | 4.1–8.7 [C] | $2.1–8.7M | $0.4–1.7M |

The low end of each range is a 150 t payload, a 120 t ship and 150 t
tankers. The high end is a 100 t payload, a 150 t ship and 100 t tankers.

**Note that GEO is deeper than L2.** A shipyard placed in GEO "to be closer"
would be the worst choice on this measure.

**Solar-electric tugs** could change the multiplier for slow dry cargo:

- A spiral from LEO to L2 of about 7 km/s [M] at an Isp of 2,000 s
  delivers about 70% of the starting mass [C, exp(−7,000 / 19,600) = 0.70].
- But a 1 MW tug at 60% efficiency gives about 61 N [C, F = 2ηP / vₑ]. On a
  150 t stack that takes about 200 days [C].
- It also spends weeks in the radiation belts.
- And no SEP system above the PPE's 60 kW is built [V].

This is a gate, not a baseline (section 5.4).

### 2.3 Building where ships depart: the decisive calculation

**The reference ship.** A crewed Mars transfer vehicle (MTV) of 400 t dry
[P], on methalox. After leaving Earth it needs about 2.9 km/s for Mars
capture, Earth return and Earth capture [P]. So its mass after the departure
burn is 400 × e^(2.9/3.68) = **880 t** [C].

**The departure burn** for a Mars trajectory with C3 = 10 km²/s²:

- From a 450 km LEO it is 3.62 km/s [C]: v = √(v_esc² + C3) − v_circ.
- From the NRHO or the L2 halo it is about 0.75 km/s [C]:
  - about 0.3 km/s to fall to a 450 km perigee using a lunar flyby [M];
  - then a 0.45 km/s perigee burn [C].

| Plan | Where the propellant comes from | Propellant | Launches at 100–150 t each |
| --- | --- | --- | --- |
| A. Build and fuel in LEO; depart from LEO | Earth, to LEO | 1,956 t in LEO [C: 880 × e^(3.62/3.68) = 2,355 t wet] | 13–20 tankers + 3–4 hardware = **16–24** |
| B. Build in LEO; fly to L2; fuel there; depart | Earth, delivered to L2 | 622 t in LEO to fly the dry ship up [C], plus 679 t at L2 [C: 880 × e^(0.75/3.68) = 1,079 t wet] | 4–6 + 18–36 + 3–4 = **25–46** |
| C. As B, with 78% of the L2 propellant as lunar oxygen | Earth methane plus lunar oxygen | 149 t of Earth methane at L2 [C, at an oxidiser-to-fuel ratio of 3.6] | 11–18, **plus** the cost of lifting 530 t of lunar oxygen to L2 (section 2.4) |

**Result [C].** With Earth propellant, building and departing in LEO wins by
about a factor of 1.5–2. The rocket equation favours fuelling at the bottom
of the well when the fuel comes from the bottom of the well.

Departure from L2 wins only when propellant there comes from a shallower
well. Plan C is the only plan that beats A, and only if lunar oxygen at L2
costs less than Earth propellant. The next section checks that.

**A practical variant of plan A** avoids one huge LEO fill. Depart LEO
partly fuelled into a high elliptical orbit, then top up at apogee from
tankers before the perigee departure burn. It costs the same in total
propellant but splits the burn and the tanker traffic [M]. This is the
pattern the LEO Yard should support.

### 2.4 Does lunar oxygen change the answer?

**The method [C].** A reusable lander with 50 t dry [P] carries 100 t of
lunar liquid oxygen from the surface to the port, then flies back empty. Its
fuel (methane or hydrogen) has to come from Earth. Regolith gives oxygen
only; [cislunar-strategy.md](cislunar-strategy.md) says the same.

**The assumptions:**

- 2.6 km/s each way between the surface and the NRHO or L2 [M];
- 1.9 km/s each way for low lunar orbit [M];
- the lander refuels its oxidiser from the delivered oxygen.

**How costly is Earth fuel on the surface?** A tonne delivered to the lunar
surface costs about **3.6–3.8 times** a tonne delivered to L2 [C]. That is
the same Starship model with 5.9 km/s outbound and 3.1 km/s back. It also
needs more than one full tank, so it is optimistic.

| Lander | Leg | Net oxygen delivered per trip | Earth fuel needed on the surface | Earth fuel needed at the port | Earth fuel in L2-delivered tonnes | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Methalox, Isp 375 s | Surface ↔ NRHO or L2 | 59.8 t | 33.5 t | 11.2 t | about 132 | **Loses**: 132 t of Earth effort to supply 60 t of oxygen |
| Methalox | Surface ↔ low lunar orbit | 73.5 t | 22.1 t | 7.4 t | about 87 | Loses |
| Hydrolox, Isp 450 s | Surface ↔ NRHO or L2 | 66.0 t | 18.5 t | 6.2 t | about 73 | About break-even |
| Hydrolox | Surface ↔ low lunar orbit | 77.2 t | 12.4 t | 4.1 t | about 49 | Saves about 37% |

All [C], from up-leg propellant = (dry + payload)(e^(Δv/vₑ) − 1) and
down-leg propellant = dry (e^(Δv/vₑ) − 1).

**Conclusion [C]:**

- **Regolith oxygen alone does not make a cislunar port's propellant
  cheaper,** because the lander's fuel still comes from Earth.
- Lunar oxygen pays first **on the surface**: as ascent oxidiser for
  landers, and as breathing oxygen. There, every tonne saves 3.6–3.8 t of
  L2-equivalent delivery.
- It pays for export only with a hydrogen lander to low lunar orbit, or
  when the fuel is also lunar.
- **Lunar water** (both oxygen and hydrogen) is what turns L2 into the
  cheapest place to fuel ships. That is outside "today's technology", so
  it is the trigger for moving the heavy yard to L2 (section 6).

### 2.5 Departure Δv for the ships built there

The Mars, Belt and cycler columns use v = √(v_esc² + C3) − v_start [C]:

- Mars: C3 = 10 km²/s² [M, a typical opportunity].
- Ceres: C3 = 40 km²/s² [C], from a 1 → 2.77 AU Hohmann v∞ of 6.3 km/s. This
  is departure only; the inclination change and rendezvous add several
  km/s more [C, tycho-assessment.md].
- Aldrin cycler: C3 = 42 km²/s² [M, from a v∞ of about 6.5 km/s].

| Start | Mars departure | Belt departure (Ceres) | Cycler injection | To GEO | To the lunar surface |
| --- | --- | --- | --- | --- | --- |
| LEO, 450 km | **3.62** [C] | 4.88 [C] | 4.97 [C] | 3.9–4.3 [M] | 5.9 [M] |
| GEO | 2.30 [C] | 4.60 [C] | about 4.7 [C] | 0 | about 3.5–4 [M] |
| NRHO | about 0.75–1.2 [C+M] | about 2.0–2.4 [C+M] | about 2.1–2.5 [C+M] | about 1.4–1.8 [C+M] | 2.5–2.7 [M]; 0.2–0.4 to a polar low orbit [V, secondary, [Cislunar Space guide](https://www.cislunarspace.cn/en/cislunar-orbits/nrho/gateway-cases/)] |
| L2 halo | about 0.75–1.0 [C+M] | about 2.0 [C+M] | about 2.1 [C+M] | about 1.3–1.5 [C: a 1.08 km/s GEO insertion after a fall from L2, plus 0.2–0.4 to leave] | 2.6–2.8 [M] |
| L1 | about 0.8–1.2 [M] | about 2.0–2.3 [M] | about 2.1–2.4 [M] | about 1.3 [M] | 2.5 [M] |
| DRO | about 0.8–1.2 [M] | about 2.0–2.4 [M] | about 2.1–2.5 [M] | about 1.5 [M] | 2.8–3.0 [M] |
| Low lunar orbit | about 1.4 [C+M: 0.9 to leave + a 0.45 perigee burn] | about 2.6 [C+M] | about 2.7 [C+M] | about 2.0 [M] | 1.9 [M] |

**The L2-family departures save 2.5–3 km/s** for Mars, Belt and cycler work
[C]. That is a factor of about e^(2.9/3.68) ≈ 2.2 in mass ratio for
methalox [C]. It is real, and it is why the port goes there. Section 2.3 shows
why it doesn't pay with Earth propellant.

### 2.6 The other criteria

| Criterion | LEO, 28.5–33° | LEO, 51.6° | GEO | NRHO | L2 halo | L1 | DRO | Low lunar orbit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Radiation, crew | Inside the magnetosphere; ISS (51.6°) gets 0.4–1.0 mSv/day [V, [SwRI via Universe Today](https://www.universetoday.com/articles/human-voyages-to-mars-pose-higher-cancer-risks)]; lower inclination gets less GCR [M] | 0.4–1.0 mSv/day [V] | Outer belt: severe electron dose for electronics [M] | Deep space GCR: about 1.8 mSv/day measured in cruise [V, [SwRI](https://www.swri.org/newsroom/press-releases/swri-led-team-calculates-the-radiation-exposure-associated-trip-mars)]; solar events unshielded | As the NRHO; passes the magnetotail monthly [V, autonomy-and-environment.md] | As the NRHO | As the NRHO | As the NRHO, with the Moon blocking about half the sky |
| Eclipses and thermal | About 36 min of each 94 min orbit in shadow [C: arcsin(R_E / r) / π = 0.38]; 15 thermal cycles a day | Same | Eclipse seasons up to about 72 min [M] | Eclipse-free by design (9:2 resonance) [V, astrodynamics.md] | Mostly sunlit; eclipses up to a few hours, avoidable [M] | Mostly sunlit [M] | Some lunar and Earth shadow passes [M] | About 45 min in each 2 h orbit, plus lunar IR [M] |
| Tides on long structures | 3n² ≈ 3.8 × 10⁻⁶ s⁻² [C]: a released part 100 m from the centre of mass drifts about 0.17 m in 30 s | Same | About 1.6 × 10⁻⁸ s⁻² [C] | 2.3 × 10⁻⁷ s⁻² at perilune [C] | About 5 × 10⁻¹¹ s⁻² [C, sim-mapping.md] | Similar | Small | About 10⁻⁶ s⁻² [C] |
| Debris | High; Starlink and other shells nearby [M]; routine avoidance burns [M] | High | Crowded, regulated belt [M] | Negligible debris; meteoroids as anywhere | Negligible | Negligible | Negligible | Negligible |
| Communications | Continuous through relays [M] | Same | Continuous | Continuous Earth view by design [V, astrodynamics.md]; 1.3–1.5 s light time [C] | Continuous on a halo [V, astrodynamics.md]; 1.5 s [C] | Continuous | Continuous | Far-side gaps |
| Station-keeping | 1.4–8.5 m/s/yr of drag at 450 km for this yard [C, below], plus avoidance burns [M] | Same | About 50 m/s/yr north–south [M] | "A few m/s per year" or less [V, Parrish et al., astrodynamics.md]; about 1.1 m/s/yr in simulation [V, [MERL TR2022-114](https://www.merl.com/publications/docs/TR2022-114.pdf)] | 7.39 m/s/yr for ARTEMIS P1 [V] | 5.3 m/s/yr for ARTEMIS [V] | Under 1 m/s/yr; stable [M] | Tens to 100+ m/s/yr except frozen orbits [M] |
| Abort and rescue | A docked capsule reaches the ground in under a day [M] | Same | 5–10 h to the atmosphere [M] | About 4–6 days home [M]; no rescue vehicle on standby | About 5–7 days [M] | About 4–5 days [M] | About 6–10 days [M] | About 4–5 days [M] |
| Earth's view of the Moon | Never between Earth and Moon; a bright moving star | Same | Never in front of the Moon | Behind the Moon or near its limb; can't occult the lunar surface from Earth [V, cislunar-strategy.md] | Same as the NRHO | **Directly between Earth and Moon: ruled out by the owner's preference** | Circles the Moon; sometimes in front | Seen against the lunar disc, but too small to resolve [C] |

**The drag figure [C].**

- The yard and the ships in build total about 5,000 t, with about 7,000 m²
  of average projected area (arrays partly feathered) [P].
- Drag acceleration is ½ ρ v² C_d A / m, with C_d = 2.2 [M].
- Density at 450–500 km is about 5 × 10⁻¹³ kg/m³ at solar minimum and
  3 × 10⁻¹² at solar maximum [M].
- That gives 1.4–8.5 m/s/yr. At an Isp of 2,000 s, 5,000 t × 8.5 / 19,600 is
  about **2.2 t/yr** of xenon or krypton [C]. Chemical reboost would need
  about 13 t/yr.

### 2.7 The recommendation

| Node | Location | Job | When [P] |
| --- | --- | --- | --- |
| **LEO Yard** (the biggest) | About 30° (28.5–33°), 450–500 km, coordinated around the megaconstellation shells | Assemble and integrate ships of up to about 600 t dry; crew base; robot depot; ship checkout | Grows from the strategy's LEO work station (2029–2030); full build 2034–2036 |
| **LEO depot** | Co-orbiting 5–10 km from the yard, never at the yard | Receive tanker loads, store methalox, fuel finished ships away from the crew | 2032–2035, after transfer and storage gates |
| **NRHO port** | L2 southern NRHO, the strategy's lunar logistics orbit | Top up departing ships; receive lunar oxygen for landers; store spares; a staging point for crewed lunar flights | 2031–2033, robotic; crew visits only |
| **L2 halo heavy yard** | A conventional L2 halo, as the strategy proposes | Becomes the main yard when lunar water or another lunar fuel closes section 2.4 | Not before about 2040; gated |

**Is L2 the right home for the biggest yard? Not today.**

With Earth-launched mass, L2 multiplies the cost of every delivered tonne by
2.7–5.5 [C]. It turns its 2.5–3 km/s departure advantage into a net loss for
Earth propellant [C, section 2.3]. It triples or quadruples the crew's dose
rate [V, C], and it puts rescue days away.

L2 becomes the right home when propellant made on the Moon (from water, or
fuel and oxidiser together) is delivered there for less than the Earth
alternative. Our data say regolith oxygen alone won't do that [C, section
2.4].

**Why about 30°, not 51.6°:**

- Starship's launch sites are at about 26–28.5° latitude [M], so 30°
  maximises payload.
- Mars and Belt departure asymptotes are reachable as the orbital plane
  precesses [M].
- Its GCR exposure is lower than the ISS's [M].

The cost is no access from Baikonur, which we don't need, and a loss of easy
ISS co-orbit transfers.

**Why not the ISS's altitude:** at 450–500 km, drag is a few times lower
than at 400–420 km [M]. The FCC five-year disposal rule [V,
autonomy-and-environment.md] still makes a dead yard fall in a known time,
if its ballistic coefficient is managed.

## 3. Sizing the yard by its bottleneck

### 3.1 What actually limits it

| Candidate limit | Today's figure | Binds? |
| --- | --- | --- |
| Structure | Truss and pressure-shell loads in microgravity are small; the assembled ship sees only its own thrust [V, subsystems.md] | No |
| Launch pads and licences | Starbase: up to 25 a year [V, [Drive Tesla](https://driveteslacanada.ca/news/faa-approves-up-to-25-starship-launches-per-year-from-starbase/)]. LC-39A: 44 a year in the draft EIS [V, [NASASpaceflight](https://www.nasaspaceflight.com/2026/01/faa-advances-approval-44-starship-launches-39a/)]. SLC-37: 76 a year from the Air Force, pending the FAA [V, same]. **About 146 a year in total**, for all of SpaceX's Starship customers [V, same] | **Yes, first.** A program using 100–400 launches a year needs most of today's approved capacity, or new pads |
| Launch cost | Section 2.2 | Yes; sets operating cost |
| Propellant transfer and storage | Not yet flown at scale (section 2.1) | Yes; a gate |
| Integration and robotic assembly | ISS assembly averaged about 32 t/yr [M, tycho-assessment.md]; no robotic multi-module assembly has flown [V, in-space-assembly.md] | Yes, once launch is unlocked |
| Crew transport | Dragon carries 4 [M]; crewed Starship is not human-rated [M] | Yes, for crew above about 16 |

**Our call:** size the yard to about **200 program launches a year** [P].

- That is above today's licensed total, so it assumes the pads approved by
  the 2030s and at least one more.
- Above that, add bays rather than enlarge them. Pads, crew and propellant
  logistics bind before the yard does.

### 3.2 Delivered tonnes at aggressive cadences [C]

The split is 25% hardware, 70% propellant and 5% crew and logistics [P].

| Program launches a year | 100 t each | 150 t each | Hardware | Propellant |
| --- | --- | --- | --- | --- |
| 100 | 10 kt | 15 kt | 2.5–3.8 kt | 7–10.5 kt |
| **200** | **20 kt** | **30 kt** | **5–7.5 kt** | **14–21 kt** |
| 400 | 40 kt | 60 kt | 10–15 kt | 28–42 kt |

**The same cadence delivered to L2** gives only 1/2.7 to 1/5.5 of these
tonnes [C, section 2.2]: about 3.6–11 kt a year for 200 launches.

**Assembly capacity [P]:**

- A heavy bay joins one 100–150 t module every 2–3 weeks, including
  connection and test. That is **1.7–3.9 kt/yr per bay** [C].
- Two heavy bays handle about 3.4–7.8 kt/yr, which matches the hardware
  share of 200 launches.

### 3.3 Station specs, LEO Yard

#### Mass budget [P, rows; C, totals]

| Element | Mass | Notes |
| --- | --- | --- |
| Truss spine and two heavy bays (about 800 m of heavy truss at 0.5–1 t/m) | 400–800 t | Steel or aluminium truss; ISS heritage |
| Rigid habitats: 3 Starship-derived pressure hulls, about 1,000 m³ each [M] | 150–180 t outfitted | Starship-steel hulls are the cheapest pressurised volume available |
| Inflatable habitats: 4 LIFE-class, 283 m³ each [V] | about 100 t [M, per-unit mass not published] | Crew quarters, galley, gym |
| Life support, consumables and spares | 100–150 t | 98% water recovery [V]; oxygen loop resupplied |
| Power: arrays, batteries and distribution | 70–100 t | Section below |
| Thermal: radiators and loops | 40–60 t | About 3,200 m² at 8–12 kg/m² [M] |
| Robotics: gantries, arms, dexterous robots, free-flying inspectors | 100–120 t | Section below |
| Berthing and docking: 8 large berths, 4 crew ports | 40–60 t | |
| Attitude control: about 16–24 large control moment gyroscopes, plus 2 × 60 kW SEP reboost units | 50–70 t | |
| Workshops: pressurised machining, inspection, NDE, additive manufacturing | 50–100 t | |
| Storm shelter: about 300 m² of water wall at 20 g/cm² | about 60 t | The water doubles as stores |
| **Yard total** | **about 1,160–2,530 t; planning value about 2,000 t** | |
| Depot, co-orbiting | 300–600 t dry for 2,000–6,000 t of propellant | Tanks at about 5–10% of propellant mass [M, subsystems.md] |

#### Crew and volume [P]

- **Crew:** 16 permanent, on 6-month rotations, so 32 people a year, which
  is 8 Dragon flights or 1–2 crewed Starship flights once human-rated.
- **Surge:** to 24–32 during ship commissioning.
- **Volume:**
  - pressurised about 4,000 m³ (3 × 1,000 + 4 × 283 = 4,130 m³ [C]);
  - habitable about 1,500–2,000 m³.
  - At 16 crew that is about 100 m³ per person, against the ISS's 388 m³ for
    about 7, about 55 m³ each [V, C].
- **Why people at all:**
  - Robots close every fast loop [V, autonomy-and-environment.md].
  - Crews handle diagnosis, repairs the robots can't do, and acceptance
    sign-off.
  - Two hours' latency to a human decision is not acceptable for a
    2,400 t fuelled ship.

#### Power [C from P loads]

| Load | Average |
| --- | --- |
| Habitat, life support and labs (24 people × about 15 kW) | 360 kW |
| Robots and gantries | 200 kW |
| Workshops: welding, machining, NDE, additive | 300 kW |
| Ship checkout, including SEP firing tests | 300 kW |
| Avionics, communications, guidance | 50 kW |
| Margin (25%) | 300 kW |
| **Yard total** | **about 1.5–1.6 MW** |
| Depot cryocoolers, on the depot's own arrays | 30–90 kW (below) |

**Arrays.** The yard is sunlit 61.6% of each orbit at 450 km with the worst
β angle [C]. With a 90% battery round trip, average load P needs P × (0.616
+ 0.384 / 0.9) / 0.616 = 1.69 P from the arrays [C].

- At end of life that is about **2.7 MW**. Allowing 15% degradation, it is
  **3.2 MW** at beginning of life [C].
- At 1,361 W/m² × 30% cell efficiency × 0.85 packing, the area is about
  **9,200 m²** [C].
- That is about 150 iROSA-class wings at about 20 kW [V], or 12–16 wings
  of 200–250 kW. Wings that large have not flown [M]; they are a gate.
- At 100–150 W/kg [M] the arrays weigh 20–30 t [C].

**Batteries.** A 36-minute eclipse at 1.6 MW takes 0.96 MWh [C].

- At 35% depth of discharge, for about 5,600 cycles a year [M, subsystems.md
  range], the bank holds 2.7 MWh.
- At 150 Wh/kg that is about **18 t** [C].

**Fission doesn't fit here.** It is 40–100 kWe class [V], and a reactor in a
crewed LEO yard adds launch-safety and regulatory work for 3–6% of the load.
Keep it for the lunar surface.

#### Radiators [C]

- εσT⁴ is 296–390 W/m² per side at 280–300 K, with ε = 0.85 [C, the
  subsystems.md method].
- Earth's infrared and albedo in LEO cut that to about 250 W/m² net per side
  [M].
- Two-sided panels then need about **3,200 m²** for 1.6 MW.
- The habitat loop rejects cold. Workshop and cryocooler heat should be
  rejected warmer to save area, as
  [tycho-assessment.md](tycho-assessment.md) argues.

#### Robots, gantries and arm budgets [P, with C checks]

| Machine | Count | Reach | Rated payload | Heritage |
| --- | --- | --- | --- | --- |
| Bay gantry crane (on rails, spanning a bay) | 2 per heavy bay | 50 m span × 250 m travel | 150 t at 2 cm/s or less | Physics `Slider` joints in our simulation; a new flight design |
| Walking arm, Canadarm2-class | 6 | 17 m [V], 7 joints [V] | About 100 t berthing [M, secondary sources for Canadarm2's 116 t] | Canadarm2 |
| Long-reach arm | 2 | 30–40 m | 150 t, slow | New; scaled Canadarm2 |
| Dexterous robot, Dextre-class | 2–4 | Two 3.5 m arms [V] | Orbital replacement units | Dextre |
| Free-flying inspectors | 4–8 | n/a | Cameras, lidar, eddy-current probes | Astrobee-class [M] |

**Momentum check [C].** A 150 t module moving at 2 cm/s, 50 m from the
yard's centre of mass, carries 150,000 N·m·s of angular momentum.

- That is about **8 times** the ISS's four CMGs at about 4,760 N·m·s each
  [M, subsystems.md].
- So the yard needs a CMG set of about 10⁶ N·m·s, which happens to be
  `verse_lagrange2::factory::CMG_CAPACITY` [V, our code]. Or it moves in
  opposed pairs.

**Base reaction [C].** The yard's transverse inertia is about 2,000 t ×
(400 m)² / 12 ≈ 2.7 × 10¹⁰ kg·m².

- A 30° swing of that module turns the yard by (m r² / I) × 30° ≈ **0.4°**.
- At the 800 t Lagrange 2 factory, the same swing would turn it many
  degrees.

#### Docking, berthing and bays [P]

- **Eight large berths:** for 9 m-diameter cargo and tanker vehicles,
  berthed by arm rather than docked.
- **Four crew ports:** IDSS docking ports for Dragon, Starliner and later
  crewed Starship. One capsule stays docked as a lifeboat at all times.
- **Two heavy bays:**
  - each 250 m long with a 40 m clear envelope;
  - each holds a ship of up to about **600 t dry**;
  - a ship is fuelled at the depot, so a bay never holds more than its
    dry mass plus pressurant.
- **One precision bay:** 60 m, shaded and thermally stable, for telescopes
  and optics (section 3.4).
- **Largest ship:** about 250 m long and 2,400–3,000 t wet after fuelling at
  the depot. Mass is set by the propellant logistics; length is set by the
  bay.

#### Propellant depot [P with C]

- **Capacity:** 2,000–6,000 t of methalox. At the upper end that is 4,700 t
  of oxygen and 1,300 t of methane, about **7,200 m³** [C, at densities of
  1.14 and 0.42 t/m³ M].
- **Heat leak:** about 3,500 m² of tank surface at 0.5–1 W/m² in LEO, with
  insulation and a sunshade [M]. That is **1.75–3.5 kW** [C].
- **Passive boil-off:** 0.012–0.024% a day, or about 4–9% a year [C, at a
  latent heat of 213 kJ/kg for oxygen M].
- **Zero boil-off:** removing that heat at 90–110 K, at 15–25 W of input per
  watt lifted [M], needs **26–88 kW** of cryocooler power [C].
- **The gap:** flight cryocoolers today lift watts to tens of watts at that
  temperature [M]. Kilowatt-class lift is a gate.
- **Hydrogen:** liquid hydrogen at 20 K needs about 5–10 times more input
  per watt [M]. It stays out of the baseline, as the strategy says.

#### Shielding and crew dose

| Location | Dose rate | 180-day tour | Tours in a 600 mSv career [V, [NASA OCHMO](https://www.nasa.gov/wp-content/uploads/2023/03/radiation-protection-technical-brief-ochmo.pdf)] |
| --- | --- | --- | --- |
| LEO Yard, about 30° | 0.4 mSv/day or less, planned [P, below the ISS's 0.4–1.0 V] | 72 mSv or less [C] | About 8 |
| ISS, 51.6° | 0.4–1.0 mSv/day [V] | 72–180 mSv [C] | 3–8 |
| NRHO or L2 | About 1.8 mSv/day [V, cruise measurement] | 324 mSv [C] | **1** |

- **Strategy:** use the magnetosphere as the shield. Add a water-walled storm
  shelter of about 20 g/cm², and MMOD protection on every habitat.
- **No bulk shielding.** Thick shields cut GCR dose slowly [V,
  tycho-assessment.md]. At L2 they would cost hundreds of tonnes at 3–5
  times the launch price. That is the main reason crewed work stays in LEO.
- **At the NRHO port:** visits of 30–60 days or less [P], with a storm
  shelter.

#### Artificial gravity: not worth it at this scale

- **Tour length:** six-month tours with exercise are ISS-proven [M].
- **Cost:** a spinning section strong enough to matter is a large rotating
  joint and a dynamic disturbance next to precision assembly. Tycho's rings
  hold about 3.9 × 10¹³ N·m·s [C, tycho-assessment.md].
- **What to fly instead:** a short-arm human centrifuge as a research
  payload [P].
- **When to revisit:** for permanent crews or tours over a year.

#### Station-keeping budget [C, P]

| Item | Budget a year |
| --- | --- |
| Drag reboost | 1.4–8.5 m/s, about 0.4–2.2 t of Xe or Kr at 2,000 s |
| Debris avoidance | 1–5 m/s [M] |
| Momentum management | Torque-equilibrium attitude, as the ISS flies [M], so CMGs desaturate with gravity-gradient torque rather than propellant |
| **Planning total** | **15 m/s a year**, carried on SEP with chemical thrusters for avoidance |

### 3.4 Production

#### Ship classes

| Class | Dry | Wet at departure | Length | Propulsion | Departure Δv from the LEO Yard | From the NRHO port | Bay time [P] |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Crewed Mars transfer vehicle** | 400 t: habitat 150 t including water shielding; tanks and structure 150 t; engines and systems 100 t [P] | 2,355 t [C] | About 150–200 m | Methalox, 4–6 Raptor-Vacuum class [M] | 3.62 km/s, then 2.9 km/s on board; 6.5 km/s total [C] | 0.75 km/s; wet 1,079 t [C] | 4–6 months |
| Cycler habitat (Aldrin-type), with a kick stage | 300–500 t hab + 60 t stage | 1,390–2,160 t [C] | About 100 m | Methalox kick stage; then tens to hundreds of m/s a year of trims [M] | 4.97 km/s [C] | About 2.1 km/s [C+M] | 6–9 months; one every few years |
| Propellant barge (cislunar tanker) | 60–200 t | 1,000–3,000 t of capacity | 60–120 m | Methalox; or pushed by an SEP tug | 3.5 km/s to the NRHO [M] | n/a | 1–2 months |
| Depot module | 100–300 t | 2,000–6,000 t stored | 60–150 m | Station-keeping only | Moved by a tug | n/a | 2–3 months |
| SEP cargo tug (gated) | 50–100 t | +20–60 t Xe or Kr | Arrays 100–200 m across | 0.3–1 MW Hall clusters, 25–80 AEPS-class 12 kW units [C, 0.3–1 MW ÷ 12.5 kW] | About 7 km/s spiral to L2 [M], 6–9 months | n/a | 2–4 months |
| Large space telescope | 50–150 t; 15–30 m segmented aperture | Plus a 100–300 t stage | 30–60 m | Chemical stage or SEP tug to Sun–Earth L2 | About 3.2–3.4 km/s [M] | Under 1 km/s [M] | 6–12 months, precision bay |
| Belt prospector | 20–100 t | 50–300 t | 20–50 m | SEP, Isp about 2,000–3,000 s [M] | 4.88 km/s departure, then several km/s more [C] | About 2 km/s [C+M] | 2–3 months |

**Why build any of these in a yard?** One Starship carries a 9 m-wide,
100–150 t payload [V, M]. Starship already covers Mars cargo that fits that
envelope.

The yard's market is what doesn't fit in one ship:

- shielded crewed transfer vehicles;
- multi-thousand-tonne tank farms;
- megawatt array tugs;
- segmented apertures larger than the fairing;
- anything that must be inspected, proof-tested and commissioned as a whole
  before it leaves.

#### A reference production year at about 200 launches [C from P rates]

| Output | Hardware | Propellant and other cargo |
| --- | --- | --- |
| 3 crewed MTVs | 1,200 t | 5,868 t (3 × 1,956) |
| One-third of a cycler (one every 3 years) | 153 t | 445 t |
| Supply to the NRHO port: 1,000 t delivered | (included) | 1,000 t of cargo plus about 3,600 t of LEO propellant to deliver it [C, section 2.2] |
| 1 telescope and its stage | 100 t | 200 t |
| 2 SEP tugs | 200 t | 60 t Xe or Kr |
| Depot growth and spares | 500 t | |
| Yard logistics | 300 t | |
| Boil-off and reserve, 10% | | about 1,100 t |
| **Total** | **about 2,450 t** | **about 12,300 t** |
| **Launches** | 17–25 | 82–123 |

**Plus crew flights:** about 2–8.

**So the reference year is about 105–155 launches,** within the 200-launch
sizing [C].

**Propellant binds before hardware:**

- Each MTV needs 4.9 t of propellant per tonne of dry ship [C].
- At 200 launches the yard could build 4–5 MTVs a year.
- At 400 launches, with two more bays, about 10.

The strategy's cargo arithmetic ([cislunar-strategy.md](cislunar-strategy.md))
reaches the same point. A 1,000 t-wet ship needs about 1,000 t delivered.
Here a 2,355 t MTV needs about 2,355 t in LEO, which is 16–24 launches.

## 4. What it takes to build

### 4.1 Bootstrap sequence

This follows the strategy's dates where they overlap; every date is [P].

| Phase | Dates | What happens | Gate to proceed |
| --- | --- | --- | --- |
| 0. The strategy's early phases | 2027–2030 | Ground qualification; orbital assembly demonstrator; the LEO work station of 5–15 t at **about 30°** | Repeated robotic assembly; orbit choice fixed (this note) |
| 1. Yard core | 2030–2032 | First Starship-derived habitat, 300 kW of arrays, 2 walking arms, 1 gantry, crew-tended by 4 on Dragon; 8–12 hardware launches | 100+ robotic berthings of 10–50 t modules; crew-tended operation for 6 months |
| 2. Cryogenic depot pathfinder | 2030–2033 | After the Starship transfer demo and LOXSAT: a 500–1,000 t depot module with active cooling, co-orbiting | Vehicle-to-vehicle transfer of 100 t or more, repeated 10 times; gauging within ±2%; 180 days of storage at under 0.05% a day loss [P] |
| 3. Full yard | 2032–2036 | Two heavy bays, 3.2 MW of arrays, 16 permanent crew, the depot grown to 2–6 kt; about 20–30 hardware launches | First 300–600 t dry ship assembled, proof-tested and fuelled |
| 4. NRHO port | 2031–2034 | The strategy's lunar logistics station, sized as a top-up port: 10–30 t dry plus 200–1,000 t of tanks | Repeat cargo and propellant delivery; 3 or more top-ups of a departing vehicle |
| 5. First heavy ship | 2035–2037 | A first MTV-class or cycler-class vehicle departs | Commissioning, a departure burn, the mission trajectory |
| 6. Production rate | 2037–2040 | 3–5 heavy ships a year | Measured throughput; launch cadence of 150–200 a year held |
| 7. L2 heavy yard (conditional) | 2040 onward | Move or duplicate heavy assembly at an L2 halo | Lunar fuel delivered to L2 beats the Earth alternative (section 2.4) |

**Launches to build the yard itself:**

- about 20–30 hardware flights for the yard;
- about 5–8 for the depot's dry mass;
- 8–16 crew flights over the build.

**Cargo:** about 2,000 t of yard plus 300–600 t of depot [C].

**At L2 the same 2,300–2,600 t would need about 60–145 launches** [C: 23–26
launches at 100 t each, × 2.7–5.5], before any crew flights.

### 4.2 Cost, by range [P; method C]

| Item | Range | Basis |
| --- | --- | --- |
| Yard hardware: bulk (truss, hulls, tanks, water, radiators), about 1,200 t | $1–6B | $1–5k/kg for Starship-steel and truss-class structure [P] |
| Yard hardware: systems (life support, robots, avionics, arrays, CMGs), about 300–500 t | $9–40B | $30–80k/kg, at the commercial end of space-systems cost; the ISS's all-in figure was about $357k/kg [C, tycho-assessment.md] |
| Development and integration | $5–12B | Strategy-scale team growing to several thousand [P] |
| Launch for the yard and depot | $0.5–4B | 25–40 flights at $20–100M [P] |
| **Capital total** | **about $20–60B** | $16–62B summed [C], rounded |
| Operations, launch (150–200 flights a year at $20–100M) | $3–20B/yr | Dominant; falls with price |
| Operations, crew transport (32 seats a year) | $0.5–2B/yr | Dragon seats about $55–70M [M]; less on crewed Starship |
| Operations, ground staff, engineering and spares | $1.5–3B/yr | |
| **Operations total** | **about $6–24B/yr** | |

**For comparison:**

- the ISS: about $150B for all partners [M, tycho-assessment.md], or $75B
  for the US through 2013 [V, NASA OIG];
- Tycho: $0.5–2 trillion [P, tycho-assessment.md].

**Ships are extra.** Their hardware is paid by the customer. A crewed MTV at
these unit costs is about $5–20B, mostly its systems [P].

### 4.3 Critical technologies and their gates

| Technology | Status today | Gate before scale [P] |
| --- | --- | --- |
| **1. Starship full reuse and cadence** | V3 flew once in May 2026, booster lost [V]; reuse at 100 t not yet shown [M] | 10 or more reflights of one ship at 100 t or more to orbit; 50 or more program flights in a year; pads for 150–200 a year licensed |
| **2. Cryogenic transfer and storage at scale** | Ship-to-ship transfer NET late 2026 [V]; LOXSAT 2026, unconfirmed [V] | Repeated transfers of 100 t or more; gauging within ±2%; 180-day storage under 0.05% a day; kW-class 90 K cryocoolers flown |
| **3. Robotic berthing and joining of 100 t modules** | Canadarm2 berths cargo vehicles [V]; OSAM-1 cancelled [V]; no robotic multi-module construction yet [V] | 100 or more berthings with fluid and power connections; proof-load of each joint; stuck-latch recovery; simulation-checked stages ([autonomy-and-environment.md](autonomy-and-environment.md)) |
| 4. Megawatt arrays | iROSA at about 20 kW per wing [V] | 200 kW-class wings deployed and retracted in LEO |
| 5. High-power SEP | PPE: 60 kW, 12 kW AEPS [V]; Gateway paused [V] | 300 kW cluster flown for 1 year (tug only; not on the critical path) |
| 6. Crew transport at 32 or more seats a year | Dragon in service [M] | Crewed Starship certified, or a sustained 8 Dragon flights a year |
| 7. Life-support closure | 98% water [V]; oxygen about 50% [V, older] | 75% or more oxygen recovery to cut resupply (optional) |
| 8. Lunar oxygen | Simulant only [V] | The strategy's resource gates; used on the surface first (section 2.4) |

### 4.4 Workforce [P]

- **In orbit:** 16 permanent crew, 24–32 at surge, rotating every 6 months.
- **Ground:** about 2,000–5,000 people across:
  - flight operations, around the clock;
  - robotics supervision;
  - module manufacturing and integration;
  - the depot;
  - mission design.
- The strategy's 300–600 people is the earlier, smaller plan.
- **Robot ratio:** about 5–10 robots per crew member. Ground supervisors
  approve stages, and robots close every fast loop [V,
  autonomy-and-environment.md].

### 4.5 Regulatory items

| Item | Notes |
| --- | --- |
| Launch licences (FAA Part 450) and pad environmental reviews | The binding constraint on cadence (section 3.1) [V for cadence approvals] |
| FCC licensing and the five-year disposal rule | Applies to the yard, the depot and every free-flyer [V, autonomy-and-environment.md] |
| Human spaceflight rules (FAA informed consent and crew rules) | For a privately crewed yard [M] |
| Outer Space Treaty Article VI authorisation and supervision | No complete US "mission authorisation" process exists for novel on-orbit activities [M] |
| Export control (ITAR, EAR) | Robotics, propulsion and partners |
| Planetary protection | For Mars-bound ships [M] |
| Lunar resources | The US Commercial Space Launch Competitiveness Act of 2015 and the Artemis Accords allow extraction [M] |
| Nuclear (SPD-6) | Only if fission is used; it is not in the baseline |

## 5. Comparison

| | ISS (actual) | Cislunar-strategy L2 shipyard targets | **"Max today": LEO Yard + NRHO port** | Tycho (tycho-assessment.md) |
| --- | --- | --- | --- | --- |
| Location | LEO, 51.6° [M] | L2 halo | LEO, about 30°, 450–500 km; port in the L2 NRHO | Main Belt |
| Mass | 419,725 kg [V] | 50–150 t initial; expanded sized from loads [P] | About 2,000 t yard + 300–600 t depot [P] | About 6–17 Mt [C] |
| Crew | About 7 [M] | Robotic; crew optional later [P] | 16 permanent, up to 32 [P] | 15,000 [fiction] |
| Pressurised volume | About 916 m³ [M]; 388 m³ habitable [V] | Minimal | About 4,000 m³ [C] | About 6 million m³ in the rings [C] |
| Power | 75–90 kW [V], more with iROSA [V] | 0.5–2 MW initial, 2–10 MW expanded [P] | 1.6 MW average; 3.2 MW arrays [C] | 0.5–1.5 GW [P] |
| Largest ship built | n/a (assembled itself) | 100–300 t wet initial; up to 1,000 t wet [P] | About 600 t dry, 2,400–3,000 t wet [P, C] | 150 kt-wet capital ships; Nauvoo at Mt scale [P] |
| Throughput | About 32 t/yr of assembly [M] | Up to 1,000 t/yr [P] | About 2.5–7.5 kt/yr of hardware, 12–21 kt/yr of propellant [C] | About 75 kt/yr of structure [P] |
| Cost | About $150B all-partner [M]; $75B US through 2013 [V] | Not costed | About $20–60B capital, $6–24B/yr [P] | $0.5–2 trillion [P] |
| Timeline | Assembled 1998–2011 [M] | First heavy ship 2034–2036; 1,000 t ship 2039–2042 [P] | Core 2030–2032; full 2034–2036; production rate 2037–2040 [P] | Earliest about 2080; 2080–2100 likely [P] |

**This yard is about 5 ISS masses.** It builds ships 2–3 times the strategy's
largest, and it is three to four orders of magnitude below Tycho. That gap is
the difference between Earth-launched and asteroid-made mass.

## 6. Sensitivities: what would change the answer

| Change | Effect on the yard | Effect on location |
| --- | --- | --- |
| **Starship at $100–200/kg** ($10–20M a flight [M, secondary]) | Operations fall to about $3–8B/yr; cadence, pads and crew become the only limits | **Strengthens LEO.** The L2 multiplier is a launch-count ratio and doesn't shrink. Cheaper Earth propellant pushes the date when lunar fuel pays further out |
| Starship stuck at about 35–60 t or low reuse | The yard halves; tanker counts per ship double | Still LEO; L2 even worse |
| **Pads capped near today's 146 a year for all customers** [V] | The program gets perhaps 50–80 a year: about 1–2 MTVs a year, one heavy bay | No change |
| **Lunar regolith oxygen succeeds** | Cheaper landers and surface operations; the NRHO port grows | **Little change** (section 2.4): Earth fuel for the landers cancels the gain, except for hydrolox landers to low lunar orbit |
| **Lunar water ice produced at scale** | Full hydrolox from the Moon; departure propellant at the NRHO or L2 could fall below Earth-delivered cost | **Moves the heavy yard to L2** (phase 7): build the hulls in LEO, then integrate and fuel at L2. Gate: delivered cost at the port below the LEO price × 2.7 or so [C] |
| **Nuclear thermal propulsion** (Isp about 900 s [M]) | MTV mass ratio falls from 5.86 to about 2.1 for 6.5 km/s [C, vₑ = 8.8 km/s]; ships get much smaller | LEO build, but **depart from a higher "nuclear-safe" orbit** [M]; liquid hydrogen storage becomes a gate; the depot becomes a hydrogen depot |
| **Megawatt SEP tugs** | Cheap dry-cargo delivery to the NRHO and L2: about 70% delivered fraction instead of about 20–35% [C] | **Narrows** the L2 penalty for hardware, not for crew or cryogenic propellant |
| **Crewed Starship certified** | 32 seats a year on 1–2 flights; crew can grow to 30–50 | Easier crewed visits to the NRHO, but the dose problem remains |
| Large solar event or a debris strike | A storm shelter in LEO is enough; MMOD failure is the LEO-specific risk | A debris-driven loss is the main argument for cislunar placement |
| **Gateway paused** [V] | No government anchor at the NRHO | Favours LEO for the next decade |

## 7. Mapping to our simulation

`crates/verse-lagrange2` models an 800 t factory deck
(`factory::FACTORY_MASS`) [V, our code]:

- four SO-101-shaped arms at 100× scale;
- a CMG set of 10⁶ N·m·s (`CMG_CAPACITY`);
- stage-checked assembly with link-loss, stuck-latch, reset, solar-event and
  debris holds;
- a 54 t sample tug from the CAD feed;
- a test burn departing L2.

The field is left out because the L2 tidal gradient is about 5 × 10⁻¹¹ s⁻²
[C, sim-mapping.md].

**What this recommendation needs from it.** The yard is in LEO, so either:

- parameterise the factory by environment; or
- add a sibling `verse-leo-yard` zone that reuses the arms, the sequencer and
  the documents [P].

| Need | Why | Candidate test |
| --- | --- | --- |
| Gravity gradient and orbital dynamics (Clohessy–Wiltshire / Hill) in the factory frame | The LEO tide is about 70,000 times L2's (3.8 × 10⁻⁶ vs 5 × 10⁻¹¹ s⁻²) [C]; a released part drifts tens of centimetres in 30 s | `released_part_drifts_as_hill_predicts` |
| Torque-equilibrium attitude and CMG desaturation by gravity gradient | How the ISS manages momentum without propellant [M] | `yard_holds_torque_equilibrium_attitude` |
| Eclipse cycling: 36 minutes in each 94 [C] | Battery depth of discharge, thermal distortion of long trusses | `eclipse_drains_batteries_within_dod` |
| Drag and J2 | Reboost budget; the plane's precession sets departure windows | `drag_reboost_matches_budget` |
| Rail gantries (`physics::Slider`) and walking arms that change base | The yard's heavy handlers | `gantry_move_within_cmg_budget` (150,000 N·m·s per move [C]) |
| Scale: about 2,000 t yard, I ≈ 2.7 × 10¹⁰ kg·m² [C] | The inertia regime moves toward Tycho's: arm reactions shrink, flexibility and contact loads dominate | Reuse `waldo_swing_turns_tycho_as_predicted` with yard values |
| Propellant document and depot | Transfer, boil-off and gauging, refused when a schedule outruns supply | `depot_transfer_conserves_mass_with_boiloff` |
| Fuelling away from the crew | The strategy's separation rule | `ship_is_fuelled_only_at_the_depot` |
| A crew dose ledger | Per-location dose rates (table in section 3.3) | `tour_dose_stays_under_budget` |
| Departure | The ship departs LEO with the 3.62 km/s burn [C], or via a high-orbit top-up | `leo_departure_obeys_the_rocket_equation` |

**Where L2 stays.** The existing Lagrange 2 zone remains right for two
things:

- the NRHO or L2 port: top-ups, short assembly of smaller ships, and
  departures;
- the conditional phase-7 heavy yard.

Its halo, station-keeping and departure tests carry over unchanged. The
800 t deck with four arms is a fair model of the **NRHO port**, not of the
biggest yard.

## Sources

**Verified while writing (October 2026):**

- Starship:
  - [Wikipedia, SpaceX Starship](https://en.wikipedia.org/wiki/SpaceX_Starship):
    Block 3 100 t to LEO, 1,600 t ship propellant, Raptor Vacuum 380 s,
    $100M expendable infobox figure.
  - Flight 12: [NSS](https://nss.org/?p=119535) and
    [Pulse 2.0](https://pulse2.com/spacex-starship-flight-12-marks-first-successful-v3-test-flight-and-major-advancement-for-mars-program/amp/).
  - [Starship Propellant Transfer Demonstration](https://en.wikipedia.org/wiki/Starship_Propellant_Transfer_Demonstration):
    NET late 2026 and the "ten-ish" tankers.
- Launch cadence approvals:
  - [Drive Tesla, Starbase 25 a year](https://driveteslacanada.ca/news/faa-approves-up-to-25-starship-launches-per-year-from-starbase/);
  - [NASASpaceflight, 39A and SLC-37](https://www.nasaspaceflight.com/2026/01/faa-advances-approval-44-starship-launches-39a/).
- Arrays: [Redwire iROSA](https://ir.redwirespace.com/news-events/press-releases/detail/152/redwire-successfully-delivers-fourth-pair-of-roll-out-solar).
- Gateway and its power and propulsion element:
  - PPE and thrusters: [NASA](https://www.nasa.gov/centers-and-facilities/glenn/nasa-starts-up-gateways-power-system-for-first-time),
    [NTRS SP2022-438](https://ntrs.nasa.gov/api/citations/20220007037/downloads/SP2022_438_CASSADY.pdf)
    and [AEPS full power](https://www.globenewswire.com/news-release/2019/11/08/1944195/0/en/Advanced-Electric-Propulsion-Thruster-for-NASA-s-Gateway-Achieves-Full-Power-Demonstration.html);
  - Gateway paused: [Wikipedia, Lunar Gateway](https://en.wikipedia.org/wiki/Lunar_Gateway).
- Life support:
  - [Space.com, 98% water](https://space.com/astronaut-pee-iss-water-recycling-98-percent-milestone);
  - [ECS abstract, about 50% oxygen recovery](https://ecs.confex.com/ecs/236/webprogram/Paper127652.html).
- Cryogenic storage: [Rocket Lab, LOXSAT](https://rocketlabcorp.com/missions/launches/loxsat/)
  and [Gunter's Space Page](https://space.skyrocket.de/doc_sdat/loxsat.htm).
- Fission:
  - [Wikipedia, Kilopower](https://en.wikipedia.org/wiki/Kilopower);
  - [ANS, 100 kWe fission surface power](https://www.ans.org/news/2026-01-14/article-7674/nasa-doe-solidify-collaboration-on-a-lunar-surface-reactor/).
- Lunar oxygen: [Blue Origin, Blue Alchemist](https://www.blueorigin.com/news/blue-alchemist-hits-major-milestone-toward-permanent-sustainable-lunar-infrastructure)
  and [Gigazine, Air Pioneer](https://gigazine.net/gsc_news/en/20260412-blue-origin-moon-dust-to-fresh-air).
- Inflatables: [SatNow, LIFE burst test](https://satnow.com/news/details/1291-sierra-space-announces-pioneering-full-scale-burst-test-of-expandable-space-station-module)
  and [The Space Bucket](https://thespacebucket.com/full-scale-life-habitat-testing-is-underway/).
- Radiation:
  - [SwRI, Curiosity RAD cruise dose](https://www.swri.org/newsroom/press-releases/swri-led-team-calculates-the-radiation-exposure-associated-trip-mars);
  - [Universe Today, ISS comparison](https://www.universetoday.com/articles/human-voyages-to-mars-pose-higher-cancer-risks);
  - [NASA OCHMO](https://www.nasa.gov/wp-content/uploads/2023/03/radiation-protection-technical-brief-ochmo.pdf).
- NRHO:
  - [MERL TR2022-114](https://www.merl.com/publications/docs/TR2022-114.pdf):
    about 1.14 m/s/yr in simulation;
  - [Cislunar Space guide](https://www.cislunarspace.cn/en/cislunar-orbits/nrho/gateway-cases/):
    a secondary source for NRHO-to-pole Δv;
  - Parrish et al. and Folta et al. via [astrodynamics.md](astrodynamics.md).

**From memory or secondary reporting, to verify [M]:**

- the Δv map values;
- Starship price targets;
- Starship dry mass;
- Canadarm2's payload rating;
- Dragon seat price;
- cryocooler performance;
- tank heat leak;
- thermospheric density;
- rescue times;
- regulatory details.

Each is marked where used.
