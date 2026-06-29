import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from '../../json-boundary'
import { compactRandomId, currentIsoTimestamp } from '../../runtime-primitives'
import type {
  BlueprintActionSubmission,
  BlueprintActionSubmissionKind,
} from '../schemas/action-submission'

export type BlueprintActionSubmissionsRuntime = Readonly<{
  makeActionSubmissionId: () => string
  nowIso: () => string
}>

export const systemBlueprintActionSubmissionsRuntime: BlueprintActionSubmissionsRuntime =
  {
    makeActionSubmissionId: () =>
      compactRandomId('blueprint_action_submission'),
    nowIso: currentIsoTimestamp,
  }

export type RecordBlueprintActionSubmissionProposalInput = Readonly<{
  actionKind: BlueprintActionSubmissionKind
  approvalPolicyRef: string
  contextPackRefs?: ReadonlyArray<string> | undefined
  dryRunRequired?: boolean | undefined
  evidenceRefs: ReadonlyArray<string>
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  proposedByProgramRunId: string
  proposedEffectRef: string
  receiptRefs?: ReadonlyArray<string> | undefined
  sourceAuthorityRefs?: ReadonlyArray<string> | undefined
  summaryRef: string
  toolRefs?: ReadonlyArray<string> | undefined
}>

type ActionSubmissionRow = Readonly<{
  action_kind: BlueprintActionSubmissionKind
  approval_policy_ref: string
  approval_receipt_ref: string | null
  approval_state: 'pending'
  approved_by_ref: string | null
  archived_at: string | null
  content_redacted: number
  context_pack_refs_json: string
  created_at: string
  direct_execution: number
  direct_program_run_execution_allowed: number
  dry_run_receipt_ref: string | null
  dry_run_required: number
  evidence_refs_json: string
  execution_receipt_ref: string | null
  failure_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  model_confidence_bypass_disabled: number
  program_run_authority_boundary: 'evidence_only'
  proposal_only: number
  proposed_by_program_run_id: string
  proposed_effect_ref: string
  receipt_refs_json: string
  source_authority_refs_json: string
  status: 'pending_approval'
  summary_ref: string
  tool_refs_json: string
  updated_at: string
}>

export class BlueprintActionSubmissionValidationError extends S.TaggedErrorClass<BlueprintActionSubmissionValidationError>()(
  'BlueprintActionSubmissionValidationError',
  { reason: S.String },
) {}

