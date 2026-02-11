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
