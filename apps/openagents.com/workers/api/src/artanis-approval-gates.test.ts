import { describe, expect, test } from 'vitest'

import {
  ARTANIS_RISKY_ACTION_KINDS,
  ArtanisApprovalGateLedgerRecord,
  ArtanisApprovalGateRecord,
  ArtanisApprovalGateUnsafe,
  artanisApprovalGateEffective,
  artanisApprovalGateProjectionHasPrivateMaterial,
  exampleArtanisApprovalGateLedger,
  projectArtanisApprovalGateLedger,
} from './artanis-approval-gates'

const nowIso = '2026-06-07T02:30:00.000Z'

const approvedGate =
  exampleArtanisApprovalGateLedger.gates.find(
    gate => gate.state === 'approved',
  )!

const ledgerWithGate = (
  gate: ArtanisApprovalGateRecord,
): ArtanisApprovalGateLedgerRecord =>
  new ArtanisApprovalGateLedgerRecord({
    ...exampleArtanisApprovalGateLedger,
    gates: [gate],
  })

describe('Artanis approval gates', () => {
  test('enumerates risky actions and models all approval states', () => {
    const operator = projectArtanisApprovalGateLedger(
      exampleArtanisApprovalGateLedger,
      'operator',
      nowIso,
    )
    const publicArtanis = projectArtanisApprovalGateLedger(
      exampleArtanisApprovalGateLedger,
      'public_artanis',
      nowIso,
    )
    const publicForum = projectArtanisApprovalGateLedger(
      exampleArtanisApprovalGateLedger,
      'public_forum',
      nowIso,
    )

    expect(operator.riskyActionKinds).toEqual(ARTANIS_RISKY_ACTION_KINDS)
    expect(operator.gates.map(gate => gate.state)).toEqual([
      'approved',
      'denied',
      'expired',
      'superseded',
      'pending',
    ])
    expect(operator.effectiveGateRefs).toEqual([
      'gate.public.artanis.pylon_job_dispatch_approved',
    ])
    expect(publicArtanis.effectiveGateRefs).toEqual([])
    expect(publicForum.effectiveGateRefs).toEqual([])
    expect(publicArtanis.gates[0]).toMatchObject({
      authorityReceiptRefs: [],
      authoritySourceKinds: [],
      effective: false,
      operatorReceiptRefs: [],
      privateEvidenceRefs: [],
      rollbackRefs: [],
      state: 'approved',
    })
    expect(publicArtanis.gates.map(gate => gate.label)).toEqual([
      'Approved by operator',
      'Denied by operator',
      'Expired approval',
      'Superseded approval',
      'Pending operator review',
    ])
    expect(artanisApprovalGateProjectionHasPrivateMaterial(publicArtanis)).toBe(
      false,
    )
    expect(artanisApprovalGateProjectionHasPrivateMaterial(publicForum)).toBe(
      false,
    )
    expect(JSON.stringify(publicArtanis)).not.toContain('receipt.operator')
    expect(JSON.stringify(publicArtanis)).not.toContain('evidence.private')
    expect(JSON.stringify(publicForum)).not.toContain('authority.public')
  })

  test('requires explicit authority, operator receipts, expiry, caveats, and rollback posture', () => {
    const missingAuthority = new ArtanisApprovalGateRecord({
      ...approvedGate,
      authorityReceiptRefs: [],
      gateRef: 'gate.public.artanis.missing_authority',
      idempotencyKey: 'artanis-approval:missing-authority:v1',
    })
    const missingOperatorReceipt = new ArtanisApprovalGateRecord({
      ...approvedGate,
      gateRef: 'gate.public.artanis.missing_operator_receipt',
      idempotencyKey: 'artanis-approval:missing-operator-receipt:v1',
      operatorReceiptRefs: [],
    })
    const missingRollback = new ArtanisApprovalGateRecord({
      ...approvedGate,
      gateRef: 'gate.public.artanis.missing_rollback',
      idempotencyKey: 'artanis-approval:missing-rollback:v1',
      rollbackPosture: 'rollback_not_applicable',
      rollbackRefs: [],
    })
    const invalidExpiry = new ArtanisApprovalGateRecord({
      ...approvedGate,
      expiresAtIso: 'not-a-date',
      gateRef: 'gate.public.artanis.invalid_expiry',
      idempotencyKey: 'artanis-approval:invalid-expiry:v1',
    })

    expect(() =>
      projectArtanisApprovalGateLedger(
        ledgerWithGate(missingAuthority),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisApprovalGateUnsafe)
    expect(() =>
      projectArtanisApprovalGateLedger(
        ledgerWithGate(missingOperatorReceipt),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisApprovalGateUnsafe)
    expect(() =>
      projectArtanisApprovalGateLedger(
        ledgerWithGate(missingRollback),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisApprovalGateUnsafe)
    expect(() =>
      projectArtanisApprovalGateLedger(
        ledgerWithGate(invalidExpiry),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisApprovalGateUnsafe)
  })

  test('rejects Forum, Model Lab, retained-failure, and Pylon stats as authority sources by themselves', () => {
    const sourceKinds = [
      'forum_post',
      'model_lab_record',
      'retained_failure',
      'pylon_stats',
    ] as const

    for (const sourceKind of sourceKinds) {
      const gate = new ArtanisApprovalGateRecord({
        ...approvedGate,
        authoritySourceKinds: [sourceKind],
        gateRef: `gate.public.artanis.${sourceKind}_only`,
        idempotencyKey: `artanis-approval:${sourceKind}-only:v1`,
      })

      expect(() =>
        projectArtanisApprovalGateLedger(
          ledgerWithGate(gate),
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisApprovalGateUnsafe)
    }
  })

  test('marks approved gates ineffective after expiry or supersession', () => {
    const expiredApproval = new ArtanisApprovalGateRecord({
      ...approvedGate,
      expiresAtIso: '2026-06-07T02:00:00.000Z',
      gateRef: 'gate.public.artanis.approved_but_expired',
      idempotencyKey: 'artanis-approval:approved-but-expired:v1',
    })
    const projection = projectArtanisApprovalGateLedger(
      ledgerWithGate(expiredApproval),
      'operator',
      nowIso,
    )

    expect(artanisApprovalGateEffective(expiredApproval, nowIso)).toBe(false)
    expect(projection.gates[0]).toMatchObject({
      effective: false,
      label: 'Expired approval',
      state: 'expired',
    })
  })
})
