import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { makeLedgerMeteringHook } from './metering-hook'

describe('VP1 inference usage measurement', () => {
  it('returns a no-spend result and never consults the legacy ledger', async () => {
    const ledgerDb = new Proxy({}, { get: () => { throw new Error('consulted') } })
    const outcome = await Effect.runPromise(
      makeLedgerMeteringHook({ ledgerDb })({
        accountRef: 'agent:test',
        adapterId: 'adapter:test',
        fundingKind: 'card',
        requestId: 'request:test',
        requestedModel: 'model:test',
        servedModel: 'model:test',
        streamed: false,
        usage: { completionTokens: 1, promptTokens: 2, totalTokens: 3 },
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
})
