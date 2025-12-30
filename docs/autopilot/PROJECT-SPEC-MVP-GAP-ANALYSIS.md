# Autopilot MVP Gap Analysis (Phases 1-3)

This analysis reconciles docs/autopilot/PROJECT-SPEC.md with the current code in the four autopilot crates. It focuses on what is still missing for Phase 1-3 to ship together.

## Scope and Sources

- docs/autopilot/PROJECT-SPEC.md
- crates/autopilot/src/startup.rs
- crates/autopilot/src/claude.rs
- crates/autopilot/src/preflight.rs
- crates/autopilot/src/streaming.rs
- crates/autopilot/src/logger.rs
- crates/autopilot/src/replay.rs
- crates/autopilot/src/report.rs
- crates/autopilot/src/verification.rs
- crates/autopilot/src/workflow.rs
- crates/autopilot/src/checkpoint.rs
- crates/autopilot-service/src/runtime.rs
- crates/autopilot-service/src/cli.rs
- crates/autopilot-service/src/daemon.rs
- crates/autopilot-shell/src/shell.rs
- crates/autopilot-shell/src/panels/claude_usage.rs
- crates/autopilot-shell/src/panels/sessions.rs
- crates/autopilot-wasm/src/lib.rs

## Current Capability Snapshot (Code Reality)

- Single-session plan -> execute -> review -> verify -> fix loop with Claude, driven by StartupState and Claude SDK streaming (crates/autopilot/src/startup.rs, crates/autopilot/src/claude.rs).
- Preflight detection for auth, project info, tools, local compute, and pylon status (crates/autopilot/src/preflight.rs, crates/autopilot/src/pylon_integration.rs).
- JSONL session logging to ~/.openagents/sessions and replay bundle creation/redaction (crates/autopilot/src/logger.rs, crates/autopilot/src/replay.rs, crates/autopilot-wasm/src/lib.rs).
- After-action report and verification checklist (crates/autopilot/src/report.rs, crates/autopilot/src/verification.rs).
- UI shell with session list (Claude sessions), usage panel, and full-auto toggle that drives the local runtime tick (crates/autopilot-shell/src/shell.rs).
- Daemon status client only (no daemon server in autopilot crates) (crates/autopilot-service/src/daemon.rs).
- GitHub workflow helpers exist but are not wired into execution (crates/autopilot/src/workflow.rs).

## Phase 1 Gaps (MVP - Subscription Product)

### 1.1 Full-Auto Mode (Issue Loop)
- Implement a real issue-processing loop (ready -> claim -> implement -> test -> commit -> PR -> complete). Current runtime is not connected to issues at all; it only reads a summary via sqlite (crates/autopilot/src/streaming.rs).
- Use issues crate APIs for claim/complete/create (no raw sqlite writes). The only issue interaction today is read-only summary output.
- Add automatic issue discovery when queue is empty (creation + immediate claim).
- Loop continuation until budget exhaustion; current loop ends on review/verification outcomes, not on issue queues.
- Wire GitHubWorkflow into the runtime so commits/branches/PRs are real and attached to issues (crates/autopilot/src/workflow.rs).
- Expose full-auto control outside the UI toggle (CLI/daemon config and headless execution).

### 1.2 Budget and Turn Limits
- Pass max_budget into Claude SDK QueryOptions and enforce it across the loop (claude calls only set max_turns today).
- Unify budget/turn config across CLI, UI, and daemon configuration.
- Surface budget usage in the UI and include a hard stop behavior with a report.

### 1.3 Daemon Supervisor (autopilotd or openagents daemon)
- Implement the daemon server (autopilot-service currently only has a client) with crash recovery, stall detection, and memory monitoring.
- Provide health check endpoint and structured status for monitoring.
- Implement multi-worker support (N concurrent runs) and queue scheduling.
- Ensure known-good binary support or equivalent to guarantee restart after failures.

### 1.4 Context Management
- Add compaction strategies and context loss detection (no compaction hook in autopilot crates).
- Persist critical context for resuming multi-hour sessions.
- Add task-type adaptive compaction and a critical context preservation policy.

### 1.5 Plan Mode
- Add plan validation before execution; currently execution starts immediately after plan writing.
- Implement subagent launching and result aggregation in code, not just in prompt instructions.
- Add a structured plan file format for validation and UI visualization.

