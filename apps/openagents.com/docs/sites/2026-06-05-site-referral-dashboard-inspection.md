# Site Referral Dashboard And Inspection

Implemented: 2026-06-05

Issue: #178 / OPENAGENTS-SITES-REF-006

## Summary

REF2 now has the first Site referral accountability surface.

OpenAgents product surface exposes a public-safe owner aggregate endpoint and an admin-only
operator inspection endpoint. These endpoints turn the existing referral
source, attribution, verified-user, order-link, agent-claim, and current Site
commerce payment-event records into typed projections without exposing private
referred-user contact data.

## Endpoints

`GET /api/sites/referrals/overview`

- Requires a browser session.
- Filters by the signed-in user's `referrer_user_id`.
- Returns aggregate metrics for the user's own Site referral sources.
- Does not include referred-user emails, names, provider accounts, wallet
  material, checkout secrets, or private order logs.

`GET /api/operator/sites/referrals`

- Requires an OpenAgents admin browser session.
- Returns source aggregates plus recent attribution inspection rows.
- Includes refs and policy state needed for support/debugging.
- Still excludes contact data and secret-shaped source labels from the normal
  JSON response.

Both endpoints return no-store JSON responses and accept an optional `limit`
query parameter clamped to 1 through 200.

## Metrics

Each referral source summary includes:

- capture count;
- pending, claimed, disputed, and expired capture counts;
- verified user count;
- linked software-order count;
- agent-claim count;
- paid-workflow count from the Site referral workflow event ledger;
- a reward gate that shows attribution captured, reward eligible, payout
  pending, settled, and Bitcoin-withdrawal-copy allowed as separate fields;
- held, disputed, capped, reversed, and operator-overridden policy-event
  counts from the Site referral policy event ledger;
- latest capture timestamp;
- latest verified timestamp.

The paid-workflow count is backed by the dedicated referral workflow event
ledger added in issue #179. That ledger records paid workflows, refunds,
reversals, accepted outcomes, and eligibility states without executing payouts
or settlement.

The policy counts are backed by the dedicated Site referral policy event ledger
added in issue #180. They expose aggregate operational pressure without
exposing abuse heuristics, compliance details, private notes, or customer
contact data.

The reward gate added for issue #560 is intentionally conservative. Capture
counts show attribution only. Linked order counts do not create payout
eligibility by themselves. Only paid-workflow refs can move a source to
reward-eligible, and policy blockers keep the source in a blocked state even
when paid activity exists. Bitcoin withdrawal or stream copy remains blocked
until public settlement receipt refs are available.

## Public-Safety Boundary

Owner responses do not include:

- private referred-user identifiers beyond aggregate counts;
- emails or contact fields;
- provider grants or OAuth material;
- wallet keys, preimages, invoices, or MDK credentials;
- checkout query strings or raw payment evidence;
- private order logs, prompt/run secrets, or operator notes.

Operator responses include internal ids and public refs needed for support,
but the projection still sanitizes source labels and rejects responses that
contain private contact-field keys or secret-shaped material.

## Remaining Work

Future revenue-share and payout work still needs to add a receipt-backed
settlement ledger before the reward gate can expose settled Bitcoin withdrawal
copy.
