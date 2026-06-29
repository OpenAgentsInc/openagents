# Customer #1 Cohort Privacy Review Checklist

Date: 2026-06-17
Scope: #5233, #5098, Epic D / customer #1 dogfood (#5104).
Builds on: #5200, #5203, #5218, #5230.

## Purpose

A Customer #1 cohort row can count as `loop_completed` only when it has both a
`completionBundleRef` and a `privacyReviewRef`. This checklist defines what an
operator verifies before issuing the privacy-review ref.

The checklist is evidence-only. It does not grant deployment, merge,
accepted-work, payout, settlement, provider-account, product-promise, or public
customer-success authority.

## Privacy Review Ref

Use an opaque ref, not a customer, team, company, or person name:

```text
privacy.customer-one.<opaque-team-or-run-token>.review.v1
```

The ref points to the private operator review record. Public rows, roadmap
updates, issue comments, and `/forge` may show only the ref and safe state, not
the private review record.

## Required Review Checks

Before a completion row receives `privacyReviewRef`, confirm that the row and
its completion bundle:

1. Use opaque refs for team, workspace, run, routing, artifact, verification,
   review, completion, and privacy-review fields.
2. Include no real customer, team, company, or person names unless the owner has
   explicitly approved public attribution.
3. Include no raw prompts, private repo content, source snippets, shell logs,
   stack traces, provider payloads, provider request/response bodies, or private
   acceptance notes.
4. Include no raw URLs, email addresses, invite tokens, bearer tokens, OAuth
   material, API keys, secrets, wallet material, payment hashes, preimages, or
   invoices.
5. Include no commercial terms, customer-private notes, or identifying details
   beyond public-safe refs and generic state labels.
6. Keep blocker and caveat refs public-safe and non-identifying.
7. Match the completed row to a real completion bundle, not a planned,
   candidate, or fabricated row.
8. Preserve any unresolved blockers or caveats instead of silently marking them
   complete.

## Operator Flow

1. Prepare the row from
   `docs/blitz/forge/2026-06-17-customer-one-cohort-row-template.json` in an
   ignored operator workspace.
2. Complete the private privacy review record.
3. Set `privacyReviewRef` to the opaque review ref.
4. Run
   `node scripts/customer-one-cohort-recorder.mjs check --row-file <row.json>`
   from `apps/openagents.com`.
5. Only after the check passes, run the authenticated `upsert` command.
6. Run `node scripts/customer-one-cohort-recorder.mjs audit` to verify whether
   the public projection now satisfies the D3 completion gate.

## Closure Preservation

This checklist does not close #5098 or #5104. Those remain open until the live
public projection proves at least three `loop_completed` rows that count toward
D3 completion and the audit command passes against production.
