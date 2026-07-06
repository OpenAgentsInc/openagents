import { describe, expect, test } from 'vitest'
import {
  buildLaborEarningsProjection,
  readLaborEarnings,
} from './labor-earnings'
import { makeLedgerSqliteDb } from './test/payments-ledger-sqlite'

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

  // CFG-4 (#8519): `labor_escrows` reads run on the Postgres-authoritative
  // credits ledger handle, never on D1.
  test('readLaborEarnings reads released escrows from the credits ledger', async () => {
    const ledgerDb = makeLedgerSqliteDb()
    const seed = (
      id: string,
      state: string,
      releasedAt: string | null,
      amountMsat: number,
    ) =>
      ledgerDb.batch([
        {
          params: [
            id,
            `idem-${id}`,
            `wr-${id}`,
            'agent:requester',
            'agent:provider',
            amountMsat,
            state,
            `job-${id}`,
            `receipt.labor_escrow.reserve.${id}`,
            state === 'released_to_provider'
              ? `receipt.labor_escrow.release.${id}`
              : null,
            '2026-06-20T11:00:00.000Z',
            '2026-06-20T11:00:00.000Z',
            releasedAt,
          ],
          sql: `INSERT INTO labor_escrows (
                  id, idempotency_key, work_request_id, requester_actor_ref,
                  provider_actor_ref, amount_msat, state, job_event_id,
                  reserve_receipt_ref, release_receipt_ref, created_at,
                  updated_at, released_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        },
      ])

    await seed('esc-1', 'released_to_provider', '2026-06-20T12:00:00.000Z', 2000)
    await seed('esc-2', 'released_to_provider', '2026-06-20T13:00:00.000Z', 3000)
    await seed('esc-3', 'reserved', null, 5000)

    const projection = await readLaborEarnings(
      ledgerDb,
      'agent:provider',
      '2026-06-20T14:00:00.000Z',
    )

    expect(projection.summary.releasedEscrowCount).toBe(2)
    expect(projection.summary.totalReleasedMsat).toBe(5000)
    // Newest release first.
    expect(projection.rows[0]?.escrowRef).toBe('labor_escrow.public.esc-2')
    expect(projection.rows[0]?.receiptRef).toBe(
      'receipt.labor_escrow.release.esc-2',
    )
    expect(projection.rows[1]?.workRequestRef).toBe('work_request.public.wr-esc-1')
  })
})
