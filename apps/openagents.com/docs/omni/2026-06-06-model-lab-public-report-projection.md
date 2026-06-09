# Model Lab Public Report Projection

Date: 2026-06-06

Issue: #385 / `OPENAGENTS-LAB-006`

Status: implemented as a read-only schema/projection contract in
`workers/api/src/omni-model-lab-report.ts`.

## Purpose

The Model Lab report projection aggregates retained failures, candidates, model
artifacts, training runs, Benchmark Cloud evidence, promotion decisions,
rollback posture, attribution, and marketplace memory into one audience-safe
report packet.

The report is for inspection. It can feed public proof, investor demos, agent
inspection, and operator/customer summaries. It cannot train models, run evals,
call providers, install adapters, export raw artifacts, publish the report,
spend money, promote runtime behavior, pay out, settle, or mutate public
claims.

## Sections

Every report must include one section for each of:

- retained failures,
- candidates,
- model artifacts,
- training runs,
- Benchmark Cloud evidence,
- promotion decisions,
- rollback,
- attribution,
- marketplace memory.

Each section records item refs, evidence refs, missing-evidence refs, blockers,
caveats, and a readiness label: `complete`, `partial`, `missing_evidence`, or
`blocked`.

## Validation Rules

- A report requires all required section kinds and cannot duplicate section
  kinds or section refs.
- Complete sections require item refs and evidence refs.
- Partial sections require items plus missing-evidence refs or caveats.
- Missing-evidence sections require missing-evidence refs.
- Blocked sections require blocker refs.
- Complete reports require all sections complete and no report-level missing
  evidence or blockers.
- Partial and missing-evidence reports require missing-evidence refs.
- Blocked reports require blocker refs.
- `promotion_passed_not_deployed` claim state requires promotion decision refs
  and no-deploy caveats.
- `no_public_claim` requires caveats.
- Raw prompts, source archives, raw datasets, provider payloads, raw artifacts,
  model weights, secrets, payment or wallet material, private repos, raw logs,
  raw traces, and raw timestamps are rejected.

## Projection

`projectOmniModelLabReport(report, audience, nowIso)` returns an
`OmniModelLabReportProjection` with:

- friendly time labels,
- report readiness and claim state,
- complete/partial/missing/blocked section counts,
- audience-specific redaction for public, agent, customer, team, and operator
  views,
- a redaction summary with redacted-ref count, redaction-policy refs, and
  withheld-class refs,
- hard false authority booleans for training launch, eval execution, provider
  calls, adapter install, raw artifact export, report publication, payment
  spend, runtime promotion, payout, settlement, and public-claim mutation.

## Public Proof And Demos

Public proof and investor demo bundles should consume this report as a
source-controlled, audience-safe summary. A report can say which evidence is
complete, partial, missing, or blocked. It cannot turn internal evidence into a
public claim by itself and cannot publish anything without a separate
authorized action.

## Tests

Coverage lives in `workers/api/src/omni-model-lab-report.test.ts`. The tests
cover:

- complete report projection,
- partial, missing-evidence, and blocked readiness labels,
- section validation and duplicate-section rejection,
- claim-state caveats,
- public redaction and redaction counts,
- rejection of unsafe material and false authority.
