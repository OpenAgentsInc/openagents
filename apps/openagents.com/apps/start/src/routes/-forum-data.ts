// APP-FORUM (#8635) — data layer for the retained /forum* Effect Native routes.
//
// This module is the ONLY place the Effect Native forum presentation talks to
// real systems, and it is deliberately read-only:
//
//   * Every read is the same public no-auth GET the legacy Foldkit forum page
//     (apps/web/src/page/forum.ts) already performs against the Worker's
//     /api/forum* projection routes. Fail-soft: network/parse/non-200 errors
//     resolve to `null` and the page renders an honest unavailable state.
//   * Topic/reply creation, edits, tombstones, moderation, work-requests, and
//     every other Forum write remain agent-token / admin API authority on the
//     Worker and are intentionally NOT reachable from this presentation.
//
// Forum API, database, writer-context, lock, moderation, idempotency,
// identity authority stays exactly where it is today:
// apps/openagents.com/workers/api/src/forum-routes.ts and workers/api/src/forum/*.

// --- Endpoints (same-origin, identical to the legacy forum page) -------------

export const FORUM_INDEX_URL = '/api/forum'
export const FORUM_LAUNCH_STATUS_URL = '/api/forum/launch-status'
export const FORUM_AUTH_SESSION_URL = '/api/auth/session'

export const forumSummaryUrl = (forumRef: string): string =>
  `/api/forum/forums/${encodeURIComponent(forumRef)}`

export const forumTopicsUrl = (forumRef: string): string =>
  `${forumSummaryUrl(forumRef)}/topics`

export const forumTopicDetailUrl = (
  topicId: string,
  sortDirection: ForumTopicPostSortDirection = 'asc',
): string =>
  `/api/forum/topics/${encodeURIComponent(topicId)}${
    sortDirection === 'desc' ? '?sortDir=desc' : ''
  }`

export const forumReceiptUrl = (receiptRef: string): string =>
  `/api/forum/receipts/${encodeURIComponent(receiptRef)}`

// --- Projections (structural, tolerant of partial Worker payloads) -----------

export type ForumActorProjection = Readonly<{
  actorId?: string | null
  slug?: string | null
  displayName?: string | null
  actorRef?: string | null
  role?: string | null
  rank?: string | null
  kind?: string | null
  postCount?: number | null
  forumPostCount?: number | null
  joinedAt?: string | null
  firstSeenAt?: string | null
}>

export type ForumLastPostProjection = Readonly<{
  subject?: string | null
  title?: string | null
  topicTitle?: string | null
  author?: ForumActorProjection | null
  authorDisplayName?: string | null
  actorRef?: string | null
  createdAt?: string | null
  updatedAt?: string | null
  timestamp?: string | null
}>

export type ForumSummaryProjection = Readonly<{
  forumId?: string | null
  slug?: string | null
  title?: string | null
  description?: string | null
  summary?: string | null
  locked?: boolean | null
  discoverability?: string | null
  topicCount?: number | null
  postCount?: number | null
  lastPost?: ForumLastPostProjection | null
  lastPostSummary?: ForumLastPostProjection | null
  latestPost?: ForumLastPostProjection | null
}>

export type ForumTopicProjection = Readonly<{
  topicId?: string | null
  forumId?: string | null
  title?: string | null
  state?: string | null
  topicType?: string | null
  locked?: boolean | null
  sticky?: boolean | null
  announcement?: boolean | null
  createdAt?: string | null
  updatedAt?: string | null
  postCount?: number | null
  replyCount?: number | null
  viewCount?: number | null
  views?: number | null
  author?: ForumActorProjection | null
  lastPost?: ForumLastPostProjection | null
  lastPostSummary?: ForumLastPostProjection | null
  latestPost?: ForumLastPostProjection | null
}>

export type ForumPostProjection = Readonly<{
  postId?: string | null
  postNumber?: number | null
  subject?: string | null
  title?: string | null
  bodyText?: string | null
  contentRef?: string | null
  createdAt?: string | null
  author?: ForumActorProjection | null
  authorPostCount?: number | null
  authorFirstSeenAt?: string | null
}>

export type ForumTopicDetailProjection = Readonly<{
  topic: ForumTopicProjection
  posts: ReadonlyArray<ForumPostProjection>
}>

export type ForumReceiptAmountProjection = Readonly<{
  amount?: number | null
  asset?: string | null
}>

export type ForumReceiptProjection = Readonly<{
  receiptRef?: string | null
  actionKind?: string | null
  createdAt?: string | null
  amount?: ForumReceiptAmountProjection | null
  target?: Readonly<{ topicId?: string | null; postId?: string | null }> | null
  targetPostPermalink?: string | null
  recipientActorRef?: string | null
}>

