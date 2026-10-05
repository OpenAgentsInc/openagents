# Lagrange 1: the L1 construction station

Lagrange is a regression consumer for [Verse Engine](engine/roadmap.md).
Its generic physics mechanisms remain in `crates/physics`; orbital fields and
EVA rules remain specific to this Verse zone.

**Lagrange 1** is a Verse zone set on a small crewed station orbiting the
Sun–Earth L1 point, about 1.5 million km sunward of Earth. It is where a ship
frame begins: an astronaut in a maneuvering pack carries parts from a depot to
a keel jig. The physics is real where it matters at each scale, and each
approximation is named below.

Enter from the plaza's east arch (**L1 portal** on the map). The OpenAgents
app's Grid arch, which enters a station with its guides and panel drawn in
white and gray, is hidden for now
([the Grid's portal](mobile.md#the-grids-portal-to-lagrange-1)). Controls are in
[the zone guide](zones.md#lagrange-1-controls). The simulation lives in
[`verse-lagrange`](../../crates/verse-lagrange/), which uses the shared
zone-agnostic [`physics`](../../crates/physics/) crate for rigid bodies, fixed
stepping, and restorable world state; the scene, input mapping, and rendering
live in [`zones/lagrange/`](../../crates/verse/src/zones/lagrange/), drawn by
the renderer's physical path ([`pbr`](../../crates/verse-pbr/src/pbr/mod.rs)).

## Two clocks

| Clock | Rate | What it drives |
| --- | --- | --- |
| Local | Real time | The astronaut, pack thrust and propellant, parts, collisions |
| Orbital | 3,600 × real time (one hour per second) | The station's orbit about L1, station-keeping burns, Earth and Moon positions, mission day |

At one hour per second, one pass around the ~178-day Lissajous orbit takes
about 71 minutes of play. The two clocks are displayed separately; speeding up
the orbit never changes the local rigid-body step.

Local physics runs in fixed steps of 1/120 s (`PHYSICS_DT`). Frame time
accumulates and whole steps run, so the result does not depend on the frame
rate; the renderer interpolates between the last two poses. A frame runs at
most 12 steps (0.1 s); time beyond that is dropped and counted in
`Station::clock.dropped`. The orbit advances by each step's 1/120 s × 3,600.

`Station::save` returns a versioned `StationState` that restores and continues
bit for bit (with `serde_json`, enable `float_roundtrip`). `Station::record`
journals the pilot command and every action (`Input`: grab, release, fly-to,
stop), including NIP-MV operator commands, stamped with the physics tick;
`Station::replay` runs a journal from its start state.

## Orbit: the circular restricted three-body problem

[`orbit.rs`](../../crates/verse-lagrange/src/orbit.rs) integrates the full
nonlinear CR3BP in the rotating frame, in normalized units (1 AU, 1/n ≈ 58.1
days). The Sun and the Earth–Moon barycenter (EMB) move on circular orbits.

| Constant | Value | Source |
| --- | --- | --- |
| GM Sun | 1.32712440018 × 10²⁰ m³/s² | IAU 2015 nominal |
| GM Earth | 3.986004418 × 10¹⁴ m³/s² | WGS 84 |
| GM Moon | 4.9048695 × 10¹² m³/s² | DE440 |
| Separation | 1 AU = 1.495978707 × 10¹¹ m | IAU |
| Mass ratio μ | 3.0404 × 10⁻⁶ | Derived |

L1 comes from Newton's method on the collinear quintic: about 0.01001 AU (1.497
million km) from the EMB. Tests check that a body at rest there feels zero
rotating-frame acceleration.

Linearized about L1, Richardson's coefficient is c₂ ≈ 4.061. The in-plane
eigenvalue λ ≈ 2.533 gives an unstable e-folding time of about 23 days; the
in-plane and vertical frequencies (ω_p ≈ 2.086, ω_v ≈ 2.015) give periods of
about 175 and 181 days, bracketing the ~178-day periods flown by SOHO, ACE, and
DSCOVR. The station starts on a linear Lissajous orbit with about 48,000 km of
along-track and 50,000 km of out-of-ecliptic amplitude.

Integration is classical fourth-order Runge–Kutta with steps of at most 0.002
time units (about 2.8 hours). Without control, the Jacobi integral is conserved
to better than one part in 10⁹ over 60 days, and a 1 cm/s perturbation grows
past a million kilometers within 400 days. That is why real L1 missions fly
station-keeping maneuvers.

### Station-keeping

Every seven mission days the station evaluates the linear unstable mode, using
the left eigenvector of the linearized system, and applies the smallest
in-plane Δv that cancels it. Burns below 0.5 mm/s are skipped. The orbit stays
bounded over two years, spending about 8.6 m/s per year in tests (105 burns). Flight controllers use
nonlinear halo targeting and typically spend a few m/s per year; this
linear-mode controller is simpler, which is why it uses a modest orbit. Burns
appear as brief glows on the truss-tip thruster pods and in the HUD.

### What you see in the sky

- The **Sun** stands 30° above the −Z axis (see [Station attitude](#station-attitude))
  with its true angular diameter (~0.54°), in physical luminance with limb
  darkening. The fixed solar arrays face −Z and the radiators stay edge-on.
- The **Earth** is 30° below +Z, about 0.49° across and nearly full because L1
  sees its day side. It is a textured globe (Blue Marble surface and clouds)
  that turns once per day of mission time, lit at the true phase, with
  Rayleigh haze, ocean glint, and the Moon's shadow during a solar eclipse. A
  cyan reticle marks it. As the station moves around its orbit, the Earth
  drifts a few degrees across the sky, following the Sun–Earth–vehicle angle.
- The **Moon** circles the Earth once per synodic month (29.53 days), inclined
  5.1° to the ecliptic, appears within about 15° of the Earth, and always
  shows a nearly full disc from L1. It is tidally locked, textured from the
  LRO maps, and shaded with lunar-Lambert reflectance and an opposition surge.
- **Stars** are the Yale Bright Star Catalogue to magnitude 6.5 with the Milky
  Way behind them, at their real positions and colors. At a sunlit exposure
  they are far below black, as in every photograph taken in sunlight; the art
  camera preset raises them.

The Earth's position includes the Moon's reflex motion about the barycenter.
The sky is drawn at infinity before the station, in explicit distance order,
so the Moon can pass in front of the Earth. The mission epoch is 2026
September 27, 12:00 UTC; the star field and the Earth's rotation and axial tilt
follow low-precision ephemerides (Meeus) from it. Data sources and licenses are
in [`assets/lagrange/PROVENANCE.md`](../../crates/verse/assets/lagrange/PROVENANCE.md).

### Light and camera

Sunlight at L1 is about 130,000 lux. Earthshine is about 5 × 10⁻⁶ of that, so a
face turned from the Sun is lit almost only by sunlight bounced off the
station itself. The renderer works in physical units:

- **Materials:** white thermal paint, bare and brushed aluminium, crinkled
  aluminized-Kapton insulation, solar cells under a thin-film coated cover
  glass, a gold visor, suit fabric with sheen, a silvered radiator plate, and
  safety paint, with albedos from measured solar absorptance. A solar wing's
  glint is its cover glass alone, a sharp image of the Sun; the textured,
  antireflection-coated cells beneath return about 1% of the light, scattered
  widely, so the wing stays blue around the glint instead of washing out into
  a broad gray sheen.
- **Shadows:** a sun shadow map whose penumbra follows the Sun's 0.27°
  radius, so edges are sharp at contact and soften with distance.
- **Bounce light:** irradiance probes baked on a worker thread from one diffuse
  bounce of sunlight, baked again when parts rest in the rack or latch in the
  jig, plus baked ambient occlusion.
- **Camera:** a helmet camera at a sunny-16 exposure (EV 15) with the clamped
  automatic exposure of a small action camera, local exposure that lifts
  shadows, energy-conserving bloom, a six-blade diffraction pattern around
  the Sun, faint lens ghosts, vignetting, and grain. The art preset brightens
  shadows and stars.
- **Effects:** tethers and depot lines are tubes along their ropes, wrapped
  on what they touch, with a clip on an unclipped end; the solar
  wings bend with their structural modes; cold-gas plumes show only a brief
  glint of sunlit condensate; ice flakes from the habitat vent drift
  anti-sunward under radiation pressure and glint as they tumble. Every effect
  follows from the physics tick, so a replay shows the same frames.

On a display with extended dynamic range (an iPhone or Mac with an XDR or
OLED screen), the Sun, glints, and sunlit white render brighter than reference
white. The renderer draws to an RGBA16F extended linear sRGB surface and bends
the tone curve's shoulder toward the screen's current headroom, so everything
below the shoulder looks exactly as it does in standard range. Launch iOS with
`--sdr` or set `VERSE_SDR=1` on the desktop to force standard range, and run
`cargo run --release -p verse --example hdr_probe` to check the extended
output offscreen.

Guides (latch outlines, routes, the refill ring, the reticle, and the forces
overlay) keep their display colors. Adapters that cannot render a
floating-point target tone-map each draw directly and skip post-processing.

## Local physics

Near the station, [`station.rs`](../../crates/verse-lagrange/src/station.rs)
uses the CR3BP linearized about L1 in SI units, including the Coriolis terms of
the rotating frame. At station scale the tidal field is about 10⁻¹¹ m/s², true
microgravity, but it is integrated so free parts obey the real field.

Scene axes are the station's body axes. With the attitude below, the Sun is
30° above −Z and the Earth 30° below +Z. Before that pitch, −Z points at the
Sun, +Z at the Earth, +Y at ecliptic north, and +X
along Earth's orbital motion. The station's physics world uses these axes and
the station origin directly, so body positions, rope points, plume positions,
and array deflections are scene coordinates in meters; the renderer maps only
rotating-frame orbit vectors.

### Station attitude

The station holds a fixed attitude pitched 30° about its truss (x) axis, so
the Sun stands 30° above −Z. The radiators on ±x stay edge-on to the Sun, the
fixed arrays still collect cos 30° (87 %) of full sunlight, and module sides
and the truss catch light instead of lying exactly along the Sun line.
`verse_lagrange::station::attitude()` maps rotating-frame vectors into body
axes; the tidal and Coriolis field is evaluated in the rotating frame's axes
through it.

### The astronaut and the pack

| Property | Value |
| --- | --- |
| Suited astronaut plus pack, dry | 230 kg |
| Nitrogen propellant | 20 kg |
| Thrusters | 24 at 10 N: three at each corner of a 0.7 × 0.9 × 0.6 m box around the center of mass |
| Thrust along any commanded direction | 40 N (four thrusters) |
| Attitude hold | 1.5 rad/s natural frequency, critically damped, turns at most 0.6 rad/s |
| Specific impulse (cold N₂) | 70 s |
| Flight-control speed limit | 2 m/s relative to the station |
| Minimum-impulse deadband | 4 mm/s |
| Safety tether | 140 m from the airlock, holds 3 kN, unclips |

The pack is a hypothetical construction unit comparable to the Manned
Maneuvering Unit, not a model of a specific flight article. Its fly-by-wire
control law commands a velocity: joystick input asks for 2 m/s along the
commanded direction, and no input asks the pack to hold position. The pack
thrusts toward that command within the 40 N limit. The wanted force and the
attitude hold's torque go to a thruster allocator (`physics::ThrusterSet`),
which picks throttles for the 24 thrusters without firing opposed pairs, and
each thruster pushes at its own mounting point, so an unbalanced firing turns
the astronaut. The camera heading is a command: the body turns to it under
the attitude hold, and plumes come from the thrusters that fire. Propellant use follows
ṁ = F / (Isp g₀), and the HUD's Δv reserve follows the ideal rocket equation
over the current total mass. With no propellant, there is no thrust and the
astronaut coasts. The airlock ring refills the tank at 2 kg/s when you are
nearly stopped inside it.

Each burn is momentum-exact: the spent gas leaves at the exhaust velocity
(Isp g₀) relative to the pack, and the remaining mass takes the equal and
opposite momentum. Refill gas starts at rest in the station tank, so taking it
on slows the pack slightly.

### Plume impingement

Exhaust in vacuum still pushes what it hits. Each firing thruster is a
free-molecular point source (`physics::plume`): the momentum flux falls as
1/r² and with angle θ from the plume axis as cosⁿ θ, the angular form of
Simons' plume model (AIAA Journal, 1972), with n = 5 for nitrogen and nothing
beyond 90°. The flux is normalized so that the gas's momentum through any
sphere about the nozzle equals the thrust. Each box face that faces the nozzle
and lies within 30 m and inside the lobe (culled by bounding sphere) is
integrated with a midpoint rule of 4 to 64 points, more on faces that are large
compared with their distance; capsules and spheres use their projected area.
Of the gas that strikes a surface, 10% reflects specularly, and the rest is
absorbed and re-emitted diffusely with a quarter of its normal momentum.

The pack's plumes push drifting parts at the quadrature points, never the
astronaut. Four 10 N thrusters firing at the 90 kg avionics bay from about a
meter away push it to about 0.5 m/s in two seconds. Plumes on fixed structure (modules, the truss, the
arrays, the depot, and racked or latched parts) move nothing but load the
solar array wings and enter the ledger. By default the pack's plumes skip the
part it carries: with `Station::impinge_carried` on, the jets that push the
pair away from a load held at arm's length blow into it, so the pack can
neither back away from the load nor brake toward it. The station-keeping pods
fire for 1.2 s per burn at 220 N each, opposite the burn's Δv, with a
narrower cos⁸ θ lobe for hot monopropellant exhaust. Their plumes reach the
array tips and push anything free, the astronaut included.

`Station::plume_pulses` lists the thrusters that began firing within the last
second: thruster index (the pack's 0 to 23, then the −x and +x pods as 24 and
25), nozzle position, exhaust direction, onset tick, thrust, and a seed from
SplitMix64 of the index and tick, so replays produce the same pulses.

### Tethers and lines as ropes

The safety tether and each part's depot line are drawn as ropes
(`physics::Rope`): 96 particles for the tether and 48 for each line, 0.05 kg/m,
solved with extended position-based dynamics (XPBD) in six substeps of one
iteration each per physics step. The methods follow Jakobsen (2001), Müller
et al. (2007), and Macklin et al. (XPBD, 2016; "Small Steps", 2019). Both
ends are pinned to the tether joint's anchors. Long-range attachments (Kim et
al., 2012) keep every particle within its rest path length of each end, so at
full length in open space the rope lies exactly on the joint line, and shorter
than that it curves and carries transverse waves when an end moves. A reel at
each anchor pays out as fast as the end moves away and takes in slack at
0.25 m/s, so flying back toward the airlock faster than that leaves a slack,
whipping tether. Ropes are part of `StationState`, so save, restore, and
replay reproduce their shapes bit for bit.

**Contact and wrapping.** The ropes collide with the station as it is drawn
(`station::structure_solids`: the truss, radiators, pods, habitat, node and
hatch, arm, solar wings and masts, every member of the keel jig, and the
depot), with the parts, and, for a part's line, with the astronaut. Each rope
keeps its drawn radius from every solid: 12 mm for the tether and 8 mm for a
line. After the constraints in each substep, a segment that comes within that
radius of a solid is pushed back out along the solid's normal, shared between
its two particles by where it touched, twice per substep so a corner settles,
with Coulomb friction (0.3) against sliding (Müller et al., 2007; Macklin et
al., "Unified Particle Physics", 2014). A line therefore wraps around the node,
a jig member, or a part instead of passing through it. A line never meets its
own body, racked parts beside it in the depot, or a solid its station anchor
lies inside (the depot boom, for the part lines).

The rigid tether joint follows the wrap. When a line's resting path (from the
anchor through every particle resting on fixed structure, then to its end)
comes within 2 m of the line's full length, the line could be taut, so the
station finds where it would bend if pulled taut without changing sides of
anything: from the anchor, each chord runs to the farthest particle it can
see past the fixed solids, and the particle where the view is blocked is a
bend ("string pulling", `Rope::bends`). The joint then runs from the last bend
with the line left after the path to it (lengthened 2% for the rope rounding
each corner at its radius). A tether wrapped around the habitat therefore
arrests the astronaut closer to the airlock than a straight one would, and a
new bend never shortens the joint past where the astronaut already is, so it
holds without yanking. The reel takes in slack only down to the resting path,
so a wrapped line is never pulled through what it rests on.

**Unclipping.** **Unclip** lets the safety tether go: its end floats free with
the motion it had, collides like the rest of the rope, and the reel winds it
in at 1 m/s. Nothing then holds the astronaut to the station but the EVA
boundary 142 m from the airlock. **Clip** clips it back on when the tether's
clip is within 3 m of the astronaut, which is always true at the airlock once
the reel has wound it in. A part's depot line unclips when the part latches
into the jig, and its reel winds it back to the depot; a part reeled back to
the rack brings its line in with it.

`Station::ropes` returns one `RopeView` per line, the safety tether first and
then each part's line in `PartKind::ALL` order: the particle positions from the
station anchor to the free end's center of mass (or the loose clip) after the
last step, the positions before it for interpolation with `Station::alpha`,
whether the line is taut (the joint is pulling or the rope is at full length),
the tension in newtons (the joint's pull when taut, otherwise the rope's own
pull on its ends), the paid-out length, and whether it is clipped.

`Station::rope_coupling` turns on two-way coupling: each rope's free end joins
the solve with its body's effective inverse mass, and the rope's pull goes to
the body as equal and opposite impulses. It stays off. With it on, the ledger
still balances with the ropes' momentum included: linear momentum to 10⁻¹²,
and angular momentum to the rope solver's accuracy (a few parts in a million
in the busy test), since projecting a curved rope's particles is not exactly
central. What the solids push into the ropes enters as the `structure` term.
Energy never grows, but the rope is a second load path in parallel with the
joint, so the arrest force exceeds the joint's 3 kN cap by the rope's own pull
(about 3,004 N in the arrest test). Scripted setups that move a body without
calling `Station::settle_lines` also leave a stretched coupled rope that yanks
the body; half of the existing tests do that and fail with coupling on.

### Flexible solar arrays

Each solar array wing flexes in three assumed modes (`physics::Mode`, after
Likins, 1970, and Hughes, *Spacecraft Attitude Dynamics*, 1986): first
out-of-plane bending at 0.15 Hz, first torsion at 0.5 Hz, and second bending at
0.94 Hz, each with 0.5% of critical damping. The wing is a uniform 300 kg
cantilevered plate from its root at |x| = 13 m to its tip at |x| = 30 m. Bending
uses the cantilever beam's mode shapes, and torsion is linear across the chord
and a quarter sine along the span. Every step, the plume forces on the wing's
collider, weighted by each mode shape at their points, and the structure's
acceleration during a station-keeping burn (440 N on a 60 t station) drive the
modes. The update is the exact solution of each damped oscillator under a
force held over the step, so it is stable at any step length. A burn swings
the wing tips about 2 cm, and the first mode takes about 3.5 minutes to lose
two thirds of its amplitude. The rigid collider still handles contact.

`Station::array_flex(wing)` returns an `ArrayFlex` for wing 0 (−x) or 1 (+x),
interpolated with `Station::alpha`. `ArrayFlex::displacement(s, c)` gives the
out-of-plane displacement in meters along scene z at span s from 0 at the root
to 1 at the tip and chord c from −1 at y = −0.5 m to 1 at y = 12.5 m.
`ArrayFlex::twist(s)` gives the chord line's twist in radians, and
`ArrayFlex::deflect` moves an undeflected scene point on the wing.

### Momentum ledger

`Station::momentum` sums the linear momentum and the angular momentum about
the station origin of the free system: the astronaut with its propellant and
every carried or drifting part. `Station::ledger` (a `physics::Ledger`) records
every external impulse by name:

- `exhaust`: minus the momentum the escaping gas carries away. It is the
  thrust impulse and the spent gas's share of the pack's momentum, plus the
  momentum of any gas that struck something instead of escaping.
- `impingement`: plume momentum exchanged with fixed structure. Pack gas that
  strikes structure enters with the opposite sign, since that momentum left
  the gas but not for the free system. Station-keeping gas that strikes a free
  body enters as received.
- `structure`: contact with fixed station structure.
- `tether`: the safety tether and the parts' depot lines, and, with rope
  coupling on, the ropes' pull at the station anchors.
- `reel`: a stray part reeled in, and, with rope coupling on, line paid out
  from or taken in by a reel.
- `latch`: a part joining the station.

With the tidal field off (`Station::tide = false`), the momentum always equals
the ledger start plus those terms, through grabbing, carrying, releasing,
plume impingement, and rope coupling.

Attitude is held automatically, so the astronaut turns only in yaw. Collisions
treat the body as a 0.9 m sphere against the habitat, node, truss, solar arrays,
and depot backboard, removing closing velocity with a soft 0.2 restitution. The
keel jig is open lattice and can be flown through.

### Parts and rigid bodies

| Part | Mass | Envelope |
| --- | --- | --- |
| Main engine | 450 kg | 2.2 × 2.2 × 3.0 m |
| Propellant tank | 320 kg | 2.6 m diameter, 3.6 m |
| Aft and fore keel trusses | 180 kg each | 1.2 × 1.2 × 4.0 m |
| RCS pod | 140 kg | 2.4 × 1.0 × 1.2 m |
| Avionics bay | 90 kg | 1.4 m cube |

Parts are rigid bodies with principal moments of inertia (uniform boxes; the
tank is a thin-walled cylinder). Rotation holds world angular momentum exactly
and integrates the attitude quaternion with RK4. Tests check conservation of
angular momentum and rotational energy over two minutes and reproduce the
Dzhanibekov effect: a spin about the intermediate axis flips while spin about
the major axis stays stable.

### Contact

Collision uses `physics` colliders. Station modules, the truss, the solar
arrays, and the depot backboard are fixed boxes; the astronaut is a capsule
0.9 m across and 1.8 m tall; the tank is a capsule along its keel and the
other parts are boxes filling their envelopes. Box pairs touch on up to four
points clipped from the touching faces, so a panel stops a tumbling part with
the torque of the actual contact patch. The solver applies restitution 0.2,
Coulomb friction 0.5 on an elliptic cone, and a little torsional friction, and
it catches contacts up to one step before they touch, so parts at the pack's
top speed cannot pass through a 30 cm solar array.

Collision groups decide who touches whom: the astronaut hits structure and
free parts; free parts hit structure, the astronaut, each other, and parts in
the rack or on the jig. Contacts with fixed bodies enter the momentum ledger
as the `structure` term.

Grabbing closes the glove on the point of the part nearest the hands and holds
it with a soft weld (`physics::Joint`): a critically damped spring at 6 rad/s
on position and orientation, solved implicitly. Capture is therefore an
internal impulse that conserves momentum, a heavy part drags on the astronaut,
and a part caught while moving or spinning settles into the grip without
ringing. The grip holds at most 400 N and 300 N m; pulled past that, it slips
and the part floats free. While carrying, the pack treats the astronaut and
the part as one body: it aims its thrust through their shared center of mass
and holds attitude with their combined inertia, and it spends at most 15 N m
of torque on that aim, so a heavy part far from the hands accelerates gently.
Releasing a part away from its slot leaves it drifting with its own velocity
and spin.
Each part has a 120 m line from the depot, and the astronaut a 140 m safety
tether from the airlock. Both are tether joints: slack inside their length,
they pull only, arrest at most 3 kN, and never add energy, so a fast arrival
stretches the line a few centimeters rather than stopping in one step. Past
the length plus 2 m, an emergency boundary still returns the astronaut or
reels the part back to the rack.

Releasing a part latches it when it is within 1.6 m of its slot, within 15
degrees of the slot's orientation (or of that orientation turned half a turn
about the keel), moving below 0.35 m/s, and turning below 0.05 rad/s. The
latch is a hard weld from the jig to the part, so the part settles onto its
seat and stays there when something strikes it. Otherwise the release message
names the first condition that failed.

