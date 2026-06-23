import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { claimLightningPaymentHash } from './mpp-lightning-replay'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }
  async run(): Promise<{ meta: { changes: number } }> {
    const info = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(info.changes ?? 0) } }
  }
}
class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(`
    CREATE TABLE mpp_lightning_replay (
      payment_hash TEXT PRIMARY KEY,
      challenge_id TEXT NOT NULL,
      consumed_at TEXT NOT NULL
    );
  `)
  return new SqliteD1(raw) as unknown as D1Database
}

const HASH =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'

describe('claimLightningPaymentHash (consume-once)', () => {
  test('first claim succeeds (true); second claim is a replay (false)', async () => {
    const db = makeDb()
    const first = await run(
      claimLightningPaymentHash(db, { challengeId: 'c1', paymentHash: HASH }),
    )
    expect(first).toBe(true)
    const second = await run(
      claimLightningPaymentHash(db, { challengeId: 'c1', paymentHash: HASH }),
    )
    expect(second).toBe(false)
  })

  test('a replay under a DIFFERENT challenge id is still refused (paymentHash keyed)', async () => {
    const db = makeDb()
    expect(
      await run(
        claimLightningPaymentHash(db, { challengeId: 'c1', paymentHash: HASH }),
      ),
    ).toBe(true)
    expect(
      await run(
        claimLightningPaymentHash(db, { challengeId: 'c2', paymentHash: HASH }),
      ),
    ).toBe(false)
  })

  test('distinct paymentHashes each claim independently', async () => {
    const db = makeDb()
    const other =
      'ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100'
    expect(
      await run(
        claimLightningPaymentHash(db, { challengeId: 'c1', paymentHash: HASH }),
      ),
    ).toBe(true)
    expect(
      await run(
        claimLightningPaymentHash(db, { challengeId: 'c1', paymentHash: other }),
      ),
    ).toBe(true)
  })
})
