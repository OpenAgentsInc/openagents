import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  disabledKhalaLoopArming,
  makeKhalaLoopSettlementDispatch,
  readKhalaLoopArming,
} from './khala-loop-integration'

describe('VP1 Khala loop settlement retirement', () => {
  it('cannot be armed and never invokes a legacy dispatch', async () => {
    expect(
      readKhalaLoopArming({ OPENAGENTS_KHALA_LOOP_ARMED: 'armed' }),
    ).toEqual(disabledKhalaLoopArming)

    let invoked = false
    const dispatch = makeKhalaLoopSettlementDispatch({
      realDispatch: () => {
        invoked = true
        return Effect.void
      },
    })
    await Effect.runPromise(dispatch({ contributorRef: 'node:test', settlement: {} }))
    expect(invoked).toBe(false)
  })
})
