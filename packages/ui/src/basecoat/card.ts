import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type CardSize = 'default' | 'sm'

export type CardProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  size?: CardSize
}>

export type CardHeaderProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type CardTitleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  level?: 2 | 3
}>

export type CardDescriptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type CardSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type CardFooterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type CardActionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

const cardRoot = basecoatClass('card')
const cardTitleClass = basecoatClass('card-title')
const cardDescriptionClass = basecoatClass('card-description')
const cardActionClass = basecoatClass('card-action')

export const card = <Message>(input: CardProps<Message>): Html => {
  const h = html<Message>()

  return h.article(
    [
      ...basecoatAttrs<Message>(input, cardRoot),
      ...dataAttr<Message>(
        'size',
        input.size === 'sm' ? 'sm' : undefined,
      ),
    ],
    input.children,
  )
}

export const cardHeader = <Message>(
  input: CardHeaderProps<Message>,
): Html => {
  const h = html<Message>()

  return h.header(basecoatAttrs<Message>(input), input.children)
}

export const cardTitle = <Message>(
  input: CardTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = basecoatAttrs<Message>(input, cardTitleClass)

  return input.level === 3
    ? h.h3(attrs, input.children)
    : h.h2(attrs, input.children)
}

export const cardDescription = <Message>(
  input: CardDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(
    basecoatAttrs<Message>(input, cardDescriptionClass),
    input.children,
  )
}

export const cardSection = <Message>(
  input: CardSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const cardFooter = <Message>(
  input: CardFooterProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}

export const cardAction = <Message>(
  input: CardActionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, cardActionClass),
      h.DataAttribute('slot', 'card-action'),
    ],
    input.children,
  )
}
