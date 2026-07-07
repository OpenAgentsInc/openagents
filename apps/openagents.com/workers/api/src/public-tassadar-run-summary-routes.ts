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
import { trainingWritesDatabaseForEnv } from './training-domain-store'
import {
  makeD1NexusTreasuryPayoutLedgerStore,
  type NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import { liveAtReadStaleness } from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import { settledSatsFromPaymentAuthorityReceipt } from './training-leaderboards'
import {
  type TrainingRunPublicSummary,
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
    bulletin: buildPublicTassadarRunBulletin({
      generatedAt,
      runRef,
      runState: 'planned',
      summary: null,
      windows: [],
      leases: [],
      challenges: [],
      settlementRows: [],
    }),
    metrics: {},
    realGradient: null,
  }) as const

const maxSettlementReceiptLookups = 128

type PublicTassadarRunBulletinInput = Readonly<{
  generatedAt: string
  runRef: string
  runState: string
  summary: TrainingRunPublicSummary | null
  windows: Awaited<ReturnType<TrainingAuthorityStore['listWindowsForRun']>>
  leases: Awaited<ReturnType<TrainingAuthorityStore['listWindowLeasesForRun']>>
  challenges: Awaited<
    ReturnType<TrainingAuthorityStore['listVerificationChallengesForRun']>
  >
  settlementRows: ReadonlyArray<PublicTassadarSettlementRow>
}>

const numberText = (value: number): string =>
  new Intl.NumberFormat('en-US').format(value)

const countText = (value: number, singular: string, plural = `${singular}s`) =>
  `${numberText(value)} ${value === 1 ? singular : plural}`

const metricValue = (
  metric: TrainingRunPublicSummary['metrics'][keyof TrainingRunPublicSummary['metrics']] | undefined,
): number =>
  metric === undefined || !Number.isFinite(metric.value) ? 0 : metric.value

const distinctLeasePylons = (
  leases: PublicTassadarRunBulletinInput['leases'],
): ReadonlyArray<string> =>
  uniqueRefs(leases.map(lease => lease.pylonRef))

const activeLeasePylons = (
  leases: PublicTassadarRunBulletinInput['leases'],
): ReadonlyArray<string> =>
  uniqueRefs(
    leases
      .filter(lease => lease.state === 'active')
      .map(lease => lease.pylonRef),
  )

const latestTimestamp = (
  values: ReadonlyArray<string | null | undefined>,
): string | undefined =>
  values
    .map(value => value?.trim() ?? '')
    .filter(value => value !== '')
    .sort()
    .at(-1)

