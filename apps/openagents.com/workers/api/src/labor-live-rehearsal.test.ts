import { describe, expect, test } from 'vitest'
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  decodeLbrAcceptanceEvent,
  decodeLbrAgenticCodingRequestEvent,
  decodeLbrQuoteEvent,
  decodeLbrResultEvent,
  lbrAcceptanceToDraft,
  lbrAgenticCodingRequestToDraft,
  lbrQuoteToDraft,
  lbrResultToDraft,
  makeLbrAcceptance,
  makeLbrAgenticCodingRequest,
  makeLbrQuote,
  makeLbrResult,
  type LbrAcceptance,
  type LbrAgenticCodingRequest,
} from '@openagentsinc/nip90'

import {
  assertLaborEscrowPublicSafe,
  releaseLaborEscrowStatements,
  reserveLaborEscrowStatements,
  type LaborEscrowRecord,
  type LaborEscrowState,
} from './labor-escrow'
import type { LedgerStatement } from './payments-ledger'

type FakeNostrEvent = Readonly<{
  content: string
  created_at: number
  id: string
  kind: number
  pubkey: string
  sig: string
  tags: ReadonlyArray<readonly string[]>
}>

type ModeledEscrow = Readonly<{
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
}>

type ModeledReceipt = Readonly<{
  escrowId: string
  receiptId: string
  receiptRef: string
  transitionKind: 'reserve' | 'release'
}>

type MockRunnerInput = Readonly<{
  acceptance: LbrAcceptance
  escrow: LaborEscrowRecord
  request: LbrAgenticCodingRequest
}>

const nowIso = '2026-06-10T23:50:00.000Z'
const requesterPubkey = '1'.repeat(64)
const providerPubkey = '2'.repeat(64)
const requestEventId = 'a'.repeat(64)
const quoteEventId = 'b'.repeat(64)
const acceptanceEventId = 'c'.repeat(64)
const resultEventId = 'd'.repeat(64)

const eventFromDraft = (
  draft: Readonly<{
    content: string
    kind: number
    tags: ReadonlyArray<readonly string[]>
  }>,
  overrides: Readonly<{
    id: string
    pubkey: string
  }>,
): FakeNostrEvent => ({
  content: draft.content,
  created_at: 1_780_000_000,
  id: overrides.id,
  kind: draft.kind,
  pubkey: overrides.pubkey,
  sig: 'f'.repeat(128),
  tags: draft.tags,
})

class FakeLaborRelay {
  readonly relayUrl = 'wss://relay.test.openagents.dev'
  readonly events: Array<FakeNostrEvent> = []

  publish(event: FakeNostrEvent): void {
    assertLaborEscrowPublicSafe(event, 'rehearsal relay event')
    this.events.push(event)
  }
}

class RehearsalLedgerModel {
  readonly balances = new Map<string, { balanceMsat: number; heldMsat: number }>()
  readonly escrows = new Map<string, ModeledEscrow>()
  readonly receipts: Array<ModeledReceipt> = []

