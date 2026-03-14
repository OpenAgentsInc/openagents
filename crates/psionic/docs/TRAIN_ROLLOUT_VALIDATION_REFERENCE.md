# Train Rollout Validation Reference

> Status: canonical `#3576` rollout-validation record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-rollout-validation.sh`.

This document records the first validator-ready rollout integrity layer in
`psionic-train`.

## What Landed

The issue added a new `rollout_validation` module inside `psionic-train` with:

- `RolloutVerificationBundle` over one rollout artifact, worker outcome, and
  optional benchmark observation or expectation
- `RolloutValidatorPolicy` for execution-proof requirements, deterministic
  sampled expensive checks, deterministic benchmark checks, and duplicate
  normalization posture
- `ValidatorVerdict` with typed reason codes for stale-policy rejection,
  replayed artifacts, duplicate outputs, normalized contributions, timer
  integrity, token accounting, final-state mismatches, and declared execution
  strategy mismatches
- stateful replay and duplicate detection through seen artifact digests and
  response-signature history

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-rollout-validation.sh
```

## Workload Shape

The current reference path proves one bounded but real rollout-validation
workload:

1. validate one fresh accepted rollout bundle
2. reject a stale-policy bundle using typed verdict reasons
3. reject replayed artifacts by exact digest
4. normalize copycat or duplicate outputs by response signature
5. run benchmark-gated sampled adjudication for timer, token, final-state, and
   execution-strategy checks

## Pass Criteria

The rollout-validation layer is green only if all of the following are true:

- validator policy is explicit typed state rather than hidden service config
- replay and duplicate detection are machine-legible
- benchmark-class checks are typed and policy-gated rather than hard-coded
- verdicts can explain normalized outcomes as distinct from simple accept or
  reject

## Expected Signals

The current harness should prove:

- exact replay returns `replayed_artifact_detected`
- near-duplicate outputs can return `duplicate_detected` plus
  `contribution_normalized`
- stale or discarded worker outcomes return `stale_policy_rejected`
- benchmark mismatches return timer, token, final-state, and
  execution-strategy reason codes directly in the verdict

## Current Limitations

This issue intentionally does not claim:

- external validator-service deployment or scheduling
- trainer-batch or eval-class validator verdicts
- challenge-market or authority integration
- rollout-native datastream artifact subjects

Those remain later issues. This issue makes validator-ready rollout contracts
real first.
