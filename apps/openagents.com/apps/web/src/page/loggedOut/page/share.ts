import type {
  ShareProjectionV1,
  WorkroomFileItem,
  WorkroomTimelineMessage,
} from '@openagentsinc/sync-schema'
import { clsx } from 'clsx'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import { iconView } from '../../../icon'
import { homeRouter } from '../../../route'
import * as Ui from '../../../ui'
import { ClickedCopyShareLink, type Message } from '../message'
import type { ShareProjectionModel } from '../model'

const shareLoginHref = (shareId: string): string =>
  `/login/github?returnTo=${encodeURIComponent(`/share/${shareId}`)}`

const shareStatusLabel = (projection: ShareProjectionV1): string =>
  projection.status === 'active'
    ? 'Active'
    : projection.status === 'expired'
      ? 'Expired'
      : 'Revoked'

const sourceKindLabel = (projection: ShareProjectionV1): string =>
  projection.source.kind === 'agent-run'
    ? 'Agent run'
    : projection.source.kind === 'team-thread'
      ? 'Team thread'
      : 'Project thread'

const sourceHref = (projection: ShareProjectionV1): string | undefined =>
  projection.source.kind === 'agent-run'
    ? `/t/${projection.source.id}`
    : projection.source.kind === 'team-thread'
      ? `/teams/${projection.source.teamId ?? projection.source.id}/chat`
      : `/teams/${projection.source.teamId}/projects/${projection.source.projectId ?? projection.source.id}/chat`

const sourceLabel = (projection: ShareProjectionV1): string =>
  projection.source.kind === 'agent-run'
    ? 'Open source run'
    : 'Open source thread'

const formatShareTimestamp = (value: string): string => {
  const [date, timeWithZone] = value.trim().split('T')
  const time = timeWithZone?.slice(0, 5)

  if (date === undefined || date === '' || time === undefined || time === '') {
    return value
  }

  return `${date} ${time} UTC`
}

const reviewItemCountLabel = (count: number): string =>
  count === 1 ? '1 item' : `${count} items`

const topBarLink = (input: {
  href: string
  icon: 'Copy' | 'ExternalLink'
  label: string
}): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(input.href),
      h.AriaLabel(input.label),
      Ui.className<Message>(
        'inline-flex min-h-8 items-center gap-2 border border-[#333] bg-[#080808] px-2.5 text-[0.75rem] text-white/65 no-underline hover:border-[#555] hover:text-[#f1efe8]',
      ),
    ],
    [
      iconView<Message>(input.icon, 'size-4 text-white/50'),
      h.span([Ui.className<Message>('max-[640px]:hidden')], [input.label]),
    ],
  )
}

const copyLinkButton = (url: string): Html => {
  const h = html<Message>()

  return h.button(
    [
      h.Type('button'),
      h.AriaLabel('Copy share link'),
      h.OnClick(ClickedCopyShareLink({ url })),
      Ui.className<Message>(
        'inline-flex min-h-8 items-center gap-2 border border-[#333] bg-[#080808] px-2.5 text-[0.75rem] text-white/65 hover:border-[#555] hover:text-[#f1efe8]',
      ),
    ],
    [
      iconView<Message>('Copy', 'size-4 text-white/50'),
      h.span([Ui.className<Message>('max-[640px]:hidden')], ['Copy link']),
    ],
  )
}

