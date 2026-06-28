import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  SelectedArtanisOperatorDashboardThread,
  SubmittedArtanisOperatorDashboardFilter,
  UpdatedArtanisOperatorDashboardCallerIdFilter,
  type Message,
} from '../message'
import type {
  ArtanisOperatorDashboardAccountUsage,
  ArtanisOperatorDashboardAccountUsageEntry,
  ArtanisOperatorDashboardAccountUsageWindow,
  ArtanisOperatorDashboardMessage,
  ArtanisOperatorDashboardResponse,
  ArtanisOperatorDashboardThread,
  Model,
} from '../model'

const shortTime = (iso: string): string =>
  iso.replace('T', ' ').replace('.000Z', 'Z')

const humanLabel = (value: string): string => value.replace(/_/g, ' ')

const formatNumber = (value: number): string =>
  new Intl.NumberFormat('en-US').format(value)

const usageText = (
  window: ArtanisOperatorDashboardAccountUsageWindow,
): string =>
  window.used === null || window.cap === null || window.remaining === null
    ? 'usage unmeasured'
    : `${formatNumber(window.used)} / ${formatNumber(window.cap)} used · ${formatNumber(window.remaining)} remaining`

const emptyText = (label: string): Html =>
  html<Message>().p(
    [Ui.className<Message>('m-0 text-[0.8125rem] leading-6 text-white/50')],
    [label],
  )

const threadTitle = (thread: ArtanisOperatorDashboardThread): string =>
  thread.title.trim() === ''
    ? `${thread.subjectAgentRef} / ${thread.callerId}`
    : thread.title

const threadButton = (
  thread: ArtanisOperatorDashboardThread,
  selectedThreadRef: string | null,
): Html => {
  const h = html<Message>()
  const active = thread.threadRef === selectedThreadRef

  return h.button(
    [
      h.Type('button'),
      h.OnClick(
        SelectedArtanisOperatorDashboardThread({
          threadRef: thread.threadRef,
        }),
      ),
      Ui.className<Message>(
        [
          'grid w-full gap-1 rounded-md border px-3 py-2 text-left transition',
          active
            ? 'border-[#3a7bff]/60 bg-[#3a7bff]/10 shadow-[0_0_24px_-10px_rgba(58,123,255,.7)]'
            : 'border-white/10 bg-[#0c0f13]/90 hover:border-[#3a7bff]/35 hover:bg-[#11161d]',
        ].join(' '),
      ),
    ],
    [
      h.span(
        [Ui.className<Message>('text-[0.8125rem] font-semibold leading-5 text-white')],
        [threadTitle(thread)],
      ),
      h.span(
        [Ui.className<Message>('text-[0.6875rem] leading-4 text-[#8fb6ff]')],
        [
          `${thread.callerId} · ${humanLabel(thread.subjectAgentKind)} · ${thread.messageCount} turns`,
        ],
      ),
      h.span(
        [Ui.className<Message>('text-[0.6875rem] leading-4 text-white/40')],
        [shortTime(thread.lastMessageAt)],
      ),
    ],
  )
}

const threadList = (response: ArtanisOperatorDashboardResponse): Html => {
  const h = html<Message>()
  const selectedThreadRef = response.selectedThread?.threadRef ?? null

  return h.aside(
    [
      Ui.className<Message>(
        'grid content-start gap-2 border-r border-white/10 pr-4 max-lg:border-r-0 max-lg:border-b max-lg:pb-4 max-lg:pr-0',
      ),
    ],
    response.threads.length === 0
      ? [emptyText('No Artanis operator threads match this filter.')]
      : response.threads.map(thread => threadButton(thread, selectedThreadRef)),
  )
}

const messageView = (message: ArtanisOperatorDashboardMessage): Html => {
  const h = html<Message>()
  const isOperator =
    message.authorKind === 'owner' || message.authorKind === 'operator'

  return h.article(
    [
      Ui.className<Message>(
        [
          'grid gap-2 rounded-md border px-4 py-3',
          isOperator
            ? 'border-[#3a7bff]/30 bg-[#0c0f13]/95'
            : 'border-white/10 bg-[#11161d]/80',
        ].join(' '),
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center justify-between gap-2')],
        [
          h.span(
            [Ui.className<Message>('text-[0.75rem] font-semibold leading-5 text-white/80')],
            [`${humanLabel(message.authorKind)} / ${message.authorId}`],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] leading-4 text-white/40')],
            [shortTime(message.createdAt)],
          ),
        ],
      ),
      h.pre(
        [
          Ui.className<Message>(
            'm-0 whitespace-pre-wrap break-words font-mono text-[0.8125rem] leading-6 text-[#c9d2dd]',
          ),
        ],
        [message.body],
      ),
    ],
  )
}

