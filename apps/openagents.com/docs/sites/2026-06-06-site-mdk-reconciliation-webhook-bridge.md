# Site MDK Reconciliation And Webhook Bridge

Date: 2026-06-06

Roadmap: OPENAGENTS-SITES-MDK-008 / GitHub #305

Status: implemented as a fake-provider and config-gated contract.

## Purpose

Generated Sites need a payment bridge that can observe hosted MoneyDevKit
checkout status changes and turn them into OpenAgents product surface buyer-payment records without
making the Site itself a wallet, payment processor, or payout authority.

This slice adds the first typed reconciliation boundary for hosted Site
checkout events. It normalizes safe provider event refs into
`buyer_payment_reconciliation_events`, links them to existing Site checkout,
receipt, entitlement, and clean-return projections, and keeps raw payment or
provider material out of every public/customer/agent projection.

## Implemented Contract

The Worker code now has:

- `OpenAgentsSiteMdkProviderEvent` for a redacted hosted MDK status event.
- `OpenAgentsSiteMdkReconciliationInput` for the provider event plus the
  associated hosted checkout, optional prior event, receipt, entitlement, and
  clean checkout return projection.
- `OpenAgentsSiteMdkReconciliationProjection` for audience-safe reconciliation
  output.
- `projectOpenAgentsSiteMdkReconciliation(...)` to derive matched, observed,
  rejected, and replayed buyer-payment reconciliation projections.

The projection always sets:

- `payoutAuthority: false`
- `acceptedWorkSettlementAuthority: false`

That distinction is intentional. A hosted checkout status can prove buyer-side
payment evidence for a Site product or paid action. It does not settle
OpenAgents contributor payouts, accepted-work payouts, Pylon earnings, or
Treasury claims.

## Status Mapping

The current bridge maps events as follows:

| Provider event condition | Buyer reconciliation status |
| --- | --- |
| Existing `previousEventRef` is present | `replayed` |
| Checkout, challenge, product, provider, Site, receipt, or entitlement refs do not match | `rejected` |
| Non-fake provider event lacks verified signature | `rejected` |
| Matching `payment_received` event | `matched` |
| Matching non-final status such as pending or expired | `observed` |

Fake-provider events are allowed for tests and staged generated-Site payment
flows. Non-fake provider events stay `verification_config_gated` until live
webhook verification, replay storage, and production MDK credentials are
configured through the hosted OpenAgents product surface boundary.

## Projection Split

Agent and customer projections show only the safe reconciliation state:

- buyer payment reconciliation status;
- challenge/product/receipt refs where already allowed by the buyer-payment
  ledger projection;
- hosted checkout projection with raw invoice and payment hash refs omitted;
- clean checkout return state;
- reason refs for fake-provider or verification-gated behavior.

Operator projections can include safe provider refs such as provider ref,
provider event ref, event digest ref, signature binding ref, and previous event
ref. They still never include raw provider payloads or secret-bearing values.

## Redaction Rules

The bridge rejects inputs or projections containing:

- raw provider webhook payloads;
- MDK credentials, access tokens, webhook secrets, or mnemonics;
- raw Lightning invoices, payment hashes, preimages, or wallet state;
- provider grants, provider account payloads, provider tokens, or payout claims;
- customer private data such as email/name/value fields;
- raw runner prompts, run logs, source archives, cookies, OAuth material, or
  bearer tokens.

The tests cover matched fake-provider reconciliation, replay handling,
unverified non-fake rejection, mismatched checkout rejection, operator redaction,
and raw payment/customer data rejection.

## Remaining Work

This is not yet a production webhook receiver. The next slices should add:

- durable webhook intake with verified provider signatures;
- D1 idempotency/replay storage for provider event refs and body digests;
- stale-checkout expiration sweeps;
- Site payment receipt/public proof views;
- generated Site UI that reads receipt and entitlement state after clean
  checkout returns;
- internal sandbox/signet smoke tests for a real MDK-backed checkout path;
- multi-agent Forum payment/reward simulation before any public earning claims.
