import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_LDK_READINESS_CONFORMANCE_FIXTURES,
  PYLON_LDK_READINESS_READ_ONLY_AUTHORITY,
  PylonLdkReadinessProjection,
  PylonLdkReadinessRecord,
  PylonLdkReadinessUnsafe,
  projectPylonLdkReadiness,
  pylonLdkReadinessCanMutateSettlement,
  pylonLdkReadinessHasNoSpendAuthority,
  pylonLdkReadinessProjectionHasPrivateMaterial,
} from './pylon-ldk-readiness-projections'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T05:00:00.000Z'

const readinessRecord = (
  overrides: Partial<PylonLdkReadinessRecord> = {},
): PylonLdkReadinessRecord =>
  S.decodeUnknownSync(PylonLdkReadinessRecord)({
    ...PYLON_LDK_READINESS_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon LDK read-only readiness projections', () => {
  test('decodes fixtures and projects operator-safe readiness without spend authority', () => {
    const record = readinessRecord()
    const projection = projectPylonLdkReadiness(record, 'operator', nowIso)

    expect(PYLON_LDK_READINESS_CONFORMANCE_FIXTURES.map(fixture => fixture.id))
      .toEqual([
        'pylon_ldk_readiness.provider_1',
        'pylon_ldk_readiness.provider_2',
      ])
    expect(S.decodeUnknownSync(PylonLdkReadinessRecord)(record)).toEqual(
      record,
    )
    expect(S.decodeUnknownSync(PylonLdkReadinessProjection)(projection))
      .toEqual(projection)
    expect(pylonLdkReadinessHasNoSpendAuthority(record.authority)).toBe(true)
    expect(pylonLdkReadinessCanMutateSettlement(record)).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.channelOpenMutationAllowed).toBe(false)
    expect(projection.nexusMutationAllowed).toBe(false)
    expect(projection.treasuryMutationAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.createdAtDisplay).toBe('1 hour ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonLdkReadinessProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('redacts private provider, rail, balance, channel, failed-route, and operator refs from public projection', () => {
    const projection = projectPylonLdkReadiness(
      readinessRecord(),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.railRef).toBe('rail.redacted')
    expect(projection.balanceEvidenceRefs).toEqual([
      'balance.public.summary.pylon_provider_1',
    ])
    expect(projection.channelPostureRefs).toEqual([
      'channel.public.posture.pylon_provider_1',
    ])
    expect(projection.failedRouteRefs).toEqual([
      'failed_route.public.no_route_summary',
    ])
    expect(projection.operatorActionRefs).toEqual([])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires evidence for ready, blocked, and non-ready states', () => {
    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({
          balanceEvidenceRefs: [],
          channelPostureRefs: [],
          evidenceRefs: [],
          readinessState: 'ready',
          sourceRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)

    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({
          blockerRefs: [],
          readinessState: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)

    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({
          caveatRefs: [],
          operatorActionRefs: [],
          readinessState: 'degraded',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)
  })

  test('requires non-negative route counts and failed route refs for no-route counts', () => {
    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({ failedRouteCount: -1 }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)

    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({
          failedRouteRefs: [],
          noRouteCount: 1,
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)
  })

  test('rejects spend authority and raw wallet, payment, payout, channel, provider, and timestamp material', () => {
    expect(() =>
      projectPylonLdkReadiness(
        readinessRecord({
          authority: {
            ...PYLON_LDK_READINESS_READ_ONLY_AUTHORITY,
            noLiveWalletSpend: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonLdkReadinessUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw invoice', value: 'raw_invoice.bolt11_full' },
      { label: 'payout target', value: 'payout_target.bc1qtest' },
      { label: 'private channel monitor', value: 'channel_monitor.raw_state' },
      { label: 'provider secret', value: 'provider_secret.local_node' },
      { label: 'wallet preimage', value: 'wallet_payment_preimage.raw' },
    ]) {
      expect(() =>
        projectPylonLdkReadiness(
          readinessRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(PylonLdkReadinessUnsafe)
    }
  })
})
