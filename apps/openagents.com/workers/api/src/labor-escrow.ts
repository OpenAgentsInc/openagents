import type { ForumWorkRequestRecord } from './forum-work-requests'
import { parseJsonUnknown } from './json-boundary'
import {
  type AgentBalanceRow,
  type LedgerStatement,
  readAgentBalance,
  runLedgerStatements,
} from './payments-ledger'

// Labor escrow rides the existing agent credit ledger. It never creates
// external money, never stores payment material, and never calls held value
// settled bitcoin: reserve holds available balance, release moves the claim
// to the provider balance, refund releases the hold.

export const LaborEscrowState = [
  'reserved',
  'released_to_provider',
  'refunded',
] as const
export type LaborEscrowState = (typeof LaborEscrowState)[number]

export const LaborEscrowTransitionKind = [
  'reserve',
  'release',
  'refund',
] as const
export type LaborEscrowTransitionKind =
  (typeof LaborEscrowTransitionKind)[number]

export type LaborEscrowFundingSource =
  | Readonly<{ kind: 'ledger_balance' }>
  | Readonly<{ kind: 'external_invoice'; fundingIntentRef: string }>

export type LaborEscrowReleaseAuthority =
  | Readonly<{ kind: 'requester_acceptance'; actorRef: string }>
  | Readonly<{ kind: 'validator_acceptance'; actorRef: string }>
  | Readonly<{ kind: 'provider'; actorRef: string }>
  | Readonly<{ kind: 'worker'; actorRef: string }>

export type LaborEscrowRecord = Readonly<{
  amountMsat: number
  createdAt: string
  escrowId: string
  fundingSource: 'ledger_balance' | 'external_invoice_pending'
  idempotencyKey: string
  jobEventId: string
  publicProjection: LaborEscrowPublicProjection
  requesterActorRef: string
  providerActorRef: string | null
  reserveReceiptRef: string
  releaseReceiptRef: string | null
  refundReceiptRef: string | null
  state: LaborEscrowState
  updatedAt: string
  workRequestId: string
}>

export type LaborEscrowReceiptRecord = Readonly<{
  amountMsat: number
  createdAt: string
  escrowId: string
  evidenceRef: string | null
  idempotencyKey: string
  providerActorRef: string | null
  publicProjection: LaborEscrowPublicProjection
  receiptId: string
  receiptRef: string
  requesterActorRef: string
  stateAfter: LaborEscrowState
  transitionKind: LaborEscrowTransitionKind
  workRequestId: string
}>

export type LaborEscrowPublicProjection = Readonly<{
  amountMsat: number
  escrowRef: string
  evidenceRef: string | null
  jobEventRef: string
  providerActorRef: string | null
  receiptRef: string
  requesterActorRef: string
  stateAfter: LaborEscrowState
  transitionKind: LaborEscrowTransitionKind
  workRequestRef: string
}>

export type ReserveLaborEscrowInput = Readonly<{
  amountMsat: number
  escrowId: string
  fundingSource?: LaborEscrowFundingSource | undefined
  idempotencyKey: string
  jobEventId: string
  nowIso: string
  requesterActorRef: string
  reserveReceiptId: string
  reserveReceiptRef: string
  workRequestId: string
}>

export type ReleaseLaborEscrowInput = Readonly<{
  acceptanceEventRef: string
  authority: LaborEscrowReleaseAuthority
  escrowId: string
  nowIso: string
  providerActorRef: string
  releaseReceiptId: string
  releaseReceiptRef: string
}>

export type RefundLaborEscrowInput = Readonly<{
  escrowId: string
  nowIso: string
  refundReceiptId: string
  refundReceiptRef: string
  refundReasonRef: string
}>

export type LaborEscrowResult =
  | Readonly<{ kind: 'ok'; escrow: LaborEscrowRecord; idempotent: boolean }>
  | Readonly<{
      kind: 'refused'
      reason:
        | 'external_invoice_funding_not_implemented'
        | 'insufficient_available_balance'
        | 'invalid_amount'
        | 'release_authority_forbidden'
        | 'release_authority_not_requester'
        | 'release_requires_acceptance_evidence'
        | 'escrow_not_found'
        | 'escrow_not_reserved'
      availableMsat?: number
      currentState?: LaborEscrowState
    }>

