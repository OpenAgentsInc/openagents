# OpenAgents product surface MDK Setup Audit

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`

Source follow-up:
`docs/mdk/2026-06-07-moneydevkit-local-source-audit.md` records the local
MoneyDevKit source audit across `projects/moneydevkit/repos`. The main result
is that OpenAgents product surface should port the current `mdk-checkout` core contracts into Effect
services, but must not import MDK's native `@moneydevkit/lightning-js` node
runtime into Cloudflare Worker code.

Production follow-up:
On 2026-06-07, OpenAgents product surface proved that this can be done on Cloudflare without
returning to the old Google Cloud Nexus plan. The live path is:

```text
openagents.com Worker
-> MDK_SIDECAR Durable Object binding
-> Cloudflare Container running services/mdk-sidecar
-> @moneydevkit/core route handlers
-> MDK hosted platform
```

The Worker remains the typed Site commerce, policy, redaction, receipt, and
public projection authority. The Cloudflare Container exists only because the
MDK checkout core needs a Node/native-capable runtime that a plain Worker
cannot provide. This is not a reason to move Nexus or Site commerce back to
GCP.

## Summary

OpenAgents product surface does not have the stock MoneyDevKit Next.js integration installed.
There is no Next.js `app/api/mdk/route.ts`. OpenAgents product surface now does have a first-party
`/api/mdk` Worker route that proxies to the Cloudflare Container MDK sidecar.
The sidecar is intentionally isolated from Site source generation and public
browser JavaScript.

OpenAgents product surface does have a custom Effect/Cloudflare Worker port of the parts of MDK
that matter for OpenAgents:

- checkout input modeling, metadata validation, customer normalization, clean
  checkout paths, and signed checkout URL semantics;
- hosted MDK checkout request/response contracts;
- Site commerce discovery, checkout-intent, L402 challenge, and L402 redemption
  route contracts;
- fake hosted MDK provider behavior for contract tests;
- Site MDK reconciliation/webhook verification, replay-safe reconciliation,
  checkout return, receipt, and entitlement projection contracts;
- buyer payment entitlement policy decisions for one-shot, time-window, quota,
  resource, actor, route, Site, and hybrid scopes;
- a unified payment decision contract that composes free-beta allowance,
  internal credits, Stripe top-up state, L402/MDK proof, existing entitlements,
  manual review, hard blocks, and unavailable provider state;
- an MDK agent-wallet payout adapter boundary behind Treasury payment
  authority;
- a typed payment destination input classifier, based on the local
  MoneyDevKit `bitcoin-payment-instructions` source audit, for BOLT11, BOLT12,
  LNURL, Lightning Address, BIP353-style names, `bitcoin:` URI payloads,
  unsupported, malformed, and ambiguous states without projecting raw payment
  strings; and
- a typed self-hosted `mdkd` sidecar option and runbook for route kind,
  auth-tier separation, health/readiness, storage/VSS refs, webhook source
  selection, emergency pause, and redacted observability without importing
  native MDK runtime into the Worker.

Current status: the repo is contract-ready in several places, but the
production hosted MDK checkout path is now live at the MDK route boundary
through Cloudflare Containers. The current Site commerce routes still keep the
fake provider for tests and fixtures and can return explicit
missing-configuration state when live MDK route config is absent. With the
Cloudflare sidecar configured, they can create provider checkout refs and can
persist checkout intent/provider refs, verified status transitions,
reconciliation events, receipts, and entitlements to D1 when the Worker
bindings and webhook config are available.

## Cloudflare Container Smoke Evidence

On 2026-06-07, after provisioning a corrected MDK app binding for
`https://openagents.com`, uploading the MDK secrets as Cloudflare Worker
secrets, and rotating the named container instance, production showed:

| Check | Result |
| --- | --- |
| Signed MDK core ping through `https://openagents.com/api/mdk` | HTTP `200`, body `OK` |
| Live amount checkout creation | HTTP `200`; created a production `SAT` checkout for `100` bitcoin sats |
| Checkout invoice payment | Local MDK agent wallet send returned exit status `0` |
| Merchant checkout status after payment | `PAYMENT_RECEIVED` |
| Payer wallet balance delta | Decreased by `100` bitcoin sats |

Private material retained only in local ignored/temp locations:

- raw MDK access token;
- MDK mnemonic;
- raw checkout client secret;
- raw BOLT11 invoice;
- payment hash;
- preimage; and
- raw provider checkout payload.

Public-safe conclusion: Cloudflare Workers plus Cloudflare Containers is a
valid production runtime for the MDK checkout sidecar. Plain Workers alone are
not enough for the current MDK checkout core because the checkout route depends
on Node/native behavior, but that does not require Google Cloud.

## Account State

Authenticated MDK MCP account inspection found:

| Resource | Status |
| --- | --- |
| App | One app: `OpenAgents product surface`, domain `https://openagents.com` |
| App usage | `0`, last used `Never` |
| Products | None |
| Checkouts | None |
| Orders | None |