export const buildPublicTassadarRunBulletin = (
  input: PublicTassadarRunBulletinInput,
) => {
  const metrics = input.summary?.metrics
  const totalPylons = Math.max(
    distinctLeasePylons(input.leases).length,
    metricValue(metrics?.assignedContributorCount),
    metricValue(metrics?.qualifiedContributorCount),
  )
  const activePylons = activeLeasePylons(input.leases).length
  const activeWindows = metricValue(metrics?.activeWindowCount)
  const acceptedTraces = input.summary?.corpus.acceptedTraceCount ?? 0
  const verifiedWork = metricValue(metrics?.verifiedWorkCount)
  const rejectedWork = metricValue(metrics?.rejectedWorkCount)
  const settledSats = metricValue(metrics?.providerConfirmedSettledPayoutSats)
  const realSettlementRows = input.settlementRows.filter(
    row => row.realBitcoinMoved,
  )
  const latestRunActivityAt =
    latestTimestamp([
      ...input.windows.map(window => window.updatedAt),
      ...input.leases.map(lease => lease.claimedAt),
      ...input.challenges.map(challenge => challenge.updatedAt),
    ]) ?? input.generatedAt
  const status =
    input.runState === 'active'
      ? 'active'
      : input.runState === 'sealed' || input.runState === 'reconciled'
        ? 'sealed'
        : input.runState === 'planned'
          ? 'planned'
          : input.runState
  const pylonLine = `${countText(totalPylons, 'pylon')}, ${numberText(
    activePylons,
  )} active`
  const workLine = `${countText(
    acceptedTraces,
    'accepted trace',
  )}; ${numberText(verifiedWork)} verified, ${numberText(rejectedWork)} rejected`
  const settlementLine =
    settledSats > 0
      ? `${numberText(settledSats)} sats paid across ${countText(
          realSettlementRows.length,
          'real settlement',
        )}`
      : 'no provider-confirmed real Bitcoin settlement counted yet'
  const headline = `Tassadar is ${status}: ${pylonLine}.`
  const body = [
    headline,
    activeWindows > 0
      ? `${countText(
          activeWindows,
          'training window',
        )} active right now.`
      : 'No active training window is visible in the public projection right now.',
    workLine,
    settlementLine,
  ].join(' ')

  return {
    schemaVersion: 'openagents.public_tassadar_run_bulletin.v1',
    title: 'Tassadar Run Board',
    headline,
    summary: body,
    statusLine: `${status} · ${pylonLine}`,
    onBoardLines: [
      `Status: ${status}`,
      pylonLine,
      settledSats > 0 ? `${numberText(settledSats)} sats paid` : 'settlement pending',
    ],
    metrics: {
      acceptedTraceCount: acceptedTraces,
      activePylonCount: activePylons,
      activeWindowCount: activeWindows,
      realSettlementCount: realSettlementRows.length,
      settledSats,
      totalPylonCount: totalPylons,
      verifiedWorkCount: verifiedWork,
    },
    latestActivity: [
      {
        label: 'latest update',
        text:
          input.summary === null
            ? 'The run has no public Worker-authoritative records yet.'
            : `Latest public run activity was recorded at ${latestRunActivityAt}.`,
        occurredAt: latestRunActivityAt,
        sourceRefs: uniqueRefs([
          input.runRef,
          'route:/api/public/tassadar-run-summary',
        ]),
      },
      ...(realSettlementRows.length === 0
        ? []
        : [
            {
              label: 'bitcoin settlement',
              text: settlementLine,
              occurredAt: input.generatedAt,
              sourceRefs: uniqueRefs(
                realSettlementRows.flatMap(row => row.sourceRefs),
              ),
            },
          ]),
    ],
    sourceRefs: uniqueRefs([
      input.runRef,
      'route:/api/public/tassadar-run-summary',
      ...(input.summary?.sourceRefs ?? []),
      ...(input.summary?.receiptRefs ?? []),
    ]),
  } as const
}

export type PublicTassadarSettlementRow = Readonly<{
  amountSats: number
  apiUrl: string
  contributorRef: string | null
  movementMode: 'real_bitcoin' | 'simulation'
  realBitcoinMoved: boolean
  receiptKind: string
  receiptPageUrl: string
  receiptRef: string
  sourceRefs: ReadonlyArray<string>
  state: string
  trainingRunRef: string | null
  verificationChallengeRef: string | null
}>

type RunSettlementInfo = Readonly<{
  contributorRef: string | null
  row: PublicTassadarSettlementRow | null
  settledSats: number
}>

type RunSettlementResolution = Readonly<{
  settlementRows: ReadonlyArray<PublicTassadarSettlementRow>
  settledSatsByReceiptRef: ReadonlyMap<string, number>
  settlementReceiptRefsByContributor: ReadonlyMap<string, ReadonlyArray<string>>
}>

