# Astrodynamics for Earth–Moon L2

The orbit side of Lagrange 2: the circular restricted three-body problem
(CR3BP), the Earth–Moon collinear points, halo and Lissajous orbits,
station-keeping, transfers, and the rocket equation. See
[the README](README.md) for the confidence marks. Every [C] value here was
computed by us with the same collinear-quintic Newton iteration that
`verse_lagrange::orbit::L1::new` uses, from the [V] constants below.

## Constants

| Constant | Value | Mark and source |
| --- | --- | --- |
| GM of the Earth | 398,600.435507 km³/s² | [V] JPL [Astrodynamic Parameters](https://ssd.jpl.nasa.gov/astro_par.html), DE440 |
| GM of the Moon | 4,902.800118 km³/s² | [V] same |
| Earth/Moon mass ratio | 81.30057 | [C] ratio of the two GM values |
| μ = M_Moon / (M_Earth + M_Moon) | 0.012150584 | [C] |
| Earth–Moon distance (CR3BP length unit) | 384,400 km | [M] conventional; the Moon's orbital semimajor axis is about 384,400 km and its mean distance about 384,750 km |
| Time unit 1/n | 4.3425 days | [C] from √(D³ / (GM_E + GM_M)) |
| Velocity unit D·n | 1,024.5 m/s | [C] |
| Sidereal month in this model | 27.28 days | [C]; the true sidereal month is 27.32 days [M] because the real orbit is eccentric and perturbed |
| Standard gravity g₀ | 9.80665 m/s² | [M] exact by definition |

The often-quoted μ ≈ 0.01215 is the value above, rounded.

**Note for implementers:** the Lagrange 1 crate defines `GM_MOON =
4.9048695e12` and labels it DE440, but DE440 gives 4.902800118e12 (0.04%
lower). That barely matters at Sun–Earth L1, but used in the Earth–Moon
system it shifts μ to 0.0121557 and L2 by about 9 km. Lagrange 2 should use
the DE440 value; see [sim-mapping.md](sim-mapping.md).

## The CR3BP in brief

Two primaries (Earth, mass 1 − μ, at x = −μ; Moon, mass μ, at x = 1 − μ)
move on circles about their barycenter. In the frame rotating with them,
normalized so the distance, the total mass, and the angular rate are all 1, a
massless body obeys:

- ẍ − 2ẏ = ∂Ω/∂x, ÿ + 2ẋ = ∂Ω/∂y, z̈ = ∂Ω/∂z,
- with Ω = (x² + y²)/2 + (1 − μ)/r₁ + μ/r₂, where r₁ and r₂ are the
  distances to the Earth and the Moon.

The 2ẏ and 2ẋ terms are the Coriolis acceleration. The Jacobi integral
C = 2Ω − v² is conserved, which is the standard integrator check; Lagrange 1
already tests it to one part in 10⁹ over 60 days.

The five Lagrange points are the equilibria. The three collinear points sit
on the x axis. With γ the distance from the Moon:

- L1 (between): γ⁵ − (3 − μ)γ⁴ + (3 − 2μ)γ³ − μγ² + 2μγ − μ = 0.
- L2 (beyond the Moon): γ⁵ + (3 − μ)γ⁴ + (3 − 2μ)γ³ − μγ² − 2μγ − μ = 0.

Both have a single positive root near (μ/3)^(1/3), which Newton iteration
finds in a few steps.

## Earth–Moon L1 and L2

| Quantity | L1 | L2 | Mark |
| --- | --- | --- | --- |
| γ (distance from the Moon, normalized) | 0.150934 | 0.167833 | [C] |
| Distance from the Moon's center | 58,020 km | 64,515 km | [C] |
| Height above the lunar surface | 56,280 km | 62,780 km | [C], lunar radius 1,737.4 km |
| Distance from the Earth's center | 326,400 km | 448,900 km | [C], matches [V] [Wikipedia, Lagrange point](https://en.wikipedia.org/wiki/Lagrange_point) table (15.09% and 16.78% offsets) |
| Richardson's c₂ | 5.1478 | 3.1904 | [C] |
| In-plane unstable rate λ | 2.9321 | 2.1587 | [C] |
| e-folding time 1/λ | 1.48 days | 2.01 days | [C] |
| In-plane frequency ω_p (period) | 2.3344 (11.7 days) | 1.8626 (14.6 days) | [C] |
| Out-of-plane frequency ω_v (period) | 2.2689 (12.0 days) | 1.7862 (15.3 days) | [C] |

Published figures for "L2 beyond the Moon" range from about 61,000 to
65,000 km because some give height above the surface, some the distance
from the center, and some use the instantaneous Earth–Moon distance, which
varies by about ±21,000 km over a month [M]. In the CR3BP with D =
384,400 km the answer is 64,515 km from the center, about 62,800 km above
the surface. Issue #11625's "about 61,000 km" is low by a few thousand km.

Compare Sun–Earth L1, where Lagrange 1 flies: e-folding about 23 days.
Earth–Moon L2 is **more than ten times as unstable in time**, so its
station-keeping must act more often.

### Flight cross-check

ARTEMIS (two THEMIS probes moved to the Moon, 2010–2011) flew the first
Earth–Moon L1 and L2 libration orbits. Folta et al. (2013, Table 1) [V] list
P1's L2 orbit at a maximum y-amplitude of about 63,500 km, z-amplitude about
35,200 km, and an orbital period of 15.47 days, and its L1 orbit at 13.51
days. Our linear L2 periods of 14.6 and 15.3 days bracket the flight value,
just as L1's linear periods bracket the 178-day Sun–Earth orbits.

## Halo and Lissajous orbits

- **Lissajous orbits** are quasi-periodic: the in-plane and out-of-plane
  oscillations have different frequencies (ω_p ≠ ω_v), so the path never
  closes and slowly changes shape. Small amplitudes stay close to the linear
  solution. Lagrange 1 flies one.
- **Halo orbits** are periodic. At large enough amplitude the nonlinear
  terms shift the frequencies until they match, and the orbit closes into a
  three-dimensional loop. Halo orbits bifurcate from the planar Lyapunov
  family at an in-plane amplitude of several tens of thousands of kilometers
  at Earth–Moon L2 [M, compute it]. Each amplitude has a northern and a
  southern member, mirror images in z.
- **Computing a halo:** start from Richardson's third-order approximation,
  then correct with a differential corrector that varies the initial z and
  ẏ (or x and ẏ) until the next x–z plane crossing has ẋ = ż = 0. Continue
  in amplitude to trace the family. Koon et al. and Curtis describe this.
- **The L2 halo family** runs from large halos with periods near 15 days
  down to **near-rectilinear halo orbits (NRHOs)** that pass close over a
  lunar pole. NASA's Gateway is planned for a 9:2 synodic-resonant southern
  L2 NRHO: perilune about 3,500 km and apolune about 71,000 km from the
  Moon's center [V] (Parrish et al.), period 59.06/9 ≈ 6.56 days [C, from a
  29.53-day synodic month].

