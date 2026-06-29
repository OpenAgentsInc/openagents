import { Match as M } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  type Message,
  RequestedLoadMulletBootstrap,
  SelectedMulletScenarioTemplate,
  SelectedMulletSensitivityAxis,
  UpdatedMulletAssumption,
} from '../message'
import type { Model } from '../model'
import type { MulletBootstrapResponse } from './model'
import {
  assumptionGroups,
  deriveMulletWorkbenchProjection,
  dollars,
  mulletScenarioTemplates,
  mulletScenarioTemplateIds,
  sensitivityAxisOptions,
  type MulletAssumption,
  type MulletScenarioTemplateId,
  type MulletSensitivityAxisId,
  defaultMulletSensitivityAxisId,
} from './workbench'

const boundaryLabel = (value: boolean): string => (value ? 'Enabled' : 'Denied')

const bootstrapSummary = (response: MulletBootstrapResponse): Html => {
  const h = html<Message>()
  const boundary = response.authorityBoundary
  const boundaryRows: ReadonlyArray<readonly [string, boolean]> = [
    ['Live work assignment', boundary.canAssignLiveWork],
    ['Provider mutation', boundary.canMutateProviders],
    ['Wallet spend', boundary.canSpendWalletFunds],
    ['Payout settlement', boundary.canSettlePayouts],
    ['Public claim promotion', boundary.canPromotePublicClaims],
  ]

  return h.section(
    [Ui.className<Message>('grid gap-3 @container')],
    [
      h.div(
        [Ui.className<Message>('grid grid-cols-1 gap-3 @lg:grid-cols-3')],
        [
          metric('Operator', response.access.operatorEmail),
          metric('Visibility', response.access.visibility),
          metric('Schema', response.schemaVersion),
        ],
      ),
      Ui.tableList<Message>({
        caption: 'Mullet authority boundary',
        columns: [
          { key: 'boundary', label: 'Boundary' },
          { key: 'state', label: 'State' },
        ],
        rows: boundaryRows.map(([label, value]) => ({
          id: label,
          cells: {
            boundary: label,
            state: boundaryLabel(value),
          },
        })),
      }),
    ],
  )
}

const metric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-w-0 border border-[#222] bg-[#080808] p-3')],
    [
      h.div([Ui.className<Message>(`${Ui.eyebrowClass} truncate`)], [label]),
      h.div(
        [
          Ui.className<Message>(
            'mt-2 truncate text-base/6 font-medium text-[#f1efe8] sm:text-sm/6',
          ),
        ],
        [value],
      ),
    ],
  )
}

const bootstrapState = (model: Model): Html => {
  const h = html<Message>()

  return M.value(model.mullet.bootstrap).pipe(
    M.tagsExhaustive({
      MulletBootstrapIdle: () => loadingPanel('Not loaded'),
      MulletBootstrapLoading: () => loadingPanel('Loading'),
      MulletBootstrapFailed: ({ error }) =>
        h.section(
          [Ui.className<Message>('border border-[#333] bg-[#080808] p-4')],
          [
            h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Bootstrap']),
            h.h2(
              [
                Ui.className<Message>(
                  'mt-2 text-xl/7 font-medium tracking-normal text-[#f1efe8]',
                ),
              ],
              ['Mullet API unavailable'],
            ),
            h.p(
              [
                Ui.className<Message>(
                  'mt-2 max-w-[72ch] text-base/7 text-white/60 sm:text-sm/6',
                ),
              ],
              [error],
            ),
            h.div(
              [Ui.className<Message>('mt-4')],
              [
                Ui.button<Message>({
                  label: 'Retry',
                  size: 'sm',
                  variant: 'secondary',
                  attrs: [
                    h.Type('button'),
                    h.OnClick(RequestedLoadMulletBootstrap()),
                  ],
                }),
              ],
            ),
          ],
        ),
      MulletBootstrapLoaded: ({ response }) => bootstrapSummary(response),
    }),
  )
}

const loadingPanel = (label: string): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('border border-[#222] bg-[#080808] p-4')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Bootstrap']),
      h.div(
        [Ui.className<Message>('mt-2 text-base/7 font-medium text-[#f1efe8] sm:text-sm/6')],
        [label],
      ),
    ],
  )
}

const templateIdFromModel = (value: string): MulletScenarioTemplateId =>
  mulletScenarioTemplateIds.includes(value as MulletScenarioTemplateId)
    ? (value as MulletScenarioTemplateId)
    : mulletScenarioTemplateIds[0]

