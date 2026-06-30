import { describe, expect, test } from 'vitest'

import {
  type LaborEscrowRecord,
  type LaborEscrowState,
  assertLaborEscrowPublicSafe,
  buildLaborEscrowPublicProjection,
  createCreditLedgerBondSettlementAdapter,
  evaluateArtanisLaborBudgetGate,
  evaluateLaborEscrowFundingSource,
  forfeitLaborEscrow,
  forfeitLaborEscrowStatements,
  refundLaborEscrowStatements,
  releaseLaborEscrow,
  releaseLaborEscrowStatements,
  reserveLaborEscrow,
  reserveLaborEscrowStatements,
} from './labor-escrow'
import type { LedgerStatement } from './payments-ledger'

const nowIso = '2026-06-10T23:00:00.000Z'
const jobEventId = 'a'.repeat(64)

const reserveInput = {
  amountMsat: 50_000,
  escrowId: 'escrow_1',
  idempotencyKey: 'labor:reserve:1',
  jobEventId,
  nowIso,
  requesterActorRef: 'agent:requester',
  reserveReceiptId: 'receipt_row_reserve_1',
  reserveReceiptRef: 'receipt.labor_escrow.reserve.escrow_1',
  workRequestId: 'work_request_1',
}

type ModeledEscrow = {
  amountMsat: number
  escrowId: string
  idempotencyKey: string
  jobEventId: string
  providerActorRef: string | null
  publicProjectionJson: string
  requesterActorRef: string
  reserveReceiptRef: string
  releaseReceiptRef: string | null
  refundReceiptRef: string | null
  forfeitReceiptRef: string | null
  forfeitDestination: 'counterparty' | 'burn' | null
  forfeitDestinationActorRef: string | null
  forfeitConditionRef: string | null
  state: LaborEscrowState
  workRequestId: string
}

type ModeledReceipt = {
  escrowId: string
  receiptId: string
  receiptRef: string
  transitionKind: 'reserve' | 'release' | 'refund' | 'forfeit'
}

class LaborEscrowLedgerModel {
  balances = new Map<string, { balanceMsat: number; heldMsat: number }>()
  escrows = new Map<string, ModeledEscrow>()
  receipts: ModeledReceipt[] = []

  apply(statements: ReadonlyArray<LedgerStatement>): void {
    const snapshotBalances = new Map(
      Array.from(this.balances, ([key, value]) => [key, { ...value }] as const),
    )
    const snapshotEscrows = new Map(
      Array.from(this.escrows, ([key, value]) => [key, { ...value }] as const),
    )
    const snapshotReceipts = this.receipts.map(receipt => ({ ...receipt }))

    try {
      for (const statement of statements) {
        this.applyOne(statement)
      }
    } catch (error) {
      this.balances = snapshotBalances
      this.escrows = snapshotEscrows
      this.receipts = snapshotReceipts
      throw error
    }
  }

  asRecord(escrowId: string): LaborEscrowRecord {
    const escrow = this.escrows.get(escrowId)
    if (escrow === undefined) {
      throw new Error(`missing escrow ${escrowId}`)
    }
    return {
      amountMsat: escrow.amountMsat,
      createdAt: nowIso,
      escrowId: escrow.escrowId,
      fundingSource: 'ledger_balance',
      idempotencyKey: escrow.idempotencyKey,
      jobEventId: escrow.jobEventId,
      providerActorRef: escrow.providerActorRef,
      publicProjection: JSON.parse(escrow.publicProjectionJson),
      requesterActorRef: escrow.requesterActorRef,
      reserveReceiptRef: escrow.reserveReceiptRef,
      releaseReceiptRef: escrow.releaseReceiptRef,
      refundReceiptRef: escrow.refundReceiptRef,
      forfeitReceiptRef: escrow.forfeitReceiptRef,
      forfeitDestination: escrow.forfeitDestination,
      forfeitDestinationActorRef: escrow.forfeitDestinationActorRef,
      forfeitConditionRef: escrow.forfeitConditionRef,
      state: escrow.state,
      updatedAt: nowIso,
      workRequestId: escrow.workRequestId,
    }
  }

  private assertAvailable(actorRef: string): void {
    const balance = this.balances.get(actorRef)
    if (balance !== undefined && balance.balanceMsat < balance.heldMsat) {
      throw new Error('agent_balance_available_nonnegative')
    }
  }

