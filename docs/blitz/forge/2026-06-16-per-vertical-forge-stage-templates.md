# Per-Vertical Forge Stage Templates

Date: 2026-06-16
Status: Locked template spec for #5101
Related: #5088, #5090, #5099, #5100, #5102, #5105

## Purpose

The Forge factory dashboard uses one canonical production line:

1. Signal
2. Triage
3. Code Gen
4. Validate
5. Release
6. Document
7. Monitor
8. Deploy

That sequence is literal for software work, but the first design-partner
verticals need the same factory semantics mapped into their own work products.
This document locks those mappings so prefilled workspaces and dashboard metrics
can use one shared structure without pretending every vertical is writing code.

## Shared Contract

Every vertical template preserves the same operational meaning:

| Forge stage | Cross-vertical meaning                                                       | Required evidence                                                                            |
| ----------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Signal      | A qualified intent, problem, opportunity, or request enters the factory.     | Source refs, requester/referrer, received timestamp, scope caveats.                          |
| Triage      | The request is classified, scoped, prioritized, and assigned to a work lane. | Stage decision, owner/agent lane, blockers, acceptance criteria.                             |
| Code Gen    | The primary artifact is drafted or transformed.                              | Draft artifact ref, tool/run refs, prompt or brief summary, cost/usage refs where available. |
| Validate    | The artifact is checked against explicit criteria before external use.       | Verification refs, reviewer decision refs, failing criteria if any.                          |
| Release     | A candidate version is accepted for the target audience/channel.             | Accepted version ref, release decision, rollback caveat.                                     |
| Document    | The work is made understandable and reusable.                                | Summary, source map, change notes, handoff checklist, customer-safe explanation.             |
| Monitor     | Outcomes, defects, risk, and follow-up signals are observed.                 | Outcome refs, incident/feedback refs, metric window, freshness timestamp.                    |
| Deploy      | The approved artifact is applied to the intended system/channel.             | Delivery receipt, public/private visibility, settlement or no-settlement caveat.             |

The current `/forge` dashboard from #5088 already renders these stages with
honest `live` vs `seeded` provenance. Vertical templates must keep that
discipline: if a stage metric is not backed by a real source for that vertical,
show it as planned/seeded/absent rather than live.

## Software Factory Baseline

This is the existing coding template and the reference for all mapped variants.

| Forge stage | Software-factory name | Input                                                 | Output                                               | Primary automation                                                   |
| ----------- | --------------------- | ----------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------- |
| Signal      | Signal                | Issue, forum report, customer request, operator goal. | Work-order candidate.                                | Intake classifier, duplicate detector, scope finder.                 |
| Triage      | Triage                | Work-order candidate plus repo/product context.       | Scoped work order with acceptance criteria.          | Repo placement, priority, dependency, and blocker analysis.          |
| Code Gen    | Code Gen              | Scoped work order and repo snapshot.                  | Patch, branch, artifact, or run output.              | Coding agent run, tool calls, file edits, local checks.              |
| Validate    | Validate              | Patch/artifact and acceptance criteria.               | Verification result and review decision.             | Typecheck, tests, smoke, diff review, policy checks.                 |
| Release     | Release               | Verified candidate.                                   | Accepted delivery record.                            | Commit/push/release writer, changelog draft, receipt creation.       |
| Document    | Document              | Accepted delivery and evidence.                       | User-facing summary and operator/auditor notes.      | Summary writer, docs updater, issue/Forum commenter.                 |
| Monitor     | Monitor               | Live artifact and post-release signals.               | Incident, regression, metric, or improvement signal. | Runtime/projection checks, forum/issue feedback scan, metric rollup. |
| Deploy      | Deploy                | Accepted release package.                             | Live system state or published artifact.             | Worker/npm/desktop/mobile deploy runner, smoke verifier.             |

## E-Commerce Template

Use for #5099: inventory-aware campaign starters, storefront merchandising,
promotion pages, transactional content, and conversion-loop work.

