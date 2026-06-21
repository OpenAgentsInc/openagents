import {
  DEFAULT_DATABASE,
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
  stableHash,
  stableJson,
} from './tassadar-summary-transform.mjs'

// BF-2 (#5905): forum-activity -> world projection plan (pure transform).
//
// Maps the public-safe /api/public/forum-activity envelope (BF-1, #5904) into an
// idempotent set of `append_world_event` reducer calls for the openagents-world
// SpacetimeDB module. Only a service identity may write world events
// (ensure_service); the project-forum-activity runner applies this plan under the
// authorized identity, exactly like project-activity-timeline / project-tassadar-
// summary.
//
// Idempotent: each world event_ref is a stable hash of the forum row's identity,
// and `append_world_event` no-ops on a duplicate event_ref — so re-running the
// bridge never double-writes. (record_system_world_message bubbles are NOT in the
// plan: that reducer inserts a new row every call and would duplicate per tick;
// BF-3 renders the pylon message icon from the idempotent world_event instead.)
//
// Public-safe: the plan is asserted to carry no private material; forum/business
// authority stays in the Worker/D1 — SpacetimeDB holds only this projection.

export const DEFAULT_FORUM_ACTIVITY_SOURCE_URL =
  'https://openagents.com/api/public/forum-activity?limit=50'
export const DEFAULT_FORUM_ACTIVITY_RUN_REF = 'run.public_forum_activity'
export const DEFAULT_FORUM_ACTIVITY_BRIDGE_REF = 'bridge.public-forum-activity'

const FORUM_EVENT_KINDS = new Set([
  'forum_post',
  'forum_reply',
  'forum_tip_settled',
])

const text = value => (typeof value === 'string' ? value.trim() : '')
const array = value => (Array.isArray(value) ? value : [])
const isRecord = value =>
  value !== null && typeof value === 'object' && !Array.isArray(value)

const privateMaterialPatterns = [
  /\b(?:raw[_-]?(?:prompt|trace|payload|log)|provider[_-]?payload)\b/i,
  /\b(?:secret|token_secret|service_token|bearer\s+[a-z0-9._-]{6,})\b/i,
  /\b(?:payment_preimage|payment_hash|invoice|bolt11|mnemonic|wallet_seed|xprv)\b/i,
  /\boa_agent_[a-z0-9]/i,
  /\bspark1[a-z0-9]/i,
  /\bsk-[a-z0-9_-]+\b/i,
  /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/i,
]

export const forumActivityEnvelopeHasUnsafeMaterial = envelope =>
  privateMaterialPatterns.some(pattern => pattern.test(JSON.stringify(envelope)))

const addCall = (calls, reducer, args) => {
  calls.push({ reducer, args })
}

const eventSummary = (envelope, row) =>
  stableJson({
    schema: 'openagents.world.forum_activity_event_summary.v1',
    authority: 'worker_d1_public_projection_only',
    eventKind: text(row?.eventKind),
    eventRef: text(row?.eventRef),
    sourceRef: text(row?.sourceRef),
    topicRef: text(row?.topicRef),
    agentRef: text(row?.agentRef),
    pylonRef: text(row?.pylonRef),
    sourceGeneratedAt: text(row?.sourceGeneratedAt),
    generatedAt: text(envelope?.generatedAt),
    text: text(row?.summary),
  })

/**
 * Build the world projection plan from a public forum-activity envelope.
 * Pure; throws on a malformed or unsafe envelope.
 */
export const buildForumActivityWorldPlan = (envelope, options = {}) => {
  if (!isRecord(envelope)) {
    throw new Error('public forum-activity envelope must be an object')
  }
  if (!Array.isArray(envelope.activity)) {
    throw new Error('public forum-activity envelope must carry an activity array')
  }
  if (forumActivityEnvelopeHasUnsafeMaterial(envelope)) {
    throw new Error('public forum-activity bridge input contains private material')
  }

  const sourceUrl = options.sourceUrl ?? DEFAULT_FORUM_ACTIVITY_SOURCE_URL
  const runRef = options.runRef ?? DEFAULT_FORUM_ACTIVITY_RUN_REF
  const generatedAt = text(envelope.generatedAt)
  const sourceHash = stableHash(envelope)
  const calls = []

  array(envelope.activity).forEach(row => {
    const eventKind = text(row?.eventKind)
    const eventRef = text(row?.eventRef)
    const sourceRef = text(row?.sourceRef)
    const agentRef = text(row?.agentRef)
    // Skip malformed / unknown-kind rows rather than writing junk world events.
    if (!FORUM_EVENT_KINDS.has(eventKind)) return
    if (eventRef === '' || sourceRef === '' || agentRef === '') return
    addCall(calls, 'append_world_event', [
      `world_event.forum_activity.${stableHash([
        eventKind,
        eventRef,
        sourceRef,
      ]).slice(0, 24)}`,
      runRef,
      eventKind,
      agentRef,
      sourceRef,
      text(row?.sourceGeneratedAt) || generatedAt,
      eventSummary(envelope, row),
    ])
  })

  addCall(calls, 'record_projection_cursor', [
    'public-forum-activity',
    sourceUrl,
    generatedAt,
    sourceHash,
    calls.length + 1,
  ])

  const plan = {
    bridgeRef: DEFAULT_FORUM_ACTIVITY_BRIDGE_REF,
    database: options.database ?? DEFAULT_DATABASE,
    sourceGeneratedAt: generatedAt,
    sourceHash,
    sourceUrl,
    calls,
  }

  assertNoDuplicateWorldEvents(plan)
  assertWorldEventsAreSourced(plan)
  assertForumWorldPlanPublicSafe(plan)

  return plan
}

export const assertForumWorldPlanPublicSafe = plan => {
  if (privateMaterialPatterns.some(pattern => pattern.test(JSON.stringify(plan)))) {
    throw new Error('public forum-activity world projection plan contains private material')
  }
}

export const forumWorldReducerCounts = plan =>
  plan.calls.reduce((counts, call) => {
    counts[call.reducer] = (counts[call.reducer] ?? 0) + 1
    return counts
  }, {})
