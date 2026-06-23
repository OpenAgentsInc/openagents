# Khala on Machine Payments (MPP) + Stripe Directory — full integration plan

*2026-06-22. How to make the OpenAgents **Khala** inference API discoverable in the
Stripe Directory and payable **per call** by agents via the Machine Payments
Protocol (MPP) — and how that fits (and conflicts) with our Bitcoin-first stance.
Grounded in the live Stripe docs (read via the Stripe MCP) and the survey in
[`2026-06-22-stripe-directory-mpp-khala.md`](./2026-06-22-stripe-directory-mpp-khala.md).*

## Objective

An agent searches `stripe directory search "llm inference api"` → finds **Khala** →
calls our OpenAI-compatible endpoint and **pays per request** with `mppx fetch`
(no signup, no API key). Today **zero** inference providers in the directory accept
MPP, so this is a first-mover slot. Later: expose other Agent Cloud primitives
(fine-tuning, sandboxes, agentic compute) the same way.

## How MPP actually works (authoritative)

Machine payments let agents pay for individual HTTP requests. The mechanism is a
**`402 Payment Required` paywall middleware**:

1. Agent calls the endpoint with **no payment credential** → server returns
   **`402 Payment Required`** with one or more **`WWW-Authenticate` challenges**
   (the price + how to pay).
2. The agent's tool (**`mppx`**) pays and **retries** with a payment credential in
   the request.
3. Server **verifies** the credential and **serves** the response (the completion).

- SDK: **`Mppx.create(...)`** per payment method + **`Mppx.compose(...)`** to accept
  several at once (TypeScript); `Mpp.create` (Python). Stripe API version
  `2026-03-04.preview`. Starter: `github.com/stripe-samples/machine-payments`.
- **Payment rails MPP supports** (this is the crux for us):
  | Network | Protocol | Currency | Settles to |
  |---|---|---|---|
  | Base | x402 | USDC | (on-chain / Stripe) |
  | Solana | MPP | USDC | Stripe balance → fiat |
  | Tempo | MPP | USDC | Stripe balance → fiat |
  | Stripe card networks | MPP (via Shared Payment Tokens) | cards/Link | Stripe balance → fiat |
- **Microtransactions** down to **0.01 USDC**; payments land in the **Stripe
  balance and settle in fiat**; refunds supported. Stablecoin payins need
  "crypto payins" enabled; card/SPT needs a **US legal entity**.

## The rails decision (read this first — it's the real question)

**MPP money is USDC or card → a Stripe balance → fiat. It is NOT Bitcoin/Lightning.**
That directly meets the workspace's standing **"Bitcoin-only, no Stripe for the
credits business"** stance. So MPP is **not** a drop-in for our Spark/Lightning rail —
it's a *different, Stripe-mediated inbound rail*. Two things to separate:

- **Discovery (cheap, low-commitment, do regardless):** being *listed* in the Stripe
  Directory + publishing agent-discovery surfaces (`llms.txt`, `/agents`) costs us
  nothing in payment-rail terms and makes Khala findable. **Recommended unconditionally.**
- **Accepting MPP payments (the actual decision):** turning on a 402-paywalled Khala
  endpoint means **accepting USDC/card into a Stripe balance** — a second money rail
  alongside Bitcoin/Spark. This is an **explicit owner call** because it reintroduces
  Stripe to the money path we deliberately kept Bitcoin-only.

**Spectrum, least→most Stripe-coupled** (pick where to sit):
1. **x402 on Base (USDC)** — most crypto-native MPP option, least Stripe coupling.
2. **MPP USDC (Solana/Tempo) → Stripe balance** — Stripe custodies/settles.
3. **MPP card via SPT → Stripe balance** — fully Stripe, needs US entity.
4. *(our native rail, NOT an MPP option:* **Bitcoin/Spark** *)*.

**Recommendation:** ship **discovery now** (Phase 1) unconditionally; for payments,
default to **x402/USDC** (closest to our crypto-native ethos) as the agent-pay rail,
treat an MPP USDC charge as a **new "credits-in" path** that mints Khala credits and
then runs the *existing* metering + receipt + **Bitcoin contributor-payout** loop
underneath — so the inbound rail is additive reach while **Bitcoin/Spark stays the
settlement + payout rail**. Whether to also accept Stripe-custodied USDC/card is the
owner decision.

## Architecture against the live gateway

Khala is already live: `https://openagents.com/v1/chat/completions`
(`openagents/khala-mini`, `openagents/khala-code`), on the Cloudflare Worker, with
metering + receipts + the Bitcoin payout loop. The MPP work wraps that.

### Phase 0 — prerequisites (owner / Dashboard)
- **Create the OpenAgents public Stripe profile** (Dashboard → *Stripe profile* →
  Get started → display name + handle → fill description → keep **not private**).
  This is *required* to be listed at all (`stripe directory me` currently returns
  "Your account does not have a Stripe profile"). Capture the **`profile_…` network
  profile ID** (needed for SPT). Live account is **OpenAgents, Inc.** (`acct_1Ln7jh…`).
