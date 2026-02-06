# Effuse Testing

Effuse aims to keep docs aligned with behavior through contract tests.

## Running Tests

```
bun run test
# or: npm run test
```

Tests run with `vitest` using the `happy-dom` environment.

## Current Contract Tests

Tests live under `tests/` and currently cover:

- **Ez runtime** parsing, params collection, and concurrency semantics.

Current tests:

- `tests/ez-runtime.test.ts`

DomService swap tests and component mount tests are planned but not implemented yet.

## Adding Tests

- Prefer small, isolated tests that assert observable DOM behavior.
- Use `Effect.runPromise` (and `Effect.scoped` when applicable) to ensure cleanup.
- Keep tests aligned with `docs/effuse/SPEC.md`.
