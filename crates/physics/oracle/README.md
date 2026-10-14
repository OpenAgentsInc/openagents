# Genesis oracle

Offline calibration for the `physics` crate (GP-8,
[#9785](https://github.com/OpenAgentsInc/openagents/issues/9785)). Genesis is a
reference only: no Verse build, test, or runtime depends on it.

A scene in [`src/oracle.rs`](../src/oracle.rs) describes bodies, one shape
each, initial motion, and materials, with gravity off so the engines' axis
conventions do not matter. Both engines run it and write a trace; the
difference between them is the evidence a fidelity claim must rest on.

## Run

Install Genesis from the pinned reference clone into a scratch virtual
environment (about 2 GB with PyTorch), then:

```sh
cargo run -p physics --example oracle_scenes -- target/oracle
python crates/physics/oracle/genesis_oracle.py target/oracle tank_into_panel
cargo run -p physics --example oracle_scenes -- target/oracle compare
PHYSICS_ORACLE_DIR=target/oracle cargo test -p physics -- --ignored oracle
```

`genesis_oracle.py` builds each scene as inline MJCF with the exact mass and
inertia, at 64-bit precision on the CPU backend, with the elliptic friction
cone.

## What differs by design

Genesis's rigid solver has soft, MuJoCo-style contacts with no restitution
coefficient; `physics` uses restitution with a speculative margin. A bounce
therefore differs, and so does everything after it. Compare the approach, the
first contact time, and the direction of the impulse before comparing
rebound speeds.

## Results

2026-09-27, Genesis `23671976` (v1.4.2 + 20) on an Apple M5 Max, CPU
backend, `tank_into_panel` (a 320 kg tank spinning into a fixed panel's edge
at 0.85 m/s):

| Phase | Largest difference |
| --- | --- |
| Approach, ticks 1 to 8 | 1.7e-20 m: same frames, inertia, and initial motion |
| First contact | Tick 9 here, tick 10 in Genesis: the speculative margin starts it a step early |
| After the impact (second half of the run) | Velocity 0.18 m/s, body rate 0.012 rad/s |

After the hit, the tangential velocity (-0.38 against -0.41 m/s) and the spin
about the impact axis (0.230 against 0.218 rad/s) agree within about 8%. The
rebound differs: 0.28 m/s here with restitution 0.3 against 0.11 m/s from
Genesis's soft contact. The ignored test's bar (5 cm, 0.1 m/s) is not met, so
no fidelity claim is made. Matching the rebound needs either a comparable
restitution model on the Genesis side or a softness parameter here, which is
follow-up work.

## Articulated-joint spike

[#11607](https://github.com/OpenAgentsInc/openagents/issues/11607) adds limited
hinges, sliders, and implicit PD servos. `mujoco_articulated.py` generates
the committed `articulated-reference.json` with MuJoCo **3.3.7**; its original
models and output are CC0-1.0. The script is offline acceptance tooling.
Cargo replays the recorded actions without importing Python or MuJoCo.

On `coderos-4080`, Linux x86-64, 2026-10-14, at **240 Hz** with **80 solver
iterations**, the largest coordinate differences were **0.011115 rad** for
the hinge, **0.003762 m** for the slider, and **0.022752 rad** for the six-link
chain under gravity. The chain's largest anchor gap was **0.000117 m**.
The limits are 0.02 rad or m for single joints, 0.025 rad for the chain, and
0.001 m for hinge anchor gaps. A separate 0.5 rad step test requires a
90% rise within 0.5 s, overshoot below 5%, and capped motor effort and speed.
These fixtures qualify this spike, not arbitrary robot manipulation.

Regenerate with `python mujoco_articulated.py articulated-reference.json`
from this directory in an isolated environment with `mujoco==3.3.7`.
Run `cargo test -p physics --test articulated -- --nocapture` to compare.
