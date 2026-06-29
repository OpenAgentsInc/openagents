import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  PublicTrainingRunSummary,
  PublicTrainingRunsModel,
} from '../model'

const pageClass = 'min-h-screen bg-black text-[#f1efe8]'
const sectionClass = 'mx-auto w-full max-w-7xl px-4 py-4 sm:px-5 lg:px-6'
const panelClass = 'min-w-0 border border-[#242424] bg-[#050505] p-3 text-left'
const panelTitleClass =
  'm-0 text-[0.72rem] font-semibold uppercase leading-none text-[#f1efe8]'
const panelMetaClass = 'm-0 text-[0.68rem] leading-4 text-white/45'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-[#1d1d1d] py-2'
const rowLabelClass =
  'min-w-0 text-[0.72rem] font-medium leading-4 text-[#f1efe8]'
const rowDetailClass = 'mt-1 text-[0.66rem] leading-4 text-white/42'
const rowValueClass = 'text-right text-[0.72rem] leading-4 tabular-nums'

const numberFormatter = new Intl.NumberFormat('en-US')

const formatNumber = (value: number): string => numberFormatter.format(value)

const trainingRunHref = (runId: string): string =>
  `/training/runs/${encodeURIComponent(runId)}`

const loadedSummaries = (
  model: PublicTrainingRunsModel,
): ReadonlyArray<PublicTrainingRunSummary> =>
  model._tag === 'PublicTrainingRunsLoaded' ? model.response.summaries : []

const selectedSummary = (
  model: PublicTrainingRunsModel,
  runId: string | null,
): PublicTrainingRunSummary | null => {
  const summaries = loadedSummaries(model)

  if (runId === null) {
    return summaries[0] ?? null
  }

  return summaries.find(summary => summary.run.trainingRunRef === runId) ?? null
}

const modelStatus = (model: PublicTrainingRunsModel): string =>
  model._tag === 'PublicTrainingRunsLoaded'
    ? 'Live'
    : model._tag === 'PublicTrainingRunsFailed'
      ? 'Unavailable'
      : model._tag === 'PublicTrainingRunsLoading'
        ? 'Loading'
        : 'Idle'

const modelMeta = (model: PublicTrainingRunsModel): string =>
  model._tag === 'PublicTrainingRunsFailed'
    ? model.error
    : 'Backed by /api/training/runs.'

const statusPill = (label: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        'inline-flex min-h-6 items-center border border-[#2a2a2a] px-2 py-1 text-[0.62rem] uppercase leading-none text-white/58',
      ),
    ],
    [label],
  )
}

const panelHeader = (input: {
  meta?: string
  status?: string
  title: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('mb-3 flex items-start justify-between gap-3')],
    [
      h.div(
        [],
        [
          h.h2([Ui.className<Message>(panelTitleClass)], [input.title]),
          input.meta === undefined
            ? ''
            : h.p([Ui.className<Message>(panelMetaClass)], [input.meta]),
        ],
      ),
      input.status === undefined ? '' : statusPill(input.status),
    ],
  )
}

const metricRow = (
  label: string,
  metric: PublicTrainingRunSummary['metrics']['verifiedWorkCount'],
  suffix = '',
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(rowClass)],
    [
      h.div(
        [],
        [
          h.div([Ui.className<Message>(rowLabelClass)], [label]),
          h.div(
            [Ui.className<Message>(rowDetailClass)],
            [metric.provenanceLabel],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>(rowValueClass)],
        [`${formatNumber(metric.value)}${suffix}`],
      ),
    ],
  )
}

const summaryMetrics = (summary: PublicTrainingRunSummary): Html => {
  const h = html<Message>()
  const metrics = summary.metrics

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'Run Counts',
        meta: 'Every number is labeled with its public source.',
        status: summary.run.state,
      }),
      metricRow('Planned windows', metrics.plannedWindowCount),
      metricRow('Active windows', metrics.activeWindowCount),
      metricRow('Sealed windows', metrics.sealedWindowCount),
      metricRow('Reconciled windows', metrics.reconciledWindowCount),
      metricRow('Assigned contributors', metrics.assignedContributorCount),
      metricRow('Verified work', metrics.verifiedWorkCount),
      metricRow('Rejected work', metrics.rejectedWorkCount),
      metricRow(
        'Provider-confirmed settled payouts',
        metrics.providerConfirmedSettledPayoutSats,
        ' sats',
      ),
      metricRow('Pending payouts counted as paid', metrics.pendingPayoutCount),
      metricRow('Public receipt refs', metrics.receiptRefCount),
    ],
  )
}

