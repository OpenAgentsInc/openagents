import { describe, expect, test } from 'vitest'
import {
  buildLaborEarningsProjection,
} from './labor-earnings'

describe('LaborEarnings', () => {
  test('builds public projection', () => {
    const generatedAt = '2026-06-20T12:05:00.000Z'
    const projection = buildLaborEarningsProjection('agent:provider', [
      {
        amountMsat: 2000,
        escrowRef: 'labor_escrow.public.1',
        jobEventRef: 'nostr.event.aaa',
        receiptRef: 'receipt.labor_escrow.release.1',
        requesterActorRef: 'agent:requester',
        workRequestRef: 'work_request.public.1',
        releasedAtIso: '2026-06-20T12:00:00.000Z',
      }
    ], generatedAt)

    expect(projection.publicSafe).toBe(true)
    expect(projection.providerActorRef).toBe('agent:provider')
    expect(projection.generatedAt).toBe(generatedAt)
    expect(projection.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(projection.summary.totalReleasedMsat).toBe(2000)
    expect(projection.summary.releasedEscrowCount).toBe(1)
  })
})