  private applyOne(statement: LedgerStatement): void {
    const sql = statement.sql.replace(/\s+/g, ' ').trim()
    const params = statement.params

    if (sql.startsWith('INSERT INTO agent_balances')) {
      const actorRef = String(params[0])
      if (!this.balances.has(actorRef)) {
        this.balances.set(actorRef, { balanceMsat: 0, heldMsat: 0 })
      }
      return
    }

    if (sql.startsWith('INSERT INTO labor_escrows')) {
      const escrowId = String(params[0])
      if (this.escrows.has(escrowId)) {
        throw new Error('UNIQUE constraint failed: labor_escrows.id')
      }
      this.escrows.set(escrowId, {
        amountMsat: Number(params[4]),
        escrowId,
        idempotencyKey: String(params[1]),
        jobEventId: String(params[6]),
        providerActorRef: null,
        publicProjectionJson: String(params[8]),
        requesterActorRef: String(params[3]),
        reserveReceiptRef: String(params[7]),
        releaseReceiptRef: null,
        refundReceiptRef: null,
        forfeitReceiptRef: null,
        forfeitDestination: null,
        forfeitDestinationActorRef: null,
        forfeitConditionRef: null,
        state: 'reserved',
        workRequestId: String(params[2]),
      })
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('held_msat = held_msat + ?')
    ) {
      const amount = Number(params[0])
      const actorRef = String(params[2])
      const balance =
        this.balances.get(actorRef) ?? { balanceMsat: 0, heldMsat: 0 }
      balance.heldMsat += amount
      this.balances.set(actorRef, balance)
      this.assertAvailable(actorRef)
      return
    }

    if (
      sql.startsWith('INSERT INTO labor_escrow_receipts') &&
      sql.includes('VALUES')
    ) {
      this.insertReceipt({
        escrowId: String(params[1]),
        receiptId: String(params[0]),
        receiptRef: String(params[6]),
        transitionKind: 'reserve',
      })
      return
    }

    if (
      sql.startsWith('INSERT INTO labor_escrow_receipts') &&
      sql.includes('SELECT')
    ) {
      const transitionKind = sql.includes("'release'")
        ? 'release'
        : sql.includes("'forfeit'")
          ? 'forfeit'
          : 'refund'
      const escrowId = String(params[10])
      const escrow = this.escrows.get(escrowId)
      if (escrow === undefined || escrow.state !== 'reserved') {
        return
      }
      this.insertReceipt({
        escrowId,
        receiptId: String(params[0]),
        receiptRef: String(params[3]),
        transitionKind,
      })
      return
    }

    if (
      sql.startsWith('UPDATE labor_escrows') &&
      sql.includes("state = 'forfeited'")
    ) {
      const escrowId = String(params[7])
      if (!this.hasReceipt(String(params[8]))) {
        return
      }
      const escrow = this.escrows.get(escrowId)
      if (escrow !== undefined && escrow.state === 'reserved') {
        escrow.state = 'forfeited'
        escrow.forfeitReceiptRef = String(params[0])
        escrow.forfeitDestination = params[1] as 'counterparty' | 'burn'
        escrow.forfeitDestinationActorRef =
          params[2] === null ? null : String(params[2])
        escrow.forfeitConditionRef = String(params[3])
        escrow.publicProjectionJson = String(params[4])
      }
      return
    }

    if (
      sql.startsWith('UPDATE labor_escrows') &&
      sql.includes("state = 'released_to_provider'")
    ) {
      const escrowId = String(params[6])
      if (!this.hasReceipt(String(params[7]))) {
        return
      }
      const escrow = this.escrows.get(escrowId)
      if (escrow !== undefined && escrow.state === 'reserved') {
        escrow.state = 'released_to_provider'
        escrow.providerActorRef = String(params[0])
        escrow.releaseReceiptRef = String(params[2])
        escrow.publicProjectionJson = String(params[3])
      }
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('held_msat = held_msat - ?') &&
      sql.includes('balance_msat = balance_msat - ?')
    ) {
      if (!this.hasReceipt(String(params[4]))) {
        return
      }
      const actorRef = String(params[3])
      const balance = this.balances.get(actorRef)
      if (balance === undefined) {
        throw new Error('missing requester balance')
      }
      balance.heldMsat -= Number(params[0])
      balance.balanceMsat -= Number(params[1])
      this.assertAvailable(actorRef)
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('balance_msat = balance_msat + ?')
    ) {
      if (params.length > 3 && !this.hasReceipt(String(params[3]))) {
        return
      }
      const actorRef = String(params[2])
      const balance =
        this.balances.get(actorRef) ?? { balanceMsat: 0, heldMsat: 0 }
      balance.balanceMsat += Number(params[0])
      this.balances.set(actorRef, balance)
      return
    }

    if (
      sql.startsWith('UPDATE labor_escrows') &&
      sql.includes("state = 'refunded'")
    ) {
      const escrowId = String(params[4])
      if (!this.hasReceipt(String(params[5]))) {
        return
      }
      const escrow = this.escrows.get(escrowId)
      if (escrow !== undefined && escrow.state === 'reserved') {
        escrow.state = 'refunded'
        escrow.refundReceiptRef = String(params[0])
        escrow.publicProjectionJson = String(params[1])
      }
      return
    }

    if (
      sql.startsWith('UPDATE agent_balances') &&
      sql.includes('held_msat = held_msat - ?')
    ) {
      if (!this.hasReceipt(String(params[3]))) {
        return
      }
      const actorRef = String(params[2])
      const balance = this.balances.get(actorRef)
      if (balance === undefined) {
        throw new Error('missing requester balance')
      }
      balance.heldMsat -= Number(params[0])
      this.assertAvailable(actorRef)
      return
    }

    throw new Error(`model does not understand statement: ${sql}`)
  }

