# OpenAgents product surface Effect Test Fixtures

OpenAgents product surface uses plain Vitest plus `Effect.runPromise` for service tests while
`@effect/vitest` remains incompatible with the repo's Effect 4 beta line.

Reusable fixtures:

- `workers/api/src/test/service-fixtures.ts`
  - `makeOpenAgentsWorkerConfigTestLayer`
  - `makeOpenAgentsDatabaseTestLayer`
  - `makeProviderAccountRepositoryTestLayer`
  - `makeOpenAiCodexProviderClientTestLayer`
  - `makeProviderAccountLifecycleTestLayer`
  - `makeOmniDispatchServiceTestLayer`
  - `makeRunnerEventsQueueTestLayer`
- `packages/sync-worker/src/test-fixtures.ts`
  - `makeMemorySyncD1`
  - `syncWorkerTestRuntime`
  - `makeSyncOutboxStoreFixture`
  - `makeMemorySyncOutboxStore`

Test shape:

```ts
const result = await Effect.runPromise(
  Effect.gen(function* () {
    const service = yield* SomeService

    return yield* service.someOperation()
  }).pipe(Effect.provide(testLayer)),
)
```

Do not import `@effect/vitest` until its peer dependencies support the exact
Effect major and beta line used by OpenAgents product surface. When that happens, migrate these
fixtures by changing only the outer test harness from `test(...)` plus
`Effect.runPromise(...)` to `it.effect(...)` or `it.layer(...)`. Keep the
fixture layer constructors themselves as plain `Layer` values so production
service wiring and tests continue to share the same dependency graph shape.
