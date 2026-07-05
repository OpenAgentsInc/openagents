import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonWithSchema } from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const OmniEvidenceBundleStatus = S.Literals([
  'draft',
  'ready',
  'redaction_required',
  'superseded',
  'archived',
])
export type OmniEvidenceBundleStatus =
  typeof OmniEvidenceBundleStatus.Type

export const OmniEvidenceEntryKind = S.Literals([
  'exa_source_card',
  'research_brief',
  'source_commit',
  'generated_source',
  'build_log',
  'screenshot',
  'deployment_url',
  'diff',
  'test_report',
  'email_receipt',
  'receipt',
  'redaction_report',
])
export type OmniEvidenceEntryKind = typeof OmniEvidenceEntryKind.Type

export const OmniEvidenceEntryVisibility = S.Literals([
  'private',
  'team',
  'customer',
  'public',
])
export type OmniEvidenceEntryVisibility =
  typeof OmniEvidenceEntryVisibility.Type

export const OmniEvidenceEntryRedactionState = S.Literals([
  'not_needed',
  'redacted',
  'private_only',
  'blocked',
])
export type OmniEvidenceEntryRedactionState =
  typeof OmniEvidenceEntryRedactionState.Type

export const OmniEvidenceSourceAuthority = S.Literals([
  'agent_generated',
  'customer_supplied',
  'operator_reviewed',
  'public_web',
  'github',
  'system_receipt',
])
export type OmniEvidenceSourceAuthority =
  typeof OmniEvidenceSourceAuthority.Type

export const OmniEvidenceBundleEntry = S.Struct({
  caveatRef: S.NullOr(S.String),
  entryKind: OmniEvidenceEntryKind,
  publicSafe: S.Boolean,
  redactionState: OmniEvidenceEntryRedactionState,
  ref: S.String,
  required: S.Boolean,
  sourceAuthority: OmniEvidenceSourceAuthority,
  summaryRef: S.String,
  visibility: OmniEvidenceEntryVisibility,
})
export type OmniEvidenceBundleEntry = typeof OmniEvidenceBundleEntry.Type

const OmniEvidenceBundleEntryArray = S.Array(OmniEvidenceBundleEntry)

export const OmniEvidenceBundleRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  entries: S.Array(OmniEvidenceBundleEntry),
  id: S.String,
  idempotencyKey: S.String,
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  publicReceiptRef: S.String,
  sourceAuthorityCaveatRef: S.NullOr(S.String),
  status: OmniEvidenceBundleStatus,
  summaryRef: S.String,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniEvidenceBundleRecord =
  typeof OmniEvidenceBundleRecord.Type

export type OmniEvidenceBundlesRuntime = Readonly<{
  makeBundleId: () => string
  nowIso: () => string
}>

export const systemOmniEvidenceBundlesRuntime: OmniEvidenceBundlesRuntime = {
  makeBundleId: () => compactRandomId('omni_evidence_bundle'),
  nowIso: currentIsoTimestamp,
}

export type CreateOmniEvidenceBundleInput = Readonly<{
  entries: ReadonlyArray<OmniEvidenceBundleEntry>
  id?: string | undefined
  idempotencyKey: string
  legalSensitive?: boolean | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  publicReceiptRef?: string | undefined
  sourceAuthorityCaveatRef?: string | undefined
  status?: OmniEvidenceBundleStatus | undefined
  summaryRef: string
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

type EvidenceBundleRow = Readonly<{
  archived_at: string | null
  created_at: string
  entries_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  public_receipt_ref: string
  source_authority_caveat_ref: string | null
  status: OmniEvidenceBundleStatus
  summary_ref: string
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniEvidenceBundleValidationError extends S.TaggedErrorClass<OmniEvidenceBundleValidationError>()(
  'OmniEvidenceBundleValidationError',
  { reason: S.String },
) {}

export class OmniEvidenceBundleStorageError extends S.TaggedErrorClass<OmniEvidenceBundleStorageError>()(
  'OmniEvidenceBundleStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniEvidenceBundleWorkroomNotFound extends S.TaggedErrorClass<OmniEvidenceBundleWorkroomNotFound>()(
  'OmniEvidenceBundleWorkroomNotFound',
  { workroomId: S.String },
) {}

export type OmniEvidenceBundleError =
  | OmniEvidenceBundleStorageError
  | OmniEvidenceBundleValidationError
  | OmniEvidenceBundleWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniEvidenceBundleValidationError({
      reason: `${field} must be a public-safe ref without raw provider, run log, email, payment, wallet, or private customer material.`,
    })
  }
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
    throw new OmniEvidenceBundleValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, wallet, or private customer material.',
    })
  }
}

