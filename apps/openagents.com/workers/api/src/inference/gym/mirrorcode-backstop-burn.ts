import {
  buildMirrorCodeTokenBurnReport,
  KHALA_MODEL_ID,
  MIRRORCODE_DEMAND_KIND,
  MIRRORCODE_DEMAND_SOURCE,
  MIRRORCODE_MEMORY_POLICY,
  MIRRORCODE_PUBLIC_TARGETS_BY_BUCKET,
  type MirrorCodeBucket,
  type MirrorCodeRun,
} from './mirrorcode-contract'

export const MirrorCodeBackstopBurnSchemaVersion =
  'openagents.gym.mirrorcode_backstop_burn.v1'

export const MIRRORCODE_BACKSTOP_ISSUE_NUMBER = 6710
export const MIRRORCODE_BACKSTOP_TASK_REF =
  'issue.public.openagents.6710.mirrorcode_backstop_burn'
export const MIRRORCODE_BACKSTOP_WORKFLOW_REF =
  'workflow.public.gym.mirrorcode.backstop_burn.6710'
export const MIRRORCODE_BACKSTOP_LEDGER_REF =
  'ledger.public.gym.mirrorcode.backstop_burn.6710'

const DEFAULT_BATCH_REF = 'batch.public.gym.mirrorcode.backstop_burn.next'
const DEFAULT_LANGUAGE = 'python'
const DEFAULT_BATCH_SIZE = 3
const BUCKET_ORDER: ReadonlyArray<MirrorCodeBucket> = ['S', 'M', 'L']

export type MirrorCodeBackstopTaskPlan = Readonly<{
  taskRef: string
  runId: string
  taskId: string
  bucket: MirrorCodeBucket
  language: string
  grade: 'smoke'
  model: typeof KHALA_MODEL_ID
  demandKind: typeof MIRRORCODE_DEMAND_KIND
  demandSource: typeof MIRRORCODE_DEMAND_SOURCE
  memoryPolicy: typeof MIRRORCODE_MEMORY_POLICY
  ledgerTraceRef: string
}>

export type MirrorCodeBackstopBatchPlan = Readonly<{
  schemaVersion: typeof MirrorCodeBackstopBurnSchemaVersion
  issueNumber: typeof MIRRORCODE_BACKSTOP_ISSUE_NUMBER
  taskRef: typeof MIRRORCODE_BACKSTOP_TASK_REF
  workflowRef: typeof MIRRORCODE_BACKSTOP_WORKFLOW_REF
  batchRef: string
  demandKind: typeof MIRRORCODE_DEMAND_KIND
  demandSource: typeof MIRRORCODE_DEMAND_SOURCE
  memoryPolicy: typeof MIRRORCODE_MEMORY_POLICY
  taskCount: number
  tasks: ReadonlyArray<MirrorCodeBackstopTaskPlan>
  ledgerRef: typeof MIRRORCODE_BACKSTOP_LEDGER_REF
  caveatRefs: ReadonlyArray<string>
}>

export type MirrorCodeBackstopLedgerTrace = Readonly<{
  traceRef: string
  runId: string
  taskRef: string
  taskId: string
  bucket: MirrorCodeBucket
  status: MirrorCodeRun['status']
  grade: MirrorCodeRun['grade']
  passRateBps: number | null
  tokensTotal: number
  exactTokenUsageEventRefs: ReadonlyArray<string>
  tokenAttributionProofRef: string
}>