const transcript = (response: ArtanisOperatorDashboardResponse): Html => {
  const h = html<Message>()
  const thread = response.selectedThread

  if (thread === null) {
    return h.section([Ui.className<Message>('grid gap-3')], [
      h.h1([Ui.className<Message>('m-0 text-xl font-semibold text-white')], [
        'Artanis operator dashboard',
      ]),
      emptyText('No thread selected.'),
    ])
  }

  return h.section([Ui.className<Message>('grid content-start gap-4')], [
    h.div([Ui.className<Message>('grid gap-1')], [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Conversation']),
      h.h1(
        [Ui.className<Message>('m-0 text-2xl font-semibold leading-tight text-white')],
        [threadTitle(thread)],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.8125rem] leading-6 text-white/55')],
        [
          `${thread.callerId} · ${humanLabel(thread.callerKind)} · ${thread.subjectAgentRef} · ${shortTime(thread.lastMessageAt)}`,
        ],
      ),
    ]),
    response.messages.length === 0
      ? emptyText('This thread has no recorded messages.')
      : h.div(
          [Ui.className<Message>('grid gap-3')],
          response.messages.map(messageView),
        ),
  ])
}

const usageBarClass = (
  entry: ArtanisOperatorDashboardAccountUsageEntry,
  window: ArtanisOperatorDashboardAccountUsageWindow,
): string => {
  if (window.used === null || window.cap === null) {
    return 'bg-white/25'
  }

  if (entry.isRateLimited || window.percentUsed >= 90) {
    return 'bg-[#d32f2f]'
  }

  if (window.percentUsed >= 70) {
    return 'bg-[#ffb400]'
  }

  return 'bg-[#00c853]'
}

const accountUsageWindowView = (
  entry: ArtanisOperatorDashboardAccountUsageEntry,
  window: ArtanisOperatorDashboardAccountUsageWindow,
): Html => {
  const h = html<Message>()
  const percent = Math.max(0, Math.min(100, window.percentUsed))

  return h.div([Ui.className<Message>('grid gap-1.5')], [
    h.div([Ui.className<Message>('flex items-center justify-between gap-3')], [
      h.span(
        [Ui.className<Message>('text-[0.6875rem] font-semibold leading-4 text-white/65')],
        [humanLabel(window.label)],
      ),
      h.span(
        [Ui.className<Message>('text-[0.6875rem] leading-4 text-white/45')],
        [`${percent}%`],
      ),
    ]),
    h.div(
      [
        h.AriaLabel(`${entry.provider} ${window.label} token usage ${percent}%`),
        h.Role('meter'),
        h.Attribute('aria-valuemin', '0'),
        h.Attribute('aria-valuemax', '100'),
        h.Attribute('aria-valuenow', String(percent)),
        Ui.className<Message>('h-2 w-full overflow-hidden rounded-sm bg-white/10'),
      ],
      [
        h.div(
          [
            h.Style({ width: `${percent}%` }),
            Ui.className<Message>(`h-full rounded-sm ${usageBarClass(entry, window)}`),
          ],
          [],
        ),
      ],
    ),
    h.div(
      [Ui.className<Message>('text-[0.6875rem] leading-4 text-white/45')],
      [usageText(window)],
    ),
  ])
}

