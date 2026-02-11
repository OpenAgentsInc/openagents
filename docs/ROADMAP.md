# OpenAgents Roadmap (Web-First)

This roadmap tracks active priorities for the current TypeScript/Effect codebase.

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
- Consolidate shared Effect services used by web, worker, and test harnesses.
- Remove legacy assumptions left from deprecated runtime surfaces.

## Phase 5: Documentation Quality Gates

- Keep docs aligned with active code paths only.
- Archive deprecated docs to backroom with explicit archive indexes.
- Require updates to `docs/README.md` and `docs/PROJECT_OVERVIEW.md` when major architecture changes ship.

## Notes

- Rust code and Rust-era docs are deprecated and archived:
  - `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/`
  - `~/code/backroom/openagents-docs-rust-archive-2026-02-11/docs/`
