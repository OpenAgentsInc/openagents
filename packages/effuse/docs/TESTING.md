# Effuse Testing

Effuse aims to keep docs aligned with behavior through contract tests.

## Running Tests

```
npm run test
```

Tests run with `vitest` using the `happy-dom` environment.

## Current Contract Tests

Tests live under `tests/effuse/` and cover:

- **DomServiceLive** swap behavior and focus restoration.
- **Ez runtime** parsing, params collection, and concurrency semantics.
- **Component mount** render loop, event handling, and subscriptions.

Tests use `@effect/vitest` helpers (`it.effect`, `it.live`) to run Effects safely.

## Adding Tests

- Prefer small, isolated tests that assert observable DOM behavior.
- Use `Effect.runPromise` with `Effect.scoped` to ensure cleanup.
- Keep tests aligned with `docs/effuse/SPEC.md`.
