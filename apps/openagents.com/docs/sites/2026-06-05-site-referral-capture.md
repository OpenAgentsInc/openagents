# Site Referral Capture

Implemented: 2026-06-05

Issue: #174 / OMEGA-SITES-REF-002

## Summary

The second Site referral slice adds Omega-owned capture routes and pending
attribution persistence.

Public Sites and agent manifests should link to these capture routes instead of
putting `ref`, checkout, auth, account, or payout state on product pages.
Successful capture sets a first-party pending-attribution cookie and redirects
to a clean canonical URL.

## Routes

`GET /r/site/:publicSourceRef`

- Validates an active `site_referral_sources.public_source_ref`.
- Accepts optional public routing hints:
  - `target=home`
  - `target=order`
  - `target=agent` or `target=agent_claim`
  - `path=human` or `path=agent`
- Creates a pending `referral_attributions` record when no valid pending
  attribution already exists.
- Redirects to `/`, `/order`, or `https://openagents.com/AGENTS.md` without preserving source query
  parameters.

`GET /r/invite/:publicInviteRef`

- Validates an active, unexpired `referral_invites.public_invite_ref`.
- Validates that the owning source is active.
- Derives the target from invite scope:
  - `site_join` -> `/`
  - `order_start` -> `/order`
  - `agent_claim` -> `https://openagents.com/AGENTS.md`
- Creates a pending `referral_attributions` record.
- Redirects cleanly.

Both routes also allow `HEAD` and reject other methods.

## Persistence

`referral_attributions` stores:

- source and optional invite ids;
- public source and invite refs for public-safe traceability;
- human or agent capture path;
- clean redirect target;
- policy state;
- pending expiry;
- optional claimed user and first verification timestamp for later REF1
  consumption.

The pending cookie is `oa_pending_referral_attribution`. It stores only the
internal pending attribution id, not source secrets, invite token hashes, payout
state, checkout state, or user-private data.

First verified attribution wins at capture time: when the browser already has a
valid pending attribution cookie, a later capture link redirects cleanly but
does not create or replace attribution.

## Failure States

Unknown, expired, disabled, disputed, redeemed, or archived sources/invites
return public-safe no-store JSON errors. Responses do not include token hashes,
raw invite secrets, private source metadata, payment ids, wallet data, provider
grants, or user-private data.

## Remaining Work

REF1 must consume the pending attribution during signup, agent claim, and
order creation, then durably set first verified direct referrer state. REF2
must add owner/operator dashboards, paid-workflow event linkage, and abuse or
dispute policy before payout automation expands.
