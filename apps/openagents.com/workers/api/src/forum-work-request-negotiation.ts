import { Effect } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  ForumStorageError,
  ForumValidationError,
} from './forum/repository'

export type ForumWorkRequestOfferState =
  | 'accepted'
  | 'expired'
  | 'offered'
  | 'rejected'

export type ForumWorkRequestOfferRecord = Readonly<{
  amountMsats: number
  amountSats: number
  capabilityRefs: ReadonlyArray<string>
  createdAt: string
  offerId: string
  providerActorRef: string
  providerPubkey: string | null
  publicProjection: Record<string, unknown>
  quoteRef: string
  relayEventRef: string | null
  state: ForumWorkRequestOfferState
  updatedAt: string
  workRequestId: string
}>

export type ForumWorkRequestResultRecord = Readonly<{
  artifactRefs: ReadonlyArray<string>
  closeoutRef: string | null
  createdAt: string
  offerId: string
  providerActorRef: string
  publicProjection: Record<string, unknown>
  quoteRef: string
  resultEventRef: string
  resultId: string
  verificationCommandRef: string
  workRequestId: string
}>

export type ForumWorkRequestAcceptanceRecord = Readonly<{
  acceptanceEventRef: string
  acceptanceId: string
  amountMsats: number
  createdAt: string
  escrowId: string
  idempotencyKey: string
  offerId: string
  providerActorRef: string
  publicProjection: Record<string, unknown>
  quoteRef: string
  requesterActorRef: string
  reserveReceiptRef: string
  workRequestId: string
}>

export type ForumWorkRequestOfferInput = Readonly<{
  amountSats: number
  capabilityRefs: ReadonlyArray<string>
  offerId: string
  providerActorRef: string
  providerPubkey?: string | null
  quoteRef: string
  relayEventRef?: string | null
  workRequestId: string
}>

export type ForumWorkRequestResultInput = Readonly<{
  artifactRefs?: ReadonlyArray<string> | undefined
  closeoutRef?: string | null
  offerId: string
  providerActorRef: string
  quoteRef: string
  resultEventRef: string
  resultId: string
  verificationCommandRef: string
  workRequestId: string
}>

export type ForumWorkRequestAcceptanceInput = Readonly<{
  acceptanceEventRef: string
  acceptanceId: string
  amountMsats: number
  escrowId: string
  idempotencyKey: string
  nowIso: string
  offerId: string
  providerActorRef: string
  quoteRef: string
  requesterActorRef: string
  reserveReceiptRef: string
  workRequestId: string
}>

export class ForumWorkRequestNegotiationUnsafe extends Error {
  override readonly name = 'ForumWorkRequestNegotiationUnsafe'
}

type ForumWorkRequestOfferRow = Readonly<{
  amount_msats: number
  amount_sats: number
  capability_refs_json: string
  created_at: string
  id: string
  provider_actor_ref: string
  provider_pubkey: string | null
  public_projection_json: string
  quote_ref: string
  relay_event_ref: string | null
  state: ForumWorkRequestOfferState
  updated_at: string
  work_request_id: string
}>

type ForumWorkRequestResultRow = Readonly<{
  artifact_refs_json: string
  closeout_ref: string | null
  created_at: string
  id: string
  offer_id: string
  provider_actor_ref: string
  public_projection_json: string
  quote_ref: string
  result_event_ref: string
  verification_command_ref: string
  work_request_id: string
}>

type ForumWorkRequestAcceptanceRow = Readonly<{
  acceptance_event_ref: string
  amount_msats: number
  created_at: string
  escrow_id: string
  id: string
  idempotency_key: string
  offer_id: string
  provider_actor_ref: string
  public_projection_json: string
  quote_ref: string
  requester_actor_ref: string
  reserve_receipt_ref: string
  work_request_id: string
}>

const unsafeNegotiationPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|github\.com\/[^:/\s]+\/private|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const assertPublicSafe = (value: unknown, field: string): void => {
  if (unsafeNegotiationPattern.test(JSON.stringify(value) ?? '')) {
    throw new ForumWorkRequestNegotiationUnsafe(
      `${field} contains private, payment, credential, wallet, or raw material.`,
    )
  }
}

