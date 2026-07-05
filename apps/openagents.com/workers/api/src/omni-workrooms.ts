import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  type OmniDataClassification,
  OmniDataClassification as OmniDataClassificationSchema,
  OmniDataClassificationValidationError,
  type OmniTrustTier,
  OmniTrustTier as OmniTrustTierSchema,
  omniClassificationProjection,
} from './omni-data-classification'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const OmniWorkroomStatus = S.Literals([
  'queued',
  'active',
  'blocked',
  'waiting_review',
  'completed',
  'unavailable',
  'archived',
])
export type OmniWorkroomStatus = typeof OmniWorkroomStatus.Type

export const OmniWorkroomVisibility = S.Literals([
  'private',
  'customer',
  'team',
  'public',
])
export type OmniWorkroomVisibility = typeof OmniWorkroomVisibility.Type

export const OmniWorkroomRecord = S.Struct({
  acceptedOutcomeContractId: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
  artifactRefs: S.Array(S.String),
  assignmentId: S.NullOr(S.String),
  blockerRefs: S.Array(S.String),
  classificationCaveatRef: S.String,
  createdAt: S.String,
  customerIntentRef: S.String,
  dataClassification: OmniDataClassificationSchema,
  emailRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  publicReceiptRef: S.String,
  receiptRefs: S.Array(S.String),
  siteId: S.NullOr(S.String),
  softwareOrderId: S.String,
  sourceRefs: S.Array(S.String),
  status: OmniWorkroomStatus,
  taskPacketRef: S.NullOr(S.String),
  trustTier: OmniTrustTierSchema,
  updatedAt: S.String,
  visibility: OmniWorkroomVisibility,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
})
export type OmniWorkroomRecord = typeof OmniWorkroomRecord.Type

export type OmniWorkroomsRuntime = Readonly<{
  makeWorkroomId: () => string
  nowIso: () => string
}>

export const systemOmniWorkroomsRuntime: OmniWorkroomsRuntime = {
  makeWorkroomId: () => compactRandomId('omni_workroom'),
  nowIso: currentIsoTimestamp,
}

export type PromoteOmniWorkroomInput = Readonly<{
  acceptedOutcomeContractId?: string | undefined
  artifactRefs?: ReadonlyArray<string> | undefined
  assignmentId?: string | undefined
  blockerRefs?: ReadonlyArray<string> | undefined
  classificationCaveatRef?: string | undefined
  customerIntentRef: string
  dataClassification?: OmniDataClassification | undefined
  emailRefs?: ReadonlyArray<string> | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  publicReceiptRef?: string | undefined
  receiptRefs?: ReadonlyArray<string> | undefined
  siteId?: string | undefined
  softwareOrderId: string
  sourceRefs?: ReadonlyArray<string> | undefined
  status?: OmniWorkroomStatus | undefined
  taskPacketRef?: string | undefined
  trustTier?: OmniTrustTier | undefined
  visibility?: OmniWorkroomVisibility | undefined
  workKind: OmniAcceptedOutcomeWorkKind
}>

type ExistenceRow = Readonly<{ id: string }>
type WorkroomRow = Readonly<{
  accepted_outcome_contract_id: string | null
  archived_at: string | null
  artifact_refs_json: string
  assignment_id: string | null
  blocker_refs_json: string
  classification_caveat_ref: string
  created_at: string
  customer_intent_ref: string
  data_classification: OmniDataClassification
  email_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  public_receipt_ref: string
  receipt_refs_json: string
  site_id: string | null
  software_order_id: string
  source_refs_json: string
  status: OmniWorkroomStatus
  task_packet_ref: string | null
  trust_tier: OmniTrustTier
  updated_at: string
  visibility: OmniWorkroomVisibility
  work_kind: OmniAcceptedOutcomeWorkKind
}>

export class OmniWorkroomValidationError extends S.TaggedErrorClass<OmniWorkroomValidationError>()(
  'OmniWorkroomValidationError',
  { reason: S.String },
) {}

