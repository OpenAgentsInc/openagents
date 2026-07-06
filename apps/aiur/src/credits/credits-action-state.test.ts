import { describe, expect, test } from 'vitest'

import {
  creditsActionReducer,
  initialCreditsActionState,
} from './credits-action-state'

describe('creditsActionReducer (grant/clawback UI state machine)', () => {
  test('starts idle', () => {
    expect(initialCreditsActionState).toEqual({ status: 'idle' })
  })

  test('idle -> confirming -> submitting -> success', () => {
    let state = initialCreditsActionState
    state = creditsActionReducer(state, { actionRef: 'ref-1', type: 'start_confirm' })
    expect(state).toEqual({ actionRef: 'ref-1', status: 'confirming' })

    state = creditsActionReducer(state, { type: 'submit' })
    expect(state).toEqual({ actionRef: 'ref-1', status: 'submitting' })

    state = creditsActionReducer(state, { messageSafe: 'Granted.', type: 'succeed' })
    expect(state).toEqual({ actionRef: 'ref-1', messageSafe: 'Granted.', status: 'success' })
  })

  test('confirming -> cancel returns to idle', () => {
    let state = creditsActionReducer(initialCreditsActionState, {
      actionRef: 'ref-1',
      type: 'start_confirm',
    })
    state = creditsActionReducer(state, { type: 'cancel' })
    expect(state).toEqual({ status: 'idle' })
  })

  test('cancel is a no-op from idle (not confirming)', () => {
    const state = creditsActionReducer(initialCreditsActionState, { type: 'cancel' })
    expect(state).toEqual({ status: 'idle' })
  })

  test('submitting -> fail -> retry reuses the SAME actionRef (never mints a new idempotency key)', () => {
    let state = creditsActionReducer(initialCreditsActionState, {
      actionRef: 'ref-1',
      type: 'start_confirm',
    })
    state = creditsActionReducer(state, { type: 'submit' })
    state = creditsActionReducer(state, { messageSafe: 'Network error', type: 'fail' })
    expect(state).toEqual({ actionRef: 'ref-1', messageSafe: 'Network error', status: 'error' })

    state = creditsActionReducer(state, { type: 'retry' })
    expect(state).toEqual({ actionRef: 'ref-1', status: 'confirming' })
  })

  test('start_confirm is refused while already submitting (cannot re-enter confirm mid-flight)', () => {
    let state = creditsActionReducer(initialCreditsActionState, {
      actionRef: 'ref-1',
      type: 'start_confirm',
    })
    state = creditsActionReducer(state, { type: 'submit' })
    const beforeSubmitting = state
    state = creditsActionReducer(state, { actionRef: 'ref-2', type: 'start_confirm' })
    expect(state).toBe(beforeSubmitting)
  })

  test('submit is a no-op unless currently confirming', () => {
    const idleThenSubmit = creditsActionReducer(initialCreditsActionState, { type: 'submit' })
    expect(idleThenSubmit).toEqual({ status: 'idle' })
  })

  test('succeed/fail are no-ops unless currently submitting', () => {
    const idle = initialCreditsActionState
    expect(creditsActionReducer(idle, { messageSafe: 'x', type: 'succeed' })).toBe(idle)
    expect(creditsActionReducer(idle, { messageSafe: 'x', type: 'fail' })).toBe(idle)
  })

  test('retry is a no-op unless currently in error', () => {
    const idle = initialCreditsActionState
    expect(creditsActionReducer(idle, { type: 'retry' })).toBe(idle)
  })

  test('reset always returns to idle from any state', () => {
    let state = creditsActionReducer(initialCreditsActionState, {
      actionRef: 'ref-1',
      type: 'start_confirm',
    })
    state = creditsActionReducer(state, { type: 'submit' })
    state = creditsActionReducer(state, { messageSafe: 'Granted.', type: 'succeed' })
    state = creditsActionReducer(state, { type: 'reset' })
    expect(state).toEqual({ status: 'idle' })
  })
})
