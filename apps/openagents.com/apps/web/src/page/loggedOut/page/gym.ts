import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'
import { trainingRunView } from '@openagentsinc/three-effect/foldkit'

import * as Ui from '../../../ui'
import {
  ClickedRunGymFixture,
  ToggledGymCoordinator,
  ToggledGymLane,
  ToggledGymSequenceShape,
  UpdatedGymConcurrency,
  UpdatedGymFanoutMode,
  UpdatedGymMaxTokens,
  UpdatedGymModuleComposition,
  UpdatedGymReasoningEffort,
  UpdatedGymSamplesPerCell,
  UpdatedGymTemperature,
  UpdatedGymToolSet,
  UpdatedGymTransport,
  type Message,
} from '../message'
import {
  type GymFanoutMode,
  type GymModuleCompositionMode,
  type GymModel,
  type PublicGymMetricSummary,
  type GymReasoningEffort,
  type GymToolSetRef,
  type GymTransport,
  coordinatorOptions,
  laneOptions,
  sequenceShapeOptions,
} from '../gym/flow'
import {
  TERMINAL_BENCH_VISUAL_REPLAY,
  formatTerminalBenchMetric,
  formatTerminalBenchPercent,
  terminalBenchLaneRate,
  terminalBenchReplayTotals,
  terminalBenchVisualizationOptions,
  type TerminalBenchRunLane,
} from '../gym/terminalBenchReplay'

type InputAttr = Parameters<ReturnType<typeof html<Message>>['input']>[0][number]

const pageClass =
  'mx-auto grid min-w-0 w-full max-w-7xl gap-6 px-4 py-8 font-mono text-white sm:px-6 lg:px-8'
const panelClass =
  'grid min-w-0 gap-4 border border-white/10 bg-[#050505] p-4'
const sectionTitleClass =
  'm-0 font-mono text-[0.78rem] font-semibold uppercase tracking-wide text-white/55'
const labelClass = 'grid gap-1.5 text-[0.78rem] font-medium text-white/70'
const inputClass =
  'min-h-9 border border-white/15 bg-black px-2.5 text-[0.875rem] text-white outline-none transition focus:border-[#7fb0ff] disabled:cursor-not-allowed disabled:text-white/45'
const checkboxClass =
  'size-4 border border-white/25 bg-black text-[#7fb0ff] accent-[#7fb0ff]'
const optionRowClass =
  'flex min-h-9 items-center gap-2 border border-white/10 bg-white/[0.025] px-3 py-2 text-[0.8125rem] text-white/75'

const includes = <A,>(items: ReadonlyArray<A>, value: A): boolean =>
  items.includes(value)

const option = <A extends string>(
  value: A,
  label: string,
  selected: A,
): Html => {
  const h = html<Message>()

  return h.option(
    [h.Value(value), ...(value === selected ? [h.Selected(true)] : [])],
    [label],
  )
}

const stat = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 border border-white/10 bg-black p-3')],
    [
      h.span([Ui.className<Message>('text-[0.72rem] text-white/45')], [label]),
      h.span(
        [Ui.className<Message>('text-[1rem] font-semibold text-white')],
        [value],
      ),
    ],
  )
}

const formatMetric = (value: number, suffix: string): string =>
  `${Number.isInteger(value) ? String(value) : value.toFixed(1)}${suffix}`

const formatMsat = (value: number | null): string =>
  value === null ? 'not measured' : `${value.toLocaleString('en-US')} msat`

