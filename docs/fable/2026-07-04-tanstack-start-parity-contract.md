# TanStack Start Parity Contract

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Date: 2026-07-04
Issue: [#8341](https://github.com/OpenAgentsInc/openagents/issues/8341)
Epic: [#8339](https://github.com/OpenAgentsInc/openagents/issues/8339)
Scope: TS-1 `effect-start` bridge and pinned TanStack Start staging contracts.

This file is the parity ledger for the React/Tailwind Start stack. It pins the
libraries the staging Worker may use, records known template gaps, and defines
the upgrade procedure TS-2/TS-10b must follow before changing versions.

## Pins

| Package | Pinned version | Consumer | Notes |
| --- | --- | --- | --- |
| `@tanstack/react-start` | `1.168.26` | `apps/openagents.com/apps/start` | Start server/client scaffold from TS-2a. |
| `@tanstack/react-start-client` | `1.168.14` | `apps/openagents.com/apps/start` | Client half paired with the current scaffold. |
| `@tanstack/react-router` | `1.170.16` | `apps/openagents.com/apps/start` | The issue's "react-router" leg is the TanStack Router package used by Start, not `react-router`. |
| `@tanstack/react-router-ssr-query` | `1.167.1` | `apps/openagents.com/apps/start` | SSR query integration already present in the scaffold. |
| `@tanstack/query-core` | `5.101.0` | `apps/openagents.com/apps/start` | Explicit peer pin so SSR query integration and React Query share one private `QueryClient` type. |
| `@tanstack/react-query` | `5.101.0` | `apps/openagents.com/apps/start` | Pinned by TS-1; aligned with the workspace's existing Query core to avoid duplicate private `QueryClient` types. |
| `@tanstack/db` | `0.6.14` | `apps/openagents.com/apps/start` | Added as the future TS-3/TS-2 data bridge pin; unused by the landing page. |
| `@tanstack/react-db` | `0.1.92` | `apps/openagents.com/apps/start` | React adapter pin for the future `khala-sync-db-collection` consumer. |
| `@cloudflare/vite-plugin` | `1.42.0` | `apps/openagents.com/apps/start` | Pinned by TS-1; no caret. |

The app keeps `compatibility_flags: ["nodejs_compat"]` in
`apps/openagents.com/apps/start/wrangler.jsonc` because the shared
`@openagentsinc/effect-start` package follows the TanStack template's
`AsyncLocalStorage` host-runtime pattern with `node:async_hooks`.

## Template Gaps

- No Sentry wrapper yet. TS-2a intentionally avoided reusing upstream project
  telemetry keys; add an OpenAgents-owned telemetry plan before enabling it.
- No content collections, analytics proxy, scheduled task, or database runtime
  layer yet. The landing page has no data dependency; TS-2/TS-3 add consumers
  route by route.
- No production route cutover. The Start app still deploys only as the
  independent `openagents-com-start-staging` Worker until owner review signs a
  route-level promotion.
- No public-copy changes ride version upgrades. User-facing copy remains
  registry-governed and verbatim-port only.

## Upgrade Procedure

1. Open or cite the issue that needs the version change; do not opportunistically
   drift pins during unrelated page work.
2. Compare the local TanStack template reference
   `/Users/christopherdavid/work/projects/tanstack/repos/tanstack.com` and npm
   package metadata for the packages above. Record any API or plugin-order
   differences in this file.
3. Update `apps/openagents.com/apps/start/package.json`, `bun.lock`, and this
   parity contract in the same commit.
4. Run:

```sh
bun install
bun run --cwd packages/effect-start test
bun run --cwd packages/effect-start typecheck
bun run --cwd apps/openagents.com/apps/start test
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
```

5. For any route using TanStack DB/React DB, add a behavior contract or adapter
   test that proves the route still maps server-confirmed data without
   optimistic durable writes.

## Bridge Contract

`@openagentsinc/effect-start` is the only TS-1 bridge package. It provides:

- `withStartRequestContext` / `currentStartEnv` / `currentStartRequest` for
  per-request env and request access through `AsyncLocalStorage`;
- `makeEffectStartRuntime` and `effectStartRuntime`, which keep
  `Effect.runPromise` at the handler boundary;
- `decodeStartInput` and `handleJson`, which schema-decode server
  function/loader inputs before running an Effect program;
- typed public-safe HTTP mappings for input decode failures, missing request
  context, and explicit `StartHttpError` failures.

App routes and server functions may define their own domain errors, but they
must map them at the Start handler boundary rather than throwing raw provider,
database, or framework errors into public responses.