  private hasReceipt(receiptId: string): boolean {
    return this.receipts.some(receipt => receipt.receiptId === receiptId)
  }

  private insertReceipt(receipt: ModeledReceipt): void {
    if (this.receipts.some(existing => existing.receiptId === receipt.receiptId)) {
      throw new Error('UNIQUE constraint failed: labor_escrow_receipts.id')
    }
    if (
      this.receipts.some(
        existing =>
          existing.escrowId === receipt.escrowId &&
          existing.transitionKind === receipt.transitionKind,
      )
    ) {
      throw new Error('UNIQUE constraint failed: labor escrow transition')
    }
    this.receipts.push(receipt)
  }
}

describe('labor escrow ledger statements', () => {
  test('reserve holds available balance and writes a public receipt', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 100_000,
      heldMsat: 0,
    })

    model.apply(reserveLaborEscrowStatements(reserveInput))

    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 100_000,
      heldMsat: 50_000,
    })
    expect(model.escrows.get('escrow_1')?.state).toBe('reserved')
    expect(model.receipts).toMatchObject([
      {
        receiptRef: 'receipt.labor_escrow.reserve.escrow_1',
        transitionKind: 'reserve',
      },
    ])
  })

  test('reserve fails closed when available balance is below the hold', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 40_000,
      heldMsat: 0,
    })

    expect(() => model.apply(reserveLaborEscrowStatements(reserveInput))).toThrow(
      /agent_balance_available_nonnegative/,
    )
    expect(model.escrows.size).toBe(0)
    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 40_000,
      heldMsat: 0,
    })
  })

  test('release requires acceptance evidence and credits provider exactly once', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 100_000,
      heldMsat: 0,
    })
    model.apply(reserveLaborEscrowStatements(reserveInput))
    const escrow = model.asRecord('escrow_1')
    const releaseInput = {
      acceptanceEventRef: 'nostr.event.' + 'b'.repeat(64),
      authority: {
        actorRef: 'agent:requester',
        kind: 'requester_acceptance' as const,
      },
      escrowId: 'escrow_1',
      nowIso,
      providerActorRef: 'agent:provider',
      releaseReceiptId: 'receipt_row_release_1',
      releaseReceiptRef: 'receipt.labor_escrow.release.escrow_1',
    }

    model.apply(releaseLaborEscrowStatements(escrow, releaseInput))
    model.apply(releaseLaborEscrowStatements(model.asRecord('escrow_1'), {
      ...releaseInput,
      releaseReceiptId: 'receipt_row_release_2',
      releaseReceiptRef: 'receipt.labor_escrow.release.escrow_1.retry',
    }))
    model.apply(refundLaborEscrowStatements(model.asRecord('escrow_1'), {
      escrowId: 'escrow_1',
      nowIso,
      refundReasonRef: 'reason.public.too_late',
      refundReceiptId: 'receipt_row_refund_after_release',
      refundReceiptRef: 'receipt.labor_escrow.refund.after_release',
    }))

    expect(model.escrows.get('escrow_1')?.state).toBe('released_to_provider')
    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 50_000,
      heldMsat: 0,
    })
    expect(model.balances.get('agent:provider')).toEqual({
      balanceMsat: 50_000,
      heldMsat: 0,
    })
    expect(
      model.receipts.filter(receipt => receipt.transitionKind === 'release'),
    ).toHaveLength(1)
    expect(
      model.receipts.filter(receipt => receipt.transitionKind === 'refund'),
    ).toHaveLength(0)
  })

  test('refund releases the hold without describing the amount as settled', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 100_000,
      heldMsat: 0,
    })
    model.apply(reserveLaborEscrowStatements(reserveInput))

    model.apply(
      refundLaborEscrowStatements(model.asRecord('escrow_1'), {
        escrowId: 'escrow_1',
        nowIso,
        refundReasonRef: 'reason.public.requester_cancelled_before_acceptance',
        refundReceiptId: 'receipt_row_refund_1',
        refundReceiptRef: 'receipt.labor_escrow.refund.escrow_1',
      }),
    )
    model.apply(
      releaseLaborEscrowStatements(model.asRecord('escrow_1'), {
        acceptanceEventRef: 'nostr.event.' + 'c'.repeat(64),
        authority: {
          actorRef: 'agent:requester',
          kind: 'requester_acceptance',
        },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_after_refund',
        releaseReceiptRef: 'receipt.labor_escrow.release.after_refund',
      }),
    )

    expect(model.escrows.get('escrow_1')?.state).toBe('refunded')
    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 100_000,
      heldMsat: 0,
    })
    expect(model.balances.get('agent:provider')).toEqual({
      balanceMsat: 0,
      heldMsat: 0,
    })
    const projection = JSON.parse(
      model.escrows.get('escrow_1')!.publicProjectionJson,
    )
    expect(JSON.stringify(projection)).not.toMatch(/settled/i)
  })

  test('validator forfeit credits a counterparty exactly once and blocks later refund or release', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 100_000,
      heldMsat: 0,
    })
    model.apply(reserveLaborEscrowStatements(reserveInput))
    const forfeitInput = {
      authority: {
        actorRef: 'agent:validator',
        kind: 'validator_non_acceptance' as const,
      },
      counterpartyActorRef: 'agent:counterparty',
      escrowId: 'escrow_1',
      forfeitConditionRef: 'verdict.public.validator.non_performance',
      forfeitDestination: 'counterparty' as const,
      forfeitReceiptId: 'receipt_row_forfeit_1',
      forfeitReceiptRef: 'receipt.labor_escrow.forfeit.escrow_1',
      nowIso,
    }

    model.apply(
      forfeitLaborEscrowStatements(model.asRecord('escrow_1'), forfeitInput),
    )
    model.apply(
      forfeitLaborEscrowStatements(model.asRecord('escrow_1'), {
        ...forfeitInput,
        forfeitReceiptId: 'receipt_row_forfeit_retry',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.escrow_1.retry',
      }),
    )
    model.apply(
      refundLaborEscrowStatements(model.asRecord('escrow_1'), {
        escrowId: 'escrow_1',
        nowIso,
        refundReasonRef: 'reason.public.too_late',
        refundReceiptId: 'receipt_row_refund_after_forfeit',
        refundReceiptRef: 'receipt.labor_escrow.refund.after_forfeit',
      }),
    )
    model.apply(
      releaseLaborEscrowStatements(model.asRecord('escrow_1'), {
        acceptanceEventRef: 'nostr.event.' + 'd'.repeat(64),
        authority: {
          actorRef: 'agent:requester',
          kind: 'requester_acceptance',
        },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_after_forfeit',
        releaseReceiptRef: 'receipt.labor_escrow.release.after_forfeit',
      }),
    )

    expect(model.escrows.get('escrow_1')?.state).toBe('forfeited')
    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 50_000,
      heldMsat: 0,
    })
    expect(model.balances.get('agent:counterparty')).toEqual({
      balanceMsat: 50_000,
      heldMsat: 0,
    })
    expect(
      model.receipts.filter(receipt => receipt.transitionKind === 'forfeit'),
    ).toHaveLength(1)
    expect(
      model.receipts.filter(receipt => receipt.transitionKind === 'refund'),
    ).toHaveLength(0)
    expect(
      model.receipts.filter(receipt => receipt.transitionKind === 'release'),
    ).toHaveLength(0)
    expect(
      JSON.parse(model.escrows.get('escrow_1')!.publicProjectionJson),
    ).toMatchObject({
      forfeitDestination: 'counterparty',
      forfeitDestinationActorRef: 'agent:counterparty',
      stateAfter: 'forfeited',
      transitionKind: 'forfeit',
    })
  })

  test('burn forfeit debits the held claim without crediting a spender', () => {
    const model = new LaborEscrowLedgerModel()
    model.balances.set('agent:requester', {
      balanceMsat: 100_000,
      heldMsat: 0,
    })
    model.apply(reserveLaborEscrowStatements(reserveInput))

    model.apply(
      forfeitLaborEscrowStatements(model.asRecord('escrow_1'), {
        authority: {
          actorRef: 'agent:validator',
          kind: 'validator_non_acceptance',
        },
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'burn',
        forfeitReceiptId: 'receipt_row_forfeit_burn',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.burn',
        nowIso,
      }),
    )

    expect(model.escrows.get('escrow_1')?.state).toBe('forfeited')
    expect(model.balances.get('agent:requester')).toEqual({
      balanceMsat: 50_000,
      heldMsat: 0,
    })
    expect(model.balances.has('agent:counterparty')).toBe(false)
  })
})