The API key was visible only as an MCP redacted preview. No raw API key or
mnemonic was copied into this audit.

Later on 2026-06-07, a corrected MDK device authorization flow provisioned a
fresh app binding for `https://openagents.com` because the previous app binding
had been created with `https://openagents.com/api/mdk`, causing MDK to call
`https://openagents.com/api/mdk/api/mdk`. The corrected binding is the one used
by the successful Cloudflare smoke above.

## Demo Product, Payment, And Payout Readiness

This is the concrete state for a demo that creates a product, collects a
payment, and tests a payout through OpenAgents product surface's custom Effect/Worker MDK port.

### Create A Demo Product

Already built:

- The MDK account has an `OpenAgents product surface` app, but it has no MDK products.
- OpenAgents product surface has Site payment manifest and product/action schema foundations from
  #163, #298, and #299.
- OpenAgents product surface has a hosted Site checkout-intent route contract from #300.
- OpenAgents product surface has generated checkout UI primitives from #301, but those primitives
  are not currently wired to a live MDK-backed buyer checkout.
- #433 added a non-secret OpenAgents product surface demo amount-checkout catalog mapping for
  `site_openagents_mdk_demo`, version `version_openagents_mdk_demo_v1`, product
  `openagents_demo_checkout`, checkout path `/checkout/openagents-demo`, and public-safe
  provider config ref `mdk_amount_checkout.openagents_demo_checkout.usd_100`.
  The mapping is implemented in `workers/api/src/site-mdk-demo-product.ts` and
  injected into the Site commerce route catalog.
- #435 added the public buyer-facing demo product surface at
  `/sites/demo-checkout`, with clean return pages at
  `/sites/demo-checkout/{success,cancel,status}`. The page reads public Site
  commerce discovery, creates checkout intents with an `Idempotency-Key`, sends
  only public catalog/price/customer-data refs, opens the hosted checkout when
  a live provider is configured, and otherwise shows an explicit non-live
  provider state.

Still required:

- Create a real MDK dashboard product if the demo should use MDK product-mode
  checkout instead of OpenAgents product surface's current amount-checkout mapping. During #433,
  the authenticated MCP `create_product` tool rejected valid-looking fixed and
  custom product payloads with input validation errors, so the repo mapping
  intentionally uses the amount-checkout lane until a dashboard product ID is
  available.
- Wire the public demo product surface all the way through the now-live
  Cloudflare sidecar instead of relying on the direct `/api/mdk` smoke script.

Issue coverage:

- Closed Site commerce issues cover the product/catalog/checkout contracts.
- Closed Nexus/Pylon issues #428-#432 cover payout dispatch, visibility,
  Forum publication, real movement evidence, and the release gate; they are not
  demo product purchase flows.
- #433 covers the repo-side demo catalog mapping and discovery/checkout
  selection tests. #435 covers the buyer-facing checkout UI. Live hosted MDK
  provider runtime remains dependent on the configured sidecar/platform route.

### Collect A Payment

Already built:

- OpenAgents product surface owns the custom Site commerce endpoints under
  `/api/sites/:siteId/commerce/...`; it does not use MDK's Next.js
  `/api/mdk` endpoint.
- The checkout-intent API validates catalog membership, price, customer-data
  refs, clean return paths, and idempotency.
- `OpenAgentsHostedMdkClient` defines the provider boundary, fake checkout
  refs, redacted public projections, and error classifications.
- `OpenAgentsHostedMdkClient` now also has a Worker-safe MDK core route client
  for `create_checkout` and `get_checkout` against a Node-capable sidecar or
  equivalent MDK-compatible route. It posts the same `handler`-selected route
  shape found in local `mdk-checkout`, uses server-only route credentials, maps
  MDK statuses into OpenAgents product surface statuses, and redacts raw invoices/payment hashes.
- Worker config now has explicit hosted checkout route bindings:
  `MDK_CHECKOUT_ROUTE_URL`, optional `MDK_CHECKOUT_ROUTE_SECRET` (falling back
  to `MDK_ACCESS_TOKEN`), `MDK_CHECKOUT_ROUTE_KIND`,
  `MDK_CHECKOUT_PATH_BASE`, and public-safe provider refs. The route kind is
  `fake_provider`, `hosted_platform`, or `self_hosted_mdkd_sidecar`. The
  Worker still must not import `@moneydevkit/lightning-js`.
- `makeSiteCommerceRoutes` can persist the buyer payment challenge and Site MDK
  checkout intent/provider checkout ref through
  `site_mdk_checkout_intents`, backed by migration
  `workers/api/migrations/0124_site_mdk_checkout_intents.sql`.
- Site commerce routes now include clean checkout return handling at
  `GET /api/sites/:siteId/commerce/checkout-returns/:checkoutIntentRef/:returnAction`.
  Returns read durable checkout, receipt, and entitlement state without using
  checkout query strings.
- Site commerce routes now include MDK webhook reconciliation at
  `POST /api/sites/:siteId/commerce/mdk/webhooks`. The route verifies the
  configured exact source family before mutating state: dashboard Standard
  Webhooks, daemon invoice HMAC, or SDK node-control secret header.