const sensitivityAxisFromModel = (value: string): MulletSensitivityAxisId =>
  sensitivityAxisOptions.some(option => option.id === value)
    ? (value as MulletSensitivityAxisId)
    : defaultMulletSensitivityAxisId

const templateSelector = (model: Model): Html => {
  const h = html<Message>()
  const selected = templateIdFromModel(model.mullet.selectedTemplateId)

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Scenario templates']),
      h.div(
        [Ui.className<Message>('grid grid-cols-1 gap-2 @container sm:grid-cols-2 xl:grid-cols-3')],
        mulletScenarioTemplates.map(template => {
          const active = template.id === selected

          return h.button(
            [
              h.Type('button'),
              h.OnClick(
                SelectedMulletScenarioTemplate({ templateId: template.id }),
              ),
              Ui.className<Message>(
                [
                  'grid min-h-28 cursor-pointer content-start gap-2 border p-3 text-left transition-colors',
                  active
                    ? 'border-[#f1efe8] bg-[#0f0f0f]'
                    : 'border-[#222] bg-[#050505] hover:border-[#444]',
                ].join(' '),
              ),
            ],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'truncate text-base/6 font-medium tracking-normal text-[#f1efe8] sm:text-sm/6',
                  ),
                ],
                [template.title],
              ),
              h.span([Ui.className<Message>('text-base/7 text-white/60 sm:text-sm/6')], [
                template.basis,
              ]),
              h.span([Ui.className<Message>(`${Ui.eyebrowClass} truncate`)], [
                template.focus,
              ]),
            ],
          )
        }),
      ),
    ],
  )
}

const stateLegend = (): Html => {
  const h = html<Message>()
  const states = [
    ['Modeled', 'Private simulation value'],
    ['Measured', 'Telemetry-backed value'],
    ['Accepted', 'Accepted-work closeout value'],
    ['Paid', 'Payment receipt-backed value'],
    ['Settled', 'Settlement receipt-backed value'],
  ] as const

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Value states']),
      h.div(
        [Ui.className<Message>('grid grid-cols-1 gap-px border border-[#222] bg-[#222] @container sm:grid-cols-5')],
        states.map(([label, detail]) =>
          h.div(
            [Ui.className<Message>('min-w-0 bg-[#050505] p-3')],
            [
              h.div([Ui.className<Message>('text-base/6 font-medium text-[#f1efe8] sm:text-sm/6')], [
                label,
              ]),
              h.p([Ui.className<Message>('mt-1 text-base/7 text-white/55 sm:text-sm/6')], [
                detail,
              ]),
            ],
          ),
        ),
      ),
    ],
  )
}

const assumptionEditor = (model: Model): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4')],
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Scenario editor',
        title: 'Assumptions',
        body: 'Every editable assumption keeps an explicit source and provenance state. Values remain private modeled inputs until measured, accepted, paid, or settled evidence is attached.',
        level: 2,
      }),
      ...assumptionGroups(model.mullet.assumptions).map(group =>
        h.section(
          [Ui.className<Message>('grid gap-2')],
          [
            h.h3(
              [
                Ui.className<Message>(
                  'text-base/6 font-medium capitalize tracking-normal text-[#f1efe8] sm:text-sm/6',
                ),
              ],
              [group.group],
            ),
            h.div(
              [Ui.className<Message>('grid grid-cols-1 gap-px border border-[#222] bg-[#222]')],
              group.assumptions.map(assumptionControl),
            ),
          ],
        ),
      ),
    ],
  )
}