const latencyMetric = (metric: PublicGymMetricSummary): Html => {
  const h = html<Message>()
  const suffix = metric.label === 'Perceived TPS' ? ' t/s' : ' ms'
  const compactMetric = (label: string, value: number): Html =>
    h.div([Ui.className<Message>('grid gap-0.5')], [
      h.span([Ui.className<Message>('text-white/40')], [label]),
      h.span([Ui.className<Message>('font-semibold text-white/85')], [
        formatMetric(value, suffix),
      ]),
    ])

  return h.div(
    [Ui.className<Message>('grid gap-2 border border-white/10 bg-black p-3')],
    [
      h.div([Ui.className<Message>('flex items-center justify-between gap-2')], [
        h.span([Ui.className<Message>('text-[0.78rem] font-semibold text-white')], [
          metric.label,
        ]),
        h.span([Ui.className<Message>('text-[0.7rem] text-white/45')], [
          `${metric.measuredSampleCount} measured`,
        ]),
      ]),
      h.div([Ui.className<Message>('grid grid-cols-4 gap-2 text-[0.72rem]')], [
        compactMetric('P50', metric.p50),
        compactMetric('P90', metric.p90),
        compactMetric('P99', metric.p99),
        compactMetric('Mean', metric.mean),
      ]),
      h.p([Ui.className<Message>('m-0 text-[0.72rem] text-white/45')], [
        `${metric.notMeasuredDropped} not measured samples dropped`,
      ]),
    ],
  )
}

const checkboxRow = (
  checked: boolean,
  label: string,
  attrs: ReadonlyArray<InputAttr>,
): Html => {
  const h = html<Message>()

  return h.label([Ui.className<Message>(optionRowClass)], [
    h.input([
      h.Type('checkbox'),
      h.Checked(checked),
      Ui.className<Message>(checkboxClass),
      ...attrs,
    ]),
    h.span([], [label]),
  ])
}

const terminalBenchStateClass = (state: TerminalBenchRunLane['state']): string =>
  state === 'accepted'
    ? 'border-[#6abf69]/35 bg-[#051b0a] text-[#9bf59b]'
    : state === 'failing'
      ? 'border-[#ff7b7b]/35 bg-[#1c0707] text-[#ffb3b3]'
      : 'border-[#ffcf6a]/35 bg-[#211a05] text-[#ffe0a3]'

const terminalBenchStateLabel = (
  state: TerminalBenchRunLane['state'],
): string =>
  state === 'accepted'
    ? 'accepted lane'
    : state === 'failing'
      ? 'failing lane'
      : 'not started lane'

const terminalBenchMetricStrip = (): Html => {
  const h = html<Message>()
  const replay = TERMINAL_BENCH_VISUAL_REPLAY
  const totals = terminalBenchReplayTotals(replay)

  return h.div([Ui.className<Message>('grid gap-3 sm:grid-cols-4')], [
    stat('Task set', `${replay.officialTotalTasks} official`),
    stat('Accepted tasks', String(totals.acceptedTasks)),
    stat('Failing tasks', String(totals.failingTasks)),
    stat('Not started', String(totals.notStartedTasks)),
  ])
}

const terminalBenchLaneRow = (lane: TerminalBenchRunLane): Html => {
  const h = html<Message>()

  return h.tr([Ui.className<Message>('border-t border-white/10')], [
    h.td(
      [Ui.className<Message>('py-3 pr-4 align-top text-base text-white sm:text-sm')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.span([Ui.className<Message>('font-semibold text-white')], [
            lane.label,
          ]),
          h.span([Ui.className<Message>('text-base text-white/45 sm:text-sm')], [
            lane.profileRef,
          ]),
        ]),
      ],
    ),
    h.td(
      [Ui.className<Message>('px-4 py-3 align-top text-base text-white/70 sm:text-sm')],
      [
        `${lane.acceptedTasks} accepted / ${lane.failingTasks} failing / ${lane.notStartedTasks} not started`,
      ],
    ),
    h.td(
      [Ui.className<Message>('px-4 py-3 align-top text-base text-white/70 sm:text-sm')],
      [formatTerminalBenchPercent(terminalBenchLaneRate(lane))],
    ),
    h.td(
      [Ui.className<Message>('px-4 py-3 align-top text-base text-white/70 sm:text-sm')],
      [
        `${formatTerminalBenchMetric(lane.ttftMs, ' ms')} TTFT; ${formatTerminalBenchMetric(
          lane.perceivedTps,
          ' t/s',
        )}`,
      ],
    ),
    h.td(
      [Ui.className<Message>('px-4 py-3 align-top text-base text-white/70 sm:text-sm')],
      [
        lane.distinctVerifierDevice
          ? 'distinct-device verifier'
          : 'verifier placement blocked',
      ],
    ),
    h.td(
      [Ui.className<Message>('py-3 pl-4 align-top text-base text-white/70 sm:text-sm')],
      [
        h.span(
          [
            h.DataAttribute('gym-terminal-bench-lane-state', lane.state),
            Ui.className<Message>(
              `inline-flex border px-2 py-1 text-[0.75rem] font-medium ${terminalBenchStateClass(
                lane.state,
              )}`,
            ),
          ],
          [terminalBenchStateLabel(lane.state)],
        ),
      ],
    ),
  ])
}

