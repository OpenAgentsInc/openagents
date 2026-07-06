/**
 * Pure state machine for one grant/clawback action attempt in the Aiur
 * credits console (AIUR-2, #8500). Shared by `GrantForm` and `ClawbackForm`
 * via `useReducer` so the transition logic is unit-testable independent of
 * React rendering.
 *
 * States: idle -> confirming -> submitting -> (success | error).
 * `cancel` returns to idle from confirming. `retry` returns to confirming
 * from error (re-using the SAME action ref — a retry is the same attempt,
 * never a new idempotency key). `reset` clears everything (e.g. after
 * switching the selected target user).
 */

export type CreditsActionState =
  | Readonly<{ status: 'idle' }>
  | Readonly<{ status: 'confirming'; actionRef: string }>
  | Readonly<{ status: 'submitting'; actionRef: string }>
  | Readonly<{ status: 'success'; actionRef: string; messageSafe: string }>
  | Readonly<{ status: 'error'; actionRef: string; messageSafe: string }>

export type CreditsActionEvent =
  | Readonly<{ type: 'start_confirm'; actionRef: string }>
  | Readonly<{ type: 'cancel' }>
  | Readonly<{ type: 'submit' }>
  | Readonly<{ type: 'succeed'; messageSafe: string }>
  | Readonly<{ type: 'fail'; messageSafe: string }>
  | Readonly<{ type: 'retry' }>
  | Readonly<{ type: 'reset' }>

export const initialCreditsActionState: CreditsActionState = { status: 'idle' }

export const creditsActionReducer = (
  state: CreditsActionState,
  event: CreditsActionEvent,
): CreditsActionState => {
  switch (event.type) {
    case 'start_confirm':
      // Only a fresh idle (or a reset error/success) may start confirming —
      // never re-enter confirm while already submitting.
      if (state.status === 'submitting') return state
      return { actionRef: event.actionRef, status: 'confirming' }

    case 'cancel':
      return state.status === 'confirming' ? { status: 'idle' } : state

    case 'submit':
      return state.status === 'confirming'
        ? { actionRef: state.actionRef, status: 'submitting' }
        : state

    case 'succeed':
      return state.status === 'submitting'
        ? { actionRef: state.actionRef, messageSafe: event.messageSafe, status: 'success' }
        : state

    case 'fail':
      return state.status === 'submitting'
        ? { actionRef: state.actionRef, messageSafe: event.messageSafe, status: 'error' }
        : state

    case 'retry':
      // A retry reuses the SAME actionRef — it is the same attempt, not a
      // new idempotency key, so a resubmission after a network failure can
      // never double-grant/double-clawback.
      return state.status === 'error'
        ? { actionRef: state.actionRef, status: 'confirming' }
        : state

    case 'reset':
      return { status: 'idle' }
  }
}
