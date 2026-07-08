# OpenAgents Inference Gateway + Credits Business — initial thoughts

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **initial thinking doc**, not a spec. Companion to the
"Let's Make Money" revenue-engine plan (root `launch-videos/2026-06-18-video-2-referral-revenue-share.md`)
and the §2H implementation audit. The point of this doc: sketch the inference
business — one solid API where people and agents **buy inference credits**, and we
serve them from the supply we already have (Google Cloud / Vertex quota + our agent
network + partner passthrough).

> Honest framing up front: this is the **buy-side** OpenAgents has historically never
> closed (see the Ep 138 "Year One Recap" lesson — lots of sell-side, no paying
> demand). Inference credits is a buy-side people and businesses *already pay for
> elsewhere*; we don't have to invent demand, only offer a better/aggregated/settleable
> version. This is also the explicitly **allowed** resale lane under our no-resale
> invariant (API-inference resale is permitted; only subscription-seat resale is
> forbidden).

## 1. The thesis

Stand up an **OpenAgents inference gateway**: one OpenAI-/Anthropic-compatible API,
backed by credits, that routes each request to the cheapest viable supply we control.
A customer (human or agent) funds a credit balance (Stripe USD or Bitcoin/Lightning),
points their tool at our base URL + key, and gets inference. We earn the spread between
what we charge and our marginal cost, and — where the request was served by a network
node — we can fan a cut back to that contributor (closing the revenue loop into the
same accepted-outcome → receipt → settle spine the revenue-loop wiring RL-1/2/3 already
built).

**Referral revshare on ALL inference, always.** Beyond the serving-node cut, anyone who
**refers a user, agent, or business that signs up** earns an ongoing referral cut on
**all of that account's inference spend, indefinitely** — not a one-time bounty. Point a
business at us and you keep earning a percentage of everything they ever run through the
gateway. This rides the same referral ledger (RL-1: attribution → eligibility →
dispatch) and pays in credits or Bitcoin per the asset-boundary/no-resale guards (RL-3).
So every dollar of inference can split three ways — OpenAgents margin, the serving node,
and the referrer — turning the whole network into a distribution channel that's paid to
bring demand in.

## 2. Who it's for (the demand)

- **Autopilot app users with no usable subscription** — the desktop app's coding agent
  needs inference. Today it leans on the user's own Claude/Codex auth. Many users won't
  have that (or will run out). The gateway is the app's default inference when the user
  has nothing of their own. **Autopilot is the gateway's primary, anchor buyer** — coding is
  the wedge into the market, and every coding session it runs is captive first-party demand
  for our own inference/compute stack. The other segments below are additive.
- **People using their own tools (Claude Code / Codex) who run out or want one bill** —
  point Claude Code / Codex / any OpenAI-compatible client at our base URL; get one
  metered, top-up-able balance instead of juggling subscriptions.
- **Businesses that *can't* use personal subscriptions** — compliance, per-seat ToS,
  procurement, and "no personal accounts" policies block consumer subscriptions. They
  need a real B2B API with invoicing, an org key, usage reporting, and a contract. This
  is likely the highest-value segment and the cleanest fit for the ~$5K/$60K client
  pipeline in the revenue-engine plan.
- **Agents (machine consumers)** — programmatic inference with programmatic settlement
  (sats or credits), no human in the loop. This is the native OpenAgents customer.

## 3. The supply we already have (verified 2026-06-19)

### a. Google Cloud / Vertex Anthropic (project `openagentsgemini`)
Live quota, shared-lineage `base_model` dimensions (tokens per minute):

| Endpoint | Opus (in / out / rpm) | Sonnet | Haiku | Fable |
| --- | --- | --- | --- | --- |
| **Global** | 40M / 4M / 4,000 | 50M / 5M | 60M / 6M | 20M / 2M |
| **US** | 35M / 3.5M / 3,500 | (lineage exists) | (lineage exists) | (lineage exists) |
| **EU** | 20M / 2M / 2,000 | 25M / 2.5M / 2,500 | 30M / 3M / 3,000 | 10M / 1M / 1,000 |

That is tens of millions of tokens/min of first-party, already-granted capacity across
Opus / Sonnet / Haiku / Fable. (Detail + the quota-correction history is in
`../../../docs/cloud/quotas/2026-06-19-vertex-ai-anthropic-opus-quota-request.md` in the
root repo. Note May-2026 Opus models share one `anthropic-claude-opus` lineage bucket.)

### b. Our agent network (Pylon nodes)
Pylon already runs real coding-agent execution (`sessions exec` drives Claude and Codex
to completion — verified live 2026-06-19, see `docs/launch/2026-06-19-coding-agent-live-verification.md`)
and the Tassadar exact-execution substrate. Network nodes are supply for *task/agentic*
inference (not just token passthrough): a customer's coding task can be routed to a
node that runs it and is paid for the verified outcome.