- Verified payment callbacks update checkout status, write replay-safe buyer
  payment reconciliation events, and create receipt/entitlement records. Raw
  invoices, payment hashes, preimages, wallet state, MDK credentials, webhook
  secrets, customer private data, and provider payout claims are not projected.
- #457 now proves the generated `site_payment_smoke` fixture across this same
  boundary with deterministic dashboard Standard Webhooks evidence: checkout
  creation and browser return remain unpaid, a signed payment-received event
  creates one receipt, one entitlement, and one reconciliation event, replay is
  idempotent, and payment proof projects `verified_entitlement` without
  accepted-work payout or settlement authority.
- #451 added the Worker-safe payment destination input classifier at
  `workers/api/src/payment-destination-input.ts`. It uses MDK's
  `bitcoin-payment-instructions` crate as a conformance source but keeps LNURL,
  Lightning Address, and BIP353 resolver behavior behind a future WASM/sidecar
  boundary. Parser output remains redacted and cannot create checkout, payout,
  settlement, or dispatch authority.
- #452 added the typed self-hosted `mdkd` sidecar option at
  `workers/api/src/mdk-sidecar-option.ts`, config route-kind decoding, and the
  runbook at `docs/mdk/2026-06-07-self-hosted-mdkd-sidecar-option.md`. The
  sidecar option distinguishes fake provider, hosted platform, and self-hosted
  `mdkd` modes, requires auth-tier separation, models emergency pause, and
  keeps native runtime and payout authority outside the Worker.

Still required:

- Keep the Cloudflare Container sidecar operational and add an explicit
  operator restart/rotation runbook so future secret rotations do not require
  changing the source-level instance name.
- Configure `MDK_CHECKOUT_WEBHOOK_SECRET` or `MDK_WEBHOOK_SECRET` and
  `MDK_CHECKOUT_WEBHOOK_SOURCE` for the actual event source before using live
  callbacks. Without that secret/source config, the webhook route returns
  missing-configuration state.
- Connect generated Site output to the same Site commerce checkout primitives
  once the live demo path is accepted. The public demo page now covers the
  first-party buyer checkout and clean return flow.
- Use `docs/sites/2026-06-07-generated-site-payment-smoke-runbook.md` as the
  operator evidence map for generated-Site payment claims. It records what the
  deterministic fake-provider smoke batch proves, what hosted-provider
  evidence must add, and what still requires separate real bitcoin movement and
  accepted-work payout settlement receipts.

Issue coverage:

- Closed issues #295, #296, #300, #304, #305, #433, #434, #435, #451, and
  #452 supply the core contracts, demo catalog, live Worker boundary, buyer
  checkout UI, clean return flow, typed payment destination input classifier,
  and self-hosted `mdkd` sidecar option.
- #431 proves outbound two-wallet movement through the payout adapter; it is
  not a demo product purchase flow.

### Test A Payout

Already built:

- Treasury payout authority, service, simulation adapter, MDK agent-wallet
  adapter boundary, payout target policy, Pylon wallet readiness, and receipt
  API foundations exist from #421-#426.
- #427 is now closed. It connects Pylon marketplace jobs, accepted-work
  evidence, payout intents, payout attempts, and settlement receipt projections
  in simulation. It explicitly did not run real MDK payments.
- The MDK agent-wallet adapter parses typed JSON command output and keeps raw
  wallet material out of durable records and public projections.
- #436 now defines the non-secret prerequisite checklist for the real
  two-wallet smoke. That checklist is the source of truth for isolated wallet
  homes, funding readiness, payout target approval, adapter readiness, and
  public-safe receipt readiness before #431 runs.
- MDK agent-wallet readiness now records bucketed balance readiness only
  (`minimum_satisfied` or `minimum_not_satisfied`) instead of exact wallet
  balances.
- #431 has now executed the first real tiny bitcoin movement between two
  isolated MDK wallets through the OpenAgents product surface authority path. The evidence doc is
  `docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md`, and the settlement
  receipt is
  `receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507`.

Still required:

- Use #428's now-closed Artanis payment-backed dispatch gates as the dispatch
  authority boundary for later live payout work.
- Keep the treasury test wallet funded with enough bitcoin for the bounded
  smoke. Code can enforce and record readiness, but it cannot create the
  spendable balance; an operator or funding source must fund the isolated test
  wallet.
- Use #432's OpenAgents product surface/Nexus release gate as the active Pylon v0.2 classifier. It
  points to #431 real movement evidence, #434's Worker-safe MDK runtime
  boundary, #452's self-hosted `mdkd` sidecar option, and #438's retained
  Artanis real-assignment evidence. Old Google Cloud Nexus health remains
  optional transition context only.

Issue coverage:

- #428 built dispatch gating and is now closed/deployed.
- #429 built public-safe receipt/operator visibility and is now
  closed/deployed.
- #430 built public-safe Artanis/Nexus/Pylon lifecycle Forum update mapping
  and is now closed/deployed.
