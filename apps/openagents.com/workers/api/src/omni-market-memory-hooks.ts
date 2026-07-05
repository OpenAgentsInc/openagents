import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import type { OmniWorkroomLifecycleState } from './omni-workroom-lifecycle'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const OmniMarketMemoryOutcomeState = S.Literals(['accepted', 'rejected'])
export type OmniMarketMemoryOutcomeState =
  typeof OmniMarketMemoryOutcomeState.Type

export const OmniMarketMemoryCategory = S.Literals([
  'route_quality',
  'account_reliability',
  'repo_convention',
  'source_quality',
  'module_usefulness',
  'marketplace_attribution',
])
export type OmniMarketMemoryCategory = typeof OmniMarketMemoryCategory.Type

export const OmniMarketMemoryAuthorityBoundary = S.Literals(['evidence_only'])
export type OmniMarketMemoryAuthorityBoundary =
  typeof OmniMarketMemoryAuthorityBoundary.Type

export const OmniMarketMemoryHookRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  authorityBoundary: OmniMarketMemoryAuthorityBoundary,
  category: OmniMarketMemoryCategory,
  createdAt: S.String,
  economicsRef: S.NullOr(S.String),
  evidenceRef: S.String,
  id: S.String,
  idempotencyKey: S.String,
  lifecycleDecisionId: S.String,
  memoryRef: S.String,
  metadata: S.Record(S.String, S.Unknown),
  noModulePromotion: S.Boolean,
  noPayoutMutation: S.Boolean,
  noPublicClaimMutation: S.Boolean,
  noRoutingMutation: S.Boolean,
  outcomeState: OmniMarketMemoryOutcomeState,
  publicCaveatRef: S.String,
  routeScorecardRef: S.NullOr(S.String),
  sourceRef: S.String,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniMarketMemoryHookRecord =
  typeof OmniMarketMemoryHookRecord.Type

export type OmniMarketMemoryHooksRuntime = Readonly<{
  makeMemoryHookId: () => string
  nowIso: () => string
}>

export const systemOmniMarketMemoryHooksRuntime: OmniMarketMemoryHooksRuntime =
  {
    makeMemoryHookId: () => compactRandomId('omni_market_memory_hook'),
    nowIso: currentIsoTimestamp,
  }

export type RecordOmniMarketMemoryHookInput = Readonly<{
  category: OmniMarketMemoryCategory
  economicsRef?: string | undefined
  evidenceRef: string
  id?: string | undefined
  idempotencyKey: string
  lifecycleDecisionId: string
  memoryRef: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  outcomeState: OmniMarketMemoryOutcomeState
  publicCaveatRef: string
  routeScorecardRef?: string | undefined
  sourceRef: string
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

type LifecycleDecisionRefRow = Readonly<{
  archived_at: string | null
  id: string
  resulting_state: OmniWorkroomLifecycleState
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

type MarketMemoryHookRow = Readonly<{
  archived_at: string | null
  authority_boundary: OmniMarketMemoryAuthorityBoundary
  category: OmniMarketMemoryCategory
  created_at: string
  economics_ref: string | null
  evidence_ref: string
  id: string
  idempotency_key: string
  lifecycle_decision_id: string
  memory_ref: string
  metadata_json: string
  no_module_promotion: number
  no_payout_mutation: number
  no_public_claim_mutation: number
  no_routing_mutation: number
  outcome_state: OmniMarketMemoryOutcomeState
  public_caveat_ref: string
  route_scorecard_ref: string | null
  source_ref: string
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniMarketMemoryHookValidationError extends S.TaggedErrorClass<OmniMarketMemoryHookValidationError>()(
  'OmniMarketMemoryHookValidationError',
  { reason: S.String },
) {}

export class OmniMarketMemoryHookStorageError extends S.TaggedErrorClass<OmniMarketMemoryHookStorageError>()(
  'OmniMarketMemoryHookStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniMarketMemoryHookWorkroomNotFound extends S.TaggedErrorClass<OmniMarketMemoryHookWorkroomNotFound>()(
  'OmniMarketMemoryHookWorkroomNotFound',
  { workroomId: S.String },
) {}

export class OmniMarketMemoryHookLifecycleDecisionNotFound extends S.TaggedErrorClass<OmniMarketMemoryHookLifecycleDecisionNotFound>()(
  'OmniMarketMemoryHookLifecycleDecisionNotFound',
  { lifecycleDecisionId: S.String },
) {}

export type OmniMarketMemoryHookError =
  | OmniMarketMemoryHookLifecycleDecisionNotFound
  | OmniMarketMemoryHookStorageError
  | OmniMarketMemoryHookValidationError
  | OmniMarketMemoryHookWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'eligible_for_payout',
  'force_provider_route',
  'module_promoted',
  'paid_out',
  'payment_settled',
  'payout',
  'production_claim',
  'provider_payload',
  'provider_weight_override',
  'public_claim_published',
  'raw_email',
  'raw_run_log',
  'route_override',
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
    throw new OmniMarketMemoryHookValidationError({
      reason: `${field} must be an evidence-only memory ref without raw provider, run log, email, payment, settlement, payout, routing override, module promotion, public claim, wallet, or private customer material.`,
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
    PROHIBITED_TEXT_PATTERN.test(json) ||
    PROHIBITED_FRAGMENTS.some(fragment => json.toLowerCase().includes(fragment))
  ) {
    throw new OmniMarketMemoryHookValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, settlement, payout, routing override, module promotion, public claim, wallet, or private customer material.',
    })
  }
}

const assertValidInput = (input: RecordOmniMarketMemoryHookInput): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef('lifecycleDecisionId', input.lifecycleDecisionId)
  assertSafeRef('memoryRef', input.memoryRef)
  assertSafeRef('evidenceRef', input.evidenceRef)
  assertSafeRef('sourceRef', input.sourceRef)
  assertSafeRef('publicCaveatRef', input.publicCaveatRef)
  assertSafeRef('routeScorecardRef', input.routeScorecardRef)
  assertSafeRef('economicsRef', input.economicsRef)
  assertSafeMetadata(input.metadata)
}

