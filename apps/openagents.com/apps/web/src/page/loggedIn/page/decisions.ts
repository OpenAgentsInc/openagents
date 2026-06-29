import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { formatIsoDateTime } from '../../../time-format'
import { autopilotWorkDetailRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  Message,
  RequestedLoadAutopilotDecisions,
  SubmittedAutopilotDecisionAction,
} from '../message'
import type {
  AutopilotDecisionCloseoutReceipt,
  AutopilotDecisionQueueItem,
  AutopilotWorkReviewAction,
  Model,
} from '../model'

type Tone = 'accent' | 'positive' | 'warning' | 'negative' | 'info'

const statusTone = (status: string): Tone =>
  M.value(status).pipe(
    M.when('available', () => 'warning' as const),
    M.when('recommended', () => 'warning' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('completed', () => 'positive' as const),
    M.orElse(() => 'info' as const),
  )

const badge = (label: string, tone: Tone): Html => {
  const h = html<Message>()
  const color =
    tone === 'positive'
      ? 'border-[#1b5e20] text-[#7ccf8a]'
      : tone === 'warning'
        ? 'border-[#5a3b00] text-[#ffb400]'
        : tone === 'negative'
          ? 'border-[#5c1f1f] text-[#ff8a80]'
          : tone === 'info'
            ? 'border-[#1d3d63] text-[#8ab4ff]'
            : 'border-[#333] text-white/65'

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex min-h-7 items-center border px-2 text-[0.6875rem] uppercase ${color}`,
      ),
    ],
    [label],
  )
}

const loadingView = (label: string): Html => {
  const h = html<Message>()

  return h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [label])
}

const errorView = (error: string): Html => {
  const h = html<Message>()

  return h.p([Ui.className<Message>('m-0 text-sm text-[#ff8a80]')], [error])
}

const emptyView = (): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border border-[#222] bg-[#080808] p-5')],
    [
      h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
        'No decisions waiting',
      ]),
      h.p([Ui.className<Message>('m-0 mt-2 text-sm/6 text-white/50')], [
        'Nothing needs your decision right now. Delivered work that needs review will appear here.',
      ]),
    ],
  )
}

const summaryText = (item: AutopilotDecisionQueueItem): string =>
  M.value(item.decision.safeSummaryRef).pipe(
    M.when('summary.delivered_work_awaits_review', () =>
      'Delivered work is waiting for your review decision.'),
    M.when('summary.customer_access_required', () =>
      'Autopilot needs access from you before work can continue.'),
    M.when('summary.customer_payment_required', () =>
      'This work order is waiting on payment before it can run.'),
    M.when('summary.review_decision_recorded', () =>
      'Your review decision was recorded.'),
    M.orElse(() => item.decision.safeSummaryRef),
  )

const refChips = (refs: ReadonlyArray<string>): ReadonlyArray<Html> => {
  const h = html<Message>()

  return refs.map(ref =>
    h.span(
      [
        Ui.className<Message>(
          'min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap border border-[#222] px-2 py-1 text-xs text-white/55',
        ),
      ],
      [ref],
    ),
  )
}

const closeoutReceiptRow = (
  receipt: AutopilotDecisionCloseoutReceipt,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border border-[#222] bg-[#080808] px-3 py-2 text-xs text-white/55',
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(receipt.outcome, 'positive'),
        h.span([Ui.className<Message>('text-white/65')], [
          receipt.resolvedState.replaceAll('_', ' '),
        ]),
        h.span([Ui.className<Message>('text-white/35')], [
          formatIsoDateTime(receipt.decidedAt),
        ]),
      ]),
      h.a(
        [
          h.Href(
            `/api/autopilot/decision-closeouts/${encodeURIComponent(receipt.closeoutRef)}`,
          ),
          Ui.className<Message>(
            'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/45 underline underline-offset-[3px] hover:text-[#ffb400]',
          ),
        ],
        [receipt.closeoutRef],
      ),
    ],
  )
}

const decisionActButton = (
  item: AutopilotDecisionQueueItem,
  action: AutopilotWorkReviewAction,
  label: string,
  variant: Ui.ButtonVariant,
  submitting: boolean,
): Html => {
  const h = html<Message>()

  return Ui.button<Message>({
    attrs: [
      h.Type('button'),
      ...(submitting
        ? [h.Disabled(true)]
        : [
            h.OnClick(
              SubmittedAutopilotDecisionAction({
                action,
                decisionRef: item.decision.id,
              }),
            ),
          ]),
    ],
    label,
    size: 'sm',
    variant,
  })
}

const decisionActions = (model: Model, item: AutopilotDecisionQueueItem): Html => {
  const h = html<Message>()
  const submitting =
    model.autopilotDecisionAct._tag === 'AutopilotDecisionActSubmitting'

  if (item.decision.status === 'blocked') {
    return h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
      h.a(
        [
          h.Href(autopilotWorkDetailRouter({
            workOrderRef: item.work.workOrderRef,
          })),
          Ui.className<Message>(Ui.textLinkClass),
        ],
        ['Open work order'],
      ),
    ])
  }

  if (item.decision.status === 'completed') {
    return h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
      h.a(
        [
          h.Href(autopilotWorkDetailRouter({
            workOrderRef: item.work.workOrderRef,
          })),
          Ui.className<Message>(Ui.textLinkClass),
        ],
        ['View receipt trail'],
      ),
    ])
  }

  return h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
    decisionActButton(item, 'accept', 'Approve', 'primary', submitting),
    decisionActButton(
      item,
      'request_changes',
      'Request changes',
      'secondary',
      submitting,
    ),
    decisionActButton(item, 'reject', 'Reject', 'danger', submitting),
  ])
}

const decisionCard = (model: Model, item: AutopilotDecisionQueueItem): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#010102] p-4',
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(item.decision.actionLabel, statusTone(item.decision.status)),
        badge(item.decision.statusLabel, statusTone(item.decision.status)),
        h.span([Ui.className<Message>('text-xs text-white/35')], [
          `Updated ${item.decision.updatedAtDisplay}`,
        ]),
      ]),
      h.p([Ui.className<Message>('m-0 text-sm/6 text-white/80')], [
        summaryText(item),
      ]),
      h.a(
        [
          h.Href(autopilotWorkDetailRouter({
            workOrderRef: item.work.workOrderRef,
          })),
          Ui.className<Message>(
            'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/45 underline underline-offset-[3px] hover:text-[#ffb400]',
          ),
        ],
        [item.work.workOrderRef],
      ),
      item.decision.blockedReasonRefs.length === 0
        ? empty()
        : h.div(
            [Ui.className<Message>('flex flex-wrap gap-2')],
            refChips(item.decision.blockedReasonRefs),
          ),
      item.decision.receiptRefs.length === 0
        ? empty()
        : h.div(
            [Ui.className<Message>('flex flex-wrap gap-2')],
            refChips(item.decision.receiptRefs),
          ),
      item.closeoutReceipts.length === 0
        ? empty()
        : h.div(
            [Ui.className<Message>('grid gap-2')],
            item.closeoutReceipts.map(closeoutReceiptRow),
          ),
      decisionActions(model, item),
    ],
  )
}

const empty = (): Html =>
  html<Message>().span([Ui.className<Message>('hidden')], [])

const actStatusView = (model: Model): Html =>
  M.value(model.autopilotDecisionAct).pipe(
    M.tags({
      AutopilotDecisionActIdle: () => empty(),
      AutopilotDecisionActSubmitting: ({ action }) =>
        loadingView(`Recording ${action.replaceAll('_', ' ')}...`),
      AutopilotDecisionActSucceeded: () =>
        html<Message>().p(
          [Ui.className<Message>('m-0 text-sm text-[#7ccf8a]')],
          ['Decision recorded.'],
        ),
      AutopilotDecisionActFailed: ({ error }) => errorView(error),
    }),
    M.exhaustive,
  )

const loadedView = (
  model: Model,
  items: ReadonlyArray<AutopilotDecisionQueueItem>,
  generatedAt: string,
  pendingCount: number,
): Html => {
  const h = html<Message>()
  const pending = items.filter(item => item.decision.status !== 'completed')
  const completed = items.filter(item => item.decision.status === 'completed')

  return h.section([Ui.className<Message>('grid gap-4')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h1(
            [Ui.className<Message>('m-0 text-2xl font-semibold text-white')],
            ['Decisions'],
          ),
          h.p([Ui.className<Message>('m-0 text-sm/6 text-white/50')], [
            `${pendingCount} pending - Generated ${formatIsoDateTime(generatedAt)}`,
          ]),
        ]),
        Ui.button<Message>({
          attrs: [
            h.Type('button'),
            h.OnClick(RequestedLoadAutopilotDecisions()),
          ],
          label: 'Refresh',
          size: 'sm',
          variant: 'secondary',
        }),
      ],
    ),
    actStatusView(model),
    pending.length === 0
      ? emptyView()
      : h.div(
          [Ui.className<Message>('grid gap-3')],
          pending.map(item => decisionCard(model, item)),
        ),
    completed.length === 0
      ? empty()
      : h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
          h.h2(
            [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
            ['Recent decisions'],
          ),
          h.div(
            [Ui.className<Message>('grid gap-3')],
            completed.map(item => decisionCard(model, item)),
          ),
        ]),
  ])
}

export const view = (model: Model): Html =>
  M.value(model.autopilotDecisions).pipe(
    M.tags({
      AutopilotDecisionsIdle: () =>
        loadingView('Decisions have not loaded.'),
      AutopilotDecisionsLoading: () => loadingView('Loading decisions...'),
      AutopilotDecisionsFailed: ({ error }) => errorView(error),
      AutopilotDecisionsLoaded: ({ response }) =>
        loadedView(
          model,
          response.decisions,
          response.generatedAt,
          response.pendingCount,
        ),
    }),
    M.exhaustive,
  )
