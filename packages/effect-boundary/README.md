# @openagentsinc/effect-boundary

Shared Effect boundary helpers for decoding external JSON, request bodies,
local-state files, D1/SQLite rows, and config through Effect Schema.

Approved pattern:

```ts
const body = yield* readRequestJsonEffect(RequestBodySchema, request, "route.create")
const row = yield* decodeRowEffect(RowSchema, rawRow, "store.read")
```

Failures use `OpenAgentsBoundaryError`. The error keeps the operation name and
public-safe reason refs, but it does not include raw JSON text, row contents, or
secret/config values.
