# Pattern Parity

Issue coverage: `VCAD-PARITY-028`

## Purpose

Lock deterministic parity contracts for pattern feature ops:

- linear pattern (count + spacing + normalized direction)
- circular pattern (count + span angle + radius + axis origin/direction)

## Implemented Pattern Contract Layer

- Extended feature-op layer with circular pattern support:
  - `CircularPatternFeatureOp`
  - `CircularPatternInstance`
  - `CircularPatternFeatureResult`
  - `evaluate_circular_pattern_feature`
- Existing linear pattern contracts remain fixture-locked.
- Added deterministic parity lane:
  - `crates/cad/src/parity/pattern_parity.rs`
  - `crates/cad/src/bin/parity-pattern.rs`
  - `crates/cad/tests/parity_pattern.rs`
  - `scripts/cad/parity-pattern-ci.sh`
  - `crates/cad/parity/pattern_parity_manifest.json`

## Contracts Locked

- Pattern counts include the original instance (`count=1` yields one instance).
- Linear pattern direction is normalized before spacing offsets are applied.
- Circular pattern uses uniform angular steps (`span_deg / count`) around axis origin + direction.
- Circular pattern full-span loops do not emit a duplicated endpoint copy.
- Invalid linear count and invalid circular axis/radius map to stable CAD diagnostics.

## Parity Artifact

- `crates/cad/parity/pattern_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-pattern
scripts/cad/parity-pattern-ci.sh
```
