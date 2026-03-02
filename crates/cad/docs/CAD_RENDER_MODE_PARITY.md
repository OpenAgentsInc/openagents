# CAD Render Mode Parity

Issue coverage: `VCAD-PARITY-094`

## Goal

Lock deterministic parity contracts for render mode variants (`standard`, `wire`, `hidden-line`) using vcad store semantics as the reference baseline.

## Contracts

- Default render state mirrors vcad store defaults:
  - `renderMode = standard`
  - `showWireframe = false`
  - drawing `viewMode = 3d`
  - drawing `showHiddenLines = true`
- Wireframe toggle parity:
  - `showWireframe` transitions `false -> true -> false`
- Hidden-line toggle parity:
  - drawing `showHiddenLines` transitions `true -> false -> true`
- Variant style profiles are deterministic:
  - `standard`: face fill on, edge overlay off
  - `wire`: face fill off, edge overlay on, hidden edges off
  - `hidden-line`: face fill off, edge overlay on, hidden edges dashed
- Variant cycle order is deterministic:
  - `standard -> wire -> hidden-line -> standard`
- Alias parsing is deterministic (`wireframe`, `hidden_line`, etc.) with unknown tokens falling back to `standard` diagnostics.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/render_mode_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/render_mode_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-render-mode-ci.sh
cargo run -p openagents-cad --bin parity-render-mode
```
