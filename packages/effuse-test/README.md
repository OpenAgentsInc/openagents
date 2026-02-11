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
