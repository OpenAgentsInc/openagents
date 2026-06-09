import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PublicClaimUpgradeReceiptProjection,
  PublicClaimUpgradeReceiptUnsafe,
  createPublicClaimUpgradeReceipt,
  projectPublicClaimUpgradeReceipt,
  publicClaimUpgradeReceiptHasPrivateMaterial,
  resolvePublicClaimUpgradeReceipt,
  type PublicClaimUpgradeRequest,
} from './public-claim-upgrade-receipts'

const baseRequest: PublicClaimUpgradeRequest = {
  approverRefs: ['operator_ref.public_claim.review_otec'],
  claimId: 'claim_otec_site_deployment',
  claimKind: 'deployment',
  claimRef: 'claim.otec.site_deployment',
  createdAt: '2026-06-06T19:00:00.000Z',
  evidenceRefs: [
    {
      evidenceKind: 'verification',
      evidenceRef: 'receipt:deployment:otec:v3',
    },
  ],
  idempotencyKey: 'idem_claim_otec_verified_v1',
  previousState: 'measured',
  requestedState: 'verified',
  sourceAuthorityRefs: [
    'source.public_claim.otec.deployment',
    'operator_ref.public_claim.policy',
  ],
}

describe('public claim upgrade receipts', () => {
  test('accepts upgrades when required evidence and approval refs exist', () => {
    const receipt = createPublicClaimUpgradeReceipt(baseRequest)
    const publicProjection = projectPublicClaimUpgradeReceipt(receipt, 'public')
    const operatorProjection = projectPublicClaimUpgradeReceipt(
      receipt,
      'operator',
    )

    expect(S.decodeUnknownSync(PublicClaimUpgradeReceiptProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(receipt).toMatchObject({
      missingEvidenceRefs: [],
      nextState: 'verified',
      previousState: 'measured',
      requestedState: 'verified',
      status: 'accepted',
    })
    expect(receipt.requiredEvidenceRefs).toEqual([
      'required.verification_evidence',
      'required.operator_approval_ref',
    ])
    expect(publicProjection.approverRefs).toEqual([])
    expect(publicProjection.sourceAuthorityRefs).toEqual([
      'source.public_claim.otec.deployment',
    ])
    expect(operatorProjection.approverRefs).toEqual(baseRequest.approverRefs)
    expect(operatorProjection.sourceAuthorityRefs).toEqual(
      [...baseRequest.sourceAuthorityRefs].sort(),
    )
    expect(publicClaimUpgradeReceiptHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('blocks direct verified upgrade without verification and approval refs', () => {
    const receipt = createPublicClaimUpgradeReceipt({
      ...baseRequest,
      approverRefs: [],
      evidenceRefs: [],
      previousState: 'planned',
      requestedState: 'verified',
    })

    expect(receipt).toMatchObject({
      nextState: 'planned',
      previousState: 'planned',
      requestedState: 'verified',
      status: 'blocked',
    })
    expect(receipt.missingEvidenceRefs).toEqual([
      'required.verification_evidence',
      'required.operator_approval_ref',
    ])
    expect(receipt.denialRefs).toEqual([
      'denial.missing.operator_approval_ref',
      'denial.missing.verification_evidence',
    ])
  })

  test('keeps accepted-work settlement separate from buyer payment and Site checkout evidence', () => {
    const blocked = createPublicClaimUpgradeReceipt({
      ...baseRequest,
      evidenceRefs: [
        {
          evidenceKind: 'buyer_payment',
          evidenceRef: 'receipt:buyer_payment:otec:checkout',
        },
        {
          evidenceKind: 'site_checkout',
          evidenceRef: 'receipt:site_checkout:otec:hosted',
        },
      ],
      previousState: 'verified',
      requestedState: 'settled',
    })
    const accepted = createPublicClaimUpgradeReceipt({
      ...baseRequest,
      evidenceRefs: [
        {
          evidenceKind: 'accepted_work_settlement',
          evidenceRef: 'receipt:accepted_work_settlement:otec:provider_1',
        },
      ],
      previousState: 'verified',
      requestedState: 'settled',
    })

    expect(blocked.status).toBe('blocked')
    expect(blocked.nextState).toBe('verified')
    expect(blocked.missingEvidenceRefs).toEqual([
      'required.accepted_work_settlement_receipt',
    ])
    expect(blocked.denialRefs).toEqual([
      'denial.missing.accepted_work_settlement_receipt',
      'denial.settlement_requires_accepted_work_receipt',
    ])
    expect(accepted.status).toBe('accepted')
    expect(accepted.nextState).toBe('settled')
  })

  test('replays existing receipts by idempotency key', () => {
    const firstReceipt = createPublicClaimUpgradeReceipt(baseRequest)
    const replay = resolvePublicClaimUpgradeReceipt({
      ...baseRequest,
      evidenceRefs: [],
      requestedState: 'settled',
    }, [firstReceipt])

    expect(replay).toEqual(firstReceipt)
  })

  test('rejects private workroom, provider, wallet, raw payment, and customer material', () => {
    expect(() =>
      createPublicClaimUpgradeReceipt({
        ...baseRequest,
        evidenceRefs: [
          {
            evidenceKind: 'verification',
            evidenceRef: 'raw_runner_payload:abc',
          },
        ],
      }),
    ).toThrow(PublicClaimUpgradeReceiptUnsafe)
    expect(() =>
      createPublicClaimUpgradeReceipt({
        ...baseRequest,
        sourceAuthorityRefs: ['provider_grant:abc'],
      }),
    ).toThrow(PublicClaimUpgradeReceiptUnsafe)
    expect(() =>
      createPublicClaimUpgradeReceipt({
        ...baseRequest,
        evidenceRefs: [
          {
            evidenceKind: 'verification',
            evidenceRef: 'lnbc1rawinvoice',
          },
        ],
      }),
    ).toThrow(PublicClaimUpgradeReceiptUnsafe)
    expect(() =>
      createPublicClaimUpgradeReceipt({
        ...baseRequest,
        approverRefs: ['ben@example.com'],
      }),
    ).toThrow(PublicClaimUpgradeReceiptUnsafe)
    expect(() =>
      createPublicClaimUpgradeReceipt({
        ...baseRequest,
        evidenceRefs: [
          {
            evidenceKind: 'verification',
            evidenceRef: 'workroom_private:order_otec',
          },
        ],
      }),
    ).toThrow(PublicClaimUpgradeReceiptUnsafe)
  })
})
