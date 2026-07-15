# OpenAgents Autopilot

A Node 24 and pnpm workspace for the OpenAgents application stack.

- Public site: Astro candidate at `/astro`
- Public docs: Blume static site at `/docs`
- Retained web: TanStack Start/React host during the Astro cutover
- API: Node service on Google Cloud Run
- Auth: OpenAuth target
- Data and sync: Cloud SQL Postgres, Cloud Run LiveHub, Cloud Storage
- Agent: OpenCode
- Infrastructure: Google Cloud only

## Layout

```text
apps/astro/            Static OpenAgents Desktop MVP public-site candidate
apps/docs/             Curated Blume docs, static host, and deploy script
apps/start/            TanStack Start host for retained Effect Native web UI
workers/api/           retained path for the Node/Cloud Run API
packages/sync-schema/  Effect Schema protocol models
packages/sync-client/  browser-side sync helpers
packages/sync-worker/  server-side sync helpers
docs/                  architecture audits and migration plans
```

Foldkit reference source must stay outside this repo. Use
`../projects/repos/foldkit/` or the installed `foldkit` package; do not commit
`repos/foldkit`.

## Getting Started

```bash
pnpm install
pnpm run dev:web
```

API dev:

```bash
pnpm run dev:api
```

Validation:

```bash
pnpm run typecheck
pnpm run test
pnpm run lint
pnpm run build
```

## Learn More

- [Foldkit Documentation](https://github.com/foldkit/foldkit)
- [Effect Documentation](https://effect.website)