const normalizeRefs = (
  refs: ReadonlyArray<string>,
  field: string,
): ReadonlyArray<string> => {
  const normalized = Array.from(
    new Set(refs.map(ref => ref.trim()).filter(ref => ref.length > 0)),
  )

  if (normalized.length === 0 || normalized.length > 12) {
    throw new ForumWorkRequestNegotiationUnsafe(
      `${field} must contain 1-12 public refs.`,
    )
  }

  assertPublicSafe(normalized, field)
  return normalized
}

const ProviderPubkeyPattern = /^[0-9a-f]{64}$/i

const normalizeProviderPubkey = (
  pubkey: string | null | undefined,
): string | null => {
  if (pubkey === null || pubkey === undefined) {
    return null
  }

  const trimmed = pubkey.trim().toLowerCase()

  if (trimmed.length === 0) {
    return null
  }

  if (!ProviderPubkeyPattern.test(trimmed)) {
    throw new ForumWorkRequestNegotiationUnsafe(
      'providerPubkey must be a 64-char hex nostr pubkey.',
    )
  }

  return trimmed
}

const normalizeResultArtifactRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = Array.from(
    new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref.length > 0)),
  )

  if (normalized.length > 12) {
    throw new ForumWorkRequestNegotiationUnsafe(
      'artifactRefs accepts at most 12 public refs.',
    )
  }

  assertPublicSafe(normalized, 'artifactRefs')
  return normalized
}

const offerProjection = (
  input: Readonly<{
    amountMsats: number
    capabilityRefs: ReadonlyArray<string>
    offerId: string
    providerActorRef: string
    providerPubkey: string | null
    quoteRef: string
    relayEventRef: string | null
    state: ForumWorkRequestOfferState
    workRequestId: string
  }>,
): Record<string, unknown> => {
  const projection = {
    amountMsats: input.amountMsats,
    capabilityRefs: input.capabilityRefs,
    offerRef: `work_offer.public.${input.offerId}`,
    providerActorRef: input.providerActorRef,
    providerPubkey: input.providerPubkey,
    quoteRef: input.quoteRef,
    relayEventRef: input.relayEventRef,
    state: input.state,
    workRequestRef: `work_request.public.${input.workRequestId}`,
  }
  assertPublicSafe(projection, 'work request offer projection')
  return projection
}

const resultProjection = (
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    closeoutRef: string | null
    offerId: string
    providerActorRef: string
    quoteRef: string
    resultEventRef: string
    resultId: string
    verificationCommandRef: string
    workRequestId: string
  }>,
): Record<string, unknown> => {
  const projection = {
    artifactRefs: input.artifactRefs,
    closeoutRef: input.closeoutRef,
    offerRef: `work_offer.public.${input.offerId}`,
    providerActorRef: input.providerActorRef,
    quoteRef: input.quoteRef,
    resultEventRef: input.resultEventRef,
    resultRef: `work_result.public.${input.resultId}`,
    verificationCommandRef: input.verificationCommandRef,
    workRequestRef: `work_request.public.${input.workRequestId}`,
  }
  assertPublicSafe(projection, 'work request result projection')
  return projection
}

const acceptanceProjection = (
  input: ForumWorkRequestAcceptanceInput,
): Record<string, unknown> => {
  const projection = {
    acceptanceEventRef: input.acceptanceEventRef,
    acceptanceRef: `work_acceptance.public.${input.acceptanceId}`,
    amountMsats: input.amountMsats,
    escrowRef: `labor_escrow.public.${input.escrowId}`,
    providerActorRef: input.providerActorRef,
    quoteRef: input.quoteRef,
    requesterActorRef: input.requesterActorRef,
    reserveReceiptRef: input.reserveReceiptRef,
    workRequestRef: `work_request.public.${input.workRequestId}`,
  }
  assertPublicSafe(projection, 'work request acceptance projection')
  return projection
}

const storageError = (operation: string, error: unknown): ForumStorageError =>
  new ForumStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ForumStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const validationError = (error: unknown): ForumValidationError =>
  new ForumValidationError({
    reason: error instanceof Error ? error.message : String(error),
  })