const publicKinds = new Set<OmniEvidenceEntryKind>([
  'deployment_url',
  'screenshot',
  'source_commit',
  'research_brief',
  'redaction_report',
  'receipt',
])

const entryVisibleToCustomer = (entry: OmniEvidenceBundleEntry): boolean =>
  entry.publicSafe &&
  entry.redactionState !== 'private_only' &&
  entry.redactionState !== 'blocked' &&
  (entry.visibility === 'customer' || entry.visibility === 'public')

const entryVisibleToPublic = (entry: OmniEvidenceBundleEntry): boolean =>
  entryVisibleToCustomer(entry) &&
  entry.visibility === 'public' &&
  publicKinds.has(entry.entryKind)

const assertEntry = (
  entry: OmniEvidenceBundleEntry,
  legalSensitive: boolean,
): void => {
  assertSafeRef('entries.ref', entry.ref)
  assertSafeRef('entries.summaryRef', entry.summaryRef)
  assertSafeRef('entries.caveatRef', entry.caveatRef ?? undefined)

  if (entry.publicSafe && entry.redactionState === 'private_only') {
    throw new OmniEvidenceBundleValidationError({
      reason: 'publicSafe entries cannot use private_only redaction state.',
    })
  }

  if (!entry.publicSafe && entry.visibility === 'public') {
    throw new OmniEvidenceBundleValidationError({
      reason: 'public visibility requires publicSafe evidence entries.',
    })
  }

  if (
    legalSensitive &&
    (entry.visibility === 'public' || entry.visibility === 'customer') &&
    entry.caveatRef === null
  ) {
    throw new OmniEvidenceBundleValidationError({
      reason:
        'legal-sensitive public or customer evidence entries must include a caveatRef.',
    })
  }
}

const assertEntries = (
  entries: ReadonlyArray<OmniEvidenceBundleEntry>,
  legalSensitive: boolean,
): void => {
  if (entries.length === 0) {
    throw new OmniEvidenceBundleValidationError({
      reason: 'evidence bundles must include at least one entry.',
    })
  }

  entries.forEach(entry => assertEntry(entry, legalSensitive))

  if (
    legalSensitive &&
    entries.some(entry => entry.entryKind === 'redaction_report') === false
  ) {
    throw new OmniEvidenceBundleValidationError({
      reason: 'legal-sensitive evidence bundles require a redaction_report.',
    })
  }
}

const assertValidInput = (input: CreateOmniEvidenceBundleInput): void => {
  const legalSensitive =
    input.legalSensitive === true || input.workKind === 'legal_sensitive'

  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef('summaryRef', input.summaryRef)
  assertSafeRef('sourceAuthorityCaveatRef', input.sourceAuthorityCaveatRef)
  assertSafeRef('publicReceiptRef', input.publicReceiptRef)
  assertSafeMetadata(input.metadata)
  assertEntries(input.entries, legalSensitive)
}

