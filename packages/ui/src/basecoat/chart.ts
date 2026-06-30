import { Option } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type ChartType =
  | 'bar'
  | 'line'
  | 'pie'
  | 'doughnut'
  | 'polarArea'
  | 'radar'
  | 'scatter'
  | 'bubble'

export type ChartDatum = Readonly<Record<string, unknown>>

export type ChartSeries = Readonly<{
  label?: string
  color?: string
  backgroundColor?: string
  hidden?: boolean
  disabled?: boolean
}>

export type ChartPoint = Readonly<{
  id: string
  label: string
  seriesKey: string
  seriesLabel: string
  value: unknown
  color: string
  index: number
}>

export type ChartModel = Readonly<{
  type: ChartType
  labelKey: string
  data: ReadonlyArray<ChartDatum>
  series: Readonly<Record<string, ChartSeries>>
  open: boolean
  activePointId: string | null
  selectedPointId: string | null
  focusedPointId: string | null
  focusedSeriesKey: string | null
  hiddenSeriesKeys: ReadonlyArray<string>
}>

export type ChartInit = Readonly<{
  type?: ChartType
  labelKey?: string
  data?: ReadonlyArray<ChartDatum>
  series?: Readonly<Record<string, ChartSeries>>
  open?: boolean
  activePointId?: string | null
  selectedPointId?: string | null
  focusedPointId?: string | null
  focusedSeriesKey?: string | null
  hiddenSeriesKeys?: ReadonlyArray<string>
}>

export type ChartKey =
  | 'ArrowDown'
  | 'ArrowUp'
  | 'ArrowLeft'
  | 'ArrowRight'
  | 'Home'
  | 'End'
  | 'Enter'
  | ' '
  | 'Spacebar'
  | 'Escape'

export type ChartMessage =
  | Readonly<{ _tag: 'ChartOpened'; pointId?: string | null }>
  | Readonly<{ _tag: 'ChartClosed' }>
  | Readonly<{ _tag: 'ChartFocused'; pointId: string }>
  | Readonly<{ _tag: 'ChartSelected'; pointId: string | null }>
  | Readonly<{ _tag: 'ChartKeyDown'; key: ChartKey }>
  | Readonly<{ _tag: 'ChartLegendFocused'; seriesKey: string }>
  | Readonly<{ _tag: 'ChartLegendToggled'; seriesKey: string }>
  | Readonly<{ _tag: 'ChartLegendKeyDown'; key: ChartKey }>

export type ChartProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ChartModel
  children: BasecoatChildren
  ariaLabel?: string
  toMessage?: (message: ChartMessage) => Message
}>

export type ChartCanvasProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ChartModel
  id?: string
  ariaLabel?: string
  toMessage?: (message: ChartMessage) => Message
}>

export type ChartTooltipProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ChartModel
}>

export type ChartLegendProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ChartModel
  toMessage?: (message: ChartMessage) => Message
}>

export type ChartViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ChartModel
  id?: string
  ariaLabel?: string
  legend?: boolean
  toMessage: (message: ChartMessage) => Message
}>

const chartRoot = basecoatClass('chart')
const chartTooltipRoot = basecoatClass('chart-tooltip')
const chartTooltipTitleRoot = basecoatClass('chart-tooltip-title')
const chartTooltipItemsRoot = basecoatClass('chart-tooltip-items')
const chartTooltipItemRoot = basecoatClass('chart-tooltip-item')
const chartTooltipIndicatorRoot = basecoatClass('chart-tooltip-indicator')
const chartTooltipLabelRoot = basecoatClass('chart-tooltip-label')
const chartTooltipValueRoot = basecoatClass('chart-tooltip-value')
const chartLegendRoot = basecoatClass('chart-legend')
const chartLegendItemRoot = basecoatClass('chart-legend-item')
const chartLegendIndicatorRoot = basecoatClass('chart-legend-indicator')

