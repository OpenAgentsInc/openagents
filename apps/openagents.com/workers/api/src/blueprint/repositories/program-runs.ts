import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from '../../json-boundary'
import { compactRandomId, currentIsoTimestamp } from '../../runtime-primitives'
import {
  type BlueprintProgramRunRecord,
  type BlueprintProgramRunAuthorityBoundary,
} from '../schemas/program-run'

export type BlueprintProgramRunsRuntime = Readonly<{
  makeProgramRunId: () => string
  nowIso: () => string
}>

export const systemBlueprintProgramRunsRuntime: BlueprintProgramRunsRuntime = {
  makeProgramRunId: () => compactRandomId('blueprint_program_run'),
  nowIso: currentIsoTimestamp,
}

export type RecordBlueprintProgramRunInput = Readonly<{
  actorRef: string
  confidence: number
  costRef: string
  evidenceRefs?: ReadonlyArray<string> | undefined
  id?: string | undefined
  idempotencyKey: string
  inputSnapshotHash: string
  latencyMs: number
  metadata?: Readonly<Record<string, unknown>> | undefined
  moduleVersionId: string
  programSignatureId: string
  programTypeId: string
  purposeRef: string
  receiptRefs?: ReadonlyArray<string> | undefined
  routeRef: string
  typedOutput: Readonly<Record<string, unknown>>
}>

type ProgramRunRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  authority_boundary: BlueprintProgramRunAuthorityBoundary
  confidence: number
  cost_ref: string
  created_at: string
  direct_mutation_disabled: number
  evidence_refs_json: string
  id: string
  idempotency_key: string
  input_snapshot_hash: string
  latency_ms: number
  metadata_json: string
  module_version_id: string
  no_deploy: number
  no_email: number
  no_source_mutation: number
  no_spend: number
  program_signature_id: string
  program_type_id: string
  purpose_ref: string
  receipt_refs_json: string
  route_ref: string
  typed_output_json: string
  updated_at: string
}>

export class BlueprintProgramRunValidationError extends S.TaggedErrorClass<BlueprintProgramRunValidationError>()(
  'BlueprintProgramRunValidationError',
  { reason: S.String },
) {}