- #431 built the real two-wallet MDK bitcoin movement proof.
- #432 built the Pylon v0.2 release gate runbook and evidence checklist.
- #436 defines the isolated-wallet prerequisites for #431 and is now the setup
  source for that smoke.
- #438 retains the Artanis-administered real small-bitcoin assignment smoke
  needed before accepted-work payout claims can pass the gate.

### Minimal Demo Paths

| Goal | Current capability | Must still be built or done |
| --- | --- | --- |
| Demo product checkout only | Contracts, fake-provider responses, account app, Worker-side MDK route client, Cloudflare Container MDK sidecar, sidecar route-kind config, durable checkout intent/provider-ref persistence, exact-source webhook verification, status transition persistence, receipt/entitlement projection, clean checkout-return route, and public buyer UI exist. A direct live 100-sat amount checkout smoke has passed. | Wire the public demo page through the live sidecar path, create the MDK product if product-mode is desired, and configure the real webhook secret/source. |
| Payout-only smoke | Simulation, visibility, Forum bridge, mocked MDK adapter boundaries, isolated-wallet prerequisites, #431 real two-wallet movement evidence, #438 retained Artanis real-assignment evidence, and #432 release-gate classification exist. | Operator release review still must decide whether and how to publish Pylon v0.2. The gate does not publish or spend by itself. |
| Product payment to payout | Operator-gated payout-intent bridge live. `POST /api/sites/{siteId}/commerce/payout-bridges` consumes verified server-side Site checkout, buyer receipt, and MDK reconciliation state, then creates one Nexus/Treasury payout intent after Treasury authority policy gates pass. | Live provider checkout still needs real MDK route/webhook configuration before real customer payments feed the bridge. Payout dispatch and settlement claims still require their own attempt, verification, and settlement receipt evidence. |

### Sequencing: Parallel Now Vs After The Open Set

For this audit, the release-gate evidence blocker set is now cleared. #431
created the real two-wallet MDK movement evidence, with #436 as its prerequisite
source. #432 created the typed OpenAgents product surface/Nexus release classifier and runbook. #427
is already closed and should be treated as part of the existing simulated
settlement foundation. #428 is now closed and should be treated as the
dispatch-gating foundation. #429 and #430 are closed/deployed visibility and
Forum-publication foundations. #434 supplied the Worker-side hosted checkout
boundary, #435 supplied the first-party buyer checkout UI, and #438 supplied
the retained Artanis real-assignment smoke.

Work that can start now in parallel, without touching the active
Artanis/Nexus/Pylon payout code paths:

- Create or select the demo MDK account product, using MCP or the MDK dashboard,
  and record only non-secret product identifiers or redacted setup notes.
- Use #434's Site commerce live hosted-checkout lane as the Worker-side
  foundation: `OpenAgentsHostedMdkClient`, checkout intent persistence,
  exact-source webhook verification, checkout return handling, and buyer
  entitlement projection.
- Use #435's `/sites/demo-checkout` surface as the first buyer-visible checkout
  smoke once the live route is configured.
- Use #452's sidecar option runbook as the runtime-shape decision source: pure
  hosted/platform route when available, or a Node/native-capable `mdkd`
  sidecar with route binding, separated auth tiers, storage/VSS refs, exact
  webhook-source verification, health projection, and emergency pause. This
  can continue without touching Artanis, Pylon, settlement, or Forum code.
- Use #437's now-live operator bridge when a verified Site buyer payment should
  become accepted-work payout-intent evidence. The bridge rejects checkout return
  URLs, client success claims, duplicate buyer receipts, missing accepted work,
  missing payout target approval, stale wallet readiness, spend-cap violations,
  and missing real-movement gate evidence.
- Add Site-commerce-specific tests and fixtures for real-provider success,
  provider rejection, replayed webhook events, stale checkouts, clean return
  paths, and secret redaction.
- Draft generated Site output that calls the existing checkout-intent route, as
  long as it does not claim payout or settlement completion.
- Use the #431 evidence doc as the real-movement proof source for #432.
  Local/operator-only wallet setup notes must continue to include isolated
  wallet homes and funding requirements but must not commit wallet homes,
  mnemonics, invoices, preimages, exact balances, or private payout targets.

Work that should wait until the open issue set is complete:

- Bridge a verified buyer order or demo product payment into a payout intent.
  That bridge should depend on #428 dispatch gates, #429 receipt/operator
  visibility, #431 real movement evidence, and #432 release evidence.
- Use collected demo-product funds as the source of a Pylon payout. Even though
  #431 proves real movement through OpenAgents product surface receipts, payout testing should stay
  on the isolated treasury-test-wallet path until #432 and the buyer-checkout
  bridge are complete.
- Publish public claims that a product purchase funded a real Pylon settlement.
  That needs #429 public-safe receipt pages and #430 Forum update boundaries.
- Treat Artanis paid dispatch as live production behavior. #428 gates now
  exist, but production claims should still wait for #432 release evidence.
