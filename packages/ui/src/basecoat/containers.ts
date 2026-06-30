import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type ItemVariant = 'default' | 'outline' | 'muted'
export type ItemSize = 'default' | 'sm' | 'xs'
export type ItemElement = 'article' | 'a' | 'span'
export type ItemTitleLevel = 2 | 3 | 4

export type ItemGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  role?: 'list' | 'group'
}>

export type ItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  element?: ItemElement
  href?: string
  rel?: string
  role?: 'listitem' | 'link' | 'menuitem'
  size?: ItemSize
  target?: string
  variant?: ItemVariant
}>

export type ItemMediaProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ItemFigureProps<Message> = ItemMediaProps<Message>

export type ItemContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ItemSectionProps<Message> = ItemContentProps<Message>

export type ItemTitleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  level?: ItemTitleLevel
}>

export type ItemDescriptionProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type ItemAsideProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ItemHeaderProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ItemFooterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ItemSeparatorProps<Message> = BasecoatAttrs<Message>

export type EmptyTitleLevel = 2 | 3 | 4

export type EmptyProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type EmptyHeaderProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type EmptyMediaProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type EmptyFigureProps<Message> = EmptyMediaProps<Message>

export type EmptyTitleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  level?: EmptyTitleLevel
}>

export type EmptyDescriptionProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type EmptyContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type EmptySectionProps<Message> = EmptyContentProps<Message>

export type EmptyFooterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

const itemGroupRoot = basecoatClass('item-group')
const itemRoot = basecoatClass('item')
const emptyRoot = basecoatClass('empty')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

export const itemGroup = <Message>(
  input: ItemGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, itemGroupRoot),
      h.Role(input.role ?? 'list'),
    ],
    input.children,
  )
}

export const item = <Message>(input: ItemProps<Message>): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>(input, itemRoot),
    ...dataAttr<Message>(
      'variant',
      input.variant === 'default' ? undefined : input.variant,
    ),
    ...dataAttr<Message>(
      'size',
      input.size === 'default' ? undefined : input.size,
    ),
    ...(input.role === undefined ? [] : [h.Role(input.role)]),
  ]

  if (input.href !== undefined || input.element === 'a') {
    return h.a(
      [
        ...attrs,
        ...optionalStringAttr<Message>(input.href, h.Href),
        ...optionalStringAttr<Message>(input.target, h.Target),
        ...optionalStringAttr<Message>(input.rel, h.Rel),
      ],
      input.children,
    )
  }

  return input.element === 'span'
    ? h.span(attrs, input.children)
    : h.article(attrs, input.children)
}

export const itemMedia = <Message>(
  input: ItemMediaProps<Message>,
): Html => {
  const h = html<Message>()

  return h.figure(basecoatAttrs<Message>(input), input.children)
}

export const itemFigure = itemMedia

export const itemContent = <Message>(
  input: ItemContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const itemSection = itemContent

export const itemTitle = <Message>(
  input: ItemTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = basecoatAttrs<Message>(input)

  switch (input.level) {
    case 2:
      return h.h2(attrs, input.children)
    case 4:
      return h.h4(attrs, input.children)
    case 3:
    default:
      return h.h3(attrs, input.children)
  }
}

export const itemDescription = <Message>(
  input: ItemDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(basecoatAttrs<Message>(input), input.children)
}

export const itemAside = <Message>(
  input: ItemAsideProps<Message>,
): Html => {
  const h = html<Message>()

  return h.aside(basecoatAttrs<Message>(input), input.children)
}

export const itemHeader = <Message>(
  input: ItemHeaderProps<Message>,
): Html => {
  const h = html<Message>()

  return h.header(basecoatAttrs<Message>(input), input.children)
}

export const itemFooter = <Message>(
  input: ItemFooterProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}

export const itemSeparator = <Message>(
  input: ItemSeparatorProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.hr(basecoatAttrs<Message>(input))
}

export const empty = <Message>(input: EmptyProps<Message>): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input, emptyRoot), input.children)
}

export const emptyHeader = <Message>(
  input: EmptyHeaderProps<Message>,
): Html => {
  const h = html<Message>()

  return h.header(basecoatAttrs<Message>(input), input.children)
}

export const emptyMedia = <Message>(
  input: EmptyMediaProps<Message>,
): Html => {
  const h = html<Message>()

  return h.figure(basecoatAttrs<Message>(input), input.children)
}

export const emptyFigure = emptyMedia

export const emptyTitle = <Message>(
  input: EmptyTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = basecoatAttrs<Message>(input)

  switch (input.level) {
    case 2:
      return h.h2(attrs, input.children)
    case 4:
      return h.h4(attrs, input.children)
    case 3:
    default:
      return h.h3(attrs, input.children)
  }
}

export const emptyDescription = <Message>(
  input: EmptyDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(basecoatAttrs<Message>(input), input.children)
}

export const emptyContent = <Message>(
  input: EmptyContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const emptySection = emptyContent

export const emptyFooter = <Message>(
  input: EmptyFooterProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}
