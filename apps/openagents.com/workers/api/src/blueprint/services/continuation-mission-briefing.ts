import { Match as M } from 'effect'

import type {
  BlueprintContinuationDecisionQueueItem,
  BlueprintContinuationDecisionQueueProjection,
} from '../schemas/continuation-decision-queue'
import type {
  BlueprintMissionBriefingAudience,
  BlueprintMissionBriefingItem,
  BlueprintMissionBriefingItemStatus,
  BlueprintMissionBriefingProjection,
  BlueprintMissionBriefingSectionKind,
  BlueprintMissionBriefingWorkKind,
} from '../schemas/continuation-mission-briefing'

export type BuildBlueprintMissionBriefingInput = Readonly<{
  acceptanceRequestRefs?: ReadonlyArray<string> | undefined
  audience: BlueprintMissionBriefingAudience
  blockerRefs?: ReadonlyArray<string> | undefined
  buildRefs?: ReadonlyArray<string> | undefined
  changedArtifactRefs?: ReadonlyArray<string> | undefined
  costRefs?: ReadonlyArray<string> | undefined
  emailRefs?: ReadonlyArray<string> | undefined
  evidenceRefs?: ReadonlyArray<string> | undefined
  nowIso: string
  publicLinkRefs?: ReadonlyArray<string> | undefined
  queue: BlueprintContinuationDecisionQueueProjection
  routeRefs?: ReadonlyArray<string> | undefined
  testRefs?: ReadonlyArray<string> | undefined
  updatedAtIso: string
  workKind: BlueprintMissionBriefingWorkKind
  workroomRef: string
}>

const universallyUnsafeTextPattern =
  /(bearer\s+|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|mnemonic|oauth|oa_agent_|openagents_admin|password|preimage|private[_-]?key|raw[_-]?email|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet[_-]?secret|\S+@\S+)/i
const customerPrivateMaterialPattern =
  /(provider[_-]?account|provider[_-]?payload|provider[_-]?token|source[_-]?authority)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const audienceCanSeeOperatorRefs = (
  audience: BlueprintMissionBriefingAudience,
): boolean => audience === 'operator'

const textIsSafeForAudience = (
  value: string,
  audience: BlueprintMissionBriefingAudience,
): boolean =>
  value.trim() !== '' &&
  !universallyUnsafeTextPattern.test(value) &&
  !isoTimestampPattern.test(value) &&
  (audienceCanSeeOperatorRefs(audience) ||
    !customerPrivateMaterialPattern.test(value))

const safeRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => textIsSafeForAudience(ref, audience))

const item = (
  kind: BlueprintMissionBriefingSectionKind,
  ref: string,
  status: BlueprintMissionBriefingItemStatus,
  audience: BlueprintMissionBriefingAudience,
  summaryRef = `${ref}:summary`,
  displayTime: string | null = null,
  linkRefs: ReadonlyArray<string> = [],
): BlueprintMissionBriefingItem | null => {
  const safeRef = safeRefsForAudience([ref], audience)[0]
  const safeSummaryRef = safeRefsForAudience([summaryRef], audience)[0]
  const safeLinkRefs = safeRefsForAudience(linkRefs, audience)

  if (safeRef === undefined || safeSummaryRef === undefined) {
    return null
  }

  return {
    displayTime,
    kind,
    linkRefs: safeLinkRefs,
    ref: safeRef,
    status,
    summaryRef: safeSummaryRef,
  }
}

const compactItems = (
  values: ReadonlyArray<BlueprintMissionBriefingItem | null>,
): ReadonlyArray<BlueprintMissionBriefingItem> =>
  values.filter((value): value is BlueprintMissionBriefingItem => value !== null)

const itemsFromRefs = (
  kind: BlueprintMissionBriefingSectionKind,
  refs: ReadonlyArray<string>,
  status: BlueprintMissionBriefingItemStatus,
  audience: BlueprintMissionBriefingAudience,
  displayTime: string | null,
): ReadonlyArray<BlueprintMissionBriefingItem> =>
  compactItems(
    safeRefsForAudience(refs, audience).map(ref =>
      item(kind, ref, status, audience, `${ref}:summary`, displayTime),
    ),
  )

