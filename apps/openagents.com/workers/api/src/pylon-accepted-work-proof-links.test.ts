import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_ACCEPTED_WORK_PROOF_LINK_CONFORMANCE_FIXTURES,
  PYLON_ACCEPTED_WORK_PROOF_LINK_READ_ONLY_AUTHORITY,
  PylonAcceptedWorkProofLinkProjection,
  PylonAcceptedWorkProofLinkRecord,
  PylonAcceptedWorkProofLinkUnsafe,
  projectPylonAcceptedWorkProofLink,
  pylonAcceptedWorkProofLinkCanMutateSettlement,
  pylonAcceptedWorkProofLinkHasNoMutationAuthority,
  pylonAcceptedWorkProofLinkProjectionHasPrivateMaterial,
} from './pylon-accepted-work-proof-links'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T11:10:00.000Z'

const proofLinkRecord = (
  overrides: Partial<PylonAcceptedWorkProofLinkRecord> = {},
): PylonAcceptedWorkProofLinkRecord =>
  S.decodeUnknownSync(PylonAcceptedWorkProofLinkRecord)({
    ...PYLON_ACCEPTED_WORK_PROOF_LINK_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon accepted-work proof links', () => {
  test('decodes and projects settled Site/order proof links without authority', () => {
    const record = proofLinkRecord()
    const projection = projectPylonAcceptedWorkProofLink(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonAcceptedWorkProofLinkRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(PylonAcceptedWorkProofLinkProjection)(
      projection,
    )).toEqual(projection)
    expect(pylonAcceptedWorkProofLinkHasNoMutationAuthority(record.authority))
      .toBe(true)
    expect(pylonAcceptedWorkProofLinkCanMutateSettlement(record)).toBe(false)
    expect(projection.acceptedWorkMutationAllowed).toBe(false)
    expect(projection.buyerChargeMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetDisclosureAllowed).toBe(false)
    expect(projection.providerEligibilityMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.siteReleaseMutationAllowed).toBe(false)
    expect(projection).toMatchObject({
      acceptedWorkClaimAllowed: true,
      payoutConfirmationClaimAllowed: true,
      payoutDispatchClaimAllowed: true,
      payoutEligibilityClaimAllowed: true,
      payoutVerificationClaimAllowed: true,
      providerPayoutClaimIncluded: true,
      rewardIntentClaimAllowed: true,
      settlementClaimAllowed: true,
      stateLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.consumerSurfaces).toEqual([
      'customer_dashboard',
      'public_proof',
      'site_order',
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonAcceptedWorkProofLinkProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('redacts private provider and payout refs from public proof links', () => {
    const projection = projectPylonAcceptedWorkProofLink(
      proofLinkRecord(),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.providerJobRefs).toEqual([
      'job.public.site_otc_revision_3',
    ])
    expect(projection.payoutDispatchRefs).toEqual([
      'dispatch.public.site_otc_revision_3',
    ])
    expect(projection.payoutConfirmationRefs).toEqual([
      'confirmation.public.site_otc_revision_3',
    ])
    expect(projection.payoutVerificationRefs).toEqual([
      'verification.public.site_otc_revision_3',
    ])
    expect(projection.payoutSloRefs).toEqual([
      'slo.public.site_otc_revision_3',
    ])
    expect(projection.settlementEvidenceRefs).toEqual([
      'settlement.public.evidence.site_otc_revision_3',
    ])
    expect(projection.siteRefs).toEqual(['site.public.otec'])
    expect(projection.orderRefs).toEqual(['order.public.otec'])
    expect(projection.publicProofRefs).toEqual(['proof.public.otec'])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('preserves accepted work, reward, eligibility, dispatch, verification, and settlement separation', () => {
    const base = proofLinkRecord()
    const accepted = projectPylonAcceptedWorkProofLink({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      rewardIntentRefs: [],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      state: 'accepted_work',
    }, 'customer', nowIso)
    const reward = projectPylonAcceptedWorkProofLink({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      state: 'reward_intent',
    }, 'customer', nowIso)
    const eligible = projectPylonAcceptedWorkProofLink({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutVerificationRefs: [],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      state: 'payout_eligible',
    }, 'customer', nowIso)
    const dispatched = projectPylonAcceptedWorkProofLink({
      ...base,
      payoutConfirmationRefs: [],
      payoutVerificationRefs: [],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      state: 'payout_dispatched',
    }, 'customer', nowIso)
    const verified = projectPylonAcceptedWorkProofLink({
      ...base,
      settlementEvidenceRefs: [],
      settlementRefs: [],
      state: 'payout_verified',
    }, 'customer', nowIso)

    expect(accepted.acceptedWorkClaimAllowed).toBe(true)
    expect(accepted.rewardIntentClaimAllowed).toBe(false)
    expect(reward.rewardIntentClaimAllowed).toBe(true)
    expect(reward.payoutEligibilityClaimAllowed).toBe(false)
    expect(eligible.payoutEligibilityClaimAllowed).toBe(true)
    expect(eligible.payoutDispatchClaimAllowed).toBe(false)
    expect(dispatched.payoutDispatchClaimAllowed).toBe(true)
    expect(dispatched.payoutVerificationClaimAllowed).toBe(false)
    expect(verified.payoutVerificationClaimAllowed).toBe(true)
    expect(verified.settlementClaimAllowed).toBe(false)
  })

  test('requires provider jobs for provider payout claims and matching refs for state claims', () => {
    const base = proofLinkRecord()

    expect(() =>
      projectPylonAcceptedWorkProofLink({
        ...base,
        providerJobRefs: [],
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkProofLinkUnsafe)

    expect(() =>
      projectPylonAcceptedWorkProofLink({
        ...base,
        acceptedWorkRefs: [],
        state: 'accepted_work',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkProofLinkUnsafe)

    expect(() =>
      projectPylonAcceptedWorkProofLink({
        ...base,
        payoutEligibilityRefs: [],
        state: 'payout_eligible',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkProofLinkUnsafe)

    expect(() =>
      projectPylonAcceptedWorkProofLink({
        ...base,
        payoutVerificationRefs: [],
        state: 'settled',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkProofLinkUnsafe)
  })

  test('rejects mutable authority and unsafe raw payout, wallet, payment, invoice, and credential material', () => {
    const base = proofLinkRecord()

    expect(() =>
      projectPylonAcceptedWorkProofLink({
        ...base,
        authority: {
          ...PYLON_ACCEPTED_WORK_PROOF_LINK_READ_ONLY_AUTHORITY,
          noSettlementMutation: false,
        },
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkProofLinkUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'raw payout target', value: 'payout_target.raw_destination' },
      { label: 'invoice', value: 'invoice.lnbc123' },
      { label: 'preimage', value: 'payment_preimage.raw_secret' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'provider token', value: 'provider_token.local' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
    ]) {
      expect(() =>
        projectPylonAcceptedWorkProofLink({
          ...base,
          evidenceRefs: [fixture.value],
        }, 'operator', nowIso),
      ).toThrow(PylonAcceptedWorkProofLinkUnsafe)
    }
  })
})
