import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { formatIsoDateTime } from '../../../time-format'
import { autopilotWorkDetailRouter, autopilotWorkRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  Message,
  RequestedLoadAutopilotWorkDetail,
  RequestedLoadAutopilotWorkList,
  SubmittedAutopilotWorkComposer,
  SubmittedAutopilotWorkReview,
  UpdatedAutopilotWorkComposerField,
} from '../message'
import type {
  AutopilotMissionBriefing,
  AutopilotMorningReport,
  AutopilotMorningReportGroup,
  AutopilotWorkEvent,
  AutopilotWorkProjection,
  AutopilotWorkReviewAction,
  AutopilotWorkState,
  AutopilotWorkSummary,
  Model,
} from '../model'

const stateLabel = (state: AutopilotWorkState): string =>
  state.replaceAll('_', ' ')

const stateTone = (
  state: AutopilotWorkState,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  M.value(state).pipe(
    M.when('access_required', () => 'warning' as const),
    M.when('accepted', () => 'positive' as const),
    M.when('accepted_free_slice', () => 'accent' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('delivered', () => 'positive' as const),
    M.when('invalid', () => 'negative' as const),
    M.when('paid_ready', () => 'accent' as const),
    M.when('payment_required', () => 'warning' as const),
    M.when('queued_or_running', () => 'info' as const),
    M.when('rejected', () => 'negative' as const),
    M.when('revision_required', () => 'warning' as const),
    M.when('scheduled', () => 'info' as const),
    M.exhaustive,
  )

const badge = (label: string, tone: ReturnType<typeof stateTone>): Html => {
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

const ageLabel = (iso: string, generatedAt: string): string => {
  const then = Date.parse(iso)
  const now = Date.parse(generatedAt)

  if (!Number.isFinite(then) || !Number.isFinite(now)) {
    return 'Unknown age'
  }

  const minutes = Math.max(0, Math.floor((now - then) / 60_000))

  if (minutes < 60) {
    return `${minutes}m`
  }

  const hours = Math.floor(minutes / 60)

  return hours < 48 ? `${hours}h` : `${Math.floor(hours / 24)}d`
}

const issueRefs = (summary: AutopilotWorkSummary): ReadonlyArray<string> =>
  summary.issueRefs ?? []

const issueText = (refs: ReadonlyArray<string>): string =>
  refs.length === 0
    ? 'No issue ref'
    : refs.map(ref => ref.replace(/^github\.issue\./, '#')).join(', ')

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
        'No work orders',
      ]),
      h.p([Ui.className<Message>('m-0 mt-2 text-sm/6 text-white/50')], [
        'No mission-briefing work orders are visible for this owner yet.',
      ]),
    ],
  )
}

const recordFromUnknown = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : {}

const stringFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value : undefined

const accessRequirementLabels = (
  work: AutopilotWorkProjection,
): ReadonlyArray<string> =>
  work.accessRequirements
    .map(recordFromUnknown)
    .map(record =>
      [
        stringFromUnknown(record.kind),
        stringFromUnknown(record.grantAction),
      ].filter((value): value is string => value !== undefined).join(' / ')
    )
    .filter(label => label !== '')

const placementSummary = (work: AutopilotWorkProjection): string => {
  const placement = recordFromUnknown(work.placementDecision)
  const selected = stringFromUnknown(placement.selectedRunnerKind)
  const fallback = stringFromUnknown(placement.fallbackRunnerKind)

  return selected ?? fallback ?? 'No runner selected'
}

const composerStatusView = (model: Model): Html | null => {
  const h = html<Message>()

  return M.value(model.autopilotWorkComposer).pipe(
    M.tags({
      AutopilotWorkComposerIdle: () => null,
      AutopilotWorkComposerSubmitting: () =>
        h.p([Ui.className<Message>('m-0 text-sm text-white/50')], [
          'Submitting request...',
        ]),
      AutopilotWorkComposerFailed: ({ error }) => errorView(error),
      AutopilotWorkComposerSucceeded: ({ response }) => {
        const access = accessRequirementLabels(response.work)

        return h.div(
          [Ui.className<Message>('grid gap-2 border border-[#222] bg-[#050505] p-3 text-sm text-white/65')],
          [
            h.div([Ui.className<Message>('font-medium text-white/80')], [
              `${response.work.workOrderRef} - ${stateLabel(response.work.state)}`,
            ]),
            h.div([], [`Next: ${response.work.nextAction.state}`]),
            h.div([], [`Runner: ${placementSummary(response.work)}`]),
            access.length === 0
              ? null
              : h.div([], [`Needs: ${access.join(', ')}`]),
          ].filter((node): node is Html => node !== null),
        )
      },
    }),
    M.exhaustive,
  )
}