const terminalBenchMirror = (): Html => {
  const h = html<Message>()
  const replay = TERMINAL_BENCH_VISUAL_REPLAY

  return h.section(
    [
      h.DataAttribute('gym-terminal-bench-accessible-mirror', ''),
      h.AriaLabel('Terminal-Bench run mirror'),
      Ui.className<Message>('grid min-w-0 gap-4'),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2 sm:grid-cols-[1fr_auto] sm:items-end')], [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.p([Ui.className<Message>(sectionTitleClass)], [
            'Accessible run mirror',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/60 sm:text-sm')], [
            `${replay.externalClaim.label} is displayed as an external target, not an OpenAgents result. This replay is public-safe fixture data; raw task prompts, completions, and private Harbor artifacts are not included.`,
          ]),
        ]),
        h.span(
          [
            h.DataAttribute('gym-terminal-bench-decision-grade', 'false'),
            Ui.className<Message>(
              'w-fit border border-[#ffcf6a]/35 bg-[#211a05] px-2 py-1 text-[0.75rem] text-[#ffe0a3]',
            ),
          ],
          ['decisionGrade: false'],
        ),
      ]),
      h.div([Ui.className<Message>('-mx-4 -my-2 max-w-[calc(100%+2rem)] min-w-0 overflow-x-auto whitespace-nowrap sm:-mx-4')], [
        h.div([Ui.className<Message>('inline-block min-w-full px-4 py-2 align-middle')], [
          h.table([Ui.className<Message>('w-full border-collapse')], [
            h.thead([], [
              h.tr([Ui.className<Message>('text-left text-[0.75rem] text-white/45')], [
                h.th([Ui.className<Message>('whitespace-nowrap py-2 pr-4 font-medium')], [
                  'Profile lane',
                ]),
                h.th([Ui.className<Message>('whitespace-nowrap px-4 py-2 font-medium')], [
                  'Tasks',
                ]),
                h.th([Ui.className<Message>('whitespace-nowrap px-4 py-2 font-medium')], [
                  'Solve rate',
                ]),
                h.th([Ui.className<Message>('whitespace-nowrap px-4 py-2 font-medium')], [
                  'Latency / throughput',
                ]),
                h.th([Ui.className<Message>('whitespace-nowrap px-4 py-2 font-medium')], [
                  'Verifier placement',
                ]),
                h.th([Ui.className<Message>('whitespace-nowrap py-2 pl-4 font-medium')], [
                  'State',
                ]),
              ]),
            ]),
            h.tbody([], replay.lanes.map(terminalBenchLaneRow)),
          ]),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-[1fr_1fr]')], [
        h.div([Ui.className<Message>('grid gap-2 border-t border-white/10 pt-3')], [
          h.p([Ui.className<Message>(sectionTitleClass)], ['Caveats']),
          h.ul(
            [
              h.Role('list'),
              Ui.className<Message>('grid gap-1 text-base text-white/55 sm:text-sm'),
            ],
            replay.caveatRefs.map(ref => h.li([], [ref])),
          ),
        ]),
        h.div([Ui.className<Message>('grid gap-2 border-t border-white/10 pt-3')], [
          h.p([Ui.className<Message>(sectionTitleClass)], ['Blocked before claims']),
          h.ul(
            [
              h.Role('list'),
              Ui.className<Message>('grid gap-1 text-base text-white/55 sm:text-sm'),
            ],
            replay.blockerRefs.map(ref => h.li([], [ref])),
          ),
        ]),
      ]),
    ],
  )
}

