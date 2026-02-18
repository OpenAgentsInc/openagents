# OpenAgents Repository Overview

This document maps the active codebase after the Rust deprecation cleanup.

## Product Surface

- `apps/openagents.com/`
  **Core web app.** Laravel 12 + Inertia + React (TypeScript), Laravel AI SDK–backed chat and tools. See `docs/plans/active/laravel-rebuild.md`.

- `apps/mobile/`
  Mobile app surface.

- `apps/desktop/`
  Desktop Electron surface for local execution boundaries (including Lightning executor workflows).

## Shared Packages

- `packages/effuse/`
  Effect-oriented runtime and orchestration utilities used across app surfaces.

- `packages/effuse-panes/`
  Pane/window primitives and controllers.

- `packages/effuse-test/`
  E2E and runtime test tooling.

- `packages/effuse-ui/` and `packages/effuse-flow/`
  UI and flow helpers used by product surfaces.

- `packages/dse/` and `packages/hud/`
  DSE visualization and HUD components.

- `packages/lightning-effect/`
  Effect-first Lightning and L402 contracts, services, adapters, and layers shared across app surfaces.

## Docs and Operational Runbooks

- `docs/README.md`
  Documentation index and entry points.

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

Rust code and Rust-era docs were removed from this repo and archived to backroom:

- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/`
- `~/code/backroom/openagents-docs-rust-archive-2026-02-11/docs/`
- `~/code/backroom/openagents-docs-social-archive-2026-02-11/docs/`

If a legacy document references `crates/*`, `Cargo.toml`, `apps/api/`, `apps/autopilot-desktop/`, `apps/web/`, or `apps/autopilot-worker/`, treat it as historical unless it has been explicitly rewritten for the current stack. The former `apps/web` and `apps/autopilot-worker` have been removed; the web app is `apps/openagents.com`.
