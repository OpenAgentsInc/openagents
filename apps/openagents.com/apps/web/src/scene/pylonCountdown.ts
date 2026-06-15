import { Duration, Effect, Fiber } from 'effect'
import { slotText } from 'slot-text'

// Drives a centered 12-hour countdown using the slot-text vanilla roll effect
// for the digit animation and Effect for the once-a-second tick loop. The
// caller supplies the target element (already styled + carrying the slot-text
// CSS in its shadow root); this module owns the timer and the rolls.

const TWELVE_HOURS_MS = 12 * 60 * 60 * 1000

const pad = (value: number): string => value.toString().padStart(2, '0')

export const formatRemaining = (ms: number): string => {
  const total = Math.floor(Math.max(0, ms) / 1000)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`
}

export type PylonCountdownHandle = Readonly<{
  dispose: () => void
}>

export const mountPylonCountdown = (
  target: HTMLElement,
  durationMs: number = TWELVE_HOURS_MS,
): PylonCountdownHandle => {
  const controller = slotText(target, formatRemaining(durationMs), {
    direction: 'down',
  })

  const deadline = Date.now() + durationMs
  let disposed = false

  // Effect-driven tick: sleep one second, render the remaining time (only the
  // digits that changed roll), and stop once the countdown reaches zero.
  const loop = Effect.gen(function* () {
    while (true) {
      yield* Effect.sleep(Duration.seconds(1))
      if (disposed) return
      const remaining = Math.max(0, deadline - Date.now())
      controller.set(formatRemaining(remaining))
      if (remaining <= 0) return
    }
  })

  const fiber = Effect.runFork(loop)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      Effect.runFork(Fiber.interrupt(fiber))
      controller.destroy()
    },
  }
}