### 1.6 Dashboard (WGPUI)
- Real-time APM display and issue queue visualization.
- Trajectory viewer integration (use crates/autopilot/src/replay.rs or autopilot-wasm for render pipeline).
- Session browser with search/filter based on ~/.openagents/sessions and checkpoints (current UI reads ~/.claude sessions).
- Daemon health and worker controls in the UI.
- Enforce Vera Mono font only and sharp-corner styling across UI components.

### 1.7 Data Path Consistency
- Normalize file locations across components:
  - Issue DB path is inconsistent (.openagents/autopilot.db vs repo-root autopilot.db).
  - Logs are written to ~/.openagents/sessions but UI references Claude sessions.
  - Reports are written to ~/.openagents/reports while docs point to docs/logs.

## Phase 2 Gaps (Observability and Self-Improvement)

### 2.1 Metrics Collection
- Implement a metrics database (session + tool-call level) and populate it from runtime events.
- Add real-time metrics streaming during sessions for UI.
- Provide import/backfill tooling from JSONL logs.

### 2.2 Analysis Pipeline
- Baselines (p50/p90/p99) and anomaly detection are not present.
- Regression detection between periods and automated report generation are missing.
- Automated issue creation from metrics patterns needs to be implemented and wired to issues crate.

### 2.3 Velocity Tracking
- No velocity snapshots or trend outputs are in autopilot crates.
- UI visualization for trends is missing.

### 2.4 Learning System
- No automated prompt refinement or compaction instruction tuning pipeline.
- Canary deployment and change evaluation workflows are missing.
- LEARNINGS.md update automation is missing.

## Phase 3 Gaps (Multi-Agent Orchestration)

- No dependency on agent-orchestrator or equivalent in autopilot crates.
- Missing agent registry (capabilities, health), hook system, and multi-backend routing.
- No scope-based coordination or conflict detection for overlapping work.
- No background task manager for parallel subagents and result aggregation.
- UI does not show per-agent status, budgets, or task graphs.

## Cross-Phase Go-Live Checklist

- Unify configuration and data paths across CLI, daemon, UI, and docs.
- Add hard safety rails: permission modes, budget caps, and guardrails for git/FS operations.
- End-to-end tests for the full-auto loop, daemon restart, metrics pipeline, and UI views.
- Update docs to reflect the actual runtime and storage paths once gaps are closed.
- Provide empty-state UX (no placeholder data) for UI panels that depend on metrics or issue DB.

## Step-by-Step Implementation Plan (Phases 1-3)

1. Define canonical storage paths for issues DB, logs, reports, checkpoints, and plans; update preflight/streaming/UI to use the same paths and add compatibility reads for older locations.
2. Build a full-auto issue loop in the runtime: issue_ready -> claim -> implement -> test -> commit -> PR -> complete, using issues crate APIs for all writes.
3. Add issue discovery when the queue is empty (create -> claim -> execute) and persist discovery decisions in the session log.
4. Wire GitHubWorkflow into execution so branch creation, PR creation, and receipts are real and tied to issues.
5. Propagate max_budget/max_turns from CLI/daemon/UI into Claude SDK QueryOptions and enforce a hard stop with a final report when limits are hit.
6. Implement the daemon server with worker supervision, stall detection, memory monitoring, health checks, and multi-worker scheduling; connect the existing daemon client to real status endpoints.
7. Add compaction hooks with task-type strategies and critical context preservation; persist compaction summaries for resume.
8. Add plan validation before execution (parse and check required sections) and implement subagent launching with result aggregation.
9. Upgrade the UI to show APM, issue queue, trajectory viewer, session browser/search, and daemon controls; enforce Vera Mono font and sharp corner styling across panels.
10. Implement metrics collection: session + tool-call metrics DB, real-time event streaming to UI, and backfill import from JSONL logs.
11. Implement analysis pipeline: baselines, anomaly detection, regression detection, weekly reports, and automated issue creation wired to the issues crate.
12. Implement velocity tracking outputs and UI visualization.
13. Implement learning automation: prompt refinements, compaction tuning, and canary rollout gates based on metrics.
14. Integrate multi-agent orchestration (agent-orchestrator): registry, hooks, multi-backend routing, scope locking, and background task manager; expose agent status and budgets in the UI.
15. Add end-to-end tests for full-auto runs, daemon restart recovery, metrics pipeline, and UI empty states; update docs to match the shipped runtime behavior.
