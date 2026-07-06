import { Effect, Schema as S } from 'effect'

import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationQueueRecord,
  selectReadyArtanisForumPublicationIntents,
} from './artanis-forum-publication'
import {
  ArtanisPersistenceError,
  markArtanisForumPublicationIntentDelivered,
  readLatestArtanisPersistedRows,
} from './artanis-persistence'
import {
  artanisAuthorityDb,
  type ArtanisDatabase,
} from './artanis-domain-store'
import {
  ArtanisForumIdentityError,
  isArtanisForumPostActor,
  resolveRegisteredArtanisForumIdentityFromD1,
} from './artanis-forum-identity'
import type { IdentityDb } from './identity-db'
import {
  type ForumPostSummary,
  ForumPublicProjection,
  ForumPublicProjectionUnsafe,
  ForumReadAccessDenied,
  type ForumRepositoryError,
  type ForumRepositoryRuntime,
  ForumStorageError,
  ForumValidationError,
  ForumWriterAuthFailure,
  ForumWriterGrant,
  buildForumWriterContext,
  createForumReplyPost,
  readForumPostByIdempotencyKey,
  readForumSummaryByRef,
  readForumTopicById,
} from './forum'
import { decodeUnknownWithSchema } from './json-boundary'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

export const ARTANIS_FORUM_ID = '88888888-3333-4333-8333-888888888888'
export const ARTANIS_FORUM_REF = 'forum.public.artanis'

const ArtanisCanonicalTopicKey = S.Literals([
  'bitcoin_accounting',
  'model_lab',
  'operator_questions',
  'pylon_campaign',
  'pylon_release_work_log',
  'resource_modes',
  'status',
  'work_routing',
])
type ArtanisCanonicalTopicKey = typeof ArtanisCanonicalTopicKey.Type

export const ARTANIS_CANONICAL_TOPIC_IDS: Readonly<
  Record<ArtanisCanonicalTopicKey, string>
> = {
  bitcoin_accounting: '88888888-4006-4006-8006-888888888888',
  model_lab: '88888888-4003-4003-8003-888888888888',
  operator_questions: '88888888-4008-4008-8008-888888888888',
  pylon_campaign: '88888888-4002-4002-8002-888888888888',
  pylon_release_work_log: '88888888-4004-4004-8004-888888888888',
  resource_modes: '88888888-4007-4007-8007-888888888888',
  status: '88888888-4001-4001-8001-888888888888',
  work_routing: '88888888-4005-4005-8005-888888888888',
}

export type ArtanisForumDeliveryForbiddenAuthority = Readonly<{
  moderationAllowed: false
  paymentSpendAllowed: false
  providerMutationAllowed: false
  trainingLaunchAllowed: false
  walletSpendAllowed: false
}>

export const ARTANIS_FORUM_DELIVERY_NO_EXTRA_AUTHORITY: ArtanisForumDeliveryForbiddenAuthority =
  {
    moderationAllowed: false,
    paymentSpendAllowed: false,
    providerMutationAllowed: false,
    trainingLaunchAllowed: false,
    walletSpendAllowed: false,
  }

export class ArtanisForumDeliveryError extends S.TaggedErrorClass<ArtanisForumDeliveryError>()(
  'ArtanisForumDeliveryError',
  {
    kind: S.Literals([
      'existing_post_conflict',
      'idempotency_key_missing',
      'identity_unavailable',
      'persistence_error',
      'storage_error',
      'target_blocked',
      'target_not_found',
      'target_ref_unsupported',
      'unsafe_intent',
      'writer_error',
    ]),
    reason: S.String,
  },
) {}

export type ArtanisForumDeliveryErrorKind = ArtanisForumDeliveryError['kind']

export type ArtanisForumDeliveredPost = Readonly<{
  deliveryReceiptRefs: ReadonlyArray<string>
  forbiddenAuthority: ArtanisForumDeliveryForbiddenAuthority
  idempotencyKey: string
  idempotent: boolean
  intentRef: string
  postId: string
  postNumber: number
  postRef: string
  publicUrl: string
  targetTopicRef: string
}>

export type ArtanisForumDeliveryBatchResult = Readonly<{
  checkedIntentRefs: ReadonlyArray<string>
  delivered: ReadonlyArray<ArtanisForumDeliveredPost>
  forbiddenAuthority: ArtanisForumDeliveryForbiddenAuthority
}>