const summaryWindows = (summary: PublicTrainingRunSummary): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'Windows',
        meta:
          summary.windows.length === 0
            ? summary.emptyState.reason
            : 'Worker-authoritative training_windows projections.',
        status: `${summary.windows.length} rows`,
      }),
      ...summary.windows.map(window =>
        h.div(
          [Ui.className<Message>(rowClass)],
          [
            h.div(
              [],
              [
                h.div(
                  [Ui.className<Message>(rowLabelClass)],
                  [window.windowRef],
                ),
                h.div(
                  [Ui.className<Message>(rowDetailClass)],
                  [`${window.homeworkKind} · ${window.updatedAtDisplay}`],
                ),
              ],
            ),
            h.div([Ui.className<Message>(rowValueClass)], [window.state]),
          ],
        ),
      ),
    ],
  )
}

const summaryRealGradient = (summary: PublicTrainingRunSummary): Html => {
  const h = html<Message>()
  const status = summary.realGradient
  const finalLoss = status.lossUnderBudget.finalValidationLoss
  const maxLoss = status.lossUnderBudget.maxValidationLoss
  const lossText =
    finalLoss === null || maxLoss === null
      ? 'No public loss result'
      : `${finalLoss.toFixed(3)} / ${maxLoss.toFixed(3)}`

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'A1 Real Gradient Status',
        meta: status.lossUnderBudget.budgetLabel,
        status: status.externalAsk.status,
      }),
      metricRow(
        'Contributor devices',
        {
          provenanceLabel: status.deviceRequirement.provenanceLabel,
          sourceRefs: status.deviceRequirement.sourceRefs,
          value: status.deviceRequirement.observedDistinctContributorDevices,
        },
        ` / ${status.deviceRequirement.requiredDistinctContributorDevices}`,
      ),
      h.div(
        [Ui.className<Message>(rowClass)],
        [
          h.div(
            [],
            [
              h.div(
                [Ui.className<Message>(rowLabelClass)],
                ['Loss under budget'],
              ),
              h.div(
                [Ui.className<Message>(rowDetailClass)],
                [status.lossUnderBudget.provenanceLabel],
              ),
            ],
          ),
          h.div([Ui.className<Message>(rowValueClass)], [lossText]),
        ],
      ),
      h.div(
        [Ui.className<Message>(rowClass)],
        [
          h.div(
            [],
            [
              h.div([Ui.className<Message>(rowLabelClass)], ['Psionic lane']),
              h.div(
                [Ui.className<Message>(rowDetailClass)],
                [status.closeoutRequirement.provenanceLabel],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>(rowValueClass)],
            [status.externalAsk.psionicLaneRef],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap gap-2 pt-2')],
        [
          ...status.externalAsk.blockerRefs.map(ref => statusPill(ref)),
          ...status.scopeBoundaryRefs.map(ref => statusPill(ref)),
        ],
      ),
      status.lossCurve.length === 0
        ? ''
        : h.div(
            [Ui.className<Message>('grid gap-2 pt-2')],
            status.lossCurve
              .slice(-5)
              .map(point =>
                h.div(
                  [Ui.className<Message>(rowClass)],
                  [
                    h.div(
                      [],
                      [
                        h.div(
                          [Ui.className<Message>(rowLabelClass)],
                          [`Step ${formatNumber(point.step)}`],
                        ),
                        h.div(
                          [Ui.className<Message>(rowDetailClass)],
                          [point.provenanceLabel],
                        ),
                      ],
                    ),
                    h.div(
                      [Ui.className<Message>(rowValueClass)],
                      [point.validationLoss.toFixed(3)],
                    ),
                  ],
                ),
              ),
          ),
      status.leaderboardRows.length === 0
        ? ''
        : h.div(
            [Ui.className<Message>('grid gap-2 pt-2')],
            status.leaderboardRows
              .slice(0, 8)
              .map(row =>
                h.div(
                  [Ui.className<Message>(rowClass)],
                  [
                    h.div(
                      [],
                      [
                        h.div(
                          [Ui.className<Message>(rowLabelClass)],
                          [`#${row.rank} ${row.pylonRef}`],
                        ),
                        h.div(
                          [Ui.className<Message>(rowDetailClass)],
                          [row.provenanceLabel],
                        ),
                      ],
                    ),
                    h.div(
                      [Ui.className<Message>(rowValueClass)],
                      [
                        row.bestValidationLoss === null
                          ? 'no loss'
                          : row.bestValidationLoss.toFixed(3),
                      ],
                    ),
                  ],
                ),
              ),
          ),
    ],
  )
}

const summaryReceipts = (summary: PublicTrainingRunSummary): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'Receipts And Boundaries',
        meta: 'Public refs only; no raw payment material or private runner data.',
        status: `${summary.receiptRefs.length} refs`,
      }),
      h.div(
        [Ui.className<Message>('flex flex-wrap gap-2')],
        [
          ...summary.copyBoundaryRefs.map(ref => statusPill(ref)),
          ...summary.receiptRefs.slice(0, 16).map(ref => statusPill(ref)),
        ],
      ),
    ],
  )
}

