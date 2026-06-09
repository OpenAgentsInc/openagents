import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MARGOT_EXPORT_READ_ONLY_AUTHORITY,
  MargotExportPacketProjection,
  MargotExportPacketRecord,
  MargotExportPacketUnsafe,
  exampleMargotExportPacket,
  margotExportProjectionHasPrivateMaterial,
  projectMargotExportPacket,
} from './margot-export-ingestion'

const nowIso = '2026-06-06T23:50:00.000Z'

const margotPacket = (
  overrides: Partial<MargotExportPacketRecord> = {},
): MargotExportPacketRecord =>
  S.decodeUnknownSync(MargotExportPacketRecord)({
    ...exampleMargotExportPacket(),
    ...overrides,
  })

describe('Margot export ingestion', () => {
  test('projects modeled Margot simulator economics without mutation authority', () => {
    const projection = projectMargotExportPacket(
      exampleMargotExportPacket(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(MargotExportPacketProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkCentsPerMwh: 42000,
      acceptedWorkLaneClaimAllowed: false,
      acceptedWorkMutationAllowed: false,
      claimState: 'modeled',
      claimStateLabel: 'Modeled',
      dispatchPolicyLabel: 'Threshold',
      financialAdviceAllowed: false,
      generatedAtDisplay: '10 minutes ago',
      gpuRentalFloorCentsPerMwh: 27000,
      gridParticipationAllowed: false,
      gridServiceCentsPerMwh: 9000,
      liveWalletSpendAllowed: false,
      marketDataMutationAllowed: false,
      marketLabel: 'ERCOT',
      miningFloorCentsPerMwh: 18000,
      publicClaimUpgradeAllowed: false,
      settlementMutationAllowed: false,
      supportedMarket: true,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(margotExportProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('allows measured and settled packets only when evidence refs support the claim state', () => {
    const measured = projectMargotExportPacket(
      margotPacket({
        claimState: 'measured',
        generatedAtIso: '2026-06-06T23:35:00.000Z',
        updatedAtIso: '2026-06-06T23:45:00.000Z',
      }),
      'team',
      nowIso,
    )
    const settled = projectMargotExportPacket(
      margotPacket({
        claimState: 'settled',
        settlementRefs: ['settlement.public.margot.accepted_work_batch'],
      }),
      'team',
      nowIso,
    )

    expect(measured.acceptedWorkLaneClaimAllowed).toBe(true)
    expect(measured.claimStateLabel).toBe('Measured')
    expect(settled.acceptedWorkLaneClaimAllowed).toBe(true)
    expect(settled.claimStateLabel).toBe('Settled')
    expect(settled.settlementRefs).toEqual([
      'settlement.public.margot.accepted_work_batch',
    ])
  })

  test('keeps unsupported-market packets only with explicit caveats', () => {
    const projection = projectMargotExportPacket(
      margotPacket({
        caveatRefs: [
          'caveat.public.unsupported_market_operator_manual_review',
        ],
        market: 'unsupported',
      }),
      'operator',
      nowIso,
    )

    expect(projection.marketLabel).toBe('Unsupported market')
    expect(projection.supportedMarket).toBe(false)
    expect(() =>
      projectMargotExportPacket(
        margotPacket({
          caveatRefs: ['caveat.public.market_pending_support'],
          market: 'unsupported',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(MargotExportPacketUnsafe)
  })

  test('redacts private assumption, caveat, diligence, settlement, provenance, scenario, and source refs publicly', () => {
    const projection = projectMargotExportPacket(
      margotPacket({
        acceptedOutcomeAssumptionRefs: [
          'accepted_outcome_assumption.public.agentic_work_v1',
          'accepted_outcome.private.operator_batch',
        ],
        caveatRefs: [
          'caveat.public.tdp_not_facility_power',
          'caveat.private.operator_model_notes',
        ],
        dataRightsRefs: [
          'data_rights.public.operator_review_only',
          'data_rights.private.operator_model_terms',
        ],
        nextDiligenceRefs: [
          'diligence.public.facility_power_metering',
          'diligence.private.operator_follow_up',
        ],
        provenanceRefs: [
          'provenance.public.oa_aibtc_model',
          'provenance.private.operator_spreadsheet',
        ],
        scenarioRefs: [
          'scenario.public.shc_demo',
          'scenario.private.operator_case',
        ],
        settlementRefs: [
          'settlement.public.margot.accepted_work_batch',
          'settlement.private.operator_receipt',
        ],
        sourceRefs: [
          'source.public.margot_synthesis',
          'source.private.operator_export',
        ],
      }),
      'public',
      nowIso,
    )

    expect(projection.acceptedOutcomeAssumptionRefs).toEqual([
      'accepted_outcome_assumption.public.agentic_work_v1',
    ])
    expect(projection.caveatRefs).toEqual([
      'caveat.public.tdp_not_facility_power',
    ])
    expect(projection.dataRightsRefs).toEqual([
      'data_rights.public.operator_review_only',
    ])
    expect(projection.nextDiligenceRefs).toEqual([
      'diligence.public.facility_power_metering',
    ])
    expect(projection.provenanceRefs).toEqual([
      'provenance.public.oa_aibtc_model',
    ])
    expect(projection.scenarioRefs).toEqual(['scenario.public.shc_demo'])
    expect(projection.settlementRefs).toEqual([
      'settlement.public.margot.accepted_work_batch',
    ])
    expect(projection.sourceRefs).toEqual(['source.public.margot_synthesis'])
  })

  test('requires caveats, provenance, source refs, diligence refs, accepted-work assumptions, and settlement refs', () => {
    const base = exampleMargotExportPacket()

    for (const record of [
      { ...base, caveatRefs: [] },
      { ...base, provenanceRefs: [] },
      { ...base, sourceRefs: [] },
      { ...base, nextDiligenceRefs: [] },
      { ...base, acceptedOutcomeAssumptionRefs: [] },
      { ...base, claimState: 'settled' as const, settlementRefs: [] },
    ]) {
      expect(() =>
        projectMargotExportPacket(record, 'operator', nowIso),
      ).toThrow(MargotExportPacketUnsafe)
    }
  })

  test('rejects false accepted-work, grid, wallet, market, settlement, public-claim, and financial authority', () => {
    const base = exampleMargotExportPacket()

    for (const authority of [
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noAcceptedWorkMutation: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noFinancialAdvice: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noGridParticipation: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noLiveWalletSpend: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noMarketDataMutation: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noPublicClaimUpgrade: false,
      },
      {
        ...MARGOT_EXPORT_READ_ONLY_AUTHORITY,
        noSettlementMutation: false,
      },
    ]) {
      expect(() =>
        projectMargotExportPacket({
          ...base,
          authority,
        }, 'operator', nowIso),
      ).toThrow(MargotExportPacketUnsafe)
    }
  })

  test('rejects private customer, provider, wallet, payment, raw export, raw market, raw telemetry, private repo, secret, timestamp, and non-integer values', () => {
    const base = exampleMargotExportPacket()

    for (const record of [
      { ...base, sourceRefs: ['raw_export.operator_dump'] },
      { ...base, provenanceRefs: ['raw_market.price_curve'] },
      { ...base, scenarioRefs: ['hardware_telemetry.raw_power'] },
      { ...base, settlementRefs: ['payment_id.raw_123'] },
      { ...base, dataRightsRefs: ['wallet.secret.material'] },
      { ...base, caveatRefs: ['provider_token.hidden'] },
      { ...base, nextDiligenceRefs: ['github.com/team/private-model'] },
      { ...base, acceptedOutcomeAssumptionRefs: ['assumption.2026-06-06T23:00:00Z'] },
      { ...base, miningFloorCentsPerMwh: 12.5 },
      { ...base, powerCostCentsPerMwh: -1 },
    ]) {
      expect(() =>
        projectMargotExportPacket(record, 'operator', nowIso),
      ).toThrow(MargotExportPacketUnsafe)
    }
  })
})
