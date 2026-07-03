# BF-8.6 Settlement Primitive Integration Design

Date: 2026-07-03
Status: deferred design + bounded demo receipt for issue #8114. No custody
changes. No live settlement arming. No public copy or promise-state flip.

Roadmap anchor: [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md), BF-8.6.
Governing frame: [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md), AW-0 services
engine. Execution rules: [`EXECUTION.md`](./EXECUTION.md).

## Scope

BF-8.6 is the custody-agnostic settlement primitive design for
release-on-verified-delivery under accepted outcomes. It is not a new money
rail. It describes how an external trustless escrow primitive can sit behind the
already modeled accepted-outcome lifecycle once an owner-armed custody path and
real receipt gate exist.

The current deliverable is intentionally bounded:

- Define the integration seam from accepted outcome to external escrow release.
- Prove the owed partner make-good shape with an inert demo receipt.
- Keep every receipt public-safe: vertical descriptors and opaque refs only.
- Leave all real money movement disabled until an owner gate arms a reviewed
  custody adapter.

## Existing Authority

OpenAgents already has two settlement-relevant surfaces that this design must
reuse:

- Labor escrow: a held claim on the existing credit ledger, with reserve,
  release, refund, and forfeit guarded by requester or validator evidence.
- Accepted-outcome settlement projection:
  `GET /api/public/accepted-outcome/settlement/{economicsId}` projects the
  eight ordered settlement states for one accepted outcome and is inert by
  construction.

BF-8.6 must not add a parallel payout workflow. The external primitive is an
adapter behind the accepted-outcome settle step, not a second source of truth.

## Lifecycle Mapping

| Accepted-outcome state | External escrow responsibility | Authority |
| --- | --- | --- |
| `authorized` | Buyer or operator chooses the primitive and terms. | Owner or buyer authority, not worker authority |
| `paid` | Funds are locked by the external primitive or modeled as pending lock evidence. | External receipt plus OpenAgents payment receipt |
| `accepted` | The deliverable passes the accepted-outcome verifier or recorded human review gate. | Verifier/reviewer receipt |
| `pending_payout` | The accepted outcome becomes releasable, but no release has happened. | OpenAgents state machine only |
| `dispatched` | A reviewed adapter submits the release instruction. | Owner-armed adapter |
| `confirmed` | External primitive confirms the release/refund result. | External confirmation receipt |
| `reconciled` | OpenAgents reconciles the confirmation against the outcome, parties, and amount. | Ledger reconciliation |
| `margin` | Gross-margin and contributor accrual receipts are projected. | OpenAgents receipt projection |

The release trigger is `accepted`, never `delivered`. Draft delivery, worker
self-attestation, or a partner promise cannot release escrow without the
accepted-outcome verifier or explicitly recorded human review receipt.

## Adapter Contract

The adapter boundary should be small and idempotent:

```ts
type ReleaseOnVerifiedDeliveryAdapter = Readonly<{
  adapterKind: 'external_trustless_escrow'
  settlementIntentRef: string
  acceptedOutcomeRef: string
  verificationReceiptRef: string
  parties: Readonly<{
    buyerRef: string
    recipientRef: string
    platformRef?: string
  }>
  amountRef: string
  custodyPolicyRef: string
  idempotencyRef: string
}>
```

The adapter may return only public-safe refs to the normal settlement machine:

```ts
type ReleaseOnVerifiedDeliveryResult = Readonly<{
  result: 'released' | 'refunded' | 'blocked'
  externalReceiptRef: string
  externalConfirmationRef?: string
  blockerRefs: ReadonlyArray<string>
}>
```

Raw payment payloads, preimages, wallet material, client identities, local
operator notes, and provider secrets stay outside public receipts and outside
model context.

## Owed Partner Make-Good Demo

The make-good demo is an inert receipt over an opaque settlement-infrastructure
partner ref. It demonstrates the shape of the release-on-verified-delivery
settle step without moving money or arming custody.

Demo receipt:

```json
{
  "receiptKind": "bf_8_6_release_on_verified_delivery_demo",
  "schema": "openagents.bf_8_6.settlement_demo_receipt.v1",
  "issueRef": "github:OpenAgentsInc/openagents:issue:8114",
  "demoRef": "demo.public.bf_8_6.owed_partner_make_good.001",
  "acceptedOutcomeRef": "accepted_outcome.public.bf_8_6.make_good.001",
  "economicsId": "bf_8_6_make_good_demo_001",
  "verificationReceiptRef": "receipt.public.bf_8_6.verification.make_good.001",
  "settlementProjectionRoute": "/api/public/accepted-outcome/settlement/bf_8_6_make_good_demo_001",
  "externalPrimitive": {
    "kind": "trustless_escrow",
    "custody": "external",
    "custodyAgnostic": true,
    "adapterArmed": false
  },
  "states": [
    "authorized",
    "paid",
    "accepted",
    "pending_payout",
    "dispatched",
    "confirmed",
    "reconciled",
    "margin"
  ],
  "moneyMovement": "none",
  "liveCustodyChanged": false,
  "publicSafety": {
    "clientIdentifyingInfo": false,
    "rawPaymentMaterial": false,
    "walletMaterial": false,
    "opaquePartnerRefsOnly": true
  },
  "blockerRefs": [
    "blocker.bf_8_6.owner_custody_adapter_not_armed",
    "blocker.bf_8_6.real_external_receipt_missing",
    "blocker.bf_8_6.owner_signed_green_transition_missing"
  ]
}
```

This is a working demo receipt only because the existing public settlement
projection can dereference the same `economicsId` shape in tests and prove the
ordered states, live-at-read staleness contract, and `movedMoney=false`
discipline. It is not a payment receipt, not a settlement receipt, and not a
promise-green receipt.

## Gates Before Live Custody

BF-8.6 may move beyond demo only when all gates are satisfied:

- A real accepted outcome exists with a verifier or human-review receipt.
- The external primitive has a reviewed adapter contract and idempotency policy.
- The owner signs the custody adapter arm, caps, and rollback procedure.
- The public projection exposes only refs, states, caveats, and confirmation
  labels, never raw payment material.
- `payments.accepted_outcome_economics.v1` remains red until a real
  money-moving settlement receipt and owner-signed transition exist.

Until then, this lane remains design-complete, demo-proven, and custody-inert.
