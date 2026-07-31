export type SarahVoiceMaintenanceCounts = Readonly<{
  cleaned: number
  failed: number
}>

export type SarahVoiceScheduledMaintenanceOperations = Readonly<{
  sweepExpired?: (() => Promise<number>) | undefined
  reconcileProvisioning?:
    (() => Promise<SarahVoiceMaintenanceCounts>) | undefined
  reconcileTerminalRooms?:
    (() => Promise<SarahVoiceMaintenanceCounts>) | undefined
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
      if (result.cleaned > 0 || result.failed > 0) {
        report('sarah_livekit_terminal_rooms_reconciled', result)
      }
    } catch (error) {
      report(
        'sarah_livekit_terminal_rooms_reconciliation_failed',
        errorDetail(error),
      )
    }
  }
}
