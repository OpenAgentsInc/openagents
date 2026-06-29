import { artanisAdminCloseoutReceiptRef } from './artanis-admin-closeout-receipts'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const ARTANIS_TASSADAR_DISTILLATION_DATASET_STALENESS: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'artanis_admin_tick_decisions.insert',
    'artanis_closeout_verdicts.insert',
    'artanis_closeout_verdicts.update',
  ])

export const ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET = 10

export const ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER =
  'blocker.product_promises.tassadar_distillation_dataset_receipt_missing'
export const ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF =
  'receipt.training.tassadar_distillation_dataset.artanis_admin_verified_trace_refs.v1'

export type ArtanisDistillationDatasetRow = Readonly<{
  decision_id: unknown
  assignment_ref: unknown
  decision_created_at: unknown
  verdict_created_at: unknown
  verdict_outcome: unknown
  verdict_accept_state: unknown
  verdict_trace_digest_prefix: unknown
}>

export type ArtanisDistillationDatasetTrace = Readonly<{
  assignmentRef: string
  closeoutReceiptRef: string
  decisionRef: string
  sourceKind: 'artanis_admin_executor_trace_closeout'
  traceDigestPrefix: string
  verdictCreatedAt: string
  verdictRef: 'verdict.artanis_closeout.verified'
}>

export type ArtanisDistillationDatasetReceipt = Readonly<{
  kind: 'artanis_tassadar_distillation_dataset_receipt'
  publicSafe: true
  authorityBoundary: string
  staleness: PublicProjectionStalenessContract
  receiptState: 'available' | 'insufficient_verified_traces'
  receiptRef: string | null
  datasetRef: string | null
  datasetKind: 'tassadar_executor_verified_trace_refs'
  requiredVerifiedTraceCount: number
  sourceVerifiedTraceCount: number
  uniqueTraceDigestPrefixCount: number
  traceDigestPrefixes: ReadonlyArray<string>
  closeoutReceiptRefs: ReadonlyArray<string>
  traces: ReadonlyArray<ArtanisDistillationDatasetTrace>
  clearsBlockerRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  generatedAt: string
  notes: ReadonlyArray<string>
}>

const assignmentRefPattern = /^assignment\.artanis_admin\.[A-Za-z0-9_.-]+$/
const decisionIdPattern = /^[A-Za-z0-9_:-]{1,128}$/
const digestPrefixPattern = /^[a-f0-9]{1,32}$/
const isoLikePattern = /^\d{4}-\d{2}-\d{2}T[0-9:.]+Z$/

const datasetReceiptRef = ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF
const datasetRef =
  'dataset.tassadar_distillation.artanis_admin_verified_trace_refs.v1'

const stringOrNull = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : null

const safeDecisionRef = (value: unknown): string => {
  const text = stringOrNull(value)

  return text !== null && decisionIdPattern.test(text)
    ? `tick_decision.${text}`
    : 'tick_decision.redacted'
}

const safeIso = (value: unknown): string => {
  const text = stringOrNull(value)

  return text !== null && isoLikePattern.test(text) ? text : ''
}

const traceFromRow = (
  row: ArtanisDistillationDatasetRow,
): ArtanisDistillationDatasetTrace | null => {
  if (
    row.verdict_outcome !== 'verified' ||
    row.verdict_accept_state !== 'accepted'
  ) {
    return null
  }

  const assignmentRef = stringOrNull(row.assignment_ref)
  const traceDigestPrefix = stringOrNull(row.verdict_trace_digest_prefix)

  if (
    assignmentRef === null ||
    traceDigestPrefix === null ||
    !assignmentRefPattern.test(assignmentRef) ||
    !digestPrefixPattern.test(traceDigestPrefix)
  ) {
    return null
  }

  return {
    assignmentRef,
    closeoutReceiptRef: artanisAdminCloseoutReceiptRef(assignmentRef),
    decisionRef: safeDecisionRef(row.decision_id),
    sourceKind: 'artanis_admin_executor_trace_closeout',
    traceDigestPrefix,
    verdictCreatedAt: safeIso(row.verdict_created_at),
    verdictRef: 'verdict.artanis_closeout.verified',
  }
}

