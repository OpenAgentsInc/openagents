import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  parseJsonRecord,
  parseJsonStringArray,
  parseJsonWithSchema,
} from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const OmniRouteRejectionReasonKind = S.Literals([
  'cost',
  'latency',
  'privacy',
  'trust',
  'capability',
  'availability',
  'quality',
  'quota',
])
export type OmniRouteRejectionReasonKind =
  typeof OmniRouteRejectionReasonKind.Type

export const OmniRouteObservedResultKind = S.Literals([
  'success',
  'partial',
  'failure',
  'unavailable',
])
export type OmniRouteObservedResultKind =
  typeof OmniRouteObservedResultKind.Type

export const OmniRoutePrivacyTier = S.Literals([
  'public',
  'customer',
  'team',
  'operator',
  'private',
])
export type OmniRoutePrivacyTier = typeof OmniRoutePrivacyTier.Type

export const OmniRouteTrustTier = S.Literals([
  'verified',
  'reviewed',
  'unverified',
  'blocked',
])
export type OmniRouteTrustTier = typeof OmniRouteTrustTier.Type

export const OmniRejectedRouteCandidate = S.Struct({
  candidateRef: S.String,
  reasonKind: OmniRouteRejectionReasonKind,
  reasonRef: S.String,
})
export type OmniRejectedRouteCandidate =
  typeof OmniRejectedRouteCandidate.Type

const RejectedCandidateArray = S.Array(OmniRejectedRouteCandidate)

export const OmniRouteScorecardRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  costCents: S.Number,
  createdAt: S.String,
  decisionReasonRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  latencyMs: S.Number,
  metadata: S.Record(S.String, S.Unknown),
  observedResultKind: OmniRouteObservedResultKind,
  observedResultRef: S.String,
  postCloseoutScore: S.NullOr(S.Number),
  privacyTier: OmniRoutePrivacyTier,
  publicCaveatRef: S.String,
  rejectedCandidates: S.Array(OmniRejectedRouteCandidate),
  selectedAccountRef: S.NullOr(S.String),
  selectedModelRef: S.String,
  selectedProviderRef: S.String,
  selectedRouteRef: S.String,
  selectedRuntimeRef: S.String,
  trustTier: OmniRouteTrustTier,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniRouteScorecardRecord = typeof OmniRouteScorecardRecord.Type

export type OmniRouteScorecardsRuntime = Readonly<{
  makeScorecardId: () => string
  nowIso: () => string
}>

export const systemOmniRouteScorecardsRuntime: OmniRouteScorecardsRuntime = {
  makeScorecardId: () => compactRandomId('omni_route_scorecard'),
  nowIso: currentIsoTimestamp,
}