const storageError = (
  operation: string,
  error: unknown,
): OmniMarketMemoryHookStorageError =>
  new OmniMarketMemoryHookStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniMarketMemoryHookStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const hookFromRow = (row: MarketMemoryHookRow): OmniMarketMemoryHookRecord => ({
  archivedAt: row.archived_at,
  authorityBoundary: row.authority_boundary,
  category: row.category,
  createdAt: row.created_at,
  economicsRef: row.economics_ref,
  evidenceRef: row.evidence_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  lifecycleDecisionId: row.lifecycle_decision_id,
  memoryRef: row.memory_ref,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  noModulePromotion: row.no_module_promotion === 1,
  noPayoutMutation: row.no_payout_mutation === 1,
  noPublicClaimMutation: row.no_public_claim_mutation === 1,
  noRoutingMutation: row.no_routing_mutation === 1,
  outcomeState: row.outcome_state,
  publicCaveatRef: row.public_caveat_ref,
  routeScorecardRef: row.route_scorecard_ref,
  sourceRef: row.source_ref,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const readHookByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  OmniMarketMemoryHookRecord | null,
  OmniMarketMemoryHookStorageError
> =>
  d1Effect('read omni market memory hook by idempotency key', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_market_memory_hooks
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<MarketMemoryHookRow>(),
  ).pipe(Effect.map(row => (row === null ? null : hookFromRow(row))))

const readWorkroomRef = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniMarketMemoryHookStorageError> =>
  d1Effect('read omni workroom ref for market memory hook', () =>
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

const readLifecycleDecisionRef = (
  db: D1Database,
  lifecycleDecisionId: string,
): Effect.Effect<
  LifecycleDecisionRefRow | null,
  OmniMarketMemoryHookStorageError
> =>
  d1Effect('read omni lifecycle decision ref for market memory hook', () =>
    db
      .prepare(
        `SELECT id, workroom_id, work_kind, resulting_state, archived_at
           FROM omni_workroom_lifecycle_decisions
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(lifecycleDecisionId)
      .first<LifecycleDecisionRefRow>(),
  )

const ensureWorkroom = (
  workroom: WorkroomRefRow | null,
  input: RecordOmniMarketMemoryHookInput,
): Effect.Effect<WorkroomRefRow, OmniMarketMemoryHookWorkroomNotFound | OmniMarketMemoryHookValidationError> => {
  if (workroom === null) {
    return Effect.fail(
      new OmniMarketMemoryHookWorkroomNotFound({
        workroomId: input.workroomId,
      }),
    )
  }

  if (workroom.work_kind !== input.workKind) {
    return Effect.fail(
      new OmniMarketMemoryHookValidationError({
        reason: 'market memory hook workKind must match the workroom.',
      }),
    )
  }

  return Effect.succeed(workroom)
}

const ensureLifecycleDecision = (
  decision: LifecycleDecisionRefRow | null,
  input: RecordOmniMarketMemoryHookInput,
): Effect.Effect<
  LifecycleDecisionRefRow,
  | OmniMarketMemoryHookLifecycleDecisionNotFound
  | OmniMarketMemoryHookValidationError
> => {
  if (decision === null) {
    return Effect.fail(
      new OmniMarketMemoryHookLifecycleDecisionNotFound({
        lifecycleDecisionId: input.lifecycleDecisionId,
      }),
    )
  }

  if (
    decision.workroom_id !== input.workroomId ||
    decision.work_kind !== input.workKind
  ) {
    return Effect.fail(
      new OmniMarketMemoryHookValidationError({
        reason:
          'market memory hook lifecycle decision must belong to the workroom and work kind.',
      }),
    )
  }

  if (decision.resulting_state !== input.outcomeState) {
    return Effect.fail(
      new OmniMarketMemoryHookValidationError({
        reason:
          'market memory hook outcomeState must match the lifecycle decision state.',
      }),
    )
  }

  return Effect.succeed(decision)
}

const publicReceiptRef = (
  workroomId: string,
  category: OmniMarketMemoryCategory,
  idempotencyKey: string,
): string => `omni_market_memory:${workroomId}:${category}:${idempotencyKey}`

export const recordOmniMarketMemoryHook = (
  db: D1Database,
  input: RecordOmniMarketMemoryHookInput,
  runtime: OmniMarketMemoryHooksRuntime = systemOmniMarketMemoryHooksRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<OmniMarketMemoryHookRecord, OmniMarketMemoryHookError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readHookByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroomRef(db, input.workroomId).pipe(
      Effect.flatMap(row => ensureWorkroom(row, input)),
    )
    yield* readLifecycleDecisionRef(db, input.lifecycleDecisionId).pipe(
      Effect.flatMap(row => ensureLifecycleDecision(row, input)),
    )

    const nowIso = runtime.nowIso()
    const id = input.id ?? runtime.makeMemoryHookId()
    const metadata = input.metadata ?? {}
    const evidenceRef = input.evidenceRef
    const sourceRef = input.sourceRef
    const publicCaveatRef = input.publicCaveatRef

    yield* d1Effect('insert omni market memory hook', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_market_memory_hooks (
             id,
             idempotency_key,
             workroom_id,
             lifecycle_decision_id,
             work_kind,
             outcome_state,
             category,
             memory_ref,
             evidence_ref,
             source_ref,
             public_caveat_ref,
             route_scorecard_ref,
             economics_ref,
             authority_boundary,
             no_routing_mutation,
             no_payout_mutation,
             no_public_claim_mutation,
             no_module_promotion,
             metadata_json,
             created_at,
             updated_at,
             archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          id,
          input.idempotencyKey,
          workroom.id,
          input.lifecycleDecisionId,
          workroom.work_kind,
          input.outcomeState,
          input.category,
          input.memoryRef,
          evidenceRef,
          sourceRef,
          publicCaveatRef,
          input.routeScorecardRef ?? null,
          input.economicsRef ?? null,
          'evidence_only',
          1,
          1,
          1,
          1,
          JSON.stringify(metadata),
          nowIso,
          nowIso,
        )
        .run(),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('omni_market_memory_hooks', [[id]]),
      )
    }

    const inserted = yield* readHookByIdempotencyKey(db, input.idempotencyKey)

    if (inserted === null) {
      return yield* new OmniMarketMemoryHookStorageError({
        operation: 'read inserted omni market memory hook',
        reason: 'inserted or existing market memory hook was not readable.',
      })
    }

    return inserted
  })

export const operatorOmniMarketMemoryHookProjection = (
  record: OmniMarketMemoryHookRecord,
) => ({
  authorityBoundary: record.authorityBoundary,
  category: record.category,
  economicsRef: record.economicsRef,
  evidenceRef: record.evidenceRef,
  id: record.id,
  lifecycleDecisionId: record.lifecycleDecisionId,
  memoryRef: record.memoryRef,
  noModulePromotion: record.noModulePromotion,
  noPayoutMutation: record.noPayoutMutation,
  noPublicClaimMutation: record.noPublicClaimMutation,
  noRoutingMutation: record.noRoutingMutation,
  outcomeState: record.outcomeState,
  publicCaveatRef: record.publicCaveatRef,
  publicReceiptRef: publicReceiptRef(
    record.workroomId,
    record.category,
    record.idempotencyKey,
  ),
  routeScorecardRef: record.routeScorecardRef,
  sourceRef: record.sourceRef,
  workKind: record.workKind,
  workroomId: record.workroomId,
})

export const publicOmniMarketMemoryHookProjection = (
  record: OmniMarketMemoryHookRecord,
) => ({
  authorityBoundary: record.authorityBoundary,
  category: record.category,
  memoryRef: record.memoryRef,
  noDirectEffects: true,
  outcomeState: record.outcomeState,
  publicCaveatRef: record.publicCaveatRef,
  publicReceiptRef: publicReceiptRef(
    record.workroomId,
    record.category,
    record.idempotencyKey,
  ),
  workKind: record.workKind,
  workroomId: record.workroomId,
})
