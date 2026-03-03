# Kernel Text Parity

Issue coverage: `VCAD-PARITY-022`

## Purpose

Integrate parity text geometry support aligned to `vcad-kernel-text` behavior.

## Implemented Text Geometry Layer

`crates/cad/src/kernel_text.rs` now provides:

- `FontRegistry` and built-in sans font profile set
- `TextAlignment` (`Left`, `Center`, `Right`)
- `text_to_profiles(text, font, height_mm, letter_spacing, line_spacing, alignment)`
- `text_bounds(text, font, height_mm, letter_spacing, line_spacing)`
- deterministic `TextProfile` contour output including hole contours

Error contracts:

- invalid layout parameters map to `CadError::InvalidParameter`

## Parity Artifact

- `crates/cad/parity/kernel_text_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-text
scripts/cad/parity-kernel-text-ci.sh
```

## Determinism Contract

- manifest locks profile-count/alignment bounds snapshots and deterministic signatures.
- alignment modes preserve profile counts while shifting x-range.
- `crates/cad/tests/parity_kernel_text.rs` enforces fixture equivalence.