export type RecordOmniRouteScorecardInput = Readonly<{
  costCents?: number | undefined
  decisionReasonRefs?: ReadonlyArray<string> | undefined
  id?: string | undefined
  idempotencyKey: string
  latencyMs?: number | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  observedResultKind: OmniRouteObservedResultKind
  observedResultRef: string
  postCloseoutScore?: number | null | undefined
  privacyTier: OmniRoutePrivacyTier
  publicCaveatRef: string
  rejectedCandidates?: ReadonlyArray<OmniRejectedRouteCandidate> | undefined
  selectedAccountRef?: string | undefined
  selectedModelRef: string
  selectedProviderRef: string
  selectedRouteRef: string
  selectedRuntimeRef: string
  trustTier: OmniRouteTrustTier
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

type ScorecardRow = Readonly<{
  archived_at: string | null
  cost_cents: number
  created_at: string
  decision_reason_refs_json: string
  id: string
  idempotency_key: string
  latency_ms: number
  metadata_json: string
  observed_result_kind: OmniRouteObservedResultKind
  observed_result_ref: string
  post_closeout_score: number | null
  privacy_tier: OmniRoutePrivacyTier
  public_caveat_ref: string
  rejected_candidates_json: string
  selected_account_ref: string | null
  selected_model_ref: string
  selected_provider_ref: string
  selected_route_ref: string
  selected_runtime_ref: string
  trust_tier: OmniRouteTrustTier
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniRouteScorecardValidationError extends S.TaggedErrorClass<OmniRouteScorecardValidationError>()(
  'OmniRouteScorecardValidationError',
  { reason: S.String },
) {}

export class OmniRouteScorecardStorageError extends S.TaggedErrorClass<OmniRouteScorecardStorageError>()(
  'OmniRouteScorecardStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniRouteScorecardWorkroomNotFound extends S.TaggedErrorClass<OmniRouteScorecardWorkroomNotFound>()(
  'OmniRouteScorecardWorkroomNotFound',
  { workroomId: S.String },
) {}

export type OmniRouteScorecardError =
  | OmniRouteScorecardStorageError
  | OmniRouteScorecardValidationError
  | OmniRouteScorecardWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'provider_account',
  'raw_email',
  'raw_run_log',
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
    throw new OmniRouteScorecardValidationError({
      reason: `${field} must be a route-safe ref without provider account, raw provider, run log, email, payment, wallet, or private customer material.`,
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
    throw new OmniRouteScorecardValidationError({
      reason:
        'metadata must not contain provider account, raw provider, run log, email, payment, wallet, or private customer material.',
    })
  }
}

const integer = (field: string, value: number | undefined): number => {
  const normalized = value ?? 0

  if (!Number.isInteger(normalized) || normalized < 0) {
    throw new OmniRouteScorecardValidationError({
      reason: `${field} must be a non-negative integer.`,
    })
  }

  return normalized
}

const assertScore = (score: number | null | undefined): void => {
  if (score === null || score === undefined) {
    return
  }

  if (!Number.isInteger(score) || score < 0 || score > 100) {
    throw new OmniRouteScorecardValidationError({
      reason: 'postCloseoutScore must be an integer from 0 through 100.',
    })
  }
}

const assertRejectedCandidates = (
  candidates: ReadonlyArray<OmniRejectedRouteCandidate> | undefined,
): void => {
  ;[...(candidates ?? [])].forEach(candidate => {
    assertSafeRef('rejectedCandidates.candidateRef', candidate.candidateRef)
    assertSafeRef('rejectedCandidates.reasonRef', candidate.reasonRef)
  })
}

const assertDecisionReasonRefs = (
  refs: ReadonlyArray<string> | undefined,
): void => {
  ;[...(refs ?? [])].forEach(ref => {
    assertSafeRef('decisionReasonRefs', ref)
  })
}

const assertValidInput = (input: RecordOmniRouteScorecardInput): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef('selectedRouteRef', input.selectedRouteRef)
  assertSafeRef('selectedProviderRef', input.selectedProviderRef)
  assertSafeRef('selectedAccountRef', input.selectedAccountRef)
  assertSafeRef('selectedModelRef', input.selectedModelRef)
  assertSafeRef('selectedRuntimeRef', input.selectedRuntimeRef)
  assertSafeRef('observedResultRef', input.observedResultRef)
  assertSafeRef('publicCaveatRef', input.publicCaveatRef)
  assertRejectedCandidates(input.rejectedCandidates)
  assertDecisionReasonRefs(input.decisionReasonRefs)
  assertSafeMetadata(input.metadata)
  integer('costCents', input.costCents)
  integer('latencyMs', input.latencyMs)
  assertScore(input.postCloseoutScore)
}

const storageError = (
  operation: string,
  error: unknown,
): OmniRouteScorecardStorageError =>
  new OmniRouteScorecardStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniRouteScorecardStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const rowToRecord = (row: ScorecardRow): OmniRouteScorecardRecord => ({
  archivedAt: row.archived_at,
  costCents: row.cost_cents,
  createdAt: row.created_at,
  decisionReasonRefs: parseJsonStringArray(row.decision_reason_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  latencyMs: row.latency_ms,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  observedResultKind: row.observed_result_kind,
  observedResultRef: row.observed_result_ref,
  postCloseoutScore: row.post_closeout_score,
  privacyTier: row.privacy_tier,
  publicCaveatRef: row.public_caveat_ref,
  rejectedCandidates: parseJsonWithSchema(
    RejectedCandidateArray,
    row.rejected_candidates_json,
  ),
  selectedAccountRef: row.selected_account_ref,
  selectedModelRef: row.selected_model_ref,
  selectedProviderRef: row.selected_provider_ref,
  selectedRouteRef: row.selected_route_ref,
  selectedRuntimeRef: row.selected_runtime_ref,
  trustTier: row.trust_tier,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<OmniRouteScorecardRecord | null, OmniRouteScorecardStorageError> =>
  d1Effect('omniRouteScorecards.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_route_scorecards
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ScorecardRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToRecord(row))))

const readWorkroom = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniRouteScorecardStorageError> =>
  d1Effect('omniRouteScorecards.workroom', () =>
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

export const recordOmniRouteScorecard = (
  db: D1Database,
  input: RecordOmniRouteScorecardInput,
  runtime: OmniRouteScorecardsRuntime = systemOmniRouteScorecardsRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<OmniRouteScorecardRecord, OmniRouteScorecardError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroom(db, input.workroomId)

    if (workroom === null) {
      return yield* new OmniRouteScorecardWorkroomNotFound({
        workroomId: input.workroomId,
      })
    }

    if (workroom.work_kind !== input.workKind) {
      return yield* new OmniRouteScorecardValidationError({
        reason: 'route scorecard workKind must match the workroom workKind.',
      })
    }

    const now = runtime.nowIso()
    const record: OmniRouteScorecardRecord = {
      archivedAt: null,
      costCents: integer('costCents', input.costCents),
      createdAt: now,
      decisionReasonRefs: [...(input.decisionReasonRefs ?? [])],
      id: input.id ?? runtime.makeScorecardId(),
      idempotencyKey: input.idempotencyKey,
      latencyMs: integer('latencyMs', input.latencyMs),
      metadata: input.metadata ?? {},
      observedResultKind: input.observedResultKind,
      observedResultRef: input.observedResultRef,
      postCloseoutScore: input.postCloseoutScore ?? null,
      privacyTier: input.privacyTier,
      publicCaveatRef: input.publicCaveatRef,
      rejectedCandidates: [...(input.rejectedCandidates ?? [])],
      selectedAccountRef: input.selectedAccountRef ?? null,
      selectedModelRef: input.selectedModelRef,
      selectedProviderRef: input.selectedProviderRef,
      selectedRouteRef: input.selectedRouteRef,
      selectedRuntimeRef: input.selectedRuntimeRef,
      trustTier: input.trustTier,
      updatedAt: now,
      workKind: input.workKind,
      workroomId: input.workroomId,
    }

    yield* d1Effect('omniRouteScorecards.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_route_scorecards
             (id,
              idempotency_key,
              workroom_id,
              work_kind,
              selected_route_ref,
              selected_provider_ref,
              selected_account_ref,
              selected_model_ref,
              selected_runtime_ref,
              rejected_candidates_json,
              decision_reason_refs_json,
              observed_result_kind,
              observed_result_ref,
              post_closeout_score,
              cost_cents,
              latency_ms,
              privacy_tier,
              trust_tier,
              public_caveat_ref,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workroomId,
          record.workKind,
          record.selectedRouteRef,
          record.selectedProviderRef,
          record.selectedAccountRef,
          record.selectedModelRef,
          record.selectedRuntimeRef,
          JSON.stringify(record.rejectedCandidates),
          JSON.stringify(record.decisionReasonRefs),
          record.observedResultKind,
          record.observedResultRef,
          record.postCloseoutScore,
          record.costCents,
          record.latencyMs,
          record.privacyTier,
          record.trustTier,
          record.publicCaveatRef,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
        )
        .run()
        .then(() => undefined),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('omni_route_scorecards', [[record.id]]),
      )
    }

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniRouteScorecardProjection = (
  scorecard: OmniRouteScorecardRecord,
) => ({
  observedResultKind: scorecard.observedResultKind,
  observedResultRef: scorecard.observedResultRef,
  postCloseoutScore: scorecard.postCloseoutScore,
  publicCaveatRef: scorecard.publicCaveatRef,
  selectedModelRef: scorecard.selectedModelRef,
  selectedRuntimeRef: scorecard.selectedRuntimeRef,
  trustTier: scorecard.trustTier,
  workKind: scorecard.workKind,
  workroomId: scorecard.workroomId,
})

export const customerOmniRouteScorecardProjection = (
  scorecard: OmniRouteScorecardRecord,
) => ({
  ...publicOmniRouteScorecardProjection(scorecard),
  decisionReasonRefs: scorecard.decisionReasonRefs,
  privacyTier: scorecard.privacyTier,
  selectedRouteRef: scorecard.selectedRouteRef,
})

export const operatorOmniRouteScorecardProjection = (
  scorecard: OmniRouteScorecardRecord,
) => scorecard
