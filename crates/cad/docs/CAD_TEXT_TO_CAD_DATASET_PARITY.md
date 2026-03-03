# CAD Text-to-CAD Dataset Parity

Issue coverage: `VCAD-PARITY-089`

## Goal

Lock deterministic dataset-generation and annotation-tooling behavior for text-to-cad training/eval inputs.

Reference source:

- `~/code/vcad/docs/features/text-to-cad.md`

## OpenAgents Parity Surface

- Dataset module: `crates/cad/src/text_to_cad_dataset.rs`
- Parity builder: `crates/cad/src/parity/text_to_cad_dataset_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-text-to-cad-dataset.rs`
- Fixture: `crates/cad/parity/fixtures/text_to_cad_dataset_vcad_reference.json`
- Manifest: `crates/cad/parity/text_to_cad_dataset_parity_manifest.json`

## Contracts Enforced

- Deterministic sample counts by family and model profile.
- Annotation coverage for operation/root/token metadata.
- Stable dataset and NDJSON hashes for replay lanes.
- Deterministic parity snapshot across repeated generation.

## Commands

Generate/refresh manifest:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad-dataset
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad-dataset -- --check
```

CI lane:

```bash
scripts/cad/parity-text-to-cad-dataset-ci.sh
```
