import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
  PylonForwardPowerScenarioProjection,
  PylonForwardPowerScenarioUnsafe,
  examplePylonForwardPowerScenario,
  projectPylonForwardPowerScenario,
  pylonForwardPowerScenarioProjectionHasPrivateMaterial,
} from './pylon-forward-power-scenarios'

const nowIso = '2026-06-06T23:30:00.000Z'

describe('Pylon forward-power scenarios', () => {
  test('projects modeled interconnection value without grid or financial authority', () => {
    const projection = projectPylonForwardPowerScenario(
      examplePylonForwardPowerScenario(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonForwardPowerScenarioProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      avoidedCostTotalCents: 17500000,
      avoidedDelayCostCents: 2500000,
      avoidedDelayDays: 45,
      avoidedDelayValueClaimAllowed: true,
      avoidedUpgradeCostCents: 15000000,
      avoidedUpgradeValueClaimAllowed: true,
      capacityDispatchAllowed: false,
      createdAtDisplay: '10 minutes ago',
      facilityRef: 'facility.public_demo_1',
      financialAdviceAllowed: false,
      gridParticipationAllowed: false,
      interconnectionMutationAllowed: false,
      measuredPowerClaimAllowed: false,
      modeledScenario: true,
      powerTradingAllowed: false,
      publicClaimUpgradeAllowed: false,
      scenarioKindLabel: 'Interconnection value',
      settlementClaimAllowed: false,
      settlementMutationAllowed: false,
      stateLabel: 'Modeled',
      unusedPowerMwh: 5,
      updatedAtDisplay: '5 minutes ago',
      workloadFitBps: 7200,
      workloadFitPercent: 72,
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(pylonForwardPowerScenarioProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps modeled, measured, contracted, and settled claims separate', () => {
    const base = examplePylonForwardPowerScenario()
    const measured = projectPylonForwardPowerScenario({
      ...base,
      proofOfResponseRefs: ['proof.public.response_history'],
      state: 'measured',
    }, 'team', nowIso)
    const contracted = projectPylonForwardPowerScenario({
      ...base,
      contractRefs: ['contract.public.forward_power_window'],
      state: 'contracted',
    }, 'team', nowIso)
    const settled = projectPylonForwardPowerScenario({
      ...base,
      contractRefs: ['contract.public.forward_power_window'],
      proofOfResponseRefs: ['proof.public.response_history'],
      settlementRefs: ['settlement.public.forward_power_window'],
      state: 'settled',
    }, 'team', nowIso)

    expect(measured.measuredPowerClaimAllowed).toBe(true)
    expect(measured.settlementClaimAllowed).toBe(false)
    expect(contracted.measuredPowerClaimAllowed).toBe(false)
    expect(contracted.settlementClaimAllowed).toBe(false)
    expect(settled.measuredPowerClaimAllowed).toBe(true)
    expect(settled.settlementClaimAllowed).toBe(true)
  })

  test('redacts private facility, contract, interconnection, proof, and settlement refs publicly', () => {
    const projection = projectPylonForwardPowerScenario({
      ...examplePylonForwardPowerScenario(),
      contractRefs: [
        'contract.public.forward_power_window',
        'contract.private.operator_terms',
      ],
      facilityRef: 'facility.private_operator_site',
      interconnectionRefs: [
        'interconnection.public.queue_position',
        'interconnection.private.utility_case',
      ],
      proofOfResponseRefs: [
        'proof.public.response_history',
        'proof.private.operator_meter',
      ],
      settlementRefs: [
        'settlement.public.forward_power_window',
        'settlement.private.operator_receipt',
      ],
      state: 'settled',
    }, 'public', nowIso)

    expect(projection.facilityRef).toBe('facility.redacted')
    expect(projection.contractRefs).toEqual([
      'contract.public.forward_power_window',
    ])
    expect(projection.interconnectionRefs).toEqual([
      'interconnection.public.queue_position',
    ])
    expect(projection.proofOfResponseRefs).toEqual([
      'proof.public.response_history',
    ])
    expect(projection.settlementRefs).toEqual([
      'settlement.public.forward_power_window',
    ])
  })

  test('requires assumptions, caveats, interconnection evidence, measured proof, contract refs, and settlement refs', () => {
    const base = examplePylonForwardPowerScenario()

    for (const record of [
      { ...base, assumptionRefs: [] },
      { ...base, caveatRefs: [] },
      { ...base, interconnectionRefs: [] },
      { ...base, proofOfResponseRefs: [], state: 'measured' as const },
      { ...base, contractRefs: [], state: 'contracted' as const },
      {
        ...base,
        contractRefs: ['contract.public.forward_power_window'],
        proofOfResponseRefs: ['proof.public.response_history'],
        settlementRefs: [],
        state: 'settled' as const,
      },
      { ...base, workloadFitBps: 10001 },
    ]) {
      expect(() =>
        projectPylonForwardPowerScenario(record, 'operator', nowIso),
      ).toThrow(PylonForwardPowerScenarioUnsafe)
    }
  })

  test('rejects financial, trading, grid, interconnection, settlement, and public-claim authority', () => {
    const base = examplePylonForwardPowerScenario()

    for (const authority of [
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noCapacityDispatch: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noFinancialAdvice: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noGridParticipation: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noInterconnectionMutation: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noPowerTrading: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noPublicClaimUpgrade: false,
      },
      {
        ...PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
        noSettlementMutation: false,
      },
    ]) {
      expect(() =>
        projectPylonForwardPowerScenario({
          ...base,
          authority,
        }, 'operator', nowIso),
      ).toThrow(PylonForwardPowerScenarioUnsafe)
    }
  })

  test('rejects private power, contract, payment, trading, provider, and raw telemetry refs', () => {
    const base = examplePylonForwardPowerScenario()

    for (const record of [
      { ...base, evidenceRefs: ['raw_power.telemetry'] },
      { ...base, contractRefs: ['raw_contract.utility_pdf'] },
      { ...base, settlementRefs: ['payment_id.raw_123'] },
      { ...base, sourceRefs: ['trading_order.private'] },
      { ...base, proofOfResponseRefs: ['provider_telemetry.raw_power'] },
      { ...base, assumptionRefs: ['assumption.2026-06-06T23:00:00Z'] },
    ]) {
      expect(() =>
        projectPylonForwardPowerScenario(record, 'operator', nowIso),
      ).toThrow(PylonForwardPowerScenarioUnsafe)
    }
  })
})
