import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_PYLON_COMPARATIVE_ECONOMICS_READ_ONLY_AUTHORITY,
  ArtanisPylonComparativeEconomicsPacketRecord,
  ArtanisPylonComparativeEconomicsProjection,
  ArtanisPylonComparativeEconomicsUnsafe,
  artanisPylonComparativeEconomicsProjectionHasPrivateMaterial,
  exampleArtanisPylonComparativeEconomicsPacket,
  projectArtanisPylonComparativeEconomicsPacket,
} from './artanis-pylon-comparative-economics'

const nowIso = '2026-06-06T14:00:00.000Z'

const economicsPacket = (
  overrides: Partial<ArtanisPylonComparativeEconomicsPacketRecord> = {},
): ArtanisPylonComparativeEconomicsPacketRecord =>
  S.decodeUnknownSync(ArtanisPylonComparativeEconomicsPacketRecord)({
    ...exampleArtanisPylonComparativeEconomicsPacket(),
    ...overrides,
  })

describe('Artanis Pylon comparative economics packets', () => {
  test('projects a modeled ERCOT packet with read-only authority and all comparator rows', () => {
    const projection = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisPylonComparativeEconomicsProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedOutcomeValueDollarsPerMwh: 920,
      acceptedWorkMutationAllowed: false,
      buyerChargeMutationAllowed: false,
      financialAdviceAllowed: false,
      gpuRentalFloorDollarsPerMwh: 4315.45,
      gridDispatchAllowed: false,
      liveWalletSpendAllowed: false,
      market: 'ercot',
      marketDataMutationAllowed: false,
      miningFloorDollarsPerMwh: 78,
      nodePowerAdjustedFloorDollarsPerMwh: 4315.45,
      powerCostDollarsPerMwh: 31.2,
      providerSettlementMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      settlementMutationAllowed: false,
      tokenInferenceFloorDollarsPerMwh: null,
      tokenInferencePublicBlocked: true,
      updatedAtDisplay: '10 minutes ago',
    })
    expect(projection.publicBlockerRefs).toContain(
      'blocker.public.artanis_pylon_economics.token_unit_audit_required',
    )
    expect(projection.valueRows.map(row => row.label)).toEqual([
      'Mining floor',
      'GPU rental floor',
      'Token inference floor',
      'Node-power-adjusted floor',
      'Accepted-outcome value',
      'Power cost',
    ])
    expect(projection.privateEvidenceRefs).toEqual([])
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(artanisPylonComparativeEconomicsProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('projects a modeled NYISO packet as a supported market', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const projection = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket({
        powerMarket: {
          ...base.powerMarket,
          evidenceRef: 'evidence.public.nyiso.zone_j_lmp_window',
          market: 'nyiso',
          sourceRefs: ['source.public.nyiso_lmp_cache'],
          windowRef: 'window.public.nyiso_zone_j.20260601_20260606',
          zoneOrSettlementPoint: 'NYISO Zone J',
        },
        windowRef: 'window.public.nyiso_zone_j.20260601_20260606',
      }),
      'public',
      nowIso,
    )

    expect(projection.market).toBe('nyiso')
    expect(projection.powerCostDollarsPerMwh).toBe(31.2)
    expect(projection.sourceRefs).toContain('source.public.nyiso_lmp_cache')
  })

  test('redacts private evidence publicly and retains safe private refs for operator projection', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const record = economicsPacket({
      acceptedOutcome: {
        ...base.acceptedOutcome,
        artifactRefs: [
          'artifact.public.trace_summary',
          'artifact.private.operator_trace',
        ],
        economicsRefs: [
          'economics.public.trace_summary',
          'economics.private.operator_trace',
        ],
      },
      privateEvidenceRefs: [
        'evidence.private.operator.margot_export_trace',
        'meter.private.operator.pdu_snapshot',
      ],
      provenance: {
        ...base.provenance,
        sourceUrlRefs: [
          'https://github.com/dmrobotix/oa_aibtc_model',
          'url.private.operator_dashboard',
        ],
      },
      sourceRefs: [
        'source.public.margot_facility_simulator',
        'source.private.operator_export',
      ],
    })

    const publicProjection = projectArtanisPylonComparativeEconomicsPacket(
      record,
      'public',
      nowIso,
    )
    const operatorProjection = projectArtanisPylonComparativeEconomicsPacket(
      record,
      'operator',
      nowIso,
    )

    expect(publicProjection.privateEvidenceRefs).toEqual([])
    expect(publicProjection.evidenceRefs).not.toContain(
      'artifact.private.operator_trace',
    )
    expect(publicProjection.sourceRefs).toContain(
      'source.public.margot_facility_simulator',
    )
    expect(publicProjection.sourceRefs).not.toContain(
      'source.private.operator_export',
    )
    expect(publicProjection.sourceUrlRefs).toEqual([
      'https://github.com/dmrobotix/oa_aibtc_model',
    ])
    expect(operatorProjection.privateEvidenceRefs).toEqual([
      'evidence.private.operator.margot_export_trace',
      'meter.private.operator.pdu_snapshot',
    ])
    expect(operatorProjection.evidenceRefs).toContain(
      'artifact.private.operator_trace',
    )
    expect(operatorProjection.sourceRefs).toContain(
      'source.private.operator_export',
    )
    expect(operatorProjection.sourceUrlRefs).toContain(
      'url.private.operator_dashboard',
    )
  })

  test('blocks public token dollars per MWh until OpenRouter and ML Energy unit audit is verified', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const pending = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket(),
      'public',
      nowIso,
    )
    const verified = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket({
        tokenEconomics: {
          ...base.tokenEconomics,
          caveatRefs: ['caveat.public.openrouter_ml_energy_units_verified'],
          claimState: 'measured',
          unitAuditState: 'verified',
        },
      }),
      'public',
      nowIso,
    )

    expect(pending.tokenInferenceFloorDollarsPerMwh).toBeNull()
    expect(pending.tokenInferencePublicBlocked).toBe(true)
    expect(verified.tokenInferenceFloorDollarsPerMwh).toBe(6400)
    expect(verified.tokenInferencePublicBlocked).toBe(false)
    expect(verified.publicBlockerRefs).not.toContain(
      'blocker.public.artanis_pylon_economics.token_unit_audit_required',
    )
  })

  test('requires explicit unsupported-market caveats for markets outside ERCOT and NYISO', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const unsupported = economicsPacket({
      caveatRefs: [
        ...base.caveatRefs,
        'caveat.public.unsupported_market_pjm',
      ],
      powerMarket: {
        ...base.powerMarket,
        caveatRefs: ['caveat.public.unsupported_market_pjm'],
        claimState: 'unsupported',
        market: 'unsupported',
        sourceRefs: ['source.public.pjm_pending_api_access'],
        windowRef: 'window.public.pjm.unsupported',
        zoneOrSettlementPoint: 'PJM',
      },
      windowRef: 'window.public.pjm.unsupported',
    })
    const projection = projectArtanisPylonComparativeEconomicsPacket(
      unsupported,
      'operator',
      nowIso,
    )

    expect(projection.market).toBe('unsupported')
    expect(projection.publicBlockerRefs).toContain(
      'blocker.public.artanis_pylon_economics.unsupported_market',
    )
    expect(() =>
      projectArtanisPylonComparativeEconomicsPacket(
        economicsPacket({
          powerMarket: {
            ...base.powerMarket,
            caveatRefs: ['caveat.public.market_pending_support'],
            market: 'unsupported',
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisPylonComparativeEconomicsUnsafe)
  })

  test('marks stale evidence as a public blocker without changing claim state labels', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const projection = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket({
        pylonCapacity: {
          ...base.pylonCapacity,
          caveatRefs: ['caveat.public.pylon_capacity_refresh_needed'],
          claimState: 'stale',
        },
      }),
      'public',
      nowIso,
    )

    expect(projection.publicBlockerRefs).toContain(
      'blocker.public.artanis_pylon_economics.stale_source',
    )
    expect(projection.valueRows.find(
      row => row.label === 'Node-power-adjusted floor',
    )?.claimStateLabel).toBe('Stale')
  })

  test('does not mislabel chip TDP values as node or facility power', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const projection = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket({
        pylonCapacity: {
          ...base.pylonCapacity,
          caveatRefs: ['caveat.public.chip_tdp_not_node_or_facility_power'],
          denominatorKind: 'chip_tdp',
        },
      }),
      'public',
      nowIso,
    )

    expect(projection.nodePowerAdjustedFloorDollarsPerMwh).toBeNull()
    expect(projection.publicBlockerRefs).toContain(
      'blocker.public.artanis_pylon_economics.chip_tdp_not_node_power',
    )
    expect(projection.valueRows.find(
      row => row.label === 'Node-power-adjusted floor',
    )).toMatchObject({
      denominatorKind: 'chip_tdp',
      dollarsPerMwh: null,
    })
  })

  test('keeps accepted work payable and settled states separate', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()
    const payable = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket(),
      'operator',
      nowIso,
    )
    const settled = projectArtanisPylonComparativeEconomicsPacket(
      economicsPacket({
        acceptedOutcome: {
          ...base.acceptedOutcome,
          claimState: 'settled',
          providerSettledCents: 26000,
          settlementRefs: ['settlement.public.trace_summary'],
        },
      }),
      'operator',
      nowIso,
    )

    expect(payable.claimStateRefs).toContain(
      'claim_state.payable.accepted_outcome',
    )
    expect(payable.evidenceRefs).not.toContain('settlement.public.trace_summary')
    expect(settled.claimStateRefs).toContain(
      'claim_state.settled.accepted_outcome',
    )
    expect(settled.evidenceRefs).toContain('settlement.public.trace_summary')
    expect(() =>
      projectArtanisPylonComparativeEconomicsPacket(
        economicsPacket({
          acceptedOutcome: {
            ...base.acceptedOutcome,
            claimState: 'payable',
            providerSettledCents: 1,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisPylonComparativeEconomicsUnsafe)
    expect(() =>
      projectArtanisPylonComparativeEconomicsPacket(
        economicsPacket({
          acceptedOutcome: {
            ...base.acceptedOutcome,
            claimState: 'settled',
            providerSettledCents: 26000,
            settlementRefs: [],
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisPylonComparativeEconomicsUnsafe)
  })

  test('rejects mutable authority, unsafe refs, private URLs, raw material, and invalid numbers', () => {
    const base = exampleArtanisPylonComparativeEconomicsPacket()

    for (const record of [
      economicsPacket({
        authority: {
          ...ARTANIS_PYLON_COMPARATIVE_ECONOMICS_READ_ONLY_AUTHORITY,
          noLiveWalletSpend: false,
        },
      }),
      economicsPacket({
        pylonCapacity: {
          ...base.pylonCapacity,
          sourceRefs: ['raw_meter.telemetry'],
        },
      }),
      economicsPacket({
        provenance: {
          ...base.provenance,
          sourceUrlRefs: ['https://github.com/team/private-repo'],
        },
      }),
      economicsPacket({
        privateEvidenceRefs: ['wallet.secret.material'],
      }),
      economicsPacket({
        acceptedOutcome: {
          ...base.acceptedOutcome,
          economicsRefs: ['customer_email.hidden'],
        },
      }),
      economicsPacket({
        gpuRental: {
          ...base.gpuRental,
          derivedDollarsPerMwh: -1,
        },
      }),
      economicsPacket({
        gpuRental: {
          ...base.gpuRental,
          listingSampleSize: 0,
        },
      }),
    ]) {
      expect(() =>
        projectArtanisPylonComparativeEconomicsPacket(
          record,
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisPylonComparativeEconomicsUnsafe)
    }
  })
})
