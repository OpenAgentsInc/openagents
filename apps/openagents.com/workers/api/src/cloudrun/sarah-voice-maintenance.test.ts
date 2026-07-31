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

  /**
   * A stuck `accounting_uncertain` hold keeps its owner's single voice
   * concurrency slot, and `sweepExpired` — the function that opens that state —
   * cannot close it, so the per-minute sweep reports healthy while every later
   * session for that identity is refused. Every LiveKit lane shares the same
   * two acceptance identities, so one stuck hold denies voice to all of them.
   * Diagnosing that cost three lanes about an hour, because nothing reported it.
   */
  test('reports stuck accounting-uncertain holds', async () => {
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        reportStuckAccountingHolds: async () => ({
          stuck: 2,
          owners: 1,
          oldestAgeMs: 3_600_000,
        }),
      },
      report,
    )

    expect(report.mock.calls).toEqual([
      [
        'sarah_voice_accounting_uncertain_holds_stuck',
        { stuck: 2, owners: 1, oldestAgeMs: 3_600_000 },
      ],
    ])
  })

  test('stays quiet when no hold has gone stale', async () => {
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        reportStuckAccountingHolds: async () => ({
          stuck: 0,
          owners: 0,
          oldestAgeMs: 0,
        }),
      },
      report,
    )

    expect(report).not.toHaveBeenCalled()
  })

  // The scan is additive. A database it cannot reach must not take the sweep,
  // the reconcilers, or the member retirement down with it.
  test('a failed stuck-hold scan does not stop the rest of maintenance', async () => {
    const order: string[] = []
    const report = vi.fn()

    await runSarahVoiceScheduledMaintenance(
      {
        sweepExpired: async () => {
          order.push('expiry')
          return 0
        },
        reportStuckAccountingHolds: async () => {
          order.push('stuck')
          throw new Error('scan unavailable')
        },
        reconcileTerminalRooms: async () => {
          order.push('terminal')
          return { cleaned: 1, failed: 0, abandoned: 0 }
        },
      },
      report,
    )

    expect(order).toEqual(['expiry', 'stuck', 'terminal'])
    expect(report.mock.calls).toEqual([
      [
        'sarah_voice_accounting_uncertain_scan_failed',
        { error: 'scan unavailable' },
      ],
      [
        'sarah_livekit_terminal_rooms_reconciled',
        { cleaned: 1, failed: 0, abandoned: 0 },
      ],
    ])
  })
})
