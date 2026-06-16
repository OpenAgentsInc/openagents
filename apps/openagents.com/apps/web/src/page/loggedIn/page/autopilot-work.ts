// Forge cockpit — the operator surface for software-factory work.
//
// This view is a reframing of the prior operator UI onto the shared
// `@openagentsinc/ui` library (the `AiElements` family) plus the Forge object
// model from `products/forge.md`:
//
//   - work orders / sessions   -> Runs (one execution attempt against a request)
//   - the provider-account pool -> the compute / routing layer
//   - the review/accept action  -> the accepted-outcome receipt
//   - node placement / runner   -> where the work runs (local / cloud node)
//
// The control logic (messages, model fields, routes, composer + receipt
// actions) is unchanged. Only the rendering and the operator-facing language
// are reframed in Forge terms and rebuilt on `Ui.AiElements`.

import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { formatIsoDateTime } from '../../../time-format'
import { autopilotWorkDetailRouter, autopilotWorkRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  type ForgeContextDirtyState,
  type ForgeContextFreshness,
  type ForgeContextRefGroupInput,
  type ForgeContextSnapshotInput,
  type ForgeContextSnapshotStatus,
  projectForgeContextSnapshot,
} from '../autopilot-work/context-snapshot'
import {
  type ForgeDiffArtifactDrilldownFileGroup,
  type ForgeDiffArtifactDrilldownStatus,
  type ForgeDiffReviewStatus,
  type ForgeDiffReviewView,
  projectForgeDiffArtifactDrilldown,
  projectForgeDiffReview,
} from '../autopilot-work/diff-review'
import {
  type ForgeRunProgressItem,
  type ForgeRunProgressItemStatus,
  type ForgeRunProgressStatus,
  projectForgeRunProgress,
} from '../autopilot-work/progress-view'
import {
  type ForgeRetrievalCandidate,
  type ForgeRetrievalFreshness,
  type ForgeRetrievalPlanInput,
  type ForgeRetrievalPlanStatus,
  type ForgeRetrievalSkippedCandidate,
  projectForgeRetrievalPlan,
} from '../autopilot-work/retrieval-plan'
import {
  type ForgeSessionNavigationAction,
  type ForgeSessionNavigationItem,
  type ForgeSessionNavigationStatus,
  projectForgeSessionNavigation,
} from '../autopilot-work/session-navigation'
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
        'No Runs',
      ]),
      h.p([Ui.className<Message>('m-0 mt-2 text-sm/6 text-white/50')], [
        'No Runs are visible for this owner yet. Submit a Run to put work into the factory.',
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

// The compute / routing decision: which node the Run is placed on. Reframes the
// runner-selection projection as the "where work runs" line.
const placementSummary = (work: AutopilotWorkProjection): string => {
  const placement = recordFromUnknown(work.placementDecision)
  const selected = stringFromUnknown(placement.selectedRunnerKind)
  const fallback = stringFromUnknown(placement.fallbackRunnerKind)

  return selected ?? fallback ?? 'No node selected'
}

const composerStatusView = (model: Model): Html | null => {
  const h = html<Message>()

  return M.value(model.autopilotWorkComposer).pipe(
    M.tags({
      AutopilotWorkComposerIdle: () => null,
      AutopilotWorkComposerSubmitting: () =>
        h.p([Ui.className<Message>('m-0 text-sm text-white/50')], [
          'Submitting Run...',
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
            h.div([], [`Runs on: ${placementSummary(response.work)}`]),
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
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['New Run']),
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
        label: submitting ? 'Submitting...' : 'Submit Run',
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
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Forge cockpit']),
        h.h1([Ui.className<Message>('m-0 text-2xl font-semibold text-white')], [
          'Runs',
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
                h.div([], ['Run']),
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
          loadingView('Runs have not loaded.'),
        ]),
      AutopilotWorkListLoading: () =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          loadingView('Loading Runs...'),
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

// The Run lifecycle, expressed as an `AiElements` task list: each recorded
// runtime event is a step, toned by its Run state.
const eventTaskItem = (
  event: AutopilotWorkEvent,
): Ui.AiElements.TaskItemProps => {
  const tone = stateTone(event.state)
  const status: Ui.AiElements.TaskItemStatus =
    tone === 'positive'
      ? 'done'
      : tone === 'negative'
        ? 'failed'
        : tone === 'info'
          ? 'active'
          : 'queued'

  return {
    label: `${event.eventKind.replaceAll('_', ' ')} — ${formatIsoDateTime(event.occurredAt)} (seq ${event.sequence})`,
    status,
  }
}

const eventsPanel = (model: Model): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    M.value(model.autopilotWorkEvents).pipe(
      M.tags({
        AutopilotWorkEventsIdle: () => loadingView('Lifecycle has not loaded.'),
        AutopilotWorkEventsLoading: () => loadingView('Loading lifecycle...'),
        AutopilotWorkEventsFailed: ({ error }) => errorView(error),
        AutopilotWorkEventsLoaded: ({ response }) =>
          Ui.AiElements.task<Message>({
            props: {
              title: 'Run lifecycle',
              open: true,
              items: response.events.map(eventTaskItem),
            },
          }),
      }),
      M.exhaustive,
    ),
  ])
}

const loadedEvents = (model: Model): ReadonlyArray<AutopilotWorkEvent> | null =>
  model.autopilotWorkEvents._tag === 'AutopilotWorkEventsLoaded'
    ? model.autopilotWorkEvents.response.events
    : null

const progressStatusTone = (
  status: ForgeRunProgressStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'reviewed' || status === 'delivered'
    ? 'positive'
    : status === 'failed'
      ? 'negative'
      : status === 'blocked'
        ? 'warning'
        : status === 'running'
          ? 'info'
          : 'accent'

const progressTaskStatus = (
  status: ForgeRunProgressItemStatus,
): Ui.AiElements.TaskItemStatus =>
  status === 'completed'
    ? 'done'
    : status === 'failed' || status === 'blocked'
      ? 'failed'
      : status === 'active'
        ? 'active'
        : 'queued'

const progressRefPreview = (refs: ReadonlyArray<string>): string =>
  refs.length === 0
    ? ''
    : ` - ${refs.slice(0, 2).join(', ')}${refs.length > 2 ? ` (+${refs.length - 2})` : ''}`

const progressTaskItem = (
  item: ForgeRunProgressItem,
): Ui.AiElements.TaskItemProps => ({
  label: `${item.label}${progressRefPreview(item.refs)}`,
  status: progressTaskStatus(item.status),
})

const progressPanel = (
  model: Model,
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const progress = projectForgeRunProgress(work, loadedEvents(model))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Run progress',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Typed progress projection from Run state, lifecycle events, next action, and closeout evidence.',
        ]),
      ]),
      badge(progress.status.replaceAll('_', ' '), progressStatusTone(progress.status)),
    ]),
    Ui.AiElements.task<Message>({
      props: {
        title: `Progress for ${progress.workOrderRef}`,
        open: true,
        items: progress.items.map(progressTaskItem),
      },
    }),
    refSection('Progress blockers', progress.blockerRefs),
    progress.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${progress.omittedUnsafeRefCount} unsafe progress ref(s) were omitted before rendering.`,
        ]),
  ])
}

const contextSnapshotTone = (
  status: ForgeContextSnapshotStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'accent'

const contextFreshnessTone = (
  freshness: ForgeContextFreshness,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  freshness === 'fresh'
    ? 'positive'
    : freshness === 'stale'
      ? 'warning'
      : 'accent'

const contextDirtyTone = (
  dirtyState: ForgeContextDirtyState,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  dirtyState === 'clean'
    ? 'positive'
    : dirtyState === 'dirty'
      ? 'warning'
      : 'accent'

const contextMetric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-1 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('text-[0.6875rem] uppercase text-white/35')], [
      label,
    ]),
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/75')],
      [value],
    ),
  ])
}

const contextRefGroup = (
  group:
    | (Readonly<{
        blockerRefs?: ReadonlyArray<string>
        refs?: ReadonlyArray<string>
      }>)
    | undefined,
): ForgeContextRefGroupInput | undefined =>
  group === undefined
    ? undefined
    : {
        ...(group.blockerRefs === undefined ? {} : { blockerRefs: group.blockerRefs }),
        ...(group.refs === undefined ? {} : { refs: group.refs }),
      }

const contextSnapshotInput = (
  work: AutopilotWorkProjection,
): ForgeContextSnapshotInput => {
  const source = work.contextSnapshot
  const devDoctor = contextRefGroup(source?.devDoctor)
  const adapters =
    source?.adapters === undefined
      ? undefined
      : {
          ...(source.adapters.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.adapters.blockerRefs }),
          ...(source.adapters.capabilityRefs === undefined
            ? {}
            : { capabilityRefs: source.adapters.capabilityRefs }),
          ...(source.adapters.refs === undefined ? {} : { refs: source.adapters.refs }),
        }
  const currentJob =
    source?.currentJob === undefined
      ? undefined
      : {
          ...(source.currentJob.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.currentJob.blockerRefs }),
          ...(source.currentJob.capabilityRefs === undefined
            ? {}
            : { capabilityRefs: source.currentJob.capabilityRefs }),
          ...(source.currentJob.jobRefs === undefined
            ? {}
            : { jobRefs: source.currentJob.jobRefs }),
          ...(source.currentJob.verificationRefs === undefined
            ? {}
            : { verificationRefs: source.currentJob.verificationRefs }),
        }
  const instructions =
    source?.instructions === undefined
      ? undefined
      : {
          ...(source.instructions.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.instructions.blockerRefs }),
          ...(source.instructions.configRefs === undefined
            ? {}
            : { configRefs: source.instructions.configRefs }),
          ...(source.instructions.refs === undefined
            ? {}
            : { refs: source.instructions.refs }),
        }
  const repo =
    source?.repo === undefined
      ? undefined
      : {
          ...(source.repo.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.repo.blockerRefs }),
          ...(source.repo.changedCount === undefined
            ? {}
            : { changedCount: source.repo.changedCount }),
          ...(source.repo.dirtyState === undefined
            ? {}
            : { dirtyState: source.repo.dirtyState }),
          ...(source.repo.dirtyStateRefs === undefined
            ? {}
            : { dirtyStateRefs: source.repo.dirtyStateRefs }),
          ...(source.repo.identityRefs === undefined
            ? {}
            : { identityRefs: source.repo.identityRefs }),
        }

  return {
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(adapters === undefined ? {} : { adapters }),
    ...(currentJob === undefined ? {} : { currentJob }),
    ...(devDoctor === undefined ? {} : { devDoctor }),
    ...(source?.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source?.freshness === undefined ? {} : { freshness: source.freshness }),
    ...(source?.observedAt === undefined ? {} : { observedAt: source.observedAt }),
    ...(instructions === undefined ? {} : { instructions }),
    ...(repo === undefined ? {} : { repo }),
  }
}

const contextEvidenceCount = (
  context: ReturnType<typeof projectForgeContextSnapshot>,
): number =>
  context.repo.identityRefs.length +
  context.repo.dirtyStateRefs.length +
  context.instructions.instructionRefs.length +
  context.instructions.configRefs.length +
  context.adapters.readinessRefs.length +
  context.adapters.capabilityRefs.length +
  context.devDoctor.doctorRefs.length +
  context.currentJob.jobRefs.length +
  context.currentJob.verificationRefs.length +
  context.currentJob.capabilityRefs.length

const contextSnapshotPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const context = projectForgeContextSnapshot(contextSnapshotInput(work))
  const observedLabel =
    context.observedAt === null
      ? 'No observation time'
      : `Observed ${formatIsoDateTime(context.observedAt)}`

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Context snapshot',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only context readiness for repo identity, instructions, adapters, dev doctor, and current job state.',
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(context.status, contextSnapshotTone(context.status)),
        badge(context.freshness, contextFreshnessTone(context.freshness)),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
      contextMetric('Observed', observedLabel),
      contextMetric('Dirty state', context.repo.dirtyState),
      contextMetric(
        'Changed files',
        context.repo.changedCount === null ? 'unknown' : String(context.repo.changedCount),
      ),
    ]),
    h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
      badge(context.repo.dirtyState, contextDirtyTone(context.repo.dirtyState)),
      badge(`${contextEvidenceCount(context)} context ref(s)`, 'accent'),
    ]),
    contextEvidenceCount(context) === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No context evidence available yet.',
          ]),
        ])
      : h.span([Ui.className<Message>('hidden')], []),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Repo identity', context.repo.identityRefs),
        refSection('Dirty state refs', context.repo.dirtyStateRefs),
        refSection('Instruction refs', context.instructions.instructionRefs),
        refSection('Config refs', context.instructions.configRefs),
        refSection('Adapter readiness', context.adapters.readinessRefs),
        refSection('Adapter capabilities', context.adapters.capabilityRefs),
        refSection('Dev doctor', context.devDoctor.doctorRefs),
        refSection('Current job', context.currentJob.jobRefs),
        refSection('Verification refs', context.currentJob.verificationRefs),
        refSection('Current capabilities', context.currentJob.capabilityRefs),
        refSection('Context blockers', context.blockerRefs),
      ],
    ),
    context.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${context.omittedUnsafeRefCount} unsafe context ref(s) were omitted before rendering.`,
        ]),
  ])
}

