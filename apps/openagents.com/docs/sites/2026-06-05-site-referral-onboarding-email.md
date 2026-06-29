# Site Referral Onboarding Email

Implemented: 2026-06-05

Issue: #177 / OPENAGENTS-SITES-REF-005

## Summary

REF1 now has the referred-user onboarding hook after verified Site referral
attribution is consumed.

When a signed-in user or first customer order consumes a pending Site referral,
OpenAgents product surface can send a transactional onboarding email through `EmailService` and
enroll the user in the existing onboarding drip stack with referral metadata.
The hook is idempotent, suppression-aware, and public-safe.

## Trigger Points

`/api/session`

- Consumes a valid pending attribution after the signed-in user is materialized.
- Schedules referral onboarding as background work only when the attribution is
  newly consumed.
- Uses `orderState=none` because no order was created by this route.

`/api/customer-orders/active`

- Bootstraps the active order from completed onboarding when needed.
- Consumes or links the verified attribution.
- Schedules referral onboarding with `orderState=active` or `delivered` when
  a newly consumed attribution exists.

`POST /api/customer-orders`

- Creates the customer order.
- Links the order to the verified attribution.
- Schedules referral onboarding with the created order's state when the
  attribution is newly consumed.

## Transactional Email

The transactional email uses:

- `EmailService.renderSiteReferralOnboardingEmail`;
- `EmailService.sendSiteReferralOnboardingEmailWithLedger`;
- template slug `site_referral.onboarding.v1`;
- metadata policy `system.site_referral_onboarding.v1`;
- idempotency key `site_referral_onboarding:<userId>:<attributionId>`.

The ledger reservation path prevents duplicate provider sends for the same
user and attribution. If Resend is not configured, the transactional email
step is skipped without blocking the request path.

The email copy routes the user back to OpenAgents, describes that they arrived
from a public Site, and links the public source Site when a safe public slug is
available. It does not promise earnings, payouts, credits, revenue share, or
settlement.

## Drip Enrollment

The hook also calls `enrollInOnboardingDrip` with referral metadata:

- referral attribution id;
- referral source id;
- public-safe source label;
- public source Site URL when available.

The existing onboarding drip dispatcher still controls day 0/day 1/day 2 send
timing. Suppressed recipients, opted-out recipients, and users with existing
orders are handled through the existing drip enrollment policy.

## Public-Safety Boundary

The referral onboarding hook must not read or render:

- referrer email addresses or private profile data;
- provider account grants;
- OAuth tokens, API keys, or device-flow material;
- wallet keys, preimages, invoices, or checkout secrets;
- payout, settlement, revshare, or guaranteed-credit claims;
- private order details from another user.

The source label is sanitized before it can enter email copy. Unsafe or
secret-shaped source labels fall back to the public Site title, public slug, or
the generic label `an OpenAgents Site`.

## Operational Notes

- Transactional sends use `EmailService` only.
- Suppression and category preferences are checked before provider send.
- Delivery, accepted-provider, duplicate, and failed states are recorded
  through the existing transactional email ledger.
- Background work failures are logged server-side and do not mutate customer
  order responses.
- Customer-facing API responses still do not expose referral attribution ids
  or referrer-private data.

## Remaining Work

REF2 still needs the Site owner/operator referral dashboard, the paid-workflow
referral event ledger, and abuse/dispute/cap/clawback policy before any paid
referral or revenue-share automation expands.
