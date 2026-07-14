# OpenAgents Autopilot

A Node 24 and pnpm workspace for the OpenAgents application stack.

- Web: Foldkit, Vite, Effect
- API: Cloudflare Workers, Durable Objects, D1, R2, Queues
- Auth: OpenAuth target
- Sync: owned OpenAgents Sync packages
- Agent: OpenCode
- Workspace infra: SHC

## Layout

```text
apps/start/            TanStack Start host for retained Effect Native web UI
workers/api/           Cloudflare Worker API and SyncRoom Durable Object
packages/sync-schema/  Effect Schema protocol models
packages/sync-client/  browser-side sync helpers
packages/sync-worker/  Worker-side sync helpers
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

Worker dev:

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
