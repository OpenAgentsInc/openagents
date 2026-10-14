# Subsystems

First-cut sizing knowledge for each spacecraft subsystem, in our own words.
Ranges are typical of the literature and most are marked [M]: check them in
the New SMAD, Fortescue, or Brown (see [reading-list.md](reading-list.md))
before a range becomes a check in the spacecraft document. See
[the README](README.md) for the marks.

## Mass budget

Dry mass fractions by subsystem for uncrewed spacecraft, as a share of dry
mass without payload margin [M, from memory of New SMAD and Brown tables]:

| Subsystem | Typical share of dry mass |
| --- | --- |
| Payload | 15–40% |
| Structure and mechanisms | 15–25% |
| Thermal control | 2–6% |
| Electrical power (arrays, batteries, distribution) | 15–30% |
| Attitude determination and control | 4–10% |
| Communications | 2–6% |
| Command and data handling | 2–6% |
| Propulsion (dry: engines, tanks, lines) | 3–10% |
| Harness (cabling) | 3–8% |

Propellant is budgeted separately from the rocket equation. For a large ship
with a big Δv, propellant dominates: at 3 km/s and I_sp 320 s it is about
62% of the initial mass [C, 1 − 1/2.601].

Tank mass scales with propellant. Pressure-fed and pump-fed tanks commonly
weigh a few percent to about 10% of the propellant they hold, depending on
pressure and material [M].

**For us:** the spacecraft document's mass check can compare each
subsystem's share with a range, warn outside it, and refuse only hard
violations (negative margin, center of mass off the thrust line beyond a
limit).

## Power

**Solar arrays.** The solar flux at 1 AU is about 1,361 W/m² [M, the solar
constant; check against a current TSI value]. Earth–Moon L2 is at 1 AU, so
the flux is the same as near the Earth.

- Array power at beginning of life: P = S η A cos θ × packing factor.
  Cell efficiency η is about 28–32% for flown triple-junction cells [M];
  packing and wiring losses take roughly 10–20% more [M].
- Degradation over life from radiation and thermal cycling: a few tenths of
  a percent to a few percent per year [M]. Size for end-of-life.
- Specific power of arrays: tens to over 100 W/kg depending on technology;
  rigid panels are at the low end, roll-out flexible arrays at the high end
  [M].

**Batteries.** Lithium-ion, with usable specific energy around 100–200 Wh/kg
at the cell, less at the pack [M]. Depth of discharge is limited to extend
cycle life: often 20–40% for many thousand cycles in low Earth orbit, and
higher (60–80%) for the few eclipses of an orbit far from the Earth [M].

**Eclipses at L2.** A halo orbit at Earth–Moon L2 is mostly in sunlight but
can pass through the Earth's or the Moon's shadow. Eclipses last up to a few
hours [M]; the Gateway's 9:2 NRHO resonance was chosen partly to avoid long
Earth eclipses [V] (Parrish et al.). Battery capacity is sized for the
longest eclipse at the eclipse load.

**For us:** Lagrange 1 already renders sunlight at the right flux and fixes
array attitude. For Lagrange 2, the factory's arrays should be sized from
the arms' and gantries' power, and an eclipse should show as a battery
drain on the HUD.

## Thermal

Steady-state balance for a surface in sunlight:

absorbed solar + internal power = emitted infrared,
α S A_sun + Q = ε σ A_rad T⁴,

with α the solar absorptance, ε the infrared emittance, and σ = 5.670 ×
10⁻⁸ W/(m² K⁴) [M]. The ratio α/ε sets a surface's temperature in
sunlight: white paint (low α/ε) runs cold, bare aluminium (high α/ε) runs
hot [M].

- Radiators use low-α, high-ε surfaces (white paint, silvered Teflon or
  optical solar reflectors) and face away from the Sun. A radiator at about
  room temperature rejects a few hundred W/m² to deep space [C, ε σ T⁴ with
  ε = 0.85 and T = 293 K gives 355 W/m²].
- Multilayer insulation blankets cut radiative heat flow to a few percent
  of a bare surface [M].
- Electronics usually must stay roughly between −20 and +50 °C operating;
  propellant lines and tanks must not freeze (hydrazine freezes near +2 °C)
  [M].
- At L2, the Moon adds little heat at 64,500 km; the Earth adds less [M].
  Lunar infrared and albedo matter only for an NRHO near perilune.

## Attitude determination and control (ADCS)

**Sensors:** star trackers (arcsecond-level), Sun sensors, gyroscopes,
sometimes magnetometers (useless at L2, where the field is tiny) [M].

**Actuators:**

- **Reaction wheels** store angular momentum; torque is the rate of change
  of wheel momentum. Typical wheels hold from fractions of a N·m·s to a few
  hundred N·m·s and give torques up to about 1 N·m [M]. External torques
  accumulate momentum until the wheel saturates.
- **Control-moment gyroscopes (CMGs)** gimbal a spinning wheel; torque is
  ω_gimbal × h. They give far more torque per watt than wheels and suit large
  vehicles. The ISS uses four double-gimbal CMGs, each with about
  4,760 N·m·s [M]. Gimbal geometry has singular configurations that steering
  laws must avoid.
- **Thrusters** give large torques and are the only way to remove momentum
  far from Earth (no magnetic field for magnetorquers). Momentum dumping
  (desaturation) fires thruster pairs to unload wheels or CMGs, costing a
  little propellant and perturbing the orbit slightly unless the pairs are
  balanced.

**Disturbances at L2:** solar radiation pressure torque (center of pressure
offset from the center of mass) dominates; gravity-gradient torque is tiny
far from the Moon [M]. In the factory, the arms' reaction torques are far
larger than either; see [in-space-assembly.md](in-space-assembly.md).

