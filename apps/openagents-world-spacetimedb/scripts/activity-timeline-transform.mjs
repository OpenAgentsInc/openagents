import {
  DEFAULT_DATABASE,
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
  stableHash,
  stableJson,
} from './tassadar-summary-transform.mjs'

export const DEFAULT_ACTIVITY_TIMELINE_SOURCE_URL =
  'https://openagents.com/api/public/activity-timeline?limit=50'
export const DEFAULT_ACTIVITY_WORLD_RUN_REF = 'run.public_activity_timeline'
export const DEFAULT_ACTIVITY_BRIDGE_REF = 'bridge.public-activity-timeline'

const text = value => (typeof value === 'string' ? value.trim() : '')

const numberOrZero = value =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const array = value => (Array.isArray(value) ? value : [])

const unique = values =>
  [...new Set(array(values).map(text).filter(value => value.length > 0))].sort()

const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value)

const addCall = (calls, reducer, args) => {
  calls.push({ reducer, args })
}

const privateMaterialPatterns = [
  /\b(?:raw[_-]?(?:prompt|trace|payload|log)|provider[_-]?payload)\b/i,
  /\b(?:secret|token_secret|service_token|bearer\s+[a-z0-9._-]{6,})\b/i,
  /\b(?:payment_preimage|payment_hash|invoice|bolt11|mnemonic|wallet_seed|xprv)\b/i,
  /\bsk-[a-z0-9_-]+\b/i,
  /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/i,
]

export const activityTimelineEnvelopeHasUnsafeMaterial = envelope =>
  privateMaterialPatterns.some(pattern => pattern.test(JSON.stringify(envelope)))

const sourceRefsForEvent = event =>
  unique([
    ...array(event?.sourceRefs),
    ...array(event?.blockerRefs),
    ...array(event?.caveatRefs),
  ])

const primarySourceRef = event =>
  unique(event?.sourceRefs)[0] ??
  unique(event?.blockerRefs)[0] ??
  text(event?.eventRef)

const entityRefForEvent = event =>
  text(event?.actorRef) ||
  text(event?.targetRef) ||
  unique(event?.refs)[0] ||
  text(event?.eventRef)

const sourceLagForEvent = (envelope, event) =>
  array(envelope?.sourceLag).find(lag => text(lag?.sourceKind) === text(event?.sourceKind)) ??
  null

const expiryFor = envelope => {
  const generatedAt = text(envelope?.generatedAt)
  const maxStalenessSeconds = numberOrZero(envelope?.staleness?.maxStalenessSeconds)
  if (generatedAt === '' || maxStalenessSeconds <= 0) return ''
  const generatedMs = Date.parse(generatedAt)
  if (!Number.isFinite(generatedMs)) return ''
  return new Date(generatedMs + maxStalenessSeconds * 1_000).toISOString()
}

const eventSummary = (envelope, event) => {
  const sourceLag = sourceLagForEvent(envelope, event)
  const summary = {
    schema: 'openagents.world.public_activity_event_summary.v1',
    authority: 'worker_d1_public_projection_only',
    cursor: text(event?.cursor),
    eventRef: text(event?.eventRef),
    eventTs: text(event?.ts),
    expiresAt: expiryFor(envelope),
    kind: text(event?.kind),
    sourceKind: text(event?.sourceKind),
    sourceRefs: unique(event?.sourceRefs),
    blockerRefs: unique(event?.blockerRefs),
    caveatRefs: unique(event?.caveatRefs),
    generatedAt: text(envelope?.generatedAt),
    sourceLagStatus: text(sourceLag?.status),
    text: text(event?.text) || text(event?.state) || text(event?.eventRef),
  }
  return stableJson(summary)
}

export const buildActivityTimelineWorldPlan = (
  envelope,
  options = {},
) => {
  if (!isRecord(envelope)) {
    throw new Error('public activity timeline envelope must be an object')
  }
  if (text(envelope.schemaVersion) !== 'openagents.public_activity_timeline.v1') {
    throw new Error('unsupported public activity timeline schema')
  }
  if (activityTimelineEnvelopeHasUnsafeMaterial(envelope)) {
    throw new Error('public activity timeline bridge input contains private material')
  }

  const sourceUrl = options.sourceUrl ?? DEFAULT_ACTIVITY_TIMELINE_SOURCE_URL
  const calls = []
  const events = array(envelope.events)
  const sourceHash = stableHash(envelope)
  const generatedAt = text(envelope.generatedAt)

  events.forEach(event => {
    const eventRef = text(event?.eventRef)
    const cursor = text(event?.cursor)
    if (eventRef === '' || cursor === '') return
    const runRef = text(event?.runRef) || DEFAULT_ACTIVITY_WORLD_RUN_REF
    const sourceRef = primarySourceRef(event)
    addCall(calls, 'append_world_event', [
      `world_event.public_activity.${stableHash([
        cursor,
        eventRef,
        sourceRefsForEvent(event),
      ]).slice(0, 24)}`,
      runRef,
      text(event?.kind),
      entityRefForEvent(event),
      sourceRef,
      generatedAt,
      eventSummary(envelope, event),
    ])
  })

  addCall(calls, 'record_projection_cursor', [
    'public-activity-timeline',
    sourceUrl,
    generatedAt,
    sourceHash,
    calls.length + 1,
  ])

  const plan = {
    bridgeRef: DEFAULT_ACTIVITY_BRIDGE_REF,
    database: options.database ?? DEFAULT_DATABASE,
    sourceGeneratedAt: generatedAt,
    sourceHash,
    sourceNextCursor: envelope.nextCursor ?? null,
    sourceUrl,
    calls,
  }

  assertNoDuplicateWorldEvents(plan)
  assertWorldEventsAreSourced(plan)
  assertActivityWorldEventsMirrorTimeline(plan, envelope)
  assertActivityWorldPlanPublicSafe(plan)

  return plan
}

export const assertActivityWorldEventsMirrorTimeline = (plan, envelope) => {
  const timelineEvents = array(envelope?.events).filter(event => text(event?.cursor) !== '')
  const worldEvents = plan.calls.filter(call => call.reducer === 'append_world_event')
  if (worldEvents.length !== timelineEvents.length) {
    throw new Error(
      `world_event row count ${worldEvents.length} does not mirror timeline event count ${timelineEvents.length}`,
    )
  }
  timelineEvents.forEach((event, index) => {
    const call = worldEvents[index]
    const summary = JSON.parse(call.args[6])
    if (summary.eventRef !== text(event.eventRef) || summary.cursor !== text(event.cursor)) {
      throw new Error(`world_event ${call.args[0]} does not mirror timeline event ${text(event.eventRef)}`)
    }
  })
}

export const assertActivityWorldPlanPublicSafe = plan => {
  if (privateMaterialPatterns.some(pattern => pattern.test(JSON.stringify(plan)))) {
    throw new Error('public activity world projection plan contains private material')
  }
}

export const activityWorldReducerCounts = plan =>
  plan.calls.reduce((counts, call) => {
    counts[call.reducer] = (counts[call.reducer] ?? 0) + 1
    return counts
  }, {})