const offerFromRow = (row: ForumWorkRequestOfferRow): ForumWorkRequestOfferRecord => ({
  amountMsats: row.amount_msats,
  amountSats: row.amount_sats,
  capabilityRefs: parseJsonStringArray(row.capability_refs_json),
  createdAt: row.created_at,
  offerId: row.id,
  providerActorRef: row.provider_actor_ref,
  providerPubkey: row.provider_pubkey ?? null,
  publicProjection: parseJsonRecord(row.public_projection_json) ?? {},
  quoteRef: row.quote_ref,
  relayEventRef: row.relay_event_ref,
  state: row.state,
  updatedAt: row.updated_at,
  workRequestId: row.work_request_id,
})

const resultFromRow = (
  row: ForumWorkRequestResultRow,
): ForumWorkRequestResultRecord => ({
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  closeoutRef: row.closeout_ref,
  createdAt: row.created_at,
  offerId: row.offer_id,
  providerActorRef: row.provider_actor_ref,
  publicProjection: parseJsonRecord(row.public_projection_json) ?? {},
  quoteRef: row.quote_ref,
  resultEventRef: row.result_event_ref,
  resultId: row.id,
  verificationCommandRef: row.verification_command_ref,
  workRequestId: row.work_request_id,
})

const acceptanceFromRow = (row: ForumWorkRequestAcceptanceRow): ForumWorkRequestAcceptanceRecord => ({
  acceptanceEventRef: row.acceptance_event_ref,
  acceptanceId: row.id,
  amountMsats: row.amount_msats,
  createdAt: row.created_at,
  escrowId: row.escrow_id,
  idempotencyKey: row.idempotency_key,
  offerId: row.offer_id,
  providerActorRef: row.provider_actor_ref,
  publicProjection: parseJsonRecord(row.public_projection_json) ?? {},
  quoteRef: row.quote_ref,
  requesterActorRef: row.requester_actor_ref,
  reserveReceiptRef: row.reserve_receipt_ref,
  workRequestId: row.work_request_id,
})

export const recordForumWorkRequestOffer = (
  db: D1Database,
  input: ForumWorkRequestOfferInput,
  nowIso: string,
): Effect.Effect<ForumWorkRequestOfferRecord, ForumStorageError | ForumValidationError> =>
  Effect.gen(function* () {
    if (!Number.isInteger(input.amountSats) || input.amountSats <= 0) {
      return yield* new ForumValidationError({
        reason: 'work request offer amountSats must be positive.',
      })
    }

    const amountMsats = input.amountSats * 1000
    const relayEventRef = input.relayEventRef ?? null
    const { capabilityRefs, projection, providerPubkey } = yield* Effect.try({
      catch: validationError,
      try: () => {
        const capabilityRefs = normalizeRefs(
          input.capabilityRefs,
          'capabilityRefs',
        )
        const providerPubkey = normalizeProviderPubkey(input.providerPubkey)
        return {
          capabilityRefs,
          projection: offerProjection({
            amountMsats,
            capabilityRefs,
            offerId: input.offerId,
            providerActorRef: input.providerActorRef,
            providerPubkey,
            quoteRef: input.quoteRef,
            relayEventRef,
            state: 'offered',
            workRequestId: input.workRequestId,
          }),
          providerPubkey,
        }
      },
    })

    yield* d1Effect('forumWorkRequests.insertOffer', () =>
      db
        .prepare(
          `INSERT INTO forum_work_request_offers (
             id,
             work_request_id,
             quote_ref,
             provider_actor_ref,
             provider_pubkey,
             amount_sats,
             amount_msats,
             capability_refs_json,
             relay_event_ref,
             state,
             public_projection_json,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'offered', ?, ?, ?)`,
        )
        .bind(
          input.offerId,
          input.workRequestId,
          input.quoteRef,
          input.providerActorRef,
          providerPubkey,
          input.amountSats,
          amountMsats,
          JSON.stringify(capabilityRefs),
          relayEventRef,
          JSON.stringify(projection),
          nowIso,
          nowIso,
        )
        .run(),
    )

    const recorded = yield* readForumWorkRequestOfferByQuoteRef(
      db,
      input.workRequestId,
      input.quoteRef,
    )

    if (recorded === null) {
      return yield* new ForumValidationError({
        reason: 'Forum work request offer was not persisted.',
      })
    }

    return recorded
  })