const sessionNavigationTone = (
  status: ForgeSessionNavigationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'complete'
    ? 'positive'
    : status === 'attention'
      ? 'warning'
      : status === 'active'
        ? 'info'
        : 'accent'

const sessionItemTone = (
  state: ForgeSessionNavigationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'completed'
    ? 'positive'
    : state === 'failed' || state === 'cancelled'
      ? 'warning'
      : state === 'running'
        ? 'info'
        : 'accent'

const sessionActionLabel = (action: ForgeSessionNavigationAction): string =>
  action.charAt(0).toUpperCase() + action.slice(1)

const sessionActionBlockerRefs = (
  item: ForgeSessionNavigationItem,
): ReadonlyArray<string> =>
  Array.from(
    new Set(
      Object.values(item.actions).flatMap(actionState => actionState.blockerRefs),
    ),
  )

const unavailableSessionAction = (
  item: ForgeSessionNavigationItem,
  action: ForgeSessionNavigationAction,
): Html => {
  const h = html<Message>()
  const actionState = item.actions[action]

  return Ui.button<Message>({
    attrs: [
      h.Type('button'),
      h.Disabled(true),
      h.DataAttribute('forge-session-action', action),
      h.DataAttribute('forge-session-action-availability', actionState.availability),
    ],
    label: sessionActionLabel(action),
    size: 'sm',
    variant: 'secondary',
  })
}

const sessionItemPanel = (item: ForgeSessionNavigationItem): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-session-navigation-item', item.sessionRef),
      h.DataAttribute('forge-session-navigation-source', item.source),
      h.DataAttribute('forge-session-navigation-state', item.state),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [item.title],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            item.observedAt === null
              ? `Source ${item.source} - no observation time`
              : `Source ${item.source} - observed ${formatIsoDateTime(item.observedAt)}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(item.state, sessionItemTone(item.state)),
          badge(item.source, 'accent'),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Session ref', [item.sessionRef]),
        refSection('Artifacts', item.artifactRefs),
        refSection('Events', item.eventRefs),
        refSection('Checkpoints', item.checkpointRefs),
        refSection('Bridge refs', item.bridgeRefs),
        refSection('Control blockers', sessionActionBlockerRefs(item)),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
        unavailableSessionAction(item, 'resume'),
        unavailableSessionAction(item, 'fork'),
        unavailableSessionAction(item, 'rewind'),
        unavailableSessionAction(item, 'cancel'),
      ]),
    ],
  )
}

const sessionNavigationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const source = work.sessionNavigation
  const sessionNavigation = projectForgeSessionNavigation({
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source?.bridgeSessions === undefined
      ? {}
      : { bridgeSessions: source.bridgeSessions }),
    ...(source?.claudeSessions === undefined
      ? {}
      : { claudeSessions: source.claudeSessions }),
    ...(source?.codexSessions === undefined
      ? {}
      : { codexSessions: source.codexSessions }),
    ...(source?.localPylonSessions === undefined
      ? {}
      : { localPylonSessions: source.localPylonSessions }),
  })

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Session navigation',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Read-only session summaries for Pylon, Codex, Claude, and bridge runs. Control verbs stay disabled until runtime authority exists.',
        ]),
      ]),
      badge(
        sessionNavigation.status.replaceAll('_', ' '),
        sessionNavigationTone(sessionNavigation.status),
      ),
    ]),
    sessionNavigation.items.length === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No session summaries available yet.',
          ]),
        ])
      : h.div([Ui.className<Message>('grid gap-3')], [
          ...sessionNavigation.items.map(sessionItemPanel),
        ]),
    refSection('Session blockers', sessionNavigation.blockerRefs),
    sessionNavigation.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${sessionNavigation.omittedUnsafeRefCount} unsafe session ref(s) were omitted before rendering.`,
      ]),
  ])
}

