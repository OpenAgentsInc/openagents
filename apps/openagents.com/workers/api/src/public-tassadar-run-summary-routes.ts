// Public read for the live Tassadar run summary (#5114, epic #5112).
//
// Serves the public-safe `TrainingRunPublicSummary` for the live executor run so
// the data-bound 3D "living run" view (#5118) — built on the #5113 snapshot
// adapter — can fetch real run state with NO admin auth.
//
// Public-safe by construction: `publicTrainingRunSummary` is the public projection
// (metrics carry provenance, refs are redacted, no private material). RECEIPT-FIRST:
// a run that is not found or has no data returns an honest idle envelope (zeroed,
// `planned`), never a faked value.
import { liveAtReadStaleness } from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TrainingAuthorityStore,
  makeD1TrainingAuthorityStore,
  publicTrainingRunSummary,
} from './training-run-window-authority'

export const DEFAULT_TASSADAR_RUN_REF = 'run.tassadar.executor.20260615'
export const PublicTassadarRunSummarySchemaVersion =
  'openagents.public_tassadar_run_summary.v1'
const publicTassadarRunSummaryStaleness = () =>
  liveAtReadStaleness([
    'training_run_state_transition_recorded',
    'training_window_state_transition_recorded',
    'training_run_evidence_attached',
  ])

const idleEnvelope = (runRef: string, generatedAt: string) =>
  ({
    schemaVersion: PublicTassadarRunSummarySchemaVersion,
    runRef,
    runState: 'planned',
    generatedAt,
    staleness: publicTassadarRunSummaryStaleness(),
    emptyState: { idle: true, reason: 'run not found or no data yet' },
    metrics: {},
    realGradient: null,
  }) as const

/**
 * Load the run's records and build the public summary envelope the 3D view
 * consumes (the #5113 adapter reads `runRef`/`runState`/`emptyState`/`metrics`/
 * `realGradient`). Pure aside from the injected store; honest idle when absent.
 */
export const buildPublicTassadarRunSummaryEnvelope = async (
  store: TrainingAuthorityStore,
  runRef: string,
  generatedAt: string,
): Promise<Record<string, unknown>> => {
  const run = await store.readRun(runRef)
  if (run === undefined) return { ...idleEnvelope(runRef, generatedAt) }

  const [windows, leases, challenges] = await Promise.all([
    store.listWindowsForRun(runRef, 100),
    store.listWindowLeasesForRun(runRef, 1000),
    store.listVerificationChallengesForRun(runRef, 1000),
  ])
  const summary = publicTrainingRunSummary({
    challenges,
    leases,
    nowIso: generatedAt,
    run,
    windows,
  })
  return {
    schemaVersion: PublicTassadarRunSummarySchemaVersion,
    runRef: run.trainingRunRef,
    runState: run.state,
    generatedAt,
    ...summary,
    staleness: summary.run.staleness,
  }
}

export const buildPublicTassadarRunSummaryEnvelopeForRequest = async (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
  deps: {
    readonly makeStore?: (
      env: Parameters<typeof openAgentsDatabase>[0],
    ) => TrainingAuthorityStore
    readonly now?: () => string
  } = {},
): Promise<Record<string, unknown>> => {
  const makeStore =
    deps.makeStore ?? (e => makeD1TrainingAuthorityStore(openAgentsDatabase(e)))
  const generatedAt = (deps.now ?? currentIsoTimestamp)()
  const runRef =
    new URL(request.url).searchParams.get('run')?.trim() ||
    DEFAULT_TASSADAR_RUN_REF
  return buildPublicTassadarRunSummaryEnvelope(
    makeStore(env),
    runRef,
    generatedAt,
  )
}
