# Targeted Site Quality Audit Scorer

Implemented: 2026-06-05

Issue: #187 / OPENAGENTS-SITES-OUTREACH-007

## Summary

OpenAgents product surface now has a typed website quality audit scoring contract for targeted Site
remake campaigns.

The scorer consumes evidence refs from static capture, rendered capture, and
provider adapter records. It does not store raw capture payloads, provider
logs, contact data, payment material, or browser output.

## Service Boundary

`workers/api/src/targeted-site-quality-audit.ts` exposes:

- `evaluateTargetedSiteQualityAudit`;
- `recordTargetedSiteQualityAudit`;
- list helpers by campaign, prospect, and normalized domain;
- customer-safe and operator-safe projections.

## Score Dimensions

Scores are bounded from 0 through 100 across:

- design age;
- mobile/responsive risk;
- information architecture;
- local SEO and metadata;
- CTA clarity;
- trust signals;
- image quality;
- accessibility;
- performance risk;
- content quality;
- stale, broken, or mixed-content signals;
- legal-sensitive claims.

The current overall score is the average of all dimensions. Later issues can
replace that with a weighted model without changing the record boundary.

## Recommendation States

The scorer returns:

- `remake_candidate` for weak sites;
- `monitor` for middling sites;
- `skip` for strong sites;
- `manual_review` when legal-sensitive claims are present;
- `blocked` when the caller marks the audit blocked.

Legal-sensitive claims intentionally force manual review before any remake
brief or outreach step.

## Audit Records

Migration `0077_targeted_site_quality_audits.sql` adds
`targeted_site_quality_audits`.

Each audit records:

- campaign id;
- optional prospect id;
- normalized domain;
- optional static capture, rendered capture, and provider adapter refs;
- state and recommendation;
- overall score;
- legal-sensitive flag;
- bounded dimensions;
- evidence refs;
- public-safe metadata;
- audit, creation, and archive timestamps.

## Projection And Redaction

Customer-safe projections include score, recommendation, legal-sensitive flag,
evidence count, and timestamps.

Operator-safe projections include dimension scores and evidence refs, but still
hide metadata and never include raw capture payloads, provider logs, contact
data, payment/wallet material, or bot-protection bypass instructions.

## Remaining Work

Issue #188 should consume audit records and capture/source evidence to create a
reviewable remake brief and source authority pack before preview generation.
