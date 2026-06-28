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
  ArtanisOperatorDashboardMessage,
  ArtanisOperatorDashboardResponse,
  ArtanisOperatorDashboardThread,
  Model,
} from '../model'

const shortTime = (iso: string): string =>
  iso.replace('T', ' ').replace('.000Z', 'Z')

const humanLabel = (value: string): string => value.replace(/_/g, ' ')

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
