# 47. Hits through layers

> **Status: unimplemented — design draft.** Aethyr's space combat today
> subtracts hull points (`tac_hurt` in
> [`apps/openagents-web/belt-tactical.coax`](../../apps/openagents-web/belt-tactical.coax)).
> This chapter plans hypervelocity hits as calibrated penetration through a
> ship's component graph. Nothing here runs yet.

At several kilometres a second, a projectile and the wall it hits behave
like fluids: they fragment, melt and vaporize. Simulating that with a
hydrocode costs far more than a game or a training environment can spend.
Instead, this chapter uses response models fitted to published
ballistic-limit equations, and makes damage matter by what it removes: a
coolant line, a power bus, a pressure boundary.

## What this chapter teaches

- A ship as a graph of compartments and components.
- Penetration through layers (sacrificial bumper, rear wall, structure,
  components) from relative velocity, projectile mass and shape, incidence
  and material.
- Response tables fitted to ballistic-limit equations, with refusal outside
  their range.
- Fragments that become tracked bodies and debris that stays cosmetic.

## The method

A hit is a ray through the layer stack along the impact direction. Each
layer `k` has a material, thickness and standoff. A ballistic-limit equation
gives the critical projectile diameter `d_c(v, θ, ρ_p, layers)`; for a
Whipple shield it has distinct low-speed, shatter and hypervelocity regimes.
The response model is a table over `(v, θ, m, shape, material)` fitted to
those equations at sampled points, interpolated within its hull and refused
outside it:

```jai
Penetration :: struct {
    perforated: [MAX_LAYERS] bool;
    residual_mass: f64; residual_velocity: D3;
    fragments: int;              // above the tracking threshold
}
```

Perforating a layer that carries a graph edge (a coolant line between two
thermal nodes, a power bus, a door between gas volumes, a structural joint)
removes or degrades that edge. Failures then propagate through earlier
chapters: lost coolant raises drive temperature in chapter 43, which cuts
thrust in chapter 41, which changes the arrival solution in chapter 42; a
breached boundary starts a leak in chapter 44. Fragments above a mass and
energy threshold become bodies with trajectories; the rest are visual.
Impulse from the hit, recoil and venting gas act on the ship's body.

## What exists today

Belt 2200's tactical layer has `TacUnit`, `TacMissile`, `TacSlug`,
`TacTracer`, `tac_pass` (a slug passing a target), `tac_hurt` (hull damage)
and `tac_resolve`. [`aethyr/contact.coax`](../../aethyr/contact.coax)'s
`rigid_impulse_at` applies an impulse at a point and its rays
(`ray_triangle`, `ray_sphere`) find entry points. There are no layers,
components or graph.

## What it builds

- `aethyr/space/ship-graph.coax`: compartments, components and typed edges
  (thermal, power, pressure, structure), shared with chapters 43 and 44.
- `aethyr/space/penetration.coax`: layer stacks and fitted response tables
  with their source equations and fit errors.
- `aethyr/space/fragments.coax`: promotion of fragments to tracked bodies.

## Acceptance checks and oracles

- Tables match their source equations at sampled points within stated fit
  error.
- A query outside the table is refused.
- A severed coolant line raises drive temperature and cuts thrust.
- Recoil and venting change a docking solution.
- Reference: [NASA hypervelocity shield development](https://hvit.jsc.nasa.gov/shield-development/).

## Dependencies

[Chapter 41](ch041-burns-consume-mass.md),
[chapter 43](ch043-heat-has-to-go-somewhere.md) and
[chapter 44](ch044-inside-a-turning-accelerating-ship.md);
[chapter 21](ch021-find-where-they-touch.md) for entry points and
[chapter 6](ch006-entities-are-born-and-die.md) for spawning fragments.
From Coax: R12 units, R20 provenance on fitted parameters.
