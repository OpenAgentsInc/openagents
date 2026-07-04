# @openagentsinc/effect-start

Effect boundary helpers for TanStack Start server functions, route loaders,
and Cloudflare Worker entries.

The package keeps `Effect.runPromise` at the handler boundary, stores the
current request/env/execution context in `AsyncLocalStorage`, and maps typed
bridge errors to public-safe JSON HTTP responses. App code should import the
helpers here instead of creating new app-local Effect/Start bridges.

## Verify

```sh
bun run --cwd packages/effect-start test
bun run --cwd packages/effect-start typecheck
```