Implementation note: #5099 now has a typed seed template at
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`
and the operator/audit note at
`docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md`.

| Forge stage | E-commerce stage name | Input                                                                                       | Output                                                                             | Validation and evidence                                               |
| ----------- | --------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Signal      | Demand Signal         | Product catalog delta, inventory pressure, seasonal event, abandoned-cart or search signal. | Campaign/opportunity brief.                                                        | Source refs for catalog, inventory, margin, traffic, and timing.      |
| Triage      | Offer Triage          | Opportunity brief plus constraints.                                                         | Prioritized offer plan with audience, SKU set, channel, and excluded claims.       | Inventory availability, margin guard, brand/legal caveats.            |
| Code Gen    | Campaign Build        | Offer plan.                                                                                 | Landing copy, email/SMS copy, ad creative brief, product-page edits, bundle rules. | Draft refs, SKU refs, channel refs, cost/usage refs.                  |
| Validate    | Commerce QA           | Campaign artifacts.                                                                         | QA result: offer math, links, stock, taxes/shipping caveats, claim compliance.     | Link checks, price/margin math, inventory sufficiency, policy review. |
| Release     | Merchandising Release | QA-passed campaign candidate.                                                               | Approved campaign version.                                                         | Release decision, scheduled window, rollback note.                    |
| Document    | Merchant Handoff      | Approved campaign.                                                                          | Merchant summary, channel checklist, product/source map, support notes.            | Customer-safe copy, included/excluded SKUs, measurement plan.         |
| Monitor     | Conversion Watch      | Live campaign and event data.                                                               | Conversion, revenue, refund, stockout, complaint, and channel-performance signals. | Metric window, attribution caveat, freshness timestamp.               |
| Deploy      | Channel Publish       | Approved campaign.                                                                          | Published storefront/email/ad/channel update.                                      | Delivery receipt, URL/message refs, scheduled or sent status.         |

Metric mapping:

- Throughput: accepted campaign artifacts or published channel updates per
  period.
- Cycle time: demand signal received to channel publish.
- Pass rate: Commerce QA passed / total campaign candidates.
- Token efficiency: accepted campaign artifacts per model-token or credit spend,
  with power/kWh only when backed by a measured or estimated compute source.
- MTTR: time from campaign defect, bad link, stockout, or policy issue to fixed
  deployment.
- Backlog: triaged offers not yet built or released.

## Legal Template

Use for #5100: forms/intake copilot starter, clause/checklist assistance,
regulated customer intake, and non-advice document workflow support.

Safety boundary: this template supports workflow assistance and document
preparation. It does not turn Forge into legal counsel, does not issue legal
advice, and must route jurisdiction-sensitive or rights-impacting decisions to
a qualified human reviewer.

Implementation note: #5100 now has a typed seed template at
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`
and the operator/audit note at
`docs/blitz/forge/2026-06-16-legal-prefilled-workspace.md`.

| Forge stage | Legal stage name    | Input                                                                         | Output                                                                                  | Validation and evidence                                                                   |
| ----------- | ------------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Signal      | Matter Signal       | Intake form, document upload, client question, deadline, or workflow request. | Matter/workflow candidate.                                                              | Source refs, requester identity state, jurisdiction caveat, confidentiality class.        |
| Triage      | Intake Triage       | Matter candidate and available context.                                       | Scoped workflow with document type, jurisdiction caveat, reviewer lane, and risk flags. | Conflict/eligibility status if available, missing-info list, human-review requirement.    |
| Code Gen    | Draft Assembly      | Scoped workflow and source docs.                                              | Draft form, checklist, clause comparison, issue list, or intake summary.                | Draft ref, source citation map, assumptions list, model/tool refs.                        |
| Validate    | Legal Review Gate   | Draft assembly output.                                                        | Review result: safe to share, needs human review, blocked, or missing info.             | Citation coverage, redaction, jurisdiction warning, policy/risk flags, reviewer decision. |
| Release     | Client-Ready Packet | Review-passed candidate.                                                      | Approved packet for client/internal use.                                                | Version ref, reviewer acceptance, delivery caveats.                                       |
| Document    | Matter Handoff      | Approved packet and evidence.                                                 | Handoff memo, source map, next-step checklist, audit note.                              | Customer-safe explanation, retained assumptions, review trace.                            |
| Monitor     | Matter Follow-Up    | Delivered packet and client/reviewer feedback.                                | Follow-up tasks, defect/risk signals, deadline reminders.                               | Feedback refs, deadline refs, freshness timestamp.                                        |
| Deploy      | Secure Delivery     | Approved packet.                                                              | Secure share, CRM update, task creation, or document-system update.                     | Delivery receipt, visibility class, recipient/audience caveat.                            |