- Declare Pylon v0.2 releasable or update public docs as though the live
  payment loop is complete. That belongs after #432 records the required
  evidence checklist.

## Repository Setup

Package/config audit:

- `package.json` has no `@moneydevkit/*` dependency.
- `workers/api/package.json` has no `@moneydevkit/*` dependency.
- `workers/api/wrangler.jsonc` has no committed `MDK_ACCESS_TOKEN`,
  `MDK_MNEMONIC`, `WITHDRAWAL_DESTINATION`, or MDK-specific vars.
- Filename search found no repo-level `.env` or `.dev.vars` files.
- Existing authenticated credit billing uses Stripe through
  `workers/api/src/billing-routes.ts`,
  `workers/api/src/stripe-billing.ts`, and the browser command in
  `apps/web/src/page/loggedIn/billing/commands.ts`.

Answer to "do we have the API MDK thing setup?":

- No, not as MDK's unified Next.js `/api/mdk` endpoint.
- Yes, partially, as OpenAgents product surface-owned Effect/Worker contracts under
  `/api/sites/:siteId/commerce/...`.

## Worker Routes

The live custom Site commerce route matcher is in
`workers/api/src/site-commerce-routes.ts`:

- `GET /api/sites/:siteId/commerce/discovery`
- `POST /api/sites/:siteId/commerce/checkout-intents`
- `GET /api/sites/:siteId/commerce/checkout-returns/:checkoutIntentRef/:returnAction`
- `POST /api/sites/:siteId/commerce/mdk/webhooks`
- `POST /api/sites/:siteId/commerce/l402/challenges`
- `POST /api/sites/:siteId/commerce/l402/redemptions`

These routes are wired into the Worker through:

- `workers/api/src/index.ts`
- `workers/api/src/worker-routes.ts`

The route behavior is currently explicit about its state:

- discovery returns `implementationState: "fake_provider_contract"`;
- checkout intents default to `makeFakeOpenAgentsHostedMdkClient(...)`;
- configured live checkout intents use the Worker-safe MDK-compatible route
  client and persist redacted provider refs;
- checkout returns read durable checkout/receipt/entitlement state and reject
  checkout query strings;
- MDK webhooks require configured source-specific verification and write
  replay-safe status, receipt, entitlement, and reconciliation records;
- L402 challenge and redemption responses use `implementationState:
  "hosted_contract_stub"`;
- checkout intent creation requires `Idempotency-Key`;
- responses are designed to avoid raw invoices, payment preimages, wallet
  material, MDK credentials, provider grants, customer private data, and payout
  claims.

## Ported MDK Semantics

### MDK Core Checkout

Implemented in `workers/api/src/mdk-core-checkout-contract.ts`, covered by
`workers/api/src/mdk-core-checkout-contract.test.ts`.

Ported behavior includes:

- MDK-style route selection for `create_checkout`, `get_checkout`, and
  `confirm_checkout`;
- amount and product checkout input schemas;
- metadata key/size limits and secret-shaped value rejection;
- customer field normalization;
- safe Site-local checkout path sanitation;
- Worker-compatible HMAC signing and verification for checkout URLs;
- redacted checkout projections;
- hosted checkout plan schema linking prepared checkout state to hosted MDK
  client requests and optional L402 payloads.

This is not a direct port of MDK's React hooks, UI, or Next.js route handler.

### Hosted MDK Client Contract

Implemented in `workers/api/src/hosted-mdk-client.ts`, covered by
`workers/api/src/hosted-mdk-client.test.ts`.

Current behavior:

- typed hosted checkout request/response/projection schemas;
- supported checkout modes: `amount`, `product`, and `l402_invoice`;
- supported environments: `sandbox` and `production`;
- supported denominations: USD cents and bitcoin millisatoshis;
- deterministic fake checkout refs for tests;
- error classification for missing configuration, unsafe metadata, stale
  challenges, provider rejection, provider unavailability, unsupported
  denomination, and secret leakage;
- public projections omit invoice and payment-hash refs;
- `acceptedWorkSettlementAuthority` and `providerPayoutAuthority` are both
  always false.

The file does not currently call a real MDK hosted checkout API. It supplies a
fake client and contract boundary.

### Site Commerce

Implemented in:

- `workers/api/src/site-commerce.ts`
- `workers/api/src/site-payment-manifest.ts`
- `workers/api/src/site-payment-catalog.ts`
- `workers/api/src/site-payment-discovery.ts`
- `workers/api/src/site-commerce-routes.ts`
- `workers/api/src/site-payment-middleware.ts`

Current behavior:

- generated Site manifests can describe checkout products and paid actions;
- catalog code turns manifests into public-safe product/action records;
- discovery projects agent-readable endpoints and spend-cap hints;
- checkout-intent API validates catalog membership, price, customer-data refs,
  clean return paths, and idempotency;
- WFP payment middleware can classify `payment_required`, `allowed`,
  `payment_seen_entitlement_missing`, and blocked states.

Still missing for live MDK:

- a real hosted MDK provider implementation;
- buyer-facing generated Site UI that opens the MDK checkout URL and calls the
  clean return route;
