# Reading list for Lagrange 2

This list started from a set of textbook recommendations the owner received
from ChatGPT. We kept all of them, added the books and papers that list misses
for our work, checked the bibliographic details, and ranked everything by how
much it helps Lagrange 2
([issue #11625](https://github.com/OpenAgentsInc/openagents/issues/11625)).

## Rules for using these books

- **Do not copy text, figures, or tables into the repository.** Read, then
  write our own notes and code in our own words. Cite the book and section.
- **Exact coefficients come from the book or a verified primary source.** Our
  notes in this folder give typical values and ranges, marked [M] when they
  are from memory. Before a value becomes a code constant or a test
  tolerance, check it against the book, a NASA, JPL, or ESA document, or a
  peer-reviewed paper, and cite that source next to the constant, as
  `orbit.rs` already does for Lagrange 1.
- Prefer primary sources (ephemeris files, flight-dynamics papers) over a
  textbook's rounded table when a test depends on the number.

## Our needs, in priority order

1. The Earth–Moon L2 halo orbit and its station-keeping.
2. Robotic assembly in microgravity: arms on a free-floating base.
3. Mass properties of a large ship assembled from parts.
4. Propulsion and the departure test burn.
5. Attitude control while the inertia changes during assembly.
6. Structures and loads: latches, trusses, factors of safety.
7. Thermal and power at L2.

## Ranked list

Source column: **GPT** means the ChatGPT list recommended it; **Added** means
we added it. Details column: **[V]** means we verified the bibliographic
details (authors, edition, year, publisher) against a catalog or publisher
listing; **[M]** means they are from memory.

| Rank | Book | Source | Details | Serves needs |
| --- | --- | --- | --- | --- |
| 1 | Koon, Lo, Marsden, Ross, *Dynamical Systems, the Three-Body Problem and Space Mission Design* | Added | [V] | 1 |
| 2 | Yoshida, Wilcox, Hirzinger, Lampariello, "Space Robotics", chapter 55 of Siciliano and Khatib (eds.), *Springer Handbook of Robotics* | Added | [V] | 2, 5 |
| 3 | Wertz, Everett, Puschell (eds.), *Space Mission Engineering: The New SMAD* | GPT | [M] | 3, 4, 6, 7 |
| 4 | Markley, Crassidis, *Fundamentals of Spacecraft Attitude Determination and Control* | GPT | [V] | 5 |
| 5 | Fortescue, Swinerd, Stark (eds.), *Spacecraft Systems Engineering*, 4th ed. | GPT | [V] | 3, 4, 6, 7 |
| 6 | Sutton, Biblarz, *Rocket Propulsion Elements*, 9th ed. | GPT | [M] | 4 |
| 7 | Sarafin (ed.), *Spacecraft Structures and Mechanisms: From Concept to Launch* | GPT | [V] | 6, 2 |
| 8 | Szebehely, *Theory of Orbits: The Restricted Problem of Three Bodies* | Added | [V] | 1 |
| 9 | Curtis, *Orbital Mechanics for Engineering Students*, 4th ed. | GPT | [V] | 1, 4 |
| 10 | Xu, Kanade (eds.), *Space Robotics: Dynamics and Control* | Added | [V] | 2 |
| 11 | Gilmore (ed.), *Spacecraft Thermal Control Handbook*, Vol. I, 2nd ed. | GPT | [V] | 7 |
| 12 | Vallado, *Fundamentals of Astrodynamics and Applications*, 5th ed. | Added | [V] | 1, 4 |
| 13 | Hughes, *Spacecraft Attitude Dynamics* | Added | [M] | 5, 6 |
| 14 | Wertz (ed.), *Spacecraft Attitude Determination and Control* | GPT | [M] | 5 |
| 15 | Griffin, French, *Space Vehicle Design*, 2nd ed. | GPT | [V] | 3, 6, 7 |
| 16 | Brown, *Elements of Spacecraft Design* | GPT | [M] | 3, 7 |
| 17 | Humble, Henry, Larson (eds.), *Space Propulsion Analysis and Design* | GPT | [M] | 4 |
| 18 | Pisacane (ed.), *Fundamentals of Space Systems* (1st ed. with Moore) | GPT | [M] | 3, 7 |
| 19 | Bate, Mueller, White, Saylor, *Fundamentals of Astrodynamics*, 2nd ed. | GPT | [V] | 4 |
| 20 | Wertz, Larson (eds.), *Space Mission Analysis and Design* (SMAD), 3rd ed. | GPT | [M] | 3 |
| 21 | Cappelletti, Battistini, Malphrus (eds.), *CubeSat Handbook: From Mission Design to Operations* | GPT | [V] | (little) |

### Primary papers and documents

These are not textbooks but carry the exact numbers our tests should cite.

| Paper | Details | Use |
| --- | --- | --- |
| Folta, Pavlak, Haapala, Howell, Woodard, "Earth–Moon libration point orbit stationkeeping: theory, modeling, and operations", *Acta Astronautica* (2013), doi:10.1016/j.actaastro.2013.01.022 ([NTRS PDF](https://ntrs.nasa.gov/api/citations/20160010602/downloads/20160010602.pdf)) | [V] | ARTEMIS flight station-keeping costs and orbit sizes at Earth–Moon L1 and L2 |
| Umetani, Yoshida, "Resolved Motion Rate Control of Space Manipulators with Generalized Jacobian Matrix", *IEEE Transactions on Robotics and Automation* 5(3), 1989 ([PDF](https://tohoku.repo.nii.ac.jp/record/26212/files/10.1109-70.34766.pdf)) | [V] | The generalized Jacobian for an arm on a free-floating base |
| Parrish, Kayser, Bolliger, Thompson, Parker, Cheetham, Davis, Sweeney, "Near Rectilinear Halo Orbit Determination with Simulated DSN Observations" ([NTRS PDF](https://ntrs.nasa.gov/api/citations/20200011550/downloads/20200011550.pdf)) | [V] | Gateway 9:2 NRHO size and maintenance cost |
| JPL Solar System Dynamics, [Astrodynamic Parameters](https://ssd.jpl.nasa.gov/astro_par.html) (DE440) | [V] | GM of the Earth and the Moon |
| Richardson, "Analytic construction of periodic orbits about the collinear points", *Celestial Mechanics* 22 (1980) | [M] | Third-order halo approximation, the usual initial guess for differential correction |

## What to look up in each

### 1. Koon, Lo, Marsden, Ross (Added)

Marsden Books, 2011, ISBN 978-0-615-24095-4; a revised PDF (v1.3, 2022) is
free from the authors at [ross.aoe.vt.edu/books](https://ross.aoe.vt.edu/books/space_book.html).
The best single source for our orbit work, and free.

- The CR3BP in the rotating frame, the effective potential, and the Jacobi
  integral (check our `orbit::acceleration` and `orbit::jacobi` signs).
- Locating the collinear points; the linearization and its eigenvalues, the
  saddle × center × center structure.
- Lyapunov, halo, and Lissajous orbits; computing them by differential
  correction and continuation.
- Stable and unstable manifolds: low-energy transfers to and from L2 and the
  departure path for the test burn.

### 2. Space Robotics chapter, Springer Handbook of Robotics (Added)

Siciliano and Khatib (eds.), 2nd ed., Springer 2016, ISBN 978-3-319-32550-7;
chapter 55, pp. 1423–1462, doi:10.1007/978-3-319-32552-1_55.

- The equations of motion of a free-floating manipulator, momentum
  conservation, and the generalized Jacobian.
- Reaction null space: arm motions that leave the base attitude unchanged.
- Capture and contact dynamics, impedance control for berthing.
- Flight examples (ETS-VII, ISS arms) for plausible speeds and loads.
- Other chapters of the same handbook cover manipulator kinematics,
  dynamics, motion planning, and assembly; use them for the arm side.

### 3. The New SMAD (GPT)

Microcosm Press, 2011 [M]. The general reference for a whole spacecraft.

- Mass budget fractions by subsystem and the margin policy by design phase.
- Power sizing: array area from end-of-life power, battery depth of discharge.
- Thermal first-cut sizing: radiator area from dissipated power.
- Propulsion system mass: tankage and pressurant fractions.
- Cost and schedule are not needed.

### 4. Markley and Crassidis (GPT)

Springer, 2014, Space Technology Library vol. 33, doi:10.1007/978-1-4939-0802-8.

- Attitude kinematics (quaternions) and Euler's equations; compare with
  `physics::body`.
- Reaction wheel and control-moment gyroscope models, momentum management
  and unloading.
- Attitude control laws (quaternion feedback) and their gain choice from
  the inertia, needed because the factory's inertia grows as parts latch.

### 5. Fortescue, Swinerd, Stark (GPT)

Wiley, 4th ed., 2011. A readable systems text, one chapter per subsystem.

- Structures chapter: load factors, stiffness requirements, factors of
  safety, and how launch loads drive sizing (relevant to a ship built in
  orbit, which avoids launch loads for the assembled structure).
- Propulsion chapter: system layouts, the rocket equation, Isp ranges.
- The 4th edition's assembly, integration, and verification chapter.

### 6. Sutton and Biblarz (GPT)

Wiley, 9th ed., 2017 [M]. Propulsion.

- Thrust, Isp, and chamber pressure relations; nozzle expansion.
- Typical Isp and thrust for bipropellant engines, for the sample ship's
  main engine.
- Propellant densities, for tank volumes from tank masses.

### 7. Sarafin (GPT)

Kluwer and Microcosm, 1995, Space Technology Library vol. 4.

- Factors of safety and margin of safety definitions.
- Stiffness and natural frequency requirements; joint and latch design;
  preload; mechanism torque margins.

### 8. Szebehely (Added)

Academic Press, 1967; softcover reissue 2013, ISBN 978-0-12-412431-8.

- The canonical treatment of the restricted problem: the collinear points,
  zero-velocity curves, and periodic orbit families. Use it to cross-check
  our derivations, not as a first read.

### 9. Curtis (GPT)

Butterworth-Heinemann, 4th ed., 2020. The 4th edition added a chapter on the
CR3BP.

- Worked CR3BP examples with MATLAB scripts we can reproduce in Rust tests.
- Impulsive maneuvers, the rocket equation, and lunar trajectories.

### 10. Xu and Kanade (Added)

Kluwer, 1993 (edited collection), doi:10.1007/978-1-4615-3588-1.

- Papers on base–arm dynamic coupling, the nonholonomic behavior of a free
  floating arm (a closed joint path can leave the base rotated), and control.

### 11. Gilmore (GPT)

Aerospace Press and AIAA, 2nd ed., 2002.

- Surface optical properties (solar absorptance, infrared emittance) for
  paints and blankets; L1's renderer already uses measured absorptance.
- Radiator sizing, heaters, multilayer insulation performance.

### 12. Vallado (Added)

Microcosm Press, 5th ed., 2022, Space Technology Library vol. 21.

- Coordinate frames and time systems, if we move from the CR3BP to an
  ephemeris model.
- Numerical integration and perturbation methods.
- Less useful for the three-body problem itself.

### 13. Hughes (Added)

*Spacecraft Attitude Dynamics*, Wiley 1986, Dover reprint [M]. Already cited
in the Lagrange 1 document for flexible solar array modes.

- Flexible appendages and their modal coupling with attitude control; the
  long trusses of a large ship will have low natural frequencies.

### 14 to 21 (GPT)

- **Wertz, *Spacecraft Attitude Determination and Control*** (Reidel, 1978)
  [M]: sensor models and environmental torques. Superseded for control by
  Markley and Crassidis.
- **Griffin and French** (AIAA, 2nd ed., 2004, ISBN 1-56347-539-1): design
  rationale and configuration; good on structures and thermal at
  the concept level.
- **Brown** (AIAA, 2002) [M]: compact subsystem sizing equations with worked
  examples.
- **Humble, Henry, Larson** (McGraw-Hill, 1995) [M]: propulsion system
  design and mass estimation.
- **Pisacane** (Oxford; 1st ed. 1994 with Moore, 2nd ed. 2005) [M]: physics
  of the space environment and subsystems.
- **Bate, Mueller, White, Saylor** (Dover, 2nd ed., 2020, ISBN
  978-0-486-49704-4): two-body orbits and maneuvers; little three-body
  content.
- **SMAD, 3rd ed.** (Microcosm and Kluwer, 1999) [M]: superseded by the New
  SMAD; use only if that is the copy at hand.
- **CubeSat Handbook** (Academic Press, 2021, ISBN 978-0-12-817884-3):
  aimed at small satellites; little bearing on a large assembled ship.

## Gaps no book here fills

- **Large-structure assembly by many arms at once.** No textbook covers it;
  use the robotics handbook for single-arm dynamics, then our own planner.
  Flight precedents are in [in-space-assembly.md](in-space-assembly.md).
- **The robot side of Robot Lab** (URDF, MuJoCo, joint servos) is covered by
  the Robot Lab documents in [`docs/lab/`](../lab/README.md), not by these
  books.