Metric mapping:

- Throughput: reviewer-accepted packets, summaries, or forms per period.
- Cycle time: matter signal to secure delivery or reviewer handoff.
- Pass rate: Legal Review Gate passed / total drafted candidates.
- Token efficiency: accepted packets per token/credit spend; never substitute
  token efficiency for legal correctness.
- MTTR: time from flagged defect, missing citation, bad redaction, or reviewer
  rejection to corrected packet.
- Backlog: triaged matters waiting on missing info, drafting, or review.

## Marketing-Agency Template

Use for #5102: brand landing/email deliverables, campaign calendars, client
operator-on-Autopilot lanes, and white-label execution.

Implementation note: #5102 now has a typed seed template at
`apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`
and the operator/audit note at
`docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md`.

| Forge stage | Agency stage name   | Input                                                                             | Output                                                                              | Validation and evidence                                       |
| ----------- | ------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Signal      | Client Signal       | Client brief, brand guideline, analytics issue, launch date, stakeholder comment. | Creative/opportunity brief.                                                         | Brief refs, brand-source refs, channel/audience caveats.      |
| Triage      | Creative Triage     | Client signal and constraints.                                                    | Scoped deliverable with audience, channel, tone, success metric, and approval lane. | Brand fit, priority, missing assets, stakeholder dependency.  |
| Code Gen    | Creative Production | Scoped deliverable.                                                               | Landing page, email, ad copy, social sequence, campaign plan, or asset brief.       | Draft refs, source/brand map, model/tool refs.                |
| Validate    | Brand QA            | Creative production candidate.                                                    | QA result: brand consistency, factuality, accessibility, link/CTA, and channel fit. | Brand checklist, accessibility/link checks, reviewer notes.   |
| Release     | Client Approval     | QA-passed candidate.                                                              | Client-ready or internally approved version.                                        | Approval receipt, revision caveat, scheduled window.          |
| Document    | Account Handoff     | Approved version and evidence.                                                    | Account-manager summary, edit rationale, schedule, measurement plan.                | Customer-safe summary, source map, owner/next-step refs.      |
| Monitor     | Campaign Watch      | Live/pending campaign data.                                                       | Performance signal, feedback, revision request, or optimization task.               | Metric window, attribution caveat, client feedback refs.      |
| Deploy      | Channel Launch      | Approved creative.                                                                | Published page/email/ad/social/calendar update.                                     | Delivery receipt, URL/message refs, scheduled or sent status. |

Metric mapping:

- Throughput: approved client deliverables or launched channel updates per
  period.
- Cycle time: client signal to client approval or channel launch.
- Pass rate: Brand QA passed / total creative candidates.
- Token efficiency: approved deliverables per token/credit spend, separated by
  customer vs white-label lane.
- MTTR: time from client revision, broken link, brand mismatch, or campaign
  issue to corrected version.
- Backlog: scoped creative work waiting on assets, production, QA, or approval.

## General Knowledge-Work Template

Use for generic research, analysis, operations, internal process improvement,
and business-support work that does not fit a named vertical yet.

