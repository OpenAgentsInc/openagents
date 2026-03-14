# Sandbox RL Throughput Reference

> Status: canonical `#3579` sandbox-throughput record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-sandbox-rl-throughput.sh`.

This document records the first Psionic-native RL-throughput sandbox layer.

## What Landed

The issue widened `psionic-sandbox` with:

- typed `SandboxPoolSpec` and `SandboxPoolSnapshot` contracts for warm reusable
  sandbox pools
- explicit `SandboxPoolWarmReceipt` and `SandboxPoolAcquisitionReceipt`
  surfaces so pool readiness and reuse latency are machine-legible
- typed staged-input receipts for command inputs, image frames, and context
  artifacts
- repeated bounded loop execution over one acquired session workspace with
  `SandboxLoopIterationReceipt`
- in-memory pool management that reuses the same workspace across iterations
  instead of forcing a one-shot background-job shape for RL-style loops

This layer still reuses the existing bounded execution engine. The new work is
the RL-friendly control plane and receipt family above it.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-sandbox-rl-throughput.sh
```

## Reference Flow

The current reference path proves a real repeated-loop substrate:

1. create one sandbox pool with a ready target and a max-session cap
2. warm the pool until a ready workspace exists
3. acquire a session and stage the command plus image/context artifacts
4. run a bounded iteration on that session workspace
5. reacquire the same session and run a second iteration that observes prior
   workspace state

## Pass Criteria

The sandbox throughput layer is green only if all of the following are true:

- ready versus acquired pool state is inspectable
- staged input receipts are typed and tied to one acquisition
- repeated loop execution emits one bounded execution receipt plus one
  iteration-level digest
- workspace reuse is explicit rather than hidden in ad hoc temp-dir behavior
- the pool can satisfy both pre-warmed and on-demand acquisition paths

## Expected Signals

The current harness should prove:

- one session can be warmed ahead of time and then reacquired later
- acquisition receipts expose ready counts and reuse count
- staged image and context artifacts remain machine-legible
- repeated iterations over the same workspace can carry forward state such as
  a counter file

## Current Limitations

This issue intentionally does not claim:

- remote distributed sandbox pools
- persistent pool state across process restart
- environment-aware orchestration on top of the pool layer
- sandbox-level curriculum or train-stage scheduling policy
