import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

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
  type GymReasoningEffort,
  type GymToolSetRef,
  type GymTransport,
  coordinatorOptions,
  laneOptions,
  sequenceShapeOptions,
} from '../gym/flow'

type InputAttr = Parameters<ReturnType<typeof html<Message>>['input']>[0][number]

const pageClass =
  'mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 text-white sm:px-6 lg:px-8'
const panelClass = 'grid gap-4 border border-white/10 bg-[#050505] p-4'
const sectionTitleClass =
  'm-0 text-[0.78rem] font-semibold uppercase tracking-[0.08em] text-white/55'
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
        h.p([Ui.className<Message>(sectionTitleClass)], ['Report viewer']),
        h.p(
          [Ui.className<Message>('m-0 max-w-[58ch] text-sm text-white/60')],
          [
            'Run the fixture to prepare the typed report payload. Phase 0 renders the summary here; the richer report viewer lands next.',
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
      h.div([Ui.className<Message>('grid gap-3 sm:grid-cols-4')], [
        stat('Cells', String(result.expectedCellCount)),
        stat('Executed', String(result.executedCellCount)),
        stat('Skipped', String(result.skippedCellCount)),
        stat('Mean cost', `$${result.metrics.meanCostUsd.toFixed(2)}`),
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
      Ui.className<Message>('min-h-screen bg-black'),
    ],
    [
      h.div([Ui.className<Message>(pageClass)], [
        h.header([Ui.className<Message>('grid gap-3')], [
          h.div(
            [
              h.DataAttribute('gym-no-spend-banner', ''),
              Ui.className<Message>(
                'w-fit border border-[#7fb0ff]/30 bg-[#07111f] px-3 py-1 text-[0.75rem] font-semibold uppercase tracking-[0.08em] text-[#b8d4ff]',
              ),
            ],
            ['Illustrative / no-spend - fixture seam locked'],
          ),
          h.h1(
            [Ui.className<Message>('m-0 text-3xl font-semibold sm:text-5xl')],
            ['OpenAgents Gym'],
          ),
          h.p(
            [Ui.className<Message>('m-0 max-w-3xl text-base text-white/65')],
            [
              'A public fixture lab for Khala policy shapes. This Phase 0 page configures the bundled decision suite, emits a report-viewer payload, and never reaches provider accounts or billing.',
            ],
          ),
        ]),
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
                  h.span([], ['Seam']),
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
                  'min-h-11 border border-[#7fb0ff]/40 bg-[#0a1d36] px-4 text-sm font-semibold text-[#d5e7ff] transition hover:bg-[#102b4d] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#7fb0ff]',
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
