# Machine Payments (MPP) at OpenAgents — Overview

> **Current status (2026-07-05, #8387): RETIRED / DEFERRED.** The standalone
> Khala MPP/x402 chat endpoint (`POST /mpp/v1/chat/completions` and canonical
> `/api/mpp/v1/chat/completions`), root MPP discovery document, Stripe MPP
> profile config, smokes, and replay-cache writers were removed because the
> surface was default-off, unarmed in committed config, and not directly needed
> by Khala Code. This document is preserved as historical design context only.
> Future no-account machine payments need a fresh owner-approved,
> receipt-first design before any route, discovery, or public claim returns.

*Plain-language orientation. If you're new (human or agent), read this first, then
the [operations runbook](./2026-06-23-mpp-launch-and-badge-runbook.md) for the
how-to detail.*

> **Status (2026-06-23): all three rails (⚡ Lightning, USDC, card) are LIVE on
> production; only the Stripe Directory badge is pending** (an external, async Stripe
> crawl, ≤~24h). Lightning leads the 402, minting a real mainnet BOLT11 via Spark.

---

## The one-sentence version

We let AI agents **pay our inference API per request — no signup, no API key** —
by answering an unpaid request with an HTTP `402 Payment Required`, taking the
payment, and then returning the requested completion. The only external MPP
model id is `openagents/khala`. Inside the OpenAgents ecosystem the same model
is just `khala`. Raw GPT-OSS ids (`openai/gpt-oss-20b`,
`openai/gpt-oss-120b`) are internal Hydralisk supply targets only: they are not
public products, not sellable over MPP, and are rejected before any payment
challenge is issued.

## Why we're doing this

1. **A new way to get paid + discovered.** Agents are starting to find and buy
   services on their own. Stripe (with Tempo) built the **Machine Payments
   Protocol (MPP)** and a **directory** where agents browse paid APIs. Today **no
   inference provider in that directory accepts machine payments** — so being the
   first "LLM inference, pay-per-call" listing is a real first-mover slot. The
   visible proof of that is the **"Machine Payments" badge** next to our profile.
2. **Bitcoin-first, on brand.** OpenAgents already pays contributors over
   Bitcoin/Lightning (Spark). Accepting **Lightning** for inference closes the
   loop in Bitcoin rather than fiat. So when an agent is offered a choice of how
   to pay, we want **Lightning surfaced first**, with stablecoin (USDC) and card
   as alternatives.
3. **Build it once, sell it to everyone.** The same payment infrastructure we
   built for ourselves is meant to be reusable by our customers later.

## How it works (the whole flow in plain terms)

1. An agent calls our endpoint **`POST /api/mpp/v1/chat/completions`** with no
   payment. (`/api` is the canonical base for all OpenAgents API routes, #6148;
   the legacy `POST /mpp/v1/chat/completions` path keeps working as a
   non-breaking alias to the same handler.)
2. We reply **`402 Payment Required`** with a *challenge*: here's the price and
   here's how to pay (one option per supported rail).
3. The agent's wallet pays and **retries** the request, attaching proof of
   payment.
4. We **verify** the proof, run the requested completion, and return it plus a
   **receipt**.

There are three ways to pay (**rails**):

| Rail | What the agent pays with | How we verify |
|---|---|---|
| ⚡ **Lightning** | Bitcoin over Lightning (pays a BOLT11 invoice) | We check the payment secret (`preimage`) locally — one hash, instant |
| **USDC (stablecoin)** | USDC on Base / Solana / Tempo, into our Stripe balance | Stripe confirms the on-chain deposit settled |
| **Card** | A card via Stripe "shared payment token" | Stripe charges the token |

Two supporting pieces make us *discoverable*:

- **`/openapi.json`** — a machine-readable menu that says "this endpoint is
  payable, here are the prices and rails." Stripe's directory crawler reads this
  to list us and light the **Machine Payments badge**.
- **Our Stripe profile** (`@openagents`) — our public identity in the directory.

## Important design choices (so you don't get surprised)

- **Fail-closed / honesty gate.** If anything needed to take a payment is missing
  or a rail can't actually be fulfilled, we return a clean `402`/`503` and **do
  not** advertise or accept that rail. We never advertise a rail we can't honor,
  and we never serve a completion we weren't paid for.
- **Verification runs in our normal Cloudflare Worker** — no extra "sidecar"
  server was needed. The challenge is signed/verified with standard crypto.
- **Money semantics matter.** Stablecoin and card payments become
  **inference-spendable credit that is NOT Bitcoin-withdrawable** (so people can't
  use us as a USD→BTC off-ramp). Lightning payments are **real Bitcoin in**.
- **Everything is behind flags and was proven before going live.** We rehearsed
  the full pay→serve loop on staging with test money before arming production.

## Where it stands right now (2026-06-23)

**Status: all three rails are LIVE on production** (`openagents-autopilot`). The
402 ordering is **lightning → base/usdc → stripe/card**, and `/openapi.json`
lists **lightning first**. The primary and only advisory model is
`openagents/khala`; Hydralisk GPT-OSS 20B/120B remains an internal backing lane
for Khala, not a public/Mpp-payable model selector. Only the Stripe badge is
still pending (external crawl).
Production deploy `e66a59cd-7ad4-48bf-801e-1230064a467f` also has a live paid
proof from before the slug collapse: a 1-sat Lightning MPP request for
`openai/gpt-oss-20b` returned `200`, an OpenAI-compatible `chat.completion`, and
a successful `Payment-Receipt` on 2026-06-24T01:51:12Z. Current policy supersedes
that raw-id sale path: repeat payments must use `openagents/khala`.

- ✅ **Live: Lightning — and it leads.** It mints a real mainnet BOLT11 via **Spark**
  (`@breeztech/breez-sdk-spark`, **primary**) through the existing **`MDK_TREASURY`
  container** (`/spark/funding-invoice`), with the **MDK sidecar as the explicit
  fallback** issuer only. It runs behind a bounded timeout + per-rail isolation so a
  slow/failed Lightning leg only drops Lightning and can never hang the endpoint. Verify
  is local (`sha256(preimage)==paymentHash`). Latency ~0.9–2.4s warm (cold first mint
  ~5.6s). (Getting here was a saga: the Spark mint code lives in the `MDK_TREASURY`
  container, and it was stuck on an old image because the container build needs Docker
  running locally — see the runbook's deploy gotcha.)
- ✅ **Live in production:** the USDC (crypto) and card rails. An unpaid request
  gets a real `402` with a deposit address; `/openapi.json` advertises the offers.
  The full crypto pay loop (pay → verify → completion → receipt → credit) was
  proven end-to-end on staging, and the default pay-loop smoke now targets
  `openagents/khala`.
- ⏳ **The badge:** everything on our side is done; it now depends on Stripe's
  crawler indexing `/openapi.json`, which is **asynchronous (up to ~24h)**. A
  background watch re-checks the directory every 30 min and will announce the
  moment the badge appears.

## What's next

1. ✅ Done — all three rails (Lightning, USDC, card) are live; Lightning leads the 402.
2. **Badge** appears in the directory (we're watching — external Stripe crawl).
   Optionally register on broader MPP registries (MPPScan, mpp.dev/services) to widen
   agent discovery.
3. ✅ Done — the MDK fallback sidecar route now normalizes stale/doubled
   `/api/mdk/api/mdk` configs back to exactly `/api/mdk`, and the Spark issuer
   accepts both the normalized treasury response (`bolt11Invoice`/`paymentHash`)
   and Spark SDK-style response fields (`paymentRequest` +
   nested `payment.details.htlcDetails.paymentHash`).
4. **Optimization:** a **pre-minted Spark invoice pool** for zero-latency 402s (removes
   the cold-mint first-hit cost). Tracked on #6049.
5. Later: prove the card rail end-to-end, and reuse this for other paid
   primitives (sandboxes, fine-tuning).

## Where to look

| Thing | Location |
|---|---|
| Operations how-to (arm/disarm, deploy, smokes, badge-check, gotchas) | [`./2026-06-23-mpp-launch-and-badge-runbook.md`](./2026-06-23-mpp-launch-and-badge-runbook.md) |
| The code | `apps/openagents.com/workers/api/src/inference/mpp/` |
| Discovery doc builder | `…/inference/mpp-discovery-document.ts` → served at `/openapi.json` |
| Protocol spec (local mirror, authoritative) | `docs/reference/mpp/` |
| Strategy / full integration plan | `docs/stripe/2026-06-22-khala-mpp-integration-plan.md` |
| Tracking | Epic [#6049](https://github.com/OpenAgentsInc/openagents/issues/6049) |

## Mini-glossary

- **MPP** — Machine Payments Protocol: the `402` challenge → pay → retry → receipt
  standard Stripe + Tempo built. Lives at mpp.dev / paymentauth.org.
- **402** — the HTTP "Payment Required" status; our paywall response.
- **Rail** — a way to pay (Lightning, USDC, card).
- **BOLT11 / preimage** — a Lightning invoice / the secret that proves it was paid.
- **Deposit address** — the crypto address Stripe gives the agent to send USDC to.
- **The badge** — the "Machine Payments" mark on our Stripe directory profile that
  tells agents we accept pay-per-call.
- **Honesty gate** — our rule: only advertise/accept a rail that actually works.
