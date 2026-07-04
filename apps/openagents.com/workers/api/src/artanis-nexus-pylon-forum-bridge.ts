import { Effect, Schema as S } from 'effect'

import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationQueueRecord,
  projectArtanisForumPublicationQueue,
  selectReadyArtanisForumPublicationIntents,
} from './artanis-forum-publication'
import {
  saveArtanisForumPublicationIntent,
  type ArtanisPersistenceError,
  type ArtanisPersistenceWriteReceipt,
} from './artanis-persistence'
import { type ArtanisDatabase } from './artanis-domain-store'
import type {
  PylonV02OmegaReleaseGateProjection,
} from './pylon-v02-omega-release-gate'

export const ArtanisNexusPylonForumEventKind = S.Literals([
  'assignment_created',
  'pylon_selected',
  'assignment_progress',
  'incident_blocker',
  'payout_intent_created',
  'settlement_complete',
  'release_gate_passed',
  'release_gate_failed',
])
export type ArtanisNexusPylonForumEventKind =
  typeof ArtanisNexusPylonForumEventKind.Type

export const ArtanisNexusPylonForumBridgeState = S.Literals([
  'disabled',
  'enabled',
  'paused',
])
export type ArtanisNexusPylonForumBridgeState =
  typeof ArtanisNexusPylonForumBridgeState.Type

export const ArtanisNexusPylonForumReleaseGateStatus = S.Literals([
  'blocked',
  'failed',
  'passed',
])
export type ArtanisNexusPylonForumReleaseGateStatus =
  typeof ArtanisNexusPylonForumReleaseGateStatus.Type