describe('labor escrow D1 guards and projections', () => {
  test('credit-ledger bond settlement adapter preserves fail-closed authority checks', async () => {
    const adapter = createCreditLedgerBondSettlementAdapter(null as never)

    expect(adapter.adapterKind).toBe('credit_ledger')

    await expect(
      adapter.hold({
        ...reserveInput,
        amountMsat: 0,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'invalid_amount',
    })

    await expect(
      adapter.release({
        acceptanceEventRef: 'nostr.event.' + 'd'.repeat(64),
        authority: { actorRef: 'agent:provider', kind: 'provider' },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_adapter_forbidden',
        releaseReceiptRef: 'receipt.labor_escrow.release.adapter_forbidden',
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'release_authority_forbidden',
    })

    await expect(
      adapter.forfeit({
        authority: { actorRef: 'agent:requester', kind: 'requester' },
        counterpartyActorRef: 'agent:counterparty',
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_adapter_forbidden',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.adapter_forbidden',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_authority_forbidden',
    })
  })

  test('provider and worker authority cannot release escrow', async () => {
    await expect(
      releaseLaborEscrow(null as never, {
        acceptanceEventRef: 'nostr.event.' + 'd'.repeat(64),
        authority: { actorRef: 'agent:provider', kind: 'provider' },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_forbidden',
        releaseReceiptRef: 'receipt.labor_escrow.release.forbidden',
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'release_authority_forbidden',
    })

    await expect(
      releaseLaborEscrow(null as never, {
        acceptanceEventRef: 'nostr.event.' + 'e'.repeat(64),
        authority: { actorRef: 'agent:worker', kind: 'worker' },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_worker',
        releaseReceiptRef: 'receipt.labor_escrow.release.worker',
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'release_authority_forbidden',
    })
  })

  test('release requires explicit acceptance evidence', async () => {
    await expect(
      releaseLaborEscrow(null as never, {
        acceptanceEventRef: '',
        authority: { actorRef: 'agent:requester', kind: 'requester_acceptance' },
        escrowId: 'escrow_1',
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_release_no_evidence',
        releaseReceiptRef: 'receipt.labor_escrow.release.no_evidence',
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'release_requires_acceptance_evidence',
    })
  })

  test('only validator non-acceptance can trigger forfeit', async () => {
    await expect(
      forfeitLaborEscrow(null as never, {
        authority: { actorRef: 'agent:provider', kind: 'provider' },
        counterpartyActorRef: 'agent:counterparty',
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_provider',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.provider',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_authority_forbidden',
    })

    await expect(
      forfeitLaborEscrow(null as never, {
        authority: { actorRef: 'agent:worker', kind: 'worker' },
        counterpartyActorRef: 'agent:counterparty',
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_worker',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.worker',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_authority_forbidden',
    })

    await expect(
      forfeitLaborEscrow(null as never, {
        authority: { actorRef: 'agent:requester', kind: 'requester' },
        counterpartyActorRef: 'agent:counterparty',
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_requester',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.requester',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_authority_forbidden',
    })
  })

  test('forfeit requires validator evidence and destination actor when counterparty-directed', async () => {
    await expect(
      forfeitLaborEscrow(null as never, {
        authority: {
          actorRef: 'agent:validator',
          kind: 'validator_non_acceptance',
        },
        counterpartyActorRef: 'agent:counterparty',
        escrowId: 'escrow_1',
        forfeitConditionRef: '',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_no_evidence',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.no_evidence',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_requires_validator_evidence',
    })

    await expect(
      forfeitLaborEscrow(null as never, {
        authority: {
          actorRef: 'agent:validator',
          kind: 'validator_non_acceptance',
        },
        escrowId: 'escrow_1',
        forfeitConditionRef: 'verdict.public.validator.non_performance',
        forfeitDestination: 'counterparty',
        forfeitReceiptId: 'receipt_row_forfeit_no_counterparty',
        forfeitReceiptRef: 'receipt.labor_escrow.forfeit.no_counterparty',
        nowIso,
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'forfeit_counterparty_required',
    })
  })

  test('external invoice funding is a typed gap, not a silent spend path', async () => {
    expect(
      evaluateLaborEscrowFundingSource({
        fundingIntentRef: 'funding_intent.public.external_invoice.pending',
        kind: 'external_invoice',
      }),
    ).toEqual({
      fundingIntentRef: 'funding_intent.public.external_invoice.pending',
      kind: 'blocked',
      reason: 'external_invoice_funding_not_implemented',
    })

    await expect(
      reserveLaborEscrow(null as never, {
        ...reserveInput,
        fundingSource: {
          fundingIntentRef: 'funding_intent.public.external_invoice.pending',
          kind: 'external_invoice',
        },
      }),
    ).resolves.toEqual({
      kind: 'refused',
      reason: 'external_invoice_funding_not_implemented',
    })
  })

  test('public projection scanner rejects private payment material', () => {
    expect(() =>
      assertLaborEscrowPublicSafe({
        receiptRef: 'receipt.public.ok',
        rawInvoice: 'lnbc1please-not-public',
      }),
    ).toThrow(/private, payment/)

    expect(() =>
      buildLaborEscrowPublicProjection({
        amountMsat: 50_000,
        escrowId: 'escrow_1',
        evidenceRef: 'nostr.event.' + 'f'.repeat(64),
        jobEventId,
        providerActorRef: 'agent:provider',
        receiptRef: 'receipt.labor_escrow.release.safe',
        requesterActorRef: 'agent:requester',
        stateAfter: 'released_to_provider',
        transitionKind: 'release',
        workRequestId: 'work_request_1',
      }),
    ).not.toThrow()
  })
})

describe('artanis labor budget gate', () => {
  test('allows bounded labor requests under per-tick and seeded-balance caps', () => {
    expect(
      evaluateArtanisLaborBudgetGate({
        alreadyReservedThisTickMsat: 20_000,
        perTickBudgetMsat: 100_000,
        requestedAmountMsat: 30_000,
        seededBalanceAvailableMsat: 80_000,
      }),
    ).toEqual({ kind: 'allowed', remainingTickBudgetMsat: 50_000 })
  })

  test('refuses invalid, per-tick-over, and seeded-balance-over requests', () => {
    expect(
      evaluateArtanisLaborBudgetGate({
        alreadyReservedThisTickMsat: 0,
        perTickBudgetMsat: 100_000,
        requestedAmountMsat: 0,
        seededBalanceAvailableMsat: 100_000,
      }),
    ).toMatchObject({ kind: 'refused', reason: 'invalid_labor_amount' })

    expect(
      evaluateArtanisLaborBudgetGate({
        alreadyReservedThisTickMsat: 80_000,
        perTickBudgetMsat: 100_000,
        requestedAmountMsat: 30_000,
        seededBalanceAvailableMsat: 100_000,
      }),
    ).toMatchObject({
      kind: 'refused',
      reason: 'per_tick_labor_budget_exceeded',
    })

    expect(
      evaluateArtanisLaborBudgetGate({
        alreadyReservedThisTickMsat: 0,
        perTickBudgetMsat: 100_000,
        requestedAmountMsat: 70_000,
        seededBalanceAvailableMsat: 60_000,
      }),
    ).toMatchObject({
      kind: 'refused',
      reason: 'seeded_balance_ceiling_exceeded',
    })
  })
})
