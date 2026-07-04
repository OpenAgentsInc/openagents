# OpenAgents.com Start Staging

TanStack Start staging scaffold for TS-2a (#8343). This package is intentionally
isolated from the live `openagents.com` Worker and routes.

## Verify

```sh
bun run --cwd apps/openagents.com/apps/start build
bun run --cwd apps/openagents.com/apps/start test
```

## Deploy

This app deploys to its own Worker:

```sh
bun run --cwd apps/openagents.com/apps/start deploy
```

The Wrangler name is `openagents-com-start-staging`. Do not deploy it through
the live `apps/openagents.com/workers/api` deploy path, and do not attach it to
the production `openagents.com` route in this slice.

After deploy, record the `*.workers.dev` URL in the issue or PR and smoke it:

```sh
curl -fsS https://openagents-com-start-staging.<account>.workers.dev/ \
  | rg 'OpenAgents|What is Khala\\?|Join the Tassadar training run'
```

## Template Deviations

- Dropped Sentry wiring: this staging package does not yet own production error
  telemetry and should not reuse TanStack's project DSN.
- Dropped content collections: TS-2a ports only the landing route, with docs and
  content routes left for later slices.
- Dropped Drizzle/database/runtime host context: the landing page has no data
  dependency, and the issue requested room for later content negotiation and
  crons rather than a database boundary now.
- Dropped analytics proxy and scheduled tasks: no deploy in this run, and the
  staging Worker should not emit production analytics or run cron work.
- Used a static hero fold: the production Three.js scene is deferred by the
  issue; the fold preserves the dark Khala/StarCraft token palette without
  adding a second renderer.

## Site Speed Budget

Landing route budget for the first deployed staging URL:

- SSR document must contain real landing content in the initial HTML.
- JavaScript should stay split into React, TanStack Router, TanStack Query, and
  TanStack Start chunks.
- Initial landing document should keep LCP under 2.5s on a local Lighthouse
  mobile run before promotion to a custom domain.
