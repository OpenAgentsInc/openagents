# Environment ABI Reference

> Status: canonical `#3566` environment-runtime record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-environment-abi.sh`.

This document records the first Psionic-native environment ABI and runtime
contract.

## What Landed

The issue landed the `psionic-environments` crate with:

- canonical `environment_ref@version` identity through `EnvironmentPackageKey`
- a typed `EnvironmentPackageContract` for runtime execution
- execution entrypoints, tool interfaces, rubric hooks, dataset bindings, and
  expected artifact contracts
- a deterministic in-memory `EnvironmentRuntimeSession`
- session receipts and final session summaries

Kernel and Nexus still own registry and authority truth. This issue only lands
the reusable Psionic-side ABI and runtime contract.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-environment-abi.sh
```

## Workload Shape

The current reference path proves one bounded but real environment session:

1. define a versioned environment package contract
2. open a runtime session against a task
3. run a turn that emits a declared tool call
4. resolve the tool result and complete the turn with emitted artifacts
5. finalize the session with rubric outcomes

## Pass Criteria

The environment layer is green only if all of the following are true:

- the package ABI is typed and validated
- multi-turn sessions can run through a deterministic state machine
- undeclared tools are refused
- missing required artifacts or missing rubric outcomes are refused
- the final session summary is machine-legible and stable-keyed to
  `environment_ref@version`

## Expected Signals

The current harness should prove:

- `EnvironmentPackageKey::storage_key()` matches the kernel-side
  `environment_ref@version` contract
- `EnvironmentPackageContract::stable_digest()` is deterministic
- `EnvironmentRuntimeSession` records turn count, tool invocation count,
  artifacts, and rubric outcomes
- the train substrate can now reference the canonical package key rather than a
  free-form environment string

## Current Limitations

This issue intentionally does not claim:

- eval runtime implementation
- rollout worker or sandbox pooling protocols
- broader benchmark adjudication beyond the package contract itself

Package-shape metadata for workload classes, policy refs, difficulty metadata,
and benchmark profiles now live in
`ENVIRONMENT_PACKAGE_CONTRACT_REFERENCE.md`. Registry install, pinning,
composition, and train/eval parity now live in
`ENVIRONMENT_REGISTRY_REFERENCE.md`. This issue makes the reusable ABI and
runtime session contract real first.
