# @openagentsinc/effuse-test

Effect-native E2E and visual regression runner used by `apps/web`.

## Development Setup

Install deps, then patch TypeScript for Effect build-time diagnostics:

```bash
npm install
npm run effect:patch
```

The package tsconfig includes the `@effect/language-service` plugin for editor diagnostics.

## Common Commands

```bash
npm run typecheck
npm test
```

## Error Boundaries

Core modules expose typed errors for async runtime boundaries:

- `BrowserServiceError` (`src/browser/BrowserService.ts`)
- `ProbeServiceError` (`src/effect/ProbeService.ts`)
- `RunnerError` (`src/runner/runner.ts`)
- `ViewerServerError` (`src/viewer/server.ts`)
- `VisualSnapshotError` (`src/runner/visualSnapshot.ts`)

These boundaries use `Effect.tryPromise` and explicit catch mapping instead of
untyped `Effect.promise`.

## Config Service

`effuse-test` now centralizes runtime/env configuration through a typed Effect
service: `EffuseTestConfig` (`src/config/EffuseTestConfig.ts`).

Supported env vars:

- `EFFUSE_TEST_CHROME_PATH`
- `EFFUSE_TEST_UPDATE_SNAPSHOTS` (`1|0`, `true|false`, `yes|no`)
- `EFFUSE_TEST_E2E_BYPASS_SECRET`
- `EFFUSE_TEST_MAGIC_EMAIL`
- `EFFUSE_TEST_MAGIC_CODE`

Validation:

- `EFFUSE_TEST_MAGIC_EMAIL` and `EFFUSE_TEST_MAGIC_CODE` must be set together
  (or both unset).
- Invalid boolean values for `EFFUSE_TEST_UPDATE_SNAPSHOTS` fail startup with
  explicit config errors.

CLI `run` supports first-class config overrides that merge over env config:

- `--chrome-path <path>`
- `--update-snapshots <true|false|1|0|yes|no>`
- `--e2e-bypass-secret <secret>`
- `--magic-email <email>`
- `--magic-code <code>`
