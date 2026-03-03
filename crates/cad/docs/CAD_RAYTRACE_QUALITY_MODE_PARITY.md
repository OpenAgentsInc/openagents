# CAD Raytrace Quality Mode Parity

Issue coverage: `VCAD-PARITY-101`

## Goal

Lock deterministic parity contracts for vcad raytrace quality behavior across `draft/standard/high`, including resolution scale factors, per-quality pixel budgets, and interaction-time fallback to draft budgets.

## Contracts

- Quality enum parity:
  - `draft`
  - `standard`
  - `high`
- Scale parity (`RayTracedViewport.tsx`):
  - `draft`: `0.5x`
  - `standard`: `1.0x`
  - `high`: `2.0x`
- Pixel-budget parity (`MAX_PIXELS_BY_QUALITY`):
  - `draft`: `640 * 480`
  - `standard`: `1280 * 720`
  - `high`: `1920 * 1080`
- Interaction parity:
  - when frame index is `<= 1`, effective quality budget is forced to `draft`
- Resolution planning parity:
  - pre-cap render size uses `floor(viewport * selected_scale)`
  - if pre-cap pixels exceed effective budget, both dimensions are downscaled by `sqrt(max_pixels / total_pixels)` and floored

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/raytrace_quality_mode_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/raytrace_quality_mode_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-raytrace-quality-mode-ci.sh
cargo run -p openagents-cad --bin parity-raytrace-quality-mode
```
