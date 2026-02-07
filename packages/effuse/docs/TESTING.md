# Effuse Testing

Effuse aims to keep docs aligned with behavior through contract tests.

## Running Tests

From the repo root:

```bash
cd packages/effuse
npm test
```

Or, if you use Bun:

```
bun run test
```

Tests run with `vitest` using the `happy-dom` environment (see `vitest.config.ts`).

## Current Contract Tests

Tests live under `tests/` and currently cover core Effuse contracts + conformance:

- Templates/SSR: `tests/render-to-string.test.ts`, `tests/conformance-ssr-determinism.test.ts`
- DOM swapping: `tests/dom-swap.test.ts`
- Components + state: `tests/component-mount.test.ts`, `tests/state-cell.test.ts`
- EZ runtime: `tests/ez-runtime.test.ts`
- Router + app contracts: `tests/router-service.test.ts`, `tests/router-outcomes.test.ts`, `tests/router-link-interception.test.ts`, `tests/router-head.test.ts`, `tests/run-route.test.ts`, `tests/loader-key.test.ts`, `tests/cache-control.test.ts`
- Conformance: `tests/conformance-hydration.test.ts`, `tests/conformance-hydration-modes.test.ts`, `tests/conformance-shell-outlet.test.ts`, `tests/conformance-router.test.ts`, `tests/conformance-tool-parts.test.ts`

## Adding Tests

- Prefer small, isolated tests that assert observable DOM behavior.
- Use `Effect.runPromise` (and `Effect.scoped` when applicable) to ensure cleanup.
- Prefer `@effect/vitest` (`it.effect`, `it.scoped`) when you want fiber-aware failures.
- Keep tests aligned with `docs/SPEC.md` and `docs/DOM.md`.
