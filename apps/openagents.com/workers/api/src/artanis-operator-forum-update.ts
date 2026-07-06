import { Effect, Schema as S } from 'effect'

import type {
  ArtanisForumUpdateInput,
  ArtanisForumUpdateResult,
  ArtanisForumUpdateWriter,
} from './artanis-operator-tools'
import { ArtanisForumUpdateWriterError } from './artanis-operator-tools'
import {
  isArtanisForumPostActor,
  resolveRegisteredArtanisForumIdentityFromD1,
} from './artanis-forum-identity'
import type { IdentityDb } from './identity-db'
import {
  type ForumRepositoryRuntime,
  type ForumPublicProjection,
  ForumWriterGrant,
  buildForumWriterContext,
  createForumReplyPost,
  createForumTopicWithFirstPost,
  readForumPostById,
  readForumPostByIdempotencyKey,
  readForumSummaryByRef,
  readForumTopicByIdempotencyKey,
  readForumTopicByRef,
} from './forum'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

const decodeForumWriterGrant = S.decodeUnknownSync(ForumWriterGrant)

const publicProjection = (artifactRef: string) => ({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: [artifactRef],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
} satisfies ForumPublicProjection)

const safeRefSuffix = (value: string): string => {
  const suffix = value
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
    .slice(0, 120)
  return suffix === '' ? 'update' : suffix
}

const forumPostUrl = (topicId: string, postId: string): string =>
  `https://openagents.com/forum/t/${topicId}#post-${postId}`

const topicRefForId = (topicId: string): string => `forum.topic.${topicId}`
const postRefForId = (postId: string): string => `forum.post.${postId}`

