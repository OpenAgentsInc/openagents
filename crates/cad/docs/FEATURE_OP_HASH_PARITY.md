# Feature-Op Hash Parity

Issue coverage: `VCAD-PARITY-038`

## Purpose

Lock deterministic feature-op hash behavior against a pinned vcad reference corpus.

The reference corpus is stored at:

- `crates/cad/parity/fixtures/feature_op_hash_vcad_reference_corpus.json`

and is aligned to vcad feature parameter semantics from:

- `~/code/vcad/crates/vcad-app/src/materializer.rs`

at pinned commit `1b59e7948efcdb848d8dba6848785d57aa310e81`.

## Covered Hash Contracts

- primitive box hash
- primitive cylinder hash
- transform feature hash
- cut-hole feature hash
- linear pattern aggregate hash
- circular pattern aggregate hash
- fillet placeholder hash
- sweep feature hash
- loft feature hash (open + closed)

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-feature-op-hash -- --check`
- Manifest fixture:
  - `crates/cad/parity/feature_op_hash_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad parity_feature_op_hash --quiet`

## Failure Modes

- Any hash mismatch between OpenAgents and corpus expected hashes marks parity mismatch.
- Corpus vcad commit drift is surfaced as `reference_commit_match = false`.
- Non-deterministic replay for identical feature-op inputs fails the replay contract.
