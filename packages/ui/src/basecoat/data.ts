import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type TableContainerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type TableProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type TableCaptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type TableSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type TableRowState = 'selected'

export type TableRowProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  state?: TableRowState
}>

export type TableHeadProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  colSpan?: number
  rowSpan?: number
  scope?: 'col' | 'row' | 'colgroup' | 'rowgroup'
}>

export type TableCellProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  colSpan?: number
  rowSpan?: number
}>

export type ProgressProps<Message> = BasecoatAttrs<Message> & Readonly<{
  value: number
  min?: number
  max?: number
  label?: string
  labelledBy?: string
  indicatorAttrs?: BasecoatAttrs<Message>['attrs']
  indicatorClassName?: string
}>

const tableContainerRoot = basecoatClass('table-container')
const tableRoot = basecoatClass('table')
const progressRoot = basecoatClass('progress')

const numericAttr = <Message>(
  key: string,
  value: number | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (value === undefined) {
    return []
  }

  return [html<Message>().Attribute(key, String(value))]
}

const tableSpanAttrs = <Message>(
  input: Readonly<{ colSpan?: number; rowSpan?: number }>,
) => [
  ...numericAttr<Message>('colspan', input.colSpan),
  ...numericAttr<Message>('rowspan', input.rowSpan),
]

const progressPercent = (
  value: number,
  min: number,
  max: number,
): number => {
  if (!Number.isFinite(value) || !Number.isFinite(min) || !Number.isFinite(max)) {
    return 0
  }

  if (max <= min) {
    return 0
  }

  const percent = ((value - min) / (max - min)) * 100
  return Math.min(100, Math.max(0, percent))
}

const formatPercent = (percent: number): string =>
  Number.isInteger(percent) ? String(percent) : String(Number(percent.toFixed(4)))

export const tableContainer = <Message>(
  input: TableContainerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, tableContainerRoot),
    input.children,
  )
}

export const table = <Message>(input: TableProps<Message>): Html => {
  const h = html<Message>()

  return h.table(basecoatAttrs<Message>(input, tableRoot), input.children)
}

export const tableCaption = <Message>(
  input: TableCaptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.caption(basecoatAttrs<Message>(input), input.children)
}

export const tableHeader = <Message>(
  input: TableSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.thead(basecoatAttrs<Message>(input), input.children)
}

export const tableBody = <Message>(
  input: TableSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.tbody(basecoatAttrs<Message>(input), input.children)
}

export const tableFooter = <Message>(
  input: TableSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.tfoot(basecoatAttrs<Message>(input), input.children)
}

export const tableRow = <Message>(input: TableRowProps<Message>): Html => {
  const h = html<Message>()

  return h.tr(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.state === undefined
        ? []
        : [h.DataAttribute('state', input.state)]),
    ],
    input.children,
  )
}

export const tableHead = <Message>(input: TableHeadProps<Message>): Html => {
  const h = html<Message>()

  return h.th(
    [
      ...basecoatAttrs<Message>(input),
      ...tableSpanAttrs<Message>(input),
      ...(input.scope === undefined ? [] : [h.Attribute('scope', input.scope)]),
    ],
    input.children,
  )
}

export const tableCell = <Message>(input: TableCellProps<Message>): Html => {
  const h = html<Message>()

  return h.td(
    [
      ...basecoatAttrs<Message>(input),
      ...tableSpanAttrs<Message>(input),
    ],
    input.children,
  )
}

export const progress = <Message>(input: ProgressProps<Message>): Html => {
  const h = html<Message>()
  const min = input.min ?? 0
  const max = input.max ?? 100
  const width = `${formatPercent(progressPercent(input.value, min, max))}%`

  return h.div(
    [
      ...basecoatAttrs<Message>(input, progressRoot),
      h.Role('progressbar'),
      h.Attribute('aria-valuenow', String(input.value)),
      h.Attribute('aria-valuemin', String(min)),
      h.Attribute('aria-valuemax', String(max)),
      ...(input.label === undefined ? [] : [h.AriaLabel(input.label)]),
      ...(input.labelledBy === undefined
        ? []
        : [h.Attribute('aria-labelledby', input.labelledBy)]),
    ],
    [
      h.span(
        [
          ...(input.indicatorAttrs ?? []),
          ...(input.indicatorClassName === undefined
            ? []
            : [h.Class(input.indicatorClassName)]),
          h.Style({ width }),
        ],
        [],
      ),
    ],
  )
}
