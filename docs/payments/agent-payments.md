# Agent payments: pay any way (design, 2026-10-09)

**Owner direction (2026-10-09):** support every agent payment protocol, fully.
An agent that finds openagents.com should be able to do whatever it needs:
call the API, use Nostr, and pay in whatever way it already knows. Payments
are routed to the right rail behind one front door.

This doc is the plan for that: which protocols, what "full support" means for
each as a **merchant** (agents pay us) and as a **buyer** (our agents pay
others), the one payment router that sits in front of the metered gateway,
the one receipt model, how every method is advertised, how identity and
budgets fit, the custodial and non-custodial postures, and the phased plan
with issues. It answers [#11085](https://github.com/OpenAgentsInc/openagents/issues/11085)
and adopts the
[non-custodial agent commerce proposal](proposal/README.md) (PR #11088) as the
merchant-settlement profile for third-party sellers.

Status words below: **Built** (on `main`), **Live** (in production),
**Planned** (an issue exists, nothing built).

## The protocols (researched 2026-10-09 from the specs)

| Protocol | Backers, version | Rails | The one request | Discovery agents expect | Merchant: full support | Buyer: full support | Ours today |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [x402](https://github.com/x402-foundation/x402) | x402 Foundation (Linux Foundation; from Coinbase), v2 | Stablecoins on Base, Solana and ~15 more chains; Lightning (`lnbtc`) | `402` + `PAYMENT-REQUIRED`; retry with `PAYMENT-SIGNATURE`; `PAYMENT-RESPONSE` back (v1: `X-PAYMENT`) | `402` itself; Bazaar extension indexed by facilitators (`/discovery/resources`) | `accepts[]` with `lnbtc` and Base/Solana USDC `exact`, `upto` for metered calls, v1 headers too, Bazaar metadata, MCP signaling | Parse v1+v2, pick from `accepts`, sign EIP-3009/Permit2, pay BOLT11, budget check before signing | **Built**: `lnbtc` on the gateway (#11078, not live) and pay front |
| [MPP](https://mpp.dev/) (`Payment` HTTP auth scheme) | Tempo + Stripe; Lightning drafts by Lightspark; `draft-httpauth-payment-01` | Tempo, Stripe SPT/card, Lightning, EVM, Solana, others | `402` + `WWW-Authenticate: Payment id=… method=… intent=charge request=…`; retry `Authorization: Payment <cred>`; `Payment-Receipt` back | `/openapi.json` with `x-service-info` and per-operation `x-payment-info` | `lightning` (same invoice), `tempo`, `stripe`; `session` intent for metered use; problem bodies | Parse several challenges, build each method's credential, honor `digest`/`expires` | **Built** on the pay front (`payment_scheme.rs`, Lightning charge, `lnget`-tested) and on the API through the payment router (#11136, off unless `inference.x402.mpp` is set; not live) |
| [L402](https://github.com/lightninglabs/L402) | Lightning Labs | Lightning | `402` + `WWW-Authenticate: L402 macaroon=…, invoice=…` (also `LSAT`); retry `Authorization: L402 <mac>:<preimage>`; reusable until caveats expire | None standard | Macaroons bound to the payment hash and resource, same invoice and replay key | Pay, cache the token per service | Planned |
| [Cashu](https://github.com/cashubtc/nuts/blob/main/24.md) NUT-24 | Cashu community | Ecash (sats) at named mints | `402` + `X-Cashu: creqA…`; retry `X-Cashu: cashuB…` | None beyond the `402` | Accepted-mint list, swap before serving | Hold ecash at accepted mints | Planned |
| [ACP](https://www.agenticcommerce.dev/) | OpenAI + Stripe, `2026-04-17` | Card and PSP tokens (Stripe Shared Payment Token) | `POST /checkout_sessions` … `/complete` with `payment_data` (SPT) | `/.well-known/acp.json`, product feed | Checkout sessions for credits and Pro; SPT charged through Stripe | Agent platform role (needs a PSP relationship); later | Planned (Stripe built) |
| [UCP](https://ucp.dev/) | Google with Shopify, Walmart, Stripe and others, `2026-08-25` | Payment handlers (Google Pay, Shop Pay, tokenizers, AP2) | `POST /checkout-sessions` … `/complete`, with `UCP-Agent: profile=…` | `/.well-known/ucp` (services, capabilities, handlers, keys) | Profile, checkout over REST and MCP, handlers, AP2 mandate check | Platform profile with keys | Planned |
| [AP2](https://ap2-protocol.org/) | Google, v0.2 (moving to FIDO Alliance) | Cards first; x402 via `a2a-x402` | SD-JWT Checkout and Payment Mandates presented with a purchase | A2A agent card `capabilities.extensions` (AP2 URI, x402 URI) | Sign checkout, verify mandates, return receipts; declare in our agent card | Get user-signed mandates from the phone; open mandates for autonomous spend | Planned |
| Lightning (our node) | — | Lightning | BOLT11 invoice | `lud16` on our Nostr profile | `crates/wallet` receiver, MoneyDevKit LSPS4 | Spark wallet (phone, desktop) | **Built** (pay host) |
| [NWC](https://github.com/nostr-protocol/nips/blob/master/47.md) (NIP-47) | Nostr | Lightning | `pay_invoice` over Nostr | `nostr+walletconnect://` URI | — (wallet transport) | Pay any Lightning challenge from a connected wallet | Named in NIP-X402; planned |
| [Zaps](https://github.com/nostr-protocol/nips/blob/master/57.md) (NIP-57) | Nostr | Lightning | Zap request `9734` to an LNURL callback | `lud16` with `allowsNostr` | Tips to our agents and plugins (social, never a request payment) | Tip | Planned |
| Nutzaps (NIP-60/61) | Nostr | Cashu | Nutzap `9321` | `10019` info event | Accept from our accepted mints | Send | Planned |
| Card (Stripe) | Stripe | Card | Stripe Checkout | — | Pro and credits | MPP `stripe`/SPT later | **Built** (Pro, #11072) |

## The architecture in five lines

1. **One payment router** in front of the metered API answers an unpaid call
   with one `402` that carries a challenge for every method we accept (x402,
   the `Payment` scheme used by MPP, L402, Cashu), all priced from the same
   quote.
2. **Lightning challenges share one invoice.** x402 `lnbtc`, MPP `lightning`,
   and L402 are three encodings of the same BOLT11 invoice, consumed once by
   payment hash in one replay store, so a preimage can never pay twice.
3. **Each rail has one adapter** that verifies and settles: Lightning to our
   node (`crates/wallet`, MoneyDevKit LSPS4 liquidity), stablecoins through an
   x402 facilitator and Tempo, cards and Shared Payment Tokens through Stripe,
   ecash by swapping at the named mint.
4. **Every settled payment becomes one receipt** (`openagents.payment-receipt.v1`)
   in one ledger, whatever the protocol, and the paid request runs through the
   gateway exactly like a keyed request.
5. **Discovery is generated from the live adapters**: OpenAPI, the API
   catalog, the AI catalog, the agent card, the UCP profile, MCP, `llms.txt`,
   and Nostr announcements list a method only while its adapter is on.

## 1. The two roles

**Merchant (agents pay us).** What we sell to agents, in launch order:

| Product | Today | Paid how |
| --- | --- | --- |
| Model calls on the API (`/v1/responses`, `/v1/chat/completions`) | Built; keyed calls metered against credit | A key with credit, or per request with no key |
| Credits on an account | Built (card funding adapter) | Card; later every method below |
| The Pro plan | Built (Stripe subscriptions, #11072) | Card through Stripe Checkout; later ACP/UCP checkout |
| Paid plugins and author-hosted resources | Built on the pay host (`crates/x402` front, #10186, #10194) | x402 and the `Payment` scheme on one invoice |
| Compute (Pylon providers, cloud computers) | Partial | Credits; per request through the router; providers as merchants (§6) |

**Buyer and router (our agents pay others).** An OpenAgents agent, or Coder
on someone's behalf, hits another service's `402` and pays it from the
person's wallet within the budget the person set. Our buyer speaks every
protocol a merchant might answer with, picks the method it can pay that costs
least, and records the same receipt. Over-budget purchases ask the person on
their phone first (§5).

## 2. The payment router (merchant side)

### 2.1 Where it sits

```
agent ──HTTPS──▶ api.openagents.com
                   │
                   ├─ Authorization: Bearer oak_…  ──▶ metered gateway (credit)
                   │
                   └─ no key ──▶ PAYMENT ROUTER
                                   │ quote (rate card, USD micros → per-rail amounts)
                                   │ unpaid → 402 with every live challenge
                                   │ paid   → detect the credential, rail adapter
                                   │          verifies + settles, replay key consumed once
                                   │          → receipt → gateway runs the request
                                   ▼
                         Lightning node │ x402 facilitator / Tempo │ Stripe │ Cashu mint
```

The router is a library in front of the gateway's existing keyless path
(`crates/gateway/src/inference_x402.rs`), built from `crates/x402`. The pay
front already serves x402 and the `Payment` scheme on one invoice
(`crates/x402/src/front.rs`, `payment_scheme.rs`, #10186), with a tested
`lnget` compatibility fixture; the router generalizes that to every method
and puts it in front of the API, plugins, and compute alike.

### 2.2 One unpaid answer, every challenge

An unpaid call gets one `402 Payment Required`. HTTP allows several
challenges in one response, so the router sends all live ones together:

| Header | Protocol | Carries |
| --- | --- | --- |
| `PAYMENT-REQUIRED` | x402 v2 | `accepts[]`: `exact` on `lnbtc` (our invoice), `exact` and `upto` on Base USDC, `exact` on Solana USDC, and other networks a facilitator supports; `extensions.bazaar` |
| `WWW-Authenticate: Payment …` | MPP (IETF `Payment` scheme) | One challenge per method: `lightning` (the same invoice), `tempo`, `stripe` (card via Shared Payment Token) |
| `WWW-Authenticate: L402 macaroon="…", invoice="…"` (and `LSAT`) | L402 | The same invoice, with a macaroon bound to its payment hash and this request |
| `X-Cashu: creqA…` | Cashu NUT-24 | A NUT-18 payment request naming our accepted mints and the amount in sats |
| JSON body | all | Price in sats and USD, the methods, and a link to `/docs/api/for-agents` |

Every challenge comes from one quote. The quote is in USD micros from the
rate card and is converted once per rail (sats at `inference.sats_rate`,
USDC at par, card in cents with the card minimum handled by credits, below).
A quote expires with its invoice (default 120 s).

### 2.3 The paid retry

The router reads whichever credential arrives (`PAYMENT-SIGNATURE` or
v1 `X-PAYMENT`, `Authorization: Payment`, `Authorization: L402`/`LSAT`,
`X-Cashu: cashuB…`), hands it to that
rail's adapter, and admits the request only after settlement:

| Rail | Verify and settle | Replay key |
| --- | --- | --- |
| Lightning (x402 `lnbtc`, MPP `lightning`, L402) | Preimage hashes to the invoice's payment hash; invoice is ours, unexpired, exact amount, description hash binds the request; our node shows it received | `lnbtc:<network>:<payment_hash>` (shared by all three encodings) |
| x402 EVM / Solana | Facilitator `/verify` then `/settle` (EIP-3009 `transferWithAuthorization` on Base USDC; SPL transfer on Solana). Hosted facilitator first; self-hosted later | `<caip2>:<tx or authorization nonce>` |
| MPP `tempo` | Tempo transaction or Stripe's MPP verification for our account | `tempo:<tx>` |
| MPP `stripe`, ACP | Charge the Shared Payment Token through Stripe (`PaymentIntent` with `shared_payment_granted_token`) | `stripe:<payment_intent>` |
| Cashu | Swap the proofs at the named mint (NUT-03) before running; refuse mints we don't accept | `cashu:<mint>:<Y of each proof>` |

Settle before execute, one replay store per receiver, release on "no answer
at all": the rules in [NIP-X402](../../nips/openagents/NIP-X402.md) and
`inference_x402.rs` apply to every rail, not only Lightning.

### 2.4 Small amounts and cards

Model calls cost fractions of a cent. Lightning, USDC on Base or Solana, Tempo,
and Cashu handle that directly. Cards don't: a card charge has a floor. Card
and ACP payments therefore buy **credit** (a top-up, or the Pro plan), and
the credit pays per call. MPP `stripe` on a single call is offered only when
the quote is above the card floor; otherwise its challenge points at a
top-up. MPP sessions (pay once, draw down over many calls) map to the same
credit balance.

x402's `upto` scheme (authorize a ceiling, settle the actual cost) fixes
the one rough edge of per-request Lightning today: we charge the quoted
worst case and keep the unused part. On stablecoin rails we use `upto` and
settle the answer's real cost; on Lightning the worst case stays until an
MPP Lightning session or credit is used.

### 2.5 Accounts are a payment method too

A key (`oak_…`) with credit stays the cheapest path for heavy users: actual
cost per call, no per-request challenge. Credit can be bought with every
method above. A per-request payer can also attach a Nostr identity (§5) so
its purchases show up as one buyer.

## 3. One receipt and one ledger

Every settled payment, any protocol, writes one record:

```jsonc
{
  "v": "openagents.payment-receipt.v1",
  "id": "pr_…",
  "protocol": "x402 | mpp | l402 | cashu | acp | ucp | ap2 | stripe-checkout | zap",
  "rail": "lightning | evm | solana | tempo | card | ecash",
  "network": "lnbtc:000000000019d6689c085ae165831e93 | eip155:8453 | …",
  "asset": "BTC | USDC | USD | sat-ecash",
  "amount": "3000",            // smallest unit of `asset`, decimal string
  "usd_micros": 2100,          // the quote in USD, fixed at challenge time
  "replay_key": "lnbtc:…:<payment_hash>",
  "request_hash": "<sha256 of the bound request>",
  "resource": "POST /v1/responses",
  "payer": { "account": null, "nostr": "<x-only pubkey>|null", "mandate": "<AP2 ref>|null" },
  "settled_at": 1791590400,
  "outcome": "served | refunded | failed_before_answer"
}
```

- The protocol's own receipt goes back as the protocol expects
  (`PAYMENT-RESPONSE`, `Payment-Receipt`; an L402 token stays reusable until
  its caveats run out, so one receipt can cover several calls; Cashu has
  none), plus `x-openagents-receipt: pr_…` on every paid answer.
- Receipts land in the existing ledger (`crates/pay-ledger`) next to plugin
  and payout records, so `/stats` and the money flow view count every
  protocol. Bearer secrets (preimages, tokens, proofs) never go in a receipt,
  a log, or a trace.
- USD is the accounting unit, as on the rate card today; each receipt keeps
  the asset amount actually received.

## 4. Discovery: advertise every method, only while it works

The rule from #11085 stands: **nothing is advertised until the path behind
it works.** Each surface is generated from the router's live adapter list,
the way `/v1/openapi.json` already adds x402 only when the gateway has it
(`inference_openapi.rs`).

| Surface | What it says |
| --- | --- |
| `402` responses | Every live challenge (§2.2) |
| `/v1/openapi.json`, `/openapi.json` | MPP discovery: `x-service-info` at the top and `x-payment-info.offers` on every paid operation; a security scheme per method (`x402`, `payment`, `l402`, `cashu`) and the `402` headers |
| `/.well-known/api-catalog` (RFC 9727) | The API plus links to the payment docs and receipt format |
| `/.well-known/ai-catalog.json` | The API, the MCP servers, the agent card, and the accepted payment protocols |
| `/.well-known/agent-card.json` (A2A) | The AP2 extension and the x402 extension for A2A, with accepted mandates and methods |
| `/.well-known/ucp` | UCP profile: checkout and order capabilities for credits and Pro, payment handlers |
| `/.well-known/acp.json` (ACP) | API base, versions, and services; the product feed and checkout session endpoints for credits and Pro |
| x402 Bazaar | `extensions.bazaar` (input and output schema) in our `402`, so facilitators list our paid routes |
| MCP (`/mcp/docs`, and paid MCP tools later) | x402 MCP payment signaling (`_meta["x402/payment"]`) on paid tools |
| `llms.txt`, `/auth.md`, `/docs/api/for-agents` | Plain words: how to get a key, how to pay per request, each method's one request |
| Nostr | A NIP-89 handler and a NIP-MKT offering (`3192`/`30192`) with the CAP `oa-x402-v1` descriptor; our profile's `lud16` for zaps; a `10019` nutzap info event once Cashu is on |

## 5. Identity, budgets, and approvals

Paying and being known are separate. A per-request payer needs no account.
An agent that wants to be recognized, get a better rate, or prove it is
spending its owner's money can say who it is three ways:

| Identity | How | Status |
| --- | --- | --- |
| API key | `Authorization: Bearer oak_…` (a person makes it at Settings → API keys) | Built |
| OAuth 2.1 | For MCP and agents acting for a signed-in person | Planned (#11084) |
| Nostr key | NIP-98 signed request (`Authorization: Nostr …`); the pay front already checks NIP-98 for author registration | Built on the pay host; planned on the API |
| Owner-backed budget | `BuyerAttestation`: a NIP-SOV `sovereign-profile.v1` naming the owner, plus a NIP-CAP `grant.v1` with the `spend` effect and its ceilings | Proposed in PR #11088; its fixtures are our starting test vectors |
| Payment mandate | AP2 v0.2 Checkout and Payment Mandates (SD-JWT credentials) for purchases made for a person | Planned |

The buyer side uses the same pieces in reverse: our agent's spending is
bounded by a CAP `spend` grant and the SOV treasury ceilings; under the
ceiling it pays on its own, above it the person approves on their phone
(NIP-POL, single use), and listed operations can require more than one
approver. The phone approval reuses the device sign-in approval screens
(#11045). See
[agent ownership and mobile approval](proposal/2026-10-09-agent-ownership-and-mobile-approval.md).

## 6. Custodial and non-custodial, side by side

PR #11088's posture is adopted: **coexistence**.

- **First-party sales** (the API, credits, Pro, plugins sold through the pay
  front): we are the merchant, so one receiver of ours is correct and
  compliant. Lightning lands on our node; stablecoins at our receiving
  address; cards at Stripe. Splits and payouts stay in `pay-ledger`
  ([central receive and splits](2026-10-02-central-receive-and-splits.md)).
- **Third-party merchants** (Pylon compute providers first, then API and data
  sellers): the [merchant-hosted receiver profile](proposal/2026-10-09-x402-merchant-receiver-profile.md).
  The merchant's own node is `payTo`; we take no custody. Order, per the
  contributor: compute providers self-host (topology 4A) first; then we
  offer inbound liquidity as an LSP (4C) so non-crypto API and data sellers
  can join. The managed forwarder (4B) waits for a custody review. A
  platform fee, if any, is a separate requirement, never inside the
  merchant's invoice, and is gated on the NIP-X402 spec review.
- The router is the same code in both postures; only who owns the receiver
  and replay store changes.

## 7. How each protocol fits NIP-X402

NIP-X402 says Cashu and L402 are not implicit fallbacks for x402 and that
"protocol conversions require a separate reviewed profile." That stays true,
and it is how we support them: each is its **own adapter and its own
challenge**, never a disguised x402 payment. L402 and MPP `lightning` are
separate encodings of the same invoice and consume the same replay key, as
`payment_scheme.rs` already does for MPP. Cashu proofs carry mint trust, so
the Cashu adapter names the mints it accepts and swaps proofs before
running. Zaps stay social: a zap never pays for a request, and an x402
invoice is never shown as a zap. Each new adapter gets a short profile
section in NIP-X402's "Wallets" part when it lands.

## 8. Plan

**By Monday 2026-10-12 (launch):**

- x402 on Lightning live on the API ([#11077](https://github.com/OpenAgentsInc/openagents/issues/11077)): built, needs the receiver configured on production and a deploy.
- The `Payment` scheme (MPP `lightning`) on the API's `402`, sharing the x402 invoice: reuse the pay front's encoder (#11136). **Built**: `crates/x402/src/router.rs` (the `Adapter` trait, x402 and MPP adapters, one invoice, one replay store) in front of `crates/gateway/src/inference_x402.rs`; on in production once the owner sets `inference.x402` (and `mpp`) and deploys.
- Discovery lists exactly what is live (OpenAPI, API catalog, AI catalog, agent card, docs MCP, `llms.txt`, `/auth.md`, the For agents table, and the x402 Bazaar extension in the `402`) (#11137). **Built**: every surface reads the router's list through `/v1/openapi.json` (`x-openagents-payment-methods`).
- A minimal `openagents.payment-receipt.v1` record per served paid request (#11138): **Built** as one JSON file per receipt (`crates/x402/src/receipt.rs`, schema `nips/openagents/schemas/payment-receipt.v1.json`) under the gateway's registry; moving it into `pay-ledger` and counting it on `/stats` remain.
- [For agents](../../crates/openagents-web/content/docs/api/for-agents.md) on the website: how an agent finds us, signs in, and pays. Done in this change.
- Card for Pro and credits (Stripe), built; in launch copy only if the owner keeps it there.

**Next (the week after launch):** the receipt model (#11138); L402; Cashu NUT-24 with a short accepted-mint list; the buyer
side in `openagents pay` and Coder (x402, MPP, L402, Cashu, from the
person's wallet, within budget); NIP-98 identity on the API.

**Then:** stablecoin rails (x402 on Base and Solana via a facilitator, MPP
`tempo`), MPP `stripe` and ACP checkout for credits and Pro, the UCP
profile, AP2 mandates in the agent card, NWC as a buyer wallet, zaps on our
Nostr profile, nutzaps, and the third-party merchant profile from PR #11088
(Pylon providers first).

**Owner decisions** (each named in its issue):

1. Stablecoin receiving: which account receives USDC (a Coinbase CDP or
   Stripe stablecoin account, or our own address), and whether it converts
   to dollars on receipt. This changes the Kitchen Sink promise "Bitcoin is
   the only money" (I8) to "no token; Bitcoin is our own money, and agents
   may also pay in dollars by card or stablecoin."
2. Accepted Cashu mints.
3. Whether MPP `stripe` and ACP need a separate Stripe account setting
   (Shared Payment Tokens, agentic commerce enablement).

## 9. Issues

| Issue | What | When |
| --- | --- | --- |
| [#11077](https://github.com/OpenAgentsInc/openagents/issues/11077) | x402 on Lightning live on the API (built; configure and deploy) | By Monday (on the V1 board) |
| [#11136](https://github.com/OpenAgentsInc/openagents/issues/11136) | Payment router: one 402 with every live challenge, one replay store, in front of the API | By Monday (on the V1 board) |
| [#11137](https://github.com/OpenAgentsInc/openagents/issues/11137) | Discovery: advertise every live payment method (OpenAPI x-payment-info, API catalog, AI catalog, llms.txt, auth.md, Bazaar) | By Monday (on the V1 board) |
| [#11138](https://github.com/OpenAgentsInc/openagents/issues/11138) | Payment receipts: openagents.payment-receipt.v1 for every protocol, in pay-ledger | Next |
| [#11139](https://github.com/OpenAgentsInc/openagents/issues/11139) | L402 on the API: same invoice, same replay key | Next |
| [#11140](https://github.com/OpenAgentsInc/openagents/issues/11140) | Cashu NUT-24 on the API: X-Cashu payment requests and tokens | Next |
| [#11141](https://github.com/OpenAgentsInc/openagents/issues/11141) | x402 stablecoin rails: Base and Solana USDC (exact and upto) via a facilitator | Then |
| [#11142](https://github.com/OpenAgentsInc/openagents/issues/11142) | MPP tempo and stripe methods (and sessions) on the payment router | Then |
| [#11143](https://github.com/OpenAgentsInc/openagents/issues/11143) | ACP: checkout sessions for credits and Pro, /.well-known/acp.json | Then |
| [#11144](https://github.com/OpenAgentsInc/openagents/issues/11144) | UCP: /.well-known/ucp profile and checkout for credits and Pro | Then |
| [#11145](https://github.com/OpenAgentsInc/openagents/issues/11145) | AP2: mandates and the agent card extension | Then |
| [#11146](https://github.com/OpenAgentsInc/openagents/issues/11146) | Buyer: openagents pay and Coder pay any 402 (x402, MPP, L402, Cashu) within the person's budget | Next |
| [#11147](https://github.com/OpenAgentsInc/openagents/issues/11147) | Nostr payments: NWC wallet, zaps, nutzaps, and NIP-MKT/NIP-89 announcements of our paid routes | Then |
| [#11148](https://github.com/OpenAgentsInc/openagents/issues/11148) | Nostr identity on the API: NIP-98 signed requests | Next |
| [#11149](https://github.com/OpenAgentsInc/openagents/issues/11149) | Third-party merchants: merchant-hosted x402 receivers (Pylon providers first) and the BuyerAttestation verifier | Then |

Tracking issue: [#11085](https://github.com/OpenAgentsInc/openagents/issues/11085).

## Related

- [Non-custodial agent commerce proposal](proposal/README.md) (PR #11088)
- [Central receive, splits, and payouts](2026-10-02-central-receive-and-splits.md)
- [NIP-X402](../../nips/openagents/NIP-X402.md), [NIP-SOV](../../nips/openagents/NIP-SOV.md), [NIP-CAP](../../nips/openagents/NIP-CAP.md), [NIP-POL](../../nips/openagents/NIP-POL.md), [NIP-MKT](../../nips/openagents/NIP-MKT.md)
- [The OpenAgents API](../api/README.md), [inference gateway](../inference/gateway.md)
- [Kitchen Sink ledger](../kitchen-sink/ledger.md), section I and K6
