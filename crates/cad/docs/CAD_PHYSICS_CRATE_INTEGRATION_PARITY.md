# CAD Physics Crate Integration Parity

Issue coverage: `VCAD-PARITY-105`

## Goal

Lock deterministic parity contracts for vcad physics crate integration (`vcad-kernel-physics` + `vcad-sim`) as the baseline for Phase I simulation work.

## Contracts

- Physics crate export parity:
  - `vcad-kernel-physics` exposes `PhysicsWorld`, `JointState`, and gym API (`Action`, `Observation`, `RobotEnv`)
- Simulation lane parity:
  - `vcad-sim` exports both `SimPipeline` (single CPU env) and `BatchSimPipeline` (GPU batch env)
- Engine integration parity:
  - phyz backend gravity remains `[0.0, 0.0, -9.81]`
  - default model timestep remains `1.0 / 240.0`

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/physics_crate_integration_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/physics_crate_integration_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-physics-crate-integration-ci.sh
cargo run -p openagents-cad --bin parity-physics-crate-integration
```
