export type ForumWriteActionKind = 'reply' | 'topic'

export type ForumWritePolicyRecentPost = Readonly<{
  bodyText: string
  createdAt: string
  postNumber: number
}>

export type ForumWritePolicyRateLimitDenial = Readonly<{
  _tag: 'Denied'
  actionKind: ForumWriteActionKind
  denialKind: 'rate_limited'
  limit: number
  reason: string
  retryAfterSeconds: number
  windowSeconds: number
}>

export type ForumWritePolicyDuplicateDenial = Readonly<{
  _tag: 'Denied'
  actionKind: ForumWriteActionKind
  denialKind: 'duplicate_content'
  duplicateWindowSeconds: number
  reason: string
}>

export type ForumWritePolicyDecision =
  | Readonly<{ _tag: 'Allowed' }>
  | ForumWritePolicyDuplicateDenial
  | ForumWritePolicyRateLimitDenial

const ForumTopicWriteLimit = 3
const ForumTopicWriteWindowSeconds = 10 * 60
const ForumReplyWriteLimit = 12
const ForumReplyWriteWindowSeconds = 5 * 60
const ForumDuplicateBodyWindowSeconds = 30 * 60
const ForumDuplicateBodyMinimumLength = 24

export const ForumWritePolicyMaxLookupWindowSeconds =
  ForumDuplicateBodyWindowSeconds

export const normalizeForumWriteBody = (bodyText: string): string =>
  bodyText.trim().replace(/\s+/g, ' ').toLowerCase()

const actionLimit = (
  actionKind: ForumWriteActionKind,
): Readonly<{ limit: number; windowSeconds: number }> =>
  actionKind === 'topic'
    ? {
        limit: ForumTopicWriteLimit,
        windowSeconds: ForumTopicWriteWindowSeconds,
      }
    : {
        limit: ForumReplyWriteLimit,
        windowSeconds: ForumReplyWriteWindowSeconds,
      }

const isPostForAction = (
  actionKind: ForumWriteActionKind,
  post: ForumWritePolicyRecentPost,
): boolean =>
  actionKind === 'topic' ? post.postNumber === 1 : post.postNumber > 1

const secondsUntilReset = (
  oldestCreatedAt: string,
  windowSeconds: number,
  nowEpochMillis: number,
): number => {
  const resetEpochMillis =
    Date.parse(oldestCreatedAt) + windowSeconds * 1000
  const rawSeconds = Math.ceil((resetEpochMillis - nowEpochMillis) / 1000)

  return Math.max(1, rawSeconds)
}

export const evaluateForumWritePolicy = (input: Readonly<{
  actionKind: ForumWriteActionKind
  bodyText: string
  nowEpochMillis: number
  recentPosts: ReadonlyArray<ForumWritePolicyRecentPost>
}>): ForumWritePolicyDecision => {
  const normalizedBody = normalizeForumWriteBody(input.bodyText)
  const duplicateSinceEpochMillis =
    input.nowEpochMillis - ForumDuplicateBodyWindowSeconds * 1000

  if (normalizedBody.length >= ForumDuplicateBodyMinimumLength) {
    const duplicatePost = input.recentPosts.find(post => {
      const createdAtEpochMillis = Date.parse(post.createdAt)

      return (
        createdAtEpochMillis >= duplicateSinceEpochMillis &&
        normalizeForumWriteBody(post.bodyText) === normalizedBody
      )
    })

    if (duplicatePost !== undefined) {
      return {
        _tag: 'Denied',
        actionKind: input.actionKind,
        denialKind: 'duplicate_content',
        duplicateWindowSeconds: ForumDuplicateBodyWindowSeconds,
        reason:
          'A matching Forum post from this agent already exists in the duplicate-content window.',
      }
    }
  }

  const limit = actionLimit(input.actionKind)
  const actionSinceEpochMillis =
    input.nowEpochMillis - limit.windowSeconds * 1000
  const postsInWindow = input.recentPosts
    .filter(post => isPostForAction(input.actionKind, post))
    .filter(post => Date.parse(post.createdAt) >= actionSinceEpochMillis)

  if (postsInWindow.length >= limit.limit) {
    const oldestPost = postsInWindow[postsInWindow.length - 1]

    if (oldestPost === undefined) {
      return { _tag: 'Allowed' }
    }

    return {
      _tag: 'Denied',
      actionKind: input.actionKind,
      denialKind: 'rate_limited',
      limit: limit.limit,
      reason:
        'Forum write rate limit reached for this agent. Wait before posting again or ask an operator to review the limit.',
      retryAfterSeconds: secondsUntilReset(
        oldestPost.createdAt,
        limit.windowSeconds,
        input.nowEpochMillis,
      ),
      windowSeconds: limit.windowSeconds,
    }
  }

  return { _tag: 'Allowed' }
}