  apply(statements: ReadonlyArray<LedgerStatement>): void {
    const snapshotBalances = new Map(
      Array.from(this.balances, ([key, value]) => [key, { ...value }] as const),
    )
    const snapshotEscrows = new Map(this.escrows)
    const snapshotReceipts = this.receipts.map(receipt => ({ ...receipt }))

    try {
      statements.forEach(statement => this.applyOne(statement))
    } catch (error) {
      this.balances.clear()
      snapshotBalances.forEach((value, key) => this.balances.set(key, value))
      this.escrows.clear()
      snapshotEscrows.forEach((value, key) => this.escrows.set(key, value))
      this.receipts.splice(0, this.receipts.length, ...snapshotReceipts)
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
      const actorRef = String(params[2])
      const balance =
        this.balances.get(actorRef) ?? { balanceMsat: 0, heldMsat: 0 }
      balance.heldMsat += Number(params[0])
      if (balance.heldMsat > balance.balanceMsat) {
        throw new Error('agent_balance_available_nonnegative')
      }
      this.balances.set(actorRef, balance)
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
      const escrowId = String(params[10])
      const escrow = this.escrows.get(escrowId)
      if (escrow !== undefined && escrow.state === 'reserved') {
        this.insertReceipt({
          escrowId,
          receiptId: String(params[0]),
          receiptRef: String(params[3]),
          transitionKind: 'release',
        })
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
        this.escrows.set(escrowId, {
          ...escrow,
          providerActorRef: String(params[0]),
          publicProjectionJson: String(params[3]),
          releaseReceiptRef: String(params[2]),
          state: 'released_to_provider',
        })
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
      if (balance.heldMsat < 0 || balance.balanceMsat < balance.heldMsat) {
        throw new Error('agent_balance_available_nonnegative')
      }
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

    throw new Error(`rehearsal ledger does not understand statement: ${sql}`)
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

const mockContributorRunner = async (input: MockRunnerInput) => {
  expect(input.request.verificationCommandRef).toBe(
    'command.public.pylon.labor.bun_test',
  )
  expect(input.acceptance.escrowReceiptRef).toBe(input.escrow.reserveReceiptRef)
  return {
    artifactRefs: ['artifact.public.lbr.rehearsal.patch_1'],
    platformCloseoutRef: 'closeout.public.lbr.rehearsal_1',
    summaryRef: 'summary.public.lbr.rehearsal.output_only',
    testRef: 'test.public.lbr.rehearsal.bun_test.passed',
  }
}

describe('first negotiated labor job rehearsal', () => {
  test('drives request, quote, acceptance, escrow, delivery, validation, release, and receipt refs without live spend', async () => {
    const relay = new FakeLaborRelay()
    const ledger = new RehearsalLedgerModel()
    ledger.balances.set('agent:requester', {
      balanceMsat: 2_000_000,
      heldMsat: 0,
    })

    const request = makeLbrAgenticCodingRequest({
      bidMsats: 2_000_000,
      deadline: 'deadline.public.lbr.20260612',
      forumTopicRef: 'forum_topic.public.lbr.rehearsal_1',
      objectiveRef: 'objective.public.lbr.rehearsal.fix_test',
      relays: [relay.relayUrl],
      repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      verificationCommandRef: 'command.public.pylon.labor.bun_test',
    })
    const requestEvent = eventFromDraft(
      lbrAgenticCodingRequestToDraft(request),
      { id: requestEventId, pubkey: requesterPubkey },
    )
    relay.publish(requestEvent)

    const parsedRequest = decodeLbrAgenticCodingRequestEvent(requestEvent)
    expect(parsedRequest.kind).toBe(LBR_AGENTIC_CODING_REQUEST_KIND)
    expect(parsedRequest.outputDelivery).toBe('output_only')

    const quote = makeLbrQuote({
      amountMsats: 1_500_000,
      capabilityRefs: ['capability.pylon.local_claude_agent'],
      providerRef: 'provider.public.pylon.independent_rehearsal',
      quoteRef: 'quote.public.lbr.rehearsal_1',
      requestId: requestEvent.id,
      requestRelay: relay.relayUrl,
      requesterPubkey,
    })
    const quoteEvent = eventFromDraft(lbrQuoteToDraft(quote), {
      id: quoteEventId,
      pubkey: providerPubkey,
    })
    relay.publish(quoteEvent)

    const parsedQuote = decodeLbrQuoteEvent(quoteEvent)
    expect(parsedQuote.amountMsats).toBe(1_500_000)
    expect(parsedQuote.quoteRef).toBe('quote.public.lbr.rehearsal_1')

    const reserveReceiptRef = 'receipt.labor_escrow.reserve.rehearsal_1'
    ledger.apply(
      reserveLaborEscrowStatements({
        amountMsat: parsedQuote.amountMsats,
        escrowId: 'escrow_lbr_rehearsal_1',
        fundingSource: { kind: 'ledger_balance' },
        idempotencyKey: 'labor-rehearsal:reserve:1',
        jobEventId: requestEvent.id,
        nowIso,
        requesterActorRef: 'agent:requester',
        reserveReceiptId: 'receipt_row_lbr_rehearsal_reserve_1',
        reserveReceiptRef,
        workRequestId: 'work_request_lbr_rehearsal_1',
      }),
    )
    const reserved = ledger.asRecord('escrow_lbr_rehearsal_1')
    expect(reserved.state).toBe('reserved')

    const acceptance = makeLbrAcceptance({
      acceptanceRef: 'acceptance.public.lbr.rehearsal_1',
      escrowReceiptRef: reserveReceiptRef,
      providerPubkey,
      requestId: requestEvent.id,
      requestRelay: relay.relayUrl,
    })
    const acceptanceEvent = eventFromDraft(lbrAcceptanceToDraft(acceptance), {
      id: acceptanceEventId,
      pubkey: requesterPubkey,
    })
    relay.publish(acceptanceEvent)

    const parsedAcceptance = decodeLbrAcceptanceEvent(acceptanceEvent)
    expect(parsedAcceptance.escrowReceiptRef).toBe(reserveReceiptRef)
    expect(parsedAcceptance.providerPubkey).toBe(providerPubkey)

    const runnerResult = await mockContributorRunner({
      acceptance: parsedAcceptance,
      escrow: reserved,
      request: parsedRequest,
    })
    const result = makeLbrResult({
      artifactRefs: runnerResult.artifactRefs,
      platformCloseoutRef: runnerResult.platformCloseoutRef,
      requestId: requestEvent.id,
      requestRelay: relay.relayUrl,
      requesterPubkey,
      summaryRef: runnerResult.summaryRef,
      testRef: runnerResult.testRef,
    })
    const resultEvent = eventFromDraft(lbrResultToDraft(result), {
      id: resultEventId,
      pubkey: providerPubkey,
    })
    relay.publish(resultEvent)

    const parsedResult = decodeLbrResultEvent(resultEvent)
    expect(parsedResult.kind).toBe(LBR_AGENTIC_CODING_RESULT_KIND)
    expect(parsedResult.artifactRefs).toEqual([
      'artifact.public.lbr.rehearsal.patch_1',
    ])
    expect(parsedResult.testRef).toBe(
      'test.public.lbr.rehearsal.bun_test.passed',
    )

    const verificationVerdictRef =
      'verdict.public.lbr.rehearsal.validator_passed'
    const releaseReceiptRef = 'receipt.labor_escrow.release.rehearsal_1'
    ledger.apply(
      releaseLaborEscrowStatements(reserved, {
        acceptanceEventRef: verificationVerdictRef,
        authority: {
          actorRef: 'agent:requester',
          kind: 'requester_acceptance',
        },
        escrowId: reserved.escrowId,
        nowIso,
        providerActorRef: 'agent:provider',
        releaseReceiptId: 'receipt_row_lbr_rehearsal_release_1',
        releaseReceiptRef,
      }),
    )

    const released = ledger.asRecord('escrow_lbr_rehearsal_1')
    const evidenceBundle = {
      acceptanceEventRef: `nostr.event.${acceptanceEvent.id}`,
      closeoutRef: runnerResult.platformCloseoutRef,
      jobEventRef: `nostr.event.${requestEvent.id}`,
      payoutRung: 'ci_safe_provider_balance_credit',
      quoteEventRef: `nostr.event.${quoteEvent.id}`,
      receiptRefs: [reserveReceiptRef, releaseReceiptRef],
      resultEventRef: `nostr.event.${resultEvent.id}`,
      streamKind: 'labor',
      topicRef: 'forum_topic.public.lbr.rehearsal_1',
      verificationVerdictRef,
    }

    assertLaborEscrowPublicSafe(evidenceBundle, 'labor rehearsal evidence')
    expect(JSON.stringify(evidenceBundle)).not.toMatch(
      /lnbc|preimage|payment_hash|mnemonic|secret|token|\/Users\//i,
    )
    expect(released).toMatchObject({
      providerActorRef: 'agent:provider',
      releaseReceiptRef,
      state: 'released_to_provider',
    })
    expect(ledger.balances.get('agent:requester')).toEqual({
      balanceMsat: 500_000,
      heldMsat: 0,
    })
    expect(ledger.balances.get('agent:provider')).toEqual({
      balanceMsat: 1_500_000,
      heldMsat: 0,
    })
    expect(ledger.receipts.map(receipt => receipt.transitionKind)).toEqual([
      'reserve',
      'release',
    ])
    expect(relay.events.map(event => event.kind)).toEqual([
      LBR_AGENTIC_CODING_REQUEST_KIND,
      LBR_FEEDBACK_KIND,
      LBR_FEEDBACK_KIND,
      LBR_AGENTIC_CODING_RESULT_KIND,
    ])
  })
})
