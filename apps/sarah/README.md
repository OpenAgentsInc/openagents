# Sarah (`apps/sarah`)

AI sales assistant for OpenAgents. Mounted at **`https://openagents.com/sarah`**
(no separate subdomain).

## Run locally

```bash
bun install
bun run --cwd apps/sarah dev
# http://127.0.0.1:8790/sarah
```

## Tests

```bash
bun run --cwd apps/sarah test
```

## Layout

- `src/server.ts` — Bun fetch handler for `/sarah/*`
- `src/services/` — domain services (CRM/sales clients, deal rules, token guard)
- `src/services/crm-email-rail.ts` — monorepo CRM approval rail (no local Resend)
- `src/agent-runtime/` — owned seed runtime (eve not required for HTTP turns)
- `src/ui/` — zero-React DOM voice shell
- `agent/` — persona + tool sources

See `docs/sarah/MIGRATION.md` and issue #8594.
