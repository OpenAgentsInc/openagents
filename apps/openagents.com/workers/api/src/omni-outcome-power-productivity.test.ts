import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
  OmniOutcomePowerProductivityProjection,
  OmniOutcomePowerProductivityRecord,
  OmniOutcomePowerProductivityUnsafe,
  projectOmniOutcomePowerProductivity,
} from './omni-outcome-power-productivity'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-06T16:00:00.000Z'

const powerRecord = (
  overrides: Partial<OmniOutcomePowerProductivityRecord> = {},
): OmniOutcomePowerProductivityRecord =>
  S.decodeUnknownSync(OmniOutcomePowerProductivityRecord)({
    acceptedGrossProfitCents: 130000,
    acceptedOutcomeCount: 4,
    acceptedOutcomeRefs: [
      'accepted_outcome.public.site_batch',
      'accepted_outcome.private.site_internal',
    ],
    acceptedRevenueCents: 200000,
    authority: OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
    caveatRefs: ['caveat.public.metered_at_provider_boundary'],
    darkCapacityReasonRefs: ['dark_reason.no_work_assigned'],
    darkCapacityWattHours: 500000,
    energyEvidenceRefs: [
      'evidence.public.energy.site_batch',
      'evidence.private.energy.operator',
    ],
    energyModelRefs: [],
    energyWattHours: 2000000,
    id: 'outcome_power.site_batch',
    measuredEnergyRefs: [
      'meter.public.site_batch',
      'meter.private.operator_panel',
    ],
    powerDataState: 'measured',
    providerPayableCents: 50000,
    providerSettledCents: 50000,
    settlementRefs: [
      'settlement.public.site_batch.provider',
      'settlement.private.site_batch.operator',
    ],
    settlementState: 'settled',
    sourceRefs: ['source.public.provider_meter_summary'],
    updatedAtIso: '2026-06-06T15:55:00.000Z',
    workKind: 'site',
    workroomRefs: ['workroom.private.site_batch'],
    ...overrides,
  })

