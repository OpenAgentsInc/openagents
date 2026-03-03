# CAD Text-to-CAD Training/Eval Parity

Issue coverage: `VCAD-PARITY-090`

## Goal

Validate deterministic, gated training/eval hook behavior for text-to-cad datasets.

Reference source:

- `~/code/vcad/docs/features/text-to-cad.md`

## OpenAgents Parity Surface

- Hook module: `crates/cad/src/text_to_cad_training_eval.rs`
- Parity builder: `crates/cad/src/parity/text_to_cad_training_eval_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-text-to-cad-training-eval.rs`
- Fixture: `crates/cad/parity/fixtures/text_to_cad_training_eval_vcad_reference.json`
- Manifest: `crates/cad/parity/text_to_cad_training_eval_parity_manifest.json`

## Contracts Enforced

- Disabled-by-default gate path emits deterministic gate code + env contract.
- Enabled path emits deterministic train/eval split hashes.
- Split total matches dataset sample count.
- Hook record NDJSON export is deterministic.

## Commands

Generate/refresh manifest:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad-training-eval
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad-training-eval -- --check
```

CI lane:

```bash
scripts/cad/parity-text-to-cad-training-eval-ci.sh
```