### Earth visibility from L2

From L2 the Moon's disc is 3.1° across and the Earth's 1.6° [C], so the Moon
can hide the Earth completely. A body exactly at L2 never sees the Earth.
But the region the Moon shadows, as seen from the Earth, is only about
2,030 km in radius at L2's distance [C, 1,737 km × 448,900 / 384,400].
**Any halo orbit stays far outside it**, so a halo has continuous line of
sight to the Earth. This is why Farquhar proposed a halo-orbit relay for the
lunar far side in the 1960s, and why China's Queqiao relay (launched 2018 for
Chang'e 4) flies an Earth–Moon L2 halo [V, press and reference sources found
while writing these notes]. A Lissajous orbit, whose shape drifts, needs
active phase control to avoid passing behind the Moon.

**For us:** issue #11625 imagines the Moon "permanently between the factory
and Earth". That holds only for a factory at the point itself or on a very
small orbit. On a halo, Earth swings around the Moon's limb. Either choice
is defensible; the sky and the comms model must match the one we pick.

## Station-keeping

Collinear orbits are unstable, so small errors grow by a factor e every
e-folding time unless corrected.

| Mission or study | Cost | Mark |
| --- | --- | --- |
| ARTEMIS P1, Earth–Moon L2 (flight, extrapolated) | 7.39 m/s per year | [V] Folta et al. 2013, Table 3 |
| ARTEMIS P1, Earth–Moon L1 | 5.28 m/s per year | [V] same |
| ARTEMIS P2, Earth–Moon L1 | 5.09 m/s per year | [V] same |
| ARTEMIS, summary | "~5–7 m/s per year", maneuvers about weekly | [V] same |
| ARTEMIS pre-flight budgets, including transfers | 15 m/s (P1), 12 m/s (P2) | [V] same |
| Gateway 9:2 NRHO | "on the order of a few meters per second per year" | [V] Parrish et al. |
| Gateway NRHO, simulation with optical navigation | about 1.1 m/s per year | [M] MERL report TR2022-114, found while writing these notes, not read in full |
| Lagrange 1's linear controller at Sun–Earth L1 | 8.6 m/s per year | [V] our own test |

