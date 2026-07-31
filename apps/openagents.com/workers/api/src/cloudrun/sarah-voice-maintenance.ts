export type SarahVoiceMaintenanceCounts = Readonly<{
  cleaned: number
  failed: number
}>

/**
 * EP263-LK H4 (#9282): `abandoned` counts rooms whose bounded cleanup attempts
 * are spent. Giving up is a real outcome and must be reported, never inferred
 * from a loop that quietly stops trying.
 */
export type SarahVoiceTerminalRoomCounts = Readonly<{
  cleaned: number
  failed: number
  abandoned: number
}>

export type SarahVoiceScheduledMaintenanceOperations = Readonly<{
  sweepExpired?: (() => Promise<number>) | undefined
  reconcileProvisioning?:
    (() => Promise<SarahVoiceMaintenanceCounts>) | undefined
  reconcileTerminalRooms?:
    (() => Promise<SarahVoiceTerminalRoomCounts>) | undefined
  retireStaleRoomMembers?: (() => Promise<number>) | undefined
}>

export type SarahVoiceMaintenanceReport = (
  event: string,
  detail: Readonly<Record<string, unknown>>,
) => void

const errorDetail = (error: unknown): Readonly<Record<string, unknown>> => ({
  error: error instanceof Error ? error.message : String(error),
})

export const runSarahVoiceScheduledMaintenance = async (
  operations: SarahVoiceScheduledMaintenanceOperations,
  report: SarahVoiceMaintenanceReport,
): Promise<void> => {
  if (operations.sweepExpired !== undefined) {
    try {
      const swept = await operations.sweepExpired()
      if (swept > 0) report('sarah_voice_expired_sessions_processed', { swept })
    } catch (error) {
      report('sarah_voice_expiry_sweep_failed', errorDetail(error))
    }
  }

  if (operations.reconcileProvisioning !== undefined) {
    try {
      const result = await operations.reconcileProvisioning()
      if (result.cleaned > 0 || result.failed > 0) {
        report('sarah_livekit_provisioning_intents_reconciled', result)
      }
    } catch (error) {
      report(
        'sarah_livekit_provisioning_intents_reconciliation_failed',
        errorDetail(error),
      )
    }
  }

  if (operations.reconcileTerminalRooms !== undefined) {
    try {
      const result = await operations.reconcileTerminalRooms()
      if (result.cleaned > 0 || result.failed > 0 || result.abandoned > 0) {
        report('sarah_livekit_terminal_rooms_reconciled', result)
      }
      // A separate event, because "we stopped trying to delete this room" is
      // an operator-visible decision and not a detail of a success counter.
      if (result.abandoned > 0) {
        report('sarah_livekit_terminal_rooms_abandoned', {
          abandoned: result.abandoned,
        })
      }
    } catch (error) {
      report(
        'sarah_livekit_terminal_rooms_reconciliation_failed',
        errorDetail(error),
      )
    }
  }

  if (operations.retireStaleRoomMembers !== undefined) {
    try {
      const retired = await operations.retireStaleRoomMembers()
      if (retired > 0) {
        report('sarah_livekit_stale_room_members_retired', { retired })
      }
    } catch (error) {
      report(
        'sarah_livekit_stale_room_members_retirement_failed',
        errorDetail(error),
      )
    }
  }
}
