import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonProviderJobLifecycleProjection,
  PylonProviderJobLifecycleUnsafe,
  examplePylonProviderJobLifecycleRecord,
  projectPylonProviderJobLifecycle,
  pylonProviderJobProjectionHasPrivateMaterial,
} from './pylon-provider-job-lifecycle'

const nowIso = '2026-06-06T21:25:00.000Z'

describe('Pylon provider job lifecycle', () => {
  test('projects settled lifecycle records with provider payout state separate from buyer payment evidence', () => {
    const publicProjection = projectPylonProviderJobLifecycle(
      examplePylonProviderJobLifecycleRecord(),
      'public',
      nowIso,
    )
    const operatorProjection = projectPylonProviderJobLifecycle(
      examplePylonProviderJobLifecycleRecord(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonProviderJobLifecycleProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      acceptedWorkClaimAllowed: true,
      buyerPaymentEvidenceRefs: [],
      payoutDispatchClaimAllowed: true,
      payoutDispatchRefs: [],
      providerRef: 'provider.pylon_public_demo',
      rewardIntentClaimAllowed: true,
      settlementClaimAllowed: true,
      settlementRefs: [],
      stage: 'settled',
      stageLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
      workroomRefs: [],
    })
    expect(operatorProjection.buyerPaymentEvidenceRefs).toEqual([
      'buyer_payment_evidence.omega_internal_budget',
    ])
    expect(operatorProjection.payoutDispatchRefs).toEqual([
      'payout_dispatch.trace_summary_1',
    ])
    expect(operatorProjection.settlementRefs).toEqual([
      'settlement.trace_summary_1',
    ])
    expect(pylonProviderJobProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('keeps accepted, reward intent, payout dispatch, and settlement claims separate', () => {
    const base = examplePylonProviderJobLifecycleRecord()
    const accepted = projectPylonProviderJobLifecycle({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutVerificationRefs: [],
      rewardIntentRefs: [],
      settlementRefs: [],
      stage: 'accepted',
    }, 'customer', nowIso)
    const rewardIntent = projectPylonProviderJobLifecycle({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      stage: 'reward_intent_recorded',
    }, 'customer', nowIso)
    const payoutDispatched = projectPylonProviderJobLifecycle({
      ...base,
      payoutConfirmationRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      stage: 'payout_dispatched',
    }, 'team', nowIso)

    expect(accepted.acceptedWorkClaimAllowed).toBe(true)
    expect(accepted.rewardIntentClaimAllowed).toBe(false)
    expect(accepted.payoutDispatchClaimAllowed).toBe(false)
    expect(accepted.settlementClaimAllowed).toBe(false)
    expect(rewardIntent.acceptedWorkClaimAllowed).toBe(true)
    expect(rewardIntent.rewardIntentClaimAllowed).toBe(true)
    expect(rewardIntent.payoutDispatchClaimAllowed).toBe(false)
    expect(rewardIntent.settlementClaimAllowed).toBe(false)
    expect(payoutDispatched.rewardIntentClaimAllowed).toBe(true)
    expect(payoutDispatched.payoutDispatchClaimAllowed).toBe(true)
    expect(payoutDispatched.settlementClaimAllowed).toBe(false)
  })

  test('requires stage evidence as lifecycle advances', () => {
    const base = examplePylonProviderJobLifecycleRecord()

    expect(() =>
      projectPylonProviderJobLifecycle({
        ...base,
        artifactRefs: [],
        stage: 'artifact_produced',
      }, 'operator', nowIso),
    ).toThrow(PylonProviderJobLifecycleUnsafe)
    expect(() =>
      projectPylonProviderJobLifecycle({
        ...base,
        acceptanceRefs: [],
        stage: 'accepted',
      }, 'operator', nowIso),
    ).toThrow(PylonProviderJobLifecycleUnsafe)
    expect(() =>
      projectPylonProviderJobLifecycle({
        ...base,
        rewardIntentRefs: [],
        stage: 'reward_intent_recorded',
      }, 'operator', nowIso),
    ).toThrow(PylonProviderJobLifecycleUnsafe)
    expect(() =>
      projectPylonProviderJobLifecycle({
        ...base,
        payoutDispatchRefs: [],
        stage: 'payout_dispatched',
      }, 'operator', nowIso),
    ).toThrow(PylonProviderJobLifecycleUnsafe)
    expect(() =>
      projectPylonProviderJobLifecycle({
        ...base,
        settlementRefs: [],
        stage: 'settled',
      }, 'operator', nowIso),
    ).toThrow(PylonProviderJobLifecycleUnsafe)
  })

  test('redacts private providers and keeps raw timestamps out of projections', () => {
    const publicProjection = projectPylonProviderJobLifecycle({
      ...examplePylonProviderJobLifecycleRecord(),
      providerRef: 'provider.private_demo',
      providerVisibility: 'private',
    }, 'public', nowIso)
    const operatorProjection = projectPylonProviderJobLifecycle({
      ...examplePylonProviderJobLifecycleRecord(),
      providerRef: 'provider.private_demo',
      providerVisibility: 'private',
    }, 'operator', nowIso)
    const serialized = JSON.stringify(publicProjection)

    expect(publicProjection.providerRef).toBe('provider.redacted')
    expect(operatorProjection.providerRef).toBe('provider.private_demo')
    expect(serialized).not.toContain('2026-06-06T21:20:00.000Z')
    expect(serialized).not.toContain('workroom.pylon_trace_summary')
  })

  test('rejects raw payout targets, payment IDs, wallet, invoices, preimages, provider tokens, runner logs, and customer refs', () => {
    const base = examplePylonProviderJobLifecycleRecord()

    for (const record of [
      { ...base, payoutDispatchRefs: ['payout_target.node_abc'] },
      { ...base, buyerPaymentEvidenceRefs: ['payment_id.raw_123'] },
      { ...base, settlementRefs: ['wallet_state.local_node'] },
      { ...base, evidenceRefs: ['invoice.lnbc123'] },
      { ...base, evidenceRefs: ['payment_preimage.secret'] },
      { ...base, evidenceRefs: ['provider_token.codex'] },
      { ...base, runRefs: ['raw_runner_log.trace_summary'] },
      { ...base, caveatRefs: ['customer_email_ben@example.com'] },
    ]) {
      expect(() =>
        projectPylonProviderJobLifecycle(record, 'operator', nowIso),
      ).toThrow(PylonProviderJobLifecycleUnsafe)
    }
  })
})
