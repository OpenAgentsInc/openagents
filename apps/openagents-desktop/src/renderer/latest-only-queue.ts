/**
 * A bounded async projection queue for high-frequency renderer streams.
 *
 * At most the value currently being processed and the newest submitted value
 * are retained. Intermediate values are deliberately superseded: live UI
 * projection is state, not an event log, and the durable main-process journal
 * remains the authority for every provider event.
 */
export const makeLatestOnlyQueue = <Value>(
  process: (value: Value) => Promise<void>,
): Readonly<{
  submit: (value: Value) => void
  flush: () => Promise<void>
}> => {
  let latest: Value | null = null
  let running: Promise<void> | null = null

  const ensureRunning = (): void => {
    if (running !== null || latest === null) return
    running = (async () => {
      while (latest !== null) {
        const value = latest
        latest = null
        await process(value)
      }
    })().finally(() => {
      running = null
      // A submit can land after the loop observes null but before this
      // finalizer runs. Re-check so that value cannot be stranded.
      ensureRunning()
    })
  }

  return {
    submit: value => {
      latest = value
      ensureRunning()
    },
    flush: async () => {
      while (running !== null || latest !== null) {
        ensureRunning()
        await running
      }
    },
  }
}
