import { describe, expect, test, vi } from 'vitest'

import { runSarahVoiceScheduledMaintenance } from './sarah-voice-maintenance'

describe('Sarah voice scheduled maintenance', () => {
  test('runs expiry, orphan provisioning, and terminal room maintenance in order', async () => {
    const order: string[] = []
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        sweepExpired: async () => {
          order.push('expiry')
          return 2
        },
        reconcileProvisioning: async () => {
          order.push('provisioning')
          return { cleaned: 1, failed: 0 }
        },
        reconcileTerminalRooms: async () => {
          order.push('terminal')
          return { cleaned: 3, failed: 1 }
        },
      },
      report,
    )

    expect(order).toEqual(['expiry', 'provisioning', 'terminal'])
    expect(report.mock.calls).toEqual([
      ['sarah_voice_expired_sessions_processed', { swept: 2 }],
      [
        'sarah_livekit_provisioning_intents_reconciled',
        { cleaned: 1, failed: 0 },
      ],
      ['sarah_livekit_terminal_rooms_reconciled', { cleaned: 3, failed: 1 }],
    ])
  })

  test('isolates each step failure so later cleanup still runs', async () => {
    const order: string[] = []
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        sweepExpired: async () => {
          order.push('expiry')
          throw new Error('expiry unavailable')
        },
        reconcileProvisioning: async () => {
          order.push('provisioning')
          throw new Error('provisioning unavailable')
        },
        reconcileTerminalRooms: async () => {
          order.push('terminal')
          return { cleaned: 1, failed: 0 }
        },
      },
      report,
    )

    expect(order).toEqual(['expiry', 'provisioning', 'terminal'])
    expect(report.mock.calls).toEqual([
      ['sarah_voice_expiry_sweep_failed', { error: 'expiry unavailable' }],
      [
        'sarah_livekit_provisioning_intents_reconciliation_failed',
        { error: 'provisioning unavailable' },
      ],
      ['sarah_livekit_terminal_rooms_reconciled', { cleaned: 1, failed: 0 }],
    ])
  })
})