export type ArtanisLaborBudgetGateDecision =
  | Readonly<{ kind: 'allowed'; remainingTickBudgetMsat: number }>
  | Readonly<{
      kind: 'refused'
      reason:
        | 'invalid_labor_amount'
        | 'per_tick_labor_budget_exceeded'
        | 'seeded_balance_ceiling_exceeded'
      refusalRef: string
    }>

const unsafeLaborEscrowPublicPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|github\.com\/[^:/\s]+\/private|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

export class LaborEscrowUnsafe extends Error {
  override readonly name = 'LaborEscrowUnsafe'
}

const assertPositiveMsat = (amountMsat: number): void => {
  if (!Number.isInteger(amountMsat) || amountMsat <= 0) {
    throw new LaborEscrowUnsafe('labor escrow amount must be positive msat')
  }
}

export const assertLaborEscrowPublicSafe = (
  value: unknown,
  field = 'labor escrow projection',
): void => {
  if (unsafeLaborEscrowPublicPattern.test(JSON.stringify(value) ?? '')) {
    throw new LaborEscrowUnsafe(
      `${field} contains private, payment, credential, wallet, or raw material.`,
    )
  }
}

const normalizeFundingSource = (
  source: LaborEscrowFundingSource | undefined,
): 'ledger_balance' | 'external_invoice_pending' =>
  source?.kind === 'external_invoice'
    ? 'external_invoice_pending'
    : 'ledger_balance'

export const laborEscrowRef = (escrowId: string): string =>
  `labor_escrow.public.${escrowId}`

export const laborWorkRequestRef = (workRequestId: string): string =>
  `work_request.public.${workRequestId}`

export const laborNostrEventRef = (eventIdOrRef: string): string =>
  eventIdOrRef.startsWith('nostr.event.')
    ? eventIdOrRef
    : `nostr.event.${eventIdOrRef}`

export const buildLaborEscrowPublicProjection = (
  input: Readonly<{
    amountMsat: number
    escrowId: string
    evidenceRef: string | null
    jobEventId: string
    providerActorRef: string | null
    receiptRef: string
    requesterActorRef: string
    stateAfter: LaborEscrowState
    transitionKind: LaborEscrowTransitionKind
    workRequestId: string
  }>,
): LaborEscrowPublicProjection => {
  assertPositiveMsat(input.amountMsat)
  const projection: LaborEscrowPublicProjection = {
    amountMsat: input.amountMsat,
    escrowRef: laborEscrowRef(input.escrowId),
    evidenceRef: input.evidenceRef,
    jobEventRef: laborNostrEventRef(input.jobEventId),
    providerActorRef: input.providerActorRef,
    receiptRef: input.receiptRef,
    requesterActorRef: input.requesterActorRef,
    stateAfter: input.stateAfter,
    transitionKind: input.transitionKind,
    workRequestRef: laborWorkRequestRef(input.workRequestId),
  }
  assertLaborEscrowPublicSafe(projection)
  return projection
}

