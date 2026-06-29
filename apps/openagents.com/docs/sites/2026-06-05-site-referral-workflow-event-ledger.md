# Site Referral Workflow Event Ledger

Implemented: 2026-06-05

Issue: #179 / OPENAGENTS-SITES-REF-007

## Summary

REF2 now has a durable referral workflow event ledger.

The ledger connects value-bearing Site activity back to verified Site referral
attribution. It records evidence for paid usage, Site checkout, L402
redemption, accepted outcomes, refunds, reversals, holds, disputes, and
operator adjustments without becoming payout or settlement authority.

## Table

`referral_workflow_events`

- Links each event to a `referral_attribution_id` and `referral_source_id`.
- Carries optional order, Site, Site version, product, paid-action, payment,
  entitlement, accepted-work, and related-event refs.
- Uses `idempotency_key` to prevent duplicate records from external callbacks
  or repeated workflow attempts.
- Requires refund and reversal events to link to a prior related event.
- Stores `policy_state`, amount, asset, public receipt ref, timestamps, and
  bounded metadata.

This table is attribution evidence. It does not execute sats payout, credit
issuance, treasury settlement, or provider payout.

## Event Kinds

- `paid_usage`
- `site_checkout`
- `l402_redemption`
- `accepted_outcome`
- `refund`
- `reversal`
- `eligibility_hold`
- `dispute_hold`
- `operator_adjustment`

## Policy States

- `recorded`
- `eligible`
- `held`
- `disputed`
- `refunded`
- `reversed`
- `ignored`

Future policy and payout systems can consume these typed states without
parsing English status text.

## Repository Helpers

`workers/api/src/site-referral-workflow-events.ts` exposes:

- `recordReferralWorkflowEvent`;
- `listReferralWorkflowEventsByAttribution`;
- `listReferralWorkflowEventsBySource`;
- `listReferralWorkflowEventsByOrder`;
- `listReferralWorkflowEventsBySite`.

`recordReferralWorkflowEvent` validates refs, rejects secret-shaped payment or
provider material, inserts idempotently, and reads back the stored event by
idempotency key.

## Redaction Boundary

The ledger may store public-safe refs such as `mdk_payment_proof_*`,
`site_payment_event_*`, `site_l402_redemption_*`, `public_receipt_ref`, or an
accepted-work ref.

It must not store:

- raw Lightning invoices;
- preimages or payment secrets;
- wallet keys, mnemonic text, or private keys;
- MDK access tokens or provider/OAuth tokens;
- webhook secrets;
- private user contact data;
- private prompt/run logs.

## Relationship To Existing Site Commerce

The earlier Site commerce revenue-share linkage records a payment/revenue
projection for a Site action. This ledger records referral attribution evidence
for paid or value-bearing workflows. The dashboard and operator inspection
surfaces now count paid workflows from this ledger.

## Remaining Work

Issue #180 still needs the abuse, dispute, cap, clawback, self-referral, and
no-raw-signup-payout policy layer that will interpret these event records for
future eligibility decisions.
