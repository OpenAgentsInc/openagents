# CAD Raytrace UI Toggle + Fallback Parity

Issue coverage: `VCAD-PARITY-103`

## Goal

Lock deterministic parity contracts for vcad raytrace UI mode toggles and fallback behavior when raytracing is unavailable.

## Contracts

- UI store default parity:
  - `renderMode = "standard"`
  - `raytraceQuality = "draft"`
  - `raytraceAvailable = false`
- Initialization parity (`useEngine` + `engine/gpu.ts`):
  - `raytraceAvailable` is true only when GPU init succeeds and ray tracer init succeeds
- Toggle parity:
  - `toggleRenderMode` flips `standard <-> raytrace`
  - keyboard `Alt+R` only toggles when `raytraceAvailable` is true
  - selecting quality from the menu while off first toggles raytrace on, then applies quality
- Visibility/fallback parity:
  - raytrace menu appears only when `raytraceAvailable`
  - overlay/sync are active only when:
    - `renderMode == raytrace`
    - `raytraceAvailable == true`
    - `electronicsActive == false`
  - otherwise viewport falls back to standard rasterized render path

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/raytrace_ui_toggle_fallback_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/raytrace_ui_toggle_fallback_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-raytrace-ui-toggle-fallback-ci.sh
cargo run -p openagents-cad --bin parity-raytrace-ui-toggle-fallback
```
