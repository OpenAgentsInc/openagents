# `@openagents/effect-spike`

Half-day feasibility sprint implementing [`docs/effect/half-day-sprint.md`](../../docs/effect/half-day-sprint.md): an **Effect v4 + `@effect/ui` (no React)** streaming web app and server bridge driving the compiled Coax coder (`oa-worker`) over bidirectional `mcworker-wire.coax` NDJSON and SSE.

## Stack

- `effect` `^4.1.6` + `@effect/platform-node` `^4.1.6`
- `@effect/ui`, `@effect/ui-kit`, `@effect/ui-devtools` pinned to [`Effect-TS/ui@9d672abe`](https://github.com/Effect-TS/ui/commit/9d672abe) via `pkg.pr.new`
- Vite 8 (`HtmlServerVite` + `DevToolsVite`) + Tailwind CSS v4 (`@tailwindcss/vite`)
- Real compiled Coax coder (`OA_WORKER_BIN`, defaulting to `/Applications/OpenAgents.app/Contents/Resources/oa-worker`)

## Commands

```sh
cd apps/openagents-effect-spike
pnpm install
pnpm check       # tsc --noEmit (strict + exactOptionalPropertyTypes)
pnpm test        # Vitest: wire codecs, Story.explore race proof, Play + TestClock + SSR
pnpm build       # dist/client + standalone production Web Handler in dist/server/index.js
pnpm dev         # http://localhost:5173 + DevTools MCP at http://127.0.0.1:4774/mcp
```

## Environment Variables

- `OA_WORKER_BIN`: Path to compiled `oa-worker` binary (default: `/Applications/OpenAgents.app/Contents/Resources/oa-worker`).
- `OA_SPIKE_CWD`: Working directory handed to spawned `oa-worker` sessions (default: current directory).
- `OA_AGENT_PERMISSION_MODE`: Initial permission mode for spawned workers (default: `default` for interactive `MWW_PERMIT` approval cards).