const shareHeader = (
  projection: ShareProjectionV1,
  reviewItems: ReadonlyArray<WorkroomFileItem>,
): Html => {
  const h = html<Message>()
  const href = sourceHref(projection)

  return h.header(
    [
      h.DataAttribute('component', 'share-header'),
      Ui.className<Message>(
        'flex h-12 flex-none items-center justify-between gap-3 border-b border-[#222] bg-[#010102] px-4 max-[760px]:px-3',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex min-w-0 items-center gap-3')],
        [
          h.a(
            [
              h.Href('/'),
              h.AriaLabel('OpenAgents'),
              Ui.className<Message>(
                'inline-flex size-6 shrink-0 items-center justify-center border border-[#333] bg-[#080808] text-[#f1efe8] no-underline hover:border-[#555]',
              ),
            ],
            [iconView<Message>('Terminal', 'size-4 text-[#f1efe8]')],
          ),
          h.div(
            [
              h.DataAttribute('share-audience-label', ''),
              Ui.className<Message>(
                'min-w-0 truncate text-[0.75rem] font-semibold text-[#ffb400]',
              ),
            ],
            [userFacingCopy(projection.audienceLabel)],
          ),
          h.div(
            [
              Ui.className<Message>(
                'hidden min-w-0 truncate text-[0.75rem] text-white/35 md:block',
              ),
            ],
            [userFacingCopy(projection.title)],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 shrink-0 items-center gap-2 max-[760px]:gap-1.5',
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                'hidden min-h-8 items-center border border-[#333] px-2.5 text-[0.75rem] text-white/55 sm:inline-flex',
              ),
            ],
            [shareStatusLabel(projection)],
          ),
          reviewItems.length === 0
            ? h.empty
            : h.span(
                [
                  Ui.className<Message>(
                    'hidden min-h-8 items-center border border-[#333] px-2.5 text-[0.75rem] text-white/55 lg:inline-flex',
                  ),
                ],
                [reviewItemCountLabel(reviewItems.length)],
              ),
          copyLinkButton(projection.url),
          href === undefined
            ? h.empty
            : topBarLink({
                href,
                icon: 'ExternalLink',
                label: sourceLabel(projection),
              }),
        ],
      ),
    ],
  )
}

const titleBadge = (label: string, tone: 'accent' | 'neutral'): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        clsx(
          'inline-flex min-h-6 items-center border px-2 text-[0.75rem]',
          tone === 'accent'
            ? 'border-[#ffb400]/70 bg-[#ffb400]/10 text-[#ffb400]'
            : 'border-[#333] bg-[#080808] text-white/55',
        ),
      ),
    ],
    [label],
  )
}

const sessionTitleBlock = (
  projection: ShareProjectionV1,
  reviewItems: ReadonlyArray<WorkroomFileItem>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'share-session-title'),
      Ui.className<Message>(
        'grid w-full max-w-[980px] gap-4 px-6 py-6 max-[760px]:px-3 max-[760px]:py-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 flex-wrap items-center gap-2 text-[0.75rem] text-white/45',
          ),
        ],
        [
          titleBadge(userFacingCopy(projection.audienceLabel), 'accent'),
          titleBadge(sourceKindLabel(projection), 'neutral'),
          titleBadge(shareStatusLabel(projection), 'neutral'),
          h.span(
            [
              Ui.className<Message>(
                'min-w-0 break-words max-[760px]:basis-full',
              ),
            ],
            [formatShareTimestamp(projection.createdAt)],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.h1(
            [
              Ui.className<Message>(
                'm-0 min-w-0 text-base font-medium text-[#f1efe8]',
              ),
            ],
            [userFacingCopy(projection.title)],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[74ch] text-base text-white/45 sm:text-[0.8125rem]',
              ),
            ],
            [userFacingCopy(projection.subtitle)],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1 text-[0.75rem] text-white/40',
          ),
        ],
        [
          h.span([], [`${projection.metrics.eventCount} events`]),
          h.span([], [`${projection.metrics.toolCallCount} tools`]),
          h.span([], [`${projection.metrics.tokenTotal} tokens`]),
          reviewItems.length === 0
            ? h.empty
            : h.span(
                [],
                [`${reviewItemCountLabel(reviewItems.length)} in review`],
              ),
        ],
      ),
    ],
  )
}

const messagePreview = (message: WorkroomTimelineMessage): string => {
  const body = message.parts
    .flatMap(part => (part.kind === 'text' ? part.body : []))
    .join('\n')
    .trim()
  const firstLine = body.split('\n')[0]?.trim()

  return userFacingCopy(
    firstLine === undefined || firstLine === '' ? message.label : firstLine,
  )
}