### Sleep

Settled groups sleep: a group of bodies joined by contact, welds, or taut
tethers falls asleep after half a second below 1 cm/s and 0.02 rad/s, but only
while it rests on something fixed or already asleep. Latched parts, welded to
the jig, sleep and cost nothing until something strikes them. A free part
never sleeps, however slowly it drifts, because nothing in microgravity lets
it settle; a slack tether does not count as resting. A sleeping body wakes when
a moving body touches it, a force acts on it, a joint on it changes, or its
owner sets it moving. The snapshot reports the awake body count and the last
step's wall-clock time.

### Sensors and the forces overlay

The suit carries an inertial measurement unit (`physics::Imu`): the HUD shows
the proper acceleration in standard gravities (a full 40 N burn empty is about
0.016 g) and the spin rate. A ray along the facing reports the nearest
structure or part ahead, and the contact force on the astronaut and anything
it holds shows as an impact in newtons. Grabbing casts a ray from the hands to
the part's center and closes the glove where it meets the surface.

The **Forces** control draws the physics overlay: contact points with their
normals (cyan) and impulses (red), joints such as the grip, tethers, and
latches (yellow, magenta at their limit), and each firing thruster's force
(violet).

## Approximations

- The Sun and the EMB move on circular orbits (no eccentricity), the Moon is a
  point on a circular inclined orbit, and planetary perturbations and solar
  radiation pressure are omitted.