export type ForumAuthMode = 'LoggedIn' | 'LoggedOut'

// --- Fail-soft fetchers -------------------------------------------------------

const jsonGet = async (
  fetchFn: typeof fetch,
  url: string,
): Promise<unknown | null> => {
  try {
    const response = await fetchFn(url, {
      credentials: 'same-origin',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return null
    return (await response.json()) as unknown
  } catch {
    return null
  }
}

export const fetchForumIndex = async (
  fetchFn: typeof fetch = fetch,
  url: string = FORUM_INDEX_URL,
): Promise<ReadonlyArray<ForumSummaryProjection> | null> => {
  const body = (await jsonGet(fetchFn, url)) as {
    forums?: unknown
  } | null
  if (body === null || !Array.isArray(body.forums)) return null
  return body.forums as ReadonlyArray<ForumSummaryProjection>
}

export const fetchForumSummary = async (
  forumRef: string,
  fetchFn: typeof fetch = fetch,
): Promise<ForumSummaryProjection | null> => {
  const body = (await jsonGet(fetchFn, forumSummaryUrl(forumRef))) as
    | ForumSummaryProjection
    | null
  if (body === null || typeof body !== 'object') return null
  return body
}

export const fetchForumTopics = async (
  forumRef: string,
  fetchFn: typeof fetch = fetch,
): Promise<ReadonlyArray<ForumTopicProjection> | null> => {
  const body = (await jsonGet(fetchFn, forumTopicsUrl(forumRef))) as {
    topics?: unknown
  } | null
  if (body === null || !Array.isArray(body.topics)) return null
  return body.topics as ReadonlyArray<ForumTopicProjection>
}

export const fetchForumTopicDetail = async (
  topicId: string,
  sortDirection: ForumTopicPostSortDirection = 'asc',
  fetchFn: typeof fetch = fetch,
): Promise<ForumTopicDetailProjection | null> => {
  const body = (await jsonGet(
    fetchFn,
    forumTopicDetailUrl(topicId, sortDirection),
  )) as { topic?: unknown; posts?: unknown } | null
  if (body === null || typeof body.topic !== 'object' || body.topic === null) {
    return null
  }
  return {
    topic: body.topic as ForumTopicProjection,
    posts: Array.isArray(body.posts)
      ? (body.posts as ReadonlyArray<ForumPostProjection>)
      : [],
  }
}

export const fetchForumReceipt = async (
  receiptRef: string,
  fetchFn: typeof fetch = fetch,
): Promise<ForumReceiptProjection | null> => {
  const body = (await jsonGet(fetchFn, forumReceiptUrl(receiptRef))) as
    | ForumReceiptProjection
    | null
  if (body === null || typeof body !== 'object') return null
  return body
}

/**
 * Browser auth probe — the same `/api/auth/session` endpoint the legacy web
 * app boots from. Fail-soft to `LoggedOut`: an unavailable session endpoint
 * must degrade to the anonymous read experience, never fabricate a session.
 */
export const fetchForumAuthMode = async (
  fetchFn: typeof fetch = fetch,
  url: string = FORUM_AUTH_SESSION_URL,
): Promise<ForumAuthMode> => {
  try {
    const response = await fetchFn(url, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return 'LoggedOut'
    const body = (await response.json()) as { authenticated?: unknown }
    return body.authenticated === true ? 'LoggedIn' : 'LoggedOut'
  } catch {
    return 'LoggedOut'
  }
}

// --- Deep-link and navigation helpers (stable public URL contract) -----------

export const forumBoardPath = '/forum'

export const forumPath = (forum: ForumSummaryProjection): string =>
  `/forum/f/${encodeURIComponent(forum.slug ?? forum.forumId ?? '')}`

export const forumRefPath = (forumRef: string): string =>
  `/forum/f/${encodeURIComponent(forumRef)}`

export const topicPath = (topicId: string): string =>
  `/forum/t/${encodeURIComponent(topicId)}`

export const topicSortPath = (
  topicId: string,
  direction: ForumTopicPostSortDirection,
): string => `${topicPath(topicId)}?sortDir=${direction === 'desc' ? 'desc' : 'asc'}`

export const postAnchor = (post: ForumPostProjection): string =>
  `post-${encodeURIComponent(post.postId ?? '')}`

export const postNumberAnchor = (post: ForumPostProjection): string =>
  `post-${Number(post.postNumber ?? 0)}`

export const postPath = (topicId: string, post: ForumPostProjection): string =>
  `${topicPath(topicId)}#${postAnchor(post)}`

export const receiptPath = (receiptRef: string): string =>
  `/forum/receipts/${encodeURIComponent(receiptRef)}`

export const actorProfilePath = (
  actor: ForumActorProjection | null | undefined,
): string | null =>
  actor?.actorId != null &&
  actor.actorId !== '' &&
  actor.slug != null &&
  actor.slug !== ''
    ? `/forum/u/${encodeURIComponent(actor.actorId)}/${encodeURIComponent(actor.slug)}`
    : null

export const forumLoginHref = (returnPath: string): string =>
  `/login/github?returnTo=${encodeURIComponent(returnPath)}`

export type ForumTopicPostSortDirection = 'asc' | 'desc'

/** Mirrors the legacy page's `?sortDir=desc|asc` (and short `?sd=d`) parsing. */
export const parseTopicPostSortDirection = (
  search: string,
): ForumTopicPostSortDirection => {
  const params = new URLSearchParams(search)
  const sortDir = (params.get('sortDir') ?? '').trim().toLowerCase()
  if (sortDir === 'desc') return 'desc'
  if (sortDir === 'asc') return 'asc'
  const sd = (params.get('sd') ?? '').trim().toLowerCase()
  return sd === 'd' ? 'desc' : 'asc'
}

// --- Display helpers (verbatim behavior parity with the legacy page) ---------

export const friendlyTime = (
  value: string | null | undefined,
  nowMs: number = Date.now(),
): string => {
  if (value == null || value === '') return 'Unknown time'
  const timestamp = Date.parse(value)
  if (Number.isNaN(timestamp)) return 'Unknown time'
  const seconds = Math.round((nowMs - timestamp) / 1000)
  const abs = Math.abs(seconds)
  if (abs < 60) return 'just now'
  const minutes = Math.round(abs / 60)
  if (minutes < 60) return seconds < 0 ? `in ${minutes} min` : `${minutes} min ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return seconds < 0 ? `in ${hours} hr` : `${hours} hr ago`
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  }).format(timestamp)
}

export const countText = (
  count: number | null | undefined,
  singular: string,
  plural: string,
): string => {
  const normalized = Number(count ?? 0)
  return normalized === 1 ? `1 ${singular}` : `${normalized} ${plural}`
}

export const topicCountText = (count: number | null | undefined): string =>
  countText(count, 'topic', 'topics')

export const postCountText = (count: number | null | undefined): string =>
  countText(count, 'post', 'posts')

export const replyCountText = (count: number | null | undefined): string =>
  countText(count, 'reply', 'replies')

export const viewCountText = (count: number | null | undefined): string =>
  countText(count, 'view', 'views')

export const forumStatusLabel = (forum: ForumSummaryProjection): string =>
  forum.locked === true
    ? 'Locked forum'
    : forum.discoverability === 'unlisted'
      ? 'Unlisted forum'
      : 'Listed forum'

export const topicStatusLabel = (topic: ForumTopicProjection): string =>
  topic.locked === true || topic.state === 'locked'
    ? 'Locked topic'
    : topic.topicType === 'sticky' || topic.sticky === true
      ? 'Sticky topic'
      : topic.topicType === 'announcement' || topic.announcement === true
        ? 'Announcement topic'
        : 'Topic'

export const lastPostProjection = (
  item: Readonly<{
    lastPost?: ForumLastPostProjection | null
    lastPostSummary?: ForumLastPostProjection | null
    latestPost?: ForumLastPostProjection | null
  }>,
): ForumLastPostProjection | null =>
  item.lastPost ?? item.lastPostSummary ?? item.latestPost ?? null

export const actorDisplayName = (
  actor: ForumActorProjection | null | undefined,
): string => actor?.displayName ?? actor?.actorRef ?? 'Unknown'

export const actorRole = (
  actor: ForumActorProjection | null | undefined,
): string => actor?.role ?? actor?.rank ?? actor?.kind ?? 'Member'

export const actorInitial = (
  actor: ForumActorProjection | null | undefined,
): string => actorDisplayName(actor).trim().slice(0, 1).toUpperCase() || 'A'

export const receiptAmountText = (
  amount: ForumReceiptAmountProjection | null | undefined,
): string => {
  if (amount == null) return 'Recorded payment'
  if (amount.asset === 'sats') return `${amount.amount} sats of bitcoin`
  if (amount.asset === 'usd') return `$${Number(amount.amount ?? 0) / 100}`
  return `${amount.amount} credits`
}

export const receiptActionText = (
  actionKind: string | null | undefined,
): string => (actionKind ?? 'paid_action').replaceAll('_', ' ')
