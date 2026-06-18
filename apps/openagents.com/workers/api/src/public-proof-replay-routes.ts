import {
  buildPublicTassadarRunSummaryEnvelope,
  DEFAULT_TASSADAR_RUN_REF,
  type PublicTassadarSettlementRow,
} from './public-tassadar-run-summary-routes'
import { liveAtReadStaleness } from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import type { NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  makeD1NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import {
  makeD1TrainingAuthorityStore,
  type TrainingAuthorityStore,
} from './training-run-window-authority'

export const ProofReplayBundleSchemaVersion = 'proof_replay_bundle.v1'
export const FIRST_REAL_SETTLEMENT_BUNDLE_SLUG = 'first-real-settlement'
export const FIRST_REAL_SETTLEMENT_TITLE =
  'Tassadar Run 1: First Real Bitcoin Settlement'
export const FIRST_REAL_SETTLEMENT_LOCAL_DISPLAY_TIME = '8:38pm, June 17'
export const FIRST_REAL_SETTLEMENT_OBSERVED_AT = '2026-06-18T01:38:00.000Z'

export const FIRST_REAL_SETTLEMENT_RECEIPT_REF =
  'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618'
export const FIRST_REAL_SETTLEMENT_CHALLENGE_REF =
  'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c'
export const FIRST_REAL_SETTLEMENT_WINDOW_REF =
  'training.window.tassadar.executor.20260615.w1'
export const FIRST_REAL_SETTLEMENT_CONTRIBUTOR_REF =
  'pylon.448ba824b5fc879f3a59'
export const FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL =
  'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa'
export const FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL =
  'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-a8df2265-547a-4a18-9398-3e7412a6859a'

type SourceKind =
  | 'api'
  | 'forum_post'
  | 'pylon'
  | 'receipt'
  | 'run'
  | 'window'
  | 'verification_challenge'
  | 'payment_authority'
  | 'operator_context'

type ReplayEventKind =
  | 'actor_entered_region'
  | 'proof_submitted'
  | 'proof_verified'
  | 'claim_boundary_shown'
  | 'settlement_blocked_closed'
  | 'payout_intent_persisted'
  | 'settlement_recorded'
  | 'payment_zap_confirmed'
  | 'payment_zap_simulated'
  | 'forum_announcement_posted'

type ReplaySourceRef = Readonly<{
  ref: string
  kind: SourceKind
  url?: string
  observedAt?: string
}>

type ReplayActor = Readonly<{
  actorRef: string
  avatarRole:
    | 'contributor'
    | 'validator'
    | 'settlement_terminal'
    | 'operator_gate'
    | 'announcer'
  displayName: string
  pylonRef?: string
  fallbackAssetId: string
}>

type ReplayStage = Readonly<{
  stageRef: string
  stageKind:
    | 'run_core'
    | 'pylon_station'
    | 'proof_gate'
    | 'settlement_terminal'
    | 'registry_marker'
    | 'replay_gap'
  label: string
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayEvent = Readonly<{
  eventRef: string
  kind: ReplayEventKind
  sequenceIndex: number
  timelineSecond: number
  observedAt?: string
  actorRefs: ReadonlyArray<string>
  targetRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  displayText: string
  stateBefore?: string
  stateAfter?: string
  amountSats?: number
  rail?: string
  caveat?: string
}>

type ReplayFlow = Readonly<{
  flowRef: string
  flowKind:
    | 'work_handoff'
    | 'verification_check'
    | 'receipt_emission'
    | 'payment_movement'
    | 'simulation_marker'
  fromRef: string
  toRef: string
  sourceRefs: ReadonlyArray<string>
  amountSats?: number
  rail?: string
}>

type ReplayCameraCue = Readonly<{
  cueRef: string
  mode:
    | 'overview'
    | 'follow_actor'
    | 'orbit_proof'
    | 'zap_focus'
    | 'director_track'
  startSecond: number
  durationSecond: number
  focusRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

type ReplayCaption = Readonly<{
  captionRef: string
  sequenceIndex: number
  timelineSecond: number
  text: string
  sourceRefs: ReadonlyArray<string>
}>

type ReplayGap = Readonly<{
  gapRef: string
  reason: string
  affectedRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type ProofReplayBundle = Readonly<{
  bundleRef: string
  schemaVersion: typeof ProofReplayBundleSchemaVersion
  generatedAt: string
  title: string
  socialDisplayTime: string
  sourceRefs: ReadonlyArray<ReplaySourceRef>
  sourceAuthority: 'worker_d1_public'
  staleness: ReturnType<typeof liveAtReadStaleness>
  privacyLevel: 'public_safe'
  claimScope: 'evidence_presentation_only'
  actors: ReadonlyArray<ReplayActor>
  stages: ReadonlyArray<ReplayStage>
  events: ReadonlyArray<ReplayEvent>
  flows: ReadonlyArray<ReplayFlow>
  cameraCues: ReadonlyArray<ReplayCameraCue>
  captions: ReadonlyArray<ReplayCaption>
  gaps: ReadonlyArray<ReplayGap>
}>

type PublicTassadarSummaryEnvelope = Readonly<{
  runRef?: unknown
  settlementRows?: unknown
}>

type Deps = Readonly<{
  makePayoutLedgerStore?: (
    env: Parameters<typeof openAgentsDatabase>[0],
  ) => NexusTreasuryPayoutLedgerStore
  makeStore?: (
    env: Parameters<typeof openAgentsDatabase>[0],
  ) => TrainingAuthorityStore
  now?: () => string
}>

class ProofReplayPublicProjectionUnsafe extends Error {
  override readonly name = 'ProofReplayPublicProjectionUnsafe'
}

const replayBundleStaleness = () =>
  liveAtReadStaleness([
    'training_run_state_transition_recorded',
    'training_window_state_transition_recorded',
    'training_verification_challenge_recorded',
    'nexus_payment_authority_receipt_recorded',
    'nexus_reconciliation_event_recorded',
  ])

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]
    .sort()

const optionalRef = (value: string | null | undefined): ReadonlyArray<string> =>
  typeof value === 'string' && value.trim() !== '' ? [value] : []

const sourceKindForRef = (ref: string): SourceKind => {
  if (ref.startsWith('https://openagents.com/forum/')) {
    return 'forum_post'
  }

  if (ref.startsWith('https://openagents.com/api/')) {
    return 'api'
  }

  if (ref.startsWith('receipt.')) {
    return 'receipt'
  }

  if (
    ref.startsWith('payout_intent.') ||
    ref.startsWith('payout_attempt.') ||
    ref.startsWith('reconciliation.') ||
    ref.startsWith('external_event.')
  ) {
    return 'payment_authority'
  }

  if (ref.startsWith('training.verification.challenge.')) {
    return 'verification_challenge'
  }

  if (ref.startsWith('training.window.')) {
    return 'window'
  }

  if (ref.startsWith('run.')) {
    return 'run'
  }

  if (ref.startsWith('pylon.')) {
    return 'pylon'
  }

  return 'operator_context'
}

const sourceRecord = (ref: string, observedAt?: string): ReplaySourceRef => ({
  kind: sourceKindForRef(ref),
  ref,
  ...(observedAt === undefined ? {} : { observedAt }),
  ...(ref.startsWith('https://') ? { url: ref } : {}),
})

const asSettlementRows = (
  value: unknown,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  Array.isArray(value)
    ? value.filter((row): row is PublicTassadarSettlementRow => {
        const maybeRow = row as Partial<PublicTassadarSettlementRow>
        return (
          typeof maybeRow.receiptRef === 'string' &&
          typeof maybeRow.amountSats === 'number' &&
          typeof maybeRow.realBitcoinMoved === 'boolean' &&
          (maybeRow.movementMode === 'real_bitcoin' ||
            maybeRow.movementMode === 'simulation') &&
          typeof maybeRow.state === 'string' &&
          Array.isArray(maybeRow.sourceRefs)
        )
      })
    : []

const requestedRefsFor = (request: Request): ReadonlyArray<string> => {
  const url = new URL(request.url)
  return uniqueSorted([
    ...url.searchParams.getAll('refs').flatMap(refs => refs.split(',')),
    ...url.searchParams.getAll('ref'),
    ...url.searchParams.getAll('receiptRef'),
    ...url.searchParams.getAll('run'),
  ])
}

const requestedRunRefFor = (request: Request): string => {
  const runRef = new URL(request.url).searchParams.get('run')?.trim()
  return runRef === undefined || runRef === '' ? DEFAULT_TASSADAR_RUN_REF : runRef
}

const selectRealSettlementRow = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
  requestedRefs: ReadonlyArray<string>,
): PublicTassadarSettlementRow | undefined =>
  rows.find(row => requestedRefs.includes(row.receiptRef) && row.realBitcoinMoved) ??
  rows.find(row => row.receiptRef === FIRST_REAL_SETTLEMENT_RECEIPT_REF) ??
  rows.find(row => row.realBitcoinMoved)

const privateMaterialPatterns = [
  /mnemonic/i,
  /preimage/i,
  /bolt11/i,
  /spark[_-]?api[_-]?key/i,
  /breez[_-]?api[_-]?key/i,
  /service[_-]?token/i,
  /bearer\s+[a-z0-9._-]{12,}/i,
  /payment[_-]?hash/i,
  /provider[_-]?payload/i,
  /raw[_-]?prompt/i,
  /private[_-]?log/i,
  /wallet[_-]?path/i,
]

const assertPublicSafe = (bundle: Omit<ProofReplayBundle, 'bundleRef'>): void => {
  const serialized = JSON.stringify(bundle)
  const matchedPattern = privateMaterialPatterns.find(pattern =>
    pattern.test(serialized),
  )

  if (matchedPattern !== undefined) {
    throw new ProofReplayPublicProjectionUnsafe(
      `proof replay bundle rejected private material pattern ${matchedPattern}`,
    )
  }
}

const stableHash = (value: string): string => {
  const hash = [...value].reduce(
    (accumulator, char) =>
      Math.imul(accumulator ^ char.charCodeAt(0), 16_777_619) >>> 0,
    2_166_136_261,
  )
  return hash.toString(16).padStart(8, '0')
}

const canonicalReceiptApiUrl = (
  appUrl: string,
  receiptRef: string,
): string =>
  `${appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(receiptRef)}`

const refFromSourceRefs = (
  refs: ReadonlyArray<string>,
  prefix: string,
  fallback: string,
): string => refs.find(ref => ref.startsWith(prefix)) ?? fallback

const makeEvent = (
  input: Omit<ReplayEvent, 'eventRef'>,
): ReplayEvent => ({
  ...input,
  eventRef: `proof_replay_event.tassadar.first_real_settlement.${String(input.sequenceIndex).padStart(2, '0')}.${input.kind}`,
})

export const buildFirstRealSettlementReplayBundle = (
  input: Readonly<{
    appUrl: string
    generatedAt: string
    requestedRefs: ReadonlyArray<string>
    summary: PublicTassadarSummaryEnvelope
  }>,
): ProofReplayBundle => {
  const settlementRows = asSettlementRows(input.summary.settlementRows)
  const realSettlementRow = selectRealSettlementRow(
    settlementRows,
    input.requestedRefs,
  )
  const simulationRows = settlementRows.filter(
    row => row.movementMode === 'simulation' || !row.realBitcoinMoved,
  )
  const receiptRef =
    realSettlementRow?.receiptRef ?? FIRST_REAL_SETTLEMENT_RECEIPT_REF
  const contributorRef =
    realSettlementRow?.contributorRef ?? FIRST_REAL_SETTLEMENT_CONTRIBUTOR_REF
  const challengeRef =
    realSettlementRow?.verificationChallengeRef ??
    FIRST_REAL_SETTLEMENT_CHALLENGE_REF
  const runRef =
    typeof input.summary.runRef === 'string'
      ? input.summary.runRef
      : realSettlementRow?.trainingRunRef ?? DEFAULT_TASSADAR_RUN_REF
  const receiptApiUrl =
    realSettlementRow?.apiUrl ?? canonicalReceiptApiUrl(input.appUrl, receiptRef)
  const payoutIntentRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'payout_intent.',
    'payout_intent.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const payoutAttemptRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'payout_attempt.',
    'payout_attempt.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const reconciliationRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'reconciliation.',
    'reconciliation.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const externalEventRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'external_event.',
    'external_event.tassadar_run_settlement.spark_treasury.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const realSourceRefs = uniqueSorted([
    runRef,
    FIRST_REAL_SETTLEMENT_WINDOW_REF,
    challengeRef,
    ...optionalRef(contributorRef),
    receiptRef,
    receiptApiUrl,
    payoutIntentRef,
    payoutAttemptRef,
    reconciliationRef,
    externalEventRef,
    FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL,
    FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL,
    ...(realSettlementRow?.sourceRefs ?? []),
  ])
  const simulationSourceRefs = uniqueSorted(
    simulationRows.flatMap(row => [
      row.receiptRef,
      row.apiUrl,
      ...row.sourceRefs,
    ]),
  )
  const allSourceRefs = uniqueSorted([...realSourceRefs, ...simulationSourceRefs])
  const actors: ReadonlyArray<ReplayActor> = [
    {
      actorRef: `actor.${contributorRef}`,
      avatarRole: 'contributor',
      displayName: 'Contributor pylon',
      fallbackAssetId: 'procedural.pylon_avatar.contributor.v1',
      pylonRef: contributorRef,
    },
    {
      actorRef: 'actor.tassadar.validator',
      avatarRole: 'validator',
      displayName: 'Independent validator',
      fallbackAssetId: 'procedural.pylon_avatar.validator.v1',
    },
    {
      actorRef: 'actor.owner_gate',
      avatarRole: 'operator_gate',
      displayName: 'Owner gate',
      fallbackAssetId: 'procedural.operator_gate.v1',
    },
    {
      actorRef: 'actor.spark_treasury_terminal',
      avatarRole: 'settlement_terminal',
      displayName: 'Spark treasury terminal',
      fallbackAssetId: 'procedural.settlement_terminal.spark.v1',
    },
    {
      actorRef: 'actor.forum_announcer',
      avatarRole: 'announcer',
      displayName: 'Forum announcement',
      fallbackAssetId: 'procedural.forum_announcer.v1',
    },
  ]
  const stages: ReadonlyArray<ReplayStage> = [
    {
      label: 'Tassadar',
      sourceRefs: [runRef],
      stageKind: 'run_core',
      stageRef: 'stage.tassadar.run_core',
    },
    {
      label: 'Contributor station',
      sourceRefs: optionalRef(contributorRef),
      stageKind: 'pylon_station',
      stageRef: `stage.${contributorRef}.station`,
    },
    {
      label: 'Exact replay proof gate',
      sourceRefs: [challengeRef],
      stageKind: 'proof_gate',
      stageRef: 'stage.tassadar.proof_gate',
    },
    {
      label: 'Spark settlement terminal',
      sourceRefs: [receiptRef, payoutIntentRef, reconciliationRef],
      stageKind: 'settlement_terminal',
      stageRef: 'stage.tassadar.settlement_terminal',
    },
    {
      label: 'Simulation rehearsal lane',
      sourceRefs: simulationSourceRefs,
      stageKind: 'registry_marker',
      stageRef: 'stage.tassadar.simulation_rehearsal',
    },
  ]
  const simulationEvents = simulationRows.map((row, index) =>
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: row.amountSats,
      caveat:
        'Simulation-backed settlement record; this is not confirmed Bitcoin movement.',
      displayText: `${row.amountSats} sats simulation rehearsal remains non-payment.`,
      kind: 'payment_zap_simulated',
      sequenceIndex: 4 + index,
      sourceRefs: uniqueSorted([row.receiptRef, row.apiUrl, ...row.sourceRefs]),
      stateAfter: row.state,
      stateBefore: 'simulation_recorded',
      targetRefs: ['stage.tassadar.simulation_rehearsal'],
      timelineSecond: 18 + index,
    }),
  )
  const eventOffset = simulationEvents.length
  const events: ReadonlyArray<ReplayEvent> = [
    makeEvent({
      actorRefs: [`actor.${contributorRef}`],
      displayText: 'Contributor pylon enters the Tassadar replay stage.',
      kind: 'actor_entered_region',
      sequenceIndex: 0,
      sourceRefs: [runRef, ...optionalRef(contributorRef)],
      targetRefs: ['stage.tassadar.run_core'],
      timelineSecond: 0,
    }),
    makeEvent({
      actorRefs: [`actor.${contributorRef}`],
      displayText: 'Contributor submits exact-trace work to the proof gate.',
      kind: 'proof_submitted',
      sequenceIndex: 1,
      sourceRefs: [challengeRef, runRef],
      targetRefs: ['stage.tassadar.proof_gate'],
      timelineSecond: 5,
    }),
    makeEvent({
      actorRefs: ['actor.tassadar.validator'],
      displayText: 'Independent validator replay matches the challenge digest.',
      kind: 'proof_verified',
      sequenceIndex: 2,
      sourceRefs: [challengeRef],
      stateAfter: 'Verified',
      targetRefs: ['stage.tassadar.proof_gate'],
      timelineSecond: 10,
    }),
    makeEvent({
      actorRefs: ['actor.owner_gate'],
      caveat: 'Authorization opens a bounded settlement path; it is not payment.',
      displayText: 'Owner gate authorizes the bounded 1,000-sat settlement branch.',
      kind: 'claim_boundary_shown',
      sequenceIndex: 3,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL, runRef, challengeRef],
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 15,
    }),
    ...simulationEvents,
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      caveat: 'Failed closed before dispatch; 0 sats moved.',
      displayText: 'Payout intent lookup failed closed: 0 sats moved.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 4 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
      stateAfter: 'blocked_closed',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 22,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      displayText: 'Durable payout intent persisted for the real canary settlement.',
      kind: 'payout_intent_persisted',
      sequenceIndex: 5 + eventOffset,
      sourceRefs: [payoutIntentRef, FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL],
      stateAfter: 'persisted',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 27,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      caveat: 'Treasury adapter unavailable; failed closed before sats moved.',
      displayText: 'Spark treasury adapter was unavailable: 0 sats moved.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 6 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL, externalEventRef],
      stateAfter: 'blocked_closed',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 32,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      displayText: 'Settlement receipt recorded from the Spark treasury rail.',
      kind: 'settlement_recorded',
      observedAt: FIRST_REAL_SETTLEMENT_OBSERVED_AT,
      rail: 'spark_treasury',
      sequenceIndex: 7 + eventOffset,
      sourceRefs: [receiptRef, receiptApiUrl, reconciliationRef, payoutAttemptRef],
      stateAfter: realSettlementRow?.state ?? 'settled',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 36,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      displayText: '1,000 sats zap to the contributor pylon is receipt-backed.',
      kind: 'payment_zap_confirmed',
      observedAt: FIRST_REAL_SETTLEMENT_OBSERVED_AT,
      rail: 'spark_treasury',
      sequenceIndex: 8 + eventOffset,
      sourceRefs: [receiptRef, receiptApiUrl, externalEventRef],
      stateAfter:
        realSettlementRow?.realBitcoinMoved === true
          ? 'realBitcoinMoved:true'
          : 'awaiting_public_projection',
      targetRefs: [`actor.${contributorRef}`, `stage.${contributorRef}.station`],
      timelineSecond: 38,
    }),
    makeEvent({
      actorRefs: ['actor.forum_announcer'],
      displayText: 'Public Forum announcement links the settled receipt.',
      kind: 'forum_announcement_posted',
      sequenceIndex: 9 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL, receiptRef],
      targetRefs: ['stage.tassadar.run_core'],
      timelineSecond: 44,
    }),
  ]
  const flows: ReadonlyArray<ReplayFlow> = [
    {
      flowKind: 'work_handoff',
      flowRef: 'proof_replay_flow.tassadar.first_real_settlement.work_handoff',
      fromRef: `actor.${contributorRef}`,
      sourceRefs: [challengeRef, runRef],
      toRef: 'stage.tassadar.proof_gate',
    },
    {
      flowKind: 'verification_check',
      flowRef:
        'proof_replay_flow.tassadar.first_real_settlement.verification_check',
      fromRef: 'actor.tassadar.validator',
      sourceRefs: [challengeRef],
      toRef: 'stage.tassadar.proof_gate',
    },
    {
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      flowKind: 'payment_movement',
      flowRef: 'proof_replay_flow.tassadar.first_real_settlement.spark_zap',
      fromRef: 'actor.spark_treasury_terminal',
      rail: 'spark_treasury',
      sourceRefs: [receiptRef, receiptApiUrl, externalEventRef],
      toRef: `actor.${contributorRef}`,
    },
    ...simulationRows.map((row): ReplayFlow => ({
      amountSats: row.amountSats,
      flowKind: 'simulation_marker',
      flowRef: `proof_replay_flow.tassadar.first_real_settlement.simulation.${stableHash(row.receiptRef)}`,
      fromRef: 'actor.spark_treasury_terminal',
      sourceRefs: [row.receiptRef, row.apiUrl, ...row.sourceRefs],
      toRef: 'stage.tassadar.simulation_rehearsal',
    })),
  ]
  const cameraCues: ReadonlyArray<ReplayCameraCue> = [
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.overview',
      durationSecond: 8,
      focusRefs: ['stage.tassadar.run_core'],
      mode: 'overview',
      sourceRefs: [runRef],
      startSecond: 0,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.orbit_proof',
      durationSecond: 10,
      focusRefs: ['stage.tassadar.proof_gate', challengeRef],
      mode: 'orbit_proof',
      sourceRefs: [challengeRef],
      startSecond: 8,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.zap_focus',
      durationSecond: 8,
      focusRefs: ['stage.tassadar.settlement_terminal', receiptRef],
      mode: 'zap_focus',
      sourceRefs: [receiptRef, receiptApiUrl],
      startSecond: 34,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.final',
      durationSecond: 10,
      focusRefs: ['stage.tassadar.run_core', `actor.${contributorRef}`, receiptRef],
      mode: 'director_track',
      sourceRefs: [receiptRef, FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL],
      startSecond: 42,
    },
  ]
  const captions: ReadonlyArray<ReplayCaption> = [
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.title',
      sequenceIndex: 0,
      sourceRefs: [runRef],
      text: FIRST_REAL_SETTLEMENT_TITLE,
      timelineSecond: 0,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.verify',
      sequenceIndex: 1,
      sourceRefs: [challengeRef],
      text: 'Verified work -> owner gate -> Spark zap -> public receipt',
      timelineSecond: 9,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.failed_closed',
      sequenceIndex: 2,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
      text: 'Two real dispatch blockers failed closed before any sats moved.',
      timelineSecond: 22,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.zap',
      sequenceIndex: 3,
      sourceRefs: [receiptRef, receiptApiUrl],
      text: '1,000 sats settled, realBitcoinMoved:true',
      timelineSecond: 38,
    },
  ]
  const gaps: ReadonlyArray<ReplayGap> = [
    {
      affectedRefs: [
        'payout_intent_not_found',
        'adapter_unavailable',
        FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL,
      ],
      gapRef: 'proof_replay_gap.tassadar.first_real_settlement.operational_history_sequence',
      reason:
        'Intermediate failed-closed timestamps are Forum-announced operational history and are ordered by replay sequence.',
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
    },
  ]
  const bundleWithoutRef: Omit<ProofReplayBundle, 'bundleRef'> = {
    actors,
    cameraCues,
    captions,
    claimScope: 'evidence_presentation_only',
    events,
    flows,
    gaps,
    generatedAt: input.generatedAt,
    privacyLevel: 'public_safe',
    schemaVersion: ProofReplayBundleSchemaVersion,
    socialDisplayTime: FIRST_REAL_SETTLEMENT_LOCAL_DISPLAY_TIME,
    sourceAuthority: 'worker_d1_public',
    sourceRefs: allSourceRefs.map(ref =>
      sourceRecord(
        ref,
        ref === receiptRef || ref === receiptApiUrl
          ? FIRST_REAL_SETTLEMENT_OBSERVED_AT
          : undefined,
      ),
    ),
    stages,
    staleness: replayBundleStaleness(),
    title: FIRST_REAL_SETTLEMENT_TITLE,
  }
  assertPublicSafe(bundleWithoutRef)

  const deterministicRefSeed = JSON.stringify({
    events: bundleWithoutRef.events.map(event => ({
      kind: event.kind,
      sequenceIndex: event.sequenceIndex,
      sourceRefs: event.sourceRefs,
    })),
    receiptRef,
    sourceRefs: allSourceRefs,
    title: bundleWithoutRef.title,
  })

  return {
    ...bundleWithoutRef,
    bundleRef: `proof_replay_bundle.tassadar.first_real_settlement.${stableHash(
      deterministicRefSeed,
    )}`,
  }
}

export const buildPublicProofReplayBundleForRequest = async (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
  deps: Deps = {},
): Promise<ProofReplayBundle> => {
  const generatedAt = (deps.now ?? currentIsoTimestamp)()
  const makeStore =
    deps.makeStore ?? (e => makeD1TrainingAuthorityStore(openAgentsDatabase(e)))
  const makePayoutLedgerStore =
    deps.makePayoutLedgerStore ??
    (e => makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(e)))
  const runRef = requestedRunRefFor(request)
  const appUrl = new URL(request.url).origin
  const summary = await buildPublicTassadarRunSummaryEnvelope(
    makeStore(env),
    runRef,
    generatedAt,
    makePayoutLedgerStore(env),
    appUrl,
  )

  return buildFirstRealSettlementReplayBundle({
    appUrl,
    generatedAt,
    requestedRefs: requestedRefsFor(request),
    summary,
  })
}