- Station-keeping cancels only the linear unstable mode, so it spends more Δv
  than flight halo control and uses a smaller orbit.
- Local physics linearizes about L1 rather than about the moving station.
- The attitude hold keeps the astronaut level; only heading is commanded.
- The astronaut's inertia is a fixed 40 kg m² about every axis.
- A carried part does not collide with its carrier or with parts in the rack
  or on the jig, and the astronaut passes through parts in the rack or on the
  jig.
- The grip is a single soft weld, not a model of fingers and glove friction.
- Plume impingement ignores shadowing: a surface behind another still takes
  the flux that reaches its position. Curved surfaces use their projected
  area, and the gas-surface model is a fixed mix of specular reflection and
  diffuse re-emission.
- The pack's plumes skip the part it carries unless `impinge_carried` is on.
- A station-keeping burn fires locally for a fixed 1.2 s whatever its Δv, and
  only the solar array wings feel the structure's acceleration; free bodies do
  not.
- Ropes follow their anchors and never pull on bodies unless `rope_coupling`
  is on, the reel is idealized, and paid-out line has the rope's full particle
  count at any length. A rope pushes out of solids but never pushes them: a
  drifting part or the astronaut moves a line aside without feeling it. The
  lines wrap the drawn shapes, while the astronaut and parts collide with the
  coarser station boxes. Only the joint's bends count toward arrest; rope
  friction holds a wrap in place but does not add to the joint's limit, as a
  capstan would. A rope laid straight through structure by a scripted setup
  (`Station::settle_lines`) is pushed out toward the nearer side.
