# Pylon Distributed Training Apple Rehearsal Matrix

This document is the retained Phase 6 release gate for admitted Apple Silicon
training support.

It exists to answer one question honestly:

**Can `Psionic`, `Pylon`, and `Nexus` already support one bounded Apple-homogeneous
training lane, validate it under the same admitted-node policy contract, and
coexist beside CUDA windows without silently widening into mixed-backend claims?**

This is not a marketing checklist. It is the launch block for Phase 6 in
`docs/pylon/distributed-training-phase-tracker.md`. If this matrix is not
green, OpenAgents should not claim Apple-capable distributed training or dual
CUDA plus Apple support.

## Claim Boundary

These statements must remain true in docs, code, operator guidance, and
release notes:

- Apple support is still an admitted-node lane.
- Windows stay backend-homogeneous. Apple and CUDA may coexist in one network,
  but they may not mix inside one active window.
- The Apple lane reuses the same run-manifest, receipt, artifact, closeout, and
  TRN event shapes as the CUDA MVP.
- `Psionic` remains the authority for Apple runtime execution, Apple validator
  replay, and Apple checkpoint handoff truth.
- `Pylon` remains the authority for Apple capability detection, local
  supervision, retained upload state, and local checkpoint serving.
- `Nexus` remains the authority for Apple-versus-CUDA scheduling, validator
  family matching, closeouts, reputation, and public coordination truth.
- Passing this matrix does not authorize mixed-backend windows, permissionless
  admission, or trustless hostile-network claims.

If any public or operator-facing artifact violates those statements, the Apple
rollout should stop.

## Canonical Gate

The canonical automation entrypoint is:

```bash
scripts/release/check-pylon-distributed-training-apple-matrix.sh
```

The script writes a retained summary under:

```text
target/pylon-distributed-training-apple-rehearsal/<timestamp>/
```

and fails fast if any required Apple rehearsal bucket fails.

By default it expects a sibling standalone `psionic` checkout at `../psionic`.
Override that with:

```bash
OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic \
scripts/release/check-pylon-distributed-training-apple-matrix.sh
```

This release-gate script still uses the sibling-or-explicit-repo rule. It does
not reuse the broader packaged-runtime auto-discovery path from
`pylon training status` and `pylon doctor`. For nonstandard layouts, export
`OPENAGENTS_PSIONIC_REPO` explicitly.

Like the Phase 5 MVP gate, this is intentionally test-first. The goal is not to
pretend the repo already has one external Apple multi-machine soak harness that
proves more than the code can really prove. The goal is to run the exact retained
checks that currently bind the Apple runtime lane, Apple checkpoint handoff,
Apple validator replay, backend-homogeneous scheduling, and explicit public TRN
publication into one honest gate.

## Deterministic Matrix

| Gate | Scenario | Retained checks | Expected result |
| --- | --- | --- | --- |
| A | Apple single-node dry run | `psionic-train` Apple launch-manifest test plus `Pylon` Apple capability detection and Apple node-record publication test | one Apple-homogeneous worker lane can start under the shared manifest contract and project an admitted Apple node surface |
| B | Apple multi-node rehearsal | `psionic-train` Apple lane admission test plus `Nexus` backend-homogeneous worker scheduling test | Apple workers can be admitted and leased into Apple windows while CUDA workers are kept on separate CUDA windows |
| C | Apple validator accepted case | `psionic-train` Apple validator accepted-score test plus `Nexus` Apple accepted-outcome gate test | Apple validator replay produces accepted evidence and `Nexus` refuses Apple acceptance until evaluation and runtime validation gates pass |
| D | Apple checkpoint restore and rejoin drill | `psionic-train` Apple checkpoint emit, serve, resume, and refusal tests plus `Pylon` checkpoint server test | Apple nodes can persist checkpoint state, serve handoff state, resume from admitted peer checkpoint lineage, and refuse resume when no admitted checkpoint exists |
| E | Dual-backend claim gate | `Nexus` validator-family matching test plus `Nexus` and `Pylon` TRN mapping tests | Apple and CUDA can coexist in one network while validator matching and public TRN publication keep backend-family and environment identity explicit without new event kinds |

## Repo-Level Gate

Run the full retained Phase 6 gate with:

```bash
scripts/release/check-pylon-distributed-training-apple-matrix.sh
```

That script currently expands into these required buckets:

```bash
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_manifest_start_emits_metal_capability_projection -- --nocapture
cargo test -p pylon --lib adapter_training_detection_marks_apple_hosts_ready_when_runtime_and_host_posture_match -- --nocapture
cargo test -p pylon --lib training_trn_mapping_preserves_apple_backend_capabilities_in_node_records -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --lib apple_lane_is_admitted_by_machine_contract -- --nocapture
cargo test -p nexus-control --lib training_scheduler_matches_worker_leases_to_backend_homogeneous_runs -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_validator_manifest_emits_accepted_score_receipt_for_valid_contribution -- --nocapture
cargo test -p nexus-control --lib apple_training_outcomes_require_eval_and_runtime_validation_before_acceptance -- --nocapture

cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_manifest_record_checkpoint_persists_generic_checkpoint_artifacts -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_manifest_serve_checkpoint_retains_primary_handoff_receipt -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_manifest_resume_can_seed_from_peer_checkpoint_handoff -- --nocapture
cargo test --manifest-path ../psionic/Cargo.toml -p psionic-train --test psionic_train_cli apple_manifest_resume_refuses_without_any_admitted_checkpoint -- --nocapture
cargo test -p pylon --lib training_checkpoint_server_serves_local_checkpoint_paths -- --nocapture

cargo test -p nexus-control --lib training_validator_claims_skip_windows_from_mismatched_backend_families -- --nocapture
cargo test -p nexus-control --lib training_trn_mapping -- --nocapture
cargo test -p pylon --lib training_trn_mapping -- --nocapture
```

If those commands do not pass together, Apple support is not ready to claim
launch.

## Scenario Notes

The Phase 6 Apple gate must keep covering:

- one Apple single-node admitted dry run
- one Apple backend-homogeneous worker scheduling path
- one Apple validator accepted path
- one Apple checkpoint write, handoff, resume, and refusal path
- one explicit proof that Apple and CUDA coexist only as separate homogeneous
  windows
- one explicit proof that public TRN state keeps backend-family and
  environment identity visible across shared event shapes

This is the honest Apple bar. It is not a mixed-backend window gate, not a
permissionless miner gate, and not a generalized hostile-network audit program.