- production sidecar or pure hosted/platform MDK route configuration.

### L402

Implemented across:

- `workers/api/src/l402-credential-service.ts`
- `workers/api/src/l402-payment-headers.ts`
- `workers/api/src/l402-response-contract.ts`
- `workers/api/src/l402-deferred-settlement.ts`
- `workers/api/src/site-payment-middleware.ts`
- `workers/api/src/site-commerce-routes.ts`

Current status:

- Worker-compatible L402 credential payload and header parsing contracts exist;
- `WWW-Authenticate: L402` challenge behavior exists for Site commerce stubs;
- generated-Site L402 challenge and redemption writes now require an active
  registered OpenAgents agent bearer token plus `Idempotency-Key`;
- redemptions accept redacted proof refs only and currently grant entitlement
  stubs, not final settlement evidence;
- deferred-settlement projections now let protected handlers leave credentials
  reusable until success, artifact receipt, response closeout, or operator
  review;
- no route currently validates a real MDK-paid invoice/preimage pair against
  MDK production state.

### Reconciliation

Implemented in `workers/api/src/site-mdk-reconciliation.ts`, with docs in
`docs/sites/2026-06-06-site-mdk-reconciliation-webhook-bridge.md`.

Current status:

- provider events are modeled as redacted hosted MDK status events;
- reconciliation maps provider events into buyer-payment reconciliation records;
- the bridge preserves the distinction between buyer payment evidence and
  provider payout or accepted-work settlement;
- live webhook verification is config-gated by exact source family and secret;
- verified live callbacks can update checkout status and create buyer receipt
  and entitlement records, while repeated provider events project as replays.

### Payout Adapter

Implemented in `workers/api/src/treasury-payment-mdk-agent-wallet-adapter.ts`,
with runbook `docs/nexus/2026-06-07-mdk-agent-wallet-payout-adapter-runbook.md`.

Current status:

- `mdk_agent_wallet` is modeled as an adapter behind payment authority, not as
  route-level shell plumbing;
- command boundaries exist for balance, receive, send, and payments/history;
- JSON command output is parsed through typed boundaries;
- redaction avoids storing mnemonics, raw invoices, preimages, wallet config,
  or private payout targets;
- tests use mocked command responses.

Still missing:

- real two-wallet bitcoin movement proof through OpenAgents product surface receipts;
- production operator path for dispatching MDK wallet commands safely;
- public-safe receipt page for a real MDK payment movement.

## GitHub Issue Audit

Commands used:

- `gh issue list --state open --search "MDK OR payment OR payout OR checkout OR L402" --json number,title,state,url,updatedAt --limit 50`
- `gh search issues mdk --repo OpenAgentsInc/openagents`
- `gh search issues moneydevkit --repo OpenAgentsInc/openagents`
- `gh search issues L402 --repo OpenAgentsInc/openagents`
- `gh search issues checkout --repo OpenAgentsInc/openagents`
- `gh search issues payout --repo OpenAgentsInc/openagents`
- `gh search issues payment --repo OpenAgentsInc/openagents`
- targeted `gh issue view` calls for open Nexus/MDK issues and key closed MDK
  issues.

### Nexus/MDK Issue Coverage

| Issue | Status | Meaning for MDK |
| --- | --- | --- |
| #428 `[OPENAGENTS-NEXUS] Upgrade Artanis Nexus/Pylon adapters with payment-backed dispatch gates` | Closed | Added authority/readiness/spend-cap/idempotency gates before Artanis can dispatch payment-backed work. |
| #429 `[OPENAGENTS-NEXUS] Add public-safe Nexus/Pylon receipt pages and operator dashboard` | Closed | Added payout/settlement inspection without exposing wallet/payment secrets. |
| #430 `[OPENAGENTS-NEXUS] Add Forum bridge for Artanis assignment, incident, release, and payout updates` | Closed | Added public-safe payment and payout lifecycle Forum publication mapping. |
| #431 `[OPENAGENTS-NEXUS] Add two-wallet MDK bitcoin movement smoke with OpenAgents product surface receipts` | Closed/live | First real small-bitcoin MDK movement proof through the OpenAgents product surface authority path. Evidence doc: `docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md`. |
| #432 `[OPENAGENTS-NEXUS] Add Pylon v0.2 OpenAgents product surface release gate runbook and automated evidence checklist` | Closed/live | Release gate requiring simulation, mocked MDK adapter tests, real two-wallet proof, receipt pages, MDK runtime-boundary evidence, Artanis real-assignment evidence, Forum bridge evidence, and docs. Current projection is ready for operator release review after #434 and #438. |
| #436 `[OPENAGENTS-MDK] Prepare isolated wallet prerequisites for two-wallet smoke` | Closed | Defines the non-secret wallet existence, funding readiness, payout target approval, adapter readiness, and receipt readiness checklist for #431. |
| #438 `[OPENAGENTS-NEXUS] Run retained Artanis real small-bitcoin Pylon assignment smoke` | Closed/live | Retains Artanis assignment, accepted-work proof, artifact/proof refs, payout authority refs, public settlement receipt, and release-gate evidence. Evidence doc: `docs/nexus/2026-06-07-artanis-real-small-bitcoin-assignment-smoke-evidence.md`. |