const emptyRunSettlementResolution: RunSettlementResolution = {
  settlementRows: [],
  settledSatsByReceiptRef: new Map<string, number>(),
  settlementReceiptRefsByContributor: new Map<string, ReadonlyArray<string>>(),
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const optionalRef = (ref: string | null | undefined): ReadonlyArray<string> =>
  typeof ref === 'string' && ref.trim() !== '' ? [ref] : []

const settlementInfoFromReceipt = (
  input: Readonly<{
    appUrl: string
    eventStatus?: string | undefined
    receiptRef: string
    record: Readonly<{
      eventRef?: string | null
      payoutAttemptRef?: string | null
      payoutIntentRef?: string | null
      publicProjectionJson: string
      receiptKind: string
    }>
  }>,
): RunSettlementInfo => {
  const settledSats = settledSatsFromPaymentAuthorityReceipt(input.record)
  const projection = parseJsonRecord(input.record.publicProjectionJson)
  const contributorRef =
    typeof projection?.contributorRef === 'string'
      ? projection.contributorRef
      : null
  const amountSats =
    typeof projection?.amountSats === 'number' &&
    Number.isInteger(projection.amountSats) &&
    projection.amountSats > 0
      ? projection.amountSats
      : settledSats
  const movementMode =
    projection?.movementMode === 'real_bitcoin' ||
    projection?.moneyMovement === 'real_bitcoin'
      ? 'real_bitcoin'
      : 'simulation'
  const state = typeof projection?.state === 'string' ? projection.state : 'unknown'
  const trainingRunRef =
    typeof projection?.trainingRunRef === 'string'
      ? projection.trainingRunRef
      : null
  const verificationChallengeRef =
    typeof projection?.verificationChallengeRef === 'string'
      ? projection.verificationChallengeRef
      : null
  const realBitcoinMoved =
    movementMode === 'real_bitcoin' &&
    input.record.receiptKind === 'settlement_recorded' &&
    (input.eventStatus === 'matched' ||
      projection?.realBitcoinMoved === true)
  const row: PublicTassadarSettlementRow = {
    amountSats,
    apiUrl: `${input.appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(
      input.receiptRef,
    )}`,
    contributorRef,
    movementMode,
    realBitcoinMoved,
    receiptKind: input.record.receiptKind,
    receiptPageUrl: `${input.appUrl}/nexus-pylon/receipts/${encodeURIComponent(
      input.receiptRef,
    )}`,
    receiptRef: input.receiptRef,
    sourceRefs: uniqueRefs([
      input.receiptRef,
      ...optionalRef(input.record.eventRef),
      ...optionalRef(input.record.payoutAttemptRef),
      ...optionalRef(input.record.payoutIntentRef),
      ...optionalRef(contributorRef),
      ...optionalRef(trainingRunRef),
      ...optionalRef(verificationChallengeRef),
    ]),
    state,
    trainingRunRef,
    verificationChallengeRef,
  }

  return { contributorRef, row, settledSats }
}

export const resolveRunSettlements = async (
  payoutLedgerStore: NexusTreasuryPayoutLedgerStore | undefined,
  receiptRefs: ReadonlyArray<string>,
  appUrl: string,
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
      const event =
        record?.eventRef === null || record?.eventRef === undefined
          ? undefined
          : await payoutLedgerStore.readReconciliationEventByRef(record.eventRef)

      return [
        receiptRef,
        record === undefined
          ? { contributorRef: null, row: null, settledSats: 0 }
          : settlementInfoFromReceipt({
              appUrl,
              eventStatus: event?.status,
              receiptRef,
              record,
            }),
      ] as const
    }),
  )
  const settled = entries.filter(([, info]) => info.settledSats > 0)

  return {
    settlementRows: entries.flatMap(([, info]) =>
      info.row === null ? [] : [info.row],
    ),
    settledSatsByReceiptRef: new Map<string, number>(
      // Real-money settled total: count ONLY receipts where bitcoin actually
      // moved. A settled-STATE simulation receipt (movementMode:simulation /
      // realBitcoinMoved:false) must NOT inflate the real settled-sats total
      // (it still appears in settlementRows, flagged). Fixes the sim-vs-real
      // conflation Orrery dereferenced (1000+5 real = 1005, not 1010).
      settled
        .filter(([, info]) => info.row?.realBitcoinMoved === true)
        .map(([receiptRef, info]) => [receiptRef, info.settledSats]),
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
  appUrl = 'https://openagents.com',
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
  ], appUrl)
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
    bulletin: buildPublicTassadarRunBulletin({
      generatedAt,
      runRef: run.trainingRunRef,
      runState: run.state,
      summary,
      windows,
      leases,
      challenges,
      settlementRows: settlement.settlementRows,
    }),
    ...summary,
    settlementRows: settlement.settlementRows,
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
  // #8515 D1 evacuation: default BOTH reads off the 401-dead D1 bridge onto
  // the Postgres-backed D1 adapter (this is the route that was live-500ing).
  // The training store rides its own KHALA_SYNC_TRAINING_WRITES gate; the
  // payout-ledger read reuses the money factory UNCHANGED, just handed the
  // same Postgres handle (READ-ONLY here — no money-domain logic is touched),
  // falling back to plain D1 only when the KHALA_SYNC_DB binding is absent.
  const makeStore =
    deps.makeStore ??
    (e => makeD1TrainingAuthorityStore(trainingWritesDatabaseForEnv(e)))
  const makePayoutLedgerStore =
    deps.makePayoutLedgerStore ??
    (e =>
      makeD1NexusTreasuryPayoutLedgerStore(trainingWritesDatabaseForEnv(e)))
  const generatedAt = (deps.now ?? currentIsoTimestamp)()
  const runRef =
    new URL(request.url).searchParams.get('run')?.trim() ||
    DEFAULT_TASSADAR_RUN_REF
  const appUrl = new URL(request.url).origin
  return buildPublicTassadarRunSummaryEnvelope(
    makeStore(env),
    runRef,
    generatedAt,
    makePayoutLedgerStore(env),
    appUrl,
  )
}
