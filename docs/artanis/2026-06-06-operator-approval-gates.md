# Artanis Operator Approval Gates

Date: 2026-06-06

Issue: #393 / `ARTANIS-008`

Status: implemented as a schema/projection contract in
`workers/api/src/artanis-approval-gates.ts`.

## Purpose

Artanis can observe Model Lab evidence, Pylon stats, Forum posts, retained
failures, and operator steering records, but those records are not authority to
spend money, redeem L402 challenges, call providers, launch training/evals,
install adapters, promote runtime behavior, deploy, settle, dispatch paid Pylon
jobs, or upgrade public claims.

This contract adds the first explicit approval-gate layer for those risky
actions.

## Risky Action Kinds

The v1 risky action set is:

- adapter install
- deployment
- eval launch
- L402 redemption
- provider call
- public claim upgrade
- Pylon job dispatch
- runtime promotion
- settlement
- training launch
- wallet spend

The public projection can name these typed action kinds, but it must not expose
private authority refs, operator receipts, wallet material, payment material,
provider credentials, raw logs, private customer data, raw prompts, private
repos, or raw timestamps.

## Gate States

The contract models:

- `pending`
- `approved`
- `denied`
- `expired`
- `superseded`

An approved gate is effective only when it has an operator approval source, at
least one authority receipt ref, has not expired, and has not been superseded.
Expired approvals project as expired even if the stored state was approved.

Resolved non-pending gates require a resolved timestamp. Superseded gates
require a replacement gate ref. Non-superseded gates cannot carry a replacement
gate ref.

## Required Evidence

Every gate requires:

- an operator receipt ref
- a policy ref
- a public-safe caveat ref
- a public-safe status ref
- a valid expiry
- a stable idempotency key

Approved gates additionally require:

- `operator_approval` as an authority source
- one or more authority receipt refs

Actions that can be rolled back or halted, such as adapter install, deployment,
eval launch, provider call, public claim upgrade, Pylon job dispatch, runtime
promotion, and training launch, must carry a rollback plan or rollback receipt
ref.

## Non-Authority Sources

The following records can inform an approval request but cannot approve a risky
action by themselves:

- Forum posts
- Model Lab records
- retained failures
- Pylon stats

They can become source refs, blockers, caveats, or operator next-action drafts.
They cannot become spend, provider, training, deployment, settlement, or public
claim authority without a separate operator approval and authority receipt.

## Public Boundary

Public `/artanis` and Forum projections expose only:

- gate refs
- action refs
- risk kind labels
- public status refs
- policy refs
- caveat refs
- source refs
- friendly display times

They omit:

- authority receipt refs
- authority source kinds
- operator receipt refs
- private evidence refs
- rollback refs
- effective gate refs
- supersession refs

The operator projection can inspect those refs after validation, but the gate
contract still does not execute the risky action. A later executor must consume
the approved gate and enforce its own target-specific authority checks.

## Tests

Coverage lives in `workers/api/src/artanis-approval-gates.test.ts`. It proves:

- the risky action set is enumerated;
- all gate states project correctly;
- public projections redact authority, private evidence, rollback, and
  operator refs;
- approved actions require explicit authority receipts, operator receipts,
  caveats, expiry, and rollback posture where applicable;
- Forum posts, Model Lab records, retained failures, and Pylon stats cannot
  approve risky actions by themselves;
- approved gates become ineffective after expiry.