- If accepting payments: enable **crypto payins** on the account (USDC); for card-SPT,
  confirm US-entity eligibility.

### Phase 1 — discovery surfaces (cheap, ship first, no payment commitment)
Publish on `openagents.com` (the Worker), following the live PostalForm/Zinc pattern:
- **`/llms.txt`** — plain-language description ("OpenAI-compatible LLM inference API,
  pay-per-call; models `openagents/khala-*`; verified coding outcomes"), best-fit/
  not-a-fit, key facts, pricing pointer, and links to the agent surfaces below.
- **`/agents.md`** (or `/agents`) — machine-payment workflows + how to call Khala.
- **`/ai.md`** + **`/skill.md`** — AI instructions + MCP/skill setup (optional, mirrors
  PostalForm).
- Ensure **StripeBot can crawl** the site (don't block the crawler) so the directory
  matches us on "inference"/"llm api" searches. Write the profile description in the
  words agents type ("pay-per-call OpenAI-compatible inference"), list all capabilities.

### Phase 2 — the paid MPP/x402 endpoint
A **402-gated Khala endpoint** (e.g. `https://openagents.com/mpp/v1/chat/completions`,
or the existing route behind an MPP flag). Using `Mppx.compose` (or x402 middleware):
- No credential → **`402`** + `WWW-Authenticate` challenge(s): price per request
  (map to our per-token pricing; can quote a flat per-call or estimate-then-settle).
- Valid credential → **verify**, run the Khala completion (the same gateway path),
  **return it** + the `openagents` receipt block.
- **Runtime note:** the `Mppx`/`Mpp` SDKs are Node; the gateway is **Cloudflare
  Workers**. Either implement the 402-challenge/verify logic Worker-native (it's a
  small protocol — 402 + `WWW-Authenticate` + credential verify) or run a thin Node
  **MPP sidecar** (Cloud Run / a Pylon) that fronts the Worker. Decide during build.
- **Pricing:** Khala is paid; per-call price derives from the existing pricing-model
  doc ($0.01/credit basis, per-token). Microtransactions to 0.01 USDC are supported.

### Phase 3 — unify with the credits + Bitcoin loop
Map a settled MPP payment → **mint Khala credits** (or a directly-metered completion)
→ the **existing receipt + settlement + Bitcoin contributor-payout** flow. Result:
**one balance, two inbound rails** (Bitcoin/Spark *and* MPP-USDC/card), with the
**outbound contributor payout staying Bitcoin/Spark**. This keeps MPP buyers and
Bitcoin buyers in the same metering/verified-work economy.

### Phase 4 — other primitives on MPP
Once the Khala pattern works, expose fine-tuning, sandboxes/agentic compute, tasks,
and data as their own MPP endpoints + directory entries (the "Agent Cloud, pay-per-
call" story).

## Decisions / NEEDS-OWNER

1. **Do we accept Stripe-mediated USDC/card at all?** (vs. Bitcoin-only.) The central
   call — it reintroduces Stripe to the money path. Discovery (Phase 1) does **not**
   require answering this; the paid endpoint (Phase 2) does.
2. **Which pay rail** if yes: x402/USDC (recommended, crypto-native), MPP USDC via
   Stripe, and/or card-SPT (US entity).
3. **Stripe profile** creation + crypto-payins enablement (owner, Dashboard).
4. **Per-call pricing** mapping from our per-token model.
5. **Where the MPP middleware runs** (Worker-native vs Node sidecar).

## Next steps (sequenced)

- [ ] **(owner)** Create the OpenAgents public Stripe profile; capture `profile_…`.
- [ ] **(us, no decision needed)** Ship Phase-1 discovery surfaces (`/llms.txt`,
      `/agents.md`) on the Worker + confirm StripeBot can crawl → get listed.
- [ ] **(owner decision)** Approve a pay rail (x402/USDC recommended) and whether to
      accept Stripe-custodied funds.
- [ ] **(us)** Build the 402-gated Khala endpoint (Worker-native or Node sidecar) +
      per-call pricing; wire settled payment → Khala credits → existing loop.
- [ ] **(verify)** `stripe directory search "llm inference api" --mpp-supported` shows
      **Khala** (and we're the first inference result); `mppx fetch <endpoint>` pays +
      returns a completion end-to-end.

## References
- Machine payments overview — <https://docs.stripe.com/payments/machine>
- MPP quickstart (402 middleware, `Mppx.compose`, rails) — <https://docs.stripe.com/payments/machine/mpp/quickstart>
- x402 — <https://docs.stripe.com/payments/machine/x402>
- Stripe profiles (directory prerequisite) — <https://docs.stripe.com/get-started/account/profile>
- Stripe Directory — <https://docs.stripe.com/directory.md> · MPP — <https://mpp.dev> · `mppx` — npm `mppx`
- StripeBot crawler — <https://docs.stripe.com/stripebot-crawler.md>
- Live example pattern (PostalForm): `/llms.txt`, `/agents.md`, `/mpp.md`, `/skill.md`, `/ai.md`
- Our pricing basis — `docs/inference/2026-06-19-pricing-model.md`; revenue loop — EPIC #5457