const terminalBenchScenePanel = (): Html => {
  const h = html<Message>()
  const replay = TERMINAL_BENCH_VISUAL_REPLAY
  const totals = terminalBenchReplayTotals(replay)

  return h.section(
    [
      h.DataAttribute('gym-terminal-bench-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div([Ui.className<Message>('grid gap-3 lg:grid-cols-[1fr_auto] lg:items-end')], [
        h.div([Ui.className<Message>('grid gap-2')], [
          h.p([Ui.className<Message>(sectionTitleClass)], [
            'Terminal-Bench Gym replay',
          ]),
          h.h2(
            [Ui.className<Message>('m-0 max-w-[18ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-4xl')],
            ['Terminal-Bench 2.0 run field'],
          ),
          h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/65 sm:text-sm')], [
            'A three-effect field for profile lanes, verifier placement, accepted/failing/not-started tasks, cost basis, latency, throughput, and claim caveats. Full Autopilot Verse integration is deferred until this web surface is solid.',
          ]),
        ]),
        h.div(
          [
            h.DataAttribute('gym-terminal-bench-cost-meter', ''),
            Ui.className<Message>(
              'grid gap-1 border border-[#7cf0ff]/30 bg-[#061620] px-3 py-2 text-base text-[#bdf6ff] sm:text-sm',
            ),
          ],
          [
            h.span([Ui.className<Message>('text-white/45')], ['Cost basis']),
            h.span([Ui.className<Message>('font-semibold tabular-nums')], [
              formatMsat(totals.totalCostBasisMsat),
            ]),
          ],
        ),
      ]),
      terminalBenchMetricStrip(),
      h.div(
        [
          h.DataAttribute('gym-terminal-bench-scene', ''),
          Ui.className<Message>(
            'min-h-[420px] min-w-0 max-w-full overflow-hidden border border-[#3a7bff]/25 bg-[#020409]',
          ),
        ],
        [
          trainingRunView<Message>(
            [
              h.DataAttribute('three-effect-scene', 'gym-terminal-bench-run'),
              h.AriaLabel('Terminal-Bench Gym three-effect run field'),
              Ui.className<Message>('block min-h-[420px] min-w-0 max-w-full'),
            ],
            terminalBenchVisualizationOptions(replay),
          ),
        ],
      ),
      terminalBenchMirror(),
    ],
  )
}

const laneControl = (model: GymModel): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-2')], [
    h.p([Ui.className<Message>(sectionTitleClass)], ['Provider fan-out']),
    h.div([Ui.className<Message>('grid gap-2 sm:grid-cols-3')], [
      ...laneOptions.map(lane =>
        checkboxRow(
          includes(model.experiment.fanout.lanes, lane.value),
          lane.label,
          [h.OnInput(() => ToggledGymLane({ lane: lane.value }))],
        ),
      ),
    ]),
  ])
}

const coordinatorControl = (model: GymModel): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-2')], [
    h.p([Ui.className<Message>(sectionTitleClass)], [
      'Coordinator candidates',
    ]),
    h.div([Ui.className<Message>('grid gap-2 sm:grid-cols-3')], [
      ...coordinatorOptions.map(candidate =>
        checkboxRow(
          includes(model.experiment.coordinators, candidate.value),
          candidate.label,
          [
            h.OnInput(() =>
              ToggledGymCoordinator({ candidate: candidate.value }),
            ),
          ],
        ),
      ),
    ]),
  ])
}

const shapeControl = (model: GymModel): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-2')], [
    h.p([Ui.className<Message>(sectionTitleClass)], ['Sequence shapes']),
    h.div([Ui.className<Message>('grid gap-2 sm:grid-cols-3')], [
      ...sequenceShapeOptions.map(shape =>
        checkboxRow(
          includes(model.experiment.shapes, shape.value),
          shape.label,
          [
            h.OnInput(() =>
              ToggledGymSequenceShape({ shape: shape.value }),
            ),
          ],
        ),
      ),
    ]),
  ])
}

