# Migration: Service Tags and Tagged Errors (2026-02)

Effuse core services and runtime errors now follow Effect's class-based service
and error modeling patterns.

## What changed

1. Service tags migrated from `Context.GenericTag(...)` to class-based
   `Context.Tag`:
   - `DomServiceTag`
   - `StateServiceTag`
   - `EzRegistryTag`

2. Runtime errors crossing Effect boundaries are now `Schema.TaggedError`:
   - `DomError`
   - `RouterError`

## Compatibility notes

- Tag names are unchanged (`DomServiceTag`, `StateServiceTag`,
  `EzRegistryTag`), so existing `yield* Tag` and
  `Effect.provideService(Tag, impl)` usage remains valid.
- `DomError` and `RouterError` are now schema-backed tagged errors.
  Prefer:
  - `DomError.make({ message, cause? })`
  - `RouterError.make({ message, cause? })`

If your code previously instantiated errors via positional constructors
(`new DomError(message, cause)` / `new RouterError(message, cause)`), migrate to
`.make(...)`.

## Why

- Stronger typed dependency injection contracts for core services.
- Serializable, typed error channels with stable `_tag` values for pattern
  matching and telemetry.
