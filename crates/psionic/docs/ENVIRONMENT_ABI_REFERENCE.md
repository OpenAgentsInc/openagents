# Environment ABI Reference

> Status: canonical `#3566` environment-runtime record, updated 2026-03-15
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

On 2026-03-15, issue `#3622` extended that same crate with a repo-owned Apple
adapter environment bundle:

- a shared train/eval core package plus a benchmark-only package
- typed Apple runtime/session requirements and stable package refs for session,
  runtime, tool-bundle, rubric-binding, and structured-output profiles
- Apple metadata that now participates in `EnvironmentPackageContract` digests
  instead of sitting in unused extension fields

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

The Apple adapter extension now proves a second bounded workload:

1. define one shared Apple train/eval package and one benchmark package
2. attach explicit Apple runtime/session/tool/rubric refs as typed metadata
3. reuse the same pinned core package across train and eval
4. keep benchmark-only requirements isolated in a separate package

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
- Apple adapter environment metadata is explicit and digest-bound rather than
  hidden in desktop-local configuration

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
