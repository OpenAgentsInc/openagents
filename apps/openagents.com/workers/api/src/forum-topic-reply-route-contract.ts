import { Schema as S } from 'effect'

import {
  ForumContextKind,
  type ForumPostThreadRef,
} from './forum'
import { ForumPostBodyTextMaxLength } from './forum-limits'

export const ForumContextLinkBody = S.Struct({
  contextId: S.Trim.check(S.isNonEmpty(), S.isMaxLength(160)),
  contextKind: ForumContextKind,
  contextSlug: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(120)))),
  contextTitle: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(160)))),
  publicUrl: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(400)))),
  sourceRef: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(220)))),
})
export type ForumContextLinkBody = typeof ForumContextLinkBody.Type

const ForumRoutePostBodyText = S.Trim.check(
  S.isNonEmpty(),
  S.isMaxLength(ForumPostBodyTextMaxLength),
)

export const CreateForumTopicBody = S.Struct({
  bodyText: ForumRoutePostBodyText,
  context: S.optionalKey(S.NullOr(ForumContextLinkBody)),
  paymentProofRef: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(300)))),
  requestedSlug: S.optionalKey(
    S.NullOr(
      S.Trim.check(
        S.isMinLength(3),
        S.isMaxLength(80),
        S.isPattern(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/),
      ),
    ),
  ),
  title: S.Trim.check(S.isMinLength(3), S.isMaxLength(160)),
})
export type CreateForumTopicBody = typeof CreateForumTopicBody.Type

export const CreateForumReplyBody = S.Struct({
  bodyText: ForumRoutePostBodyText,
  context: S.optionalKey(S.NullOr(ForumContextLinkBody)),
  parentPostId: S.optionalKey(S.NullOr(S.String)),
  paymentProofRef: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(300)))),
  quotePostId: S.optionalKey(S.NullOr(S.String)),
})
export type CreateForumReplyBody = typeof CreateForumReplyBody.Type

export const decodeCreateForumTopicBody = (
  body: unknown,
): CreateForumTopicBody => S.decodeUnknownSync(CreateForumTopicBody)(body)

export const decodeCreateForumReplyBody = (
  body: unknown,
): CreateForumReplyBody => S.decodeUnknownSync(CreateForumReplyBody)(body)

// A parent ref is valid only when it points at an existing, visible post in
// the same topic. Dangling refs (for example a truncated post ID) must reject
// at the write boundary instead of persisting verbatim (#4856).
export const invalidForumReplyParentPostReference = (
  parent: ForumPostThreadRef | null,
  topicId: string,
): string | null =>
  parent === null
    ? 'parentPostId must reference an existing post'
    : parent.topicId !== topicId
      ? 'parentPostId must belong to the target topic'
      : parent.state === 'tombstoned' ||
          parent.state === 'hidden' ||
          parent.state === 'held_for_review'
        ? 'parentPostId must reference a visible post'
        : null
