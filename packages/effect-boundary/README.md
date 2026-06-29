# Effect Boundary Helpers

Use these helpers at external JSON boundaries instead of `JSON.parse(...) as T`
or broad thrown exceptions.

```ts
const body = yield* readRequestJsonEffect(MySchema, request, "my.route.body")
const row = yield* decodeRowEffect(MyRowSchema, rawRow, "my.table.select")
```

Every failure preserves the operation name and returns a public-safe
`reasonRef`; detailed parser errors stay out of public responses and logs unless
an owner-scoped caller deliberately inspects the typed error.
