# Inference gateway live verification — Gemini 3.5 Flash

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **gateway request surface built, deployed, and live in
prod**; honestly **non-green as a credits business** (paid-credits path not yet
collectable end-to-end).

## What is live

The OpenAgents inference gateway (EPIC #5474, twelve issues) is shipped to `main`
and **deployed live in production with `INFERENCE_GATEWAY_ENABLED=true`**. The
OpenAI-compatible request surface is reachable in prod:

- `POST https://openagents.com/v1/chat/completions` — key-auth → balance gate →
  cheapest-viable routing → adapter dispatch → live credit decrement metered from
  the real provider `usage` object (idempotent, never-negative) via the pricing
  engine.
- **Gemini 3.5 Flash served end-to-end** through `/v1/chat/completions` (Vertex
  Gemini adapter), verified live 2026-06-19. Free tier: free until $10 per
  verified owner-claim identity (Sybil-resistant shared pool); premium models are
  owner-grant allowlist only.

This supersedes the stale `api unbuilt` framing for
`inference.gateway_credits_business.v1`: the OpenAI-compatible gateway request
surface is **not** unbuilt — it is built, deployed, and serving real inference.

## Why it stays non-green (paid-credits path)

Free inference works; the gap is the **paid-credits path**, not the API:

- **Stripe card → credit** is fully wired in source (`stripe-billing.ts`,
  `/api/billing/checkout|summary|stripe/webhook|setup-intents`) but **no Stripe
  secrets are on prod**, so a customer cannot complete a real card → credit
  purchase against production today.
- **USD → msat bridge (#5497, MERGED)** converts purchased USD credits into
  inference-spendable msat (asset boundary enforced: USD-funded balance is
  spendable on inference, never withdrawable as Bitcoin) — but it has no real
  upstream purchase to bridge until card → credit is collectable in prod.

There is therefore **no dereferenceable paid receipt** for a real
card → credit → inference-spend round trip. Until one exists,
`inference.gateway_credits_business.v1` stays **red/non-green** as a *credits
business*, even though the gateway request surface and free inference are live.

## Evidence

- `docs/launch/JUNE19_ROADMAP.md` (VALUE — LIVE: gateway enabled in prod, Gemini
  3.5 Flash free tier)
- `docs/inference/2026-06-19-inference-gateway-business.md`
- `docs/inference/2026-06-19-fireworks-provider.md` (open-model passthrough lane,
  provider connection verified live)
- `apps/openagents.com/workers/api/src/inference/` (router, pricing, adapters:
  vertex-gemini, fireworks, passthrough; usd-credit-bridge)
- EPIC #5474 (gateway), #5485 (Autopilot free-Gemini client), #5497 (USD → msat
  bridge)

Live reachability check (unauthenticated, prod):

```
$ curl -s -o /dev/null -w "%{http_code}" -X POST \
    https://openagents.com/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -d '{"model":"gemini-3.5-flash","messages":[{"role":"user","content":"ping"}]}'
401
```

A `401 unauthorized` (not `404` / disabled) confirms the route is deployed and
key-auth-gated in production. Authenticated free-Gemini serving was verified
separately on the owner-claim free pool.