type DeliveryRuntime = Readonly<{
  makeId: () => string
  nowEpochMillis: () => number
  nowIso: () => string
}>

const decodeForumPublicProjection = S.decodeUnknownSync(ForumPublicProjection)
const decodeForumWriterGrant = S.decodeUnknownSync(ForumWriterGrant)

const topicRefAliases: Readonly<Record<string, ArtanisCanonicalTopicKey>> = {
  'topic.public.forum.artanis.bitcoin_accounting': 'bitcoin_accounting',
  'topic.public.forum.artanis.bitcoin_rewards': 'bitcoin_accounting',
  'topic.public.forum.artanis.model_lab': 'model_lab',
  'topic.public.forum.artanis.operator_questions': 'operator_questions',
  'topic.public.forum.artanis.pylon_campaign': 'pylon_campaign',
  'topic.public.forum.artanis.pylon_release': 'pylon_release_work_log',
  'topic.public.forum.artanis.pylon_release_work_log': 'pylon_release_work_log',
  'topic.public.forum.artanis.resource_modes': 'resource_modes',
  'topic.public.forum.artanis.status': 'status',
  'topic.public.forum.artanis.work_routing': 'work_routing',
}

const deliveryRuntime = (
  runtime: Partial<DeliveryRuntime> | undefined,
): DeliveryRuntime => ({
  makeId: runtime?.makeId ?? randomUuid,
  nowEpochMillis: runtime?.nowEpochMillis ?? currentEpochMillis,
  nowIso: runtime?.nowIso ?? currentIsoTimestamp,
})

const safeRefSuffix = (ref: string): string => {
  const suffix = ref
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
    .slice(0, 120)

  return suffix === '' ? 'intent' : suffix
}

const receiptRefForIntent = (intentRef: string): string =>
  `receipt.public.artanis.forum_delivery.${safeRefSuffix(intentRef)}`

const targetTopicKey = (topicRef: string): ArtanisCanonicalTopicKey | null =>
  topicRefAliases[topicRef] ?? null

const targetTopicId = (topicRef: string): string | null => {
  const key = targetTopicKey(topicRef)

  return key === null ? null : ARTANIS_CANONICAL_TOPIC_IDS[key]
}

const publicProjectionForIntent = (
  intent: ArtanisForumPublicationIntentRecord,
) =>
  decodeForumPublicProjection({
    classificationCaveatRef: 'classification.public_forum_projection',
    customerSafe: true,
    dataClassification: 'public',
    excludedPrivateRefs: [],
    publicSafe: true,
    redactionPolicyRef: intent.redactionPolicyRef,
    safeArtifactRefs: intent.artifactRefs,
    safeReceiptRefs: [...intent.receiptRefs, ...intent.deliveryReceiptRefs],
    trustTier: 'reviewed',
  })

const postRefForPost = (
  intent: ArtanisForumPublicationIntentRecord,
  post: ForumPostSummary,
): string => {
  const key = targetTopicKey(intent.targetTopicRef) ?? 'status'

  return `post.public.forum.artanis.${key}.${post.postNumber}`
}

const deliveredPost = (
  intent: ArtanisForumPublicationIntentRecord,
  post: ForumPostSummary,
  idempotent: boolean,
): ArtanisForumDeliveredPost => ({
  deliveryReceiptRefs: [receiptRefForIntent(intent.intentRef)],
  forbiddenAuthority: ARTANIS_FORUM_DELIVERY_NO_EXTRA_AUTHORITY,
  idempotencyKey: intent.idempotencyKey,
  idempotent,
  intentRef: intent.intentRef,
  postId: post.postId,
  postNumber: post.postNumber,
  postRef: postRefForPost(intent, post),
  publicUrl: `/forum/t/${post.topicId}#post-${post.postId}`,
  targetTopicRef: intent.targetTopicRef,
})

const deliveryError = (kind: ArtanisForumDeliveryErrorKind, reason: string) =>
  new ArtanisForumDeliveryError({ kind, reason })

