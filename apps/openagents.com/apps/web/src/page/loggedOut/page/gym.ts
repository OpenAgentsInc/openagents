import { Array as Arr } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
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
  type GymReasoningEffort,
  type GymToolSetRef,
  type GymTransport,
  coordinatorOptions,
  laneOptions,
  sequenceShapeOptions,
} from '../gym/flow'
import type { PublicGymRunProgressModel } from '../model'
import {
  type GymRunProgress,
  type GymRunProgressPublicProjection,
  formatRunProgressCount,
  formatRunProgressDuration,
  formatRunProgressPercent,
  runPhaseLabel,
} from '../gym/runProgress'

type InputAttr = Parameters<ReturnType<typeof html<Message>>['input']>[0][number]
type DivAttr = Parameters<ReturnType<typeof html<Message>>['div']>[0][number]

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

// ---------------------------------------------------------------------------
// Honest empty states.
//
// The Gym surfaces render LIVE data only. There is no seeded run, no fixture
// comparison, and no synthesized report on this page. Until a real Harbor/Khala
// benchmark is ingested into the Worker, each surface shows an honest empty
// state instead of fabricated numbers.
// ---------------------------------------------------------------------------

const emptyState = (
  attrs: ReadonlyArray<DivAttr>,
  eyebrow: string,
  heading: string,
  body: string,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      Ui.className<Message>(
        'grid place-items-start gap-2 border border-dashed border-white/15 bg-black p-6',
      ),
    ],
    [
      h.p([Ui.className<Message>(sectionTitleClass)], [eyebrow]),
      h.p(
        [Ui.className<Message>('m-0 text-base font-semibold text-white/80 sm:text-lg')],
        [heading],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/55 sm:text-sm')],
        [body],
      ),
    ],
  )
}

const terminalBenchPanel = (): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('gym-terminal-bench-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2')], [
        h.p([Ui.className<Message>(sectionTitleClass)], [
          'Terminal-Bench Gym replay',
        ]),
        h.h2(
          [Ui.className<Message>('m-0 max-w-[20ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-4xl')],
          ['Terminal-Bench 2.0 run field'],
        ),
        h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/65 sm:text-sm')], [
          'A three-effect field for profile lanes, verifier placement, accepted/failing tasks, cost basis, latency, throughput, and claim caveats. It renders from real published benchmark reports only.',
        ]),
      ]),
      emptyState(
        [h.DataAttribute('gym-terminal-bench-empty', '')],
        'Benchmark comparison',
        'No decision-grade benchmark reports published yet',
        'When a real Terminal-Bench report is ingested and authorized for the web, its lanes, verifier placement, and caveats appear here. No fixture or placeholder pass rates are shown.',
      ),
    ],
  )
}

const runProgressEmptyState = (): Html => {
  const h = html<Message>()

  return emptyState(
    [
      h.DataAttribute('gym-run-progress-accessible-mirror', ''),
      h.AriaLabel('Live Gym run progress'),
      h.DataAttribute('gym-run-progress-empty', ''),
    ],
    'Live run',
    'No active Gym run',
    'Live runs appear here when a real Harbor/Khala benchmark is ingested. Pass rate is always computed over completed tasks, with the official denominator kept separate, so a partial run is never read as a final solve rate.',
  )
}

