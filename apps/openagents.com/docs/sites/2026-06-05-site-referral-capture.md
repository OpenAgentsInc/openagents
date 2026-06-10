# Site Referral Capture

Implemented: 2026-06-05

Issue: #174 / OPENAGENTS-SITES-REF-002

## Summary

The second Site referral slice adds OpenAgents product surface-owned capture routes and pending
attribution persistence.

Public Sites and agent manifests should link to these capture routes instead of
putting `ref`, checkout, auth, account, or payout state on product pages.
Successful capture sets a first-party pending-attribution cookie and redirects
to a clean canonical URL.

The attribution window is thirty days from capture. Before a qualifying
signup, agent claim, or paid order consumes attribution, the browser's current
pending-attribution cookie is the last-touch winner. Consumption locks that
winner exactly once by linking it to the qualifying event and marking the
capture claimed in the same D1 batch.

## Routes

`GET /r/site/:publicSourceRef`

- Validates an active `site_referral_sources.public_source_ref`.
- Accepts optional public routing hints:
  - `target=home`
  - `target=order`
  - `target=agent` or `target=agent_claim`
  - `path=human` or `path=agent`
- Creates a pending `referral_attributions` record and replaces the browser
  pending-attribution cookie for last-touch attribution.
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

Last touch wins until first consumption: a valid pending attribution cookie can
be replaced by a later capture within the thirty-day window, but after signup,
agent claim, or paid order consumption the first consumed attribution remains
the durable direct-referrer record. Consumption is idempotent and uses `db.batch`
for the event link plus claimed-attribution mutation.

## Operator Query

`GET /api/operator/sites/referrals/consumed`

- Requires an admin browser session.
- Returns public-safe consumed attribution refs only: claimed captures with a
  first verification timestamp.
- Redacts private referred-user contact data, token hashes, wallet material,
  payment payloads, provider grants, and source labels that look secret-shaped.

## Failure States

Unknown, expired, disabled, disputed, redeemed, or archived sources/invites
return public-safe no-store JSON errors. Responses do not include token hashes,
raw invite secrets, private source metadata, payment ids, wallet data, provider
grants, or user-private data.

## Remaining Work

REF2 must add paid-workflow event linkage and abuse or dispute policy before
payout automation expands.