const composerView = (model: Model): Html => {
  const h = html<Message>()
  const draft = model.autopilotWorkComposerDraft
  const submitting =
    model.autopilotWorkComposer._tag === 'AutopilotWorkComposerSubmitting'

  return h.form(
    [
      Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-5'),
      h.OnSubmit(SubmittedAutopilotWorkComposer()),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['New work order']),
      h.label([Ui.className<Message>('grid gap-2')], [
        h.span([Ui.className<Message>('text-sm font-medium text-white/80')], [
          'Objective',
        ]),
        h.textarea(
          [
            h.Name('objective'),
            h.Value(draft.objective),
            h.Rows(4),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({ field: 'objective', value })
            ),
            Ui.className<Message>(
              'min-h-28 resize-y border border-[#333] bg-[#050505] p-3 text-base/7 text-white/85 outline-none focus:border-white/45 sm:text-sm/6',
            ),
          ],
          [],
        ),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-[minmax(0,1.2fr)_8rem_10rem]')], [
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Repository',
          ]),
          h.input([
            h.Name('repository'),
            h.Value(draft.repositoryFullName),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({
                field: 'repositoryFullName',
                value,
              })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Branch',
          ]),
          h.input([
            h.Name('branch'),
            h.Value(draft.branch),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({ field: 'branch', value })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Budget cents',
          ]),
          h.input([
            h.Name('budget'),
            h.Type('number'),
            h.Value(draft.maxSpendCents),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({
                field: 'maxSpendCents',
                value,
              })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
      ]),
      h.label([Ui.className<Message>('grid gap-2')], [
        h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
          'Verification command',
        ]),
        h.input([
          h.Name('verification'),
          h.Value(draft.verificationCommand),
          h.OnInput(value =>
            UpdatedAutopilotWorkComposerField({
              field: 'verificationCommand',
              value,
            })
          ),
          Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
        ]),
      ]),
      composerStatusView(model),
      Ui.button<Message>({
        attrs: [h.Type('submit'), ...(submitting ? [h.Disabled(true)] : [])],
        label: submitting ? 'Submitting...' : 'Submit work order',
        size: 'sm',
        variant: 'primary',
      }),
    ].filter((node): node is Html => node !== null),
  )
}

const morningReportGroupLabel = (
  group: AutopilotMorningReportGroup,
): string =>
  M.value(group).pipe(
    M.when('awaiting_decision', () => 'Awaiting decision'),
    M.when('blocked', () => 'Blocked'),
    M.when('launched', () => 'Launched'),
    M.when('reviewed', () => 'Reviewed'),
    M.when('running', () => 'Running'),
    M.when('scheduled', () => 'Scheduled'),
    M.exhaustive,
  )

const morningReportGroupTone = (
  group: AutopilotMorningReportGroup,
): ReturnType<typeof stateTone> =>
  M.value(group).pipe(
    M.when('awaiting_decision', () => 'warning' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('launched', () => 'info' as const),
    M.when('reviewed', () => 'positive' as const),
    M.when('running', () => 'info' as const),
    M.when('scheduled', () => 'accent' as const),
    M.exhaustive,
  )

const morningReportItemRow = (
  item: AutopilotMorningReport['workItems'][number],
): Html => {
  const h = html<Message>()
  const href = autopilotWorkDetailRouter({ workOrderRef: item.workOrderRef })

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] px-4 py-3 text-left no-underline last:border-b-0 hover:bg-[#080808] md:grid-cols-[9rem_minmax(0,1.4fr)_10rem] md:items-center',
      ),
    ],
    [
      h.div([], [
        badge(morningReportGroupLabel(item.group), morningReportGroupTone(item.group)),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/75',
          ),
        ],
        [item.workOrderRef],
      ),
      h.div([Ui.className<Message>('text-xs text-white/45 md:text-right')], [
        item.scheduledLaunchAt === null
          ? formatIsoDateTime(item.updatedAt)
          : `Launch ${formatIsoDateTime(item.scheduledLaunchAt)}`,
      ]),
    ],
  )
}

