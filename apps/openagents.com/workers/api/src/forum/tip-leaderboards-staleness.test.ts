// Tip leaderboards staleness declaration plus ladder credited/swept
// totals (epic #4751, the #4753 remainder): the public leaderboard
// payload declares generatedAt + its staleness contract, and ranked
// creators list their ladder-credited and swept sats alongside the
// settled receipt-backed totals that own the ranking.
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'
import {
  readForumTipLeaderboards,
  readForumTipReconciliation,
} from './tip-earnings'

const orreryActorJson = JSON.stringify({
  actorId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  actorRef: 'agent:orrery',
  displayName: 'Orrery',
  groupRefs: ['agents'],
  isAgent: true,
  slug: 'orrery',
})

const creatorRow = {
  actor_json: orreryActorJson,
  earning_actor_ref: 'agent:orrery',
  tip_count: 3,
  total_paid_sats: 120,
  total_settled_sats: 100,
}

const postRow = {
  actor_json: orreryActorJson,
  post_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  post_subject: 'Projection staleness epic synthesis',
  tip_count: 3,
  topic_id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
  total_paid_sats: 120,
  total_settled_sats: 100,
}

// 50 credited sats, 20 of them covered by settled sweeps
// (oldest-credited-first), leaving 30 still credited.
const ladderTotalsRow = {
  credited_msat: 50_000,
  recipient_actor_ref: 'agent:orrery',
  swept_msat: 20_000,
}

class LeaderboardStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(private readonly query: string) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('COUNT(*) AS count')) {
      return Promise.resolve({ count: 0 } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('GROUP BY payout.party_ref')) {
      return Promise.resolve({
        results: this.values.includes('agent:orrery') ? [ladderTotalsRow] : [],
      } as unknown as D1Result<T>)
    }

    // CFG-4 (#8519): the ladder count reads arrive through the ledger
    // handle's query(...) (row arrays) instead of first().
    if (
      this.query.includes('COUNT(*) AS count') &&
      this.query.includes('FROM pay_ins p')
    ) {
      return Promise.resolve({
        results: [{ count: 0 }],
      } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM pay_ins p')) {
      return Promise.resolve({ results: [] } as unknown as D1Result<T>)
    }

    if (this.query.includes('ma.target_post_id AS post_id')) {
      return Promise.resolve({
        results: [postRow],
      } as unknown as D1Result<T>)
    }

    if (this.query.includes('GROUP BY ma.earning_actor_ref')) {
      return Promise.resolve({
        results: [creatorRow],
      } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_money_actions')) {
      return Promise.resolve({ results: [] } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const db: D1Database = {
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new LeaderboardStatement(query),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
}

// CFG-4 (#8519): the ladder reads (pay_ins/pay_in_legs) arrive through the
// PaymentsLedgerDb seam; back it with the same fake statement router.
const ledgerDb = paymentsLedgerDbFromD1(db as never)

const nowIso = '2026-06-11T02:00:00.000Z'

describe('tip leaderboards staleness declaration (#4751)', () => {
  test('declares generatedAt, the staleness contract, and honesty caveats', async () => {
    const leaderboards = await Effect.runPromise(
      readForumTipLeaderboards(
        db,
        ledgerDb,
        { limit: 10 },
        { nowIso: () => nowIso },
      ),
    )

    expect(leaderboards.generatedAt).toBe(nowIso)
    expect(leaderboards.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
      rebuildsOn: expect.arrayContaining([
        'forum_payment_event_confirmed',
        'tip_ladder_pay_in_paid',
        'tip_sweep_settled',
      ]),
    })
    expect(leaderboards.caveatRefs).toStrictEqual([
      'caveat.public.forum_tip.leaderboards_rank_by_settled_receipt_tips_only',
      'caveat.public.forum_tip.ladder_credited_swept_sats_listed_for_ranked_creators_only',
    ])
  })

  test('lists ladder credited and swept sats for ranked creators', async () => {
    const leaderboards = await Effect.runPromise(
      readForumTipLeaderboards(
        db,
        ledgerDb,
        { limit: 10 },
        { nowIso: () => nowIso },
      ),
    )

    expect(leaderboards.creators).toHaveLength(1)
    expect(leaderboards.creators[0]).toMatchObject({
      actor: { actorRef: 'agent:orrery', slug: 'orrery' },
      tipCount: 3,
      totalCreditedSats: 30,
      totalPaidSats: 120,
      totalSettledSats: 100,
      totalSweptSats: 20,
    })
    expect(leaderboards.posts[0]).toMatchObject({
      postId: postRow.post_id,
      totalSettledSats: 100,
    })
  })

  test('tip reconciliation declares the same staleness contract', async () => {
    const reconciliation = await Effect.runPromise(
      readForumTipReconciliation(
        db,
        ledgerDb,
        { actorRef: null, limit: 10 },
        { nowIso: () => nowIso },
      ),
    )

    expect(reconciliation.generatedAt).toBe(nowIso)
    expect(reconciliation.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })
})
