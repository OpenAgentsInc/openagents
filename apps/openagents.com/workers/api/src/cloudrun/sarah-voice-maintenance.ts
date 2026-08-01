/**
 * EP263-LK H4 (#9282) and its provisioning-intent follow-up: `abandoned` counts
 * rows whose bounded attempts are spent. Giving up is a real outcome and must
 * be reported, never inferred from a loop that quietly stops trying.
 *
 * Both bounded reconcilers report this same shape. They fail for the same
 * reason and give up on the same ladder, so one counter vocabulary covers both
 * and an operator reading either event knows what the third number means.
 */
export type SarahVoiceConvergenceCounts = Readonly<{
  cleaned: number
  failed: number
  abandoned: number
}>

/**
 * A hold that has been waiting on provider reconciliation long enough to be
 * worth a human. Counts only — no session, owner, provider, room, or job
 * identifier rides in an operator log line.
 */
export type SarahVoiceStuckAccountingHolds = Readonly<{
  stuck: number
  owners: number
  oldestAgeMs: number
}>

export type SarahVoiceScheduledMaintenanceOperations = Readonly<{
  sweepExpired?: (() => Promise<number>) | undefined
  /**
   * Records a durable escalation for overdue accounting holds. The store
   * removes only the voice-concurrency lock; charge, hold, and provider
   * accounting remain unchanged until exact operator reconciliation.
   */
  escalateStuckAccountingHolds?:
    | (() => Promise<
        Readonly<{
          escalated: number
          owners: number
          oldestAgeMs: number
        }>
      >)
    | undefined
  /**
   * Reports `accounting_uncertain` holds that have gone stale.
   *
   * Additive and read-only. Before bounded escalation, an unreconciled hold
   * occupies its owner's voice slot; after escalation, it remains a financial
   * incident that needs complete provider evidence. This signal covers both
   * phases without carrying identifiers into the log.
   *
   * It must never settle or expire a hold. Escalated rows remain visible until
   * exact reconciliation moves the money and closes the incident.
   */
  reportStuckAccountingHolds?:
    (() => Promise<SarahVoiceStuckAccountingHolds>) | undefined
  reconcileProvisioning?:
    (() => Promise<SarahVoiceConvergenceCounts>) | undefined
  reconcileTerminalRooms?:
    (() => Promise<SarahVoiceConvergenceCounts>) | undefined
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

  if (operations.escalateStuckAccountingHolds !== undefined) {
    try {
      const result = await operations.escalateStuckAccountingHolds()
      if (result.escalated > 0) {
        report('sarah_voice_accounting_uncertain_holds_escalated', result)
      }
    } catch (error) {
      report(
        'sarah_voice_accounting_uncertain_escalation_failed',
        errorDetail(error),
      )
    }
  }

  if (operations.reportStuckAccountingHolds !== undefined) {
    try {
      const holds = await operations.reportStuckAccountingHolds()
      if (holds.stuck > 0) {
        report('sarah_voice_accounting_uncertain_holds_stuck', holds)
      }
    } catch (error) {
      report('sarah_voice_accounting_uncertain_scan_failed', errorDetail(error))
    }
  }

  if (operations.reconcileProvisioning !== undefined) {
    try {
      const result = await operations.reconcileProvisioning()
      if (result.cleaned > 0 || result.failed > 0 || result.abandoned > 0) {
        report('sarah_livekit_provisioning_intents_reconciled', result)
      }
      // A separate event, for the same reason the terminal-room sweep has one:
      // "we stopped trying to clean this intent" is an operator-visible
      // decision, not a detail of a success counter.
      if (result.abandoned > 0) {
        report('sarah_livekit_provisioning_intents_abandoned', {
          abandoned: result.abandoned,
        })
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
