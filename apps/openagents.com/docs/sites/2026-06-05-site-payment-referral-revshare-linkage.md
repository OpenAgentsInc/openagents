# Site Payment Referral And Revenue-Share Linkage

Issue #166 defines the first linkage model from Autopilot Site payments to
referral attribution and provider/revenue-share ledgers.

This document does not create payment obligations, legal terms, withdrawal
rights, or final payout math. It defines the records and safety boundaries that
must exist before Sites commerce can drive referral rewards or provider
revenue-share projections.

## Implementation Artifacts

- `workers/api/src/site-commerce-revenue-share.ts`
- `workers/api/src/site-commerce-revenue-share.test.ts`
- `workers/api/migrations/0066_site_commerce_revenue_share_linkage.sql`

## Core Rule

Payment evidence, referral attribution, entitlement, accepted work, provider
payout eligibility, and settlement are separate states.

```text
Site checkout or L402 payment
-> payment event
-> entitlement event
-> optional referral attribution
-> optional accepted-work ref
-> optional provider payout eligibility
-> optional payout dispatch
-> optional settlement receipt
```

No earlier state implies a later state.

## Records

### `site_commerce_payment_events`

This table records payment-side activity and public-safe references:

- `site_id`
- optional `site_version_id`
- optional `software_order_id`
- optional `product_id`
- optional `paid_action_id`
- optional `customer_ref`
- optional `referral_source_ref`
- optional `payment_evidence_ref`
- optional `entitlement_ref`
- `public_receipt_ref`
- `event_kind`
- `amount`
- `asset`

Allowed event kinds:

- `signup_attributed`
- `checkout_paid`
- `l402_redeemed`
- `credit_spent`
- `accepted_work_closed`
- `refund_or_reversal`

Payment evidence refs must be public-safe refs such as `mdk_payment_proof_...`,
`site_checkout_intent_...`, or `site_l402_redemption_...`, not raw invoices,
preimages, payment hashes, wallet mnemonics, MDK access tokens, webhook secrets,
or payout credentials.

### `site_commerce_revenue_share_links`

This table links a payment event to revenue-share projection state:

- `payment_event_id`
- optional `accepted_work_ref`
- `requested_contributor_asset`
- `provider_payout_claimed`
- optional `nexus_receipt_ref`
- optional `treasury_receipt_ref`
- optional `ldk_settlement_receipt_ref`
- `referral_reward_trigger`
- `provider_payout_eligibility_state`
- `withdrawal_posture`
- `projection_json`

The migration enforces that a provider payout claim requires:

- accepted-work ref;
- Nexus receipt ref;
- Treasury receipt ref; and
- LDK settlement receipt ref.

## Referral Boundary

Referral capture is attribution, not payout eligibility.

A `signup_attributed` event may connect:

```text
new user/customer/site/order -> referral source
```

It may not:

- create a positive reward amount;
- create provider payout eligibility;
- imply accepted work;
- create a Bitcoin withdrawal claim; or
- project a paid reward without a later paid event.

Referral reward trigger is `paid_activity` only when a later paid checkout,
L402 redemption, credit spend, or accepted paid event is linked to the
referral source.

## Asset Boundary

The asset used by the buyer controls the safest default contributor asset.

| Buyer-side asset | Default contributor asset | Withdrawal posture |
| --- | --- | --- |
| `sats` | `sats` | Bitcoin-withdrawable only after settlement, reserves, and policy checks. |
| `credits` | `credits` | Internal credit only; no immediate Bitcoin withdrawal liability. |
| `usd` | `credits` or policy-defined payable | Fiat or credit policy required before any Bitcoin conversion. |

Credit spend must not silently create immediate Bitcoin withdrawal liability.
Promo credits, launch credits, coupons, and free-beta usage should normally
create no withdrawable reward unless a separately budgeted policy says so.

## Pylon Accepted-Work Boundary

Pylon provider payout claims require accepted-work and settlement evidence.

Required refs for a positive provider payout eligibility projection:

- `acceptedWorkRef`
- `nexusReceiptRef`
- `treasuryReceiptRef`
- `ldkSettlementReceiptRef`

Payment evidence alone is not enough. A customer paying for a Site action does
not mean a Pylon provider earned a payout. A provider going online does not mean
the provider is eligible. Accepted work does not mean the payout is settled.

The public dashboard may show:

- paid action receipt;
- entitlement granted;
- referral attribution linked to paid activity;
- accepted-work ref when present;
- payout eligibility state; and
- settlement state only when receipt-backed.

It must not show:

- raw invoices;
- raw preimages;
- payment hashes;
- wallet mnemonics;
- MDK access tokens;
- private payout targets; or
- "settled" without settlement receipt refs.

## Projection Shape

The typed model returns a public-safe projection:

```json
{
  "paymentEvidence": {
    "status": "present",
    "asset": "sats",
    "ref": "mdk_payment_proof_123"
  },
  "entitlement": {
    "status": "present",
    "ref": "site_entitlement_123"
  },
  "referralAttribution": {
    "ref": "referral_source_chris",
    "rewardTrigger": "paid_activity"
  },
  "acceptedWork": {
    "status": "absent",
    "ref": null
  },
  "providerPayoutEligibility": {
    "status": "not_eligible",
    "reason": "No provider payout claim is backed by the current linkage event."
  },
  "settlement": {
    "asset": "sats",
    "withdrawalPosture": "bitcoin_withdrawable_after_settlement"
  },
  "publicReceiptRef": "site_payment_receipt_123"
}
```

This is dashboard state, not a final settlement instruction.

## Test Coverage

The focused tests prove:

- paid Site activity can trigger referral attribution without provider payout
  eligibility;
- raw signup attribution cannot create payout eligibility;
- credit spend cannot become immediate Bitcoin withdrawal liability;
- credit-sourced rewards project as internal credit only;
- Pylon accepted-work payout claims require Nexus, Treasury, and LDK refs;
- valid accepted-work refs can project payout eligibility; and
- unsafe raw payment material is rejected from public refs.

## Next Work

The next implementation slice should write these records from the hosted
checkout/L402 flow and expose them in operator/customer dashboards with the same
public-safe projection rules.
