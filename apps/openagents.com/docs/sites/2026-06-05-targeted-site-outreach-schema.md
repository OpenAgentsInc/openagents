# Targeted Site Outreach Campaign And Prospect Schema

Implemented: 2026-06-05

Issue: #181 / OPENAGENTS-SITES-OUTREACH-001

## Summary

OpenAgents product surface now has the first durable schema slice for targeted Site remake and
outreach campaigns.

This slice is intentionally internal and governed. It lets operators create a
campaign, store deduped prospect targets, and carry suppression/capture/review
state forward into the later Exa discovery and respectful capture policy
issues.

## Tables

`targeted_site_campaigns`

- campaign id and public-safe slug;
- owner and optional operator user refs;
- vertical and geography;
- source authority ref;
- optional budget cap and suppression policy refs;
- operator state;
- bounded metadata and timestamps.

`targeted_site_prospects`

- campaign id and idempotency key;
- normalized target domain and origin URL;
- public-safe company/site name;
- contact refs, not raw contact data;
- vertical, geography, source ref, and discovery confidence;
- suppression, capture, and review states;
- bounded metadata and timestamps.

Prospects dedupe by `(campaign_id, normalized_domain)` and also carry an
idempotency key so repeated import or discovery runs do not create duplicates.

## Service Boundary

`workers/api/src/targeted-site-outreach.ts` exposes:

- `createTargetedSiteCampaign`;
- `readTargetedSiteCampaignBySlug`;
- `listTargetedSiteCampaignsByOwner`;
- `listTargetedSiteCampaignsByOperator`;
- `upsertTargetedSiteProspect`;
- `listTargetedSiteProspectsByCampaign`.

The service normalizes domains, validates origin URLs, clamps discovery
confidence to 0 through 1, and stores only public-safe refs plus bounded
metadata.

## State Model

Campaign operator states:

- `draft`
- `active`
- `paused`
- `reviewing`
- `completed`
- `archived`

Prospect suppression states:

- `unknown`
- `clear`
- `suppressed`
- `manual_review`

Prospect capture states:

- `not_started`
- `policy_pending`
- `allowed`
- `blocked`
- `captured`
- `archived`

Prospect review states:

- `pending`
- `ready`
- `approved`
- `skipped`
- `archived`

## Public-Safety Boundary

The targeted outreach schema must not store:

- raw email addresses or phone numbers;
- raw provider payloads;
- OAuth tokens or provider-account material;
- wallet, payment, Lightning, or MDK secrets;
- private operator notes.

Contact material must be represented as a public-safe contact ref. Private
notes and contact data belong in future scoped CRM/suppression systems, not in
the prospect projection.

## Remaining Work

Issues #182 and #183 will add Exa-backed prospect discovery planning and the
respectful capture policy gates that decide whether a stored prospect may be
fetched at all.
