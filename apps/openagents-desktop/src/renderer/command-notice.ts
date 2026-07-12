/**
 * Transient command-notice controller (#8712 follow-up).
 *
 * `commandNotice` is the public-safe result string of the latest deferred /
 * native command admission (duplicate/unavailable rejection) or a keybinding
 * save failure. Owner report (verbatim): "what is that yellow command request
 * shit at top, is that supposed to be there, fix if not". It was NOT supposed
 * to persist: once set (e.g. a duplicate ⌘N), nothing cleared it until the
 * next successful command, so the banner sat at the top of the window forever.
 *
 * This controller makes the notice a TRANSIENT toast: setting a notice schedules
 * an Effect-timed clear (a forked fiber, never a leaked raw setTimeout), and any
 * new notice cancels the prior pending clear. Dismiss clears immediately. The
 * fiber is interrupt-safe: the mount registers `shutdown` as a scope finalizer
 * so a pending clear can never fire after the renderer unmounts. The rejection
 * itself is unchanged — the command is still rejected/ignored; only the NOTICE
 * presentation changes from permanent to transient (CUT-15 stays a real
 * visible rejection).
 */
import { Effect, Fiber, SubscriptionRef } from "@effect-native/core/effect"
import type { DesktopShellState } from "./shell.ts"

/** Bounded auto-dismiss delay for a transient command notice, in milliseconds. */
export const commandNoticeAutoDismissMillis = 4500

export interface CommandNoticeController {
  /**
   * Sets the notice string and (re)arms a single pending auto-clear, cancelling
   * any prior pending clear first. Returns after the clear fiber is armed; the
   * clear itself fires after the bounded delay.
   */
  readonly setTransientNotice: (message: string) => Effect.Effect<void>
  /** Clears the notice immediately and cancels any pending auto-clear. */
  readonly dismissNotice: Effect.Effect<void>
  /** Interrupts any pending auto-clear without touching state (unmount path). */
  readonly shutdown: Effect.Effect<void>
}

export const makeCommandNoticeController = (
  state: SubscriptionRef.SubscriptionRef<DesktopShellState>,
  delayMillis: number = commandNoticeAutoDismissMillis,
): CommandNoticeController => {
  // Single pending auto-clear fiber. A new notice or a dismiss interrupts it,
  // so at most one timer is ever live for the shared commandNotice field.
  let pending: Fiber.Fiber<void, never> | null = null

  const cancelPending = Effect.suspend(() => {
    const fiber = pending
    pending = null
    return fiber === null ? Effect.void : Fiber.interrupt(fiber)
  })

  const setTransientNotice = (message: string) =>
    Effect.gen(function* () {
      yield* cancelPending
      yield* SubscriptionRef.update(state, (current) => ({ ...current, commandNotice: message }))
      // forkDetach (outside the child scope) so the clear survives the
      // short-lived dispatch fiber, yet stays interrupt-safe: cancelPending /
      // shutdown interrupt it explicitly.
      const fiber = yield* Effect.forkDetach(
        Effect.sleep(delayMillis).pipe(
          Effect.andThen(
            SubscriptionRef.update(state, (current) =>
              // Only clear the notice this timer armed — a belt-and-suspenders
              // guard on top of the cancel-prior discipline.
              current.commandNotice === message ? { ...current, commandNotice: null } : current),
          ),
        ),
      )
      yield* Effect.sync(() => {
        pending = fiber
      })
    })

  const dismissNotice = cancelPending.pipe(
    Effect.andThen(
      SubscriptionRef.update(state, (current) =>
        current.commandNotice === null ? current : { ...current, commandNotice: null }),
    ),
  )

  return { setTransientNotice, dismissNotice, shutdown: cancelPending }
}