const defaultColors = [
  'var(--chart-1)',
  'var(--chart-2)',
  'var(--chart-3)',
  'var(--chart-4)',
  'var(--chart-5)',
]

const colorForIndex = (index: number): string =>
  defaultColors[index % defaultColors.length] ?? defaultColors[0]!

const pointId = (seriesKey: string, index: number): string =>
  `${seriesKey.replace(/[^a-zA-Z0-9_-]+/g, '-') || 'series'}-${index}`

const seriesEntries = (
  series: Readonly<Record<string, ChartSeries>>,
): ReadonlyArray<readonly [string, ChartSeries]> => Object.entries(series)

const valueLabel = (value: unknown): string =>
  typeof value === 'number' ? value.toLocaleString() : String(value ?? '')

export const chartPoints = (model: ChartModel): ReadonlyArray<ChartPoint> => {
  const hidden = new Set(model.hiddenSeriesKeys)

  return model.data.flatMap((row, index) => {
    const label = valueLabel(row[model.labelKey])

    return seriesEntries(model.series)
      .filter(([seriesKey, series]) =>
        series.disabled !== true &&
        series.hidden !== true &&
        !hidden.has(seriesKey)
      )
      .map(([seriesKey, series], seriesIndex) => ({
        id: pointId(seriesKey, index),
        label,
        seriesKey,
        seriesLabel: series.label ?? seriesKey,
        value: row[seriesKey],
        color: series.color ?? series.backgroundColor ?? colorForIndex(seriesIndex),
        index,
      }))
  })
}

const findPoint = (
  model: ChartModel,
  id: string | null | undefined,
): ChartPoint | undefined =>
  id === undefined || id === null
    ? undefined
    : chartPoints(model).find(point => point.id === id)

const activePoint = (model: ChartModel): ChartPoint | undefined =>
  findPoint(model, model.activePointId) ??
  findPoint(model, model.focusedPointId) ??
  findPoint(model, model.selectedPointId) ??
  chartPoints(model)[0]

const visibleSeriesKeys = (model: ChartModel): ReadonlyArray<string> => {
  const hidden = new Set(model.hiddenSeriesKeys)

  return seriesEntries(model.series)
    .filter(([, series]) => series.disabled !== true && series.hidden !== true)
    .map(([key]) => key)
    .filter(key => !hidden.has(key))
}

const movePoint = (model: ChartModel, offset: number): ChartModel => {
  const points = chartPoints(model)
  if (points.length === 0) return model

  const current = model.activePointId ?? model.focusedPointId ?? model.selectedPointId
  const index = current === null
    ? -1
    : points.findIndex(point => point.id === current)
  const nextIndex = Math.min(
    points.length - 1,
    Math.max(0, index === -1 ? 0 : index + offset),
  )
  const nextPoint = points[nextIndex]

  return nextPoint === undefined
    ? model
    : {
        ...model,
        open: true,
        activePointId: nextPoint.id,
        focusedPointId: nextPoint.id,
      }
}

const moveFocusedSeries = (model: ChartModel, offset: number): ChartModel => {
  const keys = visibleSeriesKeys({
    ...model,
    hiddenSeriesKeys: [],
  })
  if (keys.length === 0) return model

  const index = model.focusedSeriesKey === null
    ? -1
    : keys.indexOf(model.focusedSeriesKey)
  const nextIndex = Math.min(
    keys.length - 1,
    Math.max(0, index === -1 ? 0 : index + offset),
  )

  return { ...model, focusedSeriesKey: keys[nextIndex] ?? null }
}