export const projectArtanisDistillationDatasetReceipt = (
  rows: ReadonlyArray<ArtanisDistillationDatasetRow>,
  nowIso: string,
): ArtanisDistillationDatasetReceipt => {
  const traces = rows.flatMap(row => {
    const trace = traceFromRow(row)

    return trace === null ? [] : [trace]
  })
  const sourceVerifiedTraceCount = traces.length
  const traceDigestPrefixes = [
    ...new Set(traces.map(trace => trace.traceDigestPrefix)),
  ].sort()
  const receiptAvailable =
    sourceVerifiedTraceCount >= ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET

  return {
    authorityBoundary:
      'Read-only refs-only dataset-curation receipt over accepted Artanis admin executor-trace closeouts. It grants no dispatch, spend, assignment, settlement, model-training, eval, model-promotion, or registry-transition authority. It does not expose raw trace bodies, private runner logs, prompts, provider payloads, wallet material, or customer data.',
    blockerRefs: receiptAvailable
      ? []
      : [ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER],
    clearsBlockerRefs: receiptAvailable
      ? [ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER]
      : [],
    closeoutReceiptRefs: traces.map(trace => trace.closeoutReceiptRef),
    datasetKind: 'tassadar_executor_verified_trace_refs',
    datasetRef: receiptAvailable ? datasetRef : null,
    generatedAt: nowIso,
    kind: 'artanis_tassadar_distillation_dataset_receipt',
    notes: [
      `A trace enters this refs-only dataset receipt only when its Artanis admin assignment carries an exact-replay verdict with outcome=verified and accept_state=accepted. The receipt requires at least ${ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET} accepted closeouts.`,
      'The dataset is a public-safe manifest of verified trace refs and digest prefixes. It is not a raw trace export, not a payout receipt, not a training run, and not a model capability claim.',
      'Each closeoutReceiptRefs entry is dereferenceable at /api/public/nexus-pylon/receipts/<receiptRef> for independent inspection of the accepted_work_verified closeout.',
    ],
    publicSafe: true,
    receiptRef: receiptAvailable ? datasetReceiptRef : null,
    receiptState: receiptAvailable
      ? 'available'
      : 'insufficient_verified_traces',
    requiredVerifiedTraceCount: ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
    sourceVerifiedTraceCount,
    staleness: ARTANIS_TASSADAR_DISTILLATION_DATASET_STALENESS,
    traceDigestPrefixes,
    traces,
    uniqueTraceDigestPrefixCount: traceDigestPrefixes.length,
  }
}

export const ARTANIS_TASSADAR_DISTILLATION_DATASET_MAX_LIMIT = 200

export const boundedDistillationDatasetLimit = (raw: string | null): number => {
  const parsed = Number(raw ?? '100')

  if (!Number.isFinite(parsed)) return 100

  return Math.min(
    Math.max(1, Math.trunc(parsed)),
    ARTANIS_TASSADAR_DISTILLATION_DATASET_MAX_LIMIT,
  )
}

export const readArtanisDistillationDatasetReceipt = async (
  db: D1Database,
  input: Readonly<{ limit: number; nowIso: string }>,
): Promise<ArtanisDistillationDatasetReceipt> => {
  const limit = boundedDistillationDatasetLimit(String(input.limit))
  const result = await db
    .prepare(
      `SELECT d.id AS decision_id,
              d.assignment_ref AS assignment_ref,
              d.created_at AS decision_created_at,
              v.created_at AS verdict_created_at,
              v.outcome AS verdict_outcome,
              v.accept_state AS verdict_accept_state,
              v.claimed_trace_digest_prefix AS verdict_trace_digest_prefix
         FROM artanis_admin_tick_decisions d
         JOIN artanis_closeout_verdicts v
           ON v.assignment_ref = d.assignment_ref
        WHERE d.state = 'dispatched'
          AND v.outcome = 'verified'
          AND v.accept_state = 'accepted'
        ORDER BY d.created_at ASC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  return projectArtanisDistillationDatasetReceipt(
    (result.results ?? []) as unknown as ReadonlyArray<ArtanisDistillationDatasetRow>,
    input.nowIso,
  )
}
