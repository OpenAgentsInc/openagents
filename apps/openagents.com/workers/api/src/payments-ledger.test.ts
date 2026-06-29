import { describe, expect, test } from 'vitest'

import {
  type LedgerStatement,
  type PayInPlan,
  PayInPlanError,
  createPayInStatements,
  decodeAgentBalanceRow,
  markPayInFailedStatements,
  markPayInPaidStatements,
  payInTransitionAllowed,
  retryPayInStatements,
} from './payments-ledger'

const nowIso = '2026-06-10T20:00:00.000Z'

const tipPlan = (overrides?: Partial<PayInPlan>): PayInPlan => ({
  contextRef: 'post.test',
  costMsat: 50_000,
  genesisId: null,
  idempotencyKey: 'tip:test:1',
  legs: [
    {
      amountMsat: 50_000,
      direction: 'in',
      externalRef: null,
      kind: 'balance',
      legId: 'leg_in',
      partyRef: 'agent:sender',
    },
    {
      amountMsat: 50_000,
      direction: 'out',
      externalRef: null,
      kind: 'balance',
      legId: 'leg_out',
      partyRef: 'agent:recipient',
    },
  ],
  payInId: 'payin_1',
  payInType: 'tip',
  payerRef: 'agent:sender',
  publicReceiptRef: null,
  rung: 'credited',
  ...overrides,
})

// A tiny model of the ledger semantics the statements rely on: D1 batch
// = one transaction, sequential statements see prior effects, and the
// balance CHECK constraint aborts the whole batch when violated.
class LedgerModel {
  balances = new Map<string, number>()
  payIns = new Map<
    string,
    { state: string; successorId: string | null; failureReason: string | null }
  >()
  legs: Array<{
    id: string
    payInId: string
    direction: string
    resultingBalance: number | null
    refundOfLegId: string | null
    amountMsat: number
    partyRef: string
  }> = []

  apply(statements: ReadonlyArray<LedgerStatement>): void {
    const snapshotBalances = new Map(this.balances)
    const snapshotPayIns = new Map(
      Array.from(this.payIns, ([k, v]) => [k, { ...v }] as const),
    )
    const snapshotLegs = this.legs.map(leg => ({ ...leg }))

    try {
      for (const statement of statements) {
        this.applyOne(statement)
      }
    } catch (error) {
      this.balances = snapshotBalances
      this.payIns = new Map(snapshotPayIns)
      this.legs = snapshotLegs
      throw error
    }
  }

