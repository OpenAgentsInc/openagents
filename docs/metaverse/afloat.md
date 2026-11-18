# Floating bodies

`metaverse-engine/physics/afloat.coax` says what a barge, a caisson, a tug or a
hull made of joined pieces does on the sea, from its mass and its shape. It is
planning-level naval architecture: the sums a marine operation is first sized
with, not a seakeeping code. It draws nothing and keeps no state of its own.

SLAB's construction simulation is the first user:
`metaverse/slab-build.coax` floats, tows, joins and moors every body with it.

Units are metres, seconds, kilograms, newtons and radians. `x` runs along the
body, `z` across it, `y` up.

## What you can ask

| You want | Call |
| --- | --- |
| A waterplane from rectangles (one a piece of a joined hull) | `afloat_plane(rects, n)` |
| Draft, heel, trim and both metacentric heights of a body; whether it sinks, capsizes or puts its deck under | `afloat_statics(plane, load, rects, n)` with an `AfloatLoad` (mass, centre of gravity, free surfaces, depth). The answer's `state` is `AFLOAT_OK`, `AFLOAT_SINKS`, `AFLOAT_CAPSIZES` or `AFLOAT_DECK_UNDER` |
| A sea state as wave trains, the same for every caller with the same seed | `afloat_sea(hs, tp, dir, depth, seed, stream)`; the surface at a point and time is `afloat_eta` |
| A wave number in water of a given depth; a wind sea's peak period from its height | `afloat_wave_number(omega, depth)`, `afloat_peak_period(hs)` |
| A body's heave, roll and pitch in that sea at any time, with no stepping | `afloat_rao(...)` once, then `afloat_motion(rao, sea, px, pz, t)` |
| The same a step at a time | `afloat_step` |
| How much a point of a body moves, and how much two bodies' faces move against each other | `afloat_point_motion`, `afloat_relative_motion` |
| A towline or mooring line as an elastic catenary: tension from span, sag, the tension at its end, a snatch load | `afloat_line_span`, `afloat_line_tension`, `afloat_line_sag`, `afloat_line_end_tension`, `afloat_line_peak` |
| A mooring line from an anchor on the seabed | Not here: `metaverse-engine/physics/moor.coax` (`moor_pull`) |
| A fender's load, energy and squeeze | `afloat_fender`, `afloat_fender_energy`, `afloat_fender_deflection` |
| The pull that holds a hull in a sea, a wind and a current; what a tug pulls at a speed and in a sea; the speed a tow settles at | `afloat_holding_force`, `afloat_tug_pull`, `afloat_tow_speed` |
| A tow along a reach of channel or open water, a step at a time | `afloat_tow_step`. Speed, sheer, towline tension, the water under the keel, the room to the bank and the line's sag are results. `state` becomes `AFLOAT_TOW_AGROUND`, `AFLOAT_TOW_BANK` or `AFLOAT_TOW_PARTED` when that happens |
| The bending moment along a long body on a wave | `afloat_bending`, `afloat_bending_worst` |

## What it leaves out

- Wave loads are Froude-Krylov only: the wave's pressure on the body's buoyancy
  columns. Diffraction and radiation are a damping ratio and an added mass.
- A body is a box below the water: its waterplane does not change with draft.
- A tow has three degrees of freedom (surge, sway, yaw). Its heave, roll and
  pitch come from `afloat_rao`.
- Bending treats the body as rigid.

## Checks

```
coax/bin/coax test --library metaverse-engine/physics/afloat.coax
```

They hold the statics to hand sums, the catenary to
`metaverse-engine/physics/sim/pbd.coax`'s, the stepped motion to the closed
form, and show a tow that grounds, one that strikes a bank and one that parts
its line.