const retrievalStatusTone = (
  status: ForgeRetrievalPlanStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'accent'

const retrievalFreshnessTone = (
  freshness: ForgeRetrievalFreshness,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  freshness === 'fresh'
    ? 'positive'
    : freshness === 'stale'
      ? 'warning'
      : 'accent'

const retrievalSkipTone = (
  reason: ForgeRetrievalSkippedCandidate['reason'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  reason === 'filtered_private' || reason === 'missing_source'
    ? 'warning'
    : reason === 'unsupported_mode'
      ? 'accent'
      : 'info'

const retrievalPlanInput = (
  work: AutopilotWorkProjection,
): ForgeRetrievalPlanInput => {
  const source = work.retrievalPlan

  return {
    generatedAt: source?.generatedAt ?? work.generatedAt,
    mode: source?.mode ?? 'exact',
    planRef: source?.planRef ?? `forge-retrieval-plan:${work.workOrderRef}`,
    requestRef: source?.requestRef ?? work.clientRequestRef,
    ...(source?.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source?.candidates === undefined ? {} : { candidates: source.candidates }),
    ...(source?.freshness === undefined ? {} : { freshness: source.freshness }),
    ...(source?.queryRefs === undefined ? {} : { queryRefs: source.queryRefs }),
    ...(source?.skippedCandidates === undefined
      ? {}
      : { skippedCandidates: source.skippedCandidates }),
    ...(source?.sourceRefs === undefined ? {} : { sourceRefs: source.sourceRefs }),
  }
}

const retrievalMetricValue = (value: number | string | null): string =>
  value === null ? 'missing' : String(value)

const retrievalCandidatePanel = (candidate: ForgeRetrievalCandidate): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-retrieval-candidate', candidate.candidateRef),
      h.DataAttribute('forge-retrieval-candidate-mode', candidate.mode),
      h.DataAttribute('forge-retrieval-candidate-freshness', candidate.freshness),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [candidate.candidateRef],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Rank ${retrievalMetricValue(candidate.rank)} - score ${retrievalMetricValue(candidate.score)}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(candidate.mode, 'accent'),
          badge(candidate.freshness, retrievalFreshnessTone(candidate.freshness)),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Candidate ref', [candidate.candidateRef]),
        refSection(
          'Source ref',
          candidate.sourceRef === null ? [] : [candidate.sourceRef],
        ),
        refSection('Provenance', candidate.provenanceRefs),
        refSection('Candidate blockers', candidate.blockerRefs),
      ]),
    ],
  )
}

