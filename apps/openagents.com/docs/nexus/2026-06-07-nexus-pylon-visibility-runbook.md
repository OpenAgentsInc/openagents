# Nexus/Pylon visibility runbook

Status: implemented for OpenAgents product surface #429.

This runbook covers the first public/operator visibility layer for the OpenAgents product surface
Nexus/Pylon rebuild. It is a visibility layer, not a payout executor.

## Live routes

Public:

- `GET /api/public/artanis/report`
- `GET /api/public/pylon-capacity-funnel`
- `GET /api/public/nexus-pylon/receipts/{receiptRef}`
- `GET /nexus-pylon/receipts/{receiptRef}`

Operator:

- `GET /api/operator/nexus-pylon/dashboard`
- `GET /api/operator/nexus-pylon/receipts/{receiptRef}`

Operator routes require an OpenAgents admin browser session or the admin API
token. Public routes require no token.

## Current receipt mode

The first shipped receipt fixture is simulation-only. Public receipt responses
include:

- `movementMode: "simulation"`
- `realBitcoinMoved: false`
- receipt kind and status
- public-safe payout intent or attempt refs when applicable
- payout movement fields that separate dispatch acceptance from terminal
  result/settlement evidence
- public-safe settlement state
- a public page URL and API URL

Artanis admin assignment refs are valid public receipt lookup refs too. For
example,
`GET /api/public/nexus-pylon/receipts/assignment.artanis_admin.20260611011429`
returns an `artanis_admin_assignment_closeout` receipt when the assignment row
exists. The projection includes assignment state, public-safe closeout/proof
refs, the trace digest or prefix, verdict ref, and relative timestamp displays.
It remains a closeout proof, not a payout or terminal settlement claim.

The public Artanis report composes its autonomous-loop summary from recent
persisted Artanis loop-tick rows when D1 has them. Tick closeout writes refresh
both `record_json` and `public_projection_json`; the report reader also overlays
closed-row `closeout_json` onto decoded tick records so older closed rows do not
remain stuck on example-loop refs. The public Pylon capacity funnel composes
from live Pylon registrations, assignment rows, and provider lifecycle rows; if
the lifecycle row lags an `accepted_work` assignment, the count-only public
funnel still reports that capacity as accepted.

This is intentional. It proves the receipt and dashboard projection surfaces
before #431 moves real bitcoin through isolated MDK test wallets.
An accepted dispatch or provider call must not be described as terminal
settlement until a reconciled terminal result exists.

## Operator dashboard contents

The operator dashboard projects:

- Artanis Nexus/Pylon run and dispatch state
- Pylon marketplace assignments
- Pylon readiness and settlement bridge state
- payout intents
- payout attempts
- payment-authority receipts
- blocked gates
- release-gate evidence
- the simulation/real-bitcoin distinction

It exists so operators can classify stuck Nexus/Pylon work without SSH into
the old Google Cloud Nexus machine.

## Redaction policy

Public JSON and public HTML must not expose:

- customer private data
- raw invoices
- payment hashes
- preimages
- wallet mnemonics
- wallet state or wallet secrets
- private payout targets
- operator-only notes
- private runner logs
- raw timestamps

Operator JSON can include redacted operator refs, but still must not expose
raw invoices, preimages, mnemonics, wallet secrets, provider credentials, or
private customer payloads.

The regression test is
`workers/api/src/nexus-pylon-visibility-routes.test.ts`.

## MDK secrets

`MDK_ACCESS_TOKEN` and `MDK_MNEMONIC` are Worker runtime secrets. They were set
directly in the Cloudflare Worker dashboard. Do not commit their values.

No `wrangler.jsonc` value sync is required for dashboard-set Worker secrets.
Keep repo configuration limited to bindings, typed env names, docs, and tests.
If a future operator chooses CLI rotation instead of dashboard rotation, use:

```bash
bunx wrangler secret put MDK_ACCESS_TOKEN
bunx wrangler secret put MDK_MNEMONIC
```

Only add `MDK_WALLET_MNEMONIC` if a route or adapter explicitly needs the
agent-wallet mnemonic rather than the hosted MDK account mnemonic.

## Quick smoke

Pick a receipt ref from the fixture or operator dashboard and run:

```bash
curl https://openagents.com/api/public/nexus-pylon/receipts/RECEIPT_REF
curl https://openagents.com/nexus-pylon/receipts/RECEIPT_REF
curl -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  https://openagents.com/api/operator/nexus-pylon/dashboard
```

The public response must say `realBitcoinMoved: false` until #431 records a
real two-wallet MDK movement proof.
