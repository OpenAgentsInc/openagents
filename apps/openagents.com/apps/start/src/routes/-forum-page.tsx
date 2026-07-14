// APP-FORUM (#8635) — the retained /forum* presentation re-authored as typed
// Effect Native view trees inside the one OpenAgents web app.
//
// Route surface (deep-link stable, identical URL contract to the legacy
// Foldkit page in apps/web/src/page/forum.ts):
//   /forum                      board index
//   /forum/f/$forumRef          forum topic list
//   /forum/t/$topicId           topic posts (?sortDir=asc|desc, #post-<id>)
//   /forum/receipts/$receiptRef payment receipt
//
// Authority boundary: this file is presentation only. All Forum reads hit the
// existing public Worker projections. Content writes (topics, replies, edits,
// tombstones), moderation, locks, identity, and work-request authority remain
// on the Worker's /api/forum* contracts untouched. React is
// only the thin route-shell host that mounts the Effect Native tree through
// the DOM renderer (the EN adapter rule).

import {
  Badge,
  Button,
  Card,
  CodeBlock,
  Divider,
  IntentRef,
  Link,
  Markdown,
  Navigate,
  Stack,
  StaticPayload,
  StatusBanner,
  Text,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentHandlers,
  type IntentReporter,
  type NavigationDestination,
  type TextView,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { Effect, Exit, Schema, Scope, SubscriptionRef } from '@effect-native/core/effect'
import { khalaTheme } from '@effect-native/tokens'
import { useEffect, useRef, type RefObject } from 'react'

import {
  actorDisplayName,
  actorInitial,
  actorProfilePath,
  actorRole,
  fetchForumAuthMode,
  fetchForumIndex,
  fetchForumReceipt,
  fetchForumSummary,
  fetchForumTopicDetail,
  fetchForumTopics,
  forumBoardPath,
  forumPath,
  forumRefPath,
  forumStatusLabel,
  friendlyTime,
  lastPostProjection,
  parseTopicPostSortDirection,
  postAnchor,
  postCountText,
  postNumberAnchor,
  postPath,
  receiptActionText,
  receiptAmountText,
  receiptPath,
  replyCountText,
  topicCountText,
  topicPath,
  topicSortPath,
  topicStatusLabel,
  viewCountText,
  type ForumAuthMode,
  type ForumPostProjection,
  type ForumReceiptProjection,
  type ForumSummaryProjection,
  type ForumTopicPostSortDirection,
  type ForumTopicProjection,
} from './-forum-data'
import { parseForumMarkdown } from './-forum-markdown'

// ---------------------------------------------------------------------------
// Route params + state
// ---------------------------------------------------------------------------

export type ForumRouteParams =
  | Readonly<{ kind: 'index' }>
  | Readonly<{ kind: 'forum'; forumRef: string }>
  | Readonly<{
      kind: 'topic'
      topicId: string
      sortDirection: ForumTopicPostSortDirection
    }>
  | Readonly<{ kind: 'receipt'; receiptRef: string }>

export type ForumPageState = Readonly<{
  params: ForumRouteParams
  phase: 'loading' | 'ready' | 'unavailable'
  errorMessage: string
  authMode: ForumAuthMode
  forums: ReadonlyArray<ForumSummaryProjection>
  forum: ForumSummaryProjection | null
  topics: ReadonlyArray<ForumTopicProjection>
  topic: ForumTopicProjection | null
  posts: ReadonlyArray<ForumPostProjection>
  receipt: ForumReceiptProjection | null
  copiedPermalinkPostId: string | null
}>

export const initialForumPageState = (
  params: ForumRouteParams,
): ForumPageState => ({
  params,
  phase: 'loading',
  errorMessage: '',
  authMode: 'LoggedOut',
  forums: [],
  forum: null,
  topics: [],
  topic: null,
  posts: [],
  receipt: null,
  copiedPermalinkPostId: null,
})

/** The route's return path — also the GitHub login `returnTo` target. */
export const forumReturnPath = (params: ForumRouteParams): string =>
  params.kind === 'forum'
    ? forumRefPath(params.forumRef)
    : params.kind === 'topic'
      ? topicPath(params.topicId)
      : params.kind === 'receipt'
        ? receiptPath(params.receiptRef)
        : forumBoardPath

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

const PermalinkCopied = defineIntent(
  'ForumPermalinkCopied',
  Schema.Struct({ postId: Schema.String, href: Schema.String }),
)

const forumIntents = [PermalinkCopied, Navigate] as const

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

const text = (
  key: string,
  content: string,
  variant: TextView['variant'] = 'body',
  color: TextView['color'] = 'textPrimary',
): TextView =>
  Text({
    key,
    content,
    variant,
    color,
    style: { width: 'full' },
  })

const pathLink = (
  key: string,
  label: string,
  path: string,
  variant: TextView['variant'] = 'body',
): View =>
  Link(
    {
      key,
      destination: { kind: 'path', path },
      style: { color: 'accent' },
    },
    [Text({ key: `${key}-label`, content: label, variant, color: 'accent' })],
  )

const panelCard = (key: string, children: ReadonlyArray<View>): View =>
  Card(
    {
      key,
      padding: '4',
      radius: 'md',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    children,
  )

const breadcrumb = (
  key: string,
  trail: ReadonlyArray<Readonly<{ key: string; label: string; path?: string }>>,
): View =>
  Card(
    {
      key,
      padding: '3',
      radius: 'md',
      style: {
        backgroundColor: 'surfaceRaised',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    [
      Stack(
        { key: `${key}-row`, direction: 'row', gap: '2', align: 'center' },
        trail.flatMap((item, index) => {
          const node =
            item.path === undefined
              ? Text({
                  key: item.key,
                  content: item.label,
                  variant: 'label',
                  color: 'textMuted',
                })
              : pathLink(item.key, item.label, item.path, 'label')
          return index === 0
            ? [node]
            : [
                Text({
                  key: `${item.key}-sep`,
                  content: '»',
                  variant: 'label',
                  color: 'textMuted',
                }),
                node,
              ]
        }),
      ),
    ],
  )

const boardBreadcrumbItem = { key: 'crumb-board', label: 'Board index', path: forumBoardPath }

const metaBadge = (key: string, label: string): View =>
  Badge({ key, label, tone: 'neutral' })

const lastPostSummaryText = (
  key: string,
  item: Readonly<{
    lastPost?: ForumPostProjection | null
    lastPostSummary?: ForumPostProjection | null
    latestPost?: ForumPostProjection | null
  }>,
  nowMs: number,
): View => {
  const lastPost = lastPostProjection(item)
  if (lastPost === null) {
    return text(key, 'No posts', 'caption', 'textMuted')
  }
  const subject =
    lastPost.subject ?? lastPost.title ?? lastPost.topicTitle ?? 'Last post'
  const author =
    lastPost.author?.displayName ??
    lastPost.author?.actorRef ??
    lastPost.authorDisplayName ??
    lastPost.actorRef ??
    'Unknown'
  const time = friendlyTime(
    lastPost.createdAt ?? lastPost.updatedAt ?? lastPost.timestamp,
    nowMs,
  )
  return text(key, `${subject} — by ${author} » ${time}`, 'caption', 'textMuted')
}

const loadingView = (key: string): View =>
  panelCard(key, [text(`${key}-copy`, 'Loading…', 'body', 'textMuted')])

const unavailableView = (key: string, message: string): View =>
  StatusBanner({
    key,
    tone: 'danger',
    message: message === '' ? 'Forum unavailable' : `Forum unavailable · ${message}`,
    style: { width: 'full' },
  })

// ---------------------------------------------------------------------------
// Index view
// ---------------------------------------------------------------------------

const forumRow = (
  forum: ForumSummaryProjection,
  index: number,
  nowMs: number,
): View => {
  const key = `forum-row-${forum.slug ?? forum.forumId ?? index}`
  return Card(
    {
      key,
      padding: '4',
      radius: 'md',
      style: {
        backgroundColor: index % 2 === 0 ? 'surface' : 'surfaceRaised',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    [
      Stack({ key: `${key}-head`, direction: 'row', gap: '3', align: 'center' }, [
        pathLink(`${key}-title`, forum.title ?? 'Forum', forumPath(forum), 'title'),
        Text({
          key: `${key}-slug`,
          content: forum.slug ?? forum.forumId ?? '',
          variant: 'caption',
          color: 'textMuted',
        }),
      ]),
      text(
        `${key}-desc`,
        forum.description ?? forum.summary ?? forumStatusLabel(forum),
        'caption',
        'textMuted',
      ),
      Stack({ key: `${key}-meta`, direction: 'row', gap: '2', align: 'center' }, [
        metaBadge(`${key}-topics`, topicCountText(forum.topicCount)),
        metaBadge(`${key}-posts`, postCountText(forum.postCount)),
        ...(forum.discoverability === 'unlisted'
          ? [Badge({ key: `${key}-unlisted`, label: 'Unlisted', tone: 'warn' })]
          : []),
        ...(forum.locked === true
          ? [Badge({ key: `${key}-locked`, label: 'Locked', tone: 'warn' })]
          : []),
      ]),
      lastPostSummaryText(`${key}-last`, forum, nowMs),
    ],
  )
}

const indexView = (state: ForumPageState, nowMs: number): ReadonlyArray<View> => {
  const rows =
    state.forums.length === 0
      ? [text('forum-index-empty', 'No listed forums yet.', 'body', 'textMuted')]
      : state.forums.map((forum, index) => forumRow(forum, index, nowMs))
  return [
    breadcrumb('forum-index-crumbs', [boardBreadcrumbItem]),
    panelCard('forum-index-panel', [
      text('forum-index-title', 'OpenAgents Forum', 'heading'),
      ...rows,
    ]),
  ]
}

// ---------------------------------------------------------------------------
// Forum (topic list) view
// ---------------------------------------------------------------------------

const topicRow = (
  topic: ForumTopicProjection,
  index: number,
  nowMs: number,
): View => {
  const key = `topic-row-${topic.topicId ?? index}`
  const postCount = Number(topic.postCount ?? 0)
  const replies = Number(topic.replyCount ?? Math.max(postCount - 1, 0))
  const views = Number(topic.viewCount ?? topic.views ?? 0)
  return Card(
    {
      key,
      padding: '4',
      radius: 'md',
      style: {
        backgroundColor: index % 2 === 0 ? 'surface' : 'surfaceRaised',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    [
      pathLink(`${key}-title`, topic.title ?? 'Topic', topicPath(topic.topicId ?? ''), 'title'),
      text(
        `${key}-by`,
        `by ${topic.author?.displayName ?? 'Unknown'} » ${friendlyTime(topic.createdAt ?? topic.updatedAt, nowMs)}`,
        'caption',
        'textMuted',
      ),
      Stack({ key: `${key}-meta`, direction: 'row', gap: '2', align: 'center' }, [
        metaBadge(`${key}-replies`, replyCountText(replies)),
        metaBadge(`${key}-views`, viewCountText(views)),
        ...(topicStatusLabel(topic) === 'Topic'
          ? []
          : [Badge({ key: `${key}-status`, label: topicStatusLabel(topic), tone: 'warn' })]),
      ]),
      lastPostSummaryText(`${key}-last`, topic, nowMs),
    ],
  )
}

const forumView = (state: ForumPageState, nowMs: number): ReadonlyArray<View> => {
  const forum = state.forum
  if (forum === null) {
    return [unavailableView('forum-missing', state.errorMessage)]
  }
  const rows =
    state.topics.length === 0
      ? [text('forum-topics-empty', 'No topics yet.', 'body', 'textMuted')]
      : state.topics.map((topic, index) => topicRow(topic, index, nowMs))
  return [
    breadcrumb('forum-crumbs', [
      boardBreadcrumbItem,
      { key: 'crumb-forum', label: forum.title ?? 'Forum' },
    ]),
    panelCard('forum-panel', [
      text('forum-eyebrow', 'Forum', 'label', 'accent'),
      text('forum-title', forum.title ?? 'Forum', 'heading'),
      text(
        'forum-counts',
        `${topicCountText(forum.topicCount)} · ${postCountText(forum.postCount)}${forum.locked === true ? ' · Locked' : ''}`,
        'caption',
        'textMuted',
      ),
      ...rows,
    ]),
  ]
}

// ---------------------------------------------------------------------------
// Topic (posts) view
// ---------------------------------------------------------------------------

const markdownBody = (key: string, body: string): ReadonlyArray<View> =>
  parseForumMarkdown(body).map((segment, index) =>
    segment.kind === 'markdown'
      ? Markdown({
          key: `${key}-md-${index}`,
          blocks: segment.blocks,
          style: { width: 'full' },
        })
      : segment.kind === 'code'
        ? CodeBlock({
            key: `${key}-code-${index}`,
            ...(segment.language === undefined ? {} : { language: segment.language }),
            lines: segment.code
              .split('\n')
              .map((line) => ({ tokens: [{ kind: 'plain' as const, text: line }] })),
          })
        : Divider({ key: `${key}-rule-${index}` }),
  )

const authorAside = (
  post: ForumPostProjection,
  key: string,
  nowMs: number,
): View => {
  const actor = post.author ?? null
  const displayName = actorDisplayName(actor)
  const profilePath = actorProfilePath(actor)
  const postCount = actor?.postCount ?? actor?.forumPostCount ?? post.authorPostCount
  const joinedAt = actor?.joinedAt ?? actor?.firstSeenAt ?? post.authorFirstSeenAt
  return Stack(
    { key, direction: 'column', gap: '2', style: { minWidth: 'sm' } },
    [
      Stack({ key: `${key}-id`, direction: 'row', gap: '2', align: 'center' }, [
        Badge({ key: `${key}-avatar`, label: actorInitial(actor), tone: 'info' }),
        Stack({ key: `${key}-name-col`, direction: 'column', gap: '0' }, [
          profilePath === null
            ? text(`${key}-name`, displayName, 'label')
            : pathLink(`${key}-name`, displayName, profilePath, 'label'),
          text(`${key}-role`, actorRole(actor), 'caption', 'textMuted'),
        ]),
      ]),
      ...(postCount == null
        ? []
        : [text(`${key}-posts`, `Posts: ${postCount}`, 'caption', 'textMuted')]),
      ...(joinedAt == null
        ? []
        : [text(`${key}-joined`, `Joined: ${friendlyTime(joinedAt, nowMs)}`, 'caption', 'textMuted')]),
    ],
  )
}

const postArticle = (
  state: ForumPageState,
  post: ForumPostProjection,
  index: number,
  nowMs: number,
): View => {
  const topicId = state.topic?.topicId ?? ''
  const anchor = postAnchor(post)
  const postNumber = Number(post.postNumber ?? 0)
  const subject =
    post.subject ?? post.title ?? state.topic?.title ?? `Post #${postNumber}`
  const href = postPath(topicId, post)
  const copied = state.copiedPermalinkPostId === (post.postId ?? '')
  return Card(
    {
      key: anchor,
      padding: '4',
      radius: 'md',
      style: {
        backgroundColor: index % 2 === 0 ? 'surface' : 'surfaceRaised',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    [
      Stack(
        { key: `${anchor}-grid`, direction: 'row', gap: '4', style: { width: 'full' } },
        [
          authorAside(post, `${anchor}-author`, nowMs),
          Stack(
            { key: `${anchor}-body`, direction: 'column', gap: '3', style: { flex: 1 } },
            [
              // Post-number marker keeps `#post-<n>` deep links resolvable.
              Stack(
                { key: postNumberAnchor(post), direction: 'column', gap: '1' },
                [
                  pathLink(`${anchor}-subject`, subject, href, 'title'),
                  text(
                    `${anchor}-meta`,
                    `Post #${postNumber} » ${friendlyTime(post.createdAt, nowMs)}`,
                    'caption',
                    'textMuted',
                  ),
                ],
              ),
              ...markdownBody(anchor, post.bodyText ?? post.contentRef ?? ''),
              Stack(
                {
                  key: `${anchor}-controls`,
                  direction: 'row',
                  gap: '2',
                  align: 'center',
                  style: { width: 'full' },
                },
                [
                  Button({
                    key: `${anchor}-permalink`,
                    label: copied ? 'Copied' : 'Permalink',
                    variant: 'ghost',
                    onPress: IntentRef(
                      'ForumPermalinkCopied',
                      StaticPayload({ postId: post.postId ?? '', href }),
                    ),
                    style: {
                      backgroundColor: 'surface',
                      borderColor: 'border',
                      borderRadius: 'md',
                      borderWidth: 1,
                      color: 'accent',
                      paddingTop: '1',
                      paddingRight: '2',
                      paddingBottom: '1',
                      paddingLeft: '2',
                      typeScale: 'caption',
                    },
                  }),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const sortToggle = (state: ForumPageState): View => {
  const params = state.params
  const topicId = params.kind === 'topic' ? params.topicId : ''
  const active = params.kind === 'topic' ? params.sortDirection : 'asc'
  const item = (direction: ForumTopicPostSortDirection, label: string): View =>
    active === direction
      ? Badge({ key: `sort-${direction}`, label, tone: 'info' })
      : pathLink(`sort-${direction}`, label, topicSortPath(topicId, direction), 'caption')
  return Stack({ key: 'sort-toggle', direction: 'row', gap: '2', align: 'center' }, [
    text('sort-label', 'Post order', 'caption', 'textMuted'),
    item('asc', 'Oldest first'),
    item('desc', 'Newest first'),
  ])
}

const topicView = (state: ForumPageState, nowMs: number): ReadonlyArray<View> => {
  const topic = state.topic
  if (topic === null) {
    return [unavailableView('topic-missing', state.errorMessage)]
  }
  const forumCrumbPath = forumRefPath(topic.forumId ?? '')
  const posts =
    state.posts.length === 0
      ? [text('topic-posts-empty', 'No visible posts yet.', 'body', 'textMuted')]
      : state.posts.map((post, index) => postArticle(state, post, index, nowMs))
  return [
    breadcrumb('topic-crumbs', [
      boardBreadcrumbItem,
      { key: 'crumb-forum', label: 'Forum', path: forumCrumbPath },
      { key: 'crumb-topic', label: topic.title ?? 'Topic' },
    ]),
    panelCard('topic-panel', [
      text('topic-eyebrow', 'Thread', 'label', 'accent'),
      text('topic-title', topic.title ?? 'Topic', 'heading'),
      Stack(
        { key: 'topic-meta-row', direction: 'row', gap: '3', align: 'center' },
        [
          text('topic-counts', postCountText(topic.postCount), 'caption', 'textMuted'),
          ...(topicStatusLabel(topic) === 'Topic'
            ? []
            : [Badge({ key: 'topic-status', label: topicStatusLabel(topic), tone: 'warn' })]),
          sortToggle(state),
        ],
      ),
      ...posts,
    ]),
  ]
}

// ---------------------------------------------------------------------------
// Receipt view
// ---------------------------------------------------------------------------

const receiptTargetLinks = (receipt: ForumReceiptProjection): ReadonlyArray<View> => {
  const links: View[] = []
  const target = receipt.target ?? null
  if (receipt.targetPostPermalink != null && receipt.targetPostPermalink !== '') {
    links.push(pathLink('receipt-target-post', 'Post', receipt.targetPostPermalink, 'body'))
  }
  if (target?.topicId != null && target.topicId !== '') {
    links.push(pathLink('receipt-target-topic', 'Topic', topicPath(target.topicId), 'body'))
    if (
      (receipt.targetPostPermalink == null || receipt.targetPostPermalink === '') &&
      target.postId != null &&
      target.postId !== ''
    ) {
      links.push(
        pathLink(
          'receipt-target-topic-post',
          'Post',
          `${topicPath(target.topicId)}#post-${encodeURIComponent(target.postId)}`,
          'body',
        ),
      )
    }
  }
  return links.length === 0
    ? [text('receipt-target-none', 'Forum payment', 'body', 'textMuted')]
    : links
}

const receiptRowView = (
  key: string,
  label: string,
  value: ReadonlyArray<View>,
): View =>
  Stack({ key, direction: 'column', gap: '1', style: { width: 'full' } }, [
    text(`${key}-label`, label, 'label', 'accent'),
    ...value,
  ])

const receiptView = (state: ForumPageState, nowMs: number): ReadonlyArray<View> => {
  const receipt = state.receipt
  if (receipt === null) {
    return [unavailableView('receipt-missing', state.errorMessage)]
  }
  return [
    breadcrumb('receipt-crumbs', [
      boardBreadcrumbItem,
      { key: 'crumb-receipt', label: 'Receipt' },
    ]),
    panelCard('receipt-panel', [
      text('receipt-eyebrow', 'Forum receipt', 'label', 'accent'),
      text('receipt-title', receiptActionText(receipt.actionKind), 'heading'),
      text(
        'receipt-summary',
        `${receiptAmountText(receipt.amount)} · ${friendlyTime(receipt.createdAt, nowMs)}`,
        'caption',
        'textMuted',
      ),
      receiptRowView('receipt-ref-row', 'Receipt', [
        text('receipt-ref', receipt.receiptRef ?? '', 'body'),
      ]),
      receiptRowView('receipt-target-row', 'Target', receiptTargetLinks(receipt)),
      receiptRowView('receipt-recipient-row', 'Recipient', [
        text(
          'receipt-recipient',
          receipt.recipientActorRef ?? 'OpenAgents moderation pool',
          'body',
        ),
      ]),
    ]),
  ]
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

export const forumPageView = (
  state: ForumPageState,
  nowMs: number = Date.now(),
): View => {
  const body =
    state.phase === 'loading'
      ? [
          breadcrumb('loading-crumbs', [boardBreadcrumbItem]),
          loadingView('forum-loading'),
        ]
      : state.phase === 'unavailable'
        ? [
            breadcrumb('unavailable-crumbs', [boardBreadcrumbItem]),
            unavailableView('forum-unavailable', state.errorMessage),
          ]
        : state.params.kind === 'index'
          ? indexView(state, nowMs)
          : state.params.kind === 'forum'
            ? forumView(state, nowMs)
            : state.params.kind === 'topic'
              ? topicView(state, nowMs)
              : receiptView(state, nowMs)
  return Stack(
    {
      key: 'forum-root',
      direction: 'column',
      gap: '4',
      padding: '4',
      style: {
        backgroundColor: 'background',
        minHeight: 'full',
        width: 'full',
        maxWidth: 1180,
        alignSelf: 'center',
      },
    },
    body,
  )
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

export type ForumSurfaceDependencies = Readonly<{
  fetchFn?: typeof fetch
  now?: () => number
  copyToClipboard?: (value: string) => Promise<void>
  assignLocation?: (href: string) => void
  scrollToAnchor?: (anchor: string) => void
}>

const loadForumState = (
  params: ForumRouteParams,
  fetchFn: typeof fetch,
): Effect.Effect<Partial<ForumPageState>> =>
  Effect.promise(async (): Promise<Partial<ForumPageState>> => {
    if (params.kind === 'index') {
      const forums = await fetchForumIndex(fetchFn)
      if (forums === null) {
        return { phase: 'unavailable', errorMessage: 'Board index unavailable' }
      }
      return { phase: 'ready', forums }
    }
    if (params.kind === 'forum') {
      const [forum, topics] = await Promise.all([
        fetchForumSummary(params.forumRef, fetchFn),
        fetchForumTopics(params.forumRef, fetchFn),
      ])
      if (forum === null) {
        return { phase: 'unavailable', errorMessage: 'Forum unavailable' }
      }
      return { phase: 'ready', forum, topics: topics ?? [] }
    }
    if (params.kind === 'topic') {
      const [detail, authMode] = await Promise.all([
        fetchForumTopicDetail(params.topicId, params.sortDirection, fetchFn),
        fetchForumAuthMode(fetchFn),
      ])
      if (detail === null) {
        return { phase: 'unavailable', errorMessage: 'Topic unavailable' }
      }
      return {
        phase: 'ready',
        topic: detail.topic,
        posts: detail.posts,
        authMode,
      }
    }
    const receipt = await fetchForumReceipt(params.receiptRef, fetchFn)
    if (receipt === null) {
      return { phase: 'unavailable', errorMessage: 'Receipt unavailable' }
    }
    return { phase: 'ready', receipt }
  })

// ---------------------------------------------------------------------------
// Mount
// ---------------------------------------------------------------------------

const destinationToHref = (destination: NavigationDestination): string =>
  destination.kind === 'path'
    ? destination.path
    : destination.kind === 'url'
      ? destination.href
      : `#${destination.id}`

export const mountForumSurface = (
  container: HTMLElement,
  params: ForumRouteParams,
  deps: ForumSurfaceDependencies = {},
) =>
  Effect.gen(function* () {
    const fetchFn = deps.fetchFn ?? fetch
    const now = deps.now ?? (() => Date.now())
    const assignLocation =
      deps.assignLocation ??
      ((href: string) => {
        window.location.assign(href)
      })
    const copyToClipboard =
      deps.copyToClipboard ??
      (async (value: string) => {
        await navigator.clipboard?.writeText(value)
      })
    const scrollToAnchor =
      deps.scrollToAnchor ??
      ((anchor: string) => {
        const escaped =
          typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
            ? CSS.escape(anchor)
            : anchor.replace(/"/g, '\\"')
        const target = container.querySelector(`[data-en-key="${escaped}"]`)
        if (target instanceof HTMLElement) {
          target.scrollIntoView({ block: 'start' })
        }
      })
    const state = yield* SubscriptionRef.make(initialForumPageState(params))
    const program = makeViewProgramFromState(state, (value) =>
      forumPageView(value, now()),
    )

    const handlers: IntentHandlers<typeof forumIntents> = {
      ForumPermalinkCopied: ({ postId, href }) =>
        Effect.gen(function* () {
          const origin =
            typeof window === 'undefined' ? '' : window.location.origin
          yield* Effect.promise(() =>
            copyToClipboard(`${origin}${href}`).catch(() => undefined),
          )
          yield* SubscriptionRef.update(state, (previous) => ({
            ...previous,
            copiedPermalinkPostId: postId,
          }))
          yield* Effect.sleep(1500)
          yield* SubscriptionRef.update(state, (previous) =>
            previous.copiedPermalinkPostId === postId
              ? { ...previous, copiedPermalinkPostId: null }
              : previous,
          )
        }),
      Navigate: (destination) =>
        Effect.sync(() => {
          assignLocation(destinationToHref(destination))
        }),
    }

    const registry = yield* makeIntentRegistry(forumIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({ theme: khalaTheme }).mount(
      container,
      program.viewStream,
      report,
    )

    // Load the live projections. Fail-soft: any failure leaves the honest
    // unavailable state; nothing is fabricated client-side.
    yield* loadForumState(params, fetchFn).pipe(
      Effect.flatMap((loaded) =>
        SubscriptionRef.update(state, (previous) => ({
          ...previous,
          ...loaded,
        })),
      ),
      Effect.catch(() =>
        SubscriptionRef.update(state, (previous) => ({
          ...previous,
          phase: 'unavailable' as const,
          errorMessage: 'Forum unavailable',
        })),
      ),
    )

    // Deep-link parity: `#post-<id>` / `#post-<n>` anchors scroll to the
    // rendered post once the ready view exists.
    if (typeof window !== 'undefined' && params.kind === 'topic') {
      const rawHash = window.location.hash
      if (rawHash.startsWith('#post-')) {
        yield* Effect.sync(() => {
          const raw = rawHash.slice(1)
          let anchor = raw
          try {
            anchor = decodeURIComponent(raw)
          } catch {
            anchor = raw
          }
          scrollToAnchor(anchor)
        })
      }
    }

    return { state, unmount: surface.unmount }
  })

// ---------------------------------------------------------------------------
// React route-shell hosts (thin mount shims only — no forum content in React)
// ---------------------------------------------------------------------------

const useForumMount = (
  rootRef: RefObject<HTMLDivElement | null>,
  makeParams: () => ForumRouteParams,
): void => {
  useEffect(() => {
    const root = rootRef.current
    if (root === null) {
      return undefined
    }

    let disposed = false
    let closeScope: (() => void) | undefined

    void Effect.runPromise(Scope.make())
      .then((scope) => {
        const close = () => {
          void Effect.runPromise(Scope.close(scope, Exit.void))
        }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(
          Scope.provide(scope)(mountForumSurface(root, makeParams())),
        )
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}

type ForumShellProps = Readonly<{
  route: string
  makeParams: () => ForumRouteParams
}>

const ForumShell = ({ route, makeParams }: ForumShellProps) => {
  const rootRef = useRef<HTMLDivElement | null>(null)
  useForumMount(rootRef, makeParams)
  return (
    <main
      aria-label="OpenAgents Forum"
      data-route={route}
      data-forum-en=""
    >
      <div ref={rootRef} data-forum-en-root="" />
    </main>
  )
}

export function ForumIndexPage() {
  return (
    <ForumShell route="forum" makeParams={() => ({ kind: 'index' })} />
  )
}

export function ForumForumPage({ forumRef }: Readonly<{ forumRef: string }>) {
  return (
    <ForumShell
      route="forum-forum"
      makeParams={() => ({ kind: 'forum', forumRef })}
    />
  )
}

export function ForumTopicPage({ topicId }: Readonly<{ topicId: string }>) {
  return (
    <ForumShell
      route="forum-topic"
      makeParams={() => ({
        kind: 'topic',
        topicId,
        sortDirection: parseTopicPostSortDirection(
          typeof window === 'undefined' ? '' : window.location.search,
        ),
      })}
    />
  )
}

export function ForumReceiptPage({
  receiptRef,
}: Readonly<{ receiptRef: string }>) {
  return (
    <ForumShell
      route="forum-receipt"
      makeParams={() => ({ kind: 'receipt', receiptRef })}
    />
  )
}