const skippedRetrievalCandidatePanel = (
  candidate: ForgeRetrievalSkippedCandidate,
): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-retrieval-skipped-candidate', candidate.candidateRef),
      h.DataAttribute('forge-retrieval-skipped-reason', candidate.reason),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [candidate.candidateRef],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Skipped: ${candidate.reason.replaceAll('_', ' ')}`,
          ]),
        ]),
        badge(candidate.reason.replaceAll('_', ' '), retrievalSkipTone(candidate.reason)),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Skipped candidate', [candidate.candidateRef]),
        refSection(
          'Source ref',
          candidate.sourceRef === null ? [] : [candidate.sourceRef],
        ),
        refSection('Skip blockers', candidate.blockerRefs),
      ]),
    ],
  )
}

const retrievalSearchPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const retrieval = projectForgeRetrievalPlan(retrievalPlanInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Retrieval search',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only retrieval plan evidence. Ranking is projected upstream; the cockpit only renders selected and skipped candidates.',
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(retrieval.status, retrievalStatusTone(retrieval.status)),
        badge(retrieval.mode, 'accent'),
        badge(retrieval.freshness, retrievalFreshnessTone(retrieval.freshness)),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
      contextMetric('Selected', String(retrieval.resultSet.totalSelected)),
      contextMetric('Skipped', String(retrieval.resultSet.totalSkipped)),
      contextMetric('Sources', String(retrieval.resultSet.sourceRefs.length)),
      contextMetric('Plan ref', retrieval.planRef),
    ]),
    refSection('Query refs', retrieval.queryRefs),
    retrieval.candidates.length === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No retrieval candidates selected yet.',
          ]),
        ])
      : h.div([Ui.className<Message>('grid gap-3')], [
          ...retrieval.candidates.map(retrievalCandidatePanel),
        ]),
    retrieval.skippedCandidates.length === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.div([Ui.className<Message>('grid gap-3')], [
          h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
            'Skipped candidates',
          ]),
          ...retrieval.skippedCandidates.map(skippedRetrievalCandidatePanel),
        ]),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Source refs', retrieval.sourceRefs),
        refSection('Result selected refs', retrieval.resultSet.selectedCandidateRefs),
        refSection('Result skipped refs', retrieval.resultSet.skippedCandidateRefs),
        refSection('Retrieval blockers', retrieval.blockerRefs),
      ],
    ),
    retrieval.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${retrieval.omittedUnsafeRefCount} unsafe retrieval ref(s) were omitted before rendering.`,
        ]),
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
          ['Evidence bundle'],
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
      AutopilotWorkBriefingIdle: () =>
        loadingView('Evidence bundle has not loaded.'),
      AutopilotWorkBriefingLoading: () =>
        loadingView('Loading evidence bundle...'),
      AutopilotWorkBriefingFailed: ({ error }) => errorView(error),
      AutopilotWorkBriefingLoaded: ({ response }) =>
        briefingPanel(response.briefing),
    }),
    M.exhaustive,
  )

