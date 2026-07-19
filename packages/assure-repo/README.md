# @openagentsinc/assure-repo

Programmatic verification of the OpenAgents codebase surface (ASSURE-REPO,
epic [#9055](https://github.com/OpenAgentsInc/openagents/issues/9055)).

This package derives a deterministic inventory of every verification-bearing
surface in the monorepo and binds each to its oracles or an explicit
`unverified` reason. See [`docs/assure-repo/README.md`](../../docs/assure-repo/README.md)
for the program overview and the generated artifact.

## Modules

- `schema.ts` — Effect Schema for the inventory (`SurfaceRow`,
  `SurfaceInventoryDocument`) plus `validateSurfaceInventory`, which enforces
  the no-silent-surface invariant, oracle/unverified mutual exclusion, id
  uniqueness, canonical sort order, and summary agreement.
- `workspace.ts` — deterministic enumeration of the pnpm workspace, Cargo
  crates, tracked files, and release pipelines (no network, no wall clock).
- `oracles.ts` — oracle enumeration and surface binding: behavior-contract
  registries, tracked test files, and Rust in-tree tests.
- `inventory.ts` — the generator (`buildInventory`) and the surface policy
  loader.
- `cli.ts` — `generate` / `check` / `summary`.

## Usage

```sh
pnpm run generate:assure-repo   # from repo root
pnpm run check:assure-repo
pnpm run test:assure-repo
```