const mapDependencyError = (error: unknown): ArtanisForumDeliveryError => {
  if (error instanceof ArtanisForumDeliveryError) {
    return error
  }

  if (error instanceof ArtanisPersistenceError) {
    return deliveryError('persistence_error', error.reason)
  }

  if (error instanceof ArtanisForumIdentityError) {
    return deliveryError('identity_unavailable', error.reason)
  }

  if (error instanceof ForumPublicProjectionUnsafe) {
    return deliveryError('unsafe_intent', error.reason)
  }

  if (
    error instanceof ForumWriterAuthFailure ||
    error instanceof ForumValidationError
  ) {
    return deliveryError('writer_error', error.reason)
  }

  if (
    error instanceof ForumStorageError ||
    error instanceof ForumReadAccessDenied
  ) {
    return deliveryError(
      'storage_error',
      'Forum delivery target was unreadable.',
    )
  }

  return deliveryError('storage_error', 'Artanis Forum delivery failed.')
}

const assertReadyIntent = (
  intent: ArtanisForumPublicationIntentRecord,
): Effect.Effect<void, ArtanisForumDeliveryError> =>
  Effect.try({
    catch: error =>
      error instanceof Error
        ? deliveryError('unsafe_intent', error.message)
        : deliveryError('unsafe_intent', String(error)),
    try: () =>
      selectReadyArtanisForumPublicationIntents(
        new ArtanisForumPublicationQueueRecord({
          agentId: 'agent_artanis',
          caveatRefs: intent.caveatRefs,
          createdAtIso: intent.createdAtIso,
          intents: [intent],
          queueRef: 'queue.public.artanis.delivery',
          redactionPolicyRef: intent.redactionPolicyRef,
          updatedAtIso: intent.updatedAtIso,
        }),
      ),
  }).pipe(
    Effect.flatMap(intents =>
      intents.length === 1
        ? Effect.void
        : Effect.fail(
            deliveryError(
              'unsafe_intent',
              'Artanis Forum delivery requires one ready public-safe intent.',
            ),
          ),
    ),
    Effect.asVoid,
  )

const markDelivered = (
  db: ArtanisDatabase,
  intent: ArtanisForumPublicationIntentRecord,
  delivered: ArtanisForumDeliveredPost,
  nowIso: string,
) =>
  markArtanisForumPublicationIntentDelivered(db, intent.intentRef, {
    deliveredAtIso: nowIso,
    deliveryReceiptRefs: delivered.deliveryReceiptRefs,
    postRef: delivered.postRef,
    updatedAtIso: nowIso,
  }).pipe(Effect.mapError(mapDependencyError))

