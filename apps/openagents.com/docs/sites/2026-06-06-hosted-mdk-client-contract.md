# Hosted MDK Client Contract

Issue: #295 / OPENAGENTS-H-007

Date: 2026-06-06

## Purpose

OpenAgents needs a narrow hosted MoneyDevKit boundary before generated Sites,
agent APIs, Forum paid actions, or L402-protected routes rely on MDK payment
evidence. This slice defines the contract and fake provider only. It does not
require production MDK credentials, expose live checkout routes, settle provider
payouts, or mark accepted work paid.

The implementation lives in `workers/api/src/hosted-mdk-client.ts` with tests
in `workers/api/src/hosted-mdk-client.test.ts`.

## Contract

The hosted client accepts a request derived from:

- a paid endpoint product record;
- a buyer-side payment challenge record;
- optional L402 credential payload metadata;
- return and cancel refs;
- customer-safe metadata refs;
- sandbox/production flags.

Supported payment amounts are:

- USD cents;
- bitcoin-denominated minor units represented as `bitcoin_millisatoshi`.

Credit balances remain an OpenAgents ledger concern and are intentionally not a
hosted MDK checkout amount.

## Fake Provider

`makeFakeOpenAgentsHostedMdkClient` returns deterministic refs for tests and
local contract development:

- checkout ref;
- hosted checkout URL ref;
- redacted invoice ref;
- redacted payment hash ref;
- provider ref;
- entitlement/payment evidence state.

The fake provider has explicit error states for missing configuration, unsafe
metadata, unsupported asset/denomination, provider unavailable, provider
rejected, stale challenge, and secret leakage.

## Authority Boundary

Hosted MDK creates buyer-side payment evidence only. It is not provider payout
authority, Pylon accepted-work settlement truth, revenue-share settlement, or
Treasury payout truth.

The response contract hard-codes:

- `providerPayoutAuthority: false`;
- `acceptedWorkSettlementAuthority: false`;
- `settlementAuthority: buyer_payment_evidence_only`.

## Redaction Boundary

The client rejects raw invoices, payment preimages, wallet state, MDK tokens,
MDK mnemonics, webhook secrets, provider payloads, bearer tokens, customer
private data, raw payment payloads, raw prompts, raw runner logs, source
archives, and secret-shaped values.

Public projections hide invoice and payment-hash refs. Agent, customer, and
operator projections can include those refs because they are redacted refs, not
raw payment material.

## Integration Points

This contract integrates at the type level with:

- #290 paid endpoint product catalog records;
- #291 buyer payment challenge records;
- #292 L402 credential payloads;
- #294 L402 header/payment proof handling.

Future route work can call this client after policy, catalog, ledger, and L402
validation succeed.

## Verification

- `bun run --cwd workers/api test -- src/hosted-mdk-client.test.ts`
- `bun run --cwd workers/api typecheck`
