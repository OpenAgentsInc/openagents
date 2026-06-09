# Public Claim Upgrade Receipts

This note documents the OPENAGENTS-062 claim-upgrade receipt contract.

Public OpenAgents surfaces should not move claims from planned or measured
states into verified or settled language just because a private workroom says
the work happened. A public claim upgrade must carry a typed receipt with the
previous state, requested state, resulting next state, required evidence, any
missing evidence, denial refs, source-authority refs, approver refs, and an
idempotency key.

## Runtime Shape

`workers/api/src/public-claim-upgrade-receipts.ts` defines:

- `PublicClaimUpgradeRequest`;
- `PublicClaimUpgradeReceipt`;
- `PublicClaimUpgradeReceiptProjection`;
- `createPublicClaimUpgradeReceipt`;
- `resolvePublicClaimUpgradeReceipt`; and
- `projectPublicClaimUpgradeReceipt`.

The pure creation function is intentionally independent of storage. A route or
repository can later persist receipts and pass existing rows into
`resolvePublicClaimUpgradeReceipt` to enforce replay-safe idempotency.

## Evidence Rules

The current upgrade requirements are:

| Requested state | Required authority or evidence |
| --- | --- |
| `modeled` | source-authority ref |
| `measured` | measurement evidence |
| `verified` | verification evidence and an operator approval ref |
| `settled` | accepted-work settlement evidence and an operator approval ref |

Buyer-payment evidence and Site-checkout evidence are intentionally separate
from accepted-work settlement evidence. They may prove that a buyer paid or a
checkout completed, but they do not prove provider/agent accepted-work
settlement. A `settled` upgrade remains blocked until an
`accepted_work_settlement` receipt exists.

## Projection Rules

Receipt records may include approver and source-authority refs. Public and
customer projections hide approver refs and operator/team-only source refs.
Team projections can see non-operator source refs. Operator projections can see
all safe refs.

No projection exposes the raw idempotency key. It exposes only an
`idempotencyKeyRef` derived from the receipt id.

## Redaction Boundary

The receipt layer rejects:

- private workroom refs;
- prompt logs and raw runner payload refs;
- provider account, provider grant, and token refs;
- API tokens, bearer strings, OAuth/cookie material, and private keys;
- wallet state, invoices, preimages, and raw payment refs; and
- customer private data, including email-shaped refs.

## Verification

Coverage lives in `workers/api/src/public-claim-upgrade-receipts.test.ts` and
checks allowed upgrades, blocked verified upgrades, buyer/Site payment evidence
not satisfying accepted-work settlement, idempotency replay, and redaction.
