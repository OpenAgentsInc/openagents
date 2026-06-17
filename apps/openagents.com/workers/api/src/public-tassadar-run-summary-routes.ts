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
import { parseJsonRecord } from './json-boundary'
import {
  makeD1NexusTreasuryPayoutLedgerStore,
  type NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import { liveAtReadStaleness } from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import { settledSatsFromPaymentAuthorityReceipt } from './training-leaderboards'
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

const maxSettlementReceiptLookups = 128

type RunSettlementInfo = Readonly<{
  contributorRef: string | null
  settledSats: number
}>

type RunSettlementResolution = Readonly<{
  settledSatsByReceiptRef: ReadonlyMap<string, number>
  settlementReceiptRefsByContributor: ReadonlyMap<string, ReadonlyArray<string>>
}>

const emptyRunSettlementResolution: RunSettlementResolution = {
  settledSatsByReceiptRef: new Map<string, number>(),
  settlementReceiptRefsByContributor: new Map<string, ReadonlyArray<string>>(),
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const settlementInfoFromReceipt = (
  record: Readonly<{ publicProjectionJson: string; receiptKind: string }>,
): RunSettlementInfo => {
  const settledSats = settledSatsFromPaymentAuthorityReceipt(record)

  if (settledSats <= 0) {
    return { contributorRef: null, settledSats: 0 }
  }

  const projection = parseJsonRecord(record.publicProjectionJson)
  const contributorRef =
    typeof projection?.contributorRef === 'string'
      ? projection.contributorRef
      : null

  return { contributorRef, settledSats }
}

const resolveRunSettlements = async (
  payoutLedgerStore: NexusTreasuryPayoutLedgerStore | undefined,
  receiptRefs: ReadonlyArray<string>,
): Promise<RunSettlementResolution> => {
  if (payoutLedgerStore === undefined) {
    return emptyRunSettlementResolution
  }

  const refs = uniqueRefs(receiptRefs).slice(0, maxSettlementReceiptLookups)

  if (refs.length === 0) {
    return emptyRunSettlementResolution
  }

  const entries = await Promise.all(
    refs.map(async receiptRef => {
      const record =
        await payoutLedgerStore.readPaymentAuthorityReceiptByRef(receiptRef)

      return [
        receiptRef,
        record === undefined
          ? { contributorRef: null, settledSats: 0 }
          : settlementInfoFromReceipt(record),
      ] as const
    }),
  )
  const settled = entries.filter(([, info]) => info.settledSats > 0)

  return {
    settledSatsByReceiptRef: new Map<string, number>(
      settled.map(([receiptRef, info]) => [receiptRef, info.settledSats]),
    ),
    settlementReceiptRefsByContributor: settled.reduce(
      (byContributor, [receiptRef, info]) => {
        if (info.contributorRef === null) {
          return byContributor
        }

        byContributor.set(info.contributorRef, [
          ...(byContributor.get(info.contributorRef) ?? []),
          receiptRef,
        ])

        return byContributor
      },
      new Map<string, ReadonlyArray<string>>(),
    ),
  }
}

/**
 * Load the run's records and build the public summary envelope the 3D view
 * consumes (the #5113 adapter reads `runRef`/`runState`/`emptyState`/`metrics`/
 * `realGradient`). Pure aside from the injected store; honest idle when absent.
 */
export const buildPublicTassadarRunSummaryEnvelope = async (
  store: TrainingAuthorityStore,
  runRef: string,
  generatedAt: string,
  payoutLedgerStore?: NexusTreasuryPayoutLedgerStore,
): Promise<Record<string, unknown>> => {
  const run = await store.readRun(runRef)
  if (run === undefined) return { ...idleEnvelope(runRef, generatedAt) }

  const [windows, leases, challenges] = await Promise.all([
    store.listWindowsForRun(runRef, 100),
    store.listWindowLeasesForRun(runRef, 1000),
    store.listVerificationChallengesForRun(runRef, 1000),
  ])
  const settlement = await resolveRunSettlements(payoutLedgerStore, [
    ...run.receiptRefs,
    ...windows.flatMap(window => window.receiptRefs),
    ...leases.flatMap(lease => lease.receiptRefs),
    ...challenges.flatMap(challenge => challenge.verdictRefs),
  ])
  const summary = publicTrainingRunSummary({
    challenges,
    leases,
    nowIso: generatedAt,
    run,
    settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
    settlementReceiptRefsByContributor:
      settlement.settlementReceiptRefsByContributor,
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
    readonly makePayoutLedgerStore?: (
      env: Parameters<typeof openAgentsDatabase>[0],
    ) => NexusTreasuryPayoutLedgerStore
    readonly makeStore?: (
      env: Parameters<typeof openAgentsDatabase>[0],
    ) => TrainingAuthorityStore
    readonly now?: () => string
  } = {},
): Promise<Record<string, unknown>> => {
  const makeStore =
    deps.makeStore ?? (e => makeD1TrainingAuthorityStore(openAgentsDatabase(e)))
  const makePayoutLedgerStore =
    deps.makePayoutLedgerStore ??
    (e => makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(e)))
  const generatedAt = (deps.now ?? currentIsoTimestamp)()
  const runRef =
    new URL(request.url).searchParams.get('run')?.trim() ||
    DEFAULT_TASSADAR_RUN_REF
  return buildPublicTassadarRunSummaryEnvelope(
    makeStore(env),
    runRef,
    generatedAt,
    makePayoutLedgerStore(env),
  )
}
