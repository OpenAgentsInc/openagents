# Site Referral Source Schema

Implemented: 2026-06-05

Issue: #175 / OPENAGENTS-SITES-REF-001

## Summary

The first Site referral slice adds the storage and typed projection layer for
OpenAgents-owned referral sources and invites.

Capture redirects are implemented in #174. Public Site CTAs and agent manifest
join links remain in #173. This slice gives those features a safe data model.

## Tables

`site_referral_sources` links a public Site to a referrer:

- Site project and optional active Site version.
- Referrer user.
- Public source ref and public slug.
- Optional campaign/source label.
- Policy state: active, disabled, disputed, expired, or archived.

`referral_invites` links scoped invite/capture records to a source:

- Public invite ref.
- Server-owned token hash.
- Scope: Site join, order start, or agent claim.
- Audience path: human or agent.
- Policy state: active, redeemed, expired, disabled, or disputed.
- Optional expiry.

## Public Projection

`workers/api/src/site-referrals.ts` exposes customer/public-safe projections:

- `publicSiteReferralSource(record)`
- `publicReferralInvite(record)`

The public invite projection never includes `tokenHash`. Both projections reject
secret-shaped refs, slugs, labels, or metadata before generated Sites can receive
them.

Generated Site source should receive only:

- public source refs;
- public invite refs when needed; and
- OpenAgents product surface-owned capture URLs created in the capture/CTA issues.

It should not receive token hashes, private invite tokens, wallet material,
provider account refs, checkout state, auth state, or payout promises.