### Key Closed Issues

| Issue | Status | Result |
| --- | --- | --- |
| #163 `OPENAGENTS-SITES-COMMERCE-001` | Closed | Site commerce manifest and product/action schema. |
| #164 `OPENAGENTS-SITES-COMMERCE-002` | Closed | Hosted checkout intent and L402 paid action contract stubs. |
| #165 `OPENAGENTS-SITES-COMMERCE-003` | Closed | MDK agent-wallet sandbox smoke plan. |
| #166 `OPENAGENTS-SITES-COMMERCE-004` | Closed | Site payment to referral/revenue-share linkage model. |
| #289 `OPENAGENTS-H-001` | Closed | Recoverable/non-recoverable payment limit classifier. |
| #290 `OPENAGENTS-H-002` | Closed | Paid endpoint product catalog. |
| #291 `OPENAGENTS-H-003` | Closed | Buyer-side payment ledger contract. |
| #292 `OPENAGENTS-H-004` | Closed | Worker-compatible L402 credential service. |
| #293 `OPENAGENTS-H-005` | Closed | L402 challenge response and error contract. |
| #294 `OPENAGENTS-H-006` | Closed | Standard and collision-safe payment headers. |
| #295 `OPENAGENTS-H-007` | Closed | Hosted MDK invoice client contract with fake/stub client. |
| #296 `OPENAGENTS-H-007A` | Closed | MDK core checkout semantics ported to Effect/Worker services. |
| #297 `OPENAGENTS-H-007B` | Closed | MDK core conformance fixtures. |
| #445 `OPENAGENTS-H-008` | Closed | L402 deferred-settlement contract for success-bound protected handlers. |
| #446 `OPENAGENTS-H-009` | Closed | One-shot and durable buyer payment entitlement policy decisions. |
| #447 `OPENAGENTS-H-010` | Closed | Unified payment decision contract for free-beta, credits, Stripe top-up state, L402/MDK proof, entitlements, and hard/manual/provider blocks. |
| #448 `OPENAGENTS-H-011` | Closed | Dry-run agent spend-cap preview contract for paid routes and generated Site actions with public-safe over-budget guidance and no payment side effects. |
| #449 `OPENAGENTS-H-012` | Closed | Scheduled/queue-safe Site MDK reconciliation worker contract for stale checkouts, expired challenges, duplicate/replayed provider events, receipt repair, entitlement repair, retry/backoff, and redacted projections. |
| #450 `OPENAGENTS-H-013` | Closed | MDK agent-wallet and pay402-compatible smoke runbook plus no-funds fixture for status, balance, receive, unpaid challenge, bounded signet send, paid retry, token-cache handling, and redaction. |
| #451 `OPENAGENTS-H-014` | Closed | Payment destination parser boundary based on the MDK `bitcoin-payment-instructions` source decision, with typed BOLT11/BOLT12/LNURL/Lightning-address/URI classifications and redacted projections. |
| #452 `OPENAGENTS-H-015` | Closed | Self-hosted `mdkd` sidecar option and config route-kind boundary for fake, hosted, and sidecar modes without native MDK runtime inside the Worker. |
| #453 `OPENAGENTS-H-016` | Closed | Payment-specific redaction regression suite covering MDK, L402, Site proof/reconciliation/bridge, agent-wallet smoke fixtures, sidecar options, destination parsing, spend previews, entitlement policy, Nexus/Treasury, Artanis, public docs, OpenAPI, manifest, onboarding, and AGENTS. |
| #454 `OPENAGENTS-SITES-MDK-LIVE-001` | Closed | Deterministic generated-Site payment smoke fixture with one human checkout product, one agent-paid action, manifest/catalog/discovery/helper projections, smoke evidence, and redaction coverage. |
| #455 `OPENAGENTS-SITES-MDK-LIVE-002` | Closed | Generated-Site human checkout smoke through Site commerce APIs, including discovery, checkout intent, durable challenge/intent persistence, clean return status, redaction, and no payment-verified state before reconciliation. |
| #456 `OPENAGENTS-SITES-MDK-LIVE-003` | Closed | Registered-agent-gated generated-Site L402 smoke through discovery, spend-cap dry run, challenge creation, over-cap rejection, unsafe proof rejection, entitlement-stub redemption, deterministic idempotent replay, retry projection, and public-surface docs. |
| #457 `OPENAGENTS-SITES-MDK-LIVE-004` | Closed | Generated-Site reconciliation smoke through checkout return, dashboard Standard Webhooks verification, duplicate replay, payment-verified status transition, receipt, entitlement, payment proof, redaction, and long generated checkout-ref normalization. |
| #458 `OPENAGENTS-SITES-MDK-LIVE-005` | Closed | Generated-Site payment smoke runbook, Sites index entry, public AGENTS guidance, capability-manifest resource, onboarding hash update, and redaction coverage for separating deterministic fake-provider proof, hosted-provider proof, real bitcoin movement proof, and accepted-work payout settlement evidence. |
| #298 `OPENAGENTS-SITES-MDK-001` | Closed | Site payment manifest schema. |
| #299 `OPENAGENTS-SITES-MDK-002` | Closed | Site payment product/action catalog. |
| #300 `OPENAGENTS-SITES-MDK-003` | Closed | Hosted Site checkout intent API contract. |
| #301 `OPENAGENTS-SITES-MDK-004` | Closed | Generated checkout UI primitives. |
| #302 `OPENAGENTS-SITES-MDK-005` | Closed | WFP Site payment middleware. |
| #303 `OPENAGENTS-SITES-MDK-006` | Closed | Agent-readable Site payment manifest and OpenAPI entries. |
| #304 `OPENAGENTS-SITES-MDK-007` | Closed | Clean checkout return and entitlement projection. |
| #305 `OPENAGENTS-SITES-MDK-008` | Closed | Site MDK reconciliation and webhook bridge contract. |
| #421 `[OPENAGENTS-NEXUS] Add treasury payout authority D1 ledger` | Closed | Ledger for payout authority. |
| #422 `[OPENAGENTS-NEXUS] Implement TreasuryPaymentAuthority Effect service contract` | Closed | Authority service boundary. |
| #423 `[OPENAGENTS-NEXUS] Add simulation payout adapter and conformance tests` | Closed | Simulation adapter before live MDK movement. |
| #424 `[OPENAGENTS-NEXUS] Add MDK agent-wallet payout adapter boundary` | Closed | MDK agent-wallet adapter boundary and runbook. |
| #425 `[OPENAGENTS-NEXUS] Add payout target approval, spend caps, and emergency pause policy` | Closed | Payout safety gates. |
| #426 `[OPENAGENTS-NEXUS] Add Pylon registration, heartbeat, wallet readiness, and receipt APIs` | Closed | Pylon readiness and receipt API foundation. |
| #427 `[OPENAGENTS-NEXUS] Wire Pylon marketplace jobs to payout intents and settlement receipts` | Closed | Accepted-work to payout-intent and settlement-receipt simulation path. No real MDK movement in this issue. |