const resultPanel = (model: GymModel): Html => {
  const h = html<Message>()
  const result = model.result

  if (result === null) {
    return h.div(
      [
        h.DataAttribute('gym-result-empty', ''),
        Ui.className<Message>(panelClass),
      ],
      [
        h.p([Ui.className<Message>(sectionTitleClass)], ['Fixture scene']),
        h.p(
          [Ui.className<Message>('m-0 max-w-[58ch] text-sm text-white/60')],
          [
            'Run the fixture to prepare the deterministic scene and typed public report payload.',
          ],
        ),
      ],
    )
  }

  return h.div(
    [h.DataAttribute('gym-result', ''), Ui.className<Message>(panelClass)],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        h.p([Ui.className<Message>(sectionTitleClass)], ['Fixture result']),
        h.span(
          [
            h.DataAttribute('gym-report-viewer-input', result.viewerSchema),
            Ui.className<Message>(
              'border border-[#6abf69]/30 bg-[#08200b] px-2 py-1 text-[0.72rem] text-[#9bf59b]',
            ),
          ],
          [result.viewerSchema],
        ),
      ]),
      h.div(
        [
          h.DataAttribute('gym-illustrative-notice', ''),
          Ui.className<Message>(
            'border border-[#ffcf6a]/30 bg-[#211a05] px-3 py-2 text-[0.78rem] font-medium text-[#ffe0a3]',
          ),
        ],
        [
          `decisionGrade: false - ${result.reportViewer.illustrativeNotice}`,
        ],
      ),
      h.div([Ui.className<Message>('grid gap-3 sm:grid-cols-4')], [
        stat('Cells', String(result.expectedCellCount)),
        stat('Executed', String(result.executedCellCount)),
        stat('Skipped', String(result.skippedCellCount)),
        stat('Billed cost', `$${result.metrics.meanCostUsd.toFixed(2)}`),
      ]),
      h.section([Ui.className<Message>('grid gap-3')], [
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          h.p([Ui.className<Message>(sectionTitleClass)], [
            'Fixture lane mirror',
          ]),
          h.span(
            [
              h.DataAttribute('gym-scene-cost-meter', ''),
              Ui.className<Message>(
                'border border-[#7cf0ff]/30 bg-[#061620] px-2 py-1 text-[0.72rem] text-[#bdf6ff]',
              ),
            ],
            [
              `${result.scene.simulatedCostMsat} msat simulated meter / ${result.scene.billedCostMsat} msat billed`,
            ],
          ),
        ]),
        h.div([Ui.className<Message>('grid gap-2 sm:grid-cols-3')], [
          ...result.scene.lanes.map(lane =>
            h.div(
              [
                h.DataAttribute('gym-scene-lane', lane.lane),
                Ui.className<Message>(
                  'grid gap-1 border border-white/10 bg-black p-3 text-[0.78rem]',
                ),
              ],
              [
                h.span([Ui.className<Message>('font-semibold text-white')], [
                  lane.label,
                ]),
                h.span(
                  [
                    h.DataAttribute('gym-scene-lane-status', lane.status),
                    Ui.className<Message>(
                      lane.status === 'test_passed'
                        ? 'text-[#9bf59b]'
                        : 'text-white/45',
                    ),
                  ],
                  [
                    lane.status === 'test_passed'
                      ? 'test_passed verdict beam'
                      : 'skipped_unavailable arc',
                  ],
                ),
              ],
            ),
          ),
        ]),
      ]),
      h.section([Ui.className<Message>('grid gap-3')], [
        h.p([Ui.className<Message>(sectionTitleClass)], ['Report viewer']),
        h.div(
          [
            h.DataAttribute('gym-report-viewer', ''),
            Ui.className<Message>('grid gap-3 lg:grid-cols-2'),
          ],
          result.reportViewer.latency.map(latencyMetric),
        ),
        h.div([Ui.className<Message>('grid gap-3 sm:grid-cols-3')], [
          stat(
            'Verification rate',
            `${Math.round(result.reportViewer.verificationRate * 100)}%`,
          ),
          stat(
            'Cache hit rate',
            `${Math.round(result.reportViewer.cacheHitRate * 100)}%`,
          ),
          stat(
            'Cost per accepted outcome',
            result.reportViewer.costPerAcceptedOutcomeUsd === null
              ? 'null'
              : `$${result.reportViewer.costPerAcceptedOutcomeUsd.toFixed(4)}`,
          ),
        ]),
        h.div(
          [
            h.DataAttribute('gym-null-cost-finding', ''),
            Ui.className<Message>(
              'border border-white/10 bg-black px-3 py-2 text-sm text-white/65',
            ),
          ],
          [
            `${result.reportViewer.nullCostFinding} Zero-accepted edge: ${result.reportViewer.zeroAcceptedFinding.finding}.`,
          ],
        ),
      ]),
      h.p([Ui.className<Message>('m-0 text-sm text-white/60')], [
        `Public safety ${result.publicSafety}; ${Math.round(
          result.metrics.acceptedOutcomeRate * 100,
        )}% accepted outcome rate; fixture report ${result.reportRef}.`,
      ]),
    ],
  )
}

