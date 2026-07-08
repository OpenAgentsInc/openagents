# TS-2 TanStack Start Funnel Routes Receipt

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Date: 2026-07-04
Issue: #8344
Epic: #8339

## Scope

The TS-2 staging Worker now serves the public funnel pages from
`apps/openagents.com/apps/start`:

- `/business`
- `/docs`
- `/docs/api`
- `/blog`
- `/blog/introducing-khala-code`
- `/code/download`
- `/autopilot`
- `/autopilot/legal`

The live `openagents.com` Worker and routes were not cut over. The deprecated
`apps/openagents.com/apps/web` Foldkit pages remain in place until owner staging
sign-off and a later per-route production cutover.

## Shared Discovery Surfaces

The Start server entry checks the API Worker's shared helper modules before
falling through to TanStack Start:

- `routeSiteCrawlSurfaceRequest`
- `routeWellKnownAgentSurfaceRequest`
- `renderDiscoverySurface`

That keeps these surfaces single-sourced with the API Worker:

- `/llms.txt`
- `/agents.md`
- `/ai.md`
- `/skill.md`
- `/robots.txt`
- `/sitemap.xml`
- `/.well-known/mcp.json`
- `/.well-known/mcp/manifest.json`
- `/.well-known/ai-catalog.json`

`/.well-known/openagents.json` remains owned by the API Worker in this slice.

## Staging

- Worker: `openagents-com-start-staging`
- URL: https://openagents-com-start-staging.openagents.workers.dev
- Version ID: `01014344-715c-46f2-a71d-6b6ff5db7587`
- Wrangler startup time: 34 ms

SSR smoke markers passed on the staging URL for every route listed in scope and
for `/llms.txt`, `/robots.txt`, `/sitemap.xml`, `/.well-known/mcp.json`, and
`/.well-known/ai-catalog.json`.

## Budget Gate

`bun run --cwd apps/openagents.com/apps/start budget` records
`openagents.start_funnel_route_budget.v1` after a production build.

Current result:

- Total client JS across Start funnel routes: 409.2 KiB raw.
- Split route chunks: 9.
- Largest route-owned chunk: 22.5 KiB raw (`-funnel-components-*`).
- Per-route route-file chunks are each under 1 KiB raw.

This is the TS-2 merge gate for the staged routes. Full lab measurements can
still use the existing site-speed lane against the staging origin when owner
review needs browser-profile medians.

## Verification

- `bun run --cwd apps/openagents.com/apps/start test`
- `bun run --cwd apps/openagents.com/apps/start typecheck`
- `bun run --cwd apps/openagents.com/apps/start budget`
- `bun test apps/openagents.com/workers/api/src/well-known-agent-surfaces-routes.test.ts apps/openagents.com/workers/api/src/site-crawl-surfaces-routes.test.ts apps/openagents.com/workers/api/src/inference/discovery-surfaces.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd apps/openagents.com/apps/start deploy`
- `bun run check:deploy`
