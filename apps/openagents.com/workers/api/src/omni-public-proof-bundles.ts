import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const OmniPublicProofBundleStatus = S.Literals([
  'draft',
  'ready',
  'blocked',
  'superseded',
  'archived',
])
export type OmniPublicProofBundleStatus =
  typeof OmniPublicProofBundleStatus.Type

export const OmniPublicProofBundleRecord = S.Struct({
  acceptanceStateRef: S.String,
  archivedAt: S.NullOr(S.String),
  artifactRefs: S.Array(S.String),
  createdAt: S.String,
  economicsCaveatRef: S.String,
  id: S.String,
  idempotencyKey: S.String,
  legalCaveatRef: S.NullOr(S.String),
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  noSettlementImplication: S.Boolean,
  privacyCaveatRef: S.String,
  publicReceiptRef: S.String,
  receiptRefs: S.Array(S.String),
  reviewStateRef: S.String,
  sourceRefs: S.Array(S.String),
  status: OmniPublicProofBundleStatus,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniPublicProofBundleRecord =
  typeof OmniPublicProofBundleRecord.Type

export type OmniPublicProofBundlesRuntime = Readonly<{
  makeProofBundleId: () => string
  nowIso: () => string
}>

export const systemOmniPublicProofBundlesRuntime: OmniPublicProofBundlesRuntime =
  {
    makeProofBundleId: () => compactRandomId('omni_public_proof_bundle'),
    nowIso: currentIsoTimestamp,
  }

export type CreateOmniPublicProofBundleInput = Readonly<{
  acceptanceStateRef: string
  artifactRefs?: ReadonlyArray<string> | undefined
  economicsCaveatRef: string
  id?: string | undefined
  idempotencyKey: string
  legalCaveatRef?: string | undefined
  legalSensitive?: boolean | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  privacyCaveatRef: string
  publicReceiptRef?: string | undefined
  receiptRefs?: ReadonlyArray<string> | undefined
  reviewStateRef: string
  sourceRefs?: ReadonlyArray<string> | undefined
  status?: OmniPublicProofBundleStatus | undefined
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

export type ProofBundleRow = Readonly<{
  acceptance_state_ref: string
  archived_at: string | null
  artifact_refs_json: string
  created_at: string
  economics_caveat_ref: string
  id: string
  idempotency_key: string
  legal_caveat_ref: string | null
  legal_sensitive: number
  metadata_json: string
  no_settlement_implication: number
  privacy_caveat_ref: string
  public_receipt_ref: string
  receipt_refs_json: string
  review_state_ref: string
  source_refs_json: string
  status: OmniPublicProofBundleStatus
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniPublicProofBundleValidationError extends S.TaggedErrorClass<OmniPublicProofBundleValidationError>()(
  'OmniPublicProofBundleValidationError',
  { reason: S.String },
) {}

export class OmniPublicProofBundleStorageError extends S.TaggedErrorClass<OmniPublicProofBundleStorageError>()(
  'OmniPublicProofBundleStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniPublicProofBundleWorkroomNotFound extends S.TaggedErrorClass<OmniPublicProofBundleWorkroomNotFound>()(
  'OmniPublicProofBundleWorkroomNotFound',
  { workroomId: S.String },
) {}

export type OmniPublicProofBundleError =
  | OmniPublicProofBundleStorageError
  | OmniPublicProofBundleValidationError
  | OmniPublicProofBundleWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'customer_email',
  'payment_settled',
  'provider_payload',
  'raw_email',
  'raw_run_log',
  'settlement',
  'payout',
]

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_TEXT_PATTERN.test(value) &&
  !PROHIBITED_FRAGMENTS.some(fragment =>
    value.toLowerCase().includes(fragment),
  )

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniPublicProofBundleValidationError({
      reason: `${field} must be a public-safe proof ref without raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): void => {
  ;[...(values ?? [])].forEach(value => assertSafeRef(field, value))
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_TEXT_PATTERN.test(json) ||
    PROHIBITED_FRAGMENTS.some(fragment => json.toLowerCase().includes(fragment))
  ) {
    throw new OmniPublicProofBundleValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.',
    })
  }
}

const assertValidInput = (input: CreateOmniPublicProofBundleInput): void => {
  const legalSensitive =
    input.legalSensitive === true || input.workKind === 'legal_sensitive'

  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef('reviewStateRef', input.reviewStateRef)
  assertSafeRef('acceptanceStateRef', input.acceptanceStateRef)
  assertSafeRef('economicsCaveatRef', input.economicsCaveatRef)
  assertSafeRef('legalCaveatRef', input.legalCaveatRef)
  assertSafeRef('privacyCaveatRef', input.privacyCaveatRef)
  assertSafeRef('publicReceiptRef', input.publicReceiptRef)
  assertSafeRefs('sourceRefs', input.sourceRefs)
  assertSafeRefs('artifactRefs', input.artifactRefs)
  assertSafeRefs('receiptRefs', input.receiptRefs)
  assertSafeMetadata(input.metadata)

  if (legalSensitive && input.legalCaveatRef === undefined) {
    throw new OmniPublicProofBundleValidationError({
      reason: 'legal-sensitive proof bundles must include legalCaveatRef.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): OmniPublicProofBundleStorageError =>
  new OmniPublicProofBundleStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniPublicProofBundleStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

/**
 * KS-8.17 read-cutover follow-up (#8361): exported (not just module-private)
 * so `supervision-longtail-domain-store.ts`'s bounded real-Postgres-serve
 * reader can map a Postgres row (same-named twin, byte-identical column
 * shapes — see khala-sync migration `0024_supervision_longtail.sql`) through
 * the SAME conversion the D1-served path uses, rather than duplicating it.
 */
export const rowToRecord = (row: ProofBundleRow): OmniPublicProofBundleRecord => ({
  acceptanceStateRef: row.acceptance_state_ref,
  archivedAt: row.archived_at,
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  createdAt: row.created_at,
  economicsCaveatRef: row.economics_caveat_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalCaveatRef: row.legal_caveat_ref,
  legalSensitive: row.legal_sensitive === 1,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  noSettlementImplication: row.no_settlement_implication === 1,
  privacyCaveatRef: row.privacy_caveat_ref,
  publicReceiptRef: row.public_receipt_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  reviewStateRef: row.review_state_ref,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  status: row.status,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const publicReceiptRef = (workroomId: string, idempotencyKey: string): string =>
  `omni_public_proof_bundle:${workroomId}:${idempotencyKey}`

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<OmniPublicProofBundleRecord | null, OmniPublicProofBundleStorageError> =>
  d1Effect('omniPublicProofBundles.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_public_proof_bundles
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ProofBundleRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToRecord(row))))

const readWorkroom = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniPublicProofBundleStorageError> =>
  d1Effect('omniPublicProofBundles.workroom', () =>
    db
      .prepare(
        `SELECT id, work_kind, archived_at
           FROM omni_workrooms
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workroomId)
      .first<WorkroomRefRow>(),
  )

export const readOmniPublicProofBundleById = async (
  db: D1Database,
  id: string,
): Promise<OmniPublicProofBundleRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM omni_public_proof_bundles
        WHERE id = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(id)
    .first<ProofBundleRow>()
  return row === null ? null : rowToRecord(row)
}

export const createOmniPublicProofBundle = (
  db: D1Database,
  input: CreateOmniPublicProofBundleInput,
  runtime: OmniPublicProofBundlesRuntime = systemOmniPublicProofBundlesRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<OmniPublicProofBundleRecord, OmniPublicProofBundleError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroom(db, input.workroomId)

    if (workroom === null) {
      return yield* new OmniPublicProofBundleWorkroomNotFound({
        workroomId: input.workroomId,
      })
    }

    if (workroom.work_kind !== input.workKind) {
      return yield* new OmniPublicProofBundleValidationError({
        reason: 'proof bundle workKind must match the workroom workKind.',
      })
    }

    const now = runtime.nowIso()
    const legalSensitive =
      input.legalSensitive === true || input.workKind === 'legal_sensitive'
    const record: OmniPublicProofBundleRecord = {
      acceptanceStateRef: input.acceptanceStateRef,
      archivedAt: null,
      artifactRefs: [...(input.artifactRefs ?? [])],
      createdAt: now,
      economicsCaveatRef: input.economicsCaveatRef,
      id: input.id ?? runtime.makeProofBundleId(),
      idempotencyKey: input.idempotencyKey,
      legalCaveatRef: input.legalCaveatRef ?? null,
      legalSensitive,
      metadata: input.metadata ?? {},
      noSettlementImplication: true,
      privacyCaveatRef: input.privacyCaveatRef,
      publicReceiptRef:
        input.publicReceiptRef ??
        publicReceiptRef(input.workroomId, input.idempotencyKey),
      receiptRefs: [...(input.receiptRefs ?? [])],
      reviewStateRef: input.reviewStateRef,
      sourceRefs: [...(input.sourceRefs ?? [])],
      status: input.status ?? 'draft',
      updatedAt: now,
      workKind: input.workKind,
      workroomId: input.workroomId,
    }

    yield* d1Effect('omniPublicProofBundles.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_public_proof_bundles
             (id,
              idempotency_key,
              workroom_id,
              work_kind,
              status,
              legal_sensitive,
              source_refs_json,
              artifact_refs_json,
              receipt_refs_json,
              review_state_ref,
              acceptance_state_ref,
              economics_caveat_ref,
              legal_caveat_ref,
              privacy_caveat_ref,
              public_receipt_ref,
              no_settlement_implication,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workroomId,
          record.workKind,
          record.status,
          record.legalSensitive ? 1 : 0,
          JSON.stringify(record.sourceRefs),
          JSON.stringify(record.artifactRefs),
          JSON.stringify(record.receiptRefs),
          record.reviewStateRef,
          record.acceptanceStateRef,
          record.economicsCaveatRef,
          record.legalCaveatRef,
          record.privacyCaveatRef,
          record.publicReceiptRef,
          record.noSettlementImplication ? 1 : 0,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
        )
        .run()
        .then(() => undefined),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('omni_public_proof_bundles', [[record.id]]),
      )
    }

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniProofBundleProjection = (
  bundle: OmniPublicProofBundleRecord,
) => ({
  acceptanceStateRef: bundle.acceptanceStateRef,
  artifactRefs: bundle.artifactRefs,
  economicsCaveatRef: bundle.economicsCaveatRef,
  legalCaveatRef: bundle.legalCaveatRef,
  noSettlementImplication: bundle.noSettlementImplication,
  privacyCaveatRef: bundle.privacyCaveatRef,
  publicReceiptRef: bundle.publicReceiptRef,
  receiptRefs: bundle.receiptRefs,
  reviewStateRef: bundle.reviewStateRef,
  sourceRefs: bundle.sourceRefs,
  status: bundle.status,
  workKind: bundle.workKind,
  workroomId: bundle.workroomId,
})

export const operatorOmniProofBundleProjection = (
  bundle: OmniPublicProofBundleRecord,
) => bundle