export const listForumWorkRequestOffers = (
  db: D1Database,
  workRequestId: string,
): Effect.Effect<ReadonlyArray<ForumWorkRequestOfferRecord>, ForumStorageError> =>
  d1Effect('forumWorkRequests.listOffers', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_offers
          WHERE work_request_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC, id DESC`,
      )
      .bind(workRequestId)
      .all<ForumWorkRequestOfferRow>(),
  ).pipe(
    Effect.map(result =>
      (result.results ?? []).map(offerFromRow),
    ),
  )

export const readForumWorkRequestOfferByQuoteRef = (
  db: D1Database,
  workRequestId: string,
  quoteRef: string,
): Effect.Effect<ForumWorkRequestOfferRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readOfferByQuoteRef', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_offers
          WHERE work_request_id = ?
            AND quote_ref = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workRequestId, quoteRef)
      .first<ForumWorkRequestOfferRow>(),
  ).pipe(Effect.map(row => (row === null ? null : offerFromRow(row))))

export const listForumWorkRequestResults = (
  db: D1Database,
  workRequestId: string,
): Effect.Effect<ReadonlyArray<ForumWorkRequestResultRecord>, ForumStorageError> =>
  d1Effect('forumWorkRequests.listResults', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_results
          WHERE work_request_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC, id DESC`,
      )
      .bind(workRequestId)
      .all<ForumWorkRequestResultRow>(),
  ).pipe(Effect.map(result => (result.results ?? []).map(resultFromRow)))

export const readForumWorkRequestResultByQuoteRef = (
  db: D1Database,
  workRequestId: string,
  quoteRef: string,
): Effect.Effect<ForumWorkRequestResultRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readResultByQuoteRef', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_results
          WHERE work_request_id = ?
            AND quote_ref = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workRequestId, quoteRef)
      .first<ForumWorkRequestResultRow>(),
  ).pipe(Effect.map(row => (row === null ? null : resultFromRow(row))))

export const recordForumWorkRequestResult = (
  db: D1Database,
  input: ForumWorkRequestResultInput,
  nowIso: string,
): Effect.Effect<
  ForumWorkRequestResultRecord,
  ForumStorageError | ForumValidationError
