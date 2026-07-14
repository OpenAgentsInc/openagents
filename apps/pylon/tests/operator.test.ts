import { describe, expect, test } from 'vite-plus/test'
import { projectHostInventoryFixture } from '../src/inventory'
import { createOperatorSnapshot, formatOperatorSnapshotText } from '../src/operator'
import { assertPublicProjectionSafe } from '../src/state'

const inventory = projectHostInventoryFixture({
  platform: 'darwin',
  arch: 'arm64',
  cpuCores: 10,
  cpuModel: 'Apple M2 Pro',
  totalMemoryBytes: 32 * 1024 * 1024 * 1024,
  freeMemoryBytes: 10 * 1024 * 1024 * 1024,
  homeFreeBytes: 80 * 1024 * 1024 * 1024,
  networkInterfaceCount: 5,
  externalNetworkInterfaceCount: 2,
  opencodeInstalled: true,
  appleFmReady: true,
  now: '2026-06-09T00:00:00.000Z',
})

describe('Pylon operator snapshot', () => {
  test('builds inventory-only state with payments retired and paid fallback denied', () => {
    const snapshot = createOperatorSnapshot({
      inventory,
      recentJobRefs: ['assignment.public.job1'],
      receiptRefs: ['assignment.closeout.public1'],
    })
    const text = formatOperatorSnapshotText(snapshot)

    expect(snapshot.schema).toBe('openagents.pylon.operator_snapshot.v0.4')
    expect(snapshot.desiredMode).toBe('automated')
    expect(snapshot.paymentCapability).toEqual({
      state: 'retired',
      mutationAllowed: false,
      paidCapacityFallbackAllowed: false,
      reasonRef: 'reason.public.pylon.money_capability_retired.v1',
    })
    expect(snapshot.inspect.eligibleInventoryCount).toBe(1)
    expect(snapshot.recovery.operatorOptInRequired).toBe(true)
    expect(snapshot.recovery.sandboxProfileRequired).toBe(true)
    expect(snapshot.recovery.budgetRequired).toBe(true)
    expect(text).toContain('Payments: retired')
    expect(text).toContain('Paid capacity fallback: denied')
    expect(text).not.toContain('Wallet:')
    assertPublicProjectionSafe(snapshot)
  })

  test('keeps local-agent self-steering blocked behind explicit operator gates', () => {
    const snapshot = createOperatorSnapshot({ inventory })

    expect(snapshot.recovery.headlessCommandRefs).toContain('command.pylon.status_json')
    expect(snapshot.recovery.headlessCommandRefs).toContain('command.pylon.assignment_poll')
    expect(snapshot.recovery.headlessCommandRefs).not.toContain('command.pylon.wallet_status')
    expect(snapshot.paymentCapability.mutationAllowed).toBe(false)
  })
})
