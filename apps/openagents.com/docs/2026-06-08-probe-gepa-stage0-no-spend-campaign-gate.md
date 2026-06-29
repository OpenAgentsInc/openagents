# Probe GEPA Stage 0 No-Spend Campaign Gate

Date: 2026-06-08

Issue: [#565](https://github.com/OpenAgentsInc/openagents/issues/565)

## Launch Decision

GEPA Stage 0 may be marked green only as an unpaid multi-Pylon campaign smoke.
It is not a paid campaign, Terminal-Bench score, model-training run, runtime
candidate activation, payout claim, or settlement claim.

The launch predicate is:

1. Campaign ref is public-safe.
2. At least two distinct Pylon refs appear in coordinator imports.
3. Assignment refs exist.
4. At least one accepted closeout exists.
5. At least one rejected closeout exists.
6. Closeout refs exist.
7. Accepted closeout refs exist.
8. Rejected closeout refs exist.
9. Artifact refs exist.
10. Proof bundle refs exist.
11. Resource usage refs exist.
12. Verifier result refs exist.
13. Probe closeout import refs exist.
14. Psionic import dry-run refs exist.
15. Artanis public-safe summary refs exist.
16. Accepted imports are `unpaid_smoke`; rejected closeouts may be
    `rejected_no_pay`.
17. No import carries payment receipts, settlement receipts, payable claims, or
    settled-bitcoin claims.

Only after all predicates pass may a dashboard mark Stage 0 green.

## State Model

The gate projects two states:

- `blocked`: one or more Stage 0 predicates are missing.
- `green`: public-safe multi-Pylon Stage 0 evidence is complete.

`green` still keeps all paid and settlement claims blocked.

## Guards

The Stage 0 projection always blocks:

- payment and settlement copy
- public Terminal-Bench score copy
- model-training copy
- runtime candidate activation
- wallet spend and payout claims

Public Stage 0 refs must reject:

- raw benchmark fixtures or benchmark data
- raw prompts and traces
- provider payloads, grants, credentials, and secrets
- customer data
- wallet/payment material, payout targets, invoices, and preimages
- model weights
- private repo/source refs
- raw timestamps

## Coverage

Regression coverage lives in:

- `workers/api/src/probe-gepa-stage0-no-spend-campaign.test.ts`
- `workers/api/src/pylon-gepa-metric-call-assignments.test.ts`
- `workers/api/src/probe-gepa-campaign-projection.test.ts`

The tests cover:

- accepted and rejected unpaid closeouts generated through the assignment
  lifecycle
- multi-Pylon public-safe dashboard green projection
- blocked single-Pylon or missing Probe/Psionic import evidence
- paid/payable/settlement import rejection
- raw benchmark, model-training, wallet, and timestamp rejection
- private-material scan on the public bundle

## Current Gap

OpenAgents product surface now has the Stage 0 no-spend campaign gate and tests. This is still not a
live paid GEPA campaign. Paid and settled-bitcoin modes remain blocked until a
separate gate supplies payment receipts, settlement receipts, and paid-mode
policy evidence.
