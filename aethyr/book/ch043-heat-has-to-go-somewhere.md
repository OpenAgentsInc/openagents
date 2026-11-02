# 43. Heat has to go somewhere

> **Status: unimplemented — design draft.** Aethyr's ships have no thermal
> state today: [`aethyr/thrust.coax`](../../aethyr/thrust.coax) burns a tank
> and nothing heats. This chapter plans a lumped thermal network with
> radiators, finite sinks and failures. Nothing here runs yet.

In vacuum there is no air to carry heat away. Whatever a drive, a reactor or
a crew deposits in the hull stays there until it is radiated or carried off
by expelled mass. A small absorbed fraction of a powerful drive is still an
enormous heat load, and that load, not thrust, often limits a design. This
chapter makes heat a conserved quantity with nowhere to hide.

## What this chapter teaches

- A lumped thermal network: drive, power, electronics, occupied spaces, heat
  sinks and radiators as nodes.
- Radiative rejection and its fourth-power law.
- Finite sinks and expendable coolant.
- Declaring hypothetical propulsion heat as a model, never an assumption.

## The method

Each node `i` has heat capacity `C_i` (J/K) and temperature `T_i`:

```jai
C_i dT_i/dt = P_i + Σ_j Q_ji − ε σ A_i (T_i⁴ − T_bg⁴) − P_expelled,i
Q_ji        = G_ji (T_j − T_i)          // conductance between nodes
```

`P_i` is heat generated or absorbed; `P_expelled` consumes coolant mass and
its enthalpy. Conductances and radiator areas can fail: a damaged radiator
loses area, a jammed valve sets a conductance to zero, an exhausted coolant
tank ends expulsion. Sinks buy time; they never delete heat.

Heat from high-power propulsion enters through a declared model: absorbed
fraction, radiator temperature and area, duty cycle. It is explicitly
hypothetical, or a typed rule module (Coax R20), and shows in the scenario
configuration. With the [design reference](../../docs/research/expanse-physics-and-simulation-design-reference.md)'s
example, 0.1% of 49.03 TW of jet power is 49.03 GW of heating. At
equilibrium with a cold background:

```jai
A = P / (ε σ T⁴)
  = 49.03e9 / (0.9 × 5.670374e-8 × 1000⁴)  ≈ 0.961 km²
```

At 1,500 K the same load needs about 0.190 km². A panel radiating from both
faces needs half that geometric area. A design reports the area it needs
rather than drawing whatever radiators fit the silhouette.

The step is stiff when small capacities meet large conductances, so the
network integrates implicitly (backward Euler on the linearized radiative
term, with Newton refinement), and the energy ledger is checked per step.

## What exists today

Nothing thermal. [`aethyr/thrust.coax`](../../aethyr/thrust.coax) and
`orbit_burn_step` in [`aethyr/orbit.coax`](../../aethyr/orbit.coax) model
thrust and mass only; [`verse/stars.coax`](../../verse/stars.coax) draws
the Sun's disc and corona but carries no radiative flux. The momentum ledger
in [`aethyr/rigid.coax`](../../aethyr/rigid.coax) (`Ledger`, `ledger_add`,
`ledger_error`) is the pattern this chapter's energy ledger follows.

## What it builds

- `aethyr/space/thermal.coax`: nodes, conductances, radiators, sinks,
  coolant expulsion, failures and the implicit step.
- `aethyr/space/thermal-models.coax`: declared propulsion heat models, with
  each parameter's provenance.
- A throttle-down rule a drive consumes when a node exceeds its limit.

## Acceptance checks and oracles

- Radiator equilibrium against the closed form: 49.03 GW at 1,000 K and
  ε = 0.9 needs about 0.961 km² of emitting area.
- Energy balance per node and for the whole network, every step.
- A damaged radiator or exhausted coolant produces the expected temperature
  rise and throttle-down.
- A configuration's required radiator area is reported, not hidden.
- Reference: [NASA thermal control](https://www.nasa.gov/smallsat-institute/sst-soa/thermal-control/).

## Dependencies

[Chapter 41](ch041-burns-consume-mass.md)'s drives and accounts;
[chapter 2](ch002-spaces-and-units.md)'s units. From Coax: R12 units, R15
integrators, R20 typed rule modules and provenance.
