import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_WALLET_LIQUIDITY_CONFORMANCE_FIXTURES,
  PYLON_WALLET_LIQUIDITY_READ_ONLY_AUTHORITY,
  PylonWalletLiquidityProjection,
  PylonWalletLiquidityRecord,
  PylonWalletLiquidityUnsafe,
  projectPylonWalletLiquidity,
  pylonWalletLiquidityCanMutateWallet,
  pylonWalletLiquidityHasNoSpendAuthority,
  pylonWalletLiquidityProjectionHasPrivateMaterial,
} from './pylon-wallet-liquidity-readiness'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T07:00:00.000Z'

const liquidityRecord = (
  overrides: Partial<PylonWalletLiquidityRecord> = {},
): PylonWalletLiquidityRecord =>
  S.decodeUnknownSync(PylonWalletLiquidityRecord)({
    ...PYLON_WALLET_LIQUIDITY_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon wallet liquidity readiness', () => {
  test('decodes fixtures and projects operator-safe liquidity readiness without mutation authority', () => {
    const record = liquidityRecord()
    const projection = projectPylonWalletLiquidity(record, 'operator', nowIso)

    expect(S.decodeUnknownSync(PylonWalletLiquidityRecord)(record)).toEqual(
      record,
    )
    expect(S.decodeUnknownSync(PylonWalletLiquidityProjection)(projection))
      .toEqual(projection)
    expect(pylonWalletLiquidityHasNoSpendAuthority(record.authority)).toBe(
      true,
    )
    expect(pylonWalletLiquidityCanMutateWallet(record)).toBe(false)
    expect(projection.walletMutationAllowed).toBe(false)
    expect(projection.channelMutationAllowed).toBe(false)
    expect(projection.liquidityProvisionMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.sendReadinessLabel).toBe('Blocked')
    expect(projection.receiveReadinessLabel).toBe('Ready')
    expect(projection.createdAtDisplay).toBe('1 hour ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(projection.buckets.map(bucket => bucket.bucket).sort()).toEqual([
      'anchor_reserve',
      'inbound_liquidity',
      'outbound_liquidity',
      'spendable_onchain',
      'total_channel_balance',
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonWalletLiquidityProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('redacts private provider, wallet, amount, channel, target, sync, and evidence refs for public projection', () => {
    const projection = projectPylonWalletLiquidity(
      liquidityRecord(),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.walletRef).toBe('wallet.redacted')
    expect(projection.channelPostureRefs).toEqual([
      'channel.public.posture.summary',
    ])
    expect(projection.payoutTargetAdmissionRefs).toEqual([
      'target.public.admission_pending',
    ])
    expect(projection.syncRefs).toEqual(['sync.public.wallet_fresh'])
    expect(projection.buckets.find(
      bucket => bucket.bucket === 'anchor_reserve',
    )?.amountRef).toBeNull()
    expect(projection.buckets.find(
      bucket => bucket.bucket === 'outbound_liquidity',
    )?.blockerRefs).toEqual(['blocker.public.no_outbound_liquidity'])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires every liquidity bucket and evidence for reported or verified buckets', () => {
    const [firstBucket, ...restBuckets] = liquidityRecord().buckets

    expect(() =>
      projectPylonWalletLiquidity(
        liquidityRecord({ buckets: restBuckets }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletLiquidityUnsafe)

    expect(() =>
      projectPylonWalletLiquidity(
        liquidityRecord({
          buckets: [
            {
              ...firstBucket!,
              amountRef: null,
              evidenceRefs: [],
              evidenceState: 'verified',
            },
            ...restBuckets,
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletLiquidityUnsafe)
  })

  test('requires blockers or warnings when liquidity state needs attention', () => {
    const record = liquidityRecord({
      blockerRefs: [],
      caveatRefs: [],
      receiveReadiness: 'ready',
      sendReadiness: 'blocked',
      warningRefs: [],
    })

    expect(() =>
      projectPylonWalletLiquidity(record, 'operator', nowIso),
    ).toThrow(PylonWalletLiquidityUnsafe)

    expect(() =>
      projectPylonWalletLiquidity(
        liquidityRecord({
          caveatRefs: [],
          receiveReadiness: 'not_ready',
          sendReadiness: 'ready',
          warningRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletLiquidityUnsafe)
  })

  test('rejects mutable authority and raw wallet, channel, payment, payout, provider, and telemetry material', () => {
    expect(() =>
      projectPylonWalletLiquidity(
        liquidityRecord({
          authority: {
            ...PYLON_WALLET_LIQUIDITY_READ_ONLY_AUTHORITY,
            noWalletMutation: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletLiquidityUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
      { label: 'raw liquidity', value: 'raw_liquidity.channel_dump' },
      { label: 'payout target', value: 'payout_target.raw_bc1qtest' },
      { label: 'wallet preimage', value: 'wallet_payment_preimage.raw' },
      { label: 'provider secret', value: 'provider_secret.local_node' },
    ]) {
      expect(() =>
        projectPylonWalletLiquidity(
          liquidityRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(PylonWalletLiquidityUnsafe)
    }
  })
})