export class BlueprintActionSubmissionStorageError extends S.TaggedErrorClass<BlueprintActionSubmissionStorageError>()(
  'BlueprintActionSubmissionStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type BlueprintActionSubmissionError =
  | BlueprintActionSubmissionStorageError
  | BlueprintActionSubmissionValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(access_token|auth[_. -]?grant|callback[_. -]?(token|url)|callbackToken|callbackUrl|contact[_. -]?email|customer[_. -]?(email|name)|customerEmail|customerName|email[_. -]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|mdk_access_token|mnemonic|payment[_. -]?(hash|id|preimage|secret)|private[_. -]?(file|key|repo)|provider[_. -]?(account|payload|token)|raw[_. -]?(email|file|prompt|run[_. -]?log|source)|refresh_token|sk-[a-z0-9]+|source[_. -]?archive|wallet|wallet[_. -]?secret|webhook[_. -]?secret|xprv)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'direct_execution',
  'email_sent',
  'execution_completed',
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
export const BLUEPRINT_ACTION_SUBMISSION_STUDYBENCH_EVIDENCE_PREFIXES = [
  'probe_closeout.probe_run.studybench_',
  'rubric_score.probe.studybench',
  'study_packet.',
  'studybench_task.',
] as const

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
    throw new BlueprintActionSubmissionValidationError({
      reason: `${field} must be a redacted proposal ref without raw provider, email, payment, callback, source, wallet, or customer-private material.`,
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
    throw new BlueprintActionSubmissionValidationError({
      reason: `${field} must not contain raw provider, email, payment, callback, source, wallet, or customer-private material.`,
    })
  }
}

const uniqueStrings = (
  values: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(values)]

export const blueprintActionSubmissionEvidenceRefIsStudybench = (
  ref: string,
): boolean =>
  BLUEPRINT_ACTION_SUBMISSION_STUDYBENCH_EVIDENCE_PREFIXES.some(prefix =>
    ref.startsWith(prefix),
  )

export const blueprintActionSubmissionStudybenchEvidenceRefs = (
  evidenceRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  evidenceRefs.filter(ref =>
    blueprintActionSubmissionEvidenceRefIsStudybench(ref),
  )

const receiptRefsForProposal = (
  input: RecordBlueprintActionSubmissionProposalInput,
): ReadonlyArray<string> =>
  uniqueStrings(['receipt.action_submission', ...(input.receiptRefs ?? [])])

const assertValidInput = (
  input: RecordBlueprintActionSubmissionProposalInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('approvalPolicyRef', input.approvalPolicyRef)
  assertSafeRef('proposedByProgramRunId', input.proposedByProgramRunId)
  assertSafeRef('proposedEffectRef', input.proposedEffectRef)
  assertSafeRef('summaryRef', input.summaryRef)
  assertSafeRefs('contextPackRefs', input.contextPackRefs)
  assertSafeRefs('evidenceRefs', input.evidenceRefs)
  assertSafeRefs('receiptRefs', input.receiptRefs)
  assertSafeRefs('sourceAuthorityRefs', input.sourceAuthorityRefs)
  assertSafeRefs('toolRefs', input.toolRefs)
  assertSafeRecord('metadata', input.metadata ?? {})

  if (input.evidenceRefs.length === 0) {
    throw new BlueprintActionSubmissionValidationError({
      reason: 'Action Submission proposals require at least one evidence ref.',
    })
  }

  if (
    !input.approvalPolicyRef.startsWith(
      'policy.blueprint.action_submission.',
    )
  ) {
    throw new BlueprintActionSubmissionValidationError({
      reason: 'Action Submission proposals require a Blueprint approval policy ref.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): BlueprintActionSubmissionStorageError =>
  new BlueprintActionSubmissionStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, BlueprintActionSubmissionStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const actionSubmissionFromRow = (
  row: ActionSubmissionRow,
): BlueprintActionSubmission => ({
  actionKind: row.action_kind,
  approvalPolicyRef: row.approval_policy_ref,
  approvalReceiptRef: row.approval_receipt_ref,
  approvalState: row.approval_state,
  approvedByRef: row.approved_by_ref,
  contentRedacted: row.content_redacted === 1,
  contextPackRefs: parseJsonStringArray(row.context_pack_refs_json),
  createdAt: row.created_at,
  directExecution: row.direct_execution === 1,
  directProgramRunExecutionAllowed:
    row.direct_program_run_execution_allowed === 1,
  dryRunReceiptRef: row.dry_run_receipt_ref,
  dryRunRequired: row.dry_run_required === 1,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  executionReceiptRef: row.execution_receipt_ref,
  failureRef: row.failure_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  modelConfidenceBypassDisabled:
    row.model_confidence_bypass_disabled === 1,
  programRunAuthorityBoundary: row.program_run_authority_boundary,
  proposalOnly: row.proposal_only === 1,
  proposedByProgramRunId: row.proposed_by_program_run_id,
  proposedEffectRef: row.proposed_effect_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  sourceAuthorityRefs: parseJsonStringArray(row.source_authority_refs_json),
  status: row.status,
  summaryRef: row.summary_ref,
  toolRefs: parseJsonStringArray(row.tool_refs_json),
  updatedAt: row.updated_at,
})

export const readBlueprintActionSubmissionById = (
  db: D1Database,
  id: string,
): Effect.Effect<
  BlueprintActionSubmission | null,
  BlueprintActionSubmissionStorageError
> =>
  d1Effect('read blueprint action submission by id', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_action_submissions
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(id)
      .first<ActionSubmissionRow>(),
  ).pipe(
    Effect.map(row => (row === null ? null : actionSubmissionFromRow(row))),
  )

export const listBlueprintActionSubmissions = (
  db: D1Database,
  limit = 100,
): Effect.Effect<
  ReadonlyArray<BlueprintActionSubmission>,
  BlueprintActionSubmissionStorageError
> => {
  const boundedLimit = Math.max(1, Math.min(500, Math.trunc(limit)))

  return d1Effect('list blueprint action submissions', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_action_submissions
          WHERE archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(boundedLimit)
      .all<ActionSubmissionRow>(),
  ).pipe(
    Effect.map(result =>
      (result.results ?? []).map(row => actionSubmissionFromRow(row)),
    ),
  )
}

const readBlueprintActionSubmissionByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  BlueprintActionSubmission | null,
  BlueprintActionSubmissionStorageError
> =>
  d1Effect('read blueprint action submission by idempotency key', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_action_submissions
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ActionSubmissionRow>(),
  ).pipe(
    Effect.map(row => (row === null ? null : actionSubmissionFromRow(row))),
  )

export const recordBlueprintActionSubmissionProposal = (
  db: D1Database,
  input: RecordBlueprintActionSubmissionProposalInput,
  runtime: BlueprintActionSubmissionsRuntime = systemBlueprintActionSubmissionsRuntime,
): Effect.Effect<BlueprintActionSubmission, BlueprintActionSubmissionError> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readBlueprintActionSubmissionByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existing !== null) {
      return existing
    }

    const nowIso = runtime.nowIso()
    const id = input.id ?? runtime.makeActionSubmissionId()

    yield* d1Effect('insert blueprint action submission proposal', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO blueprint_action_submissions (
             id,
             idempotency_key,
             action_kind,
             approval_policy_ref,
             approval_receipt_ref,
             approval_state,
             approved_by_ref,
             content_redacted,
             context_pack_refs_json,
             direct_execution,
             direct_program_run_execution_allowed,
             dry_run_receipt_ref,
             dry_run_required,
             evidence_refs_json,
             execution_receipt_ref,
             failure_ref,
             model_confidence_bypass_disabled,
             program_run_authority_boundary,
             proposal_only,
             proposed_by_program_run_id,
             proposed_effect_ref,
             receipt_refs_json,
             source_authority_refs_json,
             status,
             summary_ref,
             tool_refs_json,
             metadata_json,
             created_at,
             updated_at,
             archived_at
           ) VALUES (?, ?, ?, ?, NULL, ?, NULL, ?, ?, ?, ?, NULL, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          id,
          input.idempotencyKey,
          input.actionKind,
          input.approvalPolicyRef,
          'pending',
          1,
          JSON.stringify(input.contextPackRefs ?? []),
          0,
          0,
          input.dryRunRequired ?? true ? 1 : 0,
          JSON.stringify(input.evidenceRefs),
          1,
          'evidence_only',
          1,
          input.proposedByProgramRunId,
          input.proposedEffectRef,
          JSON.stringify(receiptRefsForProposal(input)),
          JSON.stringify(input.sourceAuthorityRefs ?? []),
          'pending_approval',
          input.summaryRef,
          JSON.stringify(input.toolRefs ?? []),
          JSON.stringify(input.metadata ?? {}),
          nowIso,
          nowIso,
        )
        .run(),
    )

    const inserted = yield* readBlueprintActionSubmissionByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (inserted === null) {
      return yield* new BlueprintActionSubmissionStorageError({
        operation: 'read inserted blueprint action submission',
        reason: 'inserted or existing action submission was not readable.',
      })
    }

    return inserted
  })

export const parseBlueprintActionSubmissionMetadata = (
  value: string | null | undefined,
): Record<string, unknown> => parseJsonRecord(value) ?? {}