const messageNav = (messages: ReadonlyArray<WorkroomTimelineMessage>): Html => {
  const h = html<Message>()
  const userMessages = messages.filter(message => message.author === 'user')

  if (userMessages.length <= 1) {
    return h.empty
  }

  return h.nav(
    [
      h.AriaLabel('Message navigation'),
      Ui.className<Message>(
        'sticky top-4 hidden w-44 shrink-0 self-start px-2 py-1 lg:block',
      ),
    ],
    [
      h.ul(
        [h.Role('list'), Ui.className<Message>('m-0 grid list-none gap-1 p-0')],
        userMessages.map((message, index) =>
          h.li(
            [Ui.className<Message>('min-w-0')],
            [
              h.a(
                [
                  h.Href(`#message-${message.id}`),
                  Ui.className<Message>(
                    'grid min-h-8 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)] items-center gap-2 border border-transparent px-2 text-[0.75rem] text-white/45 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
                  ),
                ],
                [
                  h.span(
                    [Ui.className<Message>('tabular-nums')],
                    [String(index + 1).padStart(2, '0')],
                  ),
                  h.span(
                    [Ui.className<Message>('truncate')],
                    [messagePreview(message)],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

const metadataRows = (projection: ShareProjectionV1) => {
  const h = html<Message>()

  return [
    {
      label: 'Status',
      value: h.span([], [shareStatusLabel(projection)]),
    },
    {
      label: 'Source',
      value: h.span([], [sourceKindLabel(projection)]),
    },
    {
      label: 'Created',
      value: h.span([], [formatShareTimestamp(projection.createdAt)]),
    },
    {
      label: 'Events',
      value: h.span([], [String(projection.metrics.eventCount)]),
    },
    {
      label: 'Tools',
      value: h.span([], [String(projection.metrics.toolCallCount)]),
    },
    {
      label: 'Tokens',
      value: h.span([], [String(projection.metrics.tokenTotal)]),
    },
  ]
}

const fileRows = (
  projection: ShareProjectionV1,
): ReadonlyArray<WorkroomFileItem> => [
  ...projection.files,
  ...projection.artifacts.map(artifact => ({
    label: artifact,
    meta: 'artifact',
    depth: 1 as const,
  })),
  ...projection.approvals.map(approval => ({
    label: approval,
    meta: 'approval',
    depth: 1 as const,
  })),
  ...projection.receipts.map(receipt => ({
    label: receipt,
    meta: 'receipt',
    depth: 1 as const,
  })),
]

const mobileReviewPanel = (
  projection: ShareProjectionV1,
  reviewItems: ReadonlyArray<WorkroomFileItem>,
): Html => {
  const h = html<Message>()

  if (reviewItems.length === 0) {
    return h.empty
  }

  return h.details(
    [
      h.DataAttribute('component', 'share-mobile-review'),
      Ui.className<Message>(
        'mx-3 mb-6 border border-[#222] bg-[#020202] lg:hidden',
      ),
    ],
    [
      h.summary(
        [
          Ui.className<Message>(
            'flex min-h-10 cursor-pointer items-center justify-between gap-3 px-3 text-[0.8125rem] text-[#f1efe8] [&::-webkit-details-marker]:hidden',
          ),
        ],
        [
          h.span([], ['Review']),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [reviewItemCountLabel(reviewItems.length)],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid border-t border-[#222]')],
        [
          ...metadataRows(projection).map(row =>
            h.div(
              [
                Ui.className<Message>(
                  'grid min-h-9 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-[#111] px-3 text-[0.75rem]',
                ),
              ],
              [
                h.span([Ui.className<Message>('text-white/35')], [row.label]),
                h.div(
                  [Ui.className<Message>('text-right text-[#f1efe8]')],
                  [row.value],
                ),
              ],
            ),
          ),
          h.ul(
            [
              h.Role('list'),
              Ui.className<Message>('m-0 grid list-none gap-0.5 p-2'),
            ],
            reviewItems.map(item =>
              h.li(
                [
                  Ui.className<Message>(
                    clsx(
                      'grid min-h-8 min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-transparent px-2 text-[0.8125rem]',
                      item.depth === 1 ? 'pl-6' : '',
                    ),
                  ),
                ],
                [
                  h.span(
                    [Ui.className<Message>('truncate text-[#f1efe8]')],
                    [userFacingCopy(item.label)],
                  ),
                  h.span(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    [item.meta],
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    ],
  )
}

const messageTextBody = (message: WorkroomTimelineMessage): string =>
  message.parts
    .flatMap(part => (part.kind === 'text' ? part.body : []))
    .join('\n')

const timelineMessageForDisplay = (
  message: WorkroomTimelineMessage,
): WorkroomTimelineMessage => ({
  ...message,
  label: userFacingCopy(message.label),
  parts: message.parts.map(part =>
    part.kind === 'text'
      ? {
          ...part,
          body: part.body.map(userFacingCopy),
        }
      : part.kind === 'tool'
        ? {
            ...part,
            title: userFacingCopy(part.title),
            subtitle: userFacingCopy(part.subtitle),
            detail: part.detail.map(userFacingCopy),
            ...(part.actionLabel === undefined
              ? {}
              : { actionLabel: userFacingCopy(part.actionLabel) }),
          }
        : part.kind === 'file'
          ? {
              ...part,
              excerpt: part.excerpt.map(userFacingCopy),
            }
          : part,
  ),
})

const shareUserTimelineMessage = (message: WorkroomTimelineMessage): Html => {
  const h = html<Message>()
  const displayMessage = timelineMessageForDisplay(message)
  const body = messageTextBody(displayMessage)
  const nonTextParts = displayMessage.parts.filter(part => part.kind !== 'text')

  return h.article(
    [
      h.Id(`message-${message.id}`),
      h.DataAttribute('author', message.author),
      h.DataAttribute('timeline-row', 'UserMessage'),
      Ui.className<Message>('min-w-0 w-full max-w-[920px]'),
    ],
    [
      h.div(
        [
          h.DataAttribute('component', 'share-user-message'),
          Ui.className<Message>(
            'grid min-w-0 gap-3 px-4 md:px-5 max-[760px]:px-0',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex min-w-0 max-w-full items-start gap-3 text-left',
              ),
            ],
            [
              displayMessage.avatarUrl === undefined ||
              displayMessage.avatarUrl === ''
                ? Ui.v4AgentIcon<Message>({ label: displayMessage.label })
                : h.img([
                    h.Src(displayMessage.avatarUrl),
                    h.Alt(''),
                    Ui.className<Message>(
                      'size-8 shrink-0 rounded-[4px] border border-[var(--outline,#525458)] object-cover',
                    ),
                  ]),
              h.div(
                [Ui.className<Message>('grid min-w-0 flex-1 gap-1')],
                [
                  h.h3(
                    [
                      Ui.className<Message>(
                        'm-0 font-mono text-sm font-bold text-[var(--primary,#fff)]',
                      ),
                    ],
                    [displayMessage.label],
                  ),
                  body.trim() === ''
                    ? h.empty
                    : h.p(
                        [
                          Ui.className<Message>(
                            'm-0 min-w-0 max-w-full whitespace-pre-wrap break-words text-sm leading-6 text-[var(--text,#d7d8e5)]',
                          ),
                        ],
                        [body],
                      ),
                ],
              ),
            ],
          ),
          ...nonTextParts.map(part => Ui.workroomTimelinePart<Message>(part)),
        ],
      ),
    ],
  )
}

const shareTimelineMessage = (message: WorkroomTimelineMessage): Html =>
  message.author === 'user'
    ? shareUserTimelineMessage(message)
    : Ui.workroomTimelineMessage<Message>(timelineMessageForDisplay(message))

const shareTimeline = (
  projection: ShareProjectionV1,
  reviewItems: ReadonlyArray<WorkroomFileItem>,
): Html => {
  const h = html<Message>()
  const isActive = projection.messages.some(
    message => message.status === 'streaming',
  )

  return h.section(
    [
      h.DataAttribute('component', 'share-session'),
      h.AriaLabel('Shared conversation'),
      Ui.className<Message>(
        'relative h-full min-h-0 min-w-0 flex-1 overflow-hidden bg-[#000]',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            clsx(
              'absolute inset-x-0 top-0 z-[2] h-px overflow-hidden oa-progress-strip',
              { 'is-active': isActive },
            ),
          ),
        ],
        [h.span([], [])],
      ),
      h.div(
        [
          Ui.className<Message>(
            'absolute inset-0 overflow-auto overscroll-contain pb-10',
          ),
        ],
        [
          sessionTitleBlock(projection, reviewItems),
          mobileReviewPanel(projection, reviewItems),
          h.div(
            [
              Ui.className<Message>(
                'flex min-w-0 items-start gap-2 px-[clamp(12px,4vw,56px)] max-[760px]:px-3',
              ),
            ],
            [
              messageNav(projection.messages),
              h.div(
                [
                  Ui.className<Message>(
                    'flex min-w-0 flex-1 flex-col gap-[26px]',
                  ),
                ],
                projection.messages.length === 0
                  ? [
                      Ui.emptyState<Message>({
                        title: 'No messages',
                        body: 'This share does not include transcript messages.',
                      }),
                    ]
                  : [
                      ...projection.messages.map(message =>
                        shareTimelineMessage(message),
                      ),
                      Ui.workroomTimelineEndMarker<Message>([
                        h.DataAttribute('share-timeline-end', 'true'),
                      ]),
                    ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const sidePanel = (
  projection: ShareProjectionV1,
  reviewItems: ReadonlyArray<WorkroomFileItem>,
): Html =>
  Ui.workroomFilePanel<Message>({
    tabs: [],
    rows: metadataRows(projection),
    files: reviewItems,
    docks: [
      Ui.workroomPanelActionRow<Message>({
        label: 'Share',
        action: Ui.compactButton<Message>({
          label: userFacingCopy(projection.audienceLabel),
          attrs: [
            html<Message>().AriaLabel(userFacingCopy(projection.audienceLabel)),
          ],
        }),
      }),
    ],
  })

const loadedView = (projection: ShareProjectionV1): Html => {
  const h = html<Message>()
  const reviewItems = fileRows(projection)
  const main = Ui.workroomContent<Message>([
    shareTimeline(projection, reviewItems),
  ])

  return h.div(
    [
      h.DataAttribute('component', 'share-page'),
      Ui.className<Message>(
        'isolate flex h-dvh min-h-[720px] w-full min-w-0 flex-col overflow-hidden bg-[#000] font-mono text-[#f1efe8] antialiased',
      ),
    ],
    [
      shareHeader(projection, reviewItems),
      h.div(
        [
          Ui.className<Message>(
            clsx(
              'grid h-full min-h-0 min-w-0 flex-1 grid-rows-[minmax(0,1fr)] overflow-hidden select-text',
              reviewItems.length === 0
                ? 'grid-cols-[minmax(0,1fr)]'
                : 'grid-cols-[minmax(0,1fr)_420px] max-[1200px]:grid-cols-[minmax(0,1fr)_380px] max-[1100px]:grid-cols-[minmax(0,1fr)]',
            ),
          ),
        ],
        [
          main,
          reviewItems.length === 0
            ? h.empty
            : sidePanel(projection, reviewItems),
        ],
      ),
    ],
  )
}

const failedBody = (
  input: Readonly<{ error: string; shareId: string; status: number }>,
): Html => {
  if (input.status === 401) {
    return Ui.emptyState({
      title: 'Sign in to view this share',
      body: 'This share is restricted to specific OpenAgents members.',
      action: Ui.linkButton({
        href: shareLoginHref(input.shareId),
        label: 'Sign in',
      }),
    })
  }

  if (input.status === 403) {
    return Ui.emptyState({
      title: 'Share unavailable',
      body: 'This share is not available to your account.',
      action: Ui.linkButton({ href: homeRouter(), label: 'Go Home' }),
    })
  }

  if (input.status === 410) {
    return Ui.emptyState({
      title:
        input.error === 'share_expired' ? 'Share expired' : 'Share revoked',
      body: 'This shared projection is no longer available.',
      action: Ui.linkButton({ href: homeRouter(), label: 'Go Home' }),
    })
  }

  return Ui.emptyState({
    title: 'Share not found',
    body: 'This share does not exist or is no longer available.',
    action: Ui.linkButton({ href: homeRouter(), label: 'Go Home' }),
  })
}

export const view = (share: ShareProjectionModel): Html => {
  const h = html<Message>()

  if (share._tag === 'ShareProjectionLoaded') {
    return loadedView(share.projection)
  }

  return h.div(
    [
      h.DataAttribute('component', 'share-page'),
      Ui.className<Message>(
        'grid min-h-[calc(100dvh-96px)] place-items-center bg-[#000] px-4 py-12 font-mono text-[#f1efe8]',
      ),
    ],
    [
      share._tag === 'ShareProjectionFailed'
        ? failedBody(share)
        : Ui.emptyState({
            title: 'Loading share',
            body: 'Preparing the shared workroom.',
          }),
    ],
  )
}