const runProgressStat = (label: string, value: string): Html => {
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

// A `web_authorized` run: render the live counts/pass-rate-over-completed,
// official denominator, profile label, tokens, freshness, and the honest
// in-progress / decisionGrade:false markers. The accessible mirror carries the
// same numbers as text so the follow-along is readable without the field.
const renderAuthorizedRun = (run: GymRunProgress): Html => {
  const h = html<Message>()
  const { counts } = run

  return h.article(
    [
      h.DataAttribute('gym-run-progress-accessible-mirror', ''),
      h.DataAttribute('gym-run', run.runRef),
      h.AriaLabel(`Live Gym run progress: ${run.profile.publicLabel}`),
      Ui.className<Message>('grid gap-3 border border-white/10 bg-black p-4'),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center justify-between gap-2')],
        [
          h.div([Ui.className<Message>('grid gap-0.5')], [
            h.p(
              [Ui.className<Message>('m-0 text-base font-semibold text-white')],
              [run.profile.publicLabel],
            ),
            h.p(
              [Ui.className<Message>('m-0 text-[0.78rem] text-white/50')],
              [`${run.profile.model} · ${run.profile.attribution}`],
            ),
          ]),
          h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
            h.span(
              [
                h.DataAttribute('gym-run-in-progress', String(run.inProgress)),
                Ui.className<Message>(
                  'border border-[#7fb0ff]/30 bg-[#07111f] px-2 py-1 text-[0.7rem] font-semibold uppercase tracking-wide text-[#b8d4ff]',
                ),
              ],
              [runPhaseLabel(run)],
            ),
            h.span(
              [
                h.DataAttribute('gym-run-decision-grade', 'false'),
                Ui.className<Message>(
                  'border border-white/15 bg-white/[0.03] px-2 py-1 text-[0.7rem] font-semibold uppercase tracking-wide text-white/55',
                ),
              ],
              ['not decision-grade'],
            ),
          ]),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-2 lg:grid-cols-4')],
        [
          runProgressStat(
            'Completed / official',
            `${counts.completed} / ${counts.officialDenominator}`,
          ),
          runProgressStat(
            'Pass rate over completed',
            formatRunProgressPercent(run.passRateOverCompleted),
          ),
          runProgressStat(
            'Progress',
            formatRunProgressPercent(run.completionFraction),
          ),
          runProgressStat(
            'Tokens served',
            formatRunProgressCount(run.tokens.totalTokens),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-3 lg:grid-cols-5')],
        [
          runProgressStat('Passed', formatRunProgressCount(counts.completedPassed)),
          runProgressStat('Failed', formatRunProgressCount(counts.completedFailed)),
          runProgressStat('Running', formatRunProgressCount(counts.running)),
          runProgressStat('Pending', formatRunProgressCount(counts.pending)),
          runProgressStat('Errored', formatRunProgressCount(counts.error)),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.78rem] text-white/45')],
        [
          `${run.profile.hardwareProfile} · elapsed ${formatRunProgressDuration(
            run.elapsedMs,
          )} · updated ${run.lastUpdatedAt}`,
        ],
      ),
    ],
  )
}

// A `local_only` run degrades honestly: no live numbers, just the
// awaiting-authorization marker the public projection carries.
const renderUnpublishedRun = (
  run: Extract<GymRunProgressPublicProjection, { publication: 'local_only' }>,
): Html =>
  emptyState(
    [
      html<Message>().DataAttribute('gym-run-progress-accessible-mirror', ''),
      html<Message>().DataAttribute('gym-run', run.runRef),
      html<Message>().AriaLabel(`Gym run awaiting authorization: ${run.runRef}`),
    ],
    'Live run',
    'Run awaiting web authorization',
    'This run is recorded locally but is not yet authorized for web publication, so no live numbers are shown rather than inventing any.',
  )

const renderRun = (run: GymRunProgressPublicProjection): Html =>
  run.publication === 'web_authorized'
    ? renderAuthorizedRun(run)
    : renderUnpublishedRun(run)

const runProgressBody = (model: PublicGymRunProgressModel): Html => {
  const h = html<Message>()

  if (model._tag !== 'PublicGymRunProgressLoaded') {
    return runProgressEmptyState()
  }

  return Arr.match(model.runs, {
    onEmpty: () => runProgressEmptyState(),
    onNonEmpty: runs =>
      h.div(
        [Ui.className<Message>('grid gap-4')],
        Arr.map(runs, renderRun),
      ),
  })
}

const runProgressPanel = (model: PublicGymRunProgressModel): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('gym-run-progress-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2')], [
        h.p([Ui.className<Message>(sectionTitleClass)], [
          'Live Gym run follow-along',
        ]),
        h.h2(
          [Ui.className<Message>('m-0 max-w-[24ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl')],
          ['Follow an active Terminal-Bench run'],
        ),
        h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/65 sm:text-sm')], [
          'Counts, pass-rate over completed tasks, the official denominator, and freshness update from the public-safe progress projection as a real run is ingested; raw prompts, responses, logs, trajectories, keys, and private endpoints are never included.',
        ]),
      ]),
      runProgressBody(model),
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

export const view = (
  model: GymModel,
  runProgress: PublicGymRunProgressModel,
): Html => {
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
            ['Live data only / no-spend'],
          ),
          h.h1(
            [Ui.className<Message>('m-0 text-3xl font-semibold sm:text-5xl')],
            ['OpenAgents Gym'],
          ),
          h.p(
            [Ui.className<Message>('m-0 max-w-3xl text-base text-white/65')],
            [
              'A public lab for Khala policy shapes and Terminal-Bench run visualization. Configure the bundled decision suite below. Live runs and benchmark reports populate the surfaces above once a real Harbor/Khala run is ingested; nothing on this page is fabricated, and this page never reaches provider accounts or billing.',
            ],
          ),
        ]),
        terminalBenchPanel(),
        runProgressPanel(runProgress),
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
                    h.Value('no spend'),
                    Ui.className<Message>(inputClass),
                  ]),
                ]),
                h.div([Ui.className<Message>('grid grid-cols-2 gap-3')], [
                  stat('Spend cap', '0 msat'),
                  stat('Billable samples', '0'),
                ]),
              ],
            ),
            h.div(
              [
                h.DataAttribute('gym-run-ingest-note', ''),
                Ui.className<Message>(
                  'border border-[#7fb0ff]/30 bg-[#07111f] px-3 py-2 text-[0.78rem] text-[#b8d4ff]',
                ),
              ],
              [
                'Runs are not launched from this page. Live runs and benchmark reports populate above once a real Harbor/Khala run is ingested into the Worker.',
              ],
            ),
          ]),
        ]),
      ]),
    ],
  )
}
