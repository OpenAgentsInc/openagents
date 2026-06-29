import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const OmniWorkroomLifecycleActorKind = S.Literals([
  'customer',
  'operator',
  'system',
])
export type OmniWorkroomLifecycleActorKind =
  typeof OmniWorkroomLifecycleActorKind.Type

export const OmniWorkroomLifecycleDecisionKind = S.Literals([
  'accept',
  'reject',
  'provisionally_accept',
  'reopen',
  'request_revision',
  'mark_unavailable',
])
export type OmniWorkroomLifecycleDecisionKind =
  typeof OmniWorkroomLifecycleDecisionKind.Type

export const OmniWorkroomLifecycleState = S.Literals([
  'accepted',
  'rejected',
  'provisionally_accepted',
  'reopened',
  'revision_requested',
  'unavailable',
])
export type OmniWorkroomLifecycleState =
  typeof OmniWorkroomLifecycleState.Type

export const OmniWorkroomLifecycleDecisionRecord = S.Struct({
  actorKind: OmniWorkroomLifecycleActorKind,
  archivedAt: S.NullOr(S.String),
  artifactRef: S.NullOr(S.String),
  createdAt: S.String,
  customerSafeExplanationRef: S.String,
  decisionKind: OmniWorkroomLifecycleDecisionKind,
  followupRequestRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  noSettlementImplication: S.Boolean,
  receiptRef: S.String,
  resultingState: OmniWorkroomLifecycleState,
  siteRevisionFeedbackRef: S.NullOr(S.String),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniWorkroomLifecycleDecisionRecord =
  typeof OmniWorkroomLifecycleDecisionRecord.Type

export type OmniWorkroomLifecycleRuntime = Readonly<{
  makeDecisionId: () => string
  nowIso: () => string
}>

export const systemOmniWorkroomLifecycleRuntime: OmniWorkroomLifecycleRuntime =
  {
    makeDecisionId: () => compactRandomId('omni_workroom_lifecycle_decision'),
    nowIso: currentIsoTimestamp,
  }

export type RecordOmniWorkroomLifecycleDecisionInput = Readonly<{
  actorKind: OmniWorkroomLifecycleActorKind
  artifactRef?: string | undefined
  customerSafeExplanationRef: string
  decisionKind: OmniWorkroomLifecycleDecisionKind
  followupRequestRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  receiptRef: string
  siteRevisionFeedbackRef?: string | undefined
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

type LifecycleDecisionRow = Readonly<{
  actor_kind: OmniWorkroomLifecycleActorKind
  archived_at: string | null
  artifact_ref: string | null
  created_at: string
  customer_safe_explanation_ref: string
  decision_kind: OmniWorkroomLifecycleDecisionKind
  followup_request_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  no_settlement_implication: number
  receipt_ref: string
  resulting_state: OmniWorkroomLifecycleState
  site_revision_feedback_ref: string | null
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniWorkroomLifecycleValidationError extends S.TaggedErrorClass<OmniWorkroomLifecycleValidationError>()(
  'OmniWorkroomLifecycleValidationError',
  { reason: S.String },
) {}

export class OmniWorkroomLifecycleStorageError extends S.TaggedErrorClass<OmniWorkroomLifecycleStorageError>()(
  'OmniWorkroomLifecycleStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniWorkroomLifecycleWorkroomNotFound extends S.TaggedErrorClass<OmniWorkroomLifecycleWorkroomNotFound>()(
  'OmniWorkroomLifecycleWorkroomNotFound',
  { workroomId: S.String },
) {}

export type OmniWorkroomLifecycleError =
  | OmniWorkroomLifecycleStorageError
  | OmniWorkroomLifecycleValidationError
  | OmniWorkroomLifecycleWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|settlement|payout|paid[_ -]?out|payment[_ -]?settled|eligible[_ -]?for[_ -]?payout|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'eligible_for_payout',
  'paid_out',
  'payment_settled',
  'payout',
  'settlement',
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
    throw new OmniWorkroomLifecycleValidationError({
      reason: `${field} must be a customer-safe ref without raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.`,
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
    throw new OmniWorkroomLifecycleValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.',
    })
  }
}

const stateFromDecision = (
  decisionKind: OmniWorkroomLifecycleDecisionKind,
): OmniWorkroomLifecycleState => {
  switch (decisionKind) {
    case 'accept':
      return 'accepted'
    case 'reject':
      return 'rejected'
    case 'provisionally_accept':
      return 'provisionally_accepted'
    case 'reopen':
      return 'reopened'
    case 'request_revision':
      return 'revision_requested'
    case 'mark_unavailable':
      return 'unavailable'
  }
}

const assertValidInput = (
  input: RecordOmniWorkroomLifecycleDecisionInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef(
    'customerSafeExplanationRef',
    input.customerSafeExplanationRef,
  )
  assertSafeRef('receiptRef', input.receiptRef)
  assertSafeRef('siteRevisionFeedbackRef', input.siteRevisionFeedbackRef)
  assertSafeRef('followupRequestRef', input.followupRequestRef)
  assertSafeRef('artifactRef', input.artifactRef)
  assertSafeMetadata(input.metadata)

  if (
    input.decisionKind === 'request_revision' &&
    input.workKind === 'site' &&
    input.siteRevisionFeedbackRef === undefined
  ) {
    throw new OmniWorkroomLifecycleValidationError({
      reason: 'Site revision requests must include siteRevisionFeedbackRef.',
    })
  }

  if (
    input.decisionKind === 'request_revision' &&
    input.workKind !== 'site' &&
    input.followupRequestRef === undefined
  ) {
    throw new OmniWorkroomLifecycleValidationError({
      reason: 'Non-Site revision requests must include followupRequestRef.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): OmniWorkroomLifecycleStorageError =>
  new OmniWorkroomLifecycleStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniWorkroomLifecycleStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const decisionFromRow = (
  row: LifecycleDecisionRow,
): OmniWorkroomLifecycleDecisionRecord => ({
  actorKind: row.actor_kind,
  archivedAt: row.archived_at,
  artifactRef: row.artifact_ref,
  createdAt: row.created_at,
  customerSafeExplanationRef: row.customer_safe_explanation_ref,
  decisionKind: row.decision_kind,
  followupRequestRef: row.followup_request_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  noSettlementImplication: row.no_settlement_implication === 1,
  receiptRef: row.receipt_ref,
  resultingState: row.resulting_state,
  siteRevisionFeedbackRef: row.site_revision_feedback_ref,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  OmniWorkroomLifecycleDecisionRecord | null,
  OmniWorkroomLifecycleStorageError
> =>
  d1Effect('omniWorkroomLifecycle.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_workroom_lifecycle_decisions
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<LifecycleDecisionRow>(),
  ).pipe(Effect.map(row => (row === null ? null : decisionFromRow(row))))

const readWorkroom = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniWorkroomLifecycleStorageError> =>
  d1Effect('omniWorkroomLifecycle.workroom', () =>
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

export const recordOmniWorkroomLifecycleDecision = (
  db: D1Database,
  input: RecordOmniWorkroomLifecycleDecisionInput,
  runtime: OmniWorkroomLifecycleRuntime = systemOmniWorkroomLifecycleRuntime,
): Effect.Effect<OmniWorkroomLifecycleDecisionRecord, OmniWorkroomLifecycleError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroom(db, input.workroomId)

    if (workroom === null) {
      return yield* new OmniWorkroomLifecycleWorkroomNotFound({
        workroomId: input.workroomId,
      })
    }

    if (workroom.work_kind !== input.workKind) {
      return yield* new OmniWorkroomLifecycleValidationError({
        reason: 'lifecycle decision workKind must match the workroom workKind.',
      })
    }

    const now = runtime.nowIso()
    const record: OmniWorkroomLifecycleDecisionRecord = {
      actorKind: input.actorKind,
      archivedAt: null,
      artifactRef: input.artifactRef ?? null,
      createdAt: now,
      customerSafeExplanationRef: input.customerSafeExplanationRef,
      decisionKind: input.decisionKind,
      followupRequestRef: input.followupRequestRef ?? null,
      id: input.id ?? runtime.makeDecisionId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      noSettlementImplication: true,
      receiptRef: input.receiptRef,
      resultingState: stateFromDecision(input.decisionKind),
      siteRevisionFeedbackRef: input.siteRevisionFeedbackRef ?? null,
      workKind: input.workKind,
      workroomId: input.workroomId,
    }

    yield* d1Effect('omniWorkroomLifecycle.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_workroom_lifecycle_decisions
             (id,
              idempotency_key,
              workroom_id,
              work_kind,
              actor_kind,
              decision_kind,
              resulting_state,
              customer_safe_explanation_ref,
              receipt_ref,
              site_revision_feedback_ref,
              followup_request_ref,
              artifact_ref,
              no_settlement_implication,
              metadata_json,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workroomId,
          record.workKind,
          record.actorKind,
          record.decisionKind,
          record.resultingState,
          record.customerSafeExplanationRef,
          record.receiptRef,
          record.siteRevisionFeedbackRef,
          record.followupRequestRef,
          record.artifactRef,
          record.noSettlementImplication ? 1 : 0,
          JSON.stringify(record.metadata),
          record.createdAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniWorkroomLifecycleProjection = (
  decision: OmniWorkroomLifecycleDecisionRecord,
) => ({
  customerSafeExplanationRef: decision.customerSafeExplanationRef,
  noSettlementImplication: decision.noSettlementImplication,
  receiptRef: decision.receiptRef,
  resultingState: decision.resultingState,
  workKind: decision.workKind,
  workroomId: decision.workroomId,
})

export const customerOmniWorkroomLifecycleProjection = (
  decision: OmniWorkroomLifecycleDecisionRecord,
) => ({
  ...publicOmniWorkroomLifecycleProjection(decision),
  artifactRef: decision.artifactRef,
  decisionKind: decision.decisionKind,
  followupRequestRef: decision.followupRequestRef,
  siteRevisionFeedbackRef: decision.siteRevisionFeedbackRef,
})

export const operatorOmniWorkroomLifecycleProjection = (
  decision: OmniWorkroomLifecycleDecisionRecord,
) => ({
  ...customerOmniWorkroomLifecycleProjection(decision),
  actorKind: decision.actorKind,
  id: decision.id,
  idempotencyKey: decision.idempotencyKey,
  metadata: decision.metadata,
})
