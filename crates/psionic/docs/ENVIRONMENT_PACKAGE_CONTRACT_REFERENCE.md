# Environment Package Contract Reference

> Status: canonical `#3577` environment-package record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-environment-package-contract.sh`.

This document records the first typed package-shape layer for
`psionic-environments`.

## What Landed

The issue widened `EnvironmentPackageContract` with:

- `EnvironmentWorkloadClass` so one package can power SFT, RL, online eval,
  offline eval, and validator-benchmark workloads
- `EnvironmentPolicyReference` for training, reward, safety, verification, and
  benchmark policies
- `EnvironmentDifficultyMetadata` for difficulty-tier and selection hints
- `EnvironmentBenchmarkProfile` for benchmark identity, runtime profile,
  verification posture, and execution-strategy expectations
- digest and validation coverage for all of the above

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-environment-package-contract.sh
```

## Workload Shape

The current reference path proves one bounded but real package-shape workload:

1. define a versioned environment package
2. declare multiple supported workload classes on the same package
3. attach typed policy refs and difficulty metadata
4. attach one reusable benchmark profile
5. validate and digest the package without falling back to free-form metadata

## Pass Criteria

The package-shape layer is green only if all of the following are true:

- workload use is typed and inspectable
- policy refs, difficulty metadata, and benchmark profiles are validated
- package digests include the new fields
- the same package can describe ordinary runtime use and validator-local
  benchmark simulation

## Expected Signals

The current harness should prove:

- one environment package can declare RL, eval, and validator-benchmark use
- policy refs and benchmark profiles survive validation
- package digests remain stable
- benchmark profile identity is pinned directly in the package manifest

## Current Limitations

This issue intentionally does not claim:

- environment install or composition registry flows
- dynamic dependency resolution across mixed environment groups
- benchmark execution or adjudication by itself
- broader sandbox pooling or repeated-environment throughput

Those remain later issues. This issue makes package shape and benchmark profile
truth real first.
