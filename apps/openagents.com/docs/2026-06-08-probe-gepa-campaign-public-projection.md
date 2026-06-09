# Probe GEPA Campaign Public Projection

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#508`.

OpenAgents product surface now has a typed public projection shape for Artanis to summarize Probe
GEPA benchmark campaigns from refs. The implementation lives in
`workers/api/src/probe-gepa-campaign-projection.ts`.

## Fields

The projection records the plan-requested campaign fields:

- `campaignRef`
- `objectiveRef`
- `stage`
- `claimState`
- `benchmarkSuiteRefs`
- `splitManifestRefs`
- `probeCommitRefs`
- `baselineCandidateRef`
- `activeCandidateRefs`
- `candidateHashRefs`
- `pylonBatchRefs`
- `plannedMetricCalls`
- `completedMetricCalls`
- `validMetricCalls`
- `invalidMetricCalls`
- `retainedResultRefs`
- `validationResultRefs`
- `holdoutResultRefs`
- `artifactManifestRefs`
- `receiptRefs`
- `costSummaryRefs`
- `resourceReceiptRefs`
- `policyFindingRefs`
- `blockerRefs`
- `promotionDecisionRefs`
- `nextActionRefs`

It also records `settlementReceiptRefs` so public Pylon work can be shown
without implying payout unless settlement evidence exists.

## Claim Boundary

Claim states are ref-gated:

- `measured_retained_smoke` and `retained_summary` require retained result refs.
- `validation_measured_only` requires validation result refs.
- `holdout_summary` requires holdout result refs.

The public summary distinguishes retained, validation, and holdout evidence
counts. It can show Pylon work as visible work evidence without implying payout.
Settled payout claims are allowed only when both public receipt refs and
settlement receipt refs are present.

## Redaction Boundary

Projection validation rejects raw prompts, raw traces, raw benchmark fixtures,
provider credentials, account refs, bearer material, wallet material,
invoices/preimages, private repo paths, local filesystem paths, raw logs, and
raw timestamps.

## Verification

Run:

```sh
bun run --cwd workers/api test -- probe-gepa-campaign-projection.test.ts
bun run --cwd workers/api typecheck
```
