import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type TooltipSide =
  | 'top'
  | 'right'
  | 'bottom'
  | 'left'
  | 'inline-start'
  | 'inline-end'

export type TooltipAlign = 'start' | 'center' | 'end'

export type TooltipElement = 'span' | 'button' | 'div' | 'a'

export type TooltipProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  tooltip: string
  side?: TooltipSide
  align?: TooltipAlign
  element?: TooltipElement
  href?: string
  type?: 'button' | 'submit' | 'reset'
  disabled?: boolean
  ariaLabel?: string
}>

export type ScrollbarSize = 'default' | 'sm'

export type ScrollbarElement =
  | 'div'
  | 'section'
  | 'article'
  | 'main'
  | 'aside'

export type ScrollbarProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  size?: ScrollbarSize
  element?: ScrollbarElement
  role?: 'region' | 'group'
  ariaLabel?: string
}>

export type CollapsibleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  trigger: BasecoatChildren
  children: BasecoatChildren
  open?: boolean
  summaryAttrs?: ReadonlyArray<Attribute<Message>>
  summaryClassName?: string
  contentAttrs?: ReadonlyArray<Attribute<Message>>
  contentClassName?: string
}>

export type CollapsibleTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type CollapsibleContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

const scrollbarRoot = basecoatClass('scrollbar')
const scrollbarSmRoot = basecoatClass('scrollbar-sm')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

export const tooltip = <Message>(input: TooltipProps<Message>): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>(input),
    h.DataAttribute('tooltip', input.tooltip),
    ...dataAttr<Message>('side', input.side),
    ...dataAttr<Message>('align', input.align),
    ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
  ]

  switch (input.element) {
    case 'button':
      return h.button(
        [
          ...attrs,
          h.Type(input.type ?? 'button'),
          ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
        ],
        input.children,
      )
    case 'div':
      return h.div(attrs, input.children)
    case 'a':
      return h.a(
        [
          ...attrs,
          ...optionalStringAttr<Message>(input.href, h.Href),
        ],
        input.children,
      )
    default:
      return h.span(attrs, input.children)
  }
}

export const scrollbar = <Message>(input: ScrollbarProps<Message>): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>(
      input,
      input.size === 'sm' ? scrollbarSmRoot : scrollbarRoot,
    ),
    ...(input.role === undefined ? [] : [h.Role(input.role)]),
    ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
  ]

  switch (input.element) {
    case 'section':
      return h.section(attrs, input.children)
    case 'article':
      return h.article(attrs, input.children)
    case 'main':
      return h.main(attrs, input.children)
    case 'aside':
      return h.aside(attrs, input.children)
    default:
      return h.div(attrs, input.children)
  }
}

export const collapsibleTrigger = <Message>(
  input: CollapsibleTriggerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.summary(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('slot', 'collapsible-trigger'),
    ],
    input.children,
  )
}

export const collapsibleContent = <Message>(
  input: CollapsibleContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('slot', 'collapsible-content'),
    ],
    input.children,
  )
}

export const collapsible = <Message>(
  input: CollapsibleProps<Message>,
): Html => {
  const h = html<Message>()

  return h.details(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.open === true ? [h.Attribute('open', '')] : []),
    ],
    [
      collapsibleTrigger<Message>({
        attrs: input.summaryAttrs,
        className: input.summaryClassName,
        children: input.trigger,
      }),
      collapsibleContent<Message>({
        attrs: input.contentAttrs,
        className: input.contentClassName,
        children: input.children,
      }),
    ],
  )
}
