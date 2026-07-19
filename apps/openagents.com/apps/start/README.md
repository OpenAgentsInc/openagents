# OpenAgents.com Start Staging

TanStack Start application for the retained `openagents.com` document routes.
It is built into the Google Cloud Run monolith.

The historical TS-1 parity contract lives in
[`docs/fable/2026-07-04-tanstack-start-parity-contract.md`](../../../../docs/fable/2026-07-04-tanstack-start-parity-contract.md).
Server loaders/functions should use `@openagentsinc/effect-start` for the
Effect boundary and server request context instead of creating app-local
bridges.

## UI Components

This package is configured for shadcn/ui on Tailwind 4:

- `components.json` uses the `new-york` style, `@/*` imports, and
  `src/styles.css`.
- Shared primitives live under `src/components/ui`.
- New TanStack Start route UI should prefer these primitives for buttons,
  badges, cards, and repeated panel surfaces while preserving the dark-only
  OpenAgents tokens from `src/styles.css`.
- Phase-1 WEB-1 vendors the Launch UI homepage replica under
  `src/components/launch-ui` with the blue glow token preset and minimal radius
  pinned in `src/styles.css`. The MIT notice is retained in
  `THIRD_PARTY_NOTICES.md`.

## Verify

```sh
pnpm --dir packages/effect-start run test
pnpm --dir apps/openagents.com/apps/start run build
pnpm --dir apps/openagents.com/apps/start run test
pnpm --dir apps/openagents.com/apps/start run typecheck
pnpm --dir apps/openagents.com/apps/start run budget
```

## Deploy

This app deploys only as part of the Cloud Run monolith:

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  bash apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh production
```

Deploy staging first and smoke the Cloud Run revision:

```sh
curl --retry 5 --retry-delay 2 --retry-all-errors -fsS https://openagents-monolith-staging-157437760789.us-central1.run.app/ \
  | rg -a 'OpenAgents|What is Khala\\?|Join the Tassadar training run'
```

## Template Deviations

- Dropped Sentry wiring: this staging package does not yet own production error
  telemetry and should not reuse TanStack's project DSN.
- Dropped content collections: TS-2a ports only the landing route, with docs and
  content routes left for later slices.
- Dropped Drizzle/database/runtime host context: the landing page has no data
  dependency, and the issue requested room for later content negotiation and
  crons rather than a database boundary now.
- Scheduled work is owned by Cloud Scheduler and the Cloud Run API, not this
  document application.
- Used a static hero fold: the production Three.js scene is deferred by the
  issue. The fold preserves the dark Khala/StarCraft token palette without
  adding a second renderer.

## Site Speed Budget

Landing route budget for the first deployed staging URL:

- SSR document must contain real landing content in the initial HTML.
- JavaScript should stay split into React, TanStack Router, TanStack Query, and
  TanStack Start chunks.
- The Start funnel total client JavaScript budget is 780 KiB after the WEB-1
  phase-1 Launch UI replica added the full Launch UI section hierarchy.
- Initial landing document should keep LCP under 2.5s on a local Lighthouse
  mobile run before promotion to a custom domain.

## Historical 2026-07-04 edge-host receipt

This receipt records a retired experiment. It is not a current deploy target
or infrastructure authority.

- Former service: `openagents-com-start-staging`
- TS-2a version ID: `dce2450d-c23c-42ed-9eb0-8ffada0b05cb`
- TS-2 version ID: `01014344-715c-46f2-a71d-6b6ff5db7587`
- TS-2 startup time recorded by the retired host: 34 ms
- Landing SSR smoke:
  completed against the former staging origin. That origin is retired.
- TS-2 staged routes smoked with SSR markers:
  `/business`, `/docs`, `/docs/api`, `/blog`,
  `/blog/introducing-khala-code`, `/code/download`, `/autopilot`,
  `/autopilot/legal`.
- Shared agent surfaces are served before the Start fallback from the API
  helpers: `/llms.txt`, `/agents.md`, `/ai.md`, `/skill.md`,
  `/robots.txt`, `/sitemap.xml`, `/.well-known/mcp.json`,
  `/.well-known/mcp/manifest.json`, and `/.well-known/ai-catalog.json`.
- Route budget gate: `openagents.start_funnel_route_budget.v1`, 409.2 KiB
  total client JS, 9 split route chunks, largest route-owned chunk 22.5 KiB.
