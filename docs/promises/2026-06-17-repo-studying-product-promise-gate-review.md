# Repo Studying Product-Promise Gate Review

Date: 2026-06-17
Status: yellow internal-dogfood gate review
Promise id: `autopilot.repo_study_packets.v1`

## Decision

OpenAgents can say it is dogfooding public, refs-only StudyBench study packets
on its own public repository to improve OpenAgents-codebase work.

That claim stays yellow and internal-dogfood scoped. It does not authorize copy
that says OpenAgents has a trained repo expert, live customer repo studying, a
marketplace package, payout eligibility, or automatic paid work from StudyBench
rows or study packets.

## Evidence Refs

- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`
- `docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md`
- `docs/research/machine-studying/openagents-studybench/private-boundary.md`
- `docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md`
- `docs/research/machine-studying/openagents-studybench/runs/2026-06-17-mvp-14-baseline-packet-gepa-comparison.md`
- `packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json`
- `promise:repo.open_source_code_map.v1`

## Public-Safe Copy

OpenAgents is dogfooding source-grounded study packets on its own public repo.
The current evidence is internal OpenAgents lift only. Customer repo studying,
marketplace packaging, payout eligibility, and paid work remain separately
gated.

## Blocked Copy

Do not say or imply:

- OpenAgents has a trained repo expert.
- Customer repo studying is live.
- Study packets are a marketplace package.
- StudyBench rows or study packets make anyone payout eligible.
- StudyBench rows or study packets are automatically paid work.
- The recorded MVP comparison is customer validation.

## Required Gates Before Broad Product Copy

- Customer-data privacy review and redaction policy.
- Customer repo ingestion boundary and source-authority contract.
- Private validation and holdout split discipline with leak response.
- Marketplace package policy and conformance tests.
- Usage metering and exact usage-subject refs.
- Pricing, refund, dispute, and package entitlement policy.
- Payout eligibility policy.
- Settlement receipts for any paid package or accepted work claim.
- Product-promise preflight before public copy changes state.

## SA-5 Gate Matrix (#5342)

SA-5 advances only by clearing these gates with dereferenceable evidence. Until
then, `autopilot.repo_study_packets.v1` remains yellow for internal
OpenAgents-repo dogfood only.

| Capability | Required evidence before claim | Current state |
| --- | --- | --- |
| Customer repo studying | Customer consent and workspace authority refs; a tenant-scoped corpus manifest; source-ingestion and deletion/export policy; redaction report; private-material boundary tests; holdout validation showing lift on customer-safe tasks; product-promise transition receipt. | Blocked. No customer repo ingestion or public customer studying claim. |
| Marketplace study package | Package schema; conformance tests; package review/approval refs; entitlement and usage-metering refs; pricing, refund, and dispute policy; public listing copy reviewed against this promise gate. | Blocked. Study packets are not marketplace packages. |
| Repo-expert language | Evaluation report from private validation and holdout tasks; freshness and source-authority refs; failure analysis; caveat copy that distinguishes packet-assisted retrieval from a trained model/expert. | Blocked. Current evidence is internal OpenAgents-codebase lift only. |
| Payout eligibility | A separate work request or funded receipt; accepted-work evidence; independent verification; payout target approval; settlement approval; public-safe settlement receipt. Study packets or StudyBench rows alone must not create payout eligibility. | Blocked except where another already-green labor/debt-receipt rail independently supplies the payout authority. |
| Automatic paid work | Assignment/claim route, budget cap, worker/reviewer/settlement role split, idempotent settlement, reconciliation, and public receipt projection for the actual work product. | Blocked. A study packet may be evidence/context only. |

Closeout for #5342 is gate definition, not a green-claim transition. Any future
state change must update this review, the registry entry, and the relevant
tests or receipt docs in the same change.

## Authority Boundary

StudyBench rows and study packets are evidence and repository-memory inputs.
They are not runtime mutation authority, customer source-ingestion authority,
marketplace listing authority, billing authority, payout authority, settlement
authority, or public green-claim authority.

Public projections must link to refs only. They must not expose private row
bodies, hidden rubrics, hidden gold answers, private evidence spans, raw repo
archives, local paths, raw scorer rationale, secrets, provider payloads, wallet
material, payment material, or customer-sensitive content.

## Verification

- `apps/openagents.com/workers/api/src/product-promises.test.ts`
- Product-promise docs review.
- Machine-studying docs review.
- Blueprint marketplace boundary review.
