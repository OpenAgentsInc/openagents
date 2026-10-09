# Public API surfaces: what existed, what exists (2026-10-09)

Short notes for #11078 (`openagents inference`, `/v1/openapi.json`, and
x402 pay-per-request on the inference routes). Found with
`git log --oneline --all -- '*openapi*' 'docs/api/*' 'crates/gateway/src/discovery.rs'`
and greps for `x402`, `openapi`, and `llms.txt`.

## Before: the TypeScript Worker (June to August 2026, deleted)

| When | What | Where it went |
| --- | --- | --- |
| 2026-06-11 to 07-09 | `openagents.com` Worker served `GET /openapi.json` (a hand-kept OpenAPI document, `openagents-openapi.ts`), a capability manifest, and `llms.txt`. A route-coverage test (`openagents-openapi-routes.test.ts`, #4752, #8633) failed when a registered route had no OpenAPI entry. | Deleted with the TypeScript roots, `d613b8ea22` (2026-08-28, #266). |
| 2026-06-19 | Inference gateway skeleton in the Worker: OpenAI-compatible `/v1/chat/completions`, a provider adapter, a metering hook, a pricing multiplier (#5476, #5478, #5481). | Deleted (same commit). The Rust `crates/inference` replaces it. |
| 2026-06-23 | Khala: "an OpenAI-compatible endpoint at openagents.com/api" with a free tier (#6228) and an MPP discovery document at `/openapi.json` with `x-payment-info` offers (USDC and Stripe card), emitted only while the paid route was actually live (`7be1a3dafc`). Episode 242 (`docs/transcripts/242.md`) introduced it. | Deleted. The honesty rule (advertise payment only when it is armed) carries over: `/v1/openapi.json` names x402 only when `inference.x402` is configured. |
| 2026-07-16 | Desktop loopback OpenAPI control surface for Full Auto (FA-H13, #8886). | Local-only; not a public API. |
| Nexus | `nexus.openagents.com` control and treasury APIs. | Deprecated; folded into the current surfaces. Not reused. |

Lesson kept: a hand-written OpenAPI document drifts unless a test holds it
to the route table, and a payment advertisement must follow the route's
real state.

## The Rust decision gateway (September 2026, live code)

- `crates/oak` (`82e88545d5`, 2026-09-20): the decision API's caller CLI
  and `docs/decision-models/openapi.yaml`.
- `crates/gateway` discovery surface (`0550c08a1e`, #9488, 2026-09-22):
  `/llms.txt`, `/openapi.yaml` (decision API only, bundled from
  `docs/agents/openapi.yaml`), `/api-catalog.json`, agent card, MCP server
  card, `/v1/docs`. `crates/gateway/tests/discovery.rs` holds
  `api-catalog.json` to `serve::mounted_paths`. These describe the decision
  routes (`/v1/systemone`, `/v1/classify`, `/v1/jobs`), not inference.
- Keys, accounts, holds, and settlement: `tenancy::keys` (`oak_` keys),
  `tenancy::money`, the gateway's `money`, `billing`, `card_funding`
  (Stripe), Lightning `funding`, `pay-ledger`.

## x402 (September to October 2026, live code)

- `crates/x402` (#9789, `d40d606509`, 2026-09-27): x402 v2 `exact` on
  Lightning (`lnbtc`): the embedded facilitator (verify, settle once), the
  `FileReplayStore` (one consumption key per payment hash, `O_EXCL`), the
  header codecs (`PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`,
  `PAYMENT-RESPONSE`), and `server::Resource` (challenge, settle, execute)
  for one paid resource. `crates/wallet` (#9777) issues invoices whose
  description hash is the request binding.
- Pay front (#10186, `73de9ee9c6`, 2026-10-02): many priced routes on one
  wallet, x402 and the HTTP `Payment` scheme on one invoice, a settlement
  sink before execution (`openagents pay serve`). BYOK on it (#10176).
- Buyer side: `openagents x402 fetch` (policy ceilings, wallet or node
  payer, the ledger of what was paid).
- Design: `docs/api/2026-10-02-openagents-api.md` section 5 (D3, D10, D12,
  D13): a keyless call past the free tier gets a `402` with x402 terms over
  Lightning, bitcoin only; keys draw on a prepaid balance. Sessions
  (deposit, refund the unspent part) are designed, not built.

## The inference gateway (2026-10-09, this week)

- Spec: `docs/inference/gateway.md`. `crates/inference` (wire types, SSE,
  Chat Completions translation, router, meter, rate card, stored
  responses), mounted by `crates/gateway`: `/v1/responses`,
  `/v1/chat/completions`, `/v1/responses/compact`, `/v1/responses/{id}`,
  the WebSocket, `/v1/models`, `/v1/rates` (#11066), `/v1/usage/{id}`,
  `/v1/key`, key limits (#11065, #11071).
- Website: `/docs/api` guides and `/docs/api/llms.txt`
  (`crates/openagents-web/src/pages/api_docs.rs`); `openagents.com/api/v1`
  passes through to the gateway.
- Missing before #11078: a machine-readable description of the inference
  routes, a command-line client, and a way to pay without an account.

## What #11078 reuses

| Need | Reused |
| --- | --- |
| Route table as the source of truth | The discovery test's pattern: `serve::mounted_paths` and each module's `routes()` |
| Pricing a paid request | `inference_public::priced` (the worst-case hold the balance path uses) and the rate card |
| Challenge, proof check, replay | `openagents_x402::{Facilitator, FileReplayStore, wire}` and the `server::Receiver` trait; the resident wallet as receiver, as Lightning funding reaches it |
| Paying from the command line | `openagents x402`'s `buy` (policy ceilings, payer, ledger) |
| Test invoices | `nostr::x402::test_invoice` and the receiver pattern in `crates/x402/tests/front.rs` |