export const makeArtanisForumUpdateWriter = (
  input: Readonly<{
    db: D1Database
    /** CFG-4 Domain 2 (#8519): Postgres identity handle for the registered
     * Artanis `users`/`auth_identities` lookup. */
    identityDb: IdentityDb
    makeId?: (() => string) | undefined
    nowEpochMillis?: (() => number) | undefined
    nowIso?: (() => string) | undefined
  }>,
): ArtanisForumUpdateWriter => {
  const makeId = input.makeId ?? randomUuid
  const nowEpochMillis = input.nowEpochMillis ?? currentEpochMillis
  const nowIso = input.nowIso ?? currentIsoTimestamp

  return (update: ArtanisForumUpdateInput) =>
    Effect.gen(function* () {
      const now = nowIso()
      const forum = yield* readForumSummaryByRef(input.db, update.forumRef, {
        allowUnlisted: true,
      })
      if (forum === null || forum.locked) {
        return yield* new ArtanisForumUpdateWriterError({
          reason: 'target_forum_unavailable',
        })
      }

      const identity = yield* resolveRegisteredArtanisForumIdentityFromD1(
        input.db,
        input.identityDb,
        now,
      ).pipe(
        Effect.mapError(
          error => new ArtanisForumUpdateWriterError({ reason: error.reason }),
        ),
      )
      const grant = decodeForumWriterGrant({
        expiresAtEpochMillis: nowEpochMillis() + 1000 * 60 * 60,
        forumIds: [forum.forumId],
        ownerUserId: identity.userId,
        scopes: ['forum.write'],
        status: 'active',
        teamId: null,
      })
      const writer = yield* buildForumWriterContext({
        actor: identity.actor,
        grant,
        nowEpochMillis,
        paymentProofRef: null,
        requiredScope: 'forum.write',
        targetForumId: forum.forumId,
        targetOwnerUserId: identity.userId,
        targetTeamId: null,
      })
      const runtime: ForumRepositoryRuntime = { makeId, nowIso }

      if (update.action === 'create_topic') {
        const title = update.title ?? 'Artanis update'
        const existingTopic = yield* readForumTopicByIdempotencyKey(
          input.db,
          update.idempotencyKey,
        )
        if (existingTopic !== null) {
          const existingPost = yield* readForumPostById(
            input.db,
            existingTopic.firstPostId,
          )
          if (
            existingPost === null ||
            existingTopic.forumId !== forum.forumId ||
            !isArtanisForumPostActor(existingTopic.author.actorRef, identity) ||
            existingTopic.title !== title ||
            (existingPost.bodyText ?? '').trim() !== update.bodyText.trim()
          ) {
            return yield* new ArtanisForumUpdateWriterError({
              reason: 'idempotency_conflict_topic',
            })
          }
          return {
            action: update.action,
            forumRef: update.forumRef,
            idempotent: true,
            postId: existingTopic.firstPostId,
            postRef: postRefForId(existingTopic.firstPostId),
            publicUrl: forumPostUrl(
              existingTopic.topicId,
              existingTopic.firstPostId,
            ),
            topicId: existingTopic.topicId,
            topicRef: topicRefForId(existingTopic.topicId),
          } satisfies ArtanisForumUpdateResult
        }

        const topicId = makeId()
        const firstPostId = makeId()
        const created = yield* createForumTopicWithFirstPost(
          input.db,
          {
            actor: writer.actor,
            bodyText: update.bodyText,
            contentRef: `content.forum.artanis.operator_update.${safeRefSuffix(
              update.idempotencyKey,
            )}`,
            firstPostId,
            forumId: forum.forumId,
            idempotencyKey: update.idempotencyKey,
            publicProjection: publicProjection(
              `artifact.forum.artanis.topic.${topicId}`,
            ),
            slug: `artanis-${safeRefSuffix(title)}`,
            title,
            topicId,
          },
          runtime,
        )
        return {
          action: update.action,
          forumRef: update.forumRef,
          idempotent: false,
          postId: created.firstPost.postId,
          postRef: postRefForId(created.firstPost.postId),
          publicUrl: forumPostUrl(
            created.topic.topicId,
            created.firstPost.postId,
          ),
          topicId: created.topic.topicId,
          topicRef: topicRefForId(created.topic.topicId),
        } satisfies ArtanisForumUpdateResult
      }

      const topic = yield* readForumTopicByRef(input.db, update.topicRef!)
      if (
        topic === null ||
        topic.forumId !== forum.forumId ||
        topic.state !== 'open'
      ) {
        return yield* new ArtanisForumUpdateWriterError({
          reason: 'target_topic_unavailable',
        })
      }

      const existingPost = yield* readForumPostByIdempotencyKey(
        input.db,
        update.idempotencyKey,
      )
      if (existingPost !== null) {
        if (
          existingPost.topicId !== topic.topicId ||
          !isArtanisForumPostActor(existingPost.author.actorRef, identity) ||
          (existingPost.bodyText ?? '').trim() !== update.bodyText.trim()
        ) {
          return yield* new ArtanisForumUpdateWriterError({
            reason: 'idempotency_conflict_reply',
          })
        }
        return {
          action: update.action,
          forumRef: update.forumRef,
          idempotent: true,
          postId: existingPost.postId,
          postRef: postRefForId(existingPost.postId),
          publicUrl: forumPostUrl(topic.topicId, existingPost.postId),
          topicId: topic.topicId,
          topicRef: topicRefForId(topic.topicId),
        } satisfies ArtanisForumUpdateResult
      }

      const postId = makeId()
      const post = yield* createForumReplyPost(
        input.db,
        {
          actor: writer.actor,
          bodyText: update.bodyText,
          contentRef: `content.forum.artanis.operator_update.${safeRefSuffix(
            update.idempotencyKey,
          )}`,
          forumId: forum.forumId,
          idempotencyKey: update.idempotencyKey,
          parentPostId: topic.latestPostId,
          postId,
          publicProjection: publicProjection(
            `artifact.forum.artanis.reply.${postId}`,
          ),
          quotePostId: null,
          topicId: topic.topicId,
        },
        runtime,
      )
      return {
        action: update.action,
        forumRef: update.forumRef,
        idempotent: false,
        postId: post.postId,
        postRef: postRefForId(post.postId),
        publicUrl: forumPostUrl(topic.topicId, post.postId),
        topicId: topic.topicId,
        topicRef: topicRefForId(topic.topicId),
      } satisfies ArtanisForumUpdateResult
    }).pipe(
      Effect.mapError(error =>
        error instanceof ArtanisForumUpdateWriterError
          ? error
          : new ArtanisForumUpdateWriterError({
              reason: `forum_repository_error:${error._tag}`,
            }),
      ),
    )
}
