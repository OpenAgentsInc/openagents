# OpenAgents Roadmap (Current Stack)

This roadmap tracks active priorities across web, runtime, and desktop surfaces.

**Web:** The core web app is **Laravel 12 + Inertia + React** at `apps/openagents.com/`. See `docs/plans/active/laravel-rebuild.md` for the plan.

## Roadmap Navigation

- Active execution plans: `docs/plans/active/`
- Completed plans/history: `docs/plans/completed/`
- Plan conventions/template: `docs/plans/README.md`, `docs/plans/TEMPLATE.md`
- Repo map and ownership: `docs/PROJECT_OVERVIEW.md`
- Codex architecture plan: `docs/codex/unified-runtime-desktop-plan.md`
- Architecture constraints: `docs/adr/INDEX.md`
- Canonical terms: `docs/GLOSSARY.md`
- Agent contract (non-negotiables): `AGENTS.md`

## Phase 1: Chat Reliability and Observability

- Stabilize chat streaming behavior across all panes.
- Ensure every message has complete metadata (model, source, inference timing).
- Keep trace retrieval deterministic and thread-addressable.
- Expand telemetry coverage for message send/receive, errors, retries, and tool events.

## Phase 2: Pane UX and State Consistency

- Persist pane layout, size, and last known placement.
- Reduce pane re-open latency with durable local cache hydration.
- Standardize pane controls and icon behavior across chat, metadata, and trace views.
- Ensure all pane actions (open/close/toggle/debug) are tracked through telemetry.

## Phase 3: Automation and Admin Controls

- Harden admin-trigger flows for deterministic test users.
- Improve end-to-end traceability from admin trigger -> model execution -> stored receipts.
- Keep production-safe testing hooks documented and auditable.

## Phase 4: Cross-Surface Consistency

- Align web and mobile data contracts for chat and trace payloads.
- Consolidate shared services used by web, runtime, desktop, and test harnesses.
- Remove legacy assumptions left from deprecated runtime surfaces.

## Phase 5: Codex Runtime + Desktop Unification

- Ship desktop-first Codex execution with runtime as durable worker/event authority.
- Ensure web admin surfaces and mobile follow-up read/admin flows consume the same runtime-backed Codex worker contract.
- Keep a single canonical plan and contract path (`docs/codex/unified-runtime-desktop-plan.md` + runtime OpenAPI/contract docs).

## Phase 6: Documentation Quality Gates

- Keep docs aligned with active code paths only.
- Archive deprecated docs to backroom with explicit archive indexes.
- Require updates to `docs/README.md` and `docs/PROJECT_OVERVIEW.md` when major architecture changes ship.