export class OmniWorkroomStorageError extends S.TaggedErrorClass<OmniWorkroomStorageError>()(
  'OmniWorkroomStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniWorkroomOrderNotFound extends S.TaggedErrorClass<OmniWorkroomOrderNotFound>()(
  'OmniWorkroomOrderNotFound',
  { softwareOrderId: S.String },
) {}

export class OmniWorkroomSiteNotFound extends S.TaggedErrorClass<OmniWorkroomSiteNotFound>()(
  'OmniWorkroomSiteNotFound',
  { siteId: S.String },
) {}

export class OmniWorkroomAssignmentNotFound extends S.TaggedErrorClass<OmniWorkroomAssignmentNotFound>()(
  'OmniWorkroomAssignmentNotFound',
  { assignmentId: S.String },
) {}

export class OmniWorkroomAcceptedOutcomeContractNotFound extends S.TaggedErrorClass<OmniWorkroomAcceptedOutcomeContractNotFound>()(
  'OmniWorkroomAcceptedOutcomeContractNotFound',
  { acceptedOutcomeContractId: S.String },
) {}

export type OmniWorkroomError =
  | OmniWorkroomAcceptedOutcomeContractNotFound
  | OmniWorkroomAssignmentNotFound
  | OmniWorkroomOrderNotFound
  | OmniWorkroomSiteNotFound
  | OmniWorkroomStorageError
  | OmniWorkroomValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniWorkroomValidationError({
      reason: `${field} must be a public-safe ref without raw provider, run log, email, payment, wallet, or private customer material.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): void => {
  ;[...(values ?? [])].forEach(value => {
    assertSafeRef(field, value)
  })
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
    PROHIBITED_TEXT_PATTERN.test(json)
  ) {
    throw new OmniWorkroomValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, wallet, or private customer material.',
    })
  }
}

const assertValidInput = (input: PromoteOmniWorkroomInput): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('softwareOrderId', input.softwareOrderId)
  assertSafeRef('acceptedOutcomeContractId', input.acceptedOutcomeContractId)
  assertSafeRef('siteId', input.siteId)
  assertSafeRef('assignmentId', input.assignmentId)
  assertSafeRef('customerIntentRef', input.customerIntentRef)
  assertSafeRef('taskPacketRef', input.taskPacketRef)
  assertSafeRef('publicReceiptRef', input.publicReceiptRef)
  assertSafeRef('classificationCaveatRef', input.classificationCaveatRef)
  assertSafeRefs('sourceRefs', input.sourceRefs)
  assertSafeRefs('artifactRefs', input.artifactRefs)
  assertSafeRefs('emailRefs', input.emailRefs)
  assertSafeRefs('receiptRefs', input.receiptRefs)
  assertSafeRefs('blockerRefs', input.blockerRefs)
  assertSafeMetadata(input.metadata)

  if (input.workKind === 'site' && input.siteId === undefined) {
    throw new OmniWorkroomValidationError({
      reason: 'site workrooms must include siteId.',
    })
  }

  if (
    input.workKind === 'legal_sensitive' &&
    !['legal_sensitive', 'private', 'secret_bearing'].includes(
      input.dataClassification ?? 'legal_sensitive',
    )
  ) {
    throw new OmniWorkroomValidationError({
      reason:
        'legal_sensitive workrooms must use legal_sensitive, private, or secret_bearing classification.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): OmniWorkroomStorageError =>
  new OmniWorkroomStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniWorkroomStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const workroomFromRow = (row: WorkroomRow): OmniWorkroomRecord => ({
  acceptedOutcomeContractId: row.accepted_outcome_contract_id,
  archivedAt: row.archived_at,
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  assignmentId: row.assignment_id,
  blockerRefs: parseJsonStringArray(row.blocker_refs_json),
  classificationCaveatRef: row.classification_caveat_ref,
  createdAt: row.created_at,
  customerIntentRef: row.customer_intent_ref,
  dataClassification: row.data_classification,
  emailRefs: parseJsonStringArray(row.email_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  publicReceiptRef: row.public_receipt_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  status: row.status,
  taskPacketRef: row.task_packet_ref,
  trustTier: row.trust_tier,
  updatedAt: row.updated_at,
  visibility: row.visibility,
  workKind: row.work_kind,
})

const publicReceiptRef = (
  softwareOrderId: string,
  idempotencyKey: string,
): string => `omni_workroom:${softwareOrderId}:${idempotencyKey}`

const defaultDataClassification = (
  input: PromoteOmniWorkroomInput,
): OmniDataClassification => {
  if (input.dataClassification !== undefined) {
    return input.dataClassification
  }

  return input.workKind === 'legal_sensitive' ? 'legal_sensitive' : 'customer'
}

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<OmniWorkroomRecord | null, OmniWorkroomStorageError> =>
  d1Effect('omniWorkrooms.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_workrooms
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<WorkroomRow>(),
  ).pipe(Effect.map(row => (row === null ? null : workroomFromRow(row))))

const readExistingRef = (
  db: D1Database,
  operation: string,
  table: string,
  id: string,
): Effect.Effect<ExistenceRow | null, OmniWorkroomStorageError> =>
  d1Effect(operation, () =>
    db
      .prepare(
        `SELECT id
           FROM ${table}
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(id)
      .first<ExistenceRow>(),
  )

export const promoteOmniWorkroom = (
  db: D1Database,
  input: PromoteOmniWorkroomInput,
  runtime: OmniWorkroomsRuntime = systemOmniWorkroomsRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<OmniWorkroomRecord, OmniWorkroomError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const order = yield* readExistingRef(
      db,
      'omniWorkrooms.softwareOrder',
      'software_orders',
      input.softwareOrderId,
    )

    if (order === null) {
      return yield* new OmniWorkroomOrderNotFound({
        softwareOrderId: input.softwareOrderId,
      })
    }

    if (input.siteId !== undefined) {
      const site = yield* readExistingRef(
        db,
        'omniWorkrooms.site',
        'site_projects',
        input.siteId,
      )

      if (site === null) {
        return yield* new OmniWorkroomSiteNotFound({ siteId: input.siteId })
      }
    }

    if (input.assignmentId !== undefined) {
      const assignment = yield* readExistingRef(
        db,
        'omniWorkrooms.assignment',
        'adjutant_assignments',
        input.assignmentId,
      )

      if (assignment === null) {
        return yield* new OmniWorkroomAssignmentNotFound({
          assignmentId: input.assignmentId,
        })
      }
    }

    if (input.acceptedOutcomeContractId !== undefined) {
      const contract = yield* readExistingRef(
        db,
        'omniWorkrooms.acceptedOutcomeContract',
        'omni_accepted_outcome_contracts',
        input.acceptedOutcomeContractId,
      )

      if (contract === null) {
        return yield* new OmniWorkroomAcceptedOutcomeContractNotFound({
          acceptedOutcomeContractId: input.acceptedOutcomeContractId,
        })
      }
    }

    const now = runtime.nowIso()
    const record: OmniWorkroomRecord = {
      acceptedOutcomeContractId: input.acceptedOutcomeContractId ?? null,
      archivedAt: null,
      artifactRefs: [...(input.artifactRefs ?? [])],
      assignmentId: input.assignmentId ?? null,
      blockerRefs: [...(input.blockerRefs ?? [])],
      classificationCaveatRef:
        input.classificationCaveatRef ?? 'classification_caveat_unreviewed',
      createdAt: now,
      customerIntentRef: input.customerIntentRef,
      dataClassification: defaultDataClassification(input),
      emailRefs: [...(input.emailRefs ?? [])],
      id: input.id ?? runtime.makeWorkroomId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      publicReceiptRef:
        input.publicReceiptRef ??
        publicReceiptRef(input.softwareOrderId, input.idempotencyKey),
      receiptRefs: [...(input.receiptRefs ?? [])],
      siteId: input.siteId ?? null,
      softwareOrderId: input.softwareOrderId,
      sourceRefs: [...(input.sourceRefs ?? [])],
      status: input.status ?? 'queued',
      taskPacketRef: input.taskPacketRef ?? null,
      trustTier: input.trustTier ?? 'unverified',
      updatedAt: now,
      visibility: input.visibility ?? 'customer',
      workKind: input.workKind,
    }

    yield* d1Effect('omniWorkrooms.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_workrooms
             (id,
              idempotency_key,
              software_order_id,
              accepted_outcome_contract_id,
              site_id,
              assignment_id,
              work_kind,
              status,
              visibility,
              customer_intent_ref,
              task_packet_ref,
              source_refs_json,
              artifact_refs_json,
              email_refs_json,
              receipt_refs_json,
              blocker_refs_json,
              data_classification,
              trust_tier,
              classification_caveat_ref,
              public_receipt_ref,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.softwareOrderId,
          record.acceptedOutcomeContractId,
          record.siteId,
          record.assignmentId,
          record.workKind,
          record.status,
          record.visibility,
          record.customerIntentRef,
          record.taskPacketRef,
          JSON.stringify(record.sourceRefs),
          JSON.stringify(record.artifactRefs),
          JSON.stringify(record.emailRefs),
          JSON.stringify(record.receiptRefs),
          JSON.stringify(record.blockerRefs),
          record.dataClassification,
          record.trustTier,
          record.classificationCaveatRef,
          record.publicReceiptRef,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
        )
        .run()
        .then(() => undefined),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('omni_workrooms', [[record.id]]),
      )
    }

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniWorkroomProjection = (workroom: OmniWorkroomRecord) => {
  const classification = omniClassificationProjection(workroom, 'public')

  return {
    ...classification,
    publicReceiptRef: workroom.publicReceiptRef,
    siteId: workroom.siteId,
    softwareOrderId: workroom.softwareOrderId,
    status: workroom.status,
    visibility: workroom.visibility,
    workKind: workroom.workKind,
  }
}

export const customerOmniWorkroomProjection = (
  workroom: OmniWorkroomRecord,
) => {
  const classification = omniClassificationProjection(workroom, 'customer')

  return {
    ...classification,
    artifactRefs: workroom.artifactRefs,
    blockerRefs: workroom.blockerRefs,
    customerIntentRef: workroom.customerIntentRef,
    emailRefs: workroom.emailRefs,
    publicReceiptRef: workroom.publicReceiptRef,
    receiptRefs: workroom.receiptRefs,
    siteId: workroom.siteId,
    softwareOrderId: workroom.softwareOrderId,
    status: workroom.status,
    workKind: workroom.workKind,
  }
}

export const operatorOmniWorkroomProjection = (
  workroom: OmniWorkroomRecord,
) => {
  try {
    const classification = omniClassificationProjection(workroom, 'operator')

    return {
      ...classification,
      acceptedOutcomeContractId: workroom.acceptedOutcomeContractId,
      artifactRefs: workroom.artifactRefs,
      assignmentId: workroom.assignmentId,
      blockerRefs: workroom.blockerRefs,
      customerIntentRef: workroom.customerIntentRef,
      emailRefs: workroom.emailRefs,
      id: workroom.id,
      publicReceiptRef: workroom.publicReceiptRef,
      receiptRefs: workroom.receiptRefs,
      siteId: workroom.siteId,
      softwareOrderId: workroom.softwareOrderId,
      sourceRefs: workroom.sourceRefs,
      status: workroom.status,
      taskPacketRef: workroom.taskPacketRef,
      workKind: workroom.workKind,
    }
  } catch (error) {
    if (error instanceof OmniDataClassificationValidationError) {
      const classification = omniClassificationProjection(workroom, 'private')

      return {
        ...classification,
        acceptedOutcomeContractId: workroom.acceptedOutcomeContractId,
        artifactRefs: workroom.artifactRefs,
        assignmentId: workroom.assignmentId,
        blockerRefs: workroom.blockerRefs,
        customerIntentRef: workroom.customerIntentRef,
        emailRefs: workroom.emailRefs,
        id: workroom.id,
        publicReceiptRef: workroom.publicReceiptRef,
        receiptRefs: workroom.receiptRefs,
        siteId: workroom.siteId,
        softwareOrderId: workroom.softwareOrderId,
        sourceRefs: workroom.sourceRefs,
        status: workroom.status,
        taskPacketRef: workroom.taskPacketRef,
        workKind: workroom.workKind,
      }
    }

    throw error
  }
}
