import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_WALLET_TELEMETRY_CONFORMANCE_FIXTURES,
  PYLON_WALLET_TELEMETRY_READ_ONLY_AUTHORITY,
  PylonWalletTelemetryProjection,
  PylonWalletTelemetryRecord,
  PylonWalletTelemetryUnsafe,
  projectPylonWalletTelemetry,
  pylonWalletTelemetryCanMutateWallet,
  pylonWalletTelemetryHasNoMutationAuthority,
  pylonWalletTelemetryProjectionHasPrivateMaterial,
} from './pylon-wallet-telemetry'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T08:00:00.000Z'

const telemetryRecord = (
  overrides: Partial<PylonWalletTelemetryRecord> = {},
): PylonWalletTelemetryRecord =>
  S.decodeUnknownSync(PylonWalletTelemetryRecord)({
    ...PYLON_WALLET_TELEMETRY_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon wallet telemetry projection', () => {
  test('decodes fixtures and projects operator-safe telemetry without mutation authority', () => {
    const record = telemetryRecord()
    const projection = projectPylonWalletTelemetry(record, 'operator', nowIso)

    expect(S.decodeUnknownSync(PylonWalletTelemetryRecord)(record)).toEqual(
      record,
    )
    expect(S.decodeUnknownSync(PylonWalletTelemetryProjection)(projection))
      .toEqual(projection)
    expect(pylonWalletTelemetryHasNoMutationAuthority(record.authority)).toBe(
      true,
    )
    expect(pylonWalletTelemetryCanMutateWallet(record)).toBe(false)
    expect(projection.walletMutationAllowed).toBe(false)
    expect(projection.channelMutationAllowed).toBe(false)
    expect(projection.lspMutationAllowed).toBe(false)
    expect(projection.backupMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.items.map(item => item.surface).sort()).toEqual([
      'backup',
      'channel',
      'liquidity',
      'lsp',
      'sync',
      'warning',
    ])
    expect(projection.createdAtDisplay).toBe('1 hour ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonWalletTelemetryProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('redacts private provider, wallet, channel, liquidity, lsp, operator, and source refs from public projection', () => {
    const projection = projectPylonWalletTelemetry(
      telemetryRecord(),
      'public',
      nowIso,
    )
    const channel = projection.items.find(item => item.surface === 'channel')!
    const liquidity = projection.items.find(
      item => item.surface === 'liquidity',
    )!
    const lsp = projection.items.find(item => item.surface === 'lsp')!

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.walletRef).toBe('wallet.redacted')
    expect(channel.evidenceRefs).toEqual(['channel.public.posture_summary'])
    expect(channel.operatorActionRefs).toEqual([])
    expect(liquidity.evidenceRefs).toEqual([])
    expect(liquidity.operatorActionRefs).toEqual([])
    expect(lsp.sourceRefs).toEqual([])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires all telemetry surfaces and evidence for stale, degraded, critical, and blocked states', () => {
    const [firstItem, ...restItems] = telemetryRecord().items

    expect(() =>
      projectPylonWalletTelemetry(
        telemetryRecord({ items: restItems }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletTelemetryUnsafe)

    expect(() =>
      projectPylonWalletTelemetry(
        telemetryRecord({
          items: [
            {
              ...firstItem!,
              caveatRefs: [],
              freshness: 'stale',
              operatorActionRefs: [],
              state: 'degraded',
              warningRefs: [],
            },
            ...restItems,
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletTelemetryUnsafe)

    expect(() =>
      projectPylonWalletTelemetry(
        telemetryRecord({
          items: [
            {
              ...firstItem!,
              operatorActionRefs: [],
              severity: 'critical',
            },
            ...restItems,
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletTelemetryUnsafe)

    expect(() =>
      projectPylonWalletTelemetry(
        telemetryRecord({
          items: [
            {
              ...firstItem!,
              blockerRefs: [],
              severity: 'blocked',
              state: 'blocked',
            },
            ...restItems,
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletTelemetryUnsafe)
  })

  test('rejects mutable authority', () => {
    expect(() =>
      projectPylonWalletTelemetry(
        telemetryRecord({
          authority: {
            ...PYLON_WALLET_TELEMETRY_READ_ONLY_AUTHORITY,
            noBackupMutation: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonWalletTelemetryUnsafe)
  })

  test('rejects recovery phrases, raw entropy, private keys, preimages, raw channel monitor state, credentials, and raw telemetry', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw entropy', value: 'entropy.wallet_seed_dump' },
      { label: 'private key', value: 'private_key.wallet_dump' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
      { label: 'raw telemetry', value: 'raw_telemetry.wallet_status' },
      { label: 'backup secret', value: 'raw_backup.seed_phrase' },
      { label: 'provider secret', value: 'provider_secret.local_node' },
    ]) {
      expect(() =>
        projectPylonWalletTelemetry(
          telemetryRecord({
            items: [
              {
                ...telemetryRecord().items[0]!,
                evidenceRefs: [fixture.value],
              },
              ...telemetryRecord().items.slice(1),
            ],
          }),
          'operator',
          nowIso,
        ),
      ).toThrow(PylonWalletTelemetryUnsafe)
    }
  })
})
