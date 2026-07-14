import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  disabledKhalaLoopArming,
  makeKhalaLoopSettlementDispatch,
  readKhalaLoopArming,
} from './khala-loop-integration'
import { makeLedgerMeteringHook } from './metering-hook'

describe('VP1 inference no-spend contract', () => {
  it('measures provider usage without consulting or mutating a ledger', async () => {
    const ledgerDb = new Proxy(
      {},
      {
        get: () => {
          throw new Error('ledger must not be consulted')
        },
      },
    )
    const outcome = await Effect.runPromise(
      makeLedgerMeteringHook({ ledgerDb })({
        accountRef: 'agent:test',
        adapterId: 'adapter.test',
        fundingKind: 'card',
        requestId: 'request.test',
        requestedModel: 'model.test',
        servedModel: 'model.test',
        streamed: false,
        usage: { completionTokens: 3, promptTokens: 2, totalTokens: 5 },
      }),
    )

    expect(outcome).toEqual({
      metered: false,
      paymentMode: 'no-spend',
      payoutClaimAllowed: false,
      receiptRef: null,
      settlementState: 'not_applicable',
    })
  })

  it('cannot arm or invoke a legacy settlement dispatch', async () => {
    expect(
      readKhalaLoopArming({ OPENAGENTS_KHALA_LOOP_ARMED: 'armed' }),
    ).toEqual(disabledKhalaLoopArming)

    let invoked = false
    const dispatch = makeKhalaLoopSettlementDispatch({
      arming: disabledKhalaLoopArming,
      realDispatch: () => {
        invoked = true
        return Effect.void
      },
    })
    await Effect.runPromise(
      dispatch({ contributorRef: 'node.test', settlement: {} }),
    )
    expect(invoked).toBe(false)
  })
})
