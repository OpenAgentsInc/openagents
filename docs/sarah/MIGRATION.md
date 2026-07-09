# Sarah monorepo consolidation — migration receipt

Date: 2026-07-09
Tracking: [#8594](https://github.com/OpenAgentsInc/openagents/issues/8594)
Plan: `docs/fable/2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md`

## Serving amendment (owner)

**No `sarah.openagents.com` subdomain.** Production path is:

`https://openagents.com/sarah`

API routes: `https://openagents.com/sarah/api/*`
Handoffs and CTAs point at `/sarah` (and continue tokens under `/sarah/continue/...` when wired).

## Source freeze

| Field | Value |
| --- | --- |
| Private source | `OpenAgentsInc/sarah` |
| Source commit | `f027314099caf4343c5e1f283fe94c8b8e912c91` |
| Destination | `apps/sarah/` in `OpenAgentsInc/openagents` |

## Redaction audit (SM-0)

| Class | Disposition |
| --- | --- |
| `.env.local` / live tokens | **not moved** |
| `tmp/` prospect PII / evidence with contacts | **not moved** |
| Public deal-rule code + public pack prices | moved |
| Owner-priced params beyond public packs | runtime/Secret Manager only |
| `docs/evidence/*` gate JSON | classified historical; not required for runtime |
| Resend/local suppression modules | **not ported** — SM-3 converges on monorepo CRM rail |

## Phase status

| Phase | Status |
| --- | --- |
| SM-0 freeze + redaction | done |
| SM-1 Bun/Effect service backend | done (`apps/sarah` Bun server) |
| SM-2 voice UI zero-React DOM | done (`src/ui/*`); EN component promotion is follow-on via EN-2 gaps |
| SM-3 email/CRM rail convergence | done (`crm-email-rail.ts`; no parallel Resend stack) |
| SM-4 owned agent runtime | done seed (`owned-runtime.ts`; eve not a runtime dep for HTTP turns) |
| SM-5 cutover to openagents.com/sarah | in progress — Start route + handoff URL rewrites; DNS/Vercel decommission operator-armed |
| SM-6 retire private repo | README historical pointer |

## Layout

```text
apps/sarah/
  src/server.ts           # Bun fetch handler (/sarah/*)
  src/services/           # ported lib (no local email stack)
  src/agent-runtime/      # owned seed runtime
  src/ui/                 # zero-React DOM voice shell
  agent/                  # persona + tools (eve files retained as source material)
  evals/ scripts/
docs/sarah/
```

## Authority

openagents.com Worker APIs remain CRM/credits/checkout/receipts system of record.
`apps/sarah` never imports Worker internals or D1/Postgres directly.
