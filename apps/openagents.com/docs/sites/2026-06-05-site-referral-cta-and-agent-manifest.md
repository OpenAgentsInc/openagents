# Site Referral CTA And Agent Manifest Links

Implemented: 2026-06-05

Issue: #173 / OPENAGENTS-SITES-REF-003

## Summary

The third REF0 Site referral slice adds public-safe CTA and agent-manifest
projections backed by OpenAgents product surface-hosted capture URLs.

This does not make referral payouts live. It gives public Sites, public proof,
and generated Site metadata a safe way to say: start your own OpenAgents Site
from this source, using an OpenAgents capture URL, without copying referral
state into public product URLs.

## Public CTA Projection

`workers/api/src/public-site-referral-cta.ts` defines
`PublicSiteReferralCta`.

The projection includes:

- `openAgentsJoinUrl`
- `referralJoinUrl`
- `agentReferralJoinUrl`
- copyable agent instruction text
- public caveats

For Ben's OTEC Site, the current public capture URLs are:

- Human/order path:
  `https://openagents.com/r/site/site_ref_otec_ben?target=order`
- Agent path:
  `https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent`

The projection rejects secret-shaped source refs, titles, slugs, payment
material, wallet material, provider grants, private keys, webhook secrets, and
token-hash-looking values before generated Sites or manifests can receive them.

## Public Proof

`GET /api/public/proof/otec` now includes:

- top-level `referralCta`;
- `agentInstructionCard.referralCta`; and
- copyable agent instructions that tell agents to preserve attribution through
  the hosted capture URL.

The proof continues to be a discovery surface, not an authorization grant.
Mutating actions still require signed-in owner authority, a scoped API key,
owner claim, or later credits/L402 path.

## Generated Site Metadata

`.openagents/site.json` agent metadata can now carry public referral fields:

- `publicSourceRef`
- `openAgentsJoinUrl`
- `referralJoinUrl`
- `agentReferralJoinUrl`

These are public refs and capture URLs only. Generated Site artifacts must not
receive invite token hashes, private referral secrets, payout state, checkout
state, auth state, provider account refs, wallet material, or private user data.

## Capability Manifest

`/.well-known/openagents.json` now advertises:

- `site_referral_capture` as a public resource template; and
- `request_site_from_public_source` as a public browser-flow action.

This tells agents that the capture boundary is hosted by OpenAgents and that
successful capture redirects to clean product URLs.

## Remaining Work

REF1 must consume pending attribution during signup, agent claim, and order
creation. REF2 must add owner/operator dashboards, event ledgers, and abuse,
dispute, cap, clawback, and payout-eligibility policy before any automated
revshare expands.