const morningReportContinuationRow = (
  continuation: AutopilotMorningReport['continuations'][number],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] px-4 py-3 last:border-b-0 md:grid-cols-[9rem_minmax(0,1.4fr)_10rem] md:items-center',
      ),
    ],
    [
      h.div([], [
        badge(
          continuation.decision === 'dispatched' ? 'Resumed' : 'Resume failed',
          continuation.decision === 'dispatched' ? 'positive' : 'negative',
        ),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/75',
          ),
        ],
        [`${continuation.runId} - attempt ${continuation.attempt}`],
      ),
      h.div([Ui.className<Message>('text-xs text-white/45 md:text-right')], [
        formatIsoDateTime(continuation.occurredAt),
      ]),
    ],
  )
}

const morningReportPanel = (report: AutopilotMorningReport): Html => {
  const h = html<Message>()
  const rows = [
    ...report.workItems.map(morningReportItemRow),
    ...report.continuations.map(morningReportContinuationRow),
  ]

  return h.section([Ui.className<Message>('grid gap-3')], [
    h.div([Ui.className<Message>('grid gap-1')], [
      h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
        'While you were away',
      ]),
      h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
        `Since ${formatIsoDateTime(report.sinceIso)} - ${report.counts.awaitingDecision} awaiting decision, ${report.counts.blocked} blocked, ${report.counts.scheduled} scheduled, ${report.counts.continuations} resumed`,
      ]),
    ]),
    rows.length === 0
      ? h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          'Nothing ran in this window.',
        ])
      : h.div([Ui.className<Message>('overflow-hidden border border-[#222]')], rows),
  ])
}

const morningReportView = (model: Model): Html =>
  M.value(model.autopilotMorningReport).pipe(
    M.tags({
      AutopilotMorningReportIdle: () =>
        html<Message>().span([Ui.className<Message>('hidden')], []),
      AutopilotMorningReportLoading: () =>
        loadingView('Loading overnight summary...'),
      AutopilotMorningReportFailed: ({ error }) => errorView(error),
      AutopilotMorningReportLoaded: ({ response }) =>
        morningReportPanel(response.report),
    }),
    M.exhaustive,
  )

const workRow = (
  summary: AutopilotWorkSummary,
  generatedAt: string,
): Html => {
  const h = html<Message>()
  const href = autopilotWorkDetailRouter({
    workOrderRef: summary.workOrderRef,
  })

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'grid gap-3 border-b border-[#222] px-4 py-4 text-left no-underline last:border-b-0 hover:bg-[#080808] md:grid-cols-[minmax(0,1.4fr)_9rem_7rem_7rem] md:items-center',
      ),
    ],
    [
      h.div([Ui.className<Message>('min-w-0')], [
        h.div(
          [
            Ui.className<Message>(
              'overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/85',
            ),
          ],
          [summary.workOrderRef],
        ),
        h.div(
          [
            Ui.className<Message>(
              'mt-1 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/35',
            ),
          ],
          [summary.taskRefs?.[0] ?? summary.promiseRef.promiseId],
        ),
      ]),
      h.div([Ui.className<Message>('text-xs text-white/55')], [
        summary.promiseRef.promiseId,
      ]),
      h.div([Ui.className<Message>('text-xs text-white/55')], [
        issueText(issueRefs(summary)),
      ]),
      h.div([Ui.className<Message>('flex items-center gap-2 md:justify-end')], [
        badge(stateLabel(summary.state), stateTone(summary.state)),
        h.span([Ui.className<Message>('text-xs text-white/35')], [
          ageLabel(summary.createdAt, summary.generatedAt ?? generatedAt),
        ]),
      ]),
    ],
  )
}

const listLoadedView = (
  model: Model,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
  generatedAt: string,
): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-4')], [
    composerView(model),
    h.div([Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h1([Ui.className<Message>('m-0 text-2xl font-semibold text-white')], [
          'Autopilot work',
        ]),
        h.p([Ui.className<Message>('m-0 text-sm/6 text-white/50')], [
          `Generated ${formatIsoDateTime(generatedAt)}`,
        ]),
      ]),
      Ui.button<Message>({
        attrs: [
          html<Message>().Type('button'),
          html<Message>().OnClick(RequestedLoadAutopilotWorkList()),
        ],
        label: 'Refresh',
        size: 'sm',
        variant: 'secondary',
      }),
    ]),
    morningReportView(model),
    workOrders.length === 0
      ? emptyView()
      : h.div(
          [Ui.className<Message>('overflow-hidden border border-[#222]')],
          [
            h.div(
              [
                Ui.className<Message>(
                  'hidden border-b border-[#222] px-4 py-2 text-[0.6875rem] uppercase text-white/35 md:grid md:grid-cols-[minmax(0,1.4fr)_9rem_7rem_7rem]',
                ),
              ],
              [
                h.div([], ['Order']),
                h.div([], ['Lane']),
                h.div([], ['Issue']),
                h.div([Ui.className<Message>('text-right')], ['Status']),
              ],
            ),
            ...workOrders.map(workOrder => workRow(workOrder, generatedAt)),
          ],
        ),
  ])
}