- Array flex is drawn and stored but does not move the rigid array colliders.
- The station attitude is fixed; real arrays would track the Sun with
  gimbals.
- The Earth's rotation and axial tilt and the star field use low-precision
  ephemerides from the mission epoch; the rotating frame turns at the mean
  motion, so the Earth's longitude drifts slowly from the true date. The
  Moon's phase comes from the simulated synodic month, not an ephemeris.
- Bounce light is one diffuse bounce baked into a 3 m probe grid from the
  structure and resting parts; moving parts and the astronaut receive it but
  do not cast it. Earthshine is included as a dim directional disc.
- The Earth's atmosphere is single-scattered Rayleigh haze in a flat-layer
  approximation; the limb is sub-pixel at L1.
- Construction state is local and resets on each visit; there is no shared
  editing authority. `StationState` can be saved and restored, but the zone
  does not persist it.

## Tests

`cargo test -p verse-lagrange` covers L1's location and linear constants, the
exact unstable eigenvector, Jacobi conservation and uncontrolled divergence,
two years of controlled flight, microgravity magnitude, the rocket equation
and position hold, inelastic capture, latch conditions (position, speed,
spin, and alignment), a latched part under impact, the safety tether's arrest, latched parts
sleeping while free parts never do, the HUD's sensors, the grab point,
randomized releases against the ledger and the structure,
collisions, frame-rate
independence, the frame step cap, save and restore, journal replay, and the
momentum ledger through coasting, burns, structure contact, the tether,
capture, and carrying, the grip slipping past its limit, attitude hold through a translation and a commanded
turn, plumes at the firing thrusters, a spinning tank glancing off a solar
array edge, and free parts colliding with each other. They also cover the
safety tether rope lying straight when taut and curving when slack, a plume
moving a free part with the ledger balanced, plumes on structure as the
`impingement` term, the carried-part plume setting, a station-keeping burn
ringing the arrays down, real station-keeping burns firing the pods, plume
pulse onsets and seeds, the array mode shapes and masses, and the coupled
ropes' ledger through an arrest. The line tests check the tether wrapping the
node instead of passing through it, a wrapped tether arresting along its path
without adding energy, unclipping and clipping back on at the clip (and its
replay), a latched part's line reeling back to the depot, and every line
keeping clear of the structure through the busy scene with the coupled
ledger balanced.
`cargo run --release -p verse-lagrange --example step_budget` reports the
physics step time for a busy scene. `cargo test -p physics` covers the shared mechanisms, including box manifolds
through a scripted tilt, yaw, penetration, and slide sweep, friction
breakaway, torsional friction, tunneling, momentum through collisions, and a
small stack. It also covers the rope (straight when taut, curving and carrying
a whip when slack, a hanging catenary without energy gain, bit-for-bit
restore, coupled momentum, wrapping a post it is swung around, a loose end
reeled in past a block, a wrapped rope's restore, and the taut path's bends),
the solids' distances, normals, bounds, and segment queries, the plume (the lobe carrying exactly the
thrust, dynamic pressure on a plate, an enclosure catching the whole thrust,
and culling), and the modes (the exact ring-down envelope, a step load, and
stability at any step).
`cargo test -p verse --lib zones` covers portal entry and return, flight, the
grab-carry-latch flow, the **Unclip** and **Clip** control, that the solids
the lines wrap follow every drawn vertex of the structure to within a
centimeter, and that only Lagrange frames carry a physical sky
with the Sun, Earth, and illuminance at their true values.
`cargo test -p verse --lib pbr` covers the ephemeris (epoch, the Sun on the
scene axis, the ecliptic pole), star colors and magnitudes, catalogue and
texture decoding, the ray-cast hierarchy, baked occlusion, a probe's bounce
from a sunlit floor, Earthshine as a few millionths of sunlight, and the
half-float conversion. Render the scene offline with:

```sh
cargo run --release -p verse --example lagrange_capture -- out.png [VIEW]
```

Views are the player's `spawn`, `jig`, `carry`, `sun`, and `earth`, fixed
cameras `sunside` and `wide`, and telephoto `earthzoom`, `moonzoom`, and
`sunzoom`. `look EX,EY,EZ TX,TY,TZ [FOV]` aims a camera from any eye at any
target, with a vertical field of view in radians; for example,
`look 21,6,-12 21,6,-1.1` faces the +x solar wing from the sunward side. The capture waits for the light bake and runs a few frames so the
exposure settles. Set `VERSE_PHOTO_DEBUG` to 1 (direct light), 2 (probe
diffuse), 3 (probe specular), 4 (ambient occlusion), 5 (sun shadow), or 6
(N·V, N·L, N·H) to inspect one term, and `VERSE_PHOTO_RGBA16` to force a
16-bit float scene target.
