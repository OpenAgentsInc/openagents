# CAD Intent Modeling Parity

Issue coverage: `VCAD-PARITY-087`

## Goal

Match vcad intent-based modeling execution semantics for parse/infer/confirm/apply/fallback behavior.

Reference source:

- `~/code/vcad/docs/features/intent-based-modeling.md`

## OpenAgents Parity Surface

- Execution module: `crates/cad/src/intent_execution.rs`
- Parity builder: `crates/cad/src/parity/intent_modeling_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-intent-modeling.rs`
- Fixture: `crates/cad/parity/fixtures/intent_modeling_vcad_reference.json`
- Manifest: `crates/cad/parity/intent_modeling_parity_manifest.json`

## Contracts Enforced

- `intent_json` path dispatches typed intents deterministically.
- Natural-language path emits confirmation gate before mutation.
- Confirmed natural-language path mutates deterministic dispatch revision.
- Ambiguous prompt path emits deterministic clarification code + recovery hint.
- Snapshot replay is deterministic across repeated runs.

## Commands

Generate/refresh manifest:

```bash
cargo run -p openagents-cad --bin parity-intent-modeling
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-intent-modeling -- --check
```

CI lane:

```bash
scripts/cad/parity-intent-modeling-ci.sh
```