const assumptionControl = (assumption: MulletAssumption): Html => {
  const h = html<Message>()
  const valueId = `mullet-${assumption.id.replaceAll('.', '-')}-value`
  const sourceId = `mullet-${assumption.id.replaceAll('.', '-')}-source`
  const provenanceId = `mullet-${assumption.id.replaceAll('.', '-')}-provenance`

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-3 bg-[#050505] p-3 lg:grid-cols-[minmax(160px,0.9fr)_minmax(120px,0.45fr)_minmax(150px,0.6fr)_minmax(180px,0.8fr)] lg:items-start',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div([Ui.className<Message>('text-base/6 font-medium text-[#f1efe8] sm:text-sm/6')], [
            assumption.label,
          ]),
          h.p([Ui.className<Message>('mt-1 text-base/7 text-white/55 sm:text-sm/6')], [
            assumption.requiredEvidence,
          ]),
        ],
      ),
      labeledInput({
        attrs: [
          h.OnInput(value =>
            UpdatedMulletAssumption({
              assumptionId: assumption.id,
              field: 'value',
              value,
            }),
          ),
        ],
        id: valueId,
        label: assumption.unit,
        name: `${assumption.id}.value`,
        value: assumption.draftValue,
      }),
      labeledSelect({
        attrs: [
          h.OnInput(value =>
            UpdatedMulletAssumption({
              assumptionId: assumption.id,
              field: 'provenance',
              value,
            }),
          ),
        ],
        id: provenanceId,
        label: 'Provenance',
        name: `${assumption.id}.provenance`,
        selected: assumption.provenance,
      }),
      labeledInput({
        attrs: [
          h.OnInput(value =>
            UpdatedMulletAssumption({
              assumptionId: assumption.id,
              field: 'sourceLabel',
              value,
            }),
          ),
        ],
        id: sourceId,
        label: 'Source',
        name: `${assumption.id}.source`,
        value: assumption.sourceLabel,
      }),
    ],
  )
}

