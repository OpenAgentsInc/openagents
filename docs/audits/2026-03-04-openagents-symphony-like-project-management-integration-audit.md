# OpenAgents + Symphony-Like Project Management Integration Audit

Date: 2026-03-04  
Author: Codex  
Status: Exploratory architecture audit

## Objective

Explore how OpenAgents could integrate a Symphony-like project management orchestration system, rebuilt natively in Rust (not Elixir), while staying aligned with current OpenAgents MVP constraints.

## Scope and Sources Reviewed

### Symphony sources (`~/code/symphony`)

- `README.md`
- `SPEC.md`
- `elixir/README.md`
- `elixir/WORKFLOW.md`
- `elixir/lib/symphony_elixir/*` (orchestrator, agent runner, workflow/config, workspace, tracker, codex app-server client, HTTP/status surfaces)
- `elixir/test/symphony_elixir/*` (behavioral contracts)

### OpenAgents sources (`~/code/openagents`)

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `crates/codex-client/src/*`
- `crates/codex-client/tests/skills_and_user_input.rs`
- `docs/CODEX_INTEGRATION_*`
- `docs/codex/LIVE_HARNESS.md`

## Naming Clarification

OpenAgents already has docs for "Maestro Symphony" (Bitcoin indexer). This audit is about the OpenAI Symphony-style issue-orchestration system from `~/code/symphony`.

## Executive Summary

OpenAgents can integrate a Symphony-like system with relatively low protocol risk because key primitives already exist:

- Strong Codex app-server client (`crates/codex-client`)
- Desktop-side Codex lane lifecycle and command routing (`apps/autopilot-desktop/src/codex_lane.rs`)
- Existing runtime lane pattern with explicit command/result semantics (`runtime_lanes.rs`)

Main gaps are not Codex protocol. Main gaps are orchestration domain pieces:

- Tracker adapter abstraction and Linear implementation
- Issue claim/dispatch/retry/reconciliation state machine
- Workspace manager with strict path safety and hook contracts
- Workflow contract loader (`WORKFLOW.md`-style prompt + config)
- Operator observability surface for multi-issue unattended runs

Recommended architecture: build a Rust-native orchestration core as reusable crates, then expose it through `autopilot-desktop` as a new product lane/pane. Do not port Elixir code directly; port the behavioral contracts from `SPEC.md` + tests.

## What Symphony Actually Provides (Portable Contracts)

From `SPEC.md` + Elixir reference implementation:

- Single-authority orchestrator state machine (in-memory, deterministic transitions).
- Polling dispatch with bounded concurrency.
- Per-issue isolated workspaces with hook lifecycle.
- Retry/backoff and active-run reconciliation.
- Strict prompt/template contract from repo-owned `WORKFLOW.md`.
- Codex app-server session/turn protocol integration.
- Optional tracker tooling (`linear_graphql`) in-session.
- Observability via structured logs + status surface + optional HTTP endpoints.

Critical implementation detail: Symphony is primarily a scheduler/runner. Ticket writes (state moves/comments/PR actions) are generally delegated to the agent workflow/tooling, not hard-coded business logic in the orchestrator.

## OpenAgents Fit Assessment

### Reusable assets already in OpenAgents

1. `crates/codex-client`
- Already handles broad app-server method surface (requests, notifications, server requests).
- Has protocol conformance tests around skills/user-input envelopes.
- Good base for an unattended orchestrator runner.

2. `autopilot-desktop` Codex lane
- Already models lane lifecycle (`Starting/Ready/Error/Disconnected/Stopped`) and command/response plumbing.
- Already handles approval/user-input/tool-call response paths.
- Provides practical patterns for worker loop + snapshot updates.

3. Runtime lane pattern (`runtime_lanes.rs`)
- Explicit command kinds, statuses, and error classes.
- Deterministic event/result handling style compatible with replay-safe design goals in MVP docs.

### Gaps to fill

1. No tracker orchestration domain
- No issue model/claim/retry/reconcile module comparable to Symphony's orchestrator.

2. No project-workspace manager
- No dedicated per-ticket workspace lifecycle module with root containment + hook contract.

3. No `WORKFLOW.md`-style contract loader
- Need strict parsing + runtime reload semantics for unattended project execution policy.

4. No project-level status API/surface
- Current observability is Codex and mission-centric, not issue-orchestrator centric.

## MVP Alignment (Important)

From `docs/MVP.md`, the short-term product north star is paid-job -> wallet-tick-up -> withdrawal. A project-management orchestrator is adjacent leverage, not the core earn-loop blocker.

Implication:

- Integration should be feature-gated and incremental.
- It should not destabilize wallet-truth and deterministic continuity guarantees.
- It should reuse existing lanes instead of introducing cross-cutting state ambiguity.