export type MirrorCodeBackstopBurnReport = Readonly<{
  schemaVersion: typeof MirrorCodeBackstopBurnSchemaVersion
  issueNumber: typeof MIRRORCODE_BACKSTOP_ISSUE_NUMBER
  taskRef: typeof MIRRORCODE_BACKSTOP_TASK_REF
  workflowRef: typeof MIRRORCODE_BACKSTOP_WORKFLOW_REF
  ledgerRef: typeof MIRRORCODE_BACKSTOP_LEDGER_REF
  demandKind: typeof MIRRORCODE_DEMAND_KIND
  demandSource: typeof MIRRORCODE_DEMAND_SOURCE
  memoryPolicy: typeof MIRRORCODE_MEMORY_POLICY
  runCount: number
  terminalRunCount: number
  passedRunCount: number
  passRateBps: number | null
  totalTokensBurned: number
  exactTokenBackedTokens: number
  ledgerTraces: ReadonlyArray<MirrorCodeBackstopLedgerTrace>
  tokenBurnReport: ReturnType<typeof buildMirrorCodeTokenBurnReport>
  caveatRefs: ReadonlyArray<string>
}>

export type BuildMirrorCodeBackstopBatchPlanInput = Readonly<{
  batchRef?: string
  maxTasks?: number
  languages?: ReadonlyArray<string>
  completedTaskRefs?: ReadonlyArray<string>
  activeTaskRefs?: ReadonlyArray<string>
}>

const publicSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')

const taskKey = (
  bucket: MirrorCodeBucket,
  taskId: string,
  language: string,
): string => `${bucket}:${taskId}:${language}`

const taskRefFor = (
  bucket: MirrorCodeBucket,
  taskId: string,
  language: string,
): string =>
  `task.public.gym.mirrorcode.${bucket.toLowerCase()}.${publicSegment(taskId)}.${publicSegment(language)}`

const runIdFor = (
  batchRef: string,
  bucket: MirrorCodeBucket,
  taskId: string,
  language: string,
): string =>
  [
    'mc-backstop',
    publicSegment(batchRef).slice(0, 32),
    bucket.toLowerCase(),
    publicSegment(taskId),
    publicSegment(language),
  ].join('-')

const boundedBatchSize = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return DEFAULT_BATCH_SIZE
  }
  return Math.max(1, Math.min(Math.floor(value), 12))
}

const normalizedLanguages = (
  languages: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const values = languages
    ?.map(language => publicSegment(language))
    .filter(language => language.length > 0 && language.length <= 32)
  return values === undefined || values.length === 0
    ? [DEFAULT_LANGUAGE]
    : [...new Set(values)]
}

export const buildMirrorCodeBackstopBatchPlan = (
  input: BuildMirrorCodeBackstopBatchPlanInput = {},
): MirrorCodeBackstopBatchPlan => {
  const batchRef = input.batchRef ?? DEFAULT_BATCH_REF
  const skippedRefs = new Set([
    ...(input.completedTaskRefs ?? []),
    ...(input.activeTaskRefs ?? []),
  ])
  const languages = normalizedLanguages(input.languages)
  const candidates = BUCKET_ORDER.flatMap(bucket =>
    MIRRORCODE_PUBLIC_TARGETS_BY_BUCKET[bucket].flatMap(taskId =>
      languages.map(language => ({ bucket, taskId, language })),
    ),
  )
  const tasks = candidates
    .filter(
      candidate =>
        !skippedRefs.has(
          taskKey(candidate.bucket, candidate.taskId, candidate.language),
        ),
    )
    .slice(0, boundedBatchSize(input.maxTasks))
    .map(candidate => {
      const taskRef = taskRefFor(
        candidate.bucket,
        candidate.taskId,
        candidate.language,
      )
      return {
        taskRef,
        runId: runIdFor(
          batchRef,
          candidate.bucket,
          candidate.taskId,
          candidate.language,
        ),
        taskId: candidate.taskId,
        bucket: candidate.bucket,
        language: candidate.language,
        grade: 'smoke' as const,
        model: KHALA_MODEL_ID as typeof KHALA_MODEL_ID,
        demandKind: MIRRORCODE_DEMAND_KIND as typeof MIRRORCODE_DEMAND_KIND,
        demandSource:
          MIRRORCODE_DEMAND_SOURCE as typeof MIRRORCODE_DEMAND_SOURCE,
        memoryPolicy: MIRRORCODE_MEMORY_POLICY as typeof MIRRORCODE_MEMORY_POLICY,
        ledgerTraceRef: `trace.public.gym.mirrorcode.backstop.${publicSegment(taskRef)}`,
      }
    })

  return {
    schemaVersion: MirrorCodeBackstopBurnSchemaVersion,
    issueNumber: MIRRORCODE_BACKSTOP_ISSUE_NUMBER,
    taskRef: MIRRORCODE_BACKSTOP_TASK_REF,
    workflowRef: MIRRORCODE_BACKSTOP_WORKFLOW_REF,
    batchRef,
    demandKind: MIRRORCODE_DEMAND_KIND,
    demandSource: MIRRORCODE_DEMAND_SOURCE,
    memoryPolicy: MIRRORCODE_MEMORY_POLICY,
    taskCount: tasks.length,
    tasks,
    ledgerRef: MIRRORCODE_BACKSTOP_LEDGER_REF,
    caveatRefs: [
      'caveat.public.gym.mirrorcode.backstop_public_targets_only',
      'caveat.public.gym.mirrorcode.backstop_internal_own_capacity',
      'caveat.public.gym.mirrorcode.backstop_exact_rows_required_for_claims',
    ],
  }
}