Practical points:

- Flight controllers target conditions one or more revolutions downstream
  (for example ẋ at the next x–z crossings) rather than holding a fixed
  reference, which lets the orbit drift along the family and saves Δv.
- Burns are placed where the unstable direction is most controllable, and
  tiny burns are skipped (Gateway studies skip orbit-maintenance burns under
  3 cm/s [V] Parrish et al.).
- Navigation error drives cost: the better the orbit is known, the smaller
  the burns. Solar radiation pressure modeling mattered for ARTEMIS [V].
- **Cadence for our controller:** Lagrange 1 cancels the linear unstable
  mode every 7 days, which is a third of an e-folding time there. At
  Earth–Moon L2 the e-folding time is about 2 days, so 7 days lets an error
  grow about 33 times [C]. Our linear-mode controller needs a cadence of
  about a day or less, or a downstream targeting scheme like ARTEMIS's.

## Transfers to and from L2

Values here are from memory and vary with the trajectory type and date.

- **Direct transfer from low Earth orbit:** a trans-lunar injection of about
  3.1 km/s [M], a powered or unpowered lunar flyby, and an insertion of tens
  to a few hundred m/s [M]. Flight time about one to three weeks; Queqiao
  took about 24 days with a lunar flyby [V, reference sources].
- **Low-energy transfer** through the Sun–Earth system's manifolds: smaller
  insertion Δv, flight time three to four months [M]. ARTEMIS reached the
  Moon this way [V, Folta et al. describe its ballistic Sun–Earth to
  Earth–Moon transfers].
- **Departure:** the unstable manifold of a halo is a free ride away from
  it. A burn of meters to tens of m/s along the unstable direction leaves the
  orbit within a few e-folding times [M]. Reaching low lunar orbit from an
  NRHO costs several hundred m/s one way [M]. Escaping the Earth–Moon system
  from L2 with a lunar flyby costs little more than that [M].

**For the test burn:** the issue asks for a "test burn departing L2". The
honest framing is a burn whose Δv we choose (say 50–500 m/s) and check
against the rocket equation, then propagate in the CR3BP to show the ship
leaving along or across the manifold. A full mission transfer is a separate
design.

## The rocket equation

Δv = I_sp g₀ ln(m₀ / m_f), so m₀ / m_f = exp(Δv / (I_sp g₀)).

| Δv | I_sp 320 s (storable biprop) | I_sp 450 s (LOX/LH₂) | I_sp 3,000 s (ion) |
| --- | --- | --- | --- |
| 100 m/s | 1.032 | 1.023 | 1.003 |
| 500 m/s | 1.173 | 1.120 | 1.017 |
| 1,000 m/s | 1.375 | 1.254 | 1.035 |
| 3,000 m/s | 2.601 | 1.974 | 1.107 |

Mass ratios [C]; the I_sp values are typical class values [M].

Burn time for a constant-thrust engine: the mass flow is ṁ = F / (I_sp g₀),
and the propellant used is m₀ (1 − exp(−Δv / (I_sp g₀))), so
t = propellant / ṁ. A 500 t ship with a 100 kN engine at I_sp 320 s needs
about 74 t of propellant and about 39 minutes for 500 m/s [C].

Points that matter for a simulated burn:

- Thrust misalignment from the center of mass creates a torque F × d; the
  attitude system must cancel it, usually by gimballing the engine or with
  thrusters.
- The center of mass moves as propellant drains; the thrust vector must
  follow it.
- In the weak field near L2, gravity losses during a burn of under an hour
  are small; propagate the burn in the CR3BP rather than assuming them away.

## What to read next

Koon et al. for halo orbits and manifolds, Szebehely for the CR3BP theory,
Curtis for worked examples, and Folta et al. for flight numbers. See
[reading-list.md](reading-list.md).