export const chartInit = (input: ChartInit = {}): ChartModel => {
  const base: ChartModel = {
    type: input.type ?? 'bar',
    labelKey: input.labelKey ?? 'label',
    data: input.data ?? [],
    series: input.series ?? {},
    open: input.open === true,
    activePointId: input.activePointId ?? null,
    selectedPointId: input.selectedPointId ?? null,
    focusedPointId: input.focusedPointId ?? null,
    focusedSeriesKey: input.focusedSeriesKey ?? null,
    hiddenSeriesKeys: input.hiddenSeriesKeys ?? [],
  }
  const points = chartPoints(base)
  const validPointIds = new Set(points.map(point => point.id))
  const validSeriesKeys = new Set(seriesEntries(base.series).map(([key]) => key))

  return {
    ...base,
    activePointId: base.activePointId !== null && validPointIds.has(base.activePointId)
      ? base.activePointId
      : null,
    selectedPointId: base.selectedPointId !== null && validPointIds.has(base.selectedPointId)
      ? base.selectedPointId
      : null,
    focusedPointId: base.focusedPointId !== null && validPointIds.has(base.focusedPointId)
      ? base.focusedPointId
      : null,
    focusedSeriesKey: base.focusedSeriesKey !== null && validSeriesKeys.has(base.focusedSeriesKey)
      ? base.focusedSeriesKey
      : null,
    hiddenSeriesKeys: base.hiddenSeriesKeys.filter(key => validSeriesKeys.has(key)),
  }
}

export const chartUpdate = (
  model: ChartModel,
  message: ChartMessage,
): ChartModel => {
  switch (message._tag) {
    case 'ChartOpened': {
      const preferred = findPoint(model, message.pointId) ?? activePoint(model)
      return {
        ...model,
        open: preferred !== undefined,
        activePointId: preferred?.id ?? null,
      }
    }
    case 'ChartClosed':
      return { ...model, open: false, activePointId: null }
    case 'ChartFocused':
      return findPoint(model, message.pointId) === undefined
        ? model
        : {
            ...model,
            open: true,
            activePointId: message.pointId,
            focusedPointId: message.pointId,
          }
    case 'ChartSelected':
      return message.pointId === null || findPoint(model, message.pointId) !== undefined
        ? {
            ...model,
            open: message.pointId !== null,
            activePointId: message.pointId,
            selectedPointId: message.pointId,
            focusedPointId: message.pointId,
          }
        : model
    case 'ChartKeyDown':
      switch (message.key) {
        case 'ArrowRight':
        case 'ArrowDown':
          return movePoint(model, 1)
        case 'ArrowLeft':
        case 'ArrowUp':
          return movePoint(model, -1)
        case 'Home': {
          const point = chartPoints(model)[0]
          return point === undefined
            ? model
            : { ...model, open: true, activePointId: point.id, focusedPointId: point.id }
        }
        case 'End': {
          const points = chartPoints(model)
          const point = points[points.length - 1]
          return point === undefined
            ? model
            : { ...model, open: true, activePointId: point.id, focusedPointId: point.id }
        }
        case 'Enter':
        case ' ':
        case 'Spacebar': {
          const point = activePoint(model)
          return point === undefined
            ? model
            : chartUpdate(model, { _tag: 'ChartSelected', pointId: point.id })
        }
        case 'Escape':
          return chartUpdate(model, { _tag: 'ChartClosed' })
      }
    case 'ChartLegendFocused':
      return seriesEntries(model.series).some(([key]) => key === message.seriesKey)
        ? { ...model, focusedSeriesKey: message.seriesKey }
        : model
    case 'ChartLegendToggled': {
      const series = model.series[message.seriesKey]
      if (series === undefined || series.disabled === true || series.hidden === true) {
        return model
      }

      const currentlyHidden = model.hiddenSeriesKeys.includes(message.seriesKey)
      const hiddenSeriesKeys = currentlyHidden
        ? model.hiddenSeriesKeys.filter(key => key !== message.seriesKey)
        : [...model.hiddenSeriesKeys, message.seriesKey]
      const next = chartInit({ ...model, hiddenSeriesKeys })

      return {
        ...next,
        focusedSeriesKey: message.seriesKey,
      }
    }
    case 'ChartLegendKeyDown':
      switch (message.key) {
        case 'ArrowRight':
        case 'ArrowDown':
          return moveFocusedSeries(model, 1)
        case 'ArrowLeft':
        case 'ArrowUp':
          return moveFocusedSeries(model, -1)
        case 'Home':
          return { ...model, focusedSeriesKey: visibleSeriesKeys({ ...model, hiddenSeriesKeys: [] })[0] ?? null }
        case 'End': {
          const keys = visibleSeriesKeys({ ...model, hiddenSeriesKeys: [] })
          return { ...model, focusedSeriesKey: keys[keys.length - 1] ?? null }
        }
        case 'Enter':
        case ' ':
        case 'Spacebar':
          return model.focusedSeriesKey === null
            ? model
            : chartUpdate(model, {
                _tag: 'ChartLegendToggled',
                seriesKey: model.focusedSeriesKey,
              })
        case 'Escape':
          return chartUpdate(model, { _tag: 'ChartClosed' })
      }
  }
}