const ledgerTraceForRun = (
  run: MirrorCodeRun,
): MirrorCodeBackstopLedgerTrace => ({
  traceRef: `trace.public.gym.mirrorcode.backstop.${publicSegment(run.runId)}`,
  runId: run.runId,
  taskRef: taskRefFor(run.bucket, run.taskId, run.language ?? DEFAULT_LANGUAGE),
  taskId: run.taskId,
  bucket: run.bucket,
  status: run.status,
  grade: run.grade,
  passRateBps: run.passRate === null ? null : Math.round(run.passRate * 10_000),
  tokensTotal: run.tokensTotal,
  exactTokenUsageEventRefs: run.exactTokenUsageEventRefs,
  tokenAttributionProofRef: run.tokenAttributionProofRef,
})

export const buildMirrorCodeBackstopBurnReport = (
  runs: ReadonlyArray<MirrorCodeRun>,
): MirrorCodeBackstopBurnReport => {
  const terminalRuns = runs.filter(run =>
    ['passed', 'failed', 'error'].includes(run.status),
  )
  const passedRuns = runs.filter(run => run.status === 'passed')
  const tokenBurnReport = buildMirrorCodeTokenBurnReport(runs)

  return {
    schemaVersion: MirrorCodeBackstopBurnSchemaVersion,
    issueNumber: MIRRORCODE_BACKSTOP_ISSUE_NUMBER,
    taskRef: MIRRORCODE_BACKSTOP_TASK_REF,
    workflowRef: MIRRORCODE_BACKSTOP_WORKFLOW_REF,
    ledgerRef: MIRRORCODE_BACKSTOP_LEDGER_REF,
    demandKind: MIRRORCODE_DEMAND_KIND,
    demandSource: MIRRORCODE_DEMAND_SOURCE,
    memoryPolicy: MIRRORCODE_MEMORY_POLICY,
    runCount: runs.length,
    terminalRunCount: terminalRuns.length,
    passedRunCount: passedRuns.length,
    passRateBps:
      terminalRuns.length === 0
        ? null
        : Math.round((passedRuns.length / terminalRuns.length) * 10_000),
    totalTokensBurned: tokenBurnReport.totalTokensBurned,
    exactTokenBackedTokens: tokenBurnReport.exactTokenBackedTokens,
    ledgerTraces: runs.map(ledgerTraceForRun),
    tokenBurnReport,
    caveatRefs: [
      'caveat.public.gym.mirrorcode.backstop_ledger_refs_are_public_summaries',
      'caveat.public.gym.mirrorcode.backstop_no_private_task_contents',
      'caveat.public.gym.mirrorcode.backstop_decision_claims_need_exact_rows',
    ],
  }
}