const loadedBriefing = (model: Model): AutopilotMissionBriefing | null =>
  model.autopilotWorkBriefing._tag === 'AutopilotWorkBriefingLoaded'
    ? model.autopilotWorkBriefing.response.briefing
    : null

const reviewStatusLabel = (status: ForgeDiffReviewStatus): string =>
  status.replaceAll('_', ' ')

const reviewStatusTone = (
  status: ForgeDiffReviewStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'review_ready'
    ? 'positive'
    : status === 'pending_delivery'
      ? 'info'
      : 'warning'

const diffArtifactDrilldownTone = (
  status: ForgeDiffArtifactDrilldownStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready' ? 'positive' : status === 'stale' ? 'warning' : 'negative'

const reviewValue = (value: number | string | null): string =>
  value === null ? 'missing' : String(value)

const lineDeltaLabel = (review: ForgeDiffReviewView): string => {
  const added = review.addedLineCount === null ? '?' : `+${review.addedLineCount}`
  const removed =
    review.removedLineCount === null ? '?' : `-${review.removedLineCount}`

  return `${added} / ${removed}`
}

const reviewMetric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-1 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('text-[0.6875rem] uppercase text-white/35')], [
      label,
    ]),
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/75')],
      [value],
    ),
  ])
}

const diffArtifactDrilldownGroupPanel = (
  group: ForgeDiffArtifactDrilldownFileGroup,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium text-white/70')],
      [group.groupRef],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('File refs', group.fileRefs),
      refSection('Artifact refs', group.artifactRefs),
      refSection('Hunk summaries', group.hunkSummaryRefs),
      refSection('Summaries', group.summaryRefs),
    ]),
  ])
}

