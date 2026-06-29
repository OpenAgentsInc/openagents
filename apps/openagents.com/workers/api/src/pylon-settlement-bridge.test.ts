import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES,
  OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
  OpenAgentsPylonSettlementBridgeProjection,
  OpenAgentsPylonSettlementBridgeRecord,
  OpenAgentsPylonSettlementBridgeUnsafe,
  openAgentsPylonSettlementBridgeCanMutateSettlement,
  openAgentsPylonSettlementBridgeHasNoSpendAuthority,
  openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial,
  openAgentsPylonSettlementBridgeSettlementClaimAllowed,
  projectOpenAgentsPylonSettlementBridge,
} from './pylon-settlement-bridge'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T04:00:00.000Z'

const bridgeRecord = (
  overrides: Partial<OpenAgentsPylonSettlementBridgeRecord> = {},
): OpenAgentsPylonSettlementBridgeRecord =>
  S.decodeUnknownSync(OpenAgentsPylonSettlementBridgeRecord)({
    ...OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('OpenAgents Pylon settlement bridge contract', () => {
  test('decodes conformance fixtures and projects settled bridge state without spend authority', () => {
    const record = bridgeRecord()
    const projection = projectOpenAgentsPylonSettlementBridge(
      record,
      'operator',
      nowIso,
    )

    expect(
      OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES.map(
        fixture => fixture.id,
      ),
    ).toEqual([
      'pylon_settlement_bridge.trace_summary_1',
      'pylon_settlement_bridge.buyer_payment_only',
    ])
    expect(S.decodeUnknownSync(OpenAgentsPylonSettlementBridgeRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(OpenAgentsPylonSettlementBridgeProjection)(
      projection,
    )).toEqual(projection)
    expect(openAgentsPylonSettlementBridgeHasNoSpendAuthority(
      record.authority,
    )).toBe(true)
    expect(openAgentsPylonSettlementBridgeCanMutateSettlement(record)).toBe(
      false,
    )
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.settlementClaimAllowed).toBe(true)
    expect(projection.buyerPaymentEvidencePresent).toBe(true)
    expect(projection.createdAtDisplay).toBe('50 minutes ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('keeps buyer payment, accepted work, reward intent, payout, and settlement claims separate', () => {
    const buyerPaymentOnly = bridgeRecord({
      acceptedWorkRefs: [],
      buyerPaymentEvidenceRefs: ['buyer_payment_evidence.order_pending'],
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      rewardIntentRefs: [],
      settlementRefs: [],
      state: 'buyer_payment_evidence',
    })
    const acceptedOnly = bridgeRecord({
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      rewardIntentRefs: [],
      settlementRefs: [],
      state: 'accepted_work',
    })
    const rewardOnly = bridgeRecord({
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      state: 'reward_intent',
    })
    const payoutEligible = bridgeRecord({
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      state: 'payout_eligible',
    })

    expect(projectOpenAgentsPylonSettlementBridge(
      buyerPaymentOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      acceptedWorkClaimAllowed: false,
      buyerPaymentEvidencePresent: true,
      payoutEligibilityClaimAllowed: false,
      rewardIntentClaimAllowed: false,
      settlementClaimAllowed: false,
    })
    expect(projectOpenAgentsPylonSettlementBridge(
      acceptedOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      acceptedWorkClaimAllowed: true,
      payoutEligibilityClaimAllowed: false,
      rewardIntentClaimAllowed: false,
      settlementClaimAllowed: false,
    })
    expect(projectOpenAgentsPylonSettlementBridge(
      rewardOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      payoutEligibilityClaimAllowed: false,
      rewardIntentClaimAllowed: true,
      settlementClaimAllowed: false,
    })
    expect(projectOpenAgentsPylonSettlementBridge(
      payoutEligible,
      'operator',
      nowIso,
    )).toMatchObject({
      payoutDispatchClaimAllowed: false,
      payoutEligibilityClaimAllowed: true,
      settlementClaimAllowed: false,
    })
  })

  test('redacts provider, wallet-readiness, buyer-payment, dispatch, and diagnostics from public projection', () => {
    const projection = projectOpenAgentsPylonSettlementBridge(
      bridgeRecord({
        operatorDiagnosticRefs: ['diagnostic.operator.pylon_bridge_safe_summary'],
        providerRef: 'provider.private.local_node',
        providerVisibility: 'private',
        settlementRefs: [
          'settlement.private.ledger_summary',
          'settlement.public_receipt.trace_summary_1',
        ],
      }),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.buyerPaymentEvidenceRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.payoutConfirmationRefs).toEqual([])
    expect(projection.payoutDispatchRefs).toEqual([])
    expect(projection.payoutVerificationRefs).toEqual([])
    expect(projection.walletReadinessRefs).toEqual([])
    expect(projection.workroomRefs).toEqual([])
    expect(projection.settlementRefs).toEqual([
      'settlement.public_receipt.trace_summary_1',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires evidence-only authority and settlement evidence before settlement claims are allowed', () => {
    const withoutSettlement = bridgeRecord({
      settlementRefs: [],
      state: 'payout_verified',
    })
    const withMutableAuthority = bridgeRecord({
      authority: {
        ...OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
        noLiveWalletSpend: false,
      },
    })

    expect(openAgentsPylonSettlementBridgeSettlementClaimAllowed(
      withoutSettlement,
    )).toBe(false)
    expect(() =>
      projectOpenAgentsPylonSettlementBridge(
        withMutableAuthority,
        'operator',
        nowIso,
      ),
    ).toThrow(OpenAgentsPylonSettlementBridgeUnsafe)
  })

  test('rejects raw payout targets, wallet material, invoices, preimages, private channel state, provider secrets, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw invoice', value: 'raw_invoice.bolt11_full' },
      { label: 'payout target', value: 'payout_address.bc1qtest' },
      { label: 'payment id', value: 'payment_id.provider_secret' },
      { label: 'private channel', value: 'private_channel.state_dump' },
      { label: 'provider secret', value: 'provider_secret.local_node' },
    ]) {
      expect(() =>
        projectOpenAgentsPylonSettlementBridge(
          bridgeRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPylonSettlementBridgeUnsafe)
    }
  })
})
