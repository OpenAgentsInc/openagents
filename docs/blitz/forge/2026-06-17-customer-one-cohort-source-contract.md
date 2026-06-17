# Customer #1 Cohort Source Contract

Date: 2026-06-17
Scope: #5203, #5098, Epic D / customer #1 dogfood (#5104).
Builds on: #5200 and #5201.

## Purpose

#5201 made `/forge` show an honest Customer #1 cohort-readiness lane, but that
lane is still `awaiting-source`. This contract defines the source shape that can
feed that lane later without inventing ad hoc fields or exposing private team
material.

The contract is not a storage migration, API route, or UI wiring change. It is
the boundary the next implementation slices must preserve.

## Source Split

The D3 cohort source has two layers:

1. Private/operator rows: internal rows that may point at private team and
   workspace authority refs.
2. Public-safe projection rows: redacted rows safe for `/forge`, roadmap
   comments, issue closeouts, and future operator summaries.

Only public-safe projection rows may reach the browser by default.

## Private Operator Row

Private rows may exist only behind operator-authenticated surfaces and should
use opaque refs for all sensitive data:

| Field                 | Required                        | Notes                                                              |
| --------------------- | ------------------------------- | ------------------------------------------------------------------ |
| `teamCohortRef`       | yes                             | Stable opaque ref such as `cohort.team.alpha.v1`; not a real name. |
| `state`               | yes                             | One of the D3 states from #5200.                                   |
| `candidateRef`        | when candidate or later         | Opaque candidate record ref.                                       |
| `inviteRef`           | when invited or later           | Bounded invite ref and expiry policy ref.                          |
| `workspaceRef`        | when seeded or later            | Forge workspace or invited workspace ref.                          |
| `templateRef`         | when seeded or later            | Vertical/template ref.                                             |
| `runRef`              | when first run starts or later  | Primary dogfood Run or work-order ref.                             |
| `routingRef`          | when first run starts or later  | Owned-node, fallback-lane, or blocked-routing evidence ref.        |
| `reviewRef`           | when delivery reviewed or later | Human review and reviewer-authority ref.                           |
| `artifactRef`         | when delivery reviewed or later | Public-safe artifact/delivery/deferred-output ref.                 |
| `verificationRef`     | when delivery reviewed or later | Test, smoke, manual QA, or blocked-verification ref.               |
| `privacyReviewRef`    | when completion is claimed      | Confirms private material was omitted/redacted.                    |
| `completionBundleRef` | when loop completed             | Final bundle tying all public-safe refs together.                  |
| `blockerRefs`         | no                              | Public-safe blockers only.                                         |
| `caveatRefs`          | no                              | Public-safe caveats only.                                          |
| `updatedAt`           | yes                             | Source freshness timestamp.                                        |

Private rows may reference internal authority records, but they must still avoid
storing raw secrets or raw private content.

## Operator Row Packet

The public-safe row packet template for real D3 cohort evidence lives at
`docs/blitz/forge/2026-06-17-customer-one-cohort-row-template.json`.
The privacy-review checklist for issuing `privacyReviewRef` lives at
`docs/blitz/forge/2026-06-17-customer-one-cohort-privacy-review-checklist.md`.

To prepare a real row:

1. Copy the template outside the repo or into an ignored operator workspace.
2. Replace every `replace-me` / placeholder value with opaque public-safe refs.
3. Complete the privacy-review checklist before setting `privacyReviewRef`.
4. Run
   `node scripts/customer-one-cohort-recorder.mjs check --row-file <row.json>`
   from `apps/openagents.com`.
5. Only after the local check passes, run the authenticated `upsert` command
   with `OPENAGENTS_ADMIN_API_TOKEN`.

The checker rejects obvious private-material markers, unresolved placeholders,
invalid states, missing freshness timestamps, non-array blocker/caveat refs,
and `loop_completed` rows missing either `completionBundleRef` or
`privacyReviewRef`. The template and checker do not record a production row by
themselves and do not close #5098/#5104 without real evidence.

To audit closure readiness from public evidence, run
`node scripts/customer-one-cohort-recorder.mjs audit` from
`apps/openagents.com`. The audit command reads only
`/api/public/customer-one-cohort` and exits successfully only when the public
projection gate is `ready` and the projection proves at least the D3 minimum
count of completed, privacy-reviewed rows.

