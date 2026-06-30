import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  classAttrs,
  componentClass,
  type MaybeComponentClass,
} from '../class-foldkit'

export type BasecoatChildren = ReadonlyArray<string | Html | null>

export type BasecoatAttrs<Message> = Readonly<{
  attrs?: ReadonlyArray<Attribute<Message>> | undefined
  className?: string | undefined
}>

export const basecoatClass = (className: string) => componentClass(className)

export const variantClass = <Variant extends string>(
  variants: Readonly<Record<Variant, MaybeComponentClass>>,
  variant: Variant | undefined,
): MaybeComponentClass =>
  variant === undefined ? null : variants[variant]

export const basecoatAttrs = <Message>(
  input: BasecoatAttrs<Message>,
  ...classes: ReadonlyArray<MaybeComponentClass>
): ReadonlyArray<Attribute<Message>> => [
  ...(input.attrs ?? []),
  ...classAttrs<Message>(...classes, input.className),
]

export const dataAttr = <Message>(
  key: string,
  value: string | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (value === undefined) {
    return []
  }

  return [html<Message>().DataAttribute(key, value)]
}
