import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisForumPublicationDeliveryState = S.Literals([
  'blocked',
  'delivered',
  'failed',
  'queued',
  'ready',
])
export type ArtanisForumPublicationDeliveryState =
  typeof ArtanisForumPublicationDeliveryState.Type

export const ArtanisForumTopicAvailability = S.Literals([
  'archived',
  'hidden',
  'locked',
  'open',
  'unavailable',
])
export type ArtanisForumTopicAvailability =
  typeof ArtanisForumTopicAvailability.Type

export class ArtanisForumPublicationIntentRecord extends S.Class<ArtanisForumPublicationIntentRecord>(
  'ArtanisForumPublicationIntentRecord',
)({
  artifactRefs: S.Array(S.String),
  authorAgentId: S.String,
  blockerRefs: S.Array(S.String),
  bodyText: S.String,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  deliveredAtIso: S.NullOr(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  deliveryState: ArtanisForumPublicationDeliveryState,
  goalRefs: S.Array(S.String),
  idempotencyKey: S.String,
  intentRef: S.String,
  modelLabReportRefs: S.Array(S.String),
  pageUrls: S.Array(S.String),
  postRef: S.NullOr(S.String),
  pylonNexusPublicRefs: S.Array(S.String),
  r10ClaimRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  redactionPolicyRef: S.String,
  sourceRefs: S.Array(S.String),
  targetForumRef: S.String,
  targetTopicRef: S.String,
  targetTopicState: ArtanisForumTopicAvailability,
  updatedAtIso: S.String,
}) {}

export class ArtanisForumPublicationQueueRecord extends S.Class<ArtanisForumPublicationQueueRecord>(
  'ArtanisForumPublicationQueueRecord',
)({
  agentId: S.String,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  intents: S.Array(ArtanisForumPublicationIntentRecord),
  queueRef: S.String,
  redactionPolicyRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisForumPublicationIntentProjection extends S.Class<ArtanisForumPublicationIntentProjection>(
  'ArtanisForumPublicationIntentProjection',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  bodyText: S.String,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deliveredAtDisplay: S.NullOr(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  deliveryState: ArtanisForumPublicationDeliveryState,
  goalRefs: S.Array(S.String),
  idempotencyKey: S.String,
  intentRef: S.String,
  modelLabReportRefs: S.Array(S.String),
  pageUrls: S.Array(S.String),
  postRef: S.NullOr(S.String),
  pylonNexusPublicRefs: S.Array(S.String),
  r10ClaimRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  redactionPolicyRef: S.String,
  sourceRefs: S.Array(S.String),
  targetForumRef: S.String,
  targetTopicRef: S.String,
  targetTopicState: ArtanisForumTopicAvailability,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisForumPublicationQueueProjection extends S.Class<ArtanisForumPublicationQueueProjection>(
  'ArtanisForumPublicationQueueProjection',
)({
  agentId: S.String,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deliverableIntentRefs: S.Array(S.String),
  deliveredCount: S.Number,
  duplicateIntentRefs: S.Array(S.String),
  intentCount: S.Number,
  intents: S.Array(ArtanisForumPublicationIntentProjection),
  queueRef: S.String,
  redactionPolicyRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisForumPublicationUnsafe extends S.TaggedErrorClass<ArtanisForumPublicationUnsafe>()(
  'ArtanisForumPublicationUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const unsafeBodyPattern =
  /(\/Users\/|\/home\/|access token|auth\.json|bearer [A-Za-z0-9._-]+|customer email|customer prompt|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice raw|lnbc|lntb|lnbcrt|macaroon|mnemonic|payment hash|payment preimage|preimage|private repo|provider token|raw log|raw prompt|runner token|secret|sk-[a-z0-9]|wallet seed|wallet secret|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})/i
const safePageUrlPattern =
  /^https:\/\/(openagents\.com|sites\.openagents\.com|nexus\.openagents\.com)(\/[A-Za-z0-9._~:/-]*)?$/
const openPostStates: ReadonlyArray<ArtanisForumPublicationDeliveryState> = [
  'delivered',
  'queued',
  'ready',
]

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisForumPublicationUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertMaybeIso = (label: string, iso: string | null): void => {
  if (iso !== null) {
    assertValidIso(label, iso)
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisForumPublicationUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, raw timestamp, or private material.`,
    })
  }
}

const assertRefsStartWith = (
  label: string,
  refs: ReadonlyArray<string>,
  allowedPrefixes: ReadonlyArray<string>,
): void => {
  assertSafeRefs(label, refs)

  const unsupported = uniqueRefs(refs).find(
    ref => !allowedPrefixes.some(prefix => ref.startsWith(prefix)),
  )

  if (unsupported !== undefined) {
    throw new ArtanisForumPublicationUnsafe({
      reason: `${label} must use public-safe refs with one of these prefixes: ${allowedPrefixes.join(', ')}.`,
    })
  }
}

const assertSafePageUrls = (urls: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(urls).find(
    url =>
      !safePageUrlPattern.test(url) ||
      unsafeRefPattern.test(url) ||
      rawTimestampPattern.test(url),
  )

  if (unsafe !== undefined) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Artanis Forum publication page URLs must be public OpenAgents, Sites, or Nexus URLs without query strings, fragments, secrets, raw timestamps, payment material, or private refs.',
    })
  }
}

const assertBodyText = (bodyText: string): void => {
  if (bodyText.trim().length < 12 || bodyText.length > 4000) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Artanis Forum publication body text must be a bounded public-safe summary.',
    })
  }

  if (unsafeBodyPattern.test(bodyText) || rawTimestampPattern.test(bodyText)) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Artanis Forum publication body text contains private, raw, wallet, payment, provider, customer, email, secret, or raw timestamp material.',
    })
  }
}

const assertRedactionPolicy = (redactionPolicyRef: string): void => {
  assertSafeRefs('Artanis Forum publication redaction policy ref', [
    redactionPolicyRef,
  ])

  if (!redactionPolicyRef.startsWith('redaction.forum.public.')) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Artanis Forum publication intents require a public Forum redaction policy ref.',
    })
  }
}

const assertIntent = (
  intent: ArtanisForumPublicationIntentRecord,
): void => {
  assertValidIso('intent.createdAtIso', intent.createdAtIso)
  assertValidIso('intent.updatedAtIso', intent.updatedAtIso)
  assertMaybeIso('intent.deliveredAtIso', intent.deliveredAtIso)
  assertBodyText(intent.bodyText)
  assertRedactionPolicy(intent.redactionPolicyRef)
  assertSafeRefs('Artanis publication author agent id', [intent.authorAgentId])
  assertSafeRefs('Artanis publication intent ref', [intent.intentRef])
  assertSafeRefs('Artanis publication idempotency key', [intent.idempotencyKey])
  assertSafeRefs('Artanis target forum ref', [intent.targetForumRef])
  assertSafeRefs('Artanis target topic ref', [intent.targetTopicRef])
  assertSafeRefs('Artanis publication blocker refs', intent.blockerRefs)
  assertSafeRefs('Artanis publication caveat refs', intent.caveatRefs)
  assertSafeRefs(
    'Artanis publication delivery receipt refs',
    intent.deliveryReceiptRefs,
  )
  assertSafeRefs('Artanis publication post ref', [intent.postRef ?? 'post.none'])
  assertRefsStartWith('Artanis publication source refs', intent.sourceRefs, [
    'artifact.public.',
    'campaign.public.',
    'claim.public.',
    'context.public.',
    'evidence.public.',
    'forum.public.',
    'goal.public.',
    'loop.public.',
    'model_lab.public.',
    'nexus.public.',
    'omega.public.',
    'pylon.public.',
    'receipt.public.',
    'report.public.',
  ])
  assertRefsStartWith('Artanis publication goal refs', intent.goalRefs, [
    'goal.public.',
  ])
  assertRefsStartWith('Artanis publication R10 claim refs', intent.r10ClaimRefs, [
    'claim.public.r10.',
    'r10.public.',
  ])
  assertRefsStartWith(
    'Artanis publication Pylon/Nexus public refs',
    intent.pylonNexusPublicRefs,
    ['campaign.public.', 'nexus.public.', 'omega.public.', 'pylon.public.'],
  )
  assertRefsStartWith(
    'Artanis publication Model Lab report refs',
    intent.modelLabReportRefs,
    ['model_lab.public.', 'report.public.model_lab.'],
  )
  assertRefsStartWith('Artanis publication artifact refs', intent.artifactRefs, [
    'artifact.public.',
  ])
  assertRefsStartWith('Artanis publication receipt refs', intent.receiptRefs, [
    'receipt.public.',
  ])
  assertSafePageUrls(intent.pageUrls)

  if (intent.authorAgentId !== 'agent_artanis') {
    throw new ArtanisForumPublicationUnsafe({
      reason: 'Artanis Forum publication intents must use agent_artanis.',
    })
  }

  if (
    openPostStates.includes(intent.deliveryState) &&
    intent.targetTopicState !== 'open'
  ) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Artanis Forum publication cannot post to locked, hidden, archived, or unavailable topics.',
    })
  }

  if (intent.deliveryState === 'blocked' && !hasAny(intent.blockerRefs)) {
    throw new ArtanisForumPublicationUnsafe({
      reason: 'Blocked Artanis Forum publication intents require blocker refs.',
    })
  }

  if (intent.deliveryState === 'delivered') {
    if (intent.postRef === null || intent.deliveredAtIso === null) {
      throw new ArtanisForumPublicationUnsafe({
        reason:
          'Delivered Artanis Forum publication intents require post and delivered-at refs.',
      })
    }
  }

  if (
    intent.deliveryState !== 'delivered' &&
    (intent.postRef !== null || intent.deliveredAtIso !== null)
  ) {
    throw new ArtanisForumPublicationUnsafe({
      reason:
        'Only delivered Artanis Forum publication intents can carry post refs or delivered-at times.',
    })
  }
}

const intentFingerprint = (
  intent: ArtanisForumPublicationIntentRecord,
): string =>
  JSON.stringify({
    artifactRefs: uniqueRefs(intent.artifactRefs),
    authorAgentId: intent.authorAgentId,
    blockerRefs: uniqueRefs(intent.blockerRefs),
    bodyText: intent.bodyText.trim(),
    caveatRefs: uniqueRefs(intent.caveatRefs),
    deliveryReceiptRefs: uniqueRefs(intent.deliveryReceiptRefs),
    deliveryState: intent.deliveryState,
    goalRefs: uniqueRefs(intent.goalRefs),
    idempotencyKey: intent.idempotencyKey,
    modelLabReportRefs: uniqueRefs(intent.modelLabReportRefs),
    pageUrls: uniqueRefs(intent.pageUrls),
    postRef: intent.postRef,
    pylonNexusPublicRefs: uniqueRefs(intent.pylonNexusPublicRefs),
    r10ClaimRefs: uniqueRefs(intent.r10ClaimRefs),
    receiptRefs: uniqueRefs(intent.receiptRefs),
    redactionPolicyRef: intent.redactionPolicyRef,
    sourceRefs: uniqueRefs(intent.sourceRefs),
    targetForumRef: intent.targetForumRef,
    targetTopicRef: intent.targetTopicRef,
    targetTopicState: intent.targetTopicState,
  })

const canonicalIntents = (
  intents: ReadonlyArray<ArtanisForumPublicationIntentRecord>,
): ReadonlyArray<ArtanisForumPublicationIntentRecord> =>
  intents.filter(
    (intent, index) =>
      intents.findIndex(
        other => other.idempotencyKey === intent.idempotencyKey,
      ) === index,
  )

const duplicateIntentRefs = (
  intents: ReadonlyArray<ArtanisForumPublicationIntentRecord>,
): ReadonlyArray<string> =>
  uniqueRefs(
    intents
      .filter(
        (intent, index) =>
          intents.findIndex(
            other => other.idempotencyKey === intent.idempotencyKey,
          ) !== index,
      )
      .map(intent => intent.intentRef),
  )

const assertIdempotentRetries = (
  intents: ReadonlyArray<ArtanisForumPublicationIntentRecord>,
): void => {
  const seen = new Map<string, string>()

  intents.forEach(intent => {
    const fingerprint = intentFingerprint(intent)
    const existing = seen.get(intent.idempotencyKey)

    if (existing !== undefined && existing !== fingerprint) {
      throw new ArtanisForumPublicationUnsafe({
        reason:
          'Artanis Forum publication idempotency keys cannot be reused for different payloads.',
      })
    }

    seen.set(intent.idempotencyKey, fingerprint)
  })
}

const assertQueue = (queue: ArtanisForumPublicationQueueRecord): void => {
  assertValidIso('queue.createdAtIso', queue.createdAtIso)
  assertValidIso('queue.updatedAtIso', queue.updatedAtIso)
  assertRedactionPolicy(queue.redactionPolicyRef)
  assertSafeRefs('Artanis Forum publication queue agent id', [queue.agentId])
  assertSafeRefs('Artanis Forum publication queue ref', [queue.queueRef])
  assertSafeRefs('Artanis Forum publication queue caveat refs', queue.caveatRefs)

  if (queue.agentId !== 'agent_artanis') {
    throw new ArtanisForumPublicationUnsafe({
      reason: 'Artanis Forum publication queues must use agent_artanis.',
    })
  }

  if (!hasAny(queue.intents)) {
    throw new ArtanisForumPublicationUnsafe({
      reason: 'Artanis Forum publication queues require intents.',
    })
  }

  queue.intents.forEach(assertIntent)
  assertIdempotentRetries(queue.intents)
}

const projectIntent = (
  intent: ArtanisForumPublicationIntentRecord,
  nowIso: string,
): ArtanisForumPublicationIntentProjection => ({
  artifactRefs: uniqueRefs(intent.artifactRefs),
  blockerRefs: uniqueRefs(intent.blockerRefs),
  bodyText: intent.bodyText.trim(),
  caveatRefs: uniqueRefs(intent.caveatRefs),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    intent.createdAtIso,
    nowIso,
  ),
  deliveredAtDisplay: intent.deliveredAtIso === null
    ? null
    : friendlyBlueprintMissionBriefingTime(intent.deliveredAtIso, nowIso),
  deliveryReceiptRefs: uniqueRefs(intent.deliveryReceiptRefs),
  deliveryState: intent.deliveryState,
  goalRefs: uniqueRefs(intent.goalRefs),
  idempotencyKey: intent.idempotencyKey,
  intentRef: intent.intentRef,
  modelLabReportRefs: uniqueRefs(intent.modelLabReportRefs),
  pageUrls: uniqueRefs(intent.pageUrls),
  postRef: intent.postRef,
  pylonNexusPublicRefs: uniqueRefs(intent.pylonNexusPublicRefs),
  r10ClaimRefs: uniqueRefs(intent.r10ClaimRefs),
  receiptRefs: uniqueRefs(intent.receiptRefs),
  redactionPolicyRef: intent.redactionPolicyRef,
  sourceRefs: uniqueRefs(intent.sourceRefs),
  targetForumRef: intent.targetForumRef,
  targetTopicRef: intent.targetTopicRef,
  targetTopicState: intent.targetTopicState,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    intent.updatedAtIso,
    nowIso,
  ),
})

export const projectArtanisForumPublicationQueue = (
  queue: ArtanisForumPublicationQueueRecord,
  nowIso: string,
): ArtanisForumPublicationQueueProjection => {
  assertQueue(queue)

  const intents = canonicalIntents(queue.intents)
  const deliverableIntentRefs = intents
    .filter(
      intent =>
        intent.deliveryState === 'ready' && intent.targetTopicState === 'open',
    )
    .map(intent => intent.intentRef)

  return {
    agentId: queue.agentId,
    caveatRefs: uniqueRefs(queue.caveatRefs),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      queue.createdAtIso,
      nowIso,
    ),
    deliverableIntentRefs: uniqueRefs(deliverableIntentRefs),
    deliveredCount: intents.filter(intent => intent.deliveryState === 'delivered')
      .length,
    duplicateIntentRefs: duplicateIntentRefs(queue.intents),
    intentCount: intents.length,
    intents: intents.map(intent => projectIntent(intent, nowIso)),
    queueRef: queue.queueRef,
    redactionPolicyRef: queue.redactionPolicyRef,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      queue.updatedAtIso,
      nowIso,
    ),
  }
}

export const selectReadyArtanisForumPublicationIntents = (
  queue: ArtanisForumPublicationQueueRecord,
): ReadonlyArray<ArtanisForumPublicationIntentRecord> => {
  assertQueue(queue)

  return canonicalIntents(queue.intents).filter(
    intent =>
      intent.deliveryState === 'ready' && intent.targetTopicState === 'open',
  )
}

const projectionStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStringValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStringValues)
  }

  return []
}

export const artanisForumPublicationProjectionHasPrivateMaterial = (
  projection: ArtanisForumPublicationQueueProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      unsafeRefPattern.test(value) ||
      unsafeBodyPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

export const exampleArtanisForumPublicationQueue = (): ArtanisForumPublicationQueueRecord => ({
  agentId: 'agent_artanis',
  caveatRefs: ['caveat.public.forum_publication_queue_evidence_only'],
  createdAtIso: '2026-06-07T01:20:00.000Z',
  intents: [
    {
      artifactRefs: ['artifact.public.artanis.status_packet'],
      authorAgentId: 'agent_artanis',
      blockerRefs: [],
      bodyText:
        'Artanis status update: Pylon v0.2 release work is active, Model Lab evidence is being gathered, and public proofs will be linked as they are accepted.',
      caveatRefs: ['caveat.public.no_private_operator_evidence'],
      createdAtIso: '2026-06-07T01:21:00.000Z',
      deliveredAtIso: null,
      deliveryReceiptRefs: [],
      deliveryState: 'ready',
      goalRefs: ['goal.public.artanis.pylon_model_lab'],
      idempotencyKey: 'artanis-forum:status:20260607T0121:v1',
      intentRef: 'forum.public.artanis.status_intent.20260607T0121',
      modelLabReportRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
      pageUrls: [
        'https://openagents.com/artanis',
        'https://openagents.com/forum/f/artanis',
      ],
      postRef: null,
      pylonNexusPublicRefs: [
        'campaign.public.pylon.v0_2',
        'omega.public.pylon_api.registrations',
        'pylon.public.resource_modes',
      ],
      r10ClaimRefs: ['claim.public.r10.pylon_learning_loop'],
      receiptRefs: ['receipt.public.artanis.loop_closeout'],
      redactionPolicyRef: 'redaction.forum.public.artanis.v1',
      sourceRefs: [
        'artifact.public.artanis.status_packet',
        'campaign.public.pylon.v0_2',
        'goal.public.artanis.pylon_model_lab',
        'model_lab.public.report.autopilot_benchmark_loop',
        'omega.public.pylon_api.registrations',
        'receipt.public.artanis.loop_closeout',
      ],
      targetForumRef: 'forum.public.artanis',
      targetTopicRef: 'topic.public.forum.artanis.status',
      targetTopicState: 'open',
      updatedAtIso: '2026-06-07T01:22:00.000Z',
    },
  ],
  queueRef: 'queue.public.artanis.forum_publications',
  redactionPolicyRef: 'redaction.forum.public.artanis.v1',
  updatedAtIso: '2026-06-07T01:22:00.000Z',
})