## Public-Safe Projection Row

Projection rows must be safe to render in the Forge cockpit:

| Field                 | Required | Notes                                                                         |
| --------------------- | -------- | ----------------------------------------------------------------------------- |
| `teamCohortRef`       | yes      | Opaque ref only.                                                              |
| `state`               | yes      | Same D3 state enum.                                                           |
| `displayLabel`        | yes      | Generic label such as `Team 1`, never a real name unless explicitly approved. |
| `verticalRef`         | no       | Safe vertical/category ref.                                                   |
| `workspaceRef`        | no       | Safe workspace ref, never raw invite token or private project name.           |
| `templateRef`         | no       | Safe template ref.                                                            |
| `runRef`              | no       | Safe Run/work-order ref.                                                      |
| `routingRef`          | no       | Safe routing evidence ref.                                                    |
| `reviewRef`           | no       | Safe review ref.                                                              |
| `artifactRef`         | no       | Safe artifact or deferred-output ref.                                         |
| `verificationRef`     | no       | Safe verification ref.                                                        |
| `privacyReviewRef`    | no       | Required before completed state counts.                                       |
| `completionBundleRef` | no       | Required before completed state counts.                                       |
| `blockerRefs`         | yes      | Empty array when none.                                                        |
| `caveatRefs`          | yes      | Empty array when none.                                                        |

Projection rows are evidence summaries only. They cannot grant runtime,
deployment, merge, accepted-work, payout, settlement, provider-account mutation,
or public product-promise authority.

## D3 State Enum

Allowed values are:

- `candidate`
- `invited`
- `workspace_seeded`
- `first_run_started`
- `delivery_reviewed`
- `loop_completed`
- `blocked`
- `deferred`

`loop_completed` counts toward #5098 only when `completionBundleRef` and
`privacyReviewRef` are both present.

## Projection Envelope

A future source should return an envelope like this:

```json
{
  "generatedAt": "2026-06-17T00:00:00.000Z",
  "staleness": {
    "mode": "live_at_read",
    "maxStalenessSeconds": 300,
    "rebuildsOn": ["cohort_row_written", "privacy_review_recorded"]
  },
  "target": {
    "minimumCompletedTeams": 3,
    "maximumTargetTeams": 5
  },
  "counts": {
    "candidate": 0,
    "invited": 0,
    "workspace_seeded": 0,
    "first_run_started": 0,
    "delivery_reviewed": 0,
    "loop_completed": 0,
    "blocked": 0,
    "deferred": 0
  },
  "gate": {
    "state": "blocked",
    "reasonRefs": ["reason.customer_one.cohort_completion_bundles_missing"]
  },
  "rows": []
}
```

The exact implementation may use Effect Schema or a shared package type, but it
must preserve the private/public split, the `generatedAt` freshness field, and a
declared staleness contract.

## Redaction Rules

Public-safe projection rows must reject:

- real team, company, or person names without explicit public-attribution
  approval;
- raw prompts, private repository content, shell logs, stack traces, provider
  payloads, source files, invoices, payment hashes, preimages, wallet material,
  bearer tokens, OAuth material, API keys, local paths, and customer-private
  data;
- raw invite tokens, raw email addresses, private acceptance notes, and
  commercial details.

Allowed public values are opaque refs, state labels, counts, target bounds,
template refs, route refs, issue refs, policy refs, caveat refs, blocker refs,
and short operator-safe summaries.

## Closure Preservation

This contract does not close #5098 or #5104. Those remain open until:

- at least three projection rows reach `loop_completed`;
- each counted row has `completionBundleRef` and `privacyReviewRef`;
- the roadmap names the completed cohort count and any remaining caveats;
- the owner explicitly accepts any deferral if the epic closes before five
  teams.

## Future Implementation Slices

- Add private/operator storage or source rows for D3 cohort entries.
- Add an operator-authenticated Worker projection using this contract.
- Wire `/forge` cohort-readiness metrics to the projection.
- Record the first real team completion bundle, then repeat until the minimum
  count is met.