**Sizing a wheel:** momentum capacity at least the disturbance torque times
the time between dumps, with margin. Slew torque: for a rest-to-rest slew of
angle θ in time t with a bang-bang profile, the peak torque is
4 I θ / t² [C, from θ = (1/4) α t²].

**For us:** a ship whose inertia grows from tonnes to hundreds of tonnes
during assembly needs gains that scale with inertia. A fixed-gain controller
tuned for the empty frame will be sluggish or unstable at the end, which is
a good visible failure for the demo.

## Communications

Link budget, in decibels:

received Eb/N0 = EIRP + G_r − L_space − L_other − 10 log₁₀(k T_s R),

with EIRP the transmit power plus antenna gain, L_space = 20 log₁₀(4π d / λ),
k Boltzmann's constant, T_s the receiver system temperature, and R the bit
rate. A link closes when Eb/N0 exceeds the modulation and coding
requirement plus a margin, commonly 3 dB [M].

- Free-space loss to L2 at X band (8.4 GHz) over about 449,000 km is about
  224 dB [C, 20 log₁₀(4π × 4.49 × 10⁸ m / 0.0357 m)].
- Ground stations for deep space use 34 m and 70 m dishes [M] (NASA's Deep
  Space Network).
- **The Moon blocks the line of sight only near the L2 point itself.** A
  halo orbit keeps Earth in view; a relay is needed only for a station at
  the point, a small Lissajous orbit, or anything on the far side's surface.
  See [astrodynamics.md](astrodynamics.md#earth-visibility-from-l2).
- Light time from L2 to Earth is about 1.5 s [C, 448,900 km / c].

## Propulsion

| Type | I_sp | Thrust | Notes |
| --- | --- | --- | --- |
| Cold gas (N₂) | about 60–75 s | mN to tens of N | Simple; Lagrange 1's pack uses 70 s [V, our own code] |
| Monopropellant hydrazine | about 220–235 s | 0.5 N to hundreds of N | Attitude and small maneuvers |
| Storable bipropellant (MMH or hydrazine with N₂O₄) | about 300–325 s | 400 N to tens of kN | Main engines of most spacecraft |
| LOX/methane | about 350–380 s in vacuum | large | Boil-off management needed |
| LOX/LH₂ | about 440–465 s in vacuum | large | Best chemical I_sp; hydrogen is bulky and boils off |
| Hall thruster (electric) | about 1,500–2,500 s | tens of mN to a few N | Power-limited |
| Gridded ion (electric) | about 2,500–4,000 s | tens to hundreds of mN | Power-limited |

All values [M]. Electric thrust is power-limited: thrust is about
2 η P / (I_sp g₀) for input power P and efficiency η, so a few newtons need
tens of kilowatts [C, the relation; values M]. An electric test burn of
hundreds of m/s on a large ship would last weeks, which suits a time-lapse
but not a live demo.

**For us:** the sample ship's main engine should be a bipropellant or
LOX/methane engine so the test burn finishes in minutes, with RCS thrusters
for attitude. Lagrange 1's `physics::ThrusterSet` already allocates thrust
across thrusters.

## Structures

- **Load factors:** launch loads are expressed as quasi-static accelerations
  in g, typically up to about 4–6 g axial and 1–3 g lateral, plus random
  vibration and acoustic loads [M]. **Parts launched separately and assembled
  in orbit only see these loads on their own**; the assembled ship sees only
  its own thrust (well under 1 g) and docking and berthing loads.
- **Stiffness:** launch vehicles require a minimum fundamental frequency of
  the payload, commonly some tens of Hz axial and roughly 10–20 Hz lateral
  [M]. A large assembled structure in orbit has far lower frequencies
  (Lagrange 1's array wings: 0.15 Hz [V, our code]), so control loops must
  stay well below the first mode to avoid exciting it.
- **Factors of safety:** commonly about 1.25 on yield and 1.4 on ultimate
  for flight structures verified by test, higher (around 1.6–2.0 on
  ultimate) when verified by analysis only, and higher still for pressure
  vessels and crewed structures [M; Sarafin and NASA-STD-5001 give the
  exact values].
- **Margin of safety:** MS = allowable / (applied × factor of safety) − 1;
  a positive margin passes [M, the definition].
- Trusses are efficient: high stiffness per mass. Graphite composite and
  aluminium are the common materials.

**For us:** the structural check in the spacecraft document can be a load
per joint and latch: during the test burn each latch carries the mass
outboard of it times the acceleration, plus bending from offsets. Each latch
has a rated load; margin of safety ≥ 0 with a declared factor of safety
passes.

## Mechanisms

- **Docking** joins two free vehicles: the active one flies in and soft
  capture (springs and dampers) absorbs relative motion before hard capture
  (structural latches) [M]. The International Docking System Standard
  defines a common interface [M].
- **Berthing** uses an arm to place one body against another, then latches
  it with a berthing mechanism; the ISS's Common Berthing Mechanism uses
  powered bolts after capture latches pull the halves together [M].
- **Latches** need capture envelopes (allowed misalignment and closing
  speed), preload to stay stiff, and torque margin on their motors.
  Lagrange 1's latch accepts parts within 1.6 m, 15°, 0.35 m/s, and
  0.05 rad/s [V, our code]; real berthing envelopes are centimeters and a
  few degrees [M].
- **Deployables** (arrays, booms, antennas) use hinges, springs, dampers,
  and latches; deployment shocks excite the structure.

**For us:** factory arms make berthing, not docking, the natural model. The
latch should demand tighter envelopes than the astronaut's, since arms can
position parts to centimeters.
