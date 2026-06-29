# Targeted Site Remake Brief And Source Authority Pack

`targeted-site-remake-brief.ts` is the handoff contract between targeted Site
capture/audit work and concept Site generation.

The brief deliberately stores authority, not raw scraped payloads. It links a
campaign, prospect/domain, quality audit, static/rendered/provider capture
refs, screenshots, audit findings, copied text/image refs, and a source
authority pack. Later generation can use the brief as a bounded input without
receiving private provider logs, contact data, payment or wallet material,
secrets, bypass instructions, or unsupported claims.

## Data Model

The D1 table is `targeted_site_remake_briefs`.

Each record contains:

- `campaign_id`, optional `prospect_id`, and `normalized_domain`.
- `quality_audit_id` from `targeted_site_quality_audits`.
- Optional static, rendered, and provider adapter capture refs.
- State: `draft`, `ready_for_operator_review`,
  `approved_for_generation`, `rejected`, `blocked`, or `archived`.
- `source_authority_pack_json` with public refs, source hashes, allowed-use
  caveats, required disclosures, and prohibited claims.
- `audit_finding_refs_json`, `original_screenshot_refs_json`,
  `copied_text_refs_json`, and `copied_image_refs_json`.
- `generation_constraints_json`, which keeps concept-only and law-firm safety
  controls enabled.
- Public-safe metadata and timestamps.

## Source Authority Pack

A source authority card contains:

- `kind`: original screenshot, original copy, original image, public business
  fact, public listing, operator note, or audit finding.
- `publicRef`: a safe ref to the source card or captured artifact.
- `sourceHash`: a public-safe content hash ref.
- `allowedUse`: how generation may use the source.
- `caveats`: any operator-visible limitation.

The pack also includes a `sourcePackRef`, `requiredDisclosures`, and
`prohibitedClaims`.

## Guardrails

Remake briefs require:

- at least one audit finding ref;
- at least one original screenshot ref;
- at least one source authority card;
- no blocked quality audit;
- safe refs only;
- no raw provider payloads, browser logs, contacts, secrets, payment/wallet
  material, or bypass instructions;
- concept-only generation constraints;
- law-firm safety controls for fake reviews, fake credentials, fake case
  results, legal advice, misleading endorsements, and unverifiable guarantees.

Legal-sensitive briefs cannot authorize claim categories such as reviews,
case results, credentials, guarantees, legal advice, or endorsements.

## Projections

The public projection only exposes campaign/domain/prospect state,
preparation time, and source-card count.

The operator projection exposes reviewable refs, constraints, and the source
authority pack, but still redacts raw metadata and never exposes provider
payloads or private material.

## Status

Implemented in GitHub issue `#188` as
`OPENAGENTS-SITES-OUTREACH-008: Add remake brief and source authority pack`.
