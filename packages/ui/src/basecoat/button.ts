import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type ButtonVariant =
  | 'primary'
  | 'secondary'
  | 'outline'
  | 'ghost'
  | 'link'
  | 'destructive'

export type ButtonSize =
  | 'default'
  | 'xs'
  | 'sm'
  | 'lg'
  | 'icon'
  | 'icon-xs'
  | 'icon-sm'
  | 'icon-lg'

export type ButtonType = 'button' | 'submit' | 'reset'

export type ButtonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  variant?: ButtonVariant
  size?: ButtonSize
  type?: ButtonType
  disabled?: boolean
}>

export type ButtonGroupOrientation = 'horizontal' | 'vertical'

export type ButtonGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  orientation?: ButtonGroupOrientation
}>

export type KbdProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

const buttonRoot = basecoatClass('btn')
const buttonGroupRoot = basecoatClass('button-group')
const kbdRoot = basecoatClass('kbd')

export const button = <Message>(input: ButtonProps<Message>): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input, buttonRoot),
      h.Type(input.type ?? 'button'),
      ...dataAttr<Message>('variant', input.variant),
      ...dataAttr<Message>('size', input.size),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
    ],
    input.children,
  )
}

export const buttonGroup = <Message>(
  input: ButtonGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, buttonGroupRoot),
      ...dataAttr<Message>(
        'orientation',
        input.orientation === 'vertical' ? 'vertical' : undefined,
      ),
    ],
    input.children,
  )
}

export const kbd = <Message>(input: KbdProps<Message>): Html => {
  const h = html<Message>()

  return h.kbd(
    basecoatAttrs<Message>(input, kbdRoot),
    input.children,
  )
}