const labeledInput = (input: {
  id: string
  label: string
  name: string
  value: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.label([h.For(input.id), Ui.className<Message>('grid gap-1.5')], [
    h.span([Ui.className<Message>(Ui.eyebrowClass)], [input.label]),
    h.input([
      ...(input.attrs ?? []),
      h.Id(input.id),
      h.Name(input.name),
      h.Type('text'),
      h.Value(input.value),
      Ui.className<Message>(Ui.inputClass),
    ]),
  ])
}

const labeledSelect = (input: {
  id: string
  label: string
  name: string
  selected: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const options = [
    'modeled',
    'measured',
    'accepted',
    'paid',
    'settled',
    'manual_input',
    'estimated',
    'forecast',
    'placeholder',
  ]

  return h.label([h.For(input.id), Ui.className<Message>('grid gap-1.5')], [
    h.span([Ui.className<Message>(Ui.eyebrowClass)], [input.label]),
    h.select(
      [
        ...(input.attrs ?? []),
        h.Id(input.id),
        h.Name(input.name),
        Ui.className<Message>(Ui.selectClass),
      ],
      options.map(option =>
        h.option(
          [
            h.Value(option),
            ...(option === input.selected ? [h.Selected(true)] : []),
          ],
          [option.replaceAll('_', ' ')],
        ),
      ),
    ),
  ])
}

const dispatchTable = (
  rows: ReadonlyArray<Record<string, string>>,
): Html =>
  Ui.tableList<Message>({
    caption: 'Hourly candidate modes',
    columns: [
      { key: 'mode', label: 'Mode' },
      { key: 'selected', label: 'Selected' },
      { key: 'revenue', label: 'Revenue', align: 'right' },
      { key: 'margin', label: 'Margin', align: 'right' },
      { key: 'provider', label: 'Provider', align: 'right' },
      { key: 'energy', label: 'Energy', align: 'right' },
      { key: 'outcomes', label: 'Accepted outcomes', align: 'right' },
      { key: 'net', label: 'Risk-adjusted net', align: 'right' },
      { key: 'gates', label: 'Gates' },
    ],
    rows: rows.map((row, index) => ({
      id: row.mode ?? `dispatch-row-${index}`,
      cells: row,
    })),
  })

const simpleTable = (input: {
  caption: string
  columns: ReadonlyArray<Readonly<{ key: string; label: string; align?: 'right' }>>
  rows: ReadonlyArray<Record<string, string>>
}): Html =>
  Ui.tableList<Message>({
    caption: input.caption,
    columns: input.columns,
    rows: input.rows.map((row, index) => ({
      id: `${input.caption}-${index}`,
      cells: row,
    })),
  })

const sensitivityFocus = (model: Model): Html => {
  const h = html<Message>()
  const selected = sensitivityAxisFromModel(model.mullet.selectedSensitivityAxisId)

  return h.label(
    [h.For('mullet-sensitivity-axis'), Ui.className<Message>('grid gap-1.5 sm:max-w-xs')],
    [
      h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Sensitivity focus']),
      h.select(
        [
          h.Id('mullet-sensitivity-axis'),
          h.Name('mulletSensitivityAxis'),
          Ui.className<Message>(Ui.selectClass),
          h.OnInput(value => SelectedMulletSensitivityAxis({ axisId: value })),
        ],
        sensitivityAxisOptions.map(option =>
          h.option(
            [
              h.Value(option.id),
              ...(option.id === selected ? [h.Selected(true)] : []),
            ],
            [option.label],
          ),
        ),
      ),
    ],
  )
}

export const view = (model: Model): Html => {
  const h = html<Message>()
  const selectedTemplateId = templateIdFromModel(model.mullet.selectedTemplateId)
  const selectedSensitivityAxisId = sensitivityAxisFromModel(
    model.mullet.selectedSensitivityAxisId,
  )
  const projection = deriveMulletWorkbenchProjection({
    assumptions: model.mullet.assumptions,
    selectedSensitivityAxisId,
    selectedTemplateId,
  })

  return h.div(
    [Ui.className<Message>('mx-auto grid w-full max-w-7xl gap-6 px-4 sm:px-6 lg:px-8')],
    [
      h.header(
        [
          Ui.className<Message>(
            'grid gap-4 border-b border-[#222] pb-5 md:grid-cols-[minmax(0,1fr)_auto] md:items-end',
          ),
        ],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Private operator',
            title: 'Mullet',
            body: 'Unified dispatch for mining, raw GPU, token/API, curtailment, and accepted work. This surface is private and simulation-only.',
            level: 1,
          }),
          Ui.button<Message>({
            label: 'Refresh',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              h.OnClick(RequestedLoadMulletBootstrap()),
            ],
          }),
        ],
      ),
      bootstrapState(model),
      templateSelector(model),
      stateLegend(),
      h.section(
        [Ui.className<Message>('grid grid-cols-1 gap-3 @container md:grid-cols-4')],
        [
          metric('Selected dispatch', projection.selectedCandidate.mode),
          metric(
            'Effective buyer price',
            dollars(projection.effectiveBuyerPriceUsd),
          ),
          metric(
            'Energy',
            `${Number(projection.dispatch.energyMwh).toLocaleString()} MWh`,
          ),
          metric('Reason', projection.dispatch.reasonCode),
        ],
      ),
      assumptionEditor(model),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Hourly dispatch',
            title: 'Candidate modes',
            body: 'Revenue, provider payout, and OpenAgents margin are shown per candidate mode. Buyer revenue is not repeated in party payout rows.',
            level: 2,
          }),
          dispatchTable(projection.candidateRows),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Returns',
            title: 'Party-specific view',
            body: 'OpenAgents counts buyer revenue once. Provider, facility, hardware-owner, validator, and reviewer rows show payouts from that accepted-work revenue, not additional buyer revenue.',
            level: 2,
          }),
          simpleTable({
            caption: 'Party-specific returns',
            columns: [
              { key: 'party', label: 'Party' },
              { key: 'basis', label: 'Buyer revenue basis' },
              { key: 'gross', label: 'Gross', align: 'right' },
              { key: 'cogs', label: 'COGS', align: 'right' },
              { key: 'margin', label: 'Margin', align: 'right' },
              { key: 'payback', label: 'Payback', align: 'right' },
              { key: 'npv', label: 'NPV', align: 'right' },
            ],
            rows: projection.partyRows,
          }),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Accepted outcomes',
            title: 'Per-energy economics',
            level: 2,
          }),
          simpleTable({
            caption: 'Accepted outcome metrics',
            columns: [
              { key: 'metric', label: 'Metric' },
              { key: 'value', label: 'Value', align: 'right' },
              { key: 'state', label: 'State' },
            ],
            rows: projection.acceptedMetricRows,
          }),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Sensitivity',
            title: 'Decision flips',
            body: 'Each row perturbs one driver while holding the other edited assumptions fixed.',
            level: 2,
          }),
          sensitivityFocus(model),
          simpleTable({
            caption: 'Sensitivity decision flips',
            columns: [
              { key: 'axis', label: 'Axis' },
              { key: 'focus', label: 'Focus' },
              { key: 'low', label: 'Low case' },
              { key: 'high', label: 'High case' },
              { key: 'netRange', label: 'Net range', align: 'right' },
              { key: 'decision', label: 'Decision' },
            ],
            rows: projection.sensitivityRows,
          }),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Evidence',
            title: 'Empty states',
            body: 'These states block external claims and settlement until the corresponding proof is attached.',
            level: 2,
          }),
          simpleTable({
            caption: 'Mullet proof and evidence empty states',
            columns: [
              { key: 'boundary', label: 'Boundary' },
              { key: 'state', label: 'State' },
              { key: 'action', label: 'Next action' },
            ],
            rows: projection.evidenceRows,
          }),
        ],
      ),
    ],
  )
}
