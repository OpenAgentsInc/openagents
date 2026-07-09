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
| SM-1 Bun/Effect service backend | done (`apps/sarah` Bun server + `effect-layers.ts` Context.Service tags) |
| SM-2 voice UI zero-React DOM | done (`src/ui/*`); EN component promotion is follow-on via EN-2 gaps |
| SM-3 email/CRM rail convergence | done (`crm-email-rail.ts` — CRM rail client + local dry-run draft/opt-out projection; no Resend) |
| SM-4 owned agent runtime | done seed (`owned-runtime.ts`; eve not a runtime dep for HTTP turns) |
| SM-5 cutover to openagents.com/sarah | **done** — owner-confirmed path mount; `openagents-monolith` serves `/sarah*` (`handleSarahRequest`); live S-12 **6/6 CONFIRMED** on `https://openagents.com/sarah` (2026-07-09, rev `openagents-monolith-00046-pgq`) |
| SM-6 retire private repo | README historical pointer; Vercel project teardown residual (subdomain DNS already NXDOMAIN) |

## Oracle receipt (local monorepo)

Run from `apps/sarah` (S-3 self-spawns an isolated capped server in test mode):

| Gate | Result |
| --- | --- |
| `bun test` | green |
| deal-rules property | green |
| S-3 token-guard smoke | green (self-spawn + token redaction; Origin = host origin) |
| S-12 eval suite | 6/6 CONFIRMED |
| S-8 continuity / suppression | green on CRM rail projection |
| S-13 follow-up smoke | green (idempotent; clears state files) |

## Production cutover receipt (SM-5)

| Field | Value |
| --- | --- |
| Serving | `https://openagents.com/sarah` (no subdomain) |
| Host | Cloud Run `openagents-monolith` (`handleSarahRequest` front-controller mount) |
| Deploy revision | `openagents-monolith-00046-pgq` (2026-07-09) |
| Live S-12 | **6/6 CONFIRMED** against `https://openagents.com/sarah` |
| Ops probe | `GET /sarah/api/operator/ops` → `apps/sarah` mount JSON |
| Continue | `GET /sarah/continue/<token>` mints prospect cookie |
| Residual | Vercel project teardown (operator; `sarah.openagents.com` already NXDOMAIN) |
| Still open on #8594 | SM-2 EN component authoring (interim DOM shell is not the EN mandate) |

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