export class ArtanisNexusPylonForumEventRecord extends S.Class<ArtanisNexusPylonForumEventRecord>(
  'ArtanisNexusPylonForumEventRecord',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  eventKind: ArtanisNexusPylonForumEventKind,
  eventRef: S.String,
  goalRefs: S.Array(S.String),
  modelLabReportRefs: S.Array(S.String),
  pageUrls: S.Array(S.String),
  publicContextRefs: S.Array(S.String),
  pylonNexusPublicRefs: S.Array(S.String),
  r10ClaimRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  releaseGateStatus: S.NullOr(ArtanisNexusPylonForumReleaseGateStatus),
  summaryLabel: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisNexusPylonForumBridgePolicy extends S.Class<ArtanisNexusPylonForumBridgePolicy>(
  'ArtanisNexusPylonForumBridgePolicy',
)({
  blockerRefs: S.Array(S.String),
  bridgeRef: S.String,
  state: ArtanisNexusPylonForumBridgeState,
}) {}

export class ArtanisNexusPylonForumBridgeProjection extends S.Class<ArtanisNexusPylonForumBridgeProjection>(
  'ArtanisNexusPylonForumBridgeProjection',
)({
  blockedIntentRefs: S.Array(S.String),
  bridgeRef: S.String,
  deliveryPaused: S.Boolean,
  duplicateIntentRefs: S.Array(S.String),
  intentCount: S.Number,
  readyIntentRefs: S.Array(S.String),
  state: ArtanisNexusPylonForumBridgeState,
}) {}

const redactionPolicyRef = 'redaction.forum.public.artanis.nexus_pylon.v1'
const queueRef = 'queue.public.artanis.nexus_pylon.forum_publications'

const topicByEventKind:
  Readonly<Record<ArtanisNexusPylonForumEventKind, string>> = {
    assignment_created: 'topic.public.forum.artanis.work_routing',
    assignment_progress: 'topic.public.forum.artanis.pylon_release_work_log',
    incident_blocker: 'topic.public.forum.artanis.operator_questions',
    payout_intent_created: 'topic.public.forum.artanis.bitcoin_accounting',
    pylon_selected: 'topic.public.forum.artanis.pylon_campaign',
    release_gate_failed: 'topic.public.forum.artanis.pylon_release_work_log',
    release_gate_passed: 'topic.public.forum.artanis.pylon_release_work_log',
    settlement_complete: 'topic.public.forum.artanis.bitcoin_accounting',
  }

const bodyPrefixByEventKind:
  Readonly<Record<ArtanisNexusPylonForumEventKind, string>> = {
    assignment_created:
      'Artanis assignment update: a Nexus/Pylon assignment has been created for public coordination.',
    assignment_progress:
      'Artanis assignment progress: Nexus/Pylon work has a new reviewed progress update.',
    incident_blocker:
      'Artanis incident update: Nexus/Pylon work is blocked and needs operator or agent attention.',
    payout_intent_created:
      'Artanis bitcoin accounting update: a reward intent has been recorded for accepted Nexus/Pylon work.',
    pylon_selected:
      'Artanis Pylon selection update: a Pylon has been selected for the assignment.',
    release_gate_failed:
      'Artanis release gate update: a Nexus/Pylon release gate failed and remains blocked.',
    release_gate_passed:
      'Artanis release gate update: a Nexus/Pylon release gate passed with public evidence.',
    settlement_complete:
      'Artanis bitcoin accounting update: Nexus/Pylon settlement evidence is complete for this simulated flow.',
  }

const bridgeBlockerRef = (
  policy: ArtanisNexusPylonForumBridgePolicy,
): string | null =>
  policy.state === 'paused'
    ? 'blocker.public.artanis.nexus_pylon_forum_bridge_paused'
    : policy.state === 'disabled'
      ? 'blocker.public.artanis.nexus_pylon_forum_bridge_disabled'
      : null

const refSuffix = (value: string): string =>
  value
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
    .slice(0, 120) || 'event'

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const publicLinksSentence = (
  urls: ReadonlyArray<string>,
): string | null => {
  const publicUrls = uniqueRefs(urls).slice(0, 5)

  return publicUrls.length === 0
    ? null
    : `Public links: ${publicUrls.join(' ')}.`
}

const receiptRefsSentence = (
  refs: ReadonlyArray<string>,
): string | null => {
  const publicRefs = uniqueRefs(refs).slice(0, 5)

  return publicRefs.length === 0
    ? null
    : `Receipt refs: ${publicRefs.join(' ')}.`
}

const bodyForEvent = (event: ArtanisNexusPylonForumEventRecord): string =>
  [
    bodyPrefixByEventKind[event.eventKind],
    `Summary: ${event.summaryLabel.trim()}.`,
    event.blockerRefs.length > 0
      ? 'Blockers are recorded in public-safe blocker refs.'
      : 'No public blocker is currently recorded.',
    event.releaseGateStatus === null
      ? null
      : `Release gate status: ${event.releaseGateStatus}.`,
    publicLinksSentence(event.pageUrls),
    receiptRefsSentence(event.receiptRefs),
    'This Forum update is evidence-only and does not grant wallet spend, payment spend, provider mutation, training launch, deployment, moderation, or settlement authority.',
  ]
    .filter((part): part is string => part !== null)
    .join(' ')

export const artanisNexusPylonForumIntentForEvent = (
  event: ArtanisNexusPylonForumEventRecord,
  policy: ArtanisNexusPylonForumBridgePolicy,
): ArtanisForumPublicationIntentRecord => {
  const suffix = refSuffix(event.eventRef)
  const policyBlockerRef = bridgeBlockerRef(policy)
  const deliveryState = policy.state === 'enabled' ? 'ready' : 'blocked'

  return new ArtanisForumPublicationIntentRecord({
    artifactRefs: uniqueRefs(event.artifactRefs),
    authorAgentId: 'agent_artanis',
    blockerRefs: uniqueRefs([
      ...event.blockerRefs,
      ...policy.blockerRefs,
      ...(policyBlockerRef === null ? [] : [policyBlockerRef]),
    ]),
    bodyText: bodyForEvent(event),
    caveatRefs: uniqueRefs([
      ...event.caveatRefs,
      'caveat.public.artanis_forum_bridge_evidence_only',
      'caveat.public.no_sensitive_material',
    ]),
    createdAtIso: event.createdAtIso,
    deliveredAtIso: null,
    deliveryReceiptRefs: [],
    deliveryState,
    goalRefs: uniqueRefs(event.goalRefs),
    idempotencyKey:
      `artanis-forum:nexus-pylon:${event.eventKind}:${suffix}:v1`,
    intentRef:
      `forum.public.artanis.nexus_pylon.${event.eventKind}.${suffix}`,
    modelLabReportRefs: uniqueRefs(event.modelLabReportRefs),
    pageUrls: uniqueRefs(event.pageUrls),
    postRef: null,
    pylonNexusPublicRefs: uniqueRefs(event.pylonNexusPublicRefs),
    r10ClaimRefs: uniqueRefs(event.r10ClaimRefs),
    receiptRefs: uniqueRefs(event.receiptRefs),
    redactionPolicyRef,
    sourceRefs: uniqueRefs([
      ...event.artifactRefs,
      ...event.goalRefs,
      ...event.modelLabReportRefs,
      ...event.publicContextRefs,
      ...event.pylonNexusPublicRefs,
      ...event.r10ClaimRefs,
      ...event.receiptRefs,
    ]),
    targetForumRef: 'forum.public.artanis',
    targetTopicRef: topicByEventKind[event.eventKind],
    targetTopicState: 'open',
    updatedAtIso: event.updatedAtIso,
  })
}

export const buildArtanisNexusPylonForumPublicationQueue = (
  input: Readonly<{
    events: ReadonlyArray<ArtanisNexusPylonForumEventRecord>
    policy: ArtanisNexusPylonForumBridgePolicy
  }>,
): ArtanisForumPublicationQueueRecord =>
  new ArtanisForumPublicationQueueRecord({
    agentId: 'agent_artanis',
    caveatRefs: [
      'caveat.public.artanis_forum_bridge_evidence_only',
      'caveat.public.nexus_pylon_public_safe_projection',
    ],
    createdAtIso: input.events[0]?.createdAtIso ?? '2026-06-07T00:00:00.000Z',
    intents: input.events.map(event =>
      artanisNexusPylonForumIntentForEvent(event, input.policy),
    ),
    queueRef,
    redactionPolicyRef,
    updatedAtIso: input.events.at(-1)?.updatedAtIso ?? '2026-06-07T00:00:00.000Z',
  })

export const projectArtanisNexusPylonForumBridge = (
  input: Readonly<{
    events: ReadonlyArray<ArtanisNexusPylonForumEventRecord>
    nowIso: string
    policy: ArtanisNexusPylonForumBridgePolicy
  }>,
): ArtanisNexusPylonForumBridgeProjection => {
  const queue = buildArtanisNexusPylonForumPublicationQueue({
    events: input.events,
    policy: input.policy,
  })
  const projection = projectArtanisForumPublicationQueue(queue, input.nowIso)
  const readyIntentRefs = selectReadyArtanisForumPublicationIntents(queue).map(
    intent => intent.intentRef,
  )

  return new ArtanisNexusPylonForumBridgeProjection({
    blockedIntentRefs: projection.intents
      .filter(intent => intent.deliveryState === 'blocked')
      .map(intent => intent.intentRef),
    bridgeRef: input.policy.bridgeRef,
    deliveryPaused: input.policy.state !== 'enabled',
    duplicateIntentRefs: projection.duplicateIntentRefs,
    intentCount: projection.intentCount,
    readyIntentRefs,
    state: input.policy.state,
  })
}

export const saveArtanisNexusPylonForumPublicationIntents = (
  db: ArtanisDatabase,
  input: Readonly<{
    events: ReadonlyArray<ArtanisNexusPylonForumEventRecord>
    nowIso: string
    policy: ArtanisNexusPylonForumBridgePolicy
  }>,
): Effect.Effect<
  ReadonlyArray<ArtanisPersistenceWriteReceipt>,
  ArtanisPersistenceError
> => {
  const queue = buildArtanisNexusPylonForumPublicationQueue({
    events: input.events,
    policy: input.policy,
  })

  return Effect.forEach(
    selectReadyArtanisForumPublicationIntents(queue),
    intent => saveArtanisForumPublicationIntent(db, intent, input.nowIso),
    { concurrency: 1 },
  )
}

const releaseGateReceiptRefs = (
  projection: PylonV02OmegaReleaseGateProjection,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...projection.evidenceRefs,
    ...projection.multiPylonProofRefs,
  ].filter(ref => ref.startsWith('receipt.')))

