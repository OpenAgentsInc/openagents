# OpenAgents Repository Overview

This document maps the active codebase.

## Product Surfaces

- `apps/openagents.com/`
  **Core web app and control plane.** Laravel 12 + Inertia + React (TypeScript), Laravel AI SDK–backed chat and tools. See `docs/plans/active/laravel-rebuild.md`.

- `apps/runtime/`
  **Elixir runtime execution plane.** Long-running run lifecycle, stream-from-log serving, tool orchestration, spend/policy enforcement, DS-Elixir execution, and replay safety.

- `apps/autopilot-ios/`
  Native iOS app surface for Codex/runtime administration.

- `apps/autopilot-desktop/`
  Rust desktop app for local Codex execution and native Autopilot UI/runtime loops.

- `apps/onyx/`
  Rust local-first notes app (WGPUI + local vault storage).

## Control-Plane and Runtime Boundary

- `apps/openagents.com` owns public APIs, user auth/session, UI, and operator-facing settings.
- `apps/runtime` owns internal runtime correctness concerns: execution leases, durable run events, streaming, policy decisions, spend reservation state, and DS strategy execution.
- Contributor rule: runtime correctness logic belongs in `apps/runtime`, not Laravel controllers/models.
- Contract review ownership for runtime/proto surfaces is enforced in `.github/CODEOWNERS`.

## Codex Desktop/Runtime Boundary

- Desktop execution is currently centered in `apps/autopilot-desktop/` and local bridge code in `crates/pylon/`.
- Runtime exposes internal Codex worker lifecycle APIs in `apps/runtime` (`/internal/v1/codex/workers*`).
- Laravel proxies user-scoped Codex worker APIs in `apps/openagents.com` (`/api/runtime/codex/workers*`).
- Khala (sync engine codename) is the target runtime-owned WS sync subsystem (`docs/sync/thoughts.md`, `docs/sync/ROADMAP.md`, `docs/sync/SURFACES.md`).
- Khala v1 ships inside `apps/runtime` and shares Postgres for transactional projection+delivery semantics.
- Khala (if enabled) is projection/sync-only for reactive Codex read models; runtime remains source of truth and single writer for those projections.
- Canonical Codex architecture plan: `docs/codex/unified-runtime-desktop-plan.md`.
- Khala self-hosting/sync plan: `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`.

## Shared Packages

- `packages/`
  Legacy TypeScript package lanes are retired/archived from active runtime surfaces (`OA-RUST-103`).

- `crates/`
  Active shared Rust workspace for runtime, protocol, UI, and service contracts.

## Rust Workspace

- `crates/`
  Active Rust workspace with shared runtime, Codex client, UI, and integration crates used by desktop and supporting services.

## Docs and Operational Runbooks

- `docs/README.md`
  Documentation index and entry points.

- `apps/runtime/docs/RUNTIME_CONTRACT.md`
  Internal runtime contract (`/internal/v1/*`).

- `apps/runtime/docs/OPERATIONS.md`
  Runtime operations and incident runbook.

- `apps/runtime/docs/DS_ELIXIR_RUNTIME_CONTRACT.md`
  DS-Elixir contract surface and invariants.

- `apps/runtime/docs/DS_ELIXIR_OPERATIONS.md`
  DS-Elixir operational workflows and replay procedures.

- `docs/autopilot/`
  Primary operational docs for the web product (production E2E, stream testing, trace retrieval, debugging).

- `docs/lightning/`
  Lightning agent tools (L402, lnget, Aperture) integration plan and references.

- `docs/execution/`, `docs/protocol/`, `docs/dse/`
  Canonical contracts referenced by ADRs (artifacts/replay, protocol fields, compiler contracts).

- `docs/plans/`
  Repo-wide plan hub (active/completed execution plans).

- `apps/*/README.md` and `docs/STORYBOOK.md`
  Local development entry points and component-level testing/docs.

## Historical Code and Docs

Legacy references to removed surfaces should be treated as historical:

- `apps/web/`
- `apps/autopilot-worker/`
- `apps/mobile/`
- `apps/desktop/`

The active web control plane is `apps/openagents.com/`, and runtime execution is `apps/runtime/`.