const leaderboardLinks = (): Html => {
  const h = html<Message>()
  const links = [
    ['All lanes', '/api/training/leaderboards'],
    ['A1 loss', '/api/training/leaderboards/a1_loss'],
    ['A2 throughput', '/api/training/leaderboards/a2_throughput'],
    ['A3 IsoFLOP', '/api/training/leaderboards/a3_isoflop'],
    ['A4 eval delta', '/api/training/leaderboards/a4_eval_delta'],
    ['A5 accuracy', '/api/training/leaderboards/a5_accuracy'],
  ] as const

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'Receipt-Backed Leaderboards',
        meta: 'Only verified closeout-backed rows can rank.',
        status: 'Public',
      }),
      ...links.map(([label, href]) =>
        h.a(
          [
            h.Href(href),
            Ui.className<Message>(
              `${rowClass} block hover:bg-white/[0.03]`,
            ),
          ],
          [
            h.div(
              [],
              [
                h.div([Ui.className<Message>(rowLabelClass)], [label]),
                h.div([Ui.className<Message>(rowDetailClass)], [href]),
              ],
            ),
            h.div([Ui.className<Message>(rowValueClass)], ['JSON']),
          ],
        ),
      ),
    ],
  )
}

const runList = (
  summaries: ReadonlyArray<PublicTrainingRunSummary>,
  selectedRunId: string | null,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        title: 'Public Runs',
        meta:
          summaries.length === 0
            ? 'No Worker-authoritative training runs are recorded yet.'
            : 'Active and recent runs from /api/training/runs.',
        status: `${summaries.length} runs`,
      }),
      ...summaries.map(summary =>
        h.a(
          [
            h.Href(trainingRunHref(summary.run.trainingRunRef)),
            Ui.className<Message>(
              `block ${rowClass} hover:bg-white/[0.03] ${
                selectedRunId === summary.run.trainingRunRef
                  ? 'bg-white/[0.04]'
                  : ''
              }`,
            ),
          ],
          [
            h.div(
              [],
              [
                h.div(
                  [Ui.className<Message>(rowLabelClass)],
                  [summary.run.trainingRunRef],
                ),
                h.div(
                  [Ui.className<Message>(rowDetailClass)],
                  [summary.run.promiseRef],
                ),
              ],
            ),
            h.div([Ui.className<Message>(rowValueClass)], [summary.run.state]),
          ],
        ),
      ),
    ],
  )
}

export const view = (
  model: PublicTrainingRunsModel,
  runId: string | null,
): Html => {
  const h = html<Message>()
  const summaries = loadedSummaries(model)
  const summary = selectedSummary(model, runId)

  return h.div(
    [Ui.className<Message>(pageClass)],
    [
      h.section(
        [Ui.className<Message>(sectionClass)],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-end justify-between gap-3',
              ),
            ],
            [
              h.div(
                [],
                [
                  h.h1(
                    [
                      Ui.className<Message>(
                        'm-0 text-2xl font-semibold leading-tight text-[#f1efe8]',
                      ),
                    ],
                    ['Training Runs'],
                  ),
                  h.p(
                    [Ui.className<Message>(panelMetaClass)],
                    [
                      'Public CS336 run state, verification, and settlement projection.',
                    ],
                  ),
                ],
              ),
              statusPill(modelStatus(model)),
            ],
          ),
          h.p([Ui.className<Message>(panelMetaClass)], [modelMeta(model)]),
        ],
      ),
      h.section(
        [
          Ui.className<Message>(
            'mx-auto grid w-full max-w-7xl grid-cols-1 gap-3 px-4 pb-6 sm:px-5 lg:grid-cols-[20rem_minmax(0,1fr)] lg:px-6',
          ),
        ],
        [
          runList(summaries, summary?.run.trainingRunRef ?? runId),
          summary === null
            ? h.section(
                [Ui.className<Message>(panelClass)],
                [
                  panelHeader({
                    title: 'Run Detail',
                    meta:
                      model._tag === 'PublicTrainingRunsLoading'
                        ? 'Loading Worker-authoritative run data.'
                        : 'No run projection is available for this route.',
                    status: 'Idle',
                  }),
                ],
              )
            : h.div(
                [Ui.className<Message>('grid gap-3')],
                [
                  summaryMetrics(summary),
                  summaryRealGradient(summary),
                  leaderboardLinks(),
                  summaryWindows(summary),
                  summaryReceipts(summary),
                ],
              ),
        ],
      ),
    ],
  )
}