  private applyOne(statement: LedgerStatement): void {
    const sql = statement.sql.replace(/\s+/g, ' ').trim()
    const params = statement.params

    if (sql.startsWith('INSERT INTO agent_balances')) {
      const actorRef = String(params[0])
      if (!this.balances.has(actorRef)) {
        this.balances.set(actorRef, 0)
      }
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('balance_msat - ?')
    ) {
      const amount = Number(params[0])
      const actorRef = String(params[2])
      if (sql.includes('AND EXISTS (SELECT 1 FROM pay_ins WHERE id = ?)')) {
        const payInId = String(params[3])
        if (!this.payIns.has(payInId)) {
          return
        }
      }
      const next = (this.balances.get(actorRef) ?? 0) - amount
      if (next < 0) {
        throw new Error('CHECK constraint failed: balance_msat >= 0')
      }
      this.balances.set(actorRef, next)
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('balance_msat + ?')
    ) {
      const amount = Number(params[0])
      const actorRef = String(params[2])
      this.balances.set(actorRef, (this.balances.get(actorRef) ?? 0) + amount)
      return
    }

    if (sql.startsWith('INSERT INTO pay_ins') && sql.includes('SELECT ?')) {
      const id = String(params[0])
      const lockedId = String(params[11])
      const expectedSuccessor = String(params[12])
      const locked = this.payIns.get(lockedId)
      if (locked === undefined || locked.successorId !== expectedSuccessor) {
        return
      }
      this.payIns.set(id, {
        failureReason: null,
        state: 'pending',
        successorId: null,
      })
      return
    }

    if (sql.startsWith('INSERT INTO pay_ins')) {
      const id = String(params[0])
      this.payIns.set(id, {
        failureReason: null,
        state: 'pending',
        successorId: null,
      })
      return
    }

    if (sql.startsWith('INSERT INTO pay_in_legs')) {
      if (sql.includes('WHERE EXISTS (SELECT 1 FROM pay_ins WHERE id = ?)')) {
        const payInId = String(params[params.length - 1])
        if (!this.payIns.has(payInId)) {
          return
        }
      }
      const resultingFromSubquery = sql.includes(
        'SELECT balance_msat FROM agent_balances',
      )
      const partyRef = String(params[4])
      this.legs.push({
        amountMsat: Number(params[5]),
        direction: String(params[2]),
        id: String(params[0]),
        partyRef,
        payInId: String(params[1]),
        refundOfLegId:
          sql.includes('refund_of_leg_id') && !sql.includes('NULL, ?)')
            ? ((): string | null => {
                const candidate = params[params.length - 2]
                return candidate === null ? null : String(candidate)
              })()
            : null,
        resultingBalance: resultingFromSubquery
          ? (this.balances.get(partyRef) ?? null)
          : null,
      })
      return
    }

    if (
      sql.startsWith('UPDATE pay_ins') &&
      sql.includes("SET state = 'paid'")
    ) {
      const id = String(params[2])
      const payIn = this.payIns.get(id)
      if (
        payIn !== undefined &&
        (payIn.state === 'pending' || payIn.state === 'forwarding')
      ) {
        payIn.state = 'paid'
      }
      return
    }

    if (
      sql.startsWith('UPDATE pay_ins') &&
      sql.includes("SET state = 'failed'")
    ) {
      const id = String(params[2])
      const payIn = this.payIns.get(id)
      if (
        payIn !== undefined &&
        (payIn.state === 'pending' || payIn.state === 'forwarding')
      ) {
        payIn.state = 'failed'
        payIn.failureReason = String(params[0])
      }
      return
    }

    if (
      sql.startsWith('UPDATE pay_ins') &&
      sql.includes("SET state = 'forwarding'")
    ) {
      const id = String(params[1])
      const payIn = this.payIns.get(id)
      if (payIn !== undefined && payIn.state === 'pending') {
        payIn.state = 'forwarding'
      }
      return
    }

    if (sql.startsWith('UPDATE pay_ins') && sql.includes('successor_id = ?')) {
      const successorId = String(params[0])
      const id = String(params[1])
      const payIn = this.payIns.get(id)
      if (
        payIn !== undefined &&
        payIn.successorId === null &&
        payIn.state === 'failed'
      ) {
        payIn.successorId = successorId
      }
      return
    }

    if (sql.startsWith('UPDATE pay_in_legs')) {
      const partyRef = String(params[0])
      const legId = String(params[1])
      const leg = this.legs.find(candidate => candidate.id === legId)
      if (leg !== undefined) {
        leg.resultingBalance = this.balances.get(partyRef) ?? null
      }
      return
    }

    throw new Error(`model does not understand statement: ${sql}`)
  }
}

