# Coax Web Canonical Application Skeleton

Status: locked 2026-11-26 (Issue #14074; audit: [`docs/audits/2026-11-26-laravel-to-coax-web-audit.md`](../../docs/audits/2026-11-26-laravel-to-coax-web-audit.md)).

**Coax Web** (formerly Coax Native Web / Coaxial) is Coax's full-stack web
framework: Coax-to-Wasm browser pages (WebGPU worlds + `components/backend/dom.coax`
text/UI surfaces) paired with a compiled C/Coax HTTP API, Postgres job queue,
cache/lock primitives, migrations, and unified `coax web` CLI.

## Framework vs Application Boundary

- **Framework (`coax/web/`):** Reusable across every Coax Web site
  (`openagents.com`, `gamebuilder.org`, future apps). Owns:
  - Browser host (`host.js`, `dom-host.js`, `byte-storage.js`, `state-keep.js`, `wasm.sh`)
  - Dev server & hot swap (`serve.c`, `serve.sh`)
  - Unified CLI (`cli.sh`, `cli-policy.coax`, `tool.sh`)
  - Reusable HTTP/API primitives (`coax/web/api/`: `route-policy.coax`,
    `config-policy.coax`, `log-context.coax`, `maintenance-policy.coax`,
    `http-client-policy.coax`, `http_client.c`, `queue-policy.coax`,
    `lock-policy.coax`, `queue.c`, `signed-url.coax`, `cursor-wire.coax`,
    `auth.c` + `auth-policy.coax`)
  - Release & staging pipeline (`deploy.sh`, `release.sh`, `rollback.sh`)
- **Application (`apps/<site>/`, e.g. `apps/openagents-web/`, `gamebuilder/web/`):**
  Site-specific pages, routes, domain `#authority` policies, wire codecs, thin
  HTTP handlers, Postgres migrations, fixtures, and tests.

## Canonical Application Layout

New Coax Web applications — and `apps/openagents-web/` as it zero-bases — follow
this predictable directory layout so both humans and AI coding agents locate
every layer immediately:

```text
apps/<site>/
├── app.coax                  # Single app manifest (pages, routes, config, jobs)
├── .env.example              # Safe local dev defaults (zero secrets)
├── AGENTS.md                 # Concise agent guide pointing to `coax web` introspection
├── config/
│   ├── app.coax              # Typed env config + production boot assertions
│   └── testing.coax          # Deterministic test overrides (fake mail/http/clock)
├── routes/
│   ├── web.coax              # Static/Wasm pages, redirects, /.well-known/*, /llms.txt
│   ├── api.coax              # Declarative API routes, auth rules, scopes, rate limits
│   └── console.coax          # Scheduled tasks & admin CLI commands
├── app/
│   ├── pages/                # One subfolder per page (entry.coax, face.c, *.wgsl, index.md)
│   ├── policies/             # Pure #authority *-policy.coax (0 surviving mutants)
│   ├── wire/                 # *-wire.coax codecs, validation & JSON Schema / OpenAPI
│   ├── handlers/             # Thin C/Coax HTTP adapters (host I/O only, zero business rules)
│   ├── mail/                 # Pure Coax mail templates (text + HTML)
│   └── jobs/                 # Async Postgres queue jobs & scheduled tasks
├── database/
│   ├── migrations/           # Immutable NNNN_*.sql + migrations.sha256
│   ├── schema.sql            # Canonical generated Postgres snapshot
│   ├── factories/            # Composable Coax test data builders
│   └── seeders/              # Local dev seeders
├── public/                   # Static root files (favicon.svg, mark.svg, robots.txt, .well-known)
└── tests/
    ├── unit/                 # Fast compile-time `coax test` + `coax mutate` (no DB)
    └── feature/              # Isolated HTTP + Postgres integration tests
```

## Non-Negotiable Coax Web Invariants

1. **Zero npm / zero bundler / zero JS framework** on the browser host.
2. **All domain rules are `#authority` Coax** (`*-policy.coax`), verified with
   `#test`, `#forall`, and `coax mutate` with zero surviving mutants.
3. **Zero ActiveRecord ORM or runtime DI container**: SQL stores rows, guards
   uniqueness/versions, and drives transactional outboxes (`queue.jobs`); wiring
   is explicit and compile-time checked.
4. **ABI & Wire Stability**: Renaming to **Coax Web** never renames `coaxial_*`
   Wasm exports, `/coaxial/live`, `/coaxial/code`, or live GCP resource IDs.
5. **Single Source of Truth**: Routes, config schema, migrations, and OpenAPI
   manifests are derived from checked declarations — never duplicated across
   hand-maintained lists.

## Migration Path from Current `apps/openagents-web/` & `coax/web/api/`

| Existing Location | Canonical Role |
| --- | --- |
| `coax/web/api/*-policy.coax` | Framework policies (`route`, `config`, `maintenance`, `http-client`, `queue`, `lock`, `auth`) stay in `coax/web/api/`; app-specific policies move under `app/policies/` on next touch or zero-base |
| `coax/web/api/*-wire.coax` | `app/wire/` (and shared wire primitives in `coax/web/api/`) |
| `coax/web/api/*.c` | Thin adapters (`app/handlers/`); core HTTP/queue/config/http_client kernel stays in `coax/web/api/` |
| `coax/web/api/migrations/` + `schema.sql` | `database/migrations/` + `database/schema.sql` |
| `apps/openagents-web/build.sh` | Replaced incrementally by `coax web build` reading declarative page entries |
| `coax/web/site-routes.coax` + `coax/web/api/lb.sh` | Unified under `routes/web.coax` |