export const listView = (model: Model): Html =>
  M.value(model.autopilotWorkList).pipe(
    M.tags({
      AutopilotWorkListIdle: () =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          loadingView('Work orders have not loaded.'),
        ]),
      AutopilotWorkListLoading: () =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          loadingView('Loading work orders...'),
        ]),
      AutopilotWorkListFailed: ({ error }) =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          errorView(error),
        ]),
      AutopilotWorkListLoaded: ({ response }) =>
        listLoadedView(model, response.workOrders, response.generatedAt),
    }),
    M.exhaustive,
  )

const refChips = (refs: ReadonlyArray<string>): ReadonlyArray<Html> => {
  const h = html<Message>()

  return refs.length === 0
    ? [
        h.span([Ui.className<Message>('text-xs text-white/35')], [
          'No refs',
        ]),
      ]
    : refs.map(ref =>
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

const refSection = (title: string, refs: ReadonlyArray<string>): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-2')], [
    h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
      title,
    ]),
    h.div([Ui.className<Message>('flex flex-wrap gap-2')], refChips(refs)),
  ])
}

const eventRow = (event: AutopilotWorkEvent): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[8rem_minmax(0,1fr)]',
      ),
    ],
    [
      h.div([Ui.className<Message>('text-xs text-white/35')], [
        formatIsoDateTime(event.occurredAt),
      ]),
      h.div([Ui.className<Message>('grid gap-1')], [
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(event.eventKind.replaceAll('_', ' '), stateTone(event.state)),
          h.span([Ui.className<Message>('text-xs text-white/35')], [
            `Sequence ${event.sequence}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          ...refChips(event.taskRefs),
        ]),
      ]),
    ],
  )
}

const eventsPanel = (model: Model): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
      'Lifecycle',
    ]),
    M.value(model.autopilotWorkEvents).pipe(
      M.tags({
        AutopilotWorkEventsIdle: () => loadingView('Events have not loaded.'),
        AutopilotWorkEventsLoading: () => loadingView('Loading events...'),
        AutopilotWorkEventsFailed: ({ error }) => errorView(error),
        AutopilotWorkEventsLoaded: ({ response }) =>
          response.events.length === 0
            ? loadingView('No lifecycle events yet.')
            : h.div(
                [Ui.className<Message>('border border-[#222] px-4')],
                response.events.map(eventRow),
              ),
      }),
      M.exhaustive,
    ),
  ])
}

const briefingPanel = (briefing: AutopilotMissionBriefing): Html => {
  const closeoutRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'closeout' ? group.refs : [],
  )
  const assignmentRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'assignment' ? group.refs : [],
  )
  const buildRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'build' ? group.refs : [],
  )
  const testRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'test' ? group.refs : [],
  )

  return html<Message>().section(
    [Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')],
    [
      html<Message>().div([Ui.className<Message>('grid gap-1')], [
        html<Message>().h2(
          [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
          ['Briefing'],
        ),
        html<Message>().p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          `Generated ${formatIsoDateTime(briefing.generatedAt)}`,
        ]),
      ]),
      html<Message>().div(
        [Ui.className<Message>('grid gap-4 lg:grid-cols-2')],
        [
          refSection('Assignments', assignmentRefs),
          refSection('Closeouts', closeoutRefs),
          refSection('Build refs', buildRefs),
          refSection('Test refs', testRefs),
          refSection('Artifacts', briefing.whatChanged.artifactRefs),
          refSection('Results', briefing.whatChanged.resultRefs),
          refSection('Blockers', briefing.whatIsBlocked.blockerRefs),
          refSection('Next action', briefing.decisionsWaiting.callerActionRefs),
        ],
      ),
    ],
  )
}

const briefingStatePanel = (model: Model): Html =>
  M.value(model.autopilotWorkBriefing).pipe(
    M.tags({
      AutopilotWorkBriefingIdle: () => loadingView('Briefing has not loaded.'),
      AutopilotWorkBriefingLoading: () => loadingView('Loading briefing...'),
      AutopilotWorkBriefingFailed: ({ error }) => errorView(error),
      AutopilotWorkBriefingLoaded: ({ response }) =>
        briefingPanel(response.briefing),
    }),
    M.exhaustive,
  )

const reviewButton = (
  work: AutopilotWorkProjection,
  action: AutopilotWorkReviewAction,
  label: string,
  variant: Ui.ButtonVariant,
): Html => {
  const h = html<Message>()
  const disabled = work.state !== 'delivered' || work.reviewDecision !== null

  return Ui.button<Message>({
    attrs: [
      h.Type('button'),
      ...(disabled
        ? [h.Disabled(true)]
        : [
            h.OnClick(
              SubmittedAutopilotWorkReview({
                action,
                workOrderRef: work.workOrderRef,
              }),
            ),
          ]),
    ],
    label,
    size: 'sm',
    variant,
  })
}

const reviewPanel = (model: Model, work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const reviewStatus =
    work.reviewDecision === null
      ? work.state === 'delivered'
        ? 'Waiting for human review'
        : 'Review opens after delivery'
      : `Reviewed: ${work.reviewDecision.action.replaceAll('_', ' ')}`

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-center justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Review',
        ]),
        h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          reviewStatus,
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
        reviewButton(work, 'accept', 'Accept', 'primary'),
        reviewButton(work, 'request_changes', 'Request changes', 'secondary'),
        reviewButton(work, 'reject', 'Reject', 'danger'),
      ]),
    ]),
    M.value(model.autopilotWorkReview).pipe(
      M.tags({
        AutopilotWorkReviewIdle: () => h.span([Ui.className<Message>('hidden')], []),
        AutopilotWorkReviewSubmitting: ({ action }) =>
          loadingView(`Submitting ${action.replaceAll('_', ' ')}...`),
        AutopilotWorkReviewSucceeded: () =>
          h.p([Ui.className<Message>('m-0 text-sm text-[#7ccf8a]')], [
            'Review recorded.',
          ]),
        AutopilotWorkReviewFailed: ({ error }) => errorView(error),
      }),
      M.exhaustive,
    ),
  ])
}

const workSummaryPanel = (model: Model, work: AutopilotWorkProjection): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('min-w-0 grid gap-2')], [
        h.a(
          [
            h.Href(autopilotWorkRouter()),
            Ui.className<Message>(Ui.textLinkClass),
          ],
          ['Autopilot work'],
        ),
        h.h1(
          [
            Ui.className<Message>(
              'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-2xl font-semibold text-white',
            ),
          ],
          [work.workOrderRef],
        ),
        h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          `Generated ${formatIsoDateTime(work.generatedAt)}`,
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(stateLabel(work.state), stateTone(work.state)),
        Ui.button<Message>({
          attrs: [
            h.Type('button'),
            h.OnClick(
              RequestedLoadAutopilotWorkDetail({
                workOrderRef: work.workOrderRef,
              }),
            ),
          ],
          label: 'Refresh',
          size: 'sm',
          variant: 'secondary',
        }),
      ]),
    ]),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Tasks', work.taskRefs),
        refSection('Access requests', work.accessRequestRefs),
        refSection('Next action refs', work.nextAction.callerActionRefs),
        refSection('Reason refs', work.nextAction.reasonRefs),
      ],
    ),
    work.executionCloseout === null
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.div(
          [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
          [
            refSection('Assignment refs', work.executionCloseout.assignmentRefs),
            refSection('Closeout refs', work.executionCloseout.closeoutRefs),
            refSection('Proof refs', work.executionCloseout.proofRefs),
            refSection('Result refs', work.executionCloseout.resultRefs),
            refSection('Artifact refs', work.executionCloseout.artifactRefs ?? []),
            refSection('Build refs', work.executionCloseout.buildRefs ?? []),
            refSection('Test refs', work.executionCloseout.testRefs ?? []),
            refSection('Blocker refs', work.executionCloseout.blockerRefs ?? []),
          ],
        ),
    reviewPanel(model, work),
    eventsPanel(model),
    briefingStatePanel(model),
  ])
}

export const detailView = (model: Model): Html =>
  M.value(model.autopilotWorkDetail).pipe(
    M.tags({
      AutopilotWorkDetailIdle: () => loadingView('Work order has not loaded.'),
      AutopilotWorkDetailLoading: () => loadingView('Loading work order...'),
      AutopilotWorkDetailFailed: ({ error }) => errorView(error),
      AutopilotWorkDetailLoaded: ({ response }) =>
        workSummaryPanel(model, response.work),
    }),
    M.exhaustive,
  )
