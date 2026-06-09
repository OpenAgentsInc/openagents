# Site Referral Attribution Consumption

Implemented: 2026-06-05

Issue: #176 / OPENAGENTS-SITES-REF-004

## Summary

REF1 now has the first verified attribution consumption path.

REF0 capture creates a pending `referral_attributions` record and stores only
its id in the first-party `oa_pending_referral_attribution` cookie. This slice
consumes that pending attribution during session materialization and customer
order creation/bootstrapping, then links the verified relationship without
returning referral internals in customer responses.

## Tables

`user_referral_attributions`

- One active first verified referral attribution per user.
- Records the source, optional invite, capture path, target, and first
  verification timestamp.
- First verified attribution wins unless a future explicit operator
  dispute/override path changes policy state.

`order_referral_attributions`

- Links a customer software order to the user's verified referral attribution.
- Lets later paid-workflow events attach to the original source Site without
  exposing referred-user private data publicly.

`agent_referral_attributions`

- Provides the future owner-claimed agent helper path.
- Public agent mutation remains gated; this table only creates the durable
  linkage target needed when that claim path is implemented.

## Runtime Behavior

`/api/session`

- Attempts to consume a valid pending attribution for the signed-in user after
  user upsert.
- Logs storage failures server-side without adding referral details to the
  session response.
- Clears the pending cookie when the attribution is consumed, expired, or
  already superseded by first verified attribution.

`/api/customer-orders/active`

- Bootstraps an order from completed onboarding as before.
- Links the order to the user's verified attribution when a valid pending
  attribution exists.
- Preserves first verified attribution when the user already has one.

`POST /api/customer-orders`

- Creates the customer order as before.
- Links the new order to the verified attribution.
- Clears consumed or stale pending attribution cookies.

## Public-Safety Boundary

Customer order responses do not include:

- referral attribution ids;
- public source refs;
- invite refs or token hashes;
- payment material;
- provider grants;
- wallet material; or
- referred-user/referrer private data.

Future owner/operator dashboards can inspect aggregates through separate
public-safe projections.

## Remaining Work

Issue #177 adds referred-user onboarding and EmailService hooks after this
verified attribution path. REF2 still needs owner/operator dashboards,
paid-workflow referral event linkage, and abuse/dispute/cap/clawback policy.
