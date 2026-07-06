// CFG-4 (#8519): payments-ledger Postgres contract suite.
//
// The credits domain (`pay_ins` / `pay_in_legs` / `agent_balances`) is
// Postgres-authoritative — `runLedgerStatements` executes every ledger batch
// as ONE Postgres transaction. This suite proves the load-bearing semantics
// on the PRODUCTION dialect against a throwaway local Postgres (initdb /
// pg_ctl; skipped when no local Postgres binaries exist):
//
//   - create → debit → resulting-balance capture, atomically
//   - insufficient funds: the `CHECK (balance_msat >= 0)` violation aborts
//     the WHOLE transaction (no pay_ins row, no partial legs), classified
//     by `isLedgerCheckConstraintError`
//   - idempotency replay: `idempotency_key UNIQUE` aborts the whole
//     transaction with zero double-debit, classified by
//     `isLedgerUniqueConstraintError`
//   - markPaid credits payout legs and stamps resulting balances
//   - markFailed refunds funding debits with refund legs
//   - retry chaining: the set-if-null successor lock means a lost race
//     inserts ZERO rows
//   - `?` → `$n` placeholder translation, including `?` inside string
//     literals

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  createPayInStatements,
  markPayInFailedStatements,
  markPayInPaidStatements,
  readAgentBalance,
  retryPayInStatements,
  runLedgerStatements,
  sumAgentBalancesMsat,
  type PayInPlan,
} from './payments-ledger'
import {
  isLedgerCheckConstraintError,
  isLedgerUniqueConstraintError,
  makePostgresPaymentsLedgerDb,
  translateLedgerPlaceholders,
  type PaymentsLedgerDb,
} from './payments-ledger-db'

const MIGRATIONS_DIR = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations',
)

