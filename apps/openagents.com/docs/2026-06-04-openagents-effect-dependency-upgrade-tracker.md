# OpenAgents product surface Effect Dependency Upgrade Tracker

Issue: GitHub #43

This tracker records the dependency topology that must change before OpenAgents product surface can
delete the temporary Foldkit Effect beta 66 exception or move service tests from
plain Vitest to `@effect/vitest`.

## Installed State

Checked on 2026-06-04 from `main` after commit `f9783a1f`.

| Package                    | Installed state                                               | Source                                                    |
| -------------------------- | ------------------------------------------------------------- | --------------------------------------------------------- |
| `effect`                   | `4.0.0-beta.70` direct, `4.0.0-beta.66` through Foldkit peers | workspace package manifests and `bun pm why effect`       |
| `effect-cf`                | `0.13.1`                                                      | `workers/api/package.json` and `bun.lock`                 |
| `foldkit`                  | `0.102.1`                                                     | `apps/web/package.json` range plus `bun.lock` resolution  |
| `@foldkit/devtools-mcp`    | `0.9.0`                                                       | `apps/web/package.json` range plus `bun.lock` resolution  |
| `@foldkit/vite-plugin`     | `0.7.0`                                                       | `apps/web/package.json` range plus `bun.lock` resolution  |
| `@effect/platform-browser` | `4.0.0-beta.70` direct, `4.0.0-beta.66` through Foldkit       | `apps/web/package.json`, `bun.lock`, and topology checker |
| `@effect/vitest`           | not installed                                                 | workspace manifests and topology checker                  |

Executable local check:

```sh
bun run check:effect-topology
```

That check allows exactly two Effect runtime lines:

- OpenAgents product surface/effect-cf line: `effect@4.0.0-beta.70`
- Temporary Foldkit exception: `effect@4.0.0-beta.66`

Any third Effect runtime line must fail the topology check.

## Live NPM Metadata Recheck

Before any dependency upgrade attempt, run:

```sh
bun run check:effect-upgrade-metadata
```

The script performs the required `npm view` checks for:

- `effect`
- `effect-cf`
- `foldkit`
- `@foldkit/devtools-mcp`
- `@foldkit/vite-plugin`
- `@effect/platform-browser`
- `@effect/vitest`

Metadata checked on 2026-06-04:

| Package                    | Published version checked             | Relevant peer state                                             | Decision                                      |
| -------------------------- | ------------------------------------- | --------------------------------------------------------------- | --------------------------------------------- |
| `effect`                   | `latest=3.21.2`, `beta=4.0.0-beta.78` | OpenAgents product surface remains on `4.0.0-beta.70` for current `effect-cf` slice  | do not bump alone                             |
| `effect-cf`                | `0.13.1`                              | peers on `effect ^4.0.0-beta.70` and SQL beta 70 packages       | still compatible with OpenAgents product surface beta 70           |
| `foldkit`                  | `0.104.0`                             | peers exactly on `effect 4.0.0-beta.66` and platform beta 66    | blocked                                       |
| `@foldkit/devtools-mcp`    | `0.9.0`                               | peers exactly on `effect 4.0.0-beta.66`                         | blocked                                       |
| `@foldkit/vite-plugin`     | `0.7.0`                               | peers exactly on `effect 4.0.0-beta.66`                         | blocked                                       |
| `@effect/platform-browser` | `beta=4.0.0-beta.78`                  | beta peers on `effect ^4.0.0-beta.78`                           | only viable as part of a full beta-line move  |
| `@effect/vitest`           | `latest=0.29.0`, `beta=4.0.0-beta.78` | latest peers on Effect 3; beta peers on `effect ^4.0.0-beta.78` | defer until repo line moves or beta 70 exists |

Current result: do not upgrade Foldkit, Foldkit devtools, Foldkit Vite plugin,
`@effect/platform-browser`, or `@effect/vitest` in this issue. The latest
published peer metadata still cannot produce one clean Effect line for OpenAgents product surface.

## Upgrade Gate

Only attempt the dependency alignment when all of these are true in a fresh
metadata check:

1. `foldkit@latest` peers on the same Effect beta line used by OpenAgents product surface.
2. `foldkit@latest` peers on the matching `@effect/platform-browser` beta line.
3. `@foldkit/devtools-mcp@latest` peers on that same Effect beta line.
4. `@foldkit/vite-plugin@latest` peers on that same Effect beta line.
5. `effect-cf@latest` and the `@effect/sql-*` packages are compatible with that
   Effect beta line.
6. `@effect/vitest@latest` or `@effect/vitest@beta` peers on that same Effect
   beta line and the repo's Vitest major.

## Upgrade Procedure

When the gate opens, perform the alignment in one change:

1. Re-run `bun run check:effect-upgrade-metadata` and paste the relevant npm
   metadata into the issue or commit notes.
2. Upgrade `effect`, `@effect/platform-browser`, `@effect/sql-d1`,
   `@effect/sql-sqlite-do`, `effect-cf`, `foldkit`,
   `@foldkit/devtools-mcp`, and `@foldkit/vite-plugin` together.
3. Add `@effect/vitest` only if its peer range matches the same Effect line.
4. Regenerate `bun.lock`.
5. Remove `FOLDKIT_EFFECT_EXCEPTION_VERSION`,
   `requiredFoldkitExceptionPullers`, and the Foldkit beta 66 lock
   expectations from `scripts/check-effect-topology.mjs`.
6. If `@effect/vitest` is installed, migrate the plain Vitest service tests by
   changing only the outer test harness. Keep reusable fixture constructors as
   plain `Layer` values.
7. Run full verification:

```sh
bun run typecheck
bun run test
bun run check:effect-topology
bun scripts/check-zero-debt-architecture.mjs
bun run check:deploy
bun run --cwd apps/web build
bun run --cwd workers/api build
```

Representative Foldkit tests that must pass after alignment:

```sh
bun run --cwd apps/web test -- src/update.test.ts src/main.test.ts src/page/loggedOut/page/login.scene.test.ts src/page/loggedIn/view.scene.test.ts
```

## Current Follow-Up

Keep #44 scoped to deleting compatibility paths and tightening zero-allowed
architecture budgets that are not blocked by the upstream package peer
metadata recorded here.
