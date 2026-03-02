# Parity Scorecard

Issue coverage: `VCAD-PARITY-005`

## Purpose

Define deterministic parity scorecard metrics and explicit pass/fail thresholds based on
the machine-readable gap matrix.

Input:

- `crates/cad/parity/vcad_openagents_gap_matrix.json`

Output:

- `crates/cad/parity/parity_scorecard.json`

## Threshold Profiles

The scorecard defines two threshold profiles:

1. `phase_a_baseline_v1`
2. `parity_complete_v1`

Both profiles evaluate:

- docs match rate
- crates match rate
- commands match rate
- overall match rate

## Commands

Generate/update scorecard:

```bash
cargo run -p openagents-cad --bin parity-scorecard --
```

Full drift check (crawlers + gap matrix + scorecard):

```bash
scripts/cad/parity-scorecard-ci.sh
```

## Test Coverage

- `crates/cad/tests/parity_scorecard.rs`
  - fixture schema/invariant checks
  - generation parity check against committed fixture
- unit tests live in:
  - `crates/cad/src/parity/scorecard.rs`
