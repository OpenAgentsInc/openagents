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

  test('keeps fleet mutations rollback-required and ineffective without operator approval', () => {
    const pendingFleetMutation = new ArtanisApprovalGateRecord({
      ...approvedGate,
      actionRef: 'action.public.artanis.fleet_mutation.quarantine_replica',
      authorityReceiptRefs: [],
      authoritySourceKinds: ['operator_policy'],
      caveatRefs: [
        'caveat.public.fleet_mutation_not_execution_authority',
        'caveat.public.replica_quarantine_requires_operator_approval',
      ],
      gateRef: 'gate.public.artanis.fleet_mutation_quarantine_pending',
      idempotencyKey: 'artanis-approval:fleet-mutation-quarantine:v1',
      kind: 'fleet_mutation',
      operatorReceiptRefs: [
        'receipt.operator.artanis.open_fleet_mutation_review',
      ],
      policyRefs: [
        'policy.public.artanis.fleet_mutation_requires_operator_approval',
      ],
      publicStatusRefs: [
        'approval.public.artanis.fleet_mutation_quarantine_pending',
      ],
      resolvedAtIso: null,
      rollbackPosture: 'rollback_plan_recorded',
      rollbackRefs: [
        'rollback.public.artanis.restore_replica_routing_eligibility',
      ],
      sourceRefs: [
        'health.public.artanis.glm_fleet',
        'scheduler.public.glm_external_wins_proof',
      ],
      state: 'pending',
      updatedAtIso: '2026-06-07T02:18:00.000Z',
    })
    const missingRollback = new ArtanisApprovalGateRecord({
      ...pendingFleetMutation,
      gateRef: 'gate.public.artanis.fleet_mutation_missing_rollback',
      idempotencyKey: 'artanis-approval:fleet-mutation-missing-rollback:v1',
      rollbackPosture: 'rollback_not_applicable',
      rollbackRefs: [],
    })

    expect(() =>
      projectArtanisApprovalGateLedger(
        ledgerWithGate(missingRollback),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisApprovalGateUnsafe)

    const operator = projectArtanisApprovalGateLedger(
      ledgerWithGate(pendingFleetMutation),
      'operator',
      nowIso,
    )
    const publicArtanis = projectArtanisApprovalGateLedger(
      ledgerWithGate(pendingFleetMutation),
      'public_artanis',
      nowIso,
    )

    expect(ARTANIS_RISKY_ACTION_KINDS).toContain('fleet_mutation')
    expect(artanisApprovalGateEffective(pendingFleetMutation, nowIso)).toBe(
      false,
    )
    expect(operator.gates[0]).toMatchObject({
      effective: false,
      kind: 'fleet_mutation',
      rollbackPosture: 'rollback_plan_recorded',
      rollbackRefs: [
        'rollback.public.artanis.restore_replica_routing_eligibility',
      ],
      state: 'pending',
    })
    expect(publicArtanis.gates[0]).toMatchObject({
      effective: false,
      kind: 'fleet_mutation',
      rollbackRefs: [],
      state: 'pending',
    })
    expect(artanisApprovalGateProjectionHasPrivateMaterial(publicArtanis)).toBe(
      false,
    )
  })
})