### c. Partner / passthrough APIs
Direct Anthropic, OpenAI, and others as breadth + burst capacity beyond our Vertex
quota. Lowest margin (pure passthrough) but maximal coverage; useful for models/regions
we don't hold quota for.

## 4. The product — one gateway API

- **Compatibility:** OpenAI Chat Completions-compatible (and Anthropic Messages-compatible)
  so existing clients — Claude Code, Codex, OpenRouter-style SDKs, the Vercel AI SDK —
  work by changing only the base URL + key — zero-rewrite adoption removes friction.
- **Credits + metering:** a funded balance (Stripe USD top-up + Bitcoin/Lightning), priced
  per model per token, decremented per request from real usage. Reuses the openagents.com
  Worker billing/credit ledger and the revenue-loop settlement wiring (RL-1/2/3).
- **Routing:** per-request selection to the cheapest viable supply — our Vertex quota
  first (best margin), then network nodes (for task-style work, with contributor payout),
  then partner passthrough (coverage). Honors fair-share of our shared quota across
  customers + graceful overflow to passthrough on burst.
- **Settlement + invariants:** credit spend → credit accounting; Bitcoin revenue →
  Bitcoin revshare to a serving node; the credit↔Bitcoin asset boundary + no-resale
  guard (RL-3, already live) apply. API-inference resale is the *allowed* lane, so this
  business is invariant-clean by construction.

## 5. Economics (the closing-the-revenue-loop view)

Customer funds $1 of credits → we serve from supply at marginal cost C → we keep the
spread, and if a node served it, a cut fans to that contributor. The "$1 in → >$1 of
value out" promise here is concrete and defensible: a customer with no subscription
*could not otherwise run the work at all*, or runs it cheaper/aggregated/with one bill;
the value out (the completed inference/task) exceeds their dollar, and the margin +
network fan-out is the business. This is the first buy-side that is **already-paid-for
elsewhere** — we capture demand, we don't manufacture it.

## 6. How it fits the existing stack

- **Billing/credits/settlement:** openagents.com Worker (Stripe, credit ledger, RL-1/2/3
  revenue-loop wiring, the asset-boundary + no-resale live guards).
- **Routing/execution:** a new gateway service + a **Vertex adapter** (project
  `openagentsgemini`, the quota above) + Pylon (`sessions exec` for task inference) +
  partner adapters.
- **Autopilot app:** consumes the gateway as default inference when the user has no own
  subscription — directly fixes the "32k views → ~0 self-serve users" gap (a normal
  person/business can pay-as-they-go instead of needing their own Claude/Codex auth).

## 7. Open questions

- **Provider routing is standard gateway practice.** Aggregating hosted model inference
  behind one unified, OpenAI-compatible API — and selling it as credits — is exactly how
  OpenRouter, Together, Fireworks, and other gateways operate. Serving Vertex-hosted and
  partner-hosted models plus our own network through one gateway is that same well-trodden
  pattern; nothing novel here. Operational only: choose endpoints/regions and manage keys.
- **Shared-quota fairness:** our Vertex quota is a shared pool; need per-customer
  rate/fair-share + burst overflow to passthrough so one customer can't starve others.
- **Pricing:** per-model token pricing + margin vs passthrough; credit pack sizing;
  Bitcoin vs USD parity.
- **Abuse / KYC:** a funded-credits API is an abuse target (prompt-injection-for-free-compute,
  payment fraud); need rate limits, spend caps, and light KYC for fiat.
- **Metering accuracy:** token accounting must match provider billing exactly (receipt-first)
  so we never lose money on a mismeter or overcharge a customer.

## 8. Initial next steps (issues to file)

1. **Gateway API skeleton** — OpenAI-compatible `/v1/chat/completions` (+ Anthropic
   Messages) on the openagents.com Worker, key-auth against a credit balance.
2. **Vertex adapter** — server-side Vertex Anthropic calls (project `openagentsgemini`).
3. **Metering + credit decrement** — per-request usage → credit ledger, reusing the
   billing infra; receipt-first accounting.
4. **Routing** — cheapest-viable-supply selection (Vertex → network → passthrough) with
   fair-share + overflow.
5. **Network-served fan-out** — when a node serves a request, pay it via the RL revenue-loop
   spine.
6. **Pricing model + credit packs** — and the Autopilot-app default-inference integration.

> Build order: stand up the compat API + metering + credits first (works across all supply
> lanes), then wire the Vertex and network adapters and routing behind it.