const mappedAttr = <Message>(
  toMessage: ((message: ChartMessage) => Message) | undefined,
  message: ChartMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  toMessage === undefined ? [] : [attr(toMessage(message))]

const handledChartKeys = new Set<ChartKey>([
  'ArrowDown',
  'ArrowUp',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  'Enter',
  ' ',
  'Spacebar',
  'Escape',
])

const mappedKeydownAttr = <Message>(
  toMessage: ((message: ChartMessage) => Message) | undefined,
  tag: 'ChartKeyDown' | 'ChartLegendKeyDown',
): ReadonlyArray<Attribute<Message>> => {
  if (toMessage === undefined) return []
  const h = html<Message>()

  return [
    h.OnKeyDownPreventDefault(key =>
      handledChartKeys.has(key as ChartKey)
        ? Option.some(toMessage({ _tag: tag, key: key as ChartKey }))
        : Option.none(),
    ),
  ]
}

const chartDataAttribute = (model: ChartModel): string =>
  JSON.stringify(model.data)

const chartSeriesAttribute = (model: ChartModel): string =>
  JSON.stringify(
    Object.fromEntries(
      seriesEntries(model.series).map(([key, series]) => [
        key,
        {
          ...series,
          hidden: series.hidden === true || model.hiddenSeriesKeys.includes(key),
        },
      ]),
    ),
  )

const tooltipStyle = (point: ChartPoint | undefined): Record<string, string> =>
  point === undefined
    ? {}
    : {
        left: `${Math.min(95, Math.max(5, ((point.index + 1) / 6) * 100))}%`,
        top: '0',
      }

export const chartCanvas = <Message>(
  input: ChartCanvasProps<Message>,
): Html => {
  const h = html<Message>()
  const points = chartPoints(input.model)

  return h.canvas(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      h.Role('img'),
      h.Attribute('tabindex', '0'),
      h.AriaLabel(input.ariaLabel ?? 'Chart'),
      h.AriaActiveDescendant(input.model.activePointId ?? ''),
      h.DataAttribute('chart-type', input.model.type),
      h.DataAttribute('chart-label-key', input.model.labelKey),
      h.DataAttribute('chart-data', chartDataAttribute(input.model)),
      h.DataAttribute('chart-series', chartSeriesAttribute(input.model)),
      h.DataAttribute('chart-tooltip', 'true'),
      h.DataAttribute('chart-point-count', String(points.length)),
      ...mappedAttr<Message>(
        input.toMessage,
        { _tag: 'ChartOpened', pointId: input.model.activePointId },
        h.OnFocus,
      ),
      ...mappedAttr<Message>(
        input.toMessage,
        { _tag: 'ChartClosed' },
        h.OnMouseLeave,
      ),
      ...mappedKeydownAttr<Message>(input.toMessage, 'ChartKeyDown'),
    ],
    [],
  )
}

