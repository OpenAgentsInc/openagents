# CAD GPU Acceleration Parity

Issue coverage: `VCAD-PARITY-095`

## Goal

Establish deterministic parity evidence for vcad's GPU acceleration lane contracts (backend profile, exported operations, and core behavior invariants) before deeper mesh/raytrace GPU work.

## Contracts

- Backend profile matches vcad GPU lane:
  - crate identity: `vcad-kernel-gpu`
  - `wgpu` major version: `23`
  - backend flags: `webgpu`, `webgl`
  - native runtime expects `pollster` for blocking init
- Exported capability surface includes:
  - `compute_creased_normals`
  - `decimate_mesh`
- GPU context error taxonomy is parity-locked:
  - `NoAdapter`, `AlreadyInitialized`, `DeviceRequest`, `BufferMapping`, `NotInitialized`
- Creased-normal empty-input behavior is deterministic:
  - output length mirrors `positions` length
- Decimation ratio behavior is parity-locked:
  - `target_ratio` clamps to `[0.1, 1.0]` before target-triangle computation.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/gpu_acceleration_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/gpu_acceleration_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-gpu-acceleration-ci.sh
cargo run -p openagents-cad --bin parity-gpu-acceleration
```