## Practical Next Steps

Parallel now:

1. Create or select a small demo product in the MDK account, then map it into
   an OpenAgents product surface Site commerce catalog record without committing secrets.
2. Deploy/configure the MDK-compatible sidecar or hosted platform route that
   `OpenAgentsHostedMdkClient` can call in production.
3. Keep the fake provider retained for tests and explicit non-live previews,
   not production payment claims.
4. Attach the dry-run spend-cap preview contract to live agent-visible paid
   action routes before allowing agents to attempt payment.
5. Wire the Site MDK reconciliation worker planner into the durable scheduled,
   queue, or operator runner that owns D1 writes and status-check dispatch.
6. Use the MDK agent-wallet smoke runbook only for fake/sandbox or
   operator-approved signet tests; live bitcoin remains blocked without a
   separate named wallet, amount, and spend cap instruction.
7. Keep checkout intents, provider refs, reconciliation events, and entitlement
   decisions durable before exposing any public payment claim.
8. Configure or rotate secrets as Worker secrets or another server-only binding:
   `MDK_ACCESS_TOKEN`, `MDK_MNEMONIC`, and any webhook verification material.
   Do not commit them.

After #431-#432, #434, and #438:

1. Bridge verified buyer payment/order state into payout-intent creation only
   after the payout gates, receipt surfaces, Forum publication boundary, real
   two-wallet smoke, selected live MDK runtime boundary, retained Artanis
   real-assignment smoke, and release evidence are complete.
2. Use demo-product payment as payout funding only after #431 proves real MDK
   movement through OpenAgents product surface receipts and #438 proves the Artanis assignment lane.
3. Publish public product-to-payout claims only after the verified buyer payment
   to payout bridge in #437 links the buyer order, accepted work, payout
   intent, payout attempt, reconciliation, and settlement refs.
4. Update public docs only after the corresponding route is live and tested.

## Bottom Line

OpenAgents product surface has not "installed MDK" in the framework-package sense. It has ported the
useful MDK checkout, payout, and L402 ideas into its own Effect TypeScript
Worker architecture and keeps provider calls behind typed server boundaries.

As of this audit, OpenAgents product surface can expose a buyer checkout surface, persist checkout
intents, verify exact-source MDK callbacks, project buyer receipts and
entitlements, and retain real small-bitcoin payout evidence through the
Nexus/Pylon path. It cannot yet honestly claim that a real MDK-backed demo
product purchase funded a Pylon payout. The next engineering step is not adding
a generic `/api/mdk`; it is configuring the real MDK-compatible route and then
finishing #437 so verified buyer payment/order state can bridge into payout
intents under the established gates.

The safe near-term path is production MDK route/product configuration plus
#437's verified buyer-payment-to-payout bridge.