const ensureBalanceRowStatement = (
  actorRef: string,
  nowIso: string,
): LedgerStatement => ({
  params: [actorRef, nowIso, nowIso],
  sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
        VALUES (?, 0, ?, ?)
        ON CONFLICT (actor_ref) DO NOTHING`,
})

export const reserveLaborEscrowStatements = (
  input: ReserveLaborEscrowInput,
): ReadonlyArray<LedgerStatement> => {
  assertPositiveMsat(input.amountMsat)
  const fundingSource = normalizeFundingSource(input.fundingSource)
  if (fundingSource !== 'ledger_balance') {
    throw new LaborEscrowUnsafe('external invoice funding is not implemented')
  }

  const reserveProjection = buildLaborEscrowPublicProjection({
    amountMsat: input.amountMsat,
    escrowId: input.escrowId,
    evidenceRef: laborNostrEventRef(input.jobEventId),
    jobEventId: input.jobEventId,
    providerActorRef: null,
    receiptRef: input.reserveReceiptRef,
    requesterActorRef: input.requesterActorRef,
    stateAfter: 'reserved',
    transitionKind: 'reserve',
    workRequestId: input.workRequestId,
  })

  return [
    ensureBalanceRowStatement(input.requesterActorRef, input.nowIso),
    {
      params: [
        input.escrowId,
        input.idempotencyKey,
        input.workRequestId,
        input.requesterActorRef,
        input.amountMsat,
        fundingSource,
        input.jobEventId,
        input.reserveReceiptRef,
        JSON.stringify(reserveProjection),
        input.nowIso,
        input.nowIso,
      ],
      sql: `INSERT INTO labor_escrows (
              id, idempotency_key, work_request_id, requester_actor_ref,
              provider_actor_ref, amount_msat, state, funding_source,
              job_event_id, acceptance_event_ref, reserve_receipt_ref,
              public_projection_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, NULL, ?, 'reserved', ?, ?, NULL, ?, ?, ?, ?)`,
    },
    {
      params: [input.amountMsat, input.nowIso, input.requesterActorRef],
      sql: `UPDATE agent_balances
            SET held_msat = held_msat + ?, updated_at = ?
            WHERE actor_ref = ?`,
    },
    {
      params: [
        input.reserveReceiptId,
        input.escrowId,
        `${input.idempotencyKey}:reserve`,
        input.workRequestId,
        input.requesterActorRef,
        input.amountMsat,
        input.reserveReceiptRef,
        laborNostrEventRef(input.jobEventId),
        JSON.stringify(reserveProjection),
        input.nowIso,
      ],
      sql: `INSERT INTO labor_escrow_receipts (
              id, escrow_id, idempotency_key, transition_kind,
              work_request_id, requester_actor_ref, provider_actor_ref,
              amount_msat, receipt_ref, evidence_ref, state_after,
              public_projection_json, created_at
            )
            VALUES (?, ?, ?, 'reserve', ?, ?, NULL, ?, ?, ?, 'reserved', ?, ?)`,
    },
  ]
}

const transitionReceiptInsertStatement = (
  input: Readonly<{
    amountMsat: number
    escrowId: string
    evidenceRef: string
    idempotencyKey: string
    jobEventId: string
    nowIso: string
    providerActorRef: string | null
    receiptId: string
    receiptRef: string
    requesterActorRef: string
    stateAfter: 'released_to_provider' | 'refunded'
    transitionKind: 'release' | 'refund'
    workRequestId: string
  }>,
): LedgerStatement => {
  const projection = buildLaborEscrowPublicProjection({
    amountMsat: input.amountMsat,
    escrowId: input.escrowId,
    evidenceRef: input.evidenceRef,
    jobEventId: input.jobEventId,
    providerActorRef: input.providerActorRef,
    receiptRef: input.receiptRef,
    requesterActorRef: input.requesterActorRef,
    stateAfter: input.stateAfter,
    transitionKind: input.transitionKind,
    workRequestId: input.workRequestId,
  })

  return {
    params: [
      input.receiptId,
      input.idempotencyKey,
      input.providerActorRef,
      input.receiptRef,
      input.evidenceRef,
      input.stateAfter,
      JSON.stringify(projection),
      input.nowIso,
      input.escrowId,
    ],
    sql: `INSERT INTO labor_escrow_receipts (
            id, escrow_id, idempotency_key, transition_kind,
            work_request_id, requester_actor_ref, provider_actor_ref,
            amount_msat, receipt_ref, evidence_ref, state_after,
            public_projection_json, created_at
          )
          SELECT ?, e.id, ?, '${input.transitionKind}',
                 e.work_request_id, e.requester_actor_ref, ?,
                 e.amount_msat, ?, ?, ?, ?, ?
            FROM labor_escrows e
           WHERE e.id = ?
             AND e.state = 'reserved'`,
  }
}

export const releaseLaborEscrowStatements = (
  escrow: LaborEscrowRecord,
  input: ReleaseLaborEscrowInput,
): ReadonlyArray<LedgerStatement> => {
  const evidenceRef = input.acceptanceEventRef.trim()
  if (evidenceRef.length === 0) {
    throw new LaborEscrowUnsafe('release requires acceptance evidence ref')
  }

  assertLaborEscrowPublicSafe(evidenceRef, 'acceptance evidence ref')
  const releaseProjection = buildLaborEscrowPublicProjection({
    amountMsat: escrow.amountMsat,
    escrowId: escrow.escrowId,
    evidenceRef,
    jobEventId: escrow.jobEventId,
    providerActorRef: input.providerActorRef,
    receiptRef: input.releaseReceiptRef,
    requesterActorRef: escrow.requesterActorRef,
    stateAfter: 'released_to_provider',
    transitionKind: 'release',
    workRequestId: escrow.workRequestId,
  })

  return [
    transitionReceiptInsertStatement({
      amountMsat: escrow.amountMsat,
      escrowId: escrow.escrowId,
      evidenceRef,
      idempotencyKey: `${escrow.idempotencyKey}:release`,
      jobEventId: escrow.jobEventId,
      nowIso: input.nowIso,
      providerActorRef: input.providerActorRef,
      receiptId: input.releaseReceiptId,
      receiptRef: input.releaseReceiptRef,
      requesterActorRef: escrow.requesterActorRef,
      stateAfter: 'released_to_provider',
      transitionKind: 'release',
      workRequestId: escrow.workRequestId,
    }),
    {
      params: [
        input.providerActorRef,
        evidenceRef,
        input.releaseReceiptRef,
        JSON.stringify(releaseProjection),
        input.nowIso,
        input.nowIso,
        escrow.escrowId,
        input.releaseReceiptId,
      ],
      sql: `UPDATE labor_escrows
            SET state = 'released_to_provider',
                provider_actor_ref = ?,
                acceptance_event_ref = ?,
                release_receipt_ref = ?,
                public_projection_json = ?,
                updated_at = ?,
                released_at = ?
            WHERE id = ?
              AND state = 'reserved'
              AND EXISTS (
                SELECT 1 FROM labor_escrow_receipts WHERE id = ?
              )`,
    },
    {
      params: [
        escrow.amountMsat,
        escrow.amountMsat,
        input.nowIso,
        escrow.requesterActorRef,
        input.releaseReceiptId,
      ],
      sql: `UPDATE agent_balances
            SET held_msat = held_msat - ?,
                balance_msat = balance_msat - ?,
                updated_at = ?
            WHERE actor_ref = ?
              AND EXISTS (
                SELECT 1 FROM labor_escrow_receipts WHERE id = ?
              )`,
    },
    ensureBalanceRowStatement(input.providerActorRef, input.nowIso),
    {
      params: [
        escrow.amountMsat,
        input.nowIso,
        input.providerActorRef,
        input.releaseReceiptId,
      ],
      sql: `UPDATE agent_balances
            SET balance_msat = balance_msat + ?, updated_at = ?
            WHERE actor_ref = ?
              AND EXISTS (
                SELECT 1 FROM labor_escrow_receipts WHERE id = ?
              )`,
    },
  ]
}

export const refundLaborEscrowStatements = (
  escrow: LaborEscrowRecord,
  input: RefundLaborEscrowInput,
): ReadonlyArray<LedgerStatement> => {
  const reasonRef = input.refundReasonRef.trim()
  assertLaborEscrowPublicSafe(reasonRef, 'refund reason ref')
  const refundProjection = buildLaborEscrowPublicProjection({
    amountMsat: escrow.amountMsat,
    escrowId: escrow.escrowId,
    evidenceRef: reasonRef,
    jobEventId: escrow.jobEventId,
    providerActorRef: escrow.providerActorRef,
    receiptRef: input.refundReceiptRef,
    requesterActorRef: escrow.requesterActorRef,
    stateAfter: 'refunded',
    transitionKind: 'refund',
    workRequestId: escrow.workRequestId,
  })

  return [
    transitionReceiptInsertStatement({
      amountMsat: escrow.amountMsat,
      escrowId: escrow.escrowId,
      evidenceRef: reasonRef,
      idempotencyKey: `${escrow.idempotencyKey}:refund`,
      jobEventId: escrow.jobEventId,
      nowIso: input.nowIso,
      providerActorRef: escrow.providerActorRef,
      receiptId: input.refundReceiptId,
      receiptRef: input.refundReceiptRef,
      requesterActorRef: escrow.requesterActorRef,
      stateAfter: 'refunded',
      transitionKind: 'refund',
      workRequestId: escrow.workRequestId,
    }),
    {
      params: [
        input.refundReceiptRef,
        JSON.stringify(refundProjection),
        input.nowIso,
        input.nowIso,
        escrow.escrowId,
        input.refundReceiptId,
      ],
      sql: `UPDATE labor_escrows
            SET state = 'refunded',
                refund_receipt_ref = ?,
                public_projection_json = ?,
                updated_at = ?,
                refunded_at = ?
            WHERE id = ?
              AND state = 'reserved'
              AND EXISTS (
                SELECT 1 FROM labor_escrow_receipts WHERE id = ?
              )`,
    },
    {
      params: [
        escrow.amountMsat,
        input.nowIso,
        escrow.requesterActorRef,
        input.refundReceiptId,
      ],
      sql: `UPDATE agent_balances
            SET held_msat = held_msat - ?,
                updated_at = ?
            WHERE actor_ref = ?
              AND EXISTS (
                SELECT 1 FROM labor_escrow_receipts WHERE id = ?
              )`,
    },
  ]
}

type LaborEscrowRow = Readonly<{
  amount_msat: number
  created_at: string
  funding_source: 'ledger_balance' | 'external_invoice_pending'
  id: string
  idempotency_key: string
  job_event_id: string
  provider_actor_ref: string | null
  public_projection_json: string
  requester_actor_ref: string
  reserve_receipt_ref: string
  release_receipt_ref: string | null
  refund_receipt_ref: string | null
  state: LaborEscrowState
  updated_at: string
  work_request_id: string
}>

const parseProjection = (json: string): LaborEscrowPublicProjection => {
  const parsed = parseJsonUnknown(json) as LaborEscrowPublicProjection
  assertLaborEscrowPublicSafe(parsed)
  return parsed
}

const escrowFromRow = (row: LaborEscrowRow): LaborEscrowRecord => ({
  amountMsat: Number(row.amount_msat),
  createdAt: row.created_at,
  escrowId: row.id,
  fundingSource: row.funding_source,
  idempotencyKey: row.idempotency_key,
  jobEventId: row.job_event_id,
  providerActorRef: row.provider_actor_ref,
  publicProjection: parseProjection(row.public_projection_json),
  requesterActorRef: row.requester_actor_ref,
  reserveReceiptRef: row.reserve_receipt_ref,
  releaseReceiptRef: row.release_receipt_ref,
  refundReceiptRef: row.refund_receipt_ref,
  state: row.state,
  updatedAt: row.updated_at,
  workRequestId: row.work_request_id,
})

export const readLaborEscrowById = async (
  db: D1Database,
  escrowId: string,
): Promise<LaborEscrowRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM labor_escrows
        WHERE id = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(escrowId)
    .first<LaborEscrowRow>()

  return row === null ? null : escrowFromRow(row)
}

export const readLaborEscrowByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<LaborEscrowRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM labor_escrows
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<LaborEscrowRow>()

  return row === null ? null : escrowFromRow(row)
}

export const reserveLaborEscrow = async (
  db: D1Database,
  input: ReserveLaborEscrowInput,
): Promise<LaborEscrowResult> => {
  if (!Number.isInteger(input.amountMsat) || input.amountMsat <= 0) {
    return { kind: 'refused', reason: 'invalid_amount' }
  }

  if (input.fundingSource?.kind === 'external_invoice') {
    assertLaborEscrowPublicSafe(
      input.fundingSource.fundingIntentRef,
      'external funding intent ref',
    )
    return {
      kind: 'refused',
      reason: 'external_invoice_funding_not_implemented',
    }
  }

  const existing = await readLaborEscrowByIdempotencyKey(
    db,
    input.idempotencyKey,
  )
  if (existing !== null) {
    return { escrow: existing, idempotent: true, kind: 'ok' }
  }

  const balance: AgentBalanceRow | null = await readAgentBalance(
    db,
    input.requesterActorRef,
  )
  const availableMsat = balance?.availableMsat ?? 0
  if (availableMsat < input.amountMsat) {
    return {
      availableMsat,
      kind: 'refused',
      reason: 'insufficient_available_balance',
    }
  }

  await runLedgerStatements(db, reserveLaborEscrowStatements(input))
  const escrow = await readLaborEscrowById(db, input.escrowId)
  if (escrow === null) {
    return { kind: 'refused', reason: 'escrow_not_found' }
  }
  return { escrow, idempotent: false, kind: 'ok' }
}

export const releaseLaborEscrow = async (
  db: D1Database,
  input: ReleaseLaborEscrowInput,
): Promise<LaborEscrowResult> => {
  if (
    input.authority.kind === 'provider' ||
    input.authority.kind === 'worker'
  ) {
    return { kind: 'refused', reason: 'release_authority_forbidden' }
  }

  if (input.acceptanceEventRef.trim().length === 0) {
    return {
      kind: 'refused',
      reason: 'release_requires_acceptance_evidence',
    }
  }

  const escrow = await readLaborEscrowById(db, input.escrowId)
  if (escrow === null) {
    return { kind: 'refused', reason: 'escrow_not_found' }
  }

  if (
    input.authority.kind === 'requester_acceptance' &&
    input.authority.actorRef !== escrow.requesterActorRef
  ) {
    return { kind: 'refused', reason: 'release_authority_not_requester' }
  }

  if (escrow.state !== 'reserved') {
    return {
      currentState: escrow.state,
      kind: 'refused',
      reason: 'escrow_not_reserved',
    }
  }

  await runLedgerStatements(db, releaseLaborEscrowStatements(escrow, input))
  const released = await readLaborEscrowById(db, input.escrowId)
  if (released === null) {
    return { kind: 'refused', reason: 'escrow_not_found' }
  }
  return { escrow: released, idempotent: false, kind: 'ok' }
}

export const refundLaborEscrow = async (
  db: D1Database,
  input: RefundLaborEscrowInput,
): Promise<LaborEscrowResult> => {
  const escrow = await readLaborEscrowById(db, input.escrowId)
  if (escrow === null) {
    return { kind: 'refused', reason: 'escrow_not_found' }
  }

  if (escrow.state !== 'reserved') {
    return {
      currentState: escrow.state,
      kind: 'refused',
      reason: 'escrow_not_reserved',
    }
  }

  await runLedgerStatements(db, refundLaborEscrowStatements(escrow, input))
  const refunded = await readLaborEscrowById(db, input.escrowId)
  if (refunded === null) {
    return { kind: 'refused', reason: 'escrow_not_found' }
  }
  return { escrow: refunded, idempotent: false, kind: 'ok' }
}

export const reserveInputFromForumWorkRequest = (
  request: ForumWorkRequestRecord,
  options: Readonly<{
    escrowId: string
    idempotencyKey: string
    nowIso: string
    reserveReceiptId: string
    reserveReceiptRef: string
  }>,
): ReserveLaborEscrowInput => ({
  amountMsat: request.budgetMsats,
  escrowId: options.escrowId,
  fundingSource: { kind: 'ledger_balance' },
  idempotencyKey: options.idempotencyKey,
  jobEventId: request.jobEventId,
  nowIso: options.nowIso,
  requesterActorRef: request.requesterActorRef,
  reserveReceiptId: options.reserveReceiptId,
  reserveReceiptRef: options.reserveReceiptRef,
  workRequestId: request.workRequestId,
})

export const evaluateLaborEscrowFundingSource = (
  source: LaborEscrowFundingSource,
):
  | Readonly<{ kind: 'supported'; fundingSource: 'ledger_balance' }>
  | Readonly<{
      kind: 'blocked'
      reason: 'external_invoice_funding_not_implemented'
      fundingIntentRef: string
    }> => {
  if (source.kind === 'ledger_balance') {
    return { fundingSource: 'ledger_balance', kind: 'supported' }
  }
  assertLaborEscrowPublicSafe(source.fundingIntentRef, 'funding intent ref')
  return {
    fundingIntentRef: source.fundingIntentRef,
    kind: 'blocked',
    reason: 'external_invoice_funding_not_implemented',
  }
}

export const evaluateArtanisLaborBudgetGate = (input: Readonly<{
  alreadyReservedThisTickMsat: number
  perTickBudgetMsat: number
  requestedAmountMsat: number
  seededBalanceAvailableMsat: number
}>): ArtanisLaborBudgetGateDecision => {
  const requested = Math.trunc(input.requestedAmountMsat)
  if (requested <= 0) {
    return {
      kind: 'refused',
      reason: 'invalid_labor_amount',
      refusalRef: 'refusal.artanis_labor_budget.invalid_amount',
    }
  }

  const tickAfter = input.alreadyReservedThisTickMsat + requested
  if (tickAfter > input.perTickBudgetMsat) {
    return {
      kind: 'refused',
      reason: 'per_tick_labor_budget_exceeded',
      refusalRef: 'refusal.artanis_labor_budget.per_tick_cap',
    }
  }

  if (requested > input.seededBalanceAvailableMsat) {
    return {
      kind: 'refused',
      reason: 'seeded_balance_ceiling_exceeded',
      refusalRef: 'refusal.artanis_labor_budget.seeded_balance_ceiling',
    }
  }

  return {
    kind: 'allowed',
    remainingTickBudgetMsat: input.perTickBudgetMsat - tickAfter,
  }
}
