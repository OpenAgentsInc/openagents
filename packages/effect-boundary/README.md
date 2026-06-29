# OpenAgents Effect Boundary

`@openagentsinc/effect-boundary` is the shared path for external data crossing
into OpenAgents Effect code.

Approved pattern:

```ts
const Body = S.Struct({ id: S.String })
const body = yield* readRequestJsonEffect(Body, request, 'worker.route.body')
```

Use these helpers for JSON text, request bodies, local-state files, D1/SQLite
rows, and config values instead of `JSON.parse(...) as T`. Boundary errors keep
the operation name and a public-safe `reasonRef`; secret config values use
Effect `Redacted`.
