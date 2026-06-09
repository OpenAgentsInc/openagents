# Site Referral Abuse, Dispute, And Cap Policy

Implemented: 2026-06-05

Issue: #180 / OPENAGENTS-SITES-REF-008

## Summary

REF2 now has an explicit referral eligibility policy boundary.

The policy layer decides whether a captured or consumed Site referral is
eligible, not eligible, or waiting for manual review. It records those
decisions in an idempotent audit table, but it does not execute credit,
sats, treasury, payout, or settlement actions.

## Table

`site_referral_policy_events`

- records the policy subject being evaluated;
- links optional referral source, invite, attribution, workflow event, order,
  and Site refs;
- stores previous state, decision state, reason, eligibility, customer status,
  and operator override refs;
- uses `idempotency_key` so repeated payment, workflow, or operator attempts do
  not create duplicate policy records;
- stores only bounded metadata and public-safe refs.

This table is eligibility evidence. It is not payout authority.

## Decision States

- `pending`
- `active`
- `held`
- `disputed`
- `capped`
- `reversed`
- `expired`
- `archived`
- `operator_overridden`

## Reasons

- `eligible`
- `self_referral`
- `duplicate_account`
- `collusion_risk`
- `chargeback_refund`
- `sanctions_compliance`
- `expired`
- `cap_exceeded`
- `clawback`
- `operator_override`
- `refund_or_reversal`
- `first_verified_wins`
- `manual_review`

## Service Boundary

`workers/api/src/site-referral-policy.ts` exposes:

- `evaluateSiteReferralPolicy`;
- `publicSiteReferralPolicyDecision`;
- `operatorSiteReferralPolicyDecision`;
- `recordSiteReferralPolicyEvent`;
- `recordOperatorSiteReferralPolicyOverride`;
- `listSiteReferralPolicyEventsByAttribution`;
- `listSiteReferralPolicyEventsBySource`;
- `listSiteReferralPolicyEventsByWorkflowEvent`.

The evaluator enforces:

- self-referral hold;
- first-verified-wins duplicate behavior;
- expired source or attribution rejection;
- disabled/disputed/manual-review holds;
- collusion, duplicate-account, sanctions/compliance, chargeback/refund, and
  clawback signals;
- paid-workflow refund, reversal, eligibility-hold, and dispute-hold states;
- max workflow-event and amount caps;
- operator override recording without inline private notes.

## Projection Rules

Customer-safe projections expose only:

- high-level customer status;
- decision state;
- whether the referral is eligible for a future reward.

They do not expose abuse heuristics, compliance reasons, operator notes,
private user data, payment evidence, wallet state, or provider material.

Operator-safe projections may include typed reasons and refs, but raw private
notes still remain outside the record behind an `operator_note_ref`.

## Dashboard Integration

Referral owner and operator inspection metrics now include aggregate counts for:

- held policy decisions;
- disputed policy decisions;
- capped policy decisions;
- reversed policy decisions;
- operator overrides.

These are aggregate operational signals. They are not public explanations of
why a specific user or referral was held.

## Remaining Work

Future payout/revenue-share work should consume this policy layer before
credit or sats rewards are calculated. No raw signup should become payable
without a positive paid-workflow event and an eligible policy decision.