export class BlueprintProgramRunStorageError extends S.TaggedErrorClass<BlueprintProgramRunStorageError>()(
  'BlueprintProgramRunStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type BlueprintProgramRunError =
  | BlueprintProgramRunStorageError
  | BlueprintProgramRunValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'deploy_now',
  'email_sent',
  'force_write',
  'mutation_authorized',
  'paid_out',
  'payment_settled',
  'production_promoted',
  'provider_payload',
  'raw_email',
  'raw_run_log',
  'source_mutated',
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
    throw new BlueprintProgramRunValidationError({
      reason: `${field} must be an evidence-only ref without raw provider, run log, email, payment, deploy, source mutation, wallet, or private customer material.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): void => {
  ;[...(values ?? [])].forEach(value => assertSafeRef(field, value))
}

const assertSafeRecord = (
  field: string,
  value: Readonly<Record<string, unknown>>,
): void => {
  const json = JSON.stringify(value)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_TEXT_PATTERN.test(json) ||
    PROHIBITED_FRAGMENTS.some(fragment => json.toLowerCase().includes(fragment))
  ) {
    throw new BlueprintProgramRunValidationError({
      reason: `${field} must not contain raw provider, run log, email, payment, deploy, source mutation, wallet, or private customer material.`,
    })
  }
}

const assertValidInput = (input: RecordBlueprintProgramRunInput): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('actorRef', input.actorRef)
  assertSafeRef('purposeRef', input.purposeRef)
  assertSafeRef('programTypeId', input.programTypeId)
  assertSafeRef('programSignatureId', input.programSignatureId)
  assertSafeRef('moduleVersionId', input.moduleVersionId)
  assertSafeRef('inputSnapshotHash', input.inputSnapshotHash)
  assertSafeRef('routeRef', input.routeRef)
  assertSafeRef('costRef', input.costRef)
  assertSafeRefs('evidenceRefs', input.evidenceRefs)
  assertSafeRefs('receiptRefs', input.receiptRefs)
  assertSafeRecord('typedOutput', input.typedOutput)
  assertSafeRecord('metadata', input.metadata ?? {})

  if (input.confidence < 0 || input.confidence > 1) {
    throw new BlueprintProgramRunValidationError({
      reason: 'confidence must be between 0 and 1.',
    })
  }

  if (input.latencyMs < 0) {
    throw new BlueprintProgramRunValidationError({
      reason: 'latencyMs must be non-negative.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): BlueprintProgramRunStorageError =>
  new BlueprintProgramRunStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, BlueprintProgramRunStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const programRunFromRow = (row: ProgramRunRow): BlueprintProgramRunRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  authorityBoundary: row.authority_boundary,
  confidence: row.confidence,
  costRef: row.cost_ref,
  createdAt: row.created_at,
  directMutationDisabled: row.direct_mutation_disabled === 1,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  inputSnapshotHash: row.input_snapshot_hash,
  latencyMs: row.latency_ms,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  moduleVersionId: row.module_version_id,
  noDeploy: row.no_deploy === 1,
  noEmail: row.no_email === 1,
  noSourceMutation: row.no_source_mutation === 1,
  noSpend: row.no_spend === 1,
  programSignatureId: row.program_signature_id,
  programTypeId: row.program_type_id,
  purposeRef: row.purpose_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  routeRef: row.route_ref,
  typedOutput: parseJsonRecord(row.typed_output_json) ?? {},
  updatedAt: row.updated_at,
})

export const readBlueprintProgramRunById = (
  db: D1Database,
  id: string,
): Effect.Effect<
  BlueprintProgramRunRecord | null,
  BlueprintProgramRunStorageError
> =>
  d1Effect('read blueprint program run by id', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_program_runs
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(id)
      .first<ProgramRunRow>(),
  ).pipe(Effect.map(row => (row === null ? null : programRunFromRow(row))))

export const listBlueprintProgramRuns = (
  db: D1Database,
  limit = 100,
): Effect.Effect<
  ReadonlyArray<BlueprintProgramRunRecord>,
  BlueprintProgramRunStorageError
> => {
  const boundedLimit = Math.max(1, Math.min(500, Math.trunc(limit)))

  return d1Effect('list blueprint program runs', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_program_runs
          WHERE archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(boundedLimit)
      .all<ProgramRunRow>(),
  ).pipe(
    Effect.map(result =>
      (result.results ?? []).map(row => programRunFromRow(row)),
    ),
  )
}

const readBlueprintProgramRunByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  BlueprintProgramRunRecord | null,
  BlueprintProgramRunStorageError
> =>
  d1Effect('read blueprint program run by idempotency key', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_program_runs
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ProgramRunRow>(),
  ).pipe(Effect.map(row => (row === null ? null : programRunFromRow(row))))

export const recordBlueprintProgramRun = (
  db: D1Database,
  input: RecordBlueprintProgramRunInput,
  runtime: BlueprintProgramRunsRuntime = systemBlueprintProgramRunsRuntime,
): Effect.Effect<BlueprintProgramRunRecord, BlueprintProgramRunError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readBlueprintProgramRunByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existing !== null) {
      return existing
    }

    const nowIso = runtime.nowIso()
    const id = input.id ?? runtime.makeProgramRunId()

    yield* d1Effect('insert blueprint program run', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO blueprint_program_runs (
             id,
             idempotency_key,
             actor_ref,
             purpose_ref,
             program_type_id,
             program_signature_id,
             module_version_id,
             input_snapshot_hash,
             typed_output_json,
             confidence,
             route_ref,
             cost_ref,
             latency_ms,
             evidence_refs_json,
             receipt_refs_json,
             authority_boundary,
             direct_mutation_disabled,
             no_deploy,
             no_email,
             no_spend,
             no_source_mutation,
             metadata_json,
             created_at,
             updated_at,
             archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          id,
          input.idempotencyKey,
          input.actorRef,
          input.purposeRef,
          input.programTypeId,
          input.programSignatureId,
          input.moduleVersionId,
          input.inputSnapshotHash,
          JSON.stringify(input.typedOutput),
          input.confidence,
          input.routeRef,
          input.costRef,
          input.latencyMs,
          JSON.stringify(input.evidenceRefs ?? []),
          JSON.stringify(input.receiptRefs ?? []),
          'evidence_only',
          1,
          1,
          1,
          1,
          1,
          JSON.stringify(input.metadata ?? {}),
          nowIso,
          nowIso,
        )
        .run(),
    )

    const inserted = yield* readBlueprintProgramRunByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (inserted === null) {
      return yield* new BlueprintProgramRunStorageError({
        operation: 'read inserted blueprint program run',
        reason: 'inserted or existing program run was not readable.',
      })
    }

    return inserted
  })