describe('Omni outcome power productivity', () => {
  test('calculates accepted outcomes and economics per kWh and MWh', () => {
    const projection = projectOmniOutcomePowerProductivity(
      [
        powerRecord(),
        powerRecord({
          acceptedGrossProfitCents: 25000,
          acceptedOutcomeCount: 1,
          acceptedOutcomeRefs: ['accepted_outcome.public.coding_batch'],
          acceptedRevenueCents: 40000,
          caveatRefs: ['caveat.public.energy_modelled'],
          darkCapacityReasonRefs: ['dark_reason.awaiting_jobs'],
          darkCapacityWattHours: 1500000,
          energyEvidenceRefs: ['evidence.public.energy_model.coding'],
          energyModelRefs: ['model.public.energy.coding'],
          energyWattHours: 500000,
          id: 'outcome_power.coding_batch',
          measuredEnergyRefs: [],
          powerDataState: 'modeled',
          providerPayableCents: 10000,
          providerSettledCents: 0,
          settlementRefs: [],
          settlementState: 'payable',
          sourceRefs: ['source.public.energy_model.coding'],
          updatedAtIso: '2026-06-06T15:50:00.000Z',
          workKind: 'coding',
          workroomRefs: ['workroom.private.coding_batch'],
        }),
      ],
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniOutcomePowerProductivityProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.energyMeterMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.powerMarketClaimUpgradeAllowed).toBe(false)
    expect(projection.providerSettlementMutationAllowed).toBe(false)
    expect(projection.publicClaimUpgradeAllowed).toBe(false)
    expect(projection.totals).toMatchObject({
      acceptedGrossProfitCents: 155000,
      acceptedGrossProfitCentsPerKwh: 62,
      acceptedOutcomeCount: 5,
      acceptedOutcomesPerKwh: 0.002,
      acceptedOutcomesPerMwh: 2,
      acceptedRevenueCents: 240000,
      acceptedRevenueCentsPerKwh: 96,
      darkCapacityMwh: 2,
      darkCapacityWattHours: 2000000,
      energyKwh: 2500,
      energyMwh: 2.5,
      energyWattHours: 2500000,
      measuredEnergyClaimAllowed: false,
      modeledEnergyClaimAllowed: false,
      powerDataState: 'mixed',
      powerDataStateLabel: 'Mixed',
      providerPayableCents: 60000,
      providerPayableCentsPerKwh: 24,
      providerSettledCents: 50000,
      settlementClaimAllowed: true,
      settlementState: 'mixed',
      settlementStateLabel: 'Mixed',
    })
    expect(projection.workKindMetrics.find(metric => metric.workKind === 'site'))
      .toMatchObject({
        acceptedOutcomesPerKwh: 0.002,
        acceptedOutcomesPerMwh: 2,
        acceptedRevenueCentsPerKwh: 100,
        darkCapacityMwh: 0.5,
        measuredEnergyClaimAllowed: true,
        powerDataStateLabel: 'Measured',
        providerPayableCentsPerKwh: 25,
        settlementClaimAllowed: true,
        workKindLabel: 'Site',
      })
  })

  test('redacts private power, settlement, and workroom refs from public projections', () => {
    const projection = projectOmniOutcomePowerProductivity(
      [powerRecord()],
      'public',
      nowIso,
    )

    expect(projection.totals.acceptedOutcomeRefs).toEqual([
      'accepted_outcome.public.site_batch',
    ])
    expect(projection.totals.energyEvidenceRefs).toEqual([
      'evidence.public.energy.site_batch',
    ])
    expect(projection.totals.measuredEnergyRefs).toEqual([
      'meter.public.site_batch',
    ])
    expect(projection.totals.settlementRefs).toEqual([
      'settlement.public.site_batch.provider',
    ])
    expect(projection.totals.workroomRefs).toEqual([])
    expect(projection.totals.measuredEnergyClaimAllowed).toBe(true)
    expect(projection.totals.settlementClaimAllowed).toBe(true)
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('handles zero and unknown energy without inventing per-energy metrics', () => {
    const projection = projectOmniOutcomePowerProductivity(
      [
        powerRecord({
          acceptedGrossProfitCents: 0,
          acceptedOutcomeCount: 1,
          acceptedRevenueCents: 0,
          caveatRefs: ['caveat.public.energy_unknown'],
          darkCapacityReasonRefs: [],
          darkCapacityWattHours: 0,
          energyEvidenceRefs: [],
          energyModelRefs: [],
          energyWattHours: null,
          measuredEnergyRefs: [],
          powerDataState: 'unknown',
          providerPayableCents: 0,
          providerSettledCents: 0,
          settlementRefs: [],
          settlementState: 'not_settled',
        }),
      ],
      'team',
      nowIso,
    )

    expect(projection.totals.energyWattHours).toBeNull()
    expect(projection.totals.energyKwh).toBeNull()
    expect(projection.totals.acceptedOutcomesPerKwh).toBeNull()
    expect(projection.totals.acceptedOutcomesPerMwh).toBeNull()
    expect(projection.totals.acceptedRevenueCentsPerKwh).toBeNull()
    expect(projection.totals.providerPayableCentsPerKwh).toBeNull()
    expect(projection.totals.darkCapacityMwh).toBe(0)
    expect(projection.totals.powerDataStateLabel).toBe('Unknown')
    expect(projection.totals.settlementStateLabel).toBe('Not settled')
    expect(projection.totals.measuredEnergyClaimAllowed).toBe(false)
    expect(projection.totals.modeledEnergyClaimAllowed).toBe(false)
    expect(projection.totals.settlementClaimAllowed).toBe(false)
  })

  test('rejects false authority, missing energy evidence, false settlement, and unsafe refs', () => {
    expect(() =>
      projectOmniOutcomePowerProductivity(
        [
          powerRecord({
            authority: {
              ...OMNI_OUTCOME_POWER_PRODUCTIVITY_READ_ONLY_AUTHORITY,
              noPowerMarketClaimUpgrade: false,
            },
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniOutcomePowerProductivityUnsafe)

    expect(() =>
      projectOmniOutcomePowerProductivity(
        [powerRecord({ measuredEnergyRefs: [] })],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniOutcomePowerProductivityUnsafe)

    expect(() =>
      projectOmniOutcomePowerProductivity(
        [
          powerRecord({
            providerSettledCents: 50000,
            settlementRefs: [],
            settlementState: 'settled',
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniOutcomePowerProductivityUnsafe)

    expect(() =>
      projectOmniOutcomePowerProductivity(
        [
          powerRecord({
            darkCapacityReasonRefs: [],
            darkCapacityWattHours: 1000,
          }),
        ],
        'operator',
        nowIso,
      ),
    ).toThrow(OmniOutcomePowerProductivityUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw power telemetry', value: 'raw_power.telemetry.kw' },
      { label: 'raw meter telemetry', value: 'raw_meter.telemetry.panel' },
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'provider telemetry', value: 'provider_telemetry.raw_power' },
      { label: 'raw timestamp', value: 'evidence.2026-06-06T15:55:00' },
    ]) {
      expect(() =>
        projectOmniOutcomePowerProductivity(
          [
            powerRecord({
              energyEvidenceRefs: [fixture.value],
            }),
          ],
          'operator',
          nowIso,
        ),
      ).toThrow(OmniOutcomePowerProductivityUnsafe)
    }
  })
})
