import { describe, expect, test } from 'vitest'

import {
  meteredExecutionKeyInput,
  newMeteredExecutionAttempt,
} from './metered-execution-attempt'

const attempt = (attemptId: string) => newMeteredExecutionAttempt(() => attemptId)

const ATTEMPT_A = '11111111-2222-4333-8444-555555555555'
const ATTEMPT_B = '99999999-8888-4777-8666-555555555555'

const keyFor = (attemptId: string, overrides: Partial<{
  actorUserId: string
  model: string
  scope: string
}> = {}) =>
  meteredExecutionKeyInput({
    actorUserId: overrides.actorUserId ?? 'agent:test',
    attempt: attempt(attemptId),
    model: overrides.model ?? 'gemini-3.5-flash',
    scope: overrides.scope ?? 'omega:google_gemini',
  })

describe('metered execution attempt identity', () => {
  // THE DEFECT. The key used to hash the request BODY, so N byte-identical
  // requests collapsed into one `INSERT OR IGNORE` row while all N drew real
  // owner-funded provider tokens. Measured live: 7 requests, 1 row.
  test('distinct executions get distinct keys even when actor, model and content match exactly', () => {
    expect(keyFor(ATTEMPT_A)).not.toBe(keyFor(ATTEMPT_B))
  })

  test('a fresh attempt is distinct on every mint (identity is randomness, not content)', () => {
    const keys = new Set(
      Array.from({ length: 64 }, () =>
        meteredExecutionKeyInput({
          actorUserId: 'agent:test',
          attempt: newMeteredExecutionAttempt(),
          model: 'gemini-3.5-flash',
          scope: 'omega:google_gemini',
        }),
      ),
    )

    expect(keys.size).toBe(64)
  })

  // THE PROPERTY WORTH KEEPING. A retried METERING write for ONE execution must
  // land one row. The attempt id is minted once before the provider call and
  // threaded through, so re-running the write reproduces the same key and
  // `INSERT OR IGNORE` collapses it.
  test('re-metering the SAME execution reproduces the same key (exactly-once per execution)', () => {
    expect(keyFor(ATTEMPT_A)).toBe(keyFor(ATTEMPT_A))
  })

  test('keys never collide across actor, model, or scope', () => {
    const base = keyFor(ATTEMPT_A)

    expect(keyFor(ATTEMPT_A, { actorUserId: 'agent:other' })).not.toBe(base)
    expect(keyFor(ATTEMPT_A, { model: 'gemini-2.5-pro' })).not.toBe(base)
    expect(keyFor(ATTEMPT_A, { scope: 'omega:other_provider' })).not.toBe(base)
  })

  // The absence of request content is the fix, so it is asserted rather than
  // left to review.
  test('the key pre-image carries no request content', () => {
    const key = keyFor(ATTEMPT_A)

    expect(key).toContain(ATTEMPT_A)
    expect(key).not.toContain('body:')
    expect(key).toBe(
      `omega:google_gemini:agent:test:gemini-3.5-flash:attempt:${ATTEMPT_A}`,
    )
  })

  test('a malformed attempt id is rejected rather than becoming a ledger ref', () => {
    expect(() => newMeteredExecutionAttempt(() => 'not-a-uuid')).toThrow(
      'metered_execution_attempt_id_malformed',
    )
    expect(() => newMeteredExecutionAttempt(() => '')).toThrow(
      'metered_execution_attempt_id_malformed',
    )
  })
})
