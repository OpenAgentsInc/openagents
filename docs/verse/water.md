# Verse water

Water is being ported from the previous Rust engine to Coax in the phases
of its specification, [verse/everglade/previous/water.md](../../verse/everglade/previous/water.md)
(copied from `0b401561a0^`; see its phased plan). Only phase W1 is done.
The other phases wait on the owner (project task `verse-water`, #c278).

## W1: the physics (done)

[`verse/water.coax`](../../verse/water.coax) is a port of the previous
engine's `physics::water` at its W1 commit (`git show
b399fa2dbe:crates/physics/src/water/`), with the same formulas, constants
and tests. It covers what the specification's "Coupling with
`crates/physics`" section and its W1 status list:

- **Bodies.** `WaterBody`: pond, river, waterfall, ocean, pool, puddle or
  marsh; an outline (polygon, river `Course` with widths, or everywhere);
  a level (constant, or a profile along the course); a density (`FRESH`,
  `SALT`); up to eight Gerstner terms; an optional current.
- **Surface with deterministic time.** `WaveSet`: Gerstner waves whose
  frequencies snap to whole multiples of 2π/T, so each phase is an integer
  counter. `water_body_sample` at tick N equals stepping
  (`water_advance`) from tick 0 bit for bit, and the clock folds on the
  period. `water_seeded` draws terms from a SplitMix64 seed.
- **Point queries.** `WaterSet` grids a zone's bodies so `water_sample` and
  `water_bodies_overlapping` test only the bodies whose bounds cover a cell.
- **Flow grid.** `FlowGrid` (still, uniform, or `water_flow_river`:
  divergence-free potential flow round obstacles from the stream function
  by successive over-relaxation).
- **Buoyancy, drag, currents.** `water_submerged` (exact spherical caps,
  sliced capsules, clipped cuboids fitted to rough water) and
  `water_apply`: buoyancy at the centre of buoyancy, linear and quadratic
  drag relative to the water, angular damping. Bodies on still water count
  as supported, so they sleep; waves and currents keep them awake.
- **Ledger terms.** Each force is a `Push` under `water.buoyancy` or
  `water.drag`; `water_record` enters them into a momentum `Ledger`.

[`verse/rigid.coax`](../../verse/rigid.coax) holds the rigid bodies water
pushes on: the old `body.rs`, `World::step` without contacts, sleep with
support, and `ledger.rs`.

All fifteen of the old tests are ported as `#test` checks with the old
scenes, durations and tolerances (rigid.coax adds four of its own). Run
them with:

```sh
coax/bin/coax test --library verse/water.coax --steps 500000000
coax/bin/coax test --library verse/rigid.coax
```

### Differences from the Rust W1

- Coax has no growable arrays, traits, maps or serde yet. Polygons,
  courses, profiles, flow grids, a set's grid cells and pushes live in the
  caller's arrays and records hold views of them. `Water` (the trait) is a
  `WaterSet`; a single body is a set of one.
- A `WaterSet` whose storage is too small for the requested cell doubles
  the cell until it fits. Samples are the same at any cell size.
- Ledger terms are an enum (`Term`), not strings in a map.
- `WaveSet` refuses a period of 2^32 ticks or more, so the phase products
  are exact in `u64` (the Rust used `u128`).
- The rigid bodies have one collider each at their centre, all dynamic. No
  contacts, joints or kinematic bodies: W1's scenes need none.
- The old restore test round-tripped the world and water through JSON.
  Here it copies them and checks that the copy continues bit for bit.
- Not ported: the `water_buoyancy` timing example and the Water Lab's
  floats, which belonged to the old app.
- Coax has no `acos` or `atan` intrinsic, so rigid.coax defines `acos_d`
  and `atan_d` (checked against `cos` and `tan`).

## Next

W2 (the water shader), W3 (Everglade's swimmable water), and the later
phases are in the specification's phase table. None has started; they wait
on the owner.
