import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type BadgeVariant =
  | 'primary'
  | 'secondary'
  | 'outline'
  | 'destructive'
  | 'ghost'
  | 'link'

export type BadgeProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  variant?: BadgeVariant
}>

const badgeRoot = basecoatClass('badge')

export const badge = <Message>(input: BadgeProps<Message>): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input, badgeRoot),
      ...dataAttr<Message>('variant', input.variant),
    ],
    input.children,
  )
}
