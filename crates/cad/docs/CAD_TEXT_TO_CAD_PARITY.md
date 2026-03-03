# CAD Text-to-CAD Parity

Issue coverage: `VCAD-PARITY-088`

## Goal

Provide vcad-aligned text-to-cad adapter parity for prompt->model->compact IR behavior, including fallback clarification contracts.

Reference source:

- `~/code/vcad/docs/features/text-to-cad.md`

## OpenAgents Parity Surface

- Adapter module: `crates/cad/src/text_to_cad.rs`
- Parity builder: `crates/cad/src/parity/text_to_cad_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-text-to-cad.rs`
- Fixture: `crates/cad/parity/fixtures/text_to_cad_vcad_reference.json`
- Manifest: `crates/cad/parity/text_to_cad_parity_manifest.json`

## Contracts Enforced

- Default model profile resolves to `cad0`.
- Offline profile resolves to `cad0-mini`.
- Generated compact IR is deterministic and parse-stable.
- `cad0` path emits richer operation count than `cad0-mini` on shared prompts.
- Ambiguous prompts emit deterministic clarification payloads.

## Commands

Generate/refresh manifest:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-text-to-cad -- --check
```

CI lane:

```bash
scripts/cad/parity-text-to-cad-ci.sh
```