export const view = (model: GymModel): Html => {
  const h = html<Message>()

  return h.main(
    [
      h.DataAttribute('route', 'gym'),
      h.DataAttribute('gym-page', ''),
      Ui.className<Message>('min-h-dvh bg-black'),
    ],
    [
      h.div([Ui.className<Message>(pageClass)], [
        h.header([Ui.className<Message>('grid gap-3')], [
          h.div(
            [
              h.DataAttribute('gym-no-spend-banner', ''),
              Ui.className<Message>(
                'w-fit border border-[#7fb0ff]/30 bg-[#07111f] px-3 py-1 text-[0.75rem] font-semibold uppercase tracking-wide text-[#b8d4ff]',
              ),
            ],
            ['Illustrative / no-spend'],
          ),
          h.h1(
            [Ui.className<Message>('m-0 text-3xl font-semibold sm:text-5xl')],
            ['OpenAgents Gym'],
          ),
          h.p(
            [Ui.className<Message>('m-0 max-w-3xl text-base text-white/65')],
            [
              'A public fixture lab for Khala policy shapes and Terminal-Bench run visualization. This page configures the bundled decision suite, emits public-safe report payloads, and never reaches provider accounts or billing.',
            ],
          ),
        ]),
        terminalBenchScenePanel(),
        h.div([Ui.className<Message>('grid gap-4 lg:grid-cols-[1.2fr_0.8fr]')], [
          h.section([Ui.className<Message>(panelClass)], [
            h.p([Ui.className<Message>(sectionTitleClass)], ['Experiment']),
            h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Environment']),
                h.select(
                  [
                    h.Disabled(true),
                    h.Value(model.experiment.environment),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    h.option(
                      [
                        h.Value('bundled-decision-suite-v1'),
                        h.Selected(true),
                      ],
                      ['Bundled decision suite v1'],
                    ),
                  ],
                ),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Fan-out mode']),
                h.select(
                  [
                    h.Value(model.experiment.fanout.mode),
                    h.OnInput(value =>
                      UpdatedGymFanoutMode({
                        mode: value as GymFanoutMode,
                      }),
                    ),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    option('single', 'Single', model.experiment.fanout.mode),
                    option('race', 'Race', model.experiment.fanout.mode),
                    option(
                      'best-of-n',
                      'Best of n',
                      model.experiment.fanout.mode,
                    ),
                    option(
                      'verifier-pick',
                      'Verifier pick',
                      model.experiment.fanout.mode,
                    ),
                  ],
                ),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Concurrency']),
                h.input([
                  h.Type('number'),
                  h.Min('1'),
                  h.Max('8'),
                  h.Step('1'),
                  h.Value(String(model.experiment.fanout.concurrency)),
                  h.OnInput(value => UpdatedGymConcurrency({ value })),
                  Ui.className<Message>(inputClass),
                ]),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Tool set']),
                h.select(
                  [
                    h.Value(model.experiment.tools),
                    h.OnInput(value =>
                      UpdatedGymToolSet({ tools: value as GymToolSetRef }),
                    ),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    option(
                      'khala-fixture-tools',
                      'Khala fixture tools',
                      model.experiment.tools,
                    ),
                    option(
                      'khala-code-tools',
                      'Khala code tools',
                      model.experiment.tools,
                    ),
                    option('no-tools', 'No tools', model.experiment.tools),
                  ],
                ),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Program signature modules']),
                h.select(
                  [
                    h.Value(model.experiment.modules.mode),
                    h.OnInput(value =>
                      UpdatedGymModuleComposition({
                        mode: value as GymModuleCompositionMode,
                      }),
                    ),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    option(
                      'starter-catalog',
                      'Starter catalog',
                      model.experiment.modules.mode,
                    ),
                    option('none', 'None', model.experiment.modules.mode),
                  ],
                ),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Samples per cell']),
                h.input([
                  h.Type('number'),
                  h.Min('1'),
                  h.Max('25'),
                  h.Step('1'),
                  h.Value(String(model.experiment.samplesPerCell)),
                  h.OnInput(value => UpdatedGymSamplesPerCell({ value })),
                  Ui.className<Message>(inputClass),
                ]),
              ]),
            ]),
            laneControl(model),
            coordinatorControl(model),
            shapeControl(model),
          ]),
          h.aside([Ui.className<Message>('grid gap-4')], [
            h.section([Ui.className<Message>(panelClass)], [
              h.p([Ui.className<Message>(sectionTitleClass)], ['Sampling']),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Temperature']),
                h.input([
                  h.Type('number'),
                  h.Min('0'),
                  h.Max('2'),
                  h.Step('0.1'),
                  h.Value(String(model.experiment.sampling.temperature)),
                  h.OnInput(value => UpdatedGymTemperature({ value })),
                  Ui.className<Message>(inputClass),
                ]),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Reasoning effort']),
                h.select(
                  [
                    h.Value(model.experiment.sampling.reasoningEffort),
                    h.OnInput(value =>
                      UpdatedGymReasoningEffort({
                        reasoningEffort: value as GymReasoningEffort,
                      }),
                    ),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    option(
                      'off',
                      'Off',
                      model.experiment.sampling.reasoningEffort,
                    ),
                    option(
                      'low',
                      'Low',
                      model.experiment.sampling.reasoningEffort,
                    ),
                    option(
                      'medium',
                      'Medium',
                      model.experiment.sampling.reasoningEffort,
                    ),
                    option(
                      'high',
                      'High',
                      model.experiment.sampling.reasoningEffort,
                    ),
                  ],
                ),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Max tokens']),
                h.input([
                  h.Type('number'),
                  h.Min('128'),
                  h.Max('8192'),
                  h.Step('128'),
                  h.Value(String(model.experiment.sampling.maxTokens)),
                  h.OnInput(value => UpdatedGymMaxTokens({ value })),
                  Ui.className<Message>(inputClass),
                ]),
              ]),
              h.label([Ui.className<Message>(labelClass)], [
                h.span([], ['Transport']),
                h.select(
                  [
                    h.Value(model.experiment.sampling.transport),
                    h.OnInput(value =>
                      UpdatedGymTransport({
                        transport: value as GymTransport,
                      }),
                    ),
                    Ui.className<Message>(inputClass),
                  ],
                  [
                    option(
                      'streaming',
                      'Streaming',
                      model.experiment.sampling.transport,
                    ),
                    option('batch', 'Batch', model.experiment.sampling.transport),
                  ],
                ),
              ]),
            ]),
            h.section(
              [
                h.DataAttribute('gym-economics-locked', ''),
                Ui.className<Message>(panelClass),
              ],
              [
                h.p([Ui.className<Message>(sectionTitleClass)], ['Economics']),
                h.label([Ui.className<Message>(labelClass)], [
                  h.span([], ['Mode']),
                  h.input([
                    h.Type('text'),
                    h.Disabled(true),
                    h.Value('fixture only - no spend'),
                    Ui.className<Message>(inputClass),
                  ]),
                ]),
                h.div([Ui.className<Message>('grid grid-cols-2 gap-3')], [
                  stat('Spend cap', '0 msat'),
                  stat('Billable samples', '0'),
                ]),
              ],
            ),
            h.button(
              [
                h.Type('button'),
                h.DataAttribute('gym-run', ''),
                h.OnClick(ClickedRunGymFixture()),
                Ui.className<Message>(
                  'min-h-11 border border-[#7fb0ff]/40 bg-[#0a1d36] px-4 text-sm font-semibold text-[#d5e7ff] hover:bg-[#102b4d] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#7fb0ff]',
                ),
              ],
              ['Run fixture'],
            ),
          ]),
        ]),
        resultPanel(model),
      ]),
    ],
  )
}