const diffReviewPanel = (
  model: Model,
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const briefing = loadedBriefing(model)
  const review = projectForgeDiffReview(work, briefing)
  const drilldown = projectForgeDiffArtifactDrilldown(work, briefing)
  const drilldownId = `diff-artifact-drilldown-${work.workOrderRef}`

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Review changes',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only delivery review. Raw patches and private local material stay out of the cockpit projection.',
        ]),
      ]),
      badge(reviewStatusLabel(review.status), reviewStatusTone(review.status)),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
      reviewMetric('Files', reviewValue(review.fileCount)),
      reviewMetric('Lines', lineDeltaLabel(review)),
      reviewMetric('Patch digest', reviewValue(review.patchDigestRef)),
      reviewMetric('Verification', review.verificationState),
    ]),
    review.artifactRefs.length === 0
      ? h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          'No safe diff artifact ref is available for drilldown yet.',
        ])
      : h.a(
          [
            h.Href(`#${drilldownId}`),
            Ui.className<Message>(
              'w-fit border border-[#333] px-3 py-2 text-sm font-medium text-white/70 hover:border-white/45 hover:text-white',
            ),
          ],
          ['Open diff artifact drilldown'],
        ),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Change captures', review.changeCaptureRefs),
        refSection('Delivery readiness', review.deliveryReadinessRefs),
        refSection('Verification refs', review.verificationRefs),
        refSection('Writeback authority', review.authorityReceiptRefs),
        refSection('Artifacts', review.artifactRefs),
        refSection('Results', review.resultRefs),
        refSection('Caveats', review.reviewCaveatRefs),
        refSection('Blockers', review.blockerRefs),
      ],
    ),
    review.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${review.omittedUnsafeRefCount} unsafe review ref(s) were omitted before rendering.`,
        ]),
    h.div(
      [
        h.Id(drilldownId),
        Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4'),
      ],
      [
        h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
          h.div([Ui.className<Message>('grid gap-1')], [
            h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/80')], [
              'Diff artifact drilldown',
            ]),
            h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
              'Bounded artifact evidence only. This panel does not fetch raw patches or grant accept, writeback, deploy, or settlement authority.',
            ]),
          ]),
          badge(
            drilldown.status.replaceAll('_', ' '),
            diffArtifactDrilldownTone(drilldown.status),
          ),
        ]),
        h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
          reviewMetric('Patch digest', reviewValue(drilldown.patchDigestRef)),
          reviewMetric('Artifacts', String(drilldown.artifactRefs.length)),
          reviewMetric('File groups', String(drilldown.fileGroups.length)),
          reviewMetric('Hunk summaries', String(drilldown.hunkSummaryRefs.length)),
        ]),
        h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
          refSection('Drilldown artifacts', drilldown.artifactRefs),
          refSection('Change captures', drilldown.changeCaptureRefs),
          refSection('Delivery readiness', drilldown.deliveryReadinessRefs),
          refSection('Verification refs', drilldown.verificationRefs),
          refSection('Caveats', drilldown.caveatRefs),
          refSection('Drilldown blockers', drilldown.blockerRefs),
        ]),
        drilldown.fileGroups.length === 0
          ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
              h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
                'No file-group refs available for this artifact yet.',
              ]),
            ])
          : h.div([Ui.className<Message>('grid gap-3')], [
              ...drilldown.fileGroups.map(diffArtifactDrilldownGroupPanel),
            ]),
        drilldown.omittedUnsafeRefCount === 0
          ? h.span([Ui.className<Message>('hidden')], [])
          : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
              `${drilldown.omittedUnsafeRefCount} unsafe diff artifact ref(s) were omitted before rendering.`,
            ]),
      ],
    ),
  ])
}

// The accepted-outcome receipt: an approval gate over a delivered Run. Reframes
// the review action onto the `AiElements` confirmation primitive while keeping
// the existing review messages and disable rules.
const receiptAction = (
  work: AutopilotWorkProjection,
  action: AutopilotWorkReviewAction,
  label: string,
  variant: 'primary' | 'secondary' | 'danger',
): Html => {
  const h = html<Message>()
  const disabled = work.state !== 'delivered' || work.reviewDecision !== null

  return Ui.AiElements.confirmationAction<Message>({
    label,
    variant,
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
  })
}

const receiptState = (
  work: AutopilotWorkProjection,
): Ui.AiElements.ConfirmationState => {
  if (work.reviewDecision === null) {
    return 'requested'
  }

  return work.reviewDecision.action === 'accept' ? 'approved' : 'rejected'
}

const receiptDetail = (work: AutopilotWorkProjection): string =>
  work.reviewDecision === null
    ? work.state === 'delivered'
      ? 'Run delivered. Accept the outcome to record a receipt, or send it back.'
      : 'The receipt opens once the Run is delivered.'
    : `Recorded: ${work.reviewDecision.action.replaceAll('_', ' ')}`

const receiptPanel = (model: Model, work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const state = receiptState(work)

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
      'Accepted-outcome receipt',
    ]),
    Ui.AiElements.confirmation<Message>({
      props: {
        title: `Outcome for ${work.workOrderRef}`,
        state,
        detail: receiptDetail(work),
      },
      actions: [
        receiptAction(work, 'accept', 'Accept', 'primary'),
        receiptAction(work, 'request_changes', 'Request changes', 'secondary'),
        receiptAction(work, 'reject', 'Reject', 'danger'),
      ],
    }),
    M.value(model.autopilotWorkReview).pipe(
      M.tags({
        AutopilotWorkReviewIdle: () => h.span([Ui.className<Message>('hidden')], []),
        AutopilotWorkReviewSubmitting: ({ action }) =>
          loadingView(`Submitting ${action.replaceAll('_', ' ')}...`),
        AutopilotWorkReviewSucceeded: () =>
          h.p([Ui.className<Message>('m-0 text-sm text-[#7ccf8a]')], [
            'Receipt recorded.',
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
          ['Forge cockpit'],
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
          `Runs on ${placementSummary(work)} - generated ${formatIsoDateTime(work.generatedAt)}`,
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
    progressPanel(model, work),
    contextSnapshotPanel(work),
    sessionNavigationPanel(work),
    retrievalSearchPanel(work),
    diffReviewPanel(model, work),
    receiptPanel(model, work),
    eventsPanel(model),
    briefingStatePanel(model),
  ])
}

export const detailView = (model: Model): Html =>
  M.value(model.autopilotWorkDetail).pipe(
    M.tags({
      AutopilotWorkDetailIdle: () => loadingView('Run has not loaded.'),
      AutopilotWorkDetailLoading: () => loadingView('Loading Run...'),
      AutopilotWorkDetailFailed: ({ error }) => errorView(error),
      AutopilotWorkDetailLoaded: ({ response }) =>
        workSummaryPanel(model, response.work),
    }),
    M.exhaustive,
  )
