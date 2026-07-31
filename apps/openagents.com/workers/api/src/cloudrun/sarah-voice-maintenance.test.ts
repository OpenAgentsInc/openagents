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
          return { cleaned: 3, failed: 1, abandoned: 0 }
        },
        retireStaleRoomMembers: async () => {
          order.push('members')
          return 4
        },
      },
      report,
    )

    expect(order).toEqual(['expiry', 'provisioning', 'terminal', 'members'])
    expect(report.mock.calls).toEqual([
      ['sarah_voice_expired_sessions_processed', { swept: 2 }],
      [
        'sarah_livekit_provisioning_intents_reconciled',
        { cleaned: 1, failed: 0 },
      ],
      [
        'sarah_livekit_terminal_rooms_reconciled',
        { cleaned: 3, failed: 1, abandoned: 0 },
      ],
      ['sarah_livekit_stale_room_members_retired', { retired: 4 }],
    ])
  })

  // EP263-LK H4 (#9282): giving up on a room is an operator-visible decision.
  // A bounded retry that stops silently is the same defect as an unbounded one
  // that never stops, only quieter.
  test('reports abandoned terminal rooms as their own event', async () => {
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        reconcileTerminalRooms: async () => ({
          cleaned: 0,
          failed: 0,
          abandoned: 2,
        }),
      },
      report,
    )

    expect(report.mock.calls).toEqual([
      [
        'sarah_livekit_terminal_rooms_reconciled',
        { cleaned: 0, failed: 0, abandoned: 2 },
      ],
      ['sarah_livekit_terminal_rooms_abandoned', { abandoned: 2 }],
    ])
  })

  test('reports a stale member retirement failure without stopping the tick', async () => {
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        retireStaleRoomMembers: async () => {
          throw new Error('members unavailable')
        },
      },
      report,
    )

    expect(report.mock.calls).toEqual([
      [
        'sarah_livekit_stale_room_members_retirement_failed',
        { error: 'members unavailable' },
      ],
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
          return { cleaned: 1, failed: 0, abandoned: 0 }
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
      [
        'sarah_livekit_terminal_rooms_reconciled',
        { cleaned: 1, failed: 0, abandoned: 0 },
      ],
    ])
  })
})