describe('payments ledger', () => {
  test('transition table is closed over terminal states', () => {
    expect(payInTransitionAllowed('pending', 'paid')).toBe(true)
    expect(payInTransitionAllowed('pending', 'forwarding')).toBe(true)
    expect(payInTransitionAllowed('forwarding', 'failed')).toBe(true)
    expect(payInTransitionAllowed('paid', 'failed')).toBe(false)
    expect(payInTransitionAllowed('failed', 'paid')).toBe(false)
  })

  test('agent balance decoder exposes held and available balance separately', () => {
    expect(
      decodeAgentBalanceRow({
        actor_ref: 'agent:alice',
        balance_msat: 100_000,
        held_msat: 40_000,
        receive_credits_below_sat: 10,
        send_credits_below_sat: 10,
        sweep_enabled: 1,
        sweep_threshold_sat: 210,
      }),
    ).toMatchObject({
      actorRef: 'agent:alice',
      availableMsat: 60_000,
      balanceMsat: 100_000,
      heldMsat: 40_000,
    })
  })

  test('plan invariants reject uncovered cost and bad amounts', () => {
    expect(() =>
      createPayInStatements(
        tipPlan({
          legs: [
            {
              amountMsat: 10_000,
              direction: 'in',
              externalRef: null,
              kind: 'balance',
              legId: 'leg_in',
              partyRef: 'agent:sender',
            },
          ],
        }),
        nowIso,
      ),
    ).toThrow(PayInPlanError)

    expect(() =>
      createPayInStatements(tipPlan({ costMsat: -5 }), nowIso),
    ).toThrow(PayInPlanError)
  })

  test('create debits the funding balance and stamps resulting balance', () => {
    const model = new LedgerModel()
    model.balances.set('agent:sender', 80_000)

    model.apply(createPayInStatements(tipPlan(), nowIso))

    expect(model.balances.get('agent:sender')).toBe(30_000)
    const fundingLeg = model.legs.find(leg => leg.id === 'leg_in')
    expect(fundingLeg?.resultingBalance).toBe(30_000)
    // Payout credit is deferred to paid-time.
    expect(model.balances.get('agent:recipient') ?? 0).toBe(0)
  })

  test('insufficient balance aborts the whole batch atomically', () => {
    const model = new LedgerModel()
    model.balances.set('agent:sender', 10_000)

    expect(() => model.apply(createPayInStatements(tipPlan(), nowIso))).toThrow(
      /CHECK constraint/,
    )
    expect(model.balances.get('agent:sender')).toBe(10_000)
    expect(model.payIns.size).toBe(0)
    expect(model.legs.length).toBe(0)
  })

  test('paid credits payout legs with resulting balances', () => {
    const model = new LedgerModel()
    model.balances.set('agent:sender', 80_000)
    model.apply(createPayInStatements(tipPlan(), nowIso))

    model.apply(
      markPayInPaidStatements(
        {
          balancePayoutLegs: [
            {
              amountMsat: 50_000,
              legId: 'leg_out',
              partyRef: 'agent:recipient',
            },
          ],
          payInId: 'payin_1',
        },
        nowIso,
      ),
    )

    expect(model.payIns.get('payin_1')?.state).toBe('paid')
    expect(model.balances.get('agent:recipient')).toBe(50_000)
    const payoutLeg = model.legs.find(leg => leg.id === 'leg_out')
    expect(payoutLeg?.resultingBalance).toBe(50_000)
  })

  test('failed refunds funding debits and links the refund leg', () => {
    const model = new LedgerModel()
    model.balances.set('agent:sender', 80_000)
    model.apply(createPayInStatements(tipPlan(), nowIso))
    expect(model.balances.get('agent:sender')).toBe(30_000)

    model.apply(
      markPayInFailedStatements(
        {
          balanceFundingLegs: [
            {
              amountMsat: 50_000,
              legId: 'leg_in',
              partyRef: 'agent:sender',
              refundLegId: 'leg_refund',
            },
          ],
          failureReason: 'lightning_leg_failed',
          payInId: 'payin_1',
        },
        nowIso,
      ),
    )

    expect(model.payIns.get('payin_1')?.state).toBe('failed')
    expect(model.payIns.get('payin_1')?.failureReason).toBe(
      'lightning_leg_failed',
    )
    expect(model.balances.get('agent:sender')).toBe(80_000)
    const refundLeg = model.legs.find(leg => leg.id === 'leg_refund')
    expect(refundLeg?.refundOfLegId).toBe('leg_in')
    expect(refundLeg?.resultingBalance).toBe(80_000)
  })

  test('retry wins the successor lock exactly once', () => {
    const model = new LedgerModel()
    model.balances.set('agent:sender', 200_000)
    model.apply(createPayInStatements(tipPlan(), nowIso))
    model.apply(
      markPayInFailedStatements(
        {
          balanceFundingLegs: [
            {
              amountMsat: 50_000,
              legId: 'leg_in',
              partyRef: 'agent:sender',
              refundLegId: 'leg_refund',
            },
          ],
          failureReason: 'lightning_leg_failed',
          payInId: 'payin_1',
        },
        nowIso,
      ),
    )

    const retryA = retryPayInStatements(
      {
        newPlan: tipPlan({
          genesisId: 'payin_1',
          idempotencyKey: 'tip:test:retry_a',
          legs: [
            {
              amountMsat: 50_000,
              direction: 'in',
              externalRef: null,
              kind: 'balance',
              legId: 'leg_in_a',
              partyRef: 'agent:sender',
            },
            {
              amountMsat: 50_000,
              direction: 'out',
              externalRef: null,
              kind: 'balance',
              legId: 'leg_out_a',
              partyRef: 'agent:recipient',
            },
          ],
          payInId: 'payin_retry_a',
        }),
        previousPayInId: 'payin_1',
      },
      nowIso,
    )
    const retryB = retryPayInStatements(
      {
        newPlan: tipPlan({
          genesisId: 'payin_1',
          idempotencyKey: 'tip:test:retry_b',
          legs: [
            {
              amountMsat: 50_000,
              direction: 'in',
              externalRef: null,
              kind: 'balance',
              legId: 'leg_in_b',
              partyRef: 'agent:sender',
            },
            {
              amountMsat: 50_000,
              direction: 'out',
              externalRef: null,
              kind: 'balance',
              legId: 'leg_out_b',
              partyRef: 'agent:recipient',
            },
          ],
          payInId: 'payin_retry_b',
        }),
        previousPayInId: 'payin_1',
      },
      nowIso,
    )

    model.apply(retryA)
    model.apply(retryB)

    expect(model.payIns.has('payin_retry_a')).toBe(true)
    expect(model.payIns.has('payin_retry_b')).toBe(false)
    expect(model.payIns.get('payin_1')?.successorId).toBe('payin_retry_a')
    // Only the winning retry debited the sender.
    expect(model.balances.get('agent:sender')).toBe(150_000)
    expect(model.legs.some(leg => leg.id === 'leg_in_b')).toBe(false)
  })
})
