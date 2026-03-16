# Psionic Research Experiment Reference

This document is the canonical reference for the shared research-layer contracts
owned by `psionic-research`.

## What This Layer Owns

`psionic-research` owns the typed vocabulary for:

- one bounded experiment candidate
- one mutation lineage record
- one result manifest
- one score contract and evaluation
- one promotion record
- one machine-readable sweep record for same-contract candidate comparison

It does not own:

- app-level frontier policy
- authority truth
- settlement or market promotion
- execution engines

Those stay in `apps/autopilot-desktop`, `apps/nexus-control`, and the existing
Psionic runtime/sandbox/train crates.

## Core Contracts

- `ExperimentSpec`
  - typed candidate identity for one bounded run
  - includes base artifact refs, mutation digest, runner binary digest,
    runtime/sandbox request, and score contract
- `CandidateMutation`
  - explicit lineage between frontier candidates
  - records the typed policy surfaces that changed
- `ExperimentResult`
  - bounded run outputs including scores, metrics, receipt refs, produced
    artifact digests, and stdout/stderr digests
- `ExperimentScoreContract`
  - declares which metrics count, their direction, their weight, and optional
    hard-failure gates
- `PromotionRecord`
  - records keep, discard, branch, promote, or blocked decisions
  - distinguishes “better number” from “actually promotable”

## First Experiment Families

The current shared family surface covers:

- `ServingScheduler`
- `BackendTuning`
- `DatastreamTransfer`
- `SandboxWarmPool`
- `TrainingPolicy`
- `ValidatorPolicy`
- `EnvironmentMix`
- `ExecutorVariants`

These families are intentionally typed policy surfaces. The research layer does
not model arbitrary source patches as the default search contract.

`ExecutorVariants` is currently exercised by the `Tassadar` lane. It records
architecture, trace-ABI, Wasm-profile, decode-cache, and attention-mode
surfaces, and the bounded runner evaluates those candidates against the real
Tassadar benchmark backend rather than synthetic executor placeholders. The
family treats benchmark packages, program/model artifacts, runtime manifests,
execution-proof bundles, and benchmark reports as first-class experiment
inputs/outputs.

## Comparison Rules

`ExperimentScoreContract::evaluate_result` produces an
`ExperimentScoreEvaluation` that is strong enough for replay-safe same-contract
comparison:

- hard-gate failures always lose to gate-passing candidates
- missing required metrics mark the result as gate-failed
- weighted scores are only comparable when both sides share the same contract

That is the minimum substrate the app-owned frontier controller needs before it
adds keep/discard/branch policy.
