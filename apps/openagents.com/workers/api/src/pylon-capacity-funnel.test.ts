import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonCapacityFunnelAggregate,
  PylonCapacityFunnelAccountingProjection,
  PylonCapacityFunnelProjection,
  PylonCapacityFunnelUnsafe,
  accountPylonCapacityFunnel,
  aggregatePylonCapacityFunnel,
  examplePylonCapacityFunnelRecords,
  projectPylonCapacityFunnelRecord,
  pylonCapacityAccountingProjectionHasPrivateMaterial,
  pylonCapacityProjectionHasPrivateMaterial,
} from './pylon-capacity-funnel'

const nowIso = '2026-06-06T21:40:00.000Z'

describe('Pylon capacity funnel', () => {
  test('projects capacity records with public redaction and friendly times', () => {
    const [settled, dark] = examplePylonCapacityFunnelRecords()
    const settledPublic = projectPylonCapacityFunnelRecord(
      settled!,
      'public',
      nowIso,
    )
    const darkPublic = projectPylonCapacityFunnelRecord(dark!, 'public', nowIso)
    const darkOperator = projectPylonCapacityFunnelRecord(
      dark!,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonCapacityFunnelProjection)(settledPublic))
      .toEqual(settledPublic)
    expect(settledPublic).toMatchObject({
      acceptedCapacityClaimAllowed: true,
      nodeRef: 'node.public_demo_1',
      paidCapacityClaimAllowed: true,
      providerRef: 'provider.public_demo_1',
      rewardRefs: [],
      settledCapacityClaimAllowed: true,
      settlementRefs: [],
      stage: 'settled',
      stageLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(darkPublic.nodeRef).toBe('node.redacted')
    expect(darkPublic.providerRef).toBe('provider.redacted')
    expect(darkPublic.darkCapacityReasonRefs).toEqual([
      'dark_reason.missing_payout_target',
      'dark_reason.no_work_assigned',
    ])
    expect(darkOperator.nodeRef).toBe('node.private_demo_2')
    expect(darkOperator.providerRef).toBe('provider.private_demo_2')
    expect(JSON.stringify(settledPublic)).not.toContain(
      '2026-06-06T21:35:00.000Z',
    )
    expect(pylonCapacityProjectionHasPrivateMaterial(settledPublic)).toBe(false)
    expect(pylonCapacityProjectionHasPrivateMaterial(darkPublic)).toBe(false)
  })

  test('aggregates funnel counts and dark-capacity reasons', () => {
    const records = [
      ...examplePylonCapacityFunnelRecords(),
      {
        ...examplePylonCapacityFunnelRecords()[0]!,
        acceptanceRefs: [],
        artifactRefs: [],
        assignmentRefs: [],
        capacityRef: 'capacity.registered_only',
        eligibilityRefs: [],
        id: 'capacity_funnel_registered_only',
        rewardRefs: [],
        runRefs: [],
        settlementRefs: [],
        stage: 'registered' as const,
      },
      {
        ...examplePylonCapacityFunnelRecords()[0]!,
        acceptanceRefs: [],
        artifactRefs: [],
        assignmentRefs: ['assignment.capacity.assigned_1'],
        capacityRef: 'capacity.assigned_1',
        id: 'capacity_funnel_assigned_1',
        rewardRefs: [],
        runRefs: [],
        settlementRefs: [],
        stage: 'assigned' as const,
      },
    ]
    const aggregate = aggregatePylonCapacityFunnel(records, 'public', nowIso)

    expect(S.decodeUnknownSync(PylonCapacityFunnelAggregate)(aggregate))
      .toEqual(aggregate)
    expect(aggregate.totalCount).toBe(4)
    expect(aggregate.registeredCount).toBe(1)
    expect(aggregate.assignedCount).toBe(1)
    expect(aggregate.darkCount).toBe(1)
    expect(aggregate.settledCount).toBe(1)
    expect(aggregate.settledClaimAllowedCount).toBe(1)
    expect(aggregate.byDarkCapacityReason).toEqual([
      { count: 1, key: 'dark_reason.missing_payout_target' },
      { count: 1, key: 'dark_reason.no_work_assigned' },
    ])
    expect(aggregate.byStage).toEqual([
      { count: 1, key: 'assigned' },
      { count: 1, key: 'dark' },
      { count: 1, key: 'registered' },
      { count: 1, key: 'settled' },
    ])
  })

  test('accounts capacity funnel state, dark reasons, stale capacity, and visible settlement receipts', () => {
    const records = [
      ...examplePylonCapacityFunnelRecords(),
      {
        ...examplePylonCapacityFunnelRecords()[0]!,
        acceptanceRefs: ['acceptance.capacity.paid_not_settled'],
        artifactRefs: ['artifact.capacity.paid_not_settled'],
        assignmentRefs: ['assignment.capacity.paid_not_settled'],
        capacityRef: 'capacity.paid_not_settled',
        eligibilityRefs: ['eligibility.capacity.paid_not_settled'],
        id: 'capacity_funnel_paid_not_settled',
        rewardRefs: ['reward.capacity.paid_not_settled'],
        runRefs: ['run.capacity.paid_not_settled'],
        settlementRefs: [],
        stage: 'paid' as const,
        updatedAtIso: '2026-06-04T21:35:00.000Z',
      },
      {
        ...examplePylonCapacityFunnelRecords()[1]!,
        capacityRef: 'capacity.dark_extra',
        caveatRefs: ['caveat.capacity.dark_extra_operator'],
        darkCapacityReasonRefs: [
          'dark_reason.no_work_assigned',
          'dark_reason.waiting_for_benchmark_review',
        ],
        evidenceRefs: ['evidence.capacity.dark_extra'],
        id: 'capacity_funnel_dark_extra',
        nodeRef: 'node.private_dark_extra',
        providerRef: 'provider.private_dark_extra',
      },
    ]
    const publicAccounting = accountPylonCapacityFunnel(
      records,
      'public',
      nowIso,
    )
    const operatorAccounting = accountPylonCapacityFunnel(
      records,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonCapacityFunnelAccountingProjection)(
      publicAccounting,
    )).toEqual(publicAccounting)
    expect(publicAccounting).toMatchObject({
      acceptedCount: 0,
      capacityAssignmentMutationAllowed: false,
      darkCount: 2,
      freshCount: 3,
      liveWalletSpendAllowed: false,
      paidButNotSettledCount: 1,
      paidCount: 1,
      payoutDispatchMutationAllowed: false,
      payoutTargetMutationAllowed: false,
      providerEligibilityMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      registeredCount: 0,
      settlementMutationAllowed: false,
      settledCount: 1,
      settledWithoutVisibleReceiptCount: 1,
      staleCapacityRefs: ['capacity.paid_not_settled'],
      staleCount: 1,
      totalCount: 4,
      unknownFreshnessCount: 0,
      updatedAtDisplay: '5 minutes ago',
      visibleSettlementClaimAllowedCount: 0,
    })
    expect(publicAccounting.byDarkCapacityReason).toEqual([
      {
        capacityRefs: ['capacity.pylon_demo_2'],
        caveatRefs: ['caveat.capacity.dark_until_routed'],
        count: 1,
        evidenceRefs: ['evidence.capacity.demo_2'],
        reasonRef: 'dark_reason.missing_payout_target',
        workClassRefs: ['work_class.flexible_inference'],
      },
      {
        capacityRefs: ['capacity.dark_extra', 'capacity.pylon_demo_2'],
        caveatRefs: [
          'caveat.capacity.dark_extra_operator',
          'caveat.capacity.dark_until_routed',
        ],
        count: 2,
        evidenceRefs: [
          'evidence.capacity.dark_extra',
          'evidence.capacity.demo_2',
        ],
        reasonRef: 'dark_reason.no_work_assigned',
        workClassRefs: ['work_class.flexible_inference'],
      },
      {
        capacityRefs: ['capacity.dark_extra'],
        caveatRefs: ['caveat.capacity.dark_extra_operator'],
        count: 1,
        evidenceRefs: ['evidence.capacity.dark_extra'],
        reasonRef: 'dark_reason.waiting_for_benchmark_review',
        workClassRefs: ['work_class.flexible_inference'],
      },
    ])
    expect(operatorAccounting.visibleSettlementClaimAllowedCount).toBe(1)
    expect(operatorAccounting.settledWithoutVisibleReceiptCount).toBe(0)
    expect(pylonCapacityAccountingProjectionHasPrivateMaterial(publicAccounting))
      .toBe(false)
    expect(JSON.stringify(publicAccounting)).not.toContain('2026-06-')
  })

  test('allows device capability thermal reason codes through the public funnel taxonomy', () => {
    const [dark] = examplePylonCapacityFunnelRecords().slice(1)
    const projection = projectPylonCapacityFunnelRecord(
      {
        ...dark!,
        darkCapacityReasonRefs: [
          'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
        ],
        evidenceRefs: ['receipt.cs336_a2.thermal.verified_row.1'],
      },
      'public',
      nowIso,
    )

    expect(projection.darkCapacityReasonRefs).toEqual([
      'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
    ])
    expect(pylonCapacityProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('requires evidence as capacity moves down the funnel', () => {
    const [settled] = examplePylonCapacityFunnelRecords()

    expect(() =>
      projectPylonCapacityFunnelRecord({
        ...settled!,
        benchmarkRefs: [],
        stage: 'benchmarked',
      }, 'operator', nowIso),
    ).toThrow(PylonCapacityFunnelUnsafe)
    expect(() =>
      projectPylonCapacityFunnelRecord({
        ...settled!,
        eligibilityRefs: [],
        stage: 'eligible',
      }, 'operator', nowIso),
    ).toThrow(PylonCapacityFunnelUnsafe)
    expect(() =>
      projectPylonCapacityFunnelRecord({
        ...settled!,
        runRefs: [],
        stage: 'running',
      }, 'operator', nowIso),
    ).toThrow(PylonCapacityFunnelUnsafe)
    expect(() =>
      projectPylonCapacityFunnelRecord({
        ...settled!,
        rewardRefs: [],
        stage: 'paid',
      }, 'operator', nowIso),
    ).toThrow(PylonCapacityFunnelUnsafe)
    expect(() =>
      projectPylonCapacityFunnelRecord({
        ...settled!,
        darkCapacityReasonRefs: [],
        stage: 'dark',
      }, 'operator', nowIso),
    ).toThrow(PylonCapacityFunnelUnsafe)
  })

  test('rejects raw host, private hardware, wallet, payment, provider, and runner material', () => {
    const [settled] = examplePylonCapacityFunnelRecords()

    for (const record of [
      { ...settled!, nodeRef: 'raw_host_identifier.chris_macbook' },
      { ...settled!, evidenceRefs: ['private_hardware_telemetry.gpu_serial'] },
      { ...settled!, evidenceRefs: ['wallet_state.local_node'] },
      { ...settled!, rewardRefs: ['payment_id.raw_123'] },
      { ...settled!, evidenceRefs: ['provider_token.codex'] },
      { ...settled!, runRefs: ['raw_runner_log.capacity'] },
      { ...settled!, caveatRefs: ['customer_email_ben@example.com'] },
      { ...settled!, settlementRefs: ['payout_destination.raw_node'] },
    ]) {
      expect(() =>
        projectPylonCapacityFunnelRecord(record, 'operator', nowIso),
      ).toThrow(PylonCapacityFunnelUnsafe)
    }
  })
})
