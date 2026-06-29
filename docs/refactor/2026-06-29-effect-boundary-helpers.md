# Effect Boundary Helpers

Status: implementation note for issue #7010, 2026-06-29.

Use `@openagentsinc/effect-boundary` for new external-boundary decoding:

```ts
const body = yield* readRequestJsonEffect(BodySchema, request, "route.create")
const row = yield* decodeRowEffect(RowSchema, rawRow, "store.read")
const state = yield* parseLocalStateJsonEffect(StateSchema, text, "pylon.state")
```

The helper errors are typed as `OpenAgentsBoundaryError`. They preserve the
operation name plus public-safe `reasonRef` values such as
`boundary.request_json.route.create.schema_mismatch`, but they do not include
raw JSON text, row payloads, local paths, config values, or secret material.

Migration guidance:

- Prefer `parseJsonEffect(schema, text, operation)` over
  `JSON.parse(text) as T`.
- Prefer `readRequestJsonEffect(schema, request, operation)` at Worker request
  boundaries when malformed or mismatched bodies should fail closed.
- Prefer `parseLocalStateJsonEffect` for local Pylon state files; callers may
  still fail soft by catching `OpenAgentsBoundaryError` and pruning corrupt
  local cache files.
- Decode D1/SQLite rows with `decodeRowEffect` before converting storage shape
  into domain records.
- Read secrets through `Config.redacted(...)` and
  `readRedactedConfigEffect(...)` so config failures report typed blocker refs
  without echoing key names or values.

Verification note: the broader report-only Effect authority-boundary scan is
tracked by #7009 and is still open as of 2026-06-29; this checkout does not yet
include a scan command to run. This slice verified the new helper package and
the migrated Worker/Pylon boundaries with focused tests and package typechecks.
