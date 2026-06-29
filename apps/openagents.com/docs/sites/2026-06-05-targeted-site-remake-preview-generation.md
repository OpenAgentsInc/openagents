# Targeted Site Remake Preview Generation

`targeted-site-remake-preview-generation.ts` records concept Site previews
generated from approved targeted-remake briefs.

This is the first generation-side ledger in the targeted outreach lane. It
does not run a generator yet; it defines the durable contract a generator must
write to after using an approved remake brief and source authority pack.

## Data Model

The D1 table is `targeted_site_remake_preview_generations`.

Each record links:

- campaign, prospect/domain, and remake brief;
- quality audit, static capture, rendered capture, and provider adapter refs;
- source authority pack ref;
- generated artifact/source refs;
- candidate Site project and Site version refs;
- concept preview URL;
- generation receipt or failure ref;
- concept-only generation constraints;
- legal-sensitive flag and timestamps.

Preview states are:

- `requested`
- `generating`
- `generated`
- `failed`
- `blocked`
- `archived`

## Approval Gate

Generated output can only be recorded when the remake brief is
`approved_for_generation`. A generated record must have:

- `previewUrl`
- `generatedArtifactRef`
- `generatedSourceRef`
- `candidateSiteVersionRef`

Draft or review-only briefs can still create non-generated request records,
but they cannot create a generated preview candidate.

## Concept Domain

Preview URLs must use:

```text
https://sites.openagents.com/concepts/<campaign>/<target-slug>
```

The service rejects target-domain impersonation and any URL outside the
OpenAgents concept preview host/path.

## Redaction

Records carry refs, not raw provider payloads or copied raw content. Validation
rejects provider payload markers, browser logs, contacts, secrets, payment or
wallet material, and bypass instructions in refs or metadata.

The public projection only includes campaign/domain/prospect, concept slug,
preview URL, request time, and state. Operator projection exposes generated
refs and receipt refs, but not raw metadata.

## Status

Implemented in GitHub issue `#189` as
`OPENAGENTS-SITES-OUTREACH-009: Add targeted remake preview generation`.