const accountUsageEntryView = (
  entry: ArtanisOperatorDashboardAccountUsageEntry,
): Html => {
  const h = html<Message>()
  const stateClass = entry.isRateLimited
    ? 'border-[#d32f2f]/45 bg-[#1b0d0d]'
    : 'border-[#00c853]/30 bg-[#07150c]'
  const stateLabel = entry.isRateLimited ? 'limited' : 'available'
  const cooldown =
    entry.cooldownExpiresAt === null
      ? 'no cooldown'
      : `resets ${shortTime(entry.cooldownExpiresAt)}`
  const resets =
    entry.manualResetsRemaining === null
      ? 'manual resets unknown'
      : `${entry.manualResetsRemaining} manual resets`

  return h.article(
    [Ui.className<Message>('grid gap-3 border border-white/10 bg-[#0c0f13]/90 p-3')],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-2')], [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2(
            [Ui.className<Message>('m-0 text-[0.8125rem] font-semibold leading-5 text-white')],
            [`${entry.provider} · ${entry.accountRefHash.slice(0, 10)}`],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-[0.6875rem] leading-4 text-white/45')],
            [`${cooldown} · ${resets}`],
          ),
        ]),
        h.span(
          [
            Ui.className<Message>(
              `rounded-sm border px-2 py-1 text-[0.6875rem] font-semibold leading-none ${stateClass}`,
            ),
          ],
          [stateLabel],
        ),
      ]),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-2')],
        entry.windows.map(window => accountUsageWindowView(entry, window)),
      ),
    ],
  )
}

const accountUsagePanel = (
  accountUsage: ArtanisOperatorDashboardAccountUsage | undefined,
): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-3')], [
    h.div([Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Fleet capacity']),
        h.h2(
          [Ui.className<Message>('m-0 text-xl font-semibold leading-tight text-white')],
          ['Token usage windows'],
        ),
      ]),
      h.span(
        [Ui.className<Message>('text-[0.6875rem] leading-4 text-white/40')],
        [accountUsage === undefined ? 'not loaded' : shortTime(accountUsage.observedAt)],
      ),
    ]),
    accountUsage === undefined || accountUsage.accounts.length === 0
      ? emptyText('No account usage rows are available.')
      : h.div(
          [Ui.className<Message>('grid gap-3 xl:grid-cols-2')],
          accountUsage.accounts.map(accountUsageEntryView),
        ),
  ])
}

const loadedView = (
  model: Model,
  response: ArtanisOperatorDashboardResponse,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'artanis-operator-dashboard'),
      Ui.className<Message>('grid gap-5'),
    ],
    [
      h.header([Ui.className<Message>('grid gap-3')], [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], [
          'Artanis operator',
        ]),
        h.div(
          [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
          [
            h.div([Ui.className<Message>('grid gap-1')], [
              h.h1(
                [Ui.className<Message>('m-0 text-3xl font-semibold leading-tight text-white')],
                ['Operator dashboard'],
              ),
              h.p(
                [Ui.className<Message>('m-0 max-w-3xl text-[0.875rem] leading-6 text-white/60')],
                [
                  'All recorded Artanis operator threads across owner and agent callers.',
                ],
              ),
            ]),
            h.form(
              [
                h.OnSubmit(SubmittedArtanisOperatorDashboardFilter()),
                Ui.className<Message>('flex min-w-64 items-center gap-2'),
              ],
              [
                h.input([
                  h.AriaLabel('Filter by caller id'),
                  h.Name('caller_id'),
                  h.Placeholder('caller_id'),
                  h.Value(model.artanisOperatorDashboardCallerIdFilter),
                  h.OnInput(value =>
                    UpdatedArtanisOperatorDashboardCallerIdFilter({ value }),
                  ),
                  Ui.className<Message>(Ui.inputClass),
                ]),
                Ui.compactButton<Message>({
                  label: 'Filter',
                  variant: 'strong',
                  attrs: [h.Type('submit')],
                }),
              ],
            ),
          ],
        ),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'grid min-h-[32rem] grid-cols-[minmax(18rem,24rem)_1fr] gap-4 max-lg:grid-cols-1',
          ),
        ],
        [threadList(response), transcript(response)],
      ),
      accountUsagePanel(response.accountUsage),
    ],
  )
}

export const view = (model: Model): Html =>
  M.value(model.artanisOperatorDashboard).pipe(
    M.tagsExhaustive({
      ArtanisOperatorDashboardIdle: () =>
        emptyText('Loading Artanis operator threads.'),
      ArtanisOperatorDashboardLoading: () =>
        emptyText('Loading Artanis operator threads.'),
      ArtanisOperatorDashboardFailed: ({ error }) => emptyText(error),
      ArtanisOperatorDashboardLoaded: ({ response }) =>
        loadedView(model, response),
    }),
  )