const storageError = (
  operation: string,
  error: unknown,
): OmniEvidenceBundleStorageError =>
  new OmniEvidenceBundleStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniEvidenceBundleStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const bundleFromRow = (
  row: EvidenceBundleRow,
): OmniEvidenceBundleRecord => ({
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  entries: parseJsonWithSchema(OmniEvidenceBundleEntryArray, row.entries_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalSensitive: row.legal_sensitive === 1,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  publicReceiptRef: row.public_receipt_ref,
  sourceAuthorityCaveatRef: row.source_authority_caveat_ref,
  status: row.status,
  summaryRef: row.summary_ref,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const publicReceiptRef = (workroomId: string, idempotencyKey: string): string =>
  `omni_evidence_bundle:${workroomId}:${idempotencyKey}`

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<OmniEvidenceBundleRecord | null, OmniEvidenceBundleStorageError> =>
  d1Effect('omniEvidenceBundles.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_evidence_bundles
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<EvidenceBundleRow>(),
  ).pipe(
    Effect.map(row => (row === null ? null : bundleFromRow(row))),
  )

const readWorkroom = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniEvidenceBundleStorageError> =>
  d1Effect('omniEvidenceBundles.workroom', () =>
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

export const readOmniEvidenceBundleById = async (
  db: D1Database,
  id: string,
): Promise<OmniEvidenceBundleRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM omni_evidence_bundles
        WHERE id = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(id)
    .first<EvidenceBundleRow>()
  return row === null ? null : bundleFromRow(row)
}

export const createOmniEvidenceBundle = (
  db: D1Database,
  input: CreateOmniEvidenceBundleInput,
  runtime: OmniEvidenceBundlesRuntime = systemOmniEvidenceBundlesRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<OmniEvidenceBundleRecord, OmniEvidenceBundleError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroom(db, input.workroomId)

    if (workroom === null) {
      return yield* new OmniEvidenceBundleWorkroomNotFound({
        workroomId: input.workroomId,
      })
    }

    if (workroom.work_kind !== input.workKind) {
      return yield* new OmniEvidenceBundleValidationError({
        reason: 'evidence bundle workKind must match the workroom workKind.',
      })
    }

    const now = runtime.nowIso()
    const legalSensitive =
      input.legalSensitive === true || input.workKind === 'legal_sensitive'
    const record: OmniEvidenceBundleRecord = {
      archivedAt: null,
      createdAt: now,
      entries: [...input.entries],
      id: input.id ?? runtime.makeBundleId(),
      idempotencyKey: input.idempotencyKey,
      legalSensitive,
      metadata: input.metadata ?? {},
      publicReceiptRef:
        input.publicReceiptRef ??
        publicReceiptRef(input.workroomId, input.idempotencyKey),
      sourceAuthorityCaveatRef: input.sourceAuthorityCaveatRef ?? null,
      status: input.status ?? 'draft',
      summaryRef: input.summaryRef,
      updatedAt: now,
      workKind: input.workKind,
      workroomId: input.workroomId,
    }

    yield* d1Effect('omniEvidenceBundles.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_evidence_bundles
             (id,
              idempotency_key,
              workroom_id,
              work_kind,
              status,
              legal_sensitive,
              summary_ref,
              source_authority_caveat_ref,
              entries_json,
              public_receipt_ref,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workroomId,
          record.workKind,
          record.status,
          record.legalSensitive ? 1 : 0,
          record.summaryRef,
          record.sourceAuthorityCaveatRef,
          JSON.stringify(record.entries),
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
        mirror.mirrorRowsByKey('omni_evidence_bundles', [[record.id]]),
      )
    }

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniEvidenceBundleProjection = (
  bundle: OmniEvidenceBundleRecord,
) => ({
  entries: bundle.entries
    .filter(entryVisibleToPublic)
    .map(({ entryKind, ref, sourceAuthority, summaryRef }) => ({
      entryKind,
      ref,
      sourceAuthority,
      summaryRef,
    })),
  publicReceiptRef: bundle.publicReceiptRef,
  sourceAuthorityCaveatRef: bundle.sourceAuthorityCaveatRef,
  status: bundle.status,
  summaryRef: bundle.summaryRef,
  workKind: bundle.workKind,
  workroomId: bundle.workroomId,
})

export const customerOmniEvidenceBundleProjection = (
  bundle: OmniEvidenceBundleRecord,
) => ({
  ...publicOmniEvidenceBundleProjection(bundle),
  entries: bundle.entries
    .filter(entryVisibleToCustomer)
    .map(
      ({
        caveatRef,
        entryKind,
        redactionState,
        ref,
        sourceAuthority,
        summaryRef,
      }) => ({
        caveatRef,
        entryKind,
        redactionState,
        ref,
        sourceAuthority,
        summaryRef,
      }),
    ),
})

export const operatorOmniEvidenceBundleProjection = (
  bundle: OmniEvidenceBundleRecord,
) => ({
  ...customerOmniEvidenceBundleProjection(bundle),
  entries: bundle.entries,
  id: bundle.id,
  legalSensitive: bundle.legalSensitive,
  metadata: bundle.metadata,
})
