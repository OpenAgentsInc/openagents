# Targeted Site Sales Reward Policy

Date: 2026-06-05

Status: implemented for issue #208.

## Purpose

Targeted Site remake and outreach campaigns need a reward policy ledger before
agents or users can safely earn from sales work. This slice records accepted
outcome policy events and projections without dispatching payouts or claiming
settlement.

The core rule remains:

```text
buyer payment evidence != referral attribution
referral attribution != accepted work
accepted work != reward eligibility
reward eligibility != payout intent
payout intent != settlement
```

Each state is represented independently by public-safe refs.

## D1 Ledger

`targeted_site_sales_reward_policy_events` records:

- `id` and unique `idempotency_key`;
- `campaign_id`;
- `agent_ref`;
- optional `prospect_id`;
- `outcome_kind`;
- derived `policy_state`;
- `reward_asset`;
- `reward_amount`;
- optional `buyer_payment_ref`;
- optional `referral_attribution_ref`;
- optional `accepted_work_ref`;
- optional `payout_intent_ref`;
- optional `settlement_caveat_ref`;
- optional `dispute_ref`;
- required `public_receipt_ref`;
- optional `related_event_id`;
- bounded `metadata_json`;
- `occurred_at`, `created_at`, and `archived_at`.

## Outcome Kinds

The ledger supports:

- `lead_proposed`
- `meeting_accepted`
- `customer_accepted`
- `reward_eligible`
- `payout_intent_created`
- `reward_held`
- `reward_disputed`
- `reward_reversed`
- `refund_recorded`
- `complaint_recorded`
- `settlement_caveat_recorded`

These derive policy states:

- `proposed`
- `accepted`
- `held`
- `disputed`
- `reversed`
- `eligible`

Reward assets are `credits`, `sats`, or `internal_payable`.

## Service Contract

`recordTargetedSiteSalesRewardPolicyEvent`:

- validates active, unarchived campaign records;
- validates optional prospects belong to the same campaign;
- records idempotently by `idempotency_key`;
- rejects raw provider, email, private customer, payment, wallet, token,
  invoice, preimage, and secret-like material in refs or metadata;
- requires `reward_eligible` events to include `acceptedWorkRef`;
- requires payout intent events to include `payoutIntentRef` and a related
  policy event;
- requires dispute and complaint events to include `disputeRef` and a related
  policy event;
- requires reversal, refund, and settlement-caveat events to link to a related
  policy event;
- requires settlement-caveat events to include `settlementCaveatRef`.

`projectTargetedSiteSalesRewardPolicy`:

- reads reward policy events for a campaign and agent;
- can narrow projection to a prospect;
- returns the latest policy state and reward amount;
- carries buyer payment, referral, accepted work, payout intent, dispute, and
  settlement caveat refs as separate fields;
- derives settlement posture without claiming settlement.

Settlement posture can be:

- `no_settlement_claim`
- `eligible_not_settled`
- `payout_intent_not_settled`
- `blocked_or_reversed`

## Public Projection

`publicTargetedSiteSalesRewardPolicyProjection` exposes only:

- campaign id;
- agent ref;
- prospect id;
- event count;
- latest event time and kind;
- policy state;
- reward amount and asset;
- public receipt ref;
- settlement posture.

It excludes raw metadata and private payment/customer/prospect details. Buyer
payment refs, referral refs, accepted work refs, payout intent refs, dispute
refs, and settlement caveat refs remain operator/internal projection fields
until specific public proof rules are reviewed.

## Boundaries

This slice does not:

- send outreach;
- book meetings;
- accept customers by itself;
- calculate final legal payment obligations;
- create payout credentials;
- dispatch credits, sats, or Bitcoin;
- settle through MDK, LDK, Treasury, or Nexus.

It creates the policy ledger that later dashboards, agent campaign tools,
revenue-share projections, and settlement workflows can consume.
