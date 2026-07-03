# QA Swarm at Khala Code: Customer-One Standing Engagement

Date: 2026-07-02
Status: public-safe case-study seed and weekly-report contract for issue #8066.
This document does not flip a product promise green, publish a price, or claim
hosted QA Swarm self-service is generally available.

## Stable Report URL

The customer-one engagement report is the QA Swarm run projection for Khala Code:

- Stable share URL: `/qa/qa-run.khala-code-nightly.latest`
- Seed snapshot alias: `/qa/qa-run.khala-code-nightly.2026-07-02`
- Projection schema: `openagents.qa_swarm.run_projection.v1`
- Source artifact: `artifact.khala_code.qa_status_surface.latest`
- Weekly report ref: `artifact.qa_swarm.weekly_report.khala_code.latest`

The latest URL is the stable link for the standing engagement. The dated alias is
the public-safe seed snapshot from the 2026-07-02 audit session. Future nightly
or weekly automation may refresh the latest projection, but it must keep the same
redaction and evidence rules: every visible count comes from the nightly status
surface, trace/video receipts, coverage frontier, perf budgets, filed issue refs,
or distilled regression refs.

## Engagement Contract

QA Swarm customer one is Khala Code Desktop. The engagement packages the existing
Khala Code QA loop as a weekly report:

- Run the Khala Code nightly QA matrix and merge its public-safe artifacts into
  the Q1.5 status surface.
- Publish a QA Swarm run projection at the stable share URL.
- Carry a findings ledger with the lifecycle `caught -> filed -> fixed ->
  distilled`.
- Link only dereferenceable public-safe receipts: artifact refs, trace refs,
  coverage/frontier refs, perf refs, test refs, and strict issue refs.
- Carry behavior-contract registry status: the enforced contracts for the
  surface (ids + statements from
  `clients/khala-code-desktop/src/contracts/ux-contracts.ts`, human doc
  `docs/khala-code/khala-code-ux-contract.md`) with the latest sweep verdict
  per contract. The nightly matrix writes the receipt set at
  `behavior-contracts/behavior-contract-receipts.json` and mirrors the latest
  pass/fail counts plus failed contract ids into `qa-status-surface.json` under
  `behaviorContracts`. The oracles run in the nightly matrix (desktop `verify`
  step plus the dedicated `behavior-contracts` step), so contract status
  follows the same evidence rules as every other count. This is the
  customer-one instance of the invariant catalog the QA Swarm sells — see
  `docs/fable/2026-07-03-behavior-contracts-and-customer-invariants.md`.
- Keep raw logs, local paths, account identifiers, provider payloads, customer
  records, and private traces out of the projection.

The report is allowed to say what the evidence shows for Khala Code. It is not
allowed to imply third-party hosted runs, pricing, settlement, or self-serve
availability before those lanes pass their own gates.

## Case-Study Seed: 2026-07-02 Audit Session

The first useful evidence came before the standing loop was fully automated. On
2026-07-02, the QA framework parts were run against Khala Code and immediately
found three concrete items:

| Finding | Lifecycle state | Public-safe receipt |
| --- | --- | --- |
| Fleet-run RPC visual smoke stale fixture | fixed | `artifact.qa_swarm.finding.visual_fleet_run_rpc.20260702` |
| Foldkit cockpit landing visual smoke stale fixture | fixed | `artifact.qa_swarm.finding.foldkit_cockpit_visual.20260702` |
| Cockpit blanks when one startup RPC fails | filed | `artifact.qa_swarm.finding.cockpit_failed_rpc_blank.20260702` |

That is the honest case-study claim: the first audit pass caught two
main-branch regression-test gaps and one product robustness bug. The two stale
visual-smoke gaps were fixed in the audit landing commits recorded by
`ROADMAP_QA.md`; the cockpit degradation remains a tracked product fix in the
QA roadmap until its regression scenario lands.

## Findings Ledger

Current seed counts:

| Metric | Count | Meaning |
| --- | ---: | --- |
| Caught | 3 | Observed findings from the audit session |
| Filed | 3 | Findings represented by strict issue or roadmap refs |
| Fixed | 2 | Findings already closed by committed fixes |
| Distilled regressions | 1 | Deterministic regression coverage represented in the seed projection |

The ledger is intentionally conservative. A finding only moves forward when the
next receipt exists. Counter movement, screenshots without trace refs, or agent
summaries without artifacts do not advance a row.

## Copy Gate

Allowed public wording:

> QA Swarm's customer-one engagement is Khala Code itself. The weekly report
> shows the current nightly verdicts, coverage frontier, perf-budget status,
> findings ledger, videos, traces, and distilled regression refs at a stable
> share URL.

Disallowed until separately gated:

- "QA Swarm is generally available."
- "Hosted QA Swarm runs are self-serve."
- "The weekly report proves every Khala Code workflow is covered."
- Any price, SLA, settlement, or third-party customer claim.

## Verification

For this seed, the implementation evidence is:

- Web route: `/qa/qa-run.khala-code-nightly.latest`
- Projection test: `apps/openagents.com/apps/web/src/page/qa-swarm.test.ts`
- Full deploy gate for the issue PR: `bun run check:deploy`