## Proposed Rust Architecture

### Ownership-compliant layering

1. New crates (domain and reusable)
- `crates/project-orchestrator-core`
  - State machine, dispatch policy, retries, reconciliation contracts.
- `crates/project-workflow`
  - `WORKFLOW.md` parser, typed config, reload mechanics.
- `crates/project-workspace`
  - Workspace root validation, identifier sanitization, lifecycle hooks.
- `crates/project-tracker`
  - Tracker trait + normalized issue model.
- `crates/project-tracker-linear`
  - Linear GraphQL adapter implementation.

2. App ownership (`apps/autopilot-desktop`)
- Orchestrator runtime wiring (threads/tasks/timers).
- UI panes/actions for operator control and status.
- Integration with existing Codex lane and mission control UX.

This stays aligned with `docs/OWNERSHIP.md`:

- App behavior in app crate.
- Reusable primitives in crates.
- No app logic leaking into `wgpui*` crates.

### Integration strategy with existing Codex lane

Prefer one of two patterns:

- `Pattern A` (recommended initially): orchestrator uses `crates/codex-client` directly in its own worker runtime, and desktop subscribes to orchestrator snapshots/events.
- `Pattern B`: orchestrator emits commands into existing desktop `codex_lane` command surface.

Recommendation: start with Pattern A for cleaner separation and less UI coupling, then bridge summarized events into desktop state.

### Determinism/replay safety requirements

For MVP compatibility, keep an append-only event log for orchestrator state transitions:

- `IssueClaimed`
- `IssueDispatched`
- `WorkerStarted`
- `WorkerExited`
- `RetryScheduled`
- `RetryFired`
- `IssueReleased`
- `WorkspaceCreated/Removed`

Reducers over this log should reconstruct snapshot state deterministically. This gives replay safety similar to current OpenAgents state continuity requirements.

## Phased Delivery Plan

### Phase 0: Spec-conformance skeleton (no UI)

- Implement crates for workflow parsing, issue model, and orchestrator state machine.
- Build unit tests from Symphony Section 17 core contracts (especially candidate selection, retries, reconciliation).

### Phase 1: Linear + workspace + Codex runner integration

- Implement Linear adapter and workspace manager.
- Add Codex turn runner based on existing `codex-client`.
- Verify full issue attempt loop in CLI harness mode.

### Phase 2: Desktop integration (feature-gated)

- Add "Project Ops" pane in `autopilot-desktop`.
- Render running/retrying queues, attempt metadata, and recent event summaries.
- Add manual controls: pause/resume poll loop, force refresh, stop issue run.

### Phase 3: Policy + skill/tool integration

- Add optional `linear_graphql` dynamic tool bridge through existing tool routing.
- Support workflow template variables (`issue`, `attempt`) with strict rendering.

### Phase 4: Hardening

- Add restart recovery strategy tests.
- Add bounded resource controls (max parallel workers, workspace quotas, hook timeout enforcement).
- Add explicit safety modes for unattended operation.

## Key Risks and Mitigations

1. Risk: product-scope drift away from MVP earn loop.
- Mitigation: keep feature flag default-off; isolate from core wallet/earn state reducers.

2. Risk: unattended unsafe actions in high-trust mode.
- Mitigation: explicit policy profiles (`safe`, `balanced`, `high-trust`) with auditable defaults.

3. Risk: state divergence between orchestrator and desktop UI.
- Mitigation: desktop consumes orchestrator snapshots/events, not ad hoc local recomputation.

4. Risk: tracker/API schema drift.
- Mitigation: keep Linear adapter isolated + contract tests against recorded payloads.

5. Risk: workspace escape or destructive hooks.
- Mitigation: strict root containment checks, sanitized identifiers, hook timeouts, output truncation.

## Validation Plan for OpenAgents

### Core tests to add

- Workflow parsing/reload tests.
- Candidate selection and sort-order tests.
- Retry/backoff + claim-release tests.
- Stall detection and reconciliation tests.
- Workspace safety invariant tests.
- Codex runner protocol/timeout tests.

### Existing OpenAgents gates to keep green

- `scripts/lint/workspace-dependency-drift-check.sh`
- `scripts/lint/ownership-boundary-check.sh`
- `scripts/lint/touched-clippy-gate.sh`
- `scripts/skills/validate_registry.sh`
- `cargo test -p autopilot-desktop codex_lane`
- `cargo test -p autopilot-desktop assemble_chat_turn_input`
- `cargo test -p codex-client --test skills_and_user_input`

## Recommendation

Proceed with a Rust-native Symphony-like orchestrator as an optional OpenAgents subsystem, but keep it strictly modular and feature-gated until the MVP earn loop remains stable. Reuse OpenAgents' existing Codex client/lane strengths and port Symphony by behavior contract, not by code translation.
