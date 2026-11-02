# 41. Burns consume mass

> **Status: unimplemented — design draft.** Aethyr's thrusters already burn
> tanks with Tsiolkovsky delta-v (`thrust_apply`, `delta_v`, `fuel_for` in
> [`aethyr/thrust.coax`](../../aethyr/thrust.coax); `orbit_burn_step` in
> [`aethyr/orbit.coax`](../../aethyr/orbit.coax)); this chapter plans
> variable-mass flight with separate mass and energy accounts. Nothing here
> runs yet.

A rocket changes velocity by throwing mass away, so its mass, centre of mass
and inertia all change during a burn. A game can treat fuel as a meter; an
engineering model cannot, because the mistakes it hides (counting propellant
twice, thrust that exceeds the power supplied, torque from a thrust line
that misses the centre of mass) are exactly what a study needs to see.

## What this chapter teaches

- Thrust from mass flow and exhaust speed, `F = ṁ v_e`.
- Separate accounts for energy source, reaction mass, coolant and
  consumables.
- Jet power `P = F v_e / 2`, which ties thrust, exhaust speed and power.
- Torque from off-centre thrust, and attitude with inertia recomputed as
  mass moves.
- Constant-thrust and constant-acceleration control histories.

## The method

```jai
F      = ṁ v_e                     // thrust
Δv     = v_e ln(m₀ / m_f)          // rocket equation, no external force
P_jet  = ½ ṁ v_e² = ½ F v_e        // ideal directed exhaust
τ      = Σ_i (r_i − r_com) × F_i   // torque about the centre of mass
I ω̇ + ω × (I ω) = τ                // Euler, I recomputed between steps
```

A ship holds tanks by account: `energy` (what powers the drive), `reaction`
(what leaves as exhaust), `coolant` and `consumables`. A drive declares
which accounts it draws from and in what ratio; when a fuel's products also
leave as exhaust, the ledger records that one mass in one account, never
two. The centre of mass and inertia tensor are recomputed from the tanks'
fill levels and positions after each step.

A configuration is admitted only if its thrust and exhaust speed fit its
power: `F ≤ 2 η P_available / v_e`. The ideal-exhaust example from the
[design reference](../../docs/research/expanse-physics-and-simulation-design-reference.md)
(1,000 t at 1 g, `v_e` = 10,000 km/s) needs 9.807 MN, 0.981 kg/s and
49.03 TW of jet power; a design claiming that thrust from less power is
refused. Under a constant-acceleration command, demanded thrust falls as
mass is consumed; under constant thrust, acceleration rises.

## What exists today

[`aethyr/thrust.coax`](../../aethyr/thrust.coax) has `Thruster` (a force at a
body point along a body direction, with exhaust speed), `Tank` and `Burn`;
`ve_of_isp`, `delta_v`, `fuel_for`, `delta_v_left`, `thrust_apply`,
`rcs_apply` and `rcs_hold`. A tank holds real propellant or a game budget
(a boost meter). [`aethyr/orbit.coax`](../../aethyr/orbit.coax) adds
`orbit_delta_v`, `orbit_propellant`, `orbit_burn_time` and
`orbit_burn_step` (RK4 with mass flowing); its check gives the rocket
equation's delta-v to 1e-7 for Lagrange 2's tug (120 kN at Isp 330 s).
There is one propellant account, inertia does not change with tank level,
and nothing checks power.

## What it builds

- `aethyr/space/mass.coax`: the account ledger, tank placement, centre of
  mass and inertia recomputation.
- `aethyr/space/drive.coax`: drives with mass flow, exhaust speed, power
  draw and efficiency, and the power admission check.
- `aethyr/space/burn.coax`: constant-thrust and constant-acceleration
  histories over chapter 40's propagators.

## Acceptance checks and oracles

- Δv against `v_e ln(m₀/m_f)` across single and multiple burns
  ([NASA ideal rocket equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/ideal-rocket-equation/)).
- No propellant counted twice: the ledger's total mass equals ship plus
  expelled mass at every step.
- Momentum conserved only for ship plus exhaust, not for the ship alone.
- A displaced centre of mass produces the expected torque and spin-up.
- A configuration whose thrust and exhaust speed exceed its declared power
  is refused with the binding quantity named.

## Dependencies

[Chapter 40](ch040-propagate-with-a-stated-error.md)'s propagators;
[chapter 18](ch018-rigid-bodies-and-time-steps.md)'s rigid bodies;
[chapter 2](ch002-spaces-and-units.md)'s units. From Coax: R12 units, R15
integrators.