export const friendlyBlueprintMissionBriefingTime = (
  iso: string,
  nowIso: string,
): string => {
  const time = Date.parse(iso)
  const now = Date.parse(nowIso)

  if (!Number.isFinite(time) || !Number.isFinite(now)) {
    return 'Recently'
  }

  const elapsedMs = Math.max(0, now - time)
  const minuteMs = 60_000
  const hourMs = 60 * minuteMs
  const dayMs = 24 * hourMs

  if (elapsedMs < minuteMs) {
    return 'Just now'
  }

  if (elapsedMs < hourMs) {
    const minutes = Math.floor(elapsedMs / minuteMs)

    return minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`
  }

  if (elapsedMs < dayMs) {
    const hours = Math.floor(elapsedMs / hourMs)

    return hours === 1 ? '1 hour ago' : `${hours} hours ago`
  }

  if (elapsedMs < 2 * dayMs) {
    return 'Yesterday'
  }

  const days = Math.floor(elapsedMs / dayMs)

  return `${days} days ago`
}

const queueStatusToBriefingStatus = (
  status: BlueprintContinuationDecisionQueueItem['status'],
): BlueprintMissionBriefingItemStatus =>
  M.value(status).pipe(
    M.when('blocked', () => 'blocked' as const),
    M.when('needs_review', () => 'needs_review' as const),
    M.when('pending', () => 'pending' as const),
    M.when('retrying', () => 'retrying' as const),
    M.when('terminal', () => 'done' as const),
    M.exhaustive,
  )

const nextActionItems = (
  queue: BlueprintContinuationDecisionQueueProjection,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<BlueprintMissionBriefingItem> => {
  if (queue.items.length === 0) {
    return compactItems([
      item(
        'next_action',
        'next_action.awaiting_work',
        'pending',
        audience,
      ),
    ])
  }

  return compactItems(
    queue.items.map(queueItem =>
      item(
        'next_action',
        queueItem.recommendedNextOrderRef,
        queueStatusToBriefingStatus(queueItem.status),
        audience,
        queueItem.safeSummaryRef,
        null,
        [
          queueItem.decisionRef,
          queueItem.programRunRef ?? '',
          ...queueItem.workroomRefs,
        ].filter(ref => ref !== ''),
      ),
    ),
  )
}

const queueRefs = (
  queue: BlueprintContinuationDecisionQueueProjection,
  selector: (
    item: BlueprintContinuationDecisionQueueItem,
  ) => ReadonlyArray<string>,
): ReadonlyArray<string> => queue.items.flatMap(selector)

const missionStatus = (
  queue: BlueprintContinuationDecisionQueueProjection,
): string => {
  if (queue.empty) {
    return 'empty'
  }

  if (queue.blockerCount > 0) {
    return 'blocked'
  }

  if (queue.reviewCount > 0) {
    return 'needs_review'
  }

  if (queue.retryCount > 0) {
    return 'retrying'
  }

  if (queue.pendingCount > 0) {
    return 'pending'
  }

  return 'ready'
}

export const buildBlueprintMissionBriefing = (
  input: BuildBlueprintMissionBriefingInput,
): BlueprintMissionBriefingProjection => {
  const displayTime = friendlyBlueprintMissionBriefingTime(
    input.updatedAtIso,
    input.nowIso,
  )
  const audience = input.audience
  const changed = itemsFromRefs(
    'changed',
    input.changedArtifactRefs ?? [],
    'ready',
    audience,
    displayTime,
  )
  const evidence = itemsFromRefs(
    'evidence',
    [
      ...(input.evidenceRefs ?? []),
      ...queueRefs(input.queue, item => item.evidenceRefs),
      ...queueRefs(input.queue, item => item.receiptRefs),
    ],
    'ready',
    audience,
    displayTime,
  )
  const verification = itemsFromRefs(
    'verification',
    [...(input.buildRefs ?? []), ...(input.testRefs ?? [])],
    'ready',
    audience,
    displayTime,
  )
  const email = itemsFromRefs(
    'email',
    input.emailRefs ?? [],
    'sent',
    audience,
    displayTime,
  )
  const blocked = [
    ...itemsFromRefs(
      'blocked',
      [
        ...(input.blockerRefs ?? []),
        ...queueRefs(input.queue, item => item.blockerRefs),
      ],
      'blocked',
      audience,
      displayTime,
    ),
    ...itemsFromRefs(
      'blocked',
      queueRefs(input.queue, item => item.retryRefs),
      'retrying',
      audience,
      displayTime,
    ),
    ...itemsFromRefs(
      'blocked',
      queueRefs(input.queue, item => item.stopConditionRefs),
      'done',
      audience,
      displayTime,
    ),
  ]
  const costs = itemsFromRefs(
    'cost',
    input.costRefs ?? [],
    'ready',
    audience,
    displayTime,
  )
  const route = itemsFromRefs(
    'route',
    [
      ...(input.routeRefs ?? []),
      ...queueRefs(input.queue, item => item.orderRefs),
      ...queueRefs(input.queue, item => item.siteRefs),
      ...queueRefs(input.queue, item => item.workroomRefs),
    ],
    'ready',
    audience,
    displayTime,
  )
  const acceptanceRequest = itemsFromRefs(
    'acceptance_request',
    [
      ...(input.acceptanceRequestRefs ?? []),
      ...queueRefs(input.queue, item => item.approvalRefs),
    ],
    'needs_review',
    audience,
    displayTime,
  )
  const links = itemsFromRefs(
    'link',
    input.publicLinkRefs ?? [],
    'ready',
    audience,
    displayTime,
  )
  const nextAction = nextActionItems(input.queue, audience)
  const sections = {
    acceptanceRequest,
    blocked,
    changed,
    costs,
    email,
    evidence,
    links,
    nextAction,
    route,
    verification,
  }

  return {
    audience,
    empty:
      acceptanceRequest.length === 0 &&
      blocked.length === 0 &&
      changed.length === 0 &&
      costs.length === 0 &&
      email.length === 0 &&
      evidence.length === 0 &&
      links.length === 0 &&
      route.length === 0 &&
      verification.length === 0,
    generatedAtDisplay: displayTime,
    sections,
    status: missionStatus(input.queue),
    workKind: input.workKind,
    workroomRef:
      safeRefsForAudience([input.workroomRef], audience)[0] ??
      'workroom.redacted',
  }
}

export const blueprintMissionBriefingHasPrivateMaterial = (
  briefing: BlueprintMissionBriefingProjection,
): boolean =>
  universallyUnsafeTextPattern.test(JSON.stringify(briefing)) ||
  isoTimestampPattern.test(JSON.stringify(briefing)) ||
  (!audienceCanSeeOperatorRefs(briefing.audience) &&
    customerPrivateMaterialPattern.test(JSON.stringify(briefing)))
