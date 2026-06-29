# Targeted Site Operator Review

`targeted-site-operator-review.ts` is the internal operator review surface
contract for targeted Site remake campaigns.

It does not expose a public route. It defines the durable decision ledger and
UI-ready view model that a future operator page can render without receiving
raw provider payloads, contacts, secrets, payment or wallet material, or
bypass instructions.

## Review Model

The view model shows:

- campaign and target domain;
- capture policy state;
- static, rendered, and provider capture refs;
- audit score label;
- source authority card count;
- remake brief state;
- preview URL and preview state;
- outreach draft readiness;
- meeting CTA readiness;
- suppression state;
- action availability with disabled reasons.

Timestamps are formatted into operator-readable labels such as:

```text
2026-06-05 18:12 UTC
```

Raw ISO timestamps are not shown in this view model.

## Decisions

The D1 table is `targeted_site_operator_review_events`.

Supported operator decisions are:

- `approve_preview`
- `reject_preview`
- `request_regeneration`
- `skip_target`
- `approve_outreach`
- `block_target`
- `archive`

Each decision maps to a durable next state:

- `preview_approved`
- `preview_rejected`
- `regeneration_requested`
- `target_skipped`
- `outreach_approved`
- `target_blocked`
- `archived`

## Approval Gates

`approve_outreach` is disabled unless:

- the preview is generated;
- the preview URL exists;
- an outreach draft ref exists;
- a meeting CTA ref exists;
- suppression state is `clear`.

All decisions require at least one evidence ref and an operator actor ref.

## Projections

The public projection only exposes campaign/domain, preview generation ref,
and resulting state.

The operator projection exposes decision refs, evidence refs, outreach draft
ref, meeting CTA ref, suppression state, and actor ref. It does not expose raw
metadata.

## Status

Implemented in GitHub issue `#190` as
`OPENAGENTS-SITES-OUTREACH-010: Add internal operator review UI for targeted remakes`.
