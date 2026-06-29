# OpenAgents Autopilot

A Bun workspace for the Cloudflare-only OpenAgents replacement stack.

- Web: Foldkit, Vite, Effect
- API: Cloudflare Workers, Durable Objects, D1, R2, Queues
- Auth: OpenAuth target
- Sync: owned OpenAgents Sync packages
- Agent: OpenCode
- Workspace infra: SHC

## Layout

```text
apps/web/              Foldkit/Vite browser app
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
bun install
bun run dev:web
```

Worker dev:

```bash
bun run dev:api
```

Validation:

```bash
bun run typecheck
bun run test
bun run lint
bun run build
```

## Learn More

- [Foldkit Documentation](https://github.com/foldkit/foldkit)
- [Effect Documentation](https://effect.website)