export const chartTooltip = <Message>(
  input: ChartTooltipProps<Message>,
): Html => {
  const h = html<Message>()
  const point = input.model.open ? activePoint(input.model) : undefined

  return h.div(
    [
      ...basecoatAttrs<Message>(input, chartTooltipRoot),
      h.Role('status'),
      h.AriaLive('polite'),
      ...(point === undefined ? [h.Hidden(true)] : []),
      h.Style(tooltipStyle(point)),
    ],
    point === undefined
      ? []
      : [
          h.div(
            basecoatAttrs<Message>({}, chartTooltipTitleRoot),
            [point.label],
          ),
          h.div(
            basecoatAttrs<Message>({}, chartTooltipItemsRoot),
            [
              h.div(
                basecoatAttrs<Message>({}, chartTooltipItemRoot),
                [
                  h.span(
                    [
                      ...basecoatAttrs<Message>({}, chartTooltipIndicatorRoot),
                      h.Style({ '--chart-indicator-color': point.color }),
                    ],
                    [],
                  ),
                  h.span(
                    basecoatAttrs<Message>({}, chartTooltipLabelRoot),
                    [point.seriesLabel],
                  ),
                  h.span(
                    basecoatAttrs<Message>({}, chartTooltipValueRoot),
                    [valueLabel(point.value)],
                  ),
                ],
              ),
            ],
          ),
        ],
  )
}

export const chartLegend = <Message>(
  input: ChartLegendProps<Message>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [
      ...basecoatAttrs<Message>(input, chartLegendRoot),
      ...mappedKeydownAttr<Message>(input.toMessage, 'ChartLegendKeyDown'),
    ],
    seriesEntries(input.model.series).map(([seriesKey, series], index) => {
      const hidden = input.model.hiddenSeriesKeys.includes(seriesKey) || series.hidden === true
      const disabled = series.disabled === true

      return h.li(
        [],
        [
          h.button(
            [
              ...basecoatAttrs<Message>({}, chartLegendItemRoot),
              h.Type('button'),
              h.Attribute('tabindex', input.model.focusedSeriesKey === seriesKey ? '0' : '-1'),
              h.Attribute('aria-pressed', String(!hidden)),
              h.DataAttribute('series-key', seriesKey),
              ...(disabled ? [h.Disabled(true), h.AriaDisabled(true)] : []),
              h.Style({
                '--chart-indicator-color': series.color ?? series.backgroundColor ?? colorForIndex(index),
              }),
              ...mappedAttr<Message>(
                input.toMessage,
                { _tag: 'ChartLegendFocused', seriesKey },
                h.OnFocus,
              ),
              ...(disabled
                ? []
                : mappedAttr<Message>(
                    input.toMessage,
                    { _tag: 'ChartLegendToggled', seriesKey },
                    h.OnClick,
                  )),
            ],
            [
              h.span(
                basecoatAttrs<Message>({}, chartLegendIndicatorRoot),
                [],
              ),
              h.span([], [series.label ?? seriesKey]),
            ],
          ),
        ],
      )
    }),
  )
}

export const chart = <Message>(input: ChartProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, chartRoot),
      h.Role('group'),
      ...(input.ariaLabel === undefined ? [] : [h.AriaLabel(input.ariaLabel)]),
    ],
    input.children,
  )
}

export const chartView = <Message>(
  input: ChartViewProps<Message>,
): Html =>
  chart<Message>({
    model: input.model,
    children: [
      chartCanvas<Message>({
        model: input.model,
        toMessage: input.toMessage,
        ...(input.id === undefined ? {} : { id: input.id }),
        ...(input.ariaLabel === undefined ? {} : { ariaLabel: input.ariaLabel }),
      }),
      chartTooltip<Message>({ model: input.model }),
      ...(input.legend === true
        ? [
            chartLegend<Message>({
              model: input.model,
              toMessage: input.toMessage,
            }),
          ]
        : []),
    ],
    ...(input.ariaLabel === undefined ? {} : { ariaLabel: input.ariaLabel }),
    ...(input.className === undefined ? {} : { className: input.className }),
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
  })
