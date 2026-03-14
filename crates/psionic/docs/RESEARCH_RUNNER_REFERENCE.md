# Psionic Research Runner Reference

`psionic-research-runner` is the compiled bounded runner for local-first
hillclimb experiments.

## Contract

The runner accepts a JSON `ResearchRunnerInvocation` and emits:

- one typed `ExperimentResult` JSON manifest
- one stdout log file
- one stderr log file

The invocation already includes the typed `ExperimentSpec`, so the binary does
not accept ad hoc metric flags or handwritten benchmark parameters.

## CLI

```text
cargo run -p psionic-research --bin psionic-research-runner -- \
  --invocation /path/to/invocation.json \
  --result /path/to/result.json
```

`result.json` is the manifest path. The runner writes sibling files:

- `result.stdout.log`
- `result.stderr.log`

## Failure Semantics

The runner keeps failure outcomes typed:

- missing sandbox/runtime profile -> `sandbox_mismatch` with
  `missing_execution_profile`
- budget too small for the declared candidate -> `timed_out` with
  `budget_too_small`

That keeps controller logic out of raw process logs and makes the same contract
usable for later remote execution.

## Current Representative Families

The runner currently synthesizes bounded outputs for the same typed families
owned by `psionic-research`, with special receipt shapes for:

- serving scheduler experiments
- training policy experiments
- validator policy experiments

The first reference program still targets serving scheduler policy, but the
runner contract already leaves room for training-policy hillclimbs.
