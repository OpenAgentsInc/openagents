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
- `grade.ts` — AR-1 obligation grading over the inventory (mapped / designed /
  observed / accepted / inconclusive / out-of-scope, kept as independent
  facts and never emits observed/accepted) plus the program-area coverage report.
- `false-green.ts` — AR-2 heuristic classifier over test sources (coverage
  theater / round-up / mocked seam) with a string-aware test-block scanner.
  Emits candidate leads, never findings.
- `mutation-runner.ts` — AR-2 self-contained mutation runner that demonstrates
  a false green by reproduction (kill vs surviving weak oracle), restoring the
  subject in a `finally`. Shares the `openagents.mutation.v1` semantics.
- `audit.ts` — AR-2 report builder (`buildFalseGreenReport`) over every tracked
  test source.
- `drift.ts` — AR-4 drift oracles over the repository's own governing
  documents: side-effect-free path and command checks with typed
  ok/broken/unverifiable verdicts and policy-driven dispositions.
- `inventory.ts` — the generator (`buildInventory`, which also grades) and the
  surface policy loader.
- `cli.ts` — `generate` / `check` / `summary` / `coverage` / `audit*` /
  `demonstrate`.

## Usage

```sh
pnpm run generate:assure-repo   # from repo root
pnpm run check:assure-repo
pnpm run test:assure-repo
```
