# Kernel Shell Parity

Issue coverage: `VCAD-PARITY-024`

## Purpose

Integrate deterministic parity support aligned to `vcad-kernel-shell` for hollow/shell substrate behavior.

## Implemented Shell Layer

`crates/cad/src/kernel_shell.rs` now provides:

- `ShellError` (`SurfaceCollapse`, `VertexCollision`, `SelfIntersection`, `UnsupportedSurface`)
- `shell_brep(brep, thickness)` with automatic analytical -> mesh fallback path
- `shell_brep_analytical(brep, thickness, open_face_ids)` for planar-face analytical shelling
- `shell_mesh(mesh, thickness)` for mesh-based shelling

Analytical subset behavior in parity lane:

- planar-face solids (for cube baseline) are handled analytically
- non-planar surfaces return `ShellError::UnsupportedSurface` and `shell_brep` falls back to mesh shelling
- thickness that collapses body span returns `ShellError::SurfaceCollapse`

Error contracts:

- invalid thickness in `shell_brep` / `shell_mesh` maps to `CadError::InvalidParameter`

## Parity Artifact

- `crates/cad/parity/kernel_shell_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-shell
scripts/cad/parity-kernel-shell-ci.sh
```

## Determinism Contract

- closed-cube analytical shell locks face count to `12`.
- single-open-face cube analytical shell locks face count to `15`.
- mesh shell path locks vertex/triangle doubling behavior.
- collapse diagnostics are fixture-locked via manifest.
- `crates/cad/tests/parity_kernel_shell.rs` enforces fixture equivalence.