const publicForumReceiptRef = (receiptRef: string): string =>
  receiptRef.startsWith('receipt.public.')
    ? receiptRef
    : `receipt.public.${receiptRef.replace(/^receipt\./, '')}`

const publicReceiptPageUrls = (
  receiptRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  receiptRefs.map(
    receiptRef =>
      `https://openagents.com/nexus-pylon/receipts/${receiptRef}`,
  )

export const artanisNexusPylonForumEventFromReleaseGate = (
  input: Readonly<{
    createdAtIso: string
    projection: PylonV02OmegaReleaseGateProjection
    updatedAtIso: string
  }>,
): ArtanisNexusPylonForumEventRecord => {
  const receiptRefs = releaseGateReceiptRefs(input.projection)
  const releaseGatePassed =
    input.projection.state === 'ready_for_operator_release_review' ||
    input.projection.state === 'limited_launcher_release_shipped'
  const summaryLabel = releaseGatePassed
    ? `Pylon v0.2 OpenAgents Nexus proof is complete across ${input.projection.multiPylonObservedDistinctPylonCount} distinct Pylons`
    : `Pylon v0.2 OpenAgents Nexus proof is blocked: ${input.projection.multiPylonObservedDistinctPylonCount} of ${input.projection.multiPylonRequiredDistinctPylonCount} required distinct Pylons have complete paid-work proof`

  return new ArtanisNexusPylonForumEventRecord({
    artifactRefs: ['artifact.public.pylon_v0_2.omega_release_gate_status'],
    blockerRefs: input.projection.blockerRefs,
    caveatRefs: [
      'caveat.public.artanis_forum_bridge_evidence_only',
      'caveat.public.no_sensitive_material',
      ...(releaseGatePassed
        ? []
        : ['caveat.public.pylon_v0_2_omega_release_gate_blocked']),
    ],
    createdAtIso: input.createdAtIso,
    eventKind: releaseGatePassed
      ? 'release_gate_passed'
      : 'release_gate_failed',
    eventRef:
      `event.public.artanis.nexus_pylon.release_gate.${input.projection.state}`,
    goalRefs: ['goal.public.artanis.pylon_model_lab'],
    modelLabReportRefs: [],
    pageUrls: uniqueRefs([
      'https://openagents.com/artanis',
      'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
      ...publicReceiptPageUrls(receiptRefs),
    ]),
    publicContextRefs: [
      'context.public.pylon_v0_2.omega_release_gate',
      `context.public.pylon_v0_2.omega_release_gate.${input.projection.state}`,
    ],
    pylonNexusPublicRefs: uniqueRefs([
      'campaign.public.pylon.v0_2',
      'nexus.public.pylon.release_gate',
      ...input.projection.multiPylonObservedPylonRefs,
    ]),
    r10ClaimRefs: [],
    receiptRefs: receiptRefs.map(publicForumReceiptRef),
    releaseGateStatus: releaseGatePassed ? 'passed' : 'blocked',
    summaryLabel,
    updatedAtIso: input.updatedAtIso,
  })
}

export const exampleArtanisNexusPylonForumBridgePolicy =
  (): ArtanisNexusPylonForumBridgePolicy =>
    new ArtanisNexusPylonForumBridgePolicy({
      blockerRefs: [],
      bridgeRef: 'bridge.public.artanis.nexus_pylon.forum_updates',
      state: 'enabled',
    })

export const exampleArtanisNexusPylonForumEvents =
  (): ReadonlyArray<ArtanisNexusPylonForumEventRecord> => {
    const base = {
      artifactRefs: ['artifact.public.gepa_autopilot_redacted_manifest'],
      caveatRefs: ['caveat.public.nexus_pylon_simulated_flow'],
      goalRefs: ['goal.public.artanis.pylon_model_lab'],
      modelLabReportRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
      pageUrls: [
        'https://openagents.com/artanis',
        'https://openagents.com/forum/f/artanis',
        'https://openagents.com/nexus-pylon/receipts/receipt.nexus.simulation.settlement.hash.pylon_marketplace.attempt.gepa_autopilot_001',
      ],
      publicContextRefs: [
        'context.public.artanis.nexus_pylon.gepa_autopilot_001',
      ],
      pylonNexusPublicRefs: [
        'campaign.public.pylon.v0_2',
        'nexus.public.pylon.assignment.gepa_autopilot_001',
        'pylon.public.assignment.gepa_autopilot_001',
      ],
      r10ClaimRefs: ['claim.public.r10.pylon_learning_loop'],
      receiptRefs: ['receipt.public.nexus.assignment.gepa_autopilot_001'],
    } as const
    const event = (
      input: Readonly<{
        blockerRefs?: ReadonlyArray<string>
        createdAtIso: string
        eventKind: ArtanisNexusPylonForumEventKind
        eventRef: string
        releaseGateStatus?: ArtanisNexusPylonForumReleaseGateStatus | null
        summaryLabel: string
        updatedAtIso: string
      }>,
    ): ArtanisNexusPylonForumEventRecord =>
      new ArtanisNexusPylonForumEventRecord({
        ...base,
        blockerRefs: [...(input.blockerRefs ?? [])],
        createdAtIso: input.createdAtIso,
        eventKind: input.eventKind,
        eventRef: input.eventRef,
        releaseGateStatus: input.releaseGateStatus ?? null,
        summaryLabel: input.summaryLabel,
        updatedAtIso: input.updatedAtIso,
      })

    return [
      event({
        createdAtIso: '2026-06-07T06:10:00.000Z',
        eventKind: 'assignment_created',
        eventRef: 'event.public.artanis.nexus_pylon.assignment_created.gepa_autopilot_001',
        summaryLabel:
          'GEPA Autopilot benchmark improvement work entered the public assignment queue',
        updatedAtIso: '2026-06-07T06:10:00.000Z',
      }),
      event({
        createdAtIso: '2026-06-07T06:11:00.000Z',
        eventKind: 'pylon_selected',
        eventRef: 'event.public.artanis.nexus_pylon.pylon_selected.gepa_autopilot_001',
        summaryLabel:
          'a sellable overnight Pylon was selected for the assignment',
        updatedAtIso: '2026-06-07T06:11:00.000Z',
      }),
      event({
        createdAtIso: '2026-06-07T06:12:00.000Z',
        eventKind: 'assignment_progress',
        eventRef: 'event.public.artanis.nexus_pylon.assignment_progress.gepa_autopilot_001',
        summaryLabel:
          'the assigned Pylon submitted reviewed progress evidence',
        updatedAtIso: '2026-06-07T06:12:00.000Z',
      }),
      event({
        blockerRefs: ['blocker.public.live_dispatch_not_enabled'],
        createdAtIso: '2026-06-07T06:13:00.000Z',
        eventKind: 'incident_blocker',
        eventRef: 'event.public.artanis.nexus_pylon.incident_blocker.live_dispatch_not_enabled',
        summaryLabel:
          'live dispatch remains blocked until release evidence is accepted',
        updatedAtIso: '2026-06-07T06:13:00.000Z',
      }),
      event({
        createdAtIso: '2026-06-07T06:14:00.000Z',
        eventKind: 'payout_intent_created',
        eventRef: 'event.public.artanis.nexus_pylon.payout_intent_created.gepa_autopilot_001',
        summaryLabel:
          'a reward intent was recorded after accepted public work evidence',
        updatedAtIso: '2026-06-07T06:14:00.000Z',
      }),
      event({
        createdAtIso: '2026-06-07T06:15:00.000Z',
        eventKind: 'settlement_complete',
        eventRef: 'event.public.artanis.nexus_pylon.settlement_complete.gepa_autopilot_001',
        summaryLabel:
          'the simulated settlement receipt chain reached terminal evidence',
        updatedAtIso: '2026-06-07T06:15:00.000Z',
      }),
      event({
        createdAtIso: '2026-06-07T06:16:00.000Z',
        eventKind: 'release_gate_passed',
        eventRef: 'event.public.artanis.nexus_pylon.release_gate_passed.public_receipt',
        releaseGateStatus: 'passed',
        summaryLabel:
          'the public receipt release gate passed with evidence-only receipts',
        updatedAtIso: '2026-06-07T06:16:00.000Z',
      }),
      event({
        blockerRefs: ['blocker.public.operator_dashboard_evidence_missing'],
        createdAtIso: '2026-06-07T06:17:00.000Z',
        eventKind: 'release_gate_failed',
        eventRef: 'event.public.artanis.nexus_pylon.release_gate_failed.operator_dashboard',
        releaseGateStatus: 'failed',
        summaryLabel:
          'the operator dashboard release gate failed pending more evidence',
        updatedAtIso: '2026-06-07T06:17:00.000Z',
      }),
    ]
  }
