# MPP Integration Audit

Date: 2026-06-23

> **Current status (2026-07-05, #8387): retired/deferred.** The standalone Khala
> MPP/x402 chat endpoint, root MPP discovery document, Stripe MPP profile config,
> smokes, and replay-cache writers were removed because the surface was not armed
> in committed config and is not directly needed by Khala Code. This audit is now
> historical context for a future rebuild, not current operating guidance.

Scope: OpenAgentsInc/openagents#6049 plus the payment, credit, Stripe, and MPP
issues that affect whether Khala can accept Machine Payments Protocol (MPP)
payments before Stripe Directory profile approval.

> **Current status (2026-06-23, later same day): RESOLVED / SHIPPED — Lightning live on
> Spark.** This audit was written before the rails went live. All three MPP rails are now
> LIVE on production (`openagents-autopilot`, deploy version `271a3720`):
> **⚡ Lightning** (real mainnet BOLT11 via **Spark** PRIMARY through the `MDK_TREASURY`
> container, MDK fallback — and it **leads** the 402), **USDC/crypto** (full pay-loop
> proven end-to-end on staging), and **card/SPT**. The public Stripe profile `@openagents`
> is live (`profile_61Uug9…`), crypto payins enabled, and `/openapi.json` advertises the
> offers lightning-first. The "Not Yet Proven" and "What Is Actually Blocked" items below
> were the snapshot *before* arming and are largely **resolved**; the only remaining open
> item is the Stripe Directory **badge** (external async crawl). The audit body is
> preserved for historical context. Current status & ops:
> [`docs/mpp/README.md`](../mpp/README.md) and
> [`docs/mpp/2026-06-23-mpp-launch-and-badge-runbook.md`](../mpp/2026-06-23-mpp-launch-and-badge-runbook.md).

## Defect B Resolution (2026-06-23, Worker-native)

Defect B — "the credential verify path expected a Stripe `payment_intent="…"`
inside the returned credential, did not find one from a real `mppx` client, and
fell into an infinite 402 loop and never served" — is RESOLVED Worker-native
(WebCrypto, no Node sidecar). The verify path now implements the canonical
Payment Auth protocol instead of a handwritten quoted-param guess:

- **Stateless HMAC challenge binding** (`mpp-canonical.ts`,
  `draft-httpauth-payment-00` §5.1.3). The 402 challenge `id` is
  `base64url(HMAC-SHA256(server_secret, "|".join([realm, method, intent,
  request_b64url, expires, digest, opaque_b64url])))` with JCS/RFC-8785
  canonicalization of the `request` and `opaque` JSON. The server-held secret is
  the new Worker SECRET `KHALA_MPP_SIGNING_SECRET`.
- **Challenge issuance** carries our correlation data (crypto deposit
  PaymentIntent id + amount + network + model) in the `opaque` field
  (base64url JCS), so retry verification recovers it statelessly with no
  per-challenge storage. One `WWW-Authenticate: Payment …` per supported method
  (crypto networks always; `stripe`/SPT only when
  `STRIPE_MPP_NETWORK_PROFILE_ID` is set).
- **Credential verification** decodes `Authorization: Payment <base64url>` →
  `{challenge, payload, source?}` and FAILS CLOSED (returns 402, never serves)
  on any of: (a) recomputed HMAC `id` mismatch (constant-time compare),
  (b) expired challenge, (c) `request` amount/currency not binding to the
  served quote, (d) a method we did not offer. Only after that does it settle:
  - **crypto:** recover the deposit PaymentIntent id from `opaque`, retrieve it
    from Stripe, serve only when `status === 'succeeded'`.
  - **card/SPT:** extract `payload.spt`, enforce single-use via a D1 replay
    cache (`mpp_spt_replay`, migration `0224`), create+confirm a PaymentIntent
    with `shared_payment_granted_token` + the `profile_…` networkId, serve only
    when `succeeded`.
- **Payment-Receipt** (`draft-httpauth-payment-00` §6.3) is now returned on a
  settled paid completion (base64url JCS `{status:"success", method, timestamp,
  reference}`) alongside the unchanged OpenAgents receipt block, with
  `Cache-Control: private`. (Slice B.)
- The settled-payment path still mints USD-origin Khala credits (idempotent
  `mpp:<pi>`, RL-3: inference-spendable, not Bitcoin-withdrawable) and runs the
  SAME completion + metering + receipt loop.

There is NO infinite-402 loop on a real `mppx` credential anymore: a well-formed
crypto or SPT credential that binds to a settled payment serves; a malformed,
tampered, expired, mismatched, replayed, or unsettled credential returns a fresh
402 (with a detail string) exactly as the spec prescribes.

### Arming + secret steps (owner-gated; endpoint stays INERT until all done)

The endpoint is triple-gated and fail-safe inert. To arm it on the live Worker:

1. `wrangler secret put KHALA_MPP_SIGNING_SECRET` (a high-entropy random string;
   the HMAC challenge-binding key — Worker SECRET, never committed/logged).
2. `wrangler secret put STRIPE_API_KEY` (the Stripe secret key; absent ⇒ inert).
3. Set `KHALA_MPP_ENABLED` to `true` (the var, currently OFF).
4. The card/SPT rail additionally needs `STRIPE_MPP_NETWORK_PROFILE_ID` (already a
   committed `var`); without it the endpoint is crypto-only.

With any of `KHALA_MPP_ENABLED` off, no `STRIPE_API_KEY`, or no
`KHALA_MPP_SIGNING_SECRET`, the endpoint returns `503 mpp_not_configured` and
constructs no charge and issues no challenge. THIS DEFECT-B CHANGE DID NOT ARM
THE ENDPOINT, set any live key, or deploy.

### Smoke before arming

Before flipping `KHALA_MPP_ENABLED`, prove the crypto rail end-to-end against
staging with a real MPP/x402 client: issue a 402 with a `KHALA_MPP_SIGNING_SECRET`
set, use Stripe's `simulate_crypto_deposit` (test mode) to settle the deposit
PaymentIntent named in `opaque`, retry with the echoed-challenge credential, and
assert a 200 + `Payment-Receipt` + a dereferenceable USD-credit grant + metered
spend receipt. A real `mppx fetch <staging endpoint>` is now EXPECTED to complete
the crypto flow once the signing secret + test Stripe key are present, because the
challenge/credential wire format is now spec-canonical (HMAC-bound `id`, JCS
`request`/`opaque`, echoed challenge). The card/SPT rail additionally needs the
approved `profile_…` and an SPT-capable client.

## Verdict

OpenAgents is not blocked on the Stripe Directory profile to keep adding MPP
support.

The Stripe profile/Directory approval gates discoverability and, for the Stripe
card Shared Payment Token (SPT) rail, the production `profile_...` network id.
It does not block the protocol surface, the fail-safe 402 endpoint, crypto
USDC/x402 challenge work, OpenAPI/agent discovery docs, staging/test proof
harnesses, or receipt-chain hardening.

The current repo already has the right core shape:

- `/mpp/v1/chat/completions` is mounted and default-inert.
- With `KHALA_MPP_ENABLED` off or no Stripe key it returns `503
  mpp_not_configured` and constructs no charge.
- When configured, the intended flow is: unauthenticated request -> `402` with
  `WWW-Authenticate: Payment ...`; settled payment credential -> mint
  USD-origin Khala credits -> run the same Khala completion, metering, receipt,
  and Bitcoin/Spark contributor-payout loop as the keyed gateway.
- USD/card/USDC-origin value is tagged as inference-spendable
  `usd_credit_msat`, not Bitcoin-withdrawable value.

The remaining gap is not "wait for Stripe Directory." The remaining gap is
making the MPP wire format, credential verification, live/staging receipt proof,
and public discovery/listing evidence match what MPP clients and Stripe will
actually use.

## Sources Reviewed

- Local MPP reference repo: `/Users/christopherdavid/work/projects/repos/mppx`
- Local MPP spec repo: `/Users/christopherdavid/work/projects/repos/mpp-specs`
- Attached MPP docs sitemap and quickstart notes from the user
- Live `mpp.dev` docs and `paymentauth.org` specs, now mirrored locally at
  `docs/reference/mpp/`
- Existing OpenAgents docs:
  - `docs/stripe/2026-06-22-stripe-directory-mpp-khala.md`
  - `docs/stripe/2026-06-22-khala-mpp-integration-plan.md`
  - `apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md`
  - `docs/promises/2026-06-23-khala-billing-mpp-proof-gate.md`
- Current MPP implementation:
  - `apps/openagents.com/workers/api/src/inference/mpp/mpp-chat-completions-routes.ts`
  - `apps/openagents.com/workers/api/src/inference/mpp/mpp-protocol.ts`
  - `apps/openagents.com/workers/api/src/inference/mpp/stripe-mpp-client.ts`
  - `apps/openagents.com/workers/api/src/inference/mpp/mpp-credit-grant.ts`
  - `apps/openagents.com/workers/api/src/inference/mpp/mpp-pricing.ts`
- GitHub issues reviewed: #6049, #6108, #5512, #5524, #5520, #5521, #5508,
  #5477, plus the search result set for closed payment/MDK/L402/Spark issues.
- Stripe integration guidance for this audit:
  - Prefer Checkout Sessions/PaymentIntents over deprecated Charges/Sources.
  - Do not reintroduce explicit `payment_method_types` except where a Stripe
    private-preview MPP/SPT contract explicitly requires a method list.
  - Use restricted API keys where possible, never log keys or payment
  credentials, and verify Stripe webhooks/signatures where webhooks are used.

## Local Docs Mirror

This audit added a repeatable local mirror for the MPP integration corpus:

- `scripts/sync-mpp-docs.mjs`
- `docs/reference/mpp/`

The mirror includes:

- `mpp.dev` `/llms.txt` and `/llms-full.txt`;
- 156 individual `mpp.dev` Markdown pages from the docs sitemap;
- `mpp.dev/.well-known/mcp.json`, which advertises an unauthenticated
  `mpp-docs` MCP server at `https://mpp.dev/api/mcp`;
- `mpp.dev/.well-known/agent-skills/index.json`;
- the canonical Markdown specs copied from `../projects/repos/mpp-specs/specs`;
- rendered TXT specs from `https://paymentauth.org`;
- `mppx` README and agent contract pointers from `../projects/repos/mppx`.

The only mirror warning on 2026-06-23 was upstream-side: the `mpp.dev`
agent-skill index advertises `/.well-known/agent-skills/mppx/SKILL.md`, but
that URL returned 404. The mirror records this in
`docs/reference/mpp/manifest.json`; it does not block our MPP integration work
because the full `mpp.dev` docs, Payment Auth specs, and local `mppx` repo are
available.

Recommendation: use the local mirror first for implementation work, then rerun
`node scripts/sync-mpp-docs.mjs` before any final MPP activation proof because
MPP and the Payment Auth drafts are still moving quickly.

## What MPP Requires

MPP is an HTTP 402 challenge-credential-receipt protocol:

- Server returns `402 Payment Required` with one or more
  `WWW-Authenticate: Payment ...` challenges.
- Client pays and retries with `Authorization: Payment ...`.
- Server verifies the payment credential and returns the resource with
  `Payment-Receipt`.
- The `mppx` reference implementation binds challenges with a server-side
  secret/HMAC over realm, method, intent, request, expiry, digest, and opaque
  metadata.
- The reference supports one-time charges, sessions/streaming, MCP monetization,
  multiple methods, Stripe SPT, Tempo sessions, EVM/x402-style rails, and a
  proxy mode for OpenAI-compatible endpoints.

Important compatibility point: `mppx`'s README/proxy examples are Node/Bun/Deno
or framework oriented. The OpenAgents gateway is a Cloudflare Worker. The repo's
current Worker-native REST implementation is a reasonable first slice, but a
final compatible client proof must test against `mppx` itself, not only our
handwritten quoted-param parser.

## Current OpenAgents State

### Built

- `apps/openagents.com/workers/api/src/index.ts` mounts
  `/mpp/v1/chat/completions`.
- `mpp-chat-completions-routes.ts` has fail-safe gates for
  `KHALA_MPP_ENABLED` and `STRIPE_API_KEY`.
- `mpp-pricing.ts` derives a flat per-call quote from the existing Khala pricing
  model, with separate crypto and card floors.
- `stripe-mpp-client.ts` creates crypto deposit-mode PaymentIntents and
  retrieves PaymentIntents to verify settlement, pinned to the machine-payments
  preview API version used by the current implementation.
- `mpp-credit-grant.ts` mints settled MPP funds into the existing
  USD-origin inference credit ledger, idempotent by PaymentIntent id.
- Focused tests cover inert behavior, 402 challenge shape, settled-payment
  credit minting, completion/metering execution, and unpaid `processing`
  payments refusing to serve.

### Proven

- #6108 closed after commit `d0a05e641d`, deployment, and live smoke evidence
  proving the production endpoint is inert at HTTP 503 `mpp_not_configured`
  when not configured.
- #5477 closed the base inference credits and metering system: usage-based,
  receipt-first, idempotent, and tied to the agent balance ledger.
- #5520 and #5512 have public receipt readback surfaces for Stripe checkout,
  card-credit-spend, and inference receipts, but the actual staging TEST
  checkout and production live-money proofs remain open.

### Not Yet Proven

- `mppx fetch <openagents endpoint>` can complete an end-to-end paid request
  against staging or production.
- The crypto rail can be paid and verified by a real MPP/x402 client on staging.
  (The wire format is now spec-canonical per "Defect B Resolution" above; the
  remaining gap is a live staging proof with a real `mppx` client + Stripe test
  `simulate_crypto_deposit`, not the protocol shape.)

Resolved 2026-06-23 by the Defect B change (see above):

- The Worker-native challenge is now HMAC-bound with JCS `request`/`opaque`,
  expiry, and a slot for body digest, matching `draft-httpauth-payment-00`
  §5.1.3.
- The card/SPT rail settles Worker-native (create+confirm PaymentIntent with
  `shared_payment_granted_token` + replay cache) without a Node sidecar.
- `Payment-Receipt` is now returned on successful MPP completions alongside the
  unchanged `openagents` receipt block.

## Issue Read

- #6049 is the right parent epic. Its body already says the owner decision is
  made: accept MPP; USDC/card settle into Stripe balance; Bitcoin/Spark stays
  the contributor-payout rail. That means implementation can continue before
  Stripe Directory listing approval.
- #6108 is correctly closed as a safety proof, not as a full activation proof.
  The endpoint being inert is good launch safety, not a reason to stop building.
- #5520 remains the staging money-flow gate. It needs real Stripe TEST checkout,
  card-credit-spend receipt, metered spend receipt, and referral/test payout
  evidence before production live-money activation.
- #5521 remains the production promotion gate and should stay blocked on #5520.
- #5512 remains the production card-credit collection gate. It should not block
  MPP protocol/discovery work, but it does block production live card-money
  claims.
- #5508 remains environment/owner gated for staging admin/auth setup. It matters
  for proof runs, not for adding local protocol tests and docs.
- #5524 remains the broader revenue-loop promise epic. MPP should feed the same
  receipt-first ledger rather than create a second payout authority.

## What Is Actually Blocked

Blocked by Stripe profile/Directory approval:

- Appearing in Stripe Directory search as an approved/listed OpenAgents profile.
- Treating the card/SPT rail as production-ready if it requires the
  `STRIPE_MPP_NETWORK_PROFILE_ID`.
- Any public copy that says agents can already find and pay Khala through the
  Directory.

Blocked by owner/live credentials or staging evidence:

- Live MPP payment activation.
- Production Stripe live card acceptance.
- Product-promise green flips for paid credits or MPP.
- Strict `smoke:khala:billing-mpp-proof -- --require-complete`.
- Any live spend, live payout, or Bitcoin settlement.

Not blocked:

- Worker-native MPP protocol correctness tests.
- A staging/test-mode MPP proof path.
- `mppx` client compatibility tests against a local Worker handler.
- Public discovery surfaces that truthfully say MPP is rolling out/gated.
- OpenAPI/capability metadata for `/mpp/v1/chat/completions`.
- Crypto USDC/x402 challenge hardening.
- A thin Node sidecar spike if card/SPT needs `mppx` verification.
- Receipt-header alignment, request digest binding, replay protection, and
  idempotency hardening.

## Correct Implementation Direction

1. Keep the current one-balance model.
   Settled MPP funds should mint USD-origin inference credits and spend through
   the existing Khala metering hook. Do not create a separate MPP balance,
   payout path, or settlement authority.

2. Make MPP protocol compatibility a first-class test target.
   Add tests that generate a challenge, use `mppx` client primitives or fixtures
   from `/projects/repos/mppx`, retry with `Authorization: Payment ...`, and
   assert that OpenAgents accepts or rejects exactly as MPP expects.

3. Add canonical challenge binding before live activation.
   The current handwritten `Payment id="...", amount="..."` challenge is useful
   but too thin for final MPP. The live path should bind method, intent, amount,
   route, body digest, expiry, payment intent, and opaque OpenAgents correlation
   data with a server-side secret. `MPP_SECRET_KEY` or equivalent must be
   server-only and never logged.

4. Return both receipt shapes on success.
   A paid MPP completion should include the normal OpenAgents receipt block and
   a standards-shaped `Payment-Receipt` header. The OpenAgents receipt remains
   the public-proof authority; the MPP header lets generic MPP clients close the
   protocol loop.

5. Decide Worker-native versus sidecar by proof, not preference.
   Keep Worker-native crypto deposit verification if it can pass real `mppx`
   compatibility tests. If card/SPT or exact challenge signing needs Node SDK
   behavior, run a tiny sidecar only for MPP challenge/sign/verify and keep the
   Worker authoritative for auth, Khala execution, metering, credits, and
   receipts.

6. Treat Stripe card/SPT as a later rail.
   The local `mppx` Stripe example still uses `paymentMethodTypes: ['card']`
   because its SPT method advertises supported card methods. Stripe best
   practice says not to use explicit `payment_method_types` for normal
   PaymentIntent/Checkout integrations. OpenAgents already removed an explicit
   Stripe payment method setting in #6108. Before card/SPT activation, document
   whether the SPT private-preview contract requires that advertised list, and
   keep normal OpenAgents Checkout/PaymentIntent code on dynamic payment
   methods.

7. Keep public claims honest.
   The public promise state stays non-green until there is dereferenceable
   staging or production evidence. Directory profile approval should not be
   used as payment proof, and inert endpoint safety should not be described as
   paid availability.

## Concrete Next Slices

### Slice A - MPP Wire Compatibility

Files likely involved:

- `apps/openagents.com/workers/api/src/inference/mpp/mpp-protocol.ts`
- `apps/openagents.com/workers/api/src/inference/mpp/mpp-protocol.test.ts`
- `apps/openagents.com/workers/api/src/inference/mpp/mpp-chat-completions-routes.test.ts`

Acceptance:

- Challenge includes expiry, digest or body-binding, opaque/correlation data,
  and route/method scope.
- Credential replay with changed body/model/path fails.
- Tests include mppx-compatible serialized challenge/credential fixtures.
- Missing/invalid credentials return 402, never a free completion.

### Slice B - MPP Receipt Header

Files likely involved:

- `apps/openagents.com/workers/api/src/inference/mpp/mpp-chat-completions-routes.ts`
- `apps/openagents.com/workers/api/src/inference/mpp/mpp-protocol.ts`
- focused route tests

Acceptance:

- Successful settled-payment completion returns `Payment-Receipt`.
- The header includes method, status, timestamp, reference, and OpenAgents
  public receipt ref or payment intent ref.
- The existing OpenAgents response body receipt remains unchanged.

### Slice C - Staging Crypto Proof

Files likely involved:

- existing `smoke:khala:billing-mpp-proof` script
- `apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md`

Acceptance:

- With test/staging Stripe MPP credentials, a real crypto deposit challenge can
  be issued and paid by an MPP/x402-capable client.
- The smoke records public-safe refs only.
- The result proves paid completion, credit mint, metered spend, and receipt
  readback.

### Slice D - Card/SPT Sidecar Decision

Files likely involved only after compatibility proof:

- Worker MPP route
- possible tiny Node/Bun MPP sidecar package or Pylon-hosted adapter
- docs under `docs/stripe/` or `apps/openagents.com/docs/launch/`

Acceptance:

- If Worker-native card/SPT cannot verify full mppx credentials, sidecar handles
  only the protocol-specific SPT work.
- Worker still owns balances, metering, receipts, and public-proof authority.
- Restricted Stripe key and webhook/signature boundaries are documented.

### Slice E - Discovery De-stale

Files likely involved:

- `/llms.txt`, `/agents.md`, `/ai.md`, `/skill.md` rendering path
- OpenAPI/capability metadata
- #6049 issue comment after deploy/smoke

Acceptance:

- Production URLs return the intended plain text/Markdown content, not the app
  shell, for StripeBot and agent crawlers.
- Copy says MPP is gated/rolling out until proof exists.
- After profile approval, Directory search verification can be added as a
  separate evidence step.

## Security And Policy Gates

- Keep `MPP_SECRET_KEY`, Stripe keys, SPTs, payment credentials, mnemonics, and
  bearer tokens out of logs, docs, fixtures, issue bodies, and commit metadata.
- Use restricted Stripe API keys where possible.
- Use idempotency keys bound to challenge/payment refs.
- Bind credentials to route, method, body digest/model, amount, expiry, and
  payment intent.
- Never serve an unpaid or unsettled request.
- Do not allow USD-origin MPP credits to become Bitcoin-withdrawable.
- Do not run live spend, live checkout, live Stripe activation, or live payout
  from an audit lane.
- Product promises stay red/yellow/non-green until the receipt-first evidence
  and owner signoff exist.

## Recommended Issue Updates

- #6049: keep open, but add a status comment saying implementation is not
  blocked on Directory approval. Split the next work into MPP wire
  compatibility, `Payment-Receipt`, staging crypto proof, card/SPT sidecar
  decision, and discovery de-stale.
- #6108: stay closed. It proved fail-safe inert behavior, not full paid MPP.
- #5520/#5521/#5512: keep their staging and production live-money gates intact.
  They should not block protocol work, but they must block green claims and
  production live-money activation.

## Bottom Line

The right move is to continue MPP now, but to do it in the repo's existing
receipt-first shape:

- protocol and discovery now;
- staging/test proof next;
- live card/SPT and Directory claims only after Stripe profile, credentials,
  and receipt evidence;
- Bitcoin/Spark remains the outbound contributor-payout rail.

That path is compatible with the current #6049 owner decision and does not
collide with #6108's fail-safe launch safety.