| Forge stage | General stage name | Input                                                                              | Output                                                                               | Validation and evidence                                              |
| ----------- | ------------------ | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| Signal      | Work Signal        | Question, report, data source, operational blocker, meeting note, or task request. | Work brief.                                                                          | Source refs, requester/context, privacy and scope caveats.           |
| Triage      | Scope Triage       | Work brief and context.                                                            | Scoped task with expected artifact, evidence needs, and decision owner.              | Missing info, priority, blocker, privacy class.                      |
| Code Gen    | Artifact Draft     | Scoped task.                                                                       | Memo, spreadsheet, research brief, checklist, plan, script, or operational artifact. | Draft ref, source map, tool/model refs.                              |
| Validate    | Evidence Review    | Draft artifact.                                                                    | Verification result and decision: accepted, revise, blocked, or escalate.            | Source coverage, calculation checks, reviewer notes, policy caveats. |
| Release     | Decision Packet    | Review-passed artifact.                                                            | Accepted decision-ready artifact.                                                    | Version ref, acceptance record, caveats.                             |
| Document    | Knowledge Handoff  | Accepted artifact.                                                                 | Summary, reusable notes, source map, next actions, retention note.                   | Customer-safe/internal-safe split, source trace.                     |
| Monitor     | Outcome Follow-Up  | Delivered artifact and downstream effects.                                         | Feedback, stale-data signal, task completion, or improvement.                        | Feedback refs, freshness timestamp, metric window.                   |
| Deploy      | Workflow Update    | Accepted artifact.                                                                 | Shared doc, ticket update, CRM/task update, dashboard entry, or operational change.  | Delivery receipt, visibility class, affected-system refs.            |

Metric mapping:

- Throughput: accepted artifacts or workflow updates per period.
- Cycle time: work signal to accepted decision packet or workflow update.
- Pass rate: Evidence Review accepted / total artifact drafts.
- Token efficiency: accepted artifacts per token/credit spend.
- MTTR: time from stale, incorrect, or rejected artifact to corrected output.
- Backlog: triaged work waiting on input, drafting, review, or deployment.

## Template Selection Rules

1. Prefer the most specific vertical when a request has a clear domain:
   e-commerce, legal, or marketing-agency.
2. Use General Knowledge Work for mixed or unknown work until a repeated pattern
   deserves its own named template.
3. Preserve the canonical Forge stage keys internally. Display names may vary by
   vertical, but metric joins and dashboard filters should still use the shared
   keys: `signal`, `triage`, `codegen`, `validate`, `release`, `document`,
   `monitor`, `deploy`.
4. Never infer regulated authority from a template. Legal review, payment,
   deployment, settlement, payout, customer communication, or production writes
   still require the normal authority receipts for that surface.
5. A stage can be skipped only with an explicit skipped-stage receipt naming the
   reason. Hidden skips make metrics dishonest.
6. If a source is stale or seeded, surface that provenance in the workspace and
   dashboard before showing the number.

## Prefilled Workspace Requirements

Each vertical prefilled workspace should seed:

- The selected vertical template and canonical stage keys.
- A first task in Signal or Triage, not a fake completed pipeline.
- A visible expected-output contract for every stage that the initial workspace
  intends to use.
- A default metric mapping with `live`, `seeded`, or `absent` provenance.
- A public-safe handoff summary and an internal/operator evidence area.
- A blocker slot for missing authority: human review, customer approval,
  payment/credits, repository access, channel access, or deployment permission.

## Dashboard Instrumentation Notes

The #5088 dashboard currently has software-factory stage labels and a mix of
live and seeded values. The next instrumentation step can consume this document
as a display and aggregation map:

- Store `templateRef` on the workspace or work order.
- Store canonical `stageKey` for joins.
- Display `stageDisplayName` from the selected template.
- Keep metric definitions canonical and stage-keyed.
- Add vertical-specific secondary metrics only after a real source exists.

This keeps one Forge dashboard while letting the design-partner surfaces speak
in the vocabulary of their work.
