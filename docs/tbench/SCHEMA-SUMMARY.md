# TerminalBench & Trajectory Data Summary

This brief points to the authoritative schema definitions and persistence contracts you asked about for the UI work.

## ATIF Trajectories (source of truth)
- `src/atif/schema.ts` defines the full ATIF v1.4 schema (`Trajectory`, `Step`, `Agent`, `Metrics`, `FinalMetrics`, `Checkpoint`, `RecoveryInfo`, etc.) plus encode/decode helpers and guards needed when surfacing live data.
- `src/atif/validation.ts` covers the validation rules that keep every saved trajectory compliant with the schema and can be reused when reading user-provided files.

## TerminalBench Data
- `src/bench/terminal-bench.ts` is the single source for TerminalBench types: task/suite/result structures are expressed with `effect/Schema` and it also exports adapters that convert between TB tasks/results and OpenAgents tasks.
- `src/bench/reporter.ts` builds comparison reports derived from `TerminalBenchResults` (so the summary tokens/outcomes you show in the UI should mirror `MetricDelta`, `TaskComparison`, and `ComparisonReport` there).

## HUD & Persistence Touchpoints
- `src/hud/protocol.ts` exposes every HUD message type you will serialize over the websocket, including the TerminalBench run/task lifecycle events and ATIF trajectory events; use those interfaces for TypeScript-safe UI wiring.
- `src/tbench-hud/persistence.ts` is the run-store definition for the HUD: `TBRunFile` (with metadata, tasks, optional ATIF `Trajectory`) plus helper functions for listing/saving runs under `.openagents/tb-runs/`.

Linking these files together gives you the data contracts you need when visualizing TerminalBench runs or replaying ATIF trajectories in the UI.
