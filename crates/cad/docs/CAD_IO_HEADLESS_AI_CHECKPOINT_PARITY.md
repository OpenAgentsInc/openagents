# CAD IO/Headless/AI Checkpoint Parity

Issue coverage: `VCAD-PARITY-092`

## Goal

Validate that all Phase G parity issues (`VCAD-PARITY-079..091`) are complete, present, and correctly linked before Phase H starts.

## OpenAgents Parity Surface

- Checkpoint module: `crates/cad/src/parity/io_headless_ai_checkpoint_parity.rs`
- CLI parity generator: `crates/cad/src/bin/parity-io-headless-ai-checkpoint.rs`
- Manifest: `crates/cad/parity/io_headless_ai_checkpoint_parity_manifest.json`
- Test: `crates/cad/tests/parity_io_headless_ai_checkpoint.rs`
- CI lane: `scripts/cad/parity-io-headless-ai-checkpoint-ci.sh`

## Contracts Enforced

- All required Phase G manifests (`079..091`) exist.
- Every required Phase G manifest `issue_id` matches its expected issue.
- `crates/cad/docs/VCAD_PARITY_PLAN.md` marks every Phase G item (`079..092`) checked.
- Completion is exactly `100.0%` before checkpoint pass.

## Commands

Generate/refresh checkpoint manifest:

```bash
cargo run -p openagents-cad --bin parity-io-headless-ai-checkpoint
```

Check manifest lock:

```bash
cargo run -p openagents-cad --bin parity-io-headless-ai-checkpoint -- --check
```

CI lane:

```bash
scripts/cad/parity-io-headless-ai-checkpoint-ci.sh
```
