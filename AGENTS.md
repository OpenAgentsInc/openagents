# OpenAgents Agent Contract

## Scope

This repository is the new OpenAgents Bun and Effect monorepo.

Preserve `docs/transcripts/`. It is the retained transcript archive from the
previous repository shape.

## Repo Layout

- `apps/openagents.com/` owns the `openagents.com` product surface.
- `apps/forum/` owns the forum extraction target for
  `openagents.com/forum`.
- `apps/pylon/` owns the Pylon contributor app imported from the standalone
  Pylon repository.
- `packages/probe/` owns the Probe runtime imported from the standalone Probe
  repository.
- `docs/promises/` owns product-promise records, launch-promise source sets,
  verification gates, copy gates, and user/agent report templates.
- `docs/refactor/` owns migration plans, cutover notes, and architectural
  cleanup records for this repo reset.

## Working Rules

- Read `INVARIANTS.md` before changing authority, routing, payment,
  projection, or public-claim surfaces.
- For work under `apps/openagents.com/`, also read
  `apps/openagents.com/AGENTS.md` and `apps/openagents.com/INVARIANTS.md`.
- Keep new TypeScript implementation work on Bun, Effect, Effect Schema, and
  Foldkit where `apps/openagents.com` already uses it.
- Do not reintroduce the old Cargo or Tauri workspace unless the user asks for
  explicit historical compatibility work.
- Route new user-facing and agent-facing product claim systems through
  `docs/promises/` before broadening copy.
- Do not commit secrets, dependency caches, build output, `target/`, `dist/`,
  `node_modules/`, or local runtime state.
- Keep Git operations scoped to this repository when working here.
