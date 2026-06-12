import { describe, expect, it } from 'vitest'

import {
  aggregatePresenceAccruals,
  classifyReceiptTier,
  CONTRACT_PRESENCE_TIER_POLICY,
  exportPresenceComputeReceiptTierContract,
  MAX_PRESENCE_ACCRUAL_ENTRIES_PER_AGGREGATION,
  type PresenceAccrualEntry,
  PresenceComputeReceiptTierSchemaVersion,
} from './training-presence-compute-receipts'

const probeEvidenceRefs = [
  'receipt.device_benchmark.cs336_a2.example_probe_1',
] as const

const entry = (
  overrides: Partial<PresenceAccrualEntry> = {},
): PresenceAccrualEntry => ({
  accrualDayUtc: '2026-06-12',
  amountSats: 400,
  deviceRef: 'device.example.alpha',
  identityRef: 'identity.contributor.example_one',
  probeEvidenceRefs: [...probeEvidenceRefs],
  ...overrides,
})

describe('presence/compute receipt tier split (Pluralis roadmap P2.3)', () => {
  describe('tier classification', () => {
    it('classifies shadow-window (warmup-state) verified work as presence-tier by construction', () => {
      expect(
        classifyReceiptTier({
          joinLifecycleState: 'warmup',
          verificationOutcomeRefs: ['verification.outcome.example_shadow'],
          workKind: 'verified_closeout',
        }),
      ).toEqual({
        joinLifecycleState: 'warmup',
        reasonCode: 'receipt_tier.public.presence_shadow_window_work',
        tier: 'presence_tier',
        workKind: 'verified_closeout',
      })

      expect(
        classifyReceiptTier({
          joinLifecycleState: 'warmup',
          verificationOutcomeRefs: [],
          workKind: 'shadow_window_work',
        }).tier,
      ).toBe('presence_tier')
    })

    it('classifies a merged verified closeout from an active device as compute-tier', () => {
      expect(
        classifyReceiptTier({
          joinLifecycleState: 'active',
          verificationOutcomeRefs: ['verification.outcome.example_merged'],
          workKind: 'verified_closeout',
        }),
      ).toEqual({
        joinLifecycleState: 'active',
        reasonCode: 'receipt_tier.public.compute_merged_verified_closeout',
        tier: 'compute_tier',
        workKind: 'verified_closeout',
      })
    })

    it('classifies liveness and qualification probes as presence-tier', () => {
      expect(
        classifyReceiptTier({
          joinLifecycleState: 'qualified',
          verificationOutcomeRefs: [],
          workKind: 'liveness_probe',
        }).reasonCode,
      ).toBe('receipt_tier.public.presence_liveness_probe')
      expect(
        classifyReceiptTier({
          joinLifecycleState: 'registered',
          verificationOutcomeRefs: [],
          workKind: 'qualification_probe',
        }).reasonCode,
      ).toBe('receipt_tier.public.presence_qualification_probe')
    })

    it('classifies verified closeouts from non-active, non-warmup states as presence-tier unmerged work', () => {
      for (const state of ['lagged', 'state_synced', 'sync_reentry'] as const) {
        expect(
          classifyReceiptTier({
            joinLifecycleState: state,
            verificationOutcomeRefs: ['verification.outcome.example_unmerged'],
            workKind: 'verified_closeout',
          }),
        ).toMatchObject({
          reasonCode: 'receipt_tier.public.presence_unmerged_work_not_active',
          tier: 'presence_tier',
        })
      }
    })

    it('refuses to classify a compute claim without verification outcome refs', () => {
      expect(() =>
        classifyReceiptTier({
          joinLifecycleState: 'active',
          verificationOutcomeRefs: [],
          workKind: 'verified_closeout',
        }),
      ).toThrow(/unverified work is not payable on any tier/i)
    })
  })

  describe('per-identity accrual with cap', () => {
    it('aggregates two devices under one identity into one capped accrual (Sybil pricing)', () => {
      const aggregation = aggregatePresenceAccruals({
        entries: [
          entry({ amountSats: 700, deviceRef: 'device.example.alpha' }),
          entry({
            amountSats: 700,
            deviceRef: 'device.example.bravo',
            probeEvidenceRefs: [
              'receipt.device_benchmark.cs336_a2.example_probe_2',
            ],
          }),
        ],
        policy: CONTRACT_PRESENCE_TIER_POLICY,
      })

      expect(aggregation.accruals).toHaveLength(1)
      expect(aggregation.accruals[0]).toMatchObject({
        accrualDayUtc: '2026-06-12',
        accruedSats: 1000,
        capSatsPerIdentityPerDay: 1000,
        deviceRefs: ['device.example.alpha', 'device.example.bravo'],
        identityRef: 'identity.contributor.example_one',
        requestedSats: 1400,
      })
      expect(aggregation.accruals[0]!.probeEvidenceRefs).toEqual([
        'receipt.device_benchmark.cs336_a2.example_probe_1',
        'receipt.device_benchmark.cs336_a2.example_probe_2',
      ])
      expect(aggregation.truncationEvents).toEqual([
        {
          accrualDayUtc: '2026-06-12',
          identityRef: 'identity.contributor.example_one',
          reasonCode: 'receipt_tier.public.presence_cap_truncated',
          requestedSats: 1400,
          truncatedSats: 400,
        },
      ])
      expect(aggregation.refusals).toEqual([])
      expect(aggregation.schemaVersion).toBe(
        PresenceComputeReceiptTierSchemaVersion,
      )
    })

    it('keeps separate identities and separate days as separate capped accruals', () => {
      const aggregation = aggregatePresenceAccruals({
        entries: [
          entry(),
          entry({ identityRef: 'identity.contributor.example_two' }),
          entry({ accrualDayUtc: '2026-06-13' }),
        ],
        policy: CONTRACT_PRESENCE_TIER_POLICY,
      })

      expect(aggregation.accruals).toHaveLength(3)
      expect(aggregation.truncationEvents).toEqual([])
      expect(
        aggregation.accruals.every(accrual => accrual.accruedSats === 400),
      ).toBe(true)
    })

    it('does not truncate accrual at or under the cap', () => {
      const aggregation = aggregatePresenceAccruals({
        entries: [entry({ amountSats: 1000 })],
        policy: CONTRACT_PRESENCE_TIER_POLICY,
      })

      expect(aggregation.accruals[0]).toMatchObject({
        accruedSats: 1000,
        requestedSats: 1000,
      })
      expect(aggregation.truncationEvents).toEqual([])
    })

    it('refuses presence accrual without probe evidence, typed, and never accrues it', () => {
      const aggregation = aggregatePresenceAccruals({
        entries: [
          entry({ probeEvidenceRefs: [] }),
          entry({ deviceRef: 'device.example.bravo' }),
        ],
        policy: CONTRACT_PRESENCE_TIER_POLICY,
      })

      expect(aggregation.refusals).toEqual([
        {
          accrualDayUtc: '2026-06-12',
          deviceRef: 'device.example.alpha',
          identityRef: 'identity.contributor.example_one',
          reason:
            'Presence accrual requires liveness or qualification probe evidence refs; presence without probe evidence is not payable.',
          reasonCode: 'receipt_tier.public.presence_probe_evidence_missing',
        },
      ])
      expect(aggregation.accruals).toHaveLength(1)
      expect(aggregation.accruals[0]).toMatchObject({
        accruedSats: 400,
        deviceRefs: ['device.example.bravo'],
        requestedSats: 400,
      })
    })

    it('rejects unbounded aggregation batches', () => {
      const entries = Array.from(
        { length: MAX_PRESENCE_ACCRUAL_ENTRIES_PER_AGGREGATION + 1 },
        (_, index) => entry({ deviceRef: `device.example.bulk_${index}` }),
      )

      expect(() =>
        aggregatePresenceAccruals({
          entries,
          policy: CONTRACT_PRESENCE_TIER_POLICY,
        }),
      ).toThrow(/at most 500 entries/i)
    })

    it('rejects identity refs carrying wallet or payment material', () => {
      expect(() =>
        aggregatePresenceAccruals({
          entries: [entry({ identityRef: 'identity.wallet.lnbc1examplepay' })],
          policy: CONTRACT_PRESENCE_TIER_POLICY,
        }),
      ).toThrow(/private host, wallet, payment/i)
    })

    it('rejects non-positive and non-integer sats amounts', () => {
      expect(() =>
        aggregatePresenceAccruals({
          entries: [entry({ amountSats: 0 })],
          policy: CONTRACT_PRESENCE_TIER_POLICY,
        }),
      ).toThrow(/positive integer sats/i)
      expect(() =>
        aggregatePresenceAccruals({
          entries: [entry({ amountSats: 10.5 })],
          policy: CONTRACT_PRESENCE_TIER_POLICY,
        }),
      ).toThrow(/positive integer sats/i)
    })
  })

  describe('exported contract', () => {
    it('exports a frozen, versioned, definitions-only contract with no live payout claim', () => {
      const contract = exportPresenceComputeReceiptTierContract()

      expect(contract.schemaVersion).toBe(
        'openagents.training.presence_compute_receipt_tiers.v1',
      )
      expect(contract.contractDefinitionOnly).toBe(true)
      expect(contract.livePayoutClaim).toBe(false)
      expect(contract.policy.probeEvidenceRequired).toBe(true)
      expect(contract.policy.capSatsPerIdentityPerDay).toBe(1000)
      expect(contract.policyRefs).toContain(
        'policy.public.receipt_tier.presence_cap_applies_to_identity_not_process',
      )
      expect(contract.policyRefs).toContain(
        'policy.public.receipt_tier.shadow_window_work_pays_presence_tier',
      )
      expect(Object.isFrozen(contract)).toBe(true)
      expect(Object.isFrozen(contract.policy)).toBe(true)
      expect(JSON.parse(JSON.stringify(contract))).toEqual(contract)
    })

    it('rejects a policy with a non-positive cap', () => {
      expect(() =>
        exportPresenceComputeReceiptTierContract({
          ...CONTRACT_PRESENCE_TIER_POLICY,
          capSatsPerIdentityPerDay: 0,
        }),
      ).toThrow(/positive integer sats cap/i)
    })
  })
})