export const deliverArtanisForumPublicationIntent = (
  db: ArtanisDatabase,
  // CFG-4 Domain 2 (#8519): Postgres identity handle for the registered
  // Artanis `users`/`auth_identities` lookup.
  identityDb: IdentityDb,
  intent: ArtanisForumPublicationIntentRecord,
  runtimeInput?: Partial<DeliveryRuntime> | undefined,
): Effect.Effect<ArtanisForumDeliveredPost, ArtanisForumDeliveryError> => {
  const runtime = deliveryRuntime(runtimeInput)
  const nowIso = runtime.nowIso()

  return Effect.gen(function* () {
    if (intent.idempotencyKey.trim() === '') {
      return yield* deliveryError(
        'idempotency_key_missing',
        'Artanis Forum publication delivery requires an idempotency key.',
      )
    }

    yield* assertReadyIntent(intent)

    if (intent.targetForumRef !== ARTANIS_FORUM_REF) {
      return yield* deliveryError(
        'target_ref_unsupported',
        'Artanis Forum delivery only supports the canonical Artanis Forum.',
      )
    }

    const forum = yield* readForumSummaryByRef(artanisAuthorityDb(db), 'artanis').pipe(
      Effect.mapError(mapDependencyError),
    )

    if (forum === null || forum.forumId !== ARTANIS_FORUM_ID) {
      return yield* deliveryError(
        'target_not_found',
        'Canonical Artanis Forum was not found.',
      )
    }

    if (forum.locked) {
      return yield* deliveryError(
        'target_blocked',
        'Canonical Artanis Forum is locked.',
      )
    }

    const topicId = targetTopicId(intent.targetTopicRef)

    if (topicId === null) {
      return yield* deliveryError(
        'target_ref_unsupported',
        'Artanis Forum publication target topic ref is unsupported.',
      )
    }

    const topic = yield* readForumTopicById(artanisAuthorityDb(db), topicId).pipe(
      Effect.mapError(mapDependencyError),
    )

    if (topic === null || topic.forumId !== forum.forumId) {
      return yield* deliveryError(
        'target_not_found',
        'Artanis Forum publication target topic was not found.',
      )
    }

    if (topic.state !== 'open') {
      return yield* deliveryError(
        'target_blocked',
        'Artanis Forum publication target topic is not open.',
      )
    }

    const identity = yield* resolveRegisteredArtanisForumIdentityFromD1(
      artanisAuthorityDb(db),
      identityDb,
      nowIso,
    ).pipe(Effect.mapError(mapDependencyError))

    const existingPost = yield* readForumPostByIdempotencyKey(
      artanisAuthorityDb(db),
      intent.idempotencyKey,
    ).pipe(Effect.mapError(mapDependencyError))

    if (existingPost !== null) {
      if (
        existingPost.topicId !== topic.topicId ||
        !isArtanisForumPostActor(existingPost.author.actorRef, identity) ||
        (existingPost.bodyText ?? '').trim() !== intent.bodyText.trim()
      ) {
        return yield* deliveryError(
          'existing_post_conflict',
          'Artanis Forum idempotency key already belongs to a different post.',
        )
      }

      const delivered = deliveredPost(intent, existingPost, true)
      yield* markDelivered(db, intent, delivered, nowIso)

      return delivered
    }

    const grant = decodeForumWriterGrant({
      expiresAtEpochMillis: runtime.nowEpochMillis() + 1000 * 60 * 60,
      forumIds: [forum.forumId],
      ownerUserId: identity.userId,
      scopes: ['forum.write'],
      status: 'active',
      teamId: null,
    })
    const writer = yield* buildForumWriterContext({
      actor: identity.actor,
      grant,
      nowEpochMillis: runtime.nowEpochMillis,
      paymentProofRef: null,
      requiredScope: 'forum.write',
      targetForumId: forum.forumId,
      targetOwnerUserId: identity.userId,
      targetTeamId: null,
    }).pipe(Effect.mapError(mapDependencyError))
    const repositoryRuntime: ForumRepositoryRuntime = {
      makeId: runtime.makeId,
      nowIso: runtime.nowIso,
    }
    const post = yield* createForumReplyPost(
      artanisAuthorityDb(db),
      {
        actor: writer.actor,
        bodyText: intent.bodyText,
        contentRef: `content.forum.artanis.delivery.${safeRefSuffix(intent.intentRef)}`,
        forumId: forum.forumId,
        idempotencyKey: intent.idempotencyKey,
        parentPostId: topic.latestPostId,
        postId: runtime.makeId(),
        publicProjection: publicProjectionForIntent(intent),
        quotePostId: null,
        topicId: topic.topicId,
      },
      repositoryRuntime,
    ).pipe(
      Effect.mapError((error: ForumRepositoryError | unknown) =>
        mapDependencyError(error),
      ),
    )
    const delivered = deliveredPost(intent, post, false)
    yield* markDelivered(db, intent, delivered, nowIso)

    return delivered
  })
}

export const deliverReadyArtanisForumPublications = (
  db: ArtanisDatabase,
  identityDb: IdentityDb,
  input: Readonly<{
    limit?: number | undefined
    runtime?: Partial<DeliveryRuntime> | undefined
  }> = {},
): Effect.Effect<ArtanisForumDeliveryBatchResult, ArtanisForumDeliveryError> =>
  Effect.gen(function* () {
    const rows = yield* readLatestArtanisPersistedRows(
      db,
      'forum_publication_intent',
      input.limit ?? 12,
    ).pipe(Effect.mapError(mapDependencyError))
    const intents = rows
      .map(row =>
        decodeUnknownWithSchema(
          ArtanisForumPublicationIntentRecord,
          row.record,
        ),
      )
      .filter(intent => intent.deliveryState === 'ready')
    const delivered = yield* Effect.forEach(
      intents,
      intent =>
        deliverArtanisForumPublicationIntent(db, identityDb, intent, input.runtime),
      { concurrency: 1 },
    )

    return {
      checkedIntentRefs: intents.map(intent => intent.intentRef),
      delivered,
      forbiddenAuthority: ARTANIS_FORUM_DELIVERY_NO_EXTRA_AUTHORITY,
    }
  })