const NOW = '2026-07-06T00:00:00.000Z'

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}_cfg4_${++refCounter}`

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

describe('translateLedgerPlaceholders', () => {
  test('rewrites ? to $n in order', () => {
    expect(
      translateLedgerPlaceholders('SELECT * FROM t WHERE a = ? AND b = ?'),
    ).toBe('SELECT * FROM t WHERE a = $1 AND b = $2')
  })

  test('ignores ? inside string literals and quoted identifiers', () => {
    expect(
      translateLedgerPlaceholders(
        `SELECT '?' AS q, "col?umn" FROM t WHERE a = ? AND note = 'it''s ?' AND b = ?`,
      ),
    ).toBe(
      `SELECT '?' AS q, "col?umn" FROM t WHERE a = $1 AND note = 'it''s ?' AND b = $2`,
    )
  })
})

describe.skipIf(!hasLocalPostgres())(
  'payments ledger — Postgres executor contract',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let ledger: PaymentsLedgerDb

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE payments_ledger_contract')
      await admin.end({ timeout: 5 })

      const url = pg.urlFor('payments_ledger_contract')
      const raw = postgres(url, { max: 4, prepare: false })
      client = raw as unknown as PgClient
      for (const file of [
        '0015_billing_pay_ins.sql',
        '0016_treasury_domain.sql',
        '0039_credits_hard_cut_indexes.sql',
      ]) {
        await raw.unsafe(readFileSync(path.join(MIGRATIONS_DIR, file), 'utf8'))
      }
      ledger = makePostgresPaymentsLedgerDb({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      })
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const seedBalance = async (actorRef: string, balanceMsat: number) => {
      await ledger.batch([
        {
          params: [actorRef, balanceMsat, NOW, NOW],
          sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
                VALUES (?, ?, ?, ?)`,
        },
      ])
    }

    const balancePlan = (
      payerRef: string,
      recipientRef: string,
      amountMsat: number,
    ): PayInPlan => {
      const payInId = nextRef('pay_in')
      return {
        contextRef: 'contract:test',
        costMsat: amountMsat,
        genesisId: null,
        idempotencyKey: nextRef('idem'),
        legs: [
          {
            amountMsat,
            direction: 'in',
            externalRef: null,
            kind: 'balance',
            legId: nextRef('leg_in'),
            partyRef: payerRef,
          },
          {
            amountMsat,
            direction: 'out',
            externalRef: null,
            kind: 'balance',
            legId: nextRef('leg_out'),
            partyRef: recipientRef,
          },
        ],
        payerRef,
        payInId,
        payInType: 'tip',
        publicReceiptRef: null,
        rung: 'credited',
      }
    }

    test('create + markPaid: debit, resulting balance capture, payout credit', async () => {
      const payer = nextRef('payer')
      const recipient = nextRef('recipient')
      await seedBalance(payer, 10_000)

      const plan = balancePlan(payer, recipient, 4_000)
      await runLedgerStatements(ledger, createPayInStatements(plan, NOW))

      const payerBalance = await readAgentBalance(ledger, payer)
      expect(payerBalance?.balanceMsat).toBe(6_000)
      expect(payerBalance?.availableMsat).toBe(6_000)

      const fundingLegs = await ledger.query(
        `SELECT resulting_balance_msat FROM pay_in_legs
          WHERE pay_in_id = ? AND direction = 'in'`,
        [plan.payInId],
      )
      expect(Number(fundingLegs[0]?.resulting_balance_msat)).toBe(6_000)

      const outLeg = plan.legs.find(leg => leg.direction === 'out')
      if (outLeg === undefined) throw new Error('plan must have an out leg')
      await runLedgerStatements(
        ledger,
        markPayInPaidStatements(
          {
            balancePayoutLegs: [
              {
                amountMsat: outLeg.amountMsat,
                legId: outLeg.legId,
                partyRef: recipient,
              },
            ],
            payInId: plan.payInId,
          },
          NOW,
        ),
      )

      const recipientBalance = await readAgentBalance(ledger, recipient)
      expect(recipientBalance?.balanceMsat).toBe(4_000)

      const payIns = await ledger.query(
        `SELECT state FROM pay_ins WHERE id = ?`,
        [plan.payInId],
      )
      expect(payIns[0]?.state).toBe('paid')
    })

    test('insufficient funds aborts the WHOLE transaction (no partial rows)', async () => {
      const payer = nextRef('payer')
      await seedBalance(payer, 1_000)

      const plan = balancePlan(payer, nextRef('recipient'), 5_000)
      let caught: unknown
      try {
        await runLedgerStatements(ledger, createPayInStatements(plan, NOW))
      } catch (error) {
        caught = error
      }
      expect(caught).toBeDefined()
      expect(isLedgerCheckConstraintError(caught)).toBe(true)

      const payIns = await ledger.query(
        `SELECT id FROM pay_ins WHERE id = ?`,
        [plan.payInId],
      )
      expect(payIns).toHaveLength(0)
      const balance = await readAgentBalance(ledger, payer)
      expect(balance?.balanceMsat).toBe(1_000)
    })

    test('idempotency replay aborts atomically with zero double-debit', async () => {
      const payer = nextRef('payer')
      await seedBalance(payer, 10_000)

      const plan = balancePlan(payer, nextRef('recipient'), 2_000)
      await runLedgerStatements(ledger, createPayInStatements(plan, NOW))

      const replay: PayInPlan = { ...plan, payInId: nextRef('pay_in_replay') }
      let caught: unknown
      try {
        await runLedgerStatements(
          ledger,
          createPayInStatements(
            {
              ...replay,
              legs: replay.legs.map(leg => ({
                ...leg,
                legId: nextRef('leg_replay'),
              })),
            },
            NOW,
          ),
        )
      } catch (error) {
        caught = error
      }
      expect(caught).toBeDefined()
      expect(isLedgerUniqueConstraintError(caught)).toBe(true)

      const balance = await readAgentBalance(ledger, payer)
      expect(balance?.balanceMsat).toBe(8_000)
    })

    test('markFailed refunds funding debits with refund legs', async () => {
      const payer = nextRef('payer')
      await seedBalance(payer, 9_000)

      const plan = balancePlan(payer, nextRef('recipient'), 3_000)
      await runLedgerStatements(ledger, createPayInStatements(plan, NOW))
      expect((await readAgentBalance(ledger, payer))?.balanceMsat).toBe(6_000)

      const fundingLeg = plan.legs.find(leg => leg.direction === 'in')
      if (fundingLeg === undefined) throw new Error('plan must have an in leg')
      await runLedgerStatements(
        ledger,
        markPayInFailedStatements(
          {
            balanceFundingLegs: [
              {
                amountMsat: fundingLeg.amountMsat,
                legId: fundingLeg.legId,
                partyRef: payer,
                refundLegId: nextRef('leg_refund'),
              },
            ],
            failureReason: 'contract-test',
            payInId: plan.payInId,
          },
          NOW,
        ),
      )

      expect((await readAgentBalance(ledger, payer))?.balanceMsat).toBe(9_000)
      const refundLegs = await ledger.query(
        `SELECT refund_of_leg_id FROM pay_in_legs
          WHERE pay_in_id = ? AND external_ref = 'refund'`,
        [plan.payInId],
      )
      expect(refundLegs).toHaveLength(1)
      expect(refundLegs[0]?.refund_of_leg_id).toBe(fundingLeg.legId)
    })

    test('retry: lost successor-lock race inserts ZERO rows', async () => {
      const payer = nextRef('payer')
      await seedBalance(payer, 20_000)

      const plan = balancePlan(payer, nextRef('recipient'), 2_000)
      await runLedgerStatements(ledger, createPayInStatements(plan, NOW))
      const fundingLeg = plan.legs.find(leg => leg.direction === 'in')
      if (fundingLeg === undefined) throw new Error('plan must have an in leg')
      await runLedgerStatements(
        ledger,
        markPayInFailedStatements(
          {
            balanceFundingLegs: [
              {
                amountMsat: fundingLeg.amountMsat,
                legId: fundingLeg.legId,
                partyRef: payer,
                refundLegId: nextRef('leg_refund'),
              },
            ],
            failureReason: 'contract-test',
            payInId: plan.payInId,
          },
          NOW,
        ),
      )

      const retryPlan = (id: string): PayInPlan => ({
        ...balancePlan(payer, nextRef('recipient'), 2_000),
        genesisId: plan.payInId,
        payInId: id,
      })

      const winner = retryPlan(nextRef('retry_winner'))
      await runLedgerStatements(
        ledger,
        retryPayInStatements(
          { newPlan: winner, previousPayInId: plan.payInId },
          NOW,
        ),
      )

      const balanceAfterWinner = await readAgentBalance(ledger, payer)
      expect(balanceAfterWinner?.balanceMsat).toBe(18_000)

      const loser = retryPlan(nextRef('retry_loser'))
      await runLedgerStatements(
        ledger,
        retryPayInStatements(
          { newPlan: loser, previousPayInId: plan.payInId },
          NOW,
        ),
      )

      const loserRows = await ledger.query(
        `SELECT id FROM pay_ins WHERE id = ?`,
        [loser.payInId],
      )
      expect(loserRows).toHaveLength(0)
      // The lost race must not have debited anything either.
      expect((await readAgentBalance(ledger, payer))?.balanceMsat).toBe(18_000)
    })

    test('sumAgentBalancesMsat sums bigint columns exactly', async () => {
      const before = await sumAgentBalancesMsat(ledger)
      const actor = nextRef('sum_actor')
      await seedBalance(actor, 123_456)
      const after = await sumAgentBalancesMsat(ledger)
      expect(after - before).toBe(123_456)
    })
  },
)