> =>
  Effect.gen(function* () {
    const { artifactRefs, closeoutRef, projection } = yield* Effect.try({
      catch: validationError,
      try: () => {
        const artifactRefs = normalizeResultArtifactRefs(input.artifactRefs)
        const closeoutRefRaw = input.closeoutRef?.trim() ?? ''
        const closeoutRef = closeoutRefRaw.length === 0 ? null : closeoutRefRaw
        const resultEventRef = input.resultEventRef.trim()
        const verificationCommandRef = input.verificationCommandRef.trim()

        if (resultEventRef.length === 0) {
          throw new ForumWorkRequestNegotiationUnsafe(
            'resultEventRef is required.',
          )
        }

        if (verificationCommandRef.length === 0) {
          throw new ForumWorkRequestNegotiationUnsafe(
            'verificationCommandRef is required.',
          )
        }

        assertPublicSafe(
          { closeoutRef, resultEventRef, verificationCommandRef },
          'work request result refs',
        )

        return {
          artifactRefs,
          closeoutRef,
          projection: resultProjection({
            artifactRefs,
            closeoutRef,
            offerId: input.offerId,
            providerActorRef: input.providerActorRef,
            quoteRef: input.quoteRef,
            resultEventRef,
            resultId: input.resultId,
            verificationCommandRef,
            workRequestId: input.workRequestId,
          }),
        }
      },
    })

    yield* d1Effect('forumWorkRequests.insertResult', () =>
      db
        .prepare(
          `INSERT INTO forum_work_request_results (
             id,
             work_request_id,
             offer_id,
             quote_ref,
             provider_actor_ref,
             result_event_ref,
             verification_command_ref,
             artifact_refs_json,
             closeout_ref,
             public_projection_json,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.resultId,
          input.workRequestId,
          input.offerId,
          input.quoteRef,
          input.providerActorRef,
          input.resultEventRef.trim(),
          input.verificationCommandRef.trim(),
          JSON.stringify(artifactRefs),
          closeoutRef,
          JSON.stringify(projection),
          nowIso,
        )
        .run(),
    )

    const recorded = yield* readForumWorkRequestResultByQuoteRef(
      db,
      input.workRequestId,
      input.quoteRef,
    )

    if (recorded === null) {
      return yield* new ForumValidationError({
        reason: 'Forum work request result was not persisted.',
      })
    }

    return recorded
  })

// Advances a work request to the terminal `settled` state after its escrow has
// released to the provider. Bounded to a request whose escrow was accepted, so
// it is safe to call from the release route; a no-op if already settled.
export const markForumWorkRequestSettled = (
  db: D1Database,
  workRequestId: string,
  nowIso: string,
): Effect.Effect<void, ForumStorageError> =>
  d1Effect('forumWorkRequests.markWorkRequestSettled', () =>
    db
      .prepare(
        `UPDATE forum_work_requests
            SET state = 'settled',
                updated_at = ?
          WHERE id = ?
            AND state IN ('quote_accepted', 'running', 'delivered', 'accepted')
            AND archived_at IS NULL`,
      )
      .bind(nowIso, workRequestId)
      .run(),
  ).pipe(Effect.asVoid)

export const readForumWorkRequestAcceptanceByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumWorkRequestAcceptanceRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readAcceptanceByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_acceptances
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumWorkRequestAcceptanceRow>(),
  ).pipe(Effect.map(row => (row === null ? null : acceptanceFromRow(row))))

export const readForumWorkRequestAcceptanceByWorkRequestId = (
  db: D1Database,
  workRequestId: string,
): Effect.Effect<ForumWorkRequestAcceptanceRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readAcceptanceByWorkRequestId', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_acceptances
          WHERE work_request_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workRequestId)
      .first<ForumWorkRequestAcceptanceRow>(),
  ).pipe(Effect.map(row => (row === null ? null : acceptanceFromRow(row))))

export const recordForumWorkRequestAcceptance = (
  db: D1Database,
  input: ForumWorkRequestAcceptanceInput,
): Effect.Effect<ForumWorkRequestAcceptanceRecord, ForumStorageError | ForumValidationError> =>
  Effect.gen(function* () {
    const projection = yield* Effect.try({
      catch: validationError,
      try: () => {
        assertPublicSafe(input, 'work request acceptance')
        return acceptanceProjection(input)
      },
    })

    yield* d1Effect('forumWorkRequests.insertAcceptance', () =>
      db
        .prepare(
          `INSERT INTO forum_work_request_acceptances (
             id,
             idempotency_key,
             work_request_id,
             offer_id,
             quote_ref,
             requester_actor_ref,
             provider_actor_ref,
             amount_msats,
             escrow_id,
             reserve_receipt_ref,
             acceptance_event_ref,
             public_projection_json,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.acceptanceId,
          input.idempotencyKey,
          input.workRequestId,
          input.offerId,
          input.quoteRef,
          input.requesterActorRef,
          input.providerActorRef,
          input.amountMsats,
          input.escrowId,
          input.reserveReceiptRef,
          input.acceptanceEventRef,
          JSON.stringify(projection),
          input.nowIso,
        )
        .run(),
    )

    yield* d1Effect('forumWorkRequests.markAcceptedOffer', () =>
      db
        .prepare(
          `UPDATE forum_work_request_offers
              SET state = CASE WHEN quote_ref = ? THEN 'accepted' ELSE 'rejected' END,
                  updated_at = ?
            WHERE work_request_id = ?
              AND state = 'offered'
              AND archived_at IS NULL`,
        )
        .bind(input.quoteRef, input.nowIso, input.workRequestId)
        .run(),
    )

    yield* d1Effect('forumWorkRequests.markWorkRequestQuoteAccepted', () =>
      db
        .prepare(
          `UPDATE forum_work_requests
              SET state = 'quote_accepted',
                  updated_at = ?
            WHERE id = ?
              AND state IN ('open', 'quote_received')
              AND archived_at IS NULL`,
        )
        .bind(input.nowIso, input.workRequestId)
        .run(),
    )

    const recorded = yield* readForumWorkRequestAcceptanceByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (recorded === null) {
      return yield* new ForumValidationError({
        reason: 'Forum work request acceptance was not persisted.',
      })
    }

    return recorded
  })
