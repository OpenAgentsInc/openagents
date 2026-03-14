# Train Run Graph Reference

> Status: canonical `#3569` train-run-graph record, updated 2026-03-14 after
> landing the runnable harness in
> `scripts/release/check-psionic-train-run-graph.sh`.

This document records the first Psionic-native training run graph.

## What Landed

The issue landed a new `run_graph` module inside `psionic-train` with:

- typed `TrainingRunState` for stable run id, stage id, cluster binding,
  checkpoint family, and environment identity
- explicit participant admission, readiness, contribution, departure, and
  suspension state
- deterministic topology revisions over the wider admitted and heartbeat-visible
  population
- deterministic contributor-set revisions over the bounded active contributor
  population
- replay-safe `TrainingWindow` planning with deterministic batch-slice and
  eval-slice assignment
- machine-legible lifecycle events for join, rejoin, heartbeat, departure,
  contributor suspension, contributor reselection, and window transitions

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-run-graph.sh
```

## Workload Shape

The current reference path proves one bounded but real train-control workload:

1. mirror cluster membership into a training run graph
2. preserve the distinction between admission, readiness, and contribution
3. rank participants and select a bounded contributor set deterministically
4. suspend or depart participants without collapsing those states together
5. plan windows from contributor-set revisions
6. drive windows through the canonical transition state machine

## Pass Criteria

The run-graph layer is green only if all of the following are true:

- run, topology, contributor-set, and window identity are typed and stable
- active contributors are distinct from the wider admitted set
- participant lifecycle is machine-legible instead of buried in scheduler logs
- contributor reselection is deterministic and replay-safe
- window transitions are explicit and validated

## Expected Signals

The current harness should prove:

- admitted, ready, and selected populations diverge cleanly
- contributor suspension does not pretend the participant left the run
- eviction and departure semantics stay distinct from contributor suspension
- multiple windows can exist under different contributor-set revisions
- invalid window transitions are refused

## Current Limitations

This issue intentionally does not claim:

- full orchestrator control flow
- checkpoint pointer/manifest discipline
- trainer-batch propagation or batch-assembler ownership
- policy-driven rollback or cross-window settlement logic
- validator-owned training verdicts

Those remain later issues. This issue makes the canonical run graph,
participant lifecycle, and window state machine real first.
