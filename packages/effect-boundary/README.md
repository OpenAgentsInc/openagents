# Effect Boundary

`@openagentsinc/effect-boundary` centralizes schema-backed parsing at external
boundaries. Use these helpers instead of `JSON.parse(...) as T`, silent
`undefined` fallbacks, or broad thrown exceptions when decoding request bodies,
local JSON state files, D1/SQLite rows, config, and test fixtures.

Approved pattern:

```ts
const payload = yield* readRequestJsonEffect(MySchema, request, "my.route.body")
```

Failures preserve the operation name and return public-safe reason refs. Raw
payloads, secrets, local paths, and command output are never copied into the
error value.
