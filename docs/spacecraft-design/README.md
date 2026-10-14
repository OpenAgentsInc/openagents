# Spacecraft design knowledge

This folder collects what we know about spacecraft design, and where to look
for more, in support of two Verse zones:

- [Lagrange 1](../verse/lagrange-1.md) (`crates/verse-lagrange`): a construction
  station at Sun–Earth L1, simulated in the circular restricted three-body
  problem.
- **Lagrange 2** ([issue #11625](https://github.com/OpenAgentsInc/openagents/issues/11625)):
  a spaceship factory at Earth–Moon L2, where robot arms assemble a large,
  fully simulated spacecraft that then makes a test burn.

The [phased cislunar strategy](cislunar-strategy.md) extends this work into a
startup plan: a first robotic station in low Earth orbit, lunar logistics,
and a later L2 heavy shipyard, with an aggressive schedule and engineering
gates for each expansion.

The notes are written in our own words from general engineering knowledge.
They are a starting point for design and test choices, not a substitute for
the books. No text, figures, or tables are copied from any book.

## Files

| File | What it holds |
| --- | --- |
| [reading-list.md](reading-list.md) | The textbooks, ranked for Lagrange 2, with what to look up in each |
| [design-method.md](design-method.md) | Mission phases, requirements, budgets and margins, trade studies |
| [astrodynamics.md](astrodynamics.md) | The CR3BP, Earth–Moon L1 and L2, halo and Lissajous orbits, station-keeping, transfers, the rocket equation |
| [subsystems.md](subsystems.md) | Mass fractions, power, thermal, attitude control, communications, propulsion, structures, mechanisms |
| [in-space-assembly.md](in-space-assembly.md) | Robotic assembly in microgravity, free-floating arm dynamics, flight precedents, assembly planning |
| [sim-mapping.md](sim-mapping.md) | How each topic maps to our crates, and the tests Lagrange 2 needs |
| [cislunar-strategy.md](cislunar-strategy.md) | A phased station network, construction and transport targets, schedule, and alternative execution plans |

## Confidence marks

Every number in these notes carries one of these marks:

| Mark | Meaning |
| --- | --- |
| **[V]** | Verified against the cited source while writing these notes |
| **[C]** | Computed by us from [V] constants; the method is stated |
| **[M]** | From memory, verify before relying on it in code or a test |
| **[P]** | A proposed planning target or assumption, rather than a verified capability |

A number marked [M] must not become a test tolerance or a code constant until
someone checks it against the book or a primary source and changes its mark.

## Key numbers at a glance

| Quantity | Value | Mark |
| --- | --- | --- |
| Earth–Moon mass ratio μ = M_Moon / (M_Earth + M_Moon) | 0.0121506 | [C] from JPL DE440 GM values |
| Earth/Moon mass ratio | 81.3006 | [C] from JPL DE440 GM values |
| L2 beyond the Moon's center | about 64,500 km (16.78% of the Earth–Moon distance) | [C], matches [V] Wikipedia table |
| L2 from Earth's center | about 448,900 km | [V] |
| L1 short of the Moon's center | about 58,000 km (15.09%) | [C], matches [V] |
| Linear L2 halo period | about 14.7 days in plane, 15.3 days out of plane | [C]; ARTEMIS flew 15.5-day L2 orbits [V] |
| L2 instability e-folding time | about 2.0 days | [C] |
| L2 station-keeping, ARTEMIS P1 (flight) | 7.39 m/s per year projected | [V] Folta et al. 2013 |
| Gateway NRHO maintenance | "a few meters per second per year" or less | [V] Parrish et al. (NTRS) |

See [astrodynamics.md](astrodynamics.md) for the derivations and sources.
