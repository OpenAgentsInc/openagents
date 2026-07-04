# BF-9.2 Weekly Pipeline Review

Date: 2026-07-03
Status: BF-9.2 operating artifact; no promise state flip; no public copy change
Source issue: OpenAgentsInc/openagents#8116
Roadmap refs: [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-9.2 and
[`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) AW-0 A0.3

This is the public-safe weekly review artifact for the business pipeline:
intake -> scope -> receipt-plan -> close. It turns the sales motion into a
queue with instruments instead of an ad hoc owner memory. It records only
vertical descriptors and opaque refs. Client names, contact details, raw call
notes, private requirements, payment identifiers, and shared-channel content do
not belong in this file.

## Review Cadence

- Owner review: weekly, before any public case-study or promise-copy decision.
- Review window: trailing seven days, closed at the review timestamp.
- Source of truth: structured intake rows, scoped-call notes after redaction,
  receipt-plan records, payment or close receipts, and commitment-ledger rows.
- Output: one dated review entry with metric deltas, queue movements, blockers,
  commitment checks, and next actions.

## Stage Contract

| Stage | Queue state | Entry receipt | Exit receipt | Primary metric |
| --- | --- | --- | --- | --- |
| Intake | `intake_received` | Business signup, vertical form, referred lead, or operator-entered opaque lead ref | Qualified or rejected intake decision | qualified intakes |
| Scope | `scope_scheduled` / `scope_completed` | Qualified intake with source attribution | Buyer-confirmed scope summary | intake-to-scope rate |
| Receipt plan | `receipt_plan_sent` | Confirmed scope summary | Receipt plan accepted or rejected | time-to-receipt-plan |
| Close | `closed_won` / `closed_lost` | Accepted receipt plan | Payment, signed kickoff, or explicit lost reason | close rate |
| Quick win | `quick_win_started` / `quick_win_delivered` | Closed-won engagement | First accepted outcome receipt | time-to-quick-win |

Every row must carry:

- `pipelineRef`: opaque stable ref, for example `biz-pipe-2026w27-001`.
- `vertical`: non-identifying descriptor such as `legal`, `agency`,
  `e-commerce`, `health`, or `internal`.
- `sourceRef`: public-safe attribution ref, not raw UTM or private contact data.
- `businessSignupRequestId`: optional link to the originating
  `business_signup_requests.id`; when present, the row's `sourceRef` must match
  the signup row.
- `stage`: one of the queue states above.
- `ownerRole`: role label only, such as `operator`, `reviewer`, or
  `fulfillment-agent`.
- `nextActionDueAt`: date or `none`.
- `blockerRef`: typed blocker or `none`.
- `receiptRefs`: public-safe refs for the current stage receipts.

Implementation status (#8267): source attribution is durable on `/business`
signup and completed Khala intake specs. The funnel dashboard splits
visit/signup/spec/pay counts by bounded `sourceRef` with `not_measured` rates
when a denominator is absent. Pipeline rows store only opaque source refs and
can reciprocally link to signup rows, allowing questions like
`sourceRef=apollo_agent_readiness_a` payback to be answered from aggregate
pipeline/funnel metrics without storing raw UTM strings or private prospect
identity.

## Weekly Metrics

| Metric | Definition | Review rule |
| --- | --- | --- |
| Qualified intakes | Count of `intake_received` rows accepted into scope during the window | Split by vertical and source; rejected rows need a typed reason. |
| Scope calls completed | Count of qualified intakes that reached `scope_completed` | Any no-show or reschedule older than seven days gets a blocker. |
| Intake-to-scope rate | `scope_completed / qualified_intakes` | Report as `pending` when denominator is zero; never fabricate a rate. |
| Receipt plans sent | Count of `receipt_plan_sent` rows | Each plan must name deliverables, review gates, receipts, and non-goals. |
| Scope-to-plan time | Median elapsed time from `scope_completed` to `receipt_plan_sent` | Use `pending` until at least one completed elapsed interval exists. |
| Close rate | `closed_won / receipt_plan_decisions` | Decisions are `closed_won + closed_lost`; parked rows are not decisions. |
| Time-to-quick-win | Median elapsed time from `closed_won` to first accepted outcome receipt | Use `not_started` when no closed-won row has reached delivery. |
| Commitment coverage | Count of promised sends/deliverables with commitment-ledger rows | Must be 100%; missing rows are defects under BF-9.1. |
| Overclaim incidents | Count of public or buyer-facing statements not backed by a receipt/promise gate | Target is zero; any incident creates a copy-gate follow-up. |

## Review Entry: 2026-W27

Status: scaffolded baseline review. The queue instrumentation exists as a
review artifact; live data rows are not asserted by this document.

| Metric | Value | Evidence |
| --- | --- | --- |
| Qualified intakes | `pending` | Instrument defined above; awaiting first redacted weekly data pull. |
| Scope calls completed | `pending` | Instrument defined above. |
| Intake-to-scope rate | `pending` | No denominator asserted in this public checkout. |
| Receipt plans sent | `pending` | Receipt-plan contract defined above. |
| Scope-to-plan time | `pending` | No completed elapsed interval asserted. |
| Close rate | `pending` | No receipt-plan decisions asserted. |
| Time-to-quick-win | `not_started` | No closed-won quick-win row asserted. |
| Commitment coverage | `pending` | BF-9.1 ledger rows must be reviewed in this packet once live. |
| Overclaim incidents | `0 asserted` | This artifact adds no claim-bearing public copy. |

### Queue Movements

No live pipeline movements are asserted in this public checkout. Future weekly
entries should use this shape:

| Pipeline ref | Vertical | From | To | Receipt ref | Next action |
| --- | --- | --- | --- | --- | --- |
| `biz-pipe-YYYYwNN-000` | `vertical` | `stage` | `stage` | `receipt.ref` | `role: action by YYYY-MM-DD` |

### Commitment Check

Every promised deliverable, follow-up send, or buyer-facing receipt-plan item
must have a commitment-ledger row before the review can close. A missing row is
reported as:

```text
BLOCKER: commitment.untracked pipelineRef=<opaque-ref> vertical=<descriptor>
```

### Copy And Promise Gate

- Do not publish buyer-facing copy from this review.
- Do not imply self-serve service delivery, guaranteed outcomes, referral
  payouts, or regulated-professional service capability unless the matching
  promise record and receipt gate are green for that exact claim.
- Case-study candidates require public-safe receipts, opaque refs, and a
  separate copy-gate pass before publication.

## Weekly Closeout Checklist

- [ ] All active pipeline rows have a current queue state.
- [ ] Every stage movement has a receipt ref.
- [ ] Every parked or blocked row has a typed blocker and next review date.
- [ ] Every promised send or deliverable has a commitment-ledger row.
- [ ] Metrics use `pending`, `not_started`, or `not_measured` instead of
      invented values when data is absent.
- [ ] No client-identifying information appears in the review.
- [ ] No public copy or promise state is broadened by the review.

BF-9.2 is satisfied for this issue by the existence of this review artifact and
the instrument definitions above. Future implementation work can replace the
manual review values with generated rows, but the public-safety and receipt
rules stay the contract.

## Implementation Update: #8263

The paper queue now has a typed D1 implementation in
`business_pipeline_rows`, linked to BF-9.1 rows by the nullable
`business_commitment_ledger.pipeline_ref` column. The operator API is:

- `GET /api/operator/business/pipeline`
- `POST /api/operator/business/pipeline`
- `POST /api/operator/business/pipeline/{pipelineRef}/advance`
- `POST /api/operator/business/pipeline/{pipelineRef}/commitments`
- `GET /api/operator/business/pipeline/metrics`

All writes are admin-token gated, stage advancement requires a receipt ref, and
the metrics response uses `measured` / `not_measured` instead of fabricated
rates. The `$25k` target is readable as `qualifiedPipeline.targetUsdCents` with
the current quoted min/max cents range. Operators can call the same API through:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts metrics
bun apps/openagents.com/scripts/operator-business-pipeline.ts create \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --vertical e-commerce \
  --source-ref apollo_agent_readiness_ecommerce \
  --owner-role operator \
  --receipt-ref receipt.business.intake.example
bun apps/openagents.com/scripts/operator-business-pipeline.ts advance \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --stage scope_scheduled \
  --receipt-ref receipt.business.scope_scheduled.example
```

The implementation preserves this artifact's privacy boundary: rows accept only
opaque refs and vertical descriptors, never prospect names, emails, domains, raw
CRM payloads, or call notes. Pipeline rows without linked commitment-ledger rows
surface as `commitment.untracked` defects in the metrics response.

## Starter Credit Linkage: #8264

The BF-9.2 queue now carries starter-credit evidence without changing stage
state. Operators can grant the §8 sales credit through:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts grant-credit \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --account-ref agent:sales_starter_001
bun apps/openagents.com/scripts/operator-business-pipeline.ts link-credit-redemption \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --grant-ref sales-starter-grant-001 \
  --redemption-receipt-ref receipt.inference.charge.example
```

The credit grant writes a normal `usd_credit_grant` credit-ledger pay-in receipt
and records `sales_starter_credit` attribution in
`business_starter_credit_grants`. Defaults are hard-capped at 25 grants per
window and 10000 USD cents per grant; cap exceedance is a typed refusal. The
credit is non-transferable, engagement-scoped, and USD-origin
(`usd_credit_msat`), so it is spendable against the engagement but has no
crypto/cash-out path. Both the grant receipt and later redemption receipt refs
are appended to the pipeline row's `receiptRefs`.

## Outreach Sequence Tooling: #8265

Report-led outreach is now a reusable sequence substrate tied to BF-9.2 rows.
Operators render drafts first, from a pipeline row plus LG-1 audit/finding refs:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts render-outreach \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --subject-ref prospect.agent_ready.example \
  --audit-report-ref audit.agent_readiness.example \
  --finding-refs finding.agent_readiness.blank_shell,finding.agent_readiness.missing_llms
```

Before a draft is stored, the route checks the suppression boundary:
partner-routed rows, existing partners/customers, and active intake rows cannot
enter a cold sequence. Built-in template variants follow the LG-4 skeleton:
observed fact -> offer sentence -> registry-true proof point -> single CTA ->
identification/opt-out. Gated claims for self-serve delivery, pays-you loops,
HIPAA/sovereign posture, public prices, and referral payouts fail claims lint.

Send recording stays owner-gated. Store the owner approval receipt for a
template version, then record the send event against the pipeline row:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts approve-outreach-template \
  --template-version-ref business.outreach.agent_readiness_ecommerce.report_led.v1 \
  --approval-receipt-ref receipt.owner.template_approval.example \
  --approved-by-ref owner.openagents
bun apps/openagents.com/scripts/operator-business-pipeline.ts record-outreach-send \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --draft-ref business.outreach.draft.example \
  --mailbox-ref mailbox.operator.apollo \
  --source-ref apollo.sequence.agent_readiness_ecommerce \
  --approval-receipt-ref receipt.owner.template_approval.example
```

The send row carries `sourceRef`, `mailboxRef`, approval receipt, template
version, and `receipt.business.outreach_send.*`; that receipt is appended to the
pipeline row's `receiptRefs`. The default and maximum configured mailbox cap is
95 sends/day, so a route-level refusal stops over-volume recording before it can
be reported as delivered.
