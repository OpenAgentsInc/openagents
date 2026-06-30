import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
} from './shared'

export type SelectionInputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  name?: string
  value?: string
  checked?: boolean
  disabled?: boolean
  required?: boolean
  invalid?: boolean
  describedBy?: string
  ariaLabel?: string
}>

export type CheckboxProps<Message> = SelectionInputProps<Message>

export type RadioProps<Message> = SelectionInputProps<Message>

export type SwitchSize = 'default' | 'sm'

export type SwitchProps<Message> = SelectionInputProps<Message> & Readonly<{
  size?: SwitchSize
}>

export type CheckboxGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  ariaLabel?: string
}>

export type RadioGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  ariaLabel?: string
}>

const inputRoot = basecoatClass('input')

const nativeSelectionAttrs = <Message>(
  input: SelectionInputProps<Message>,
  type: 'checkbox' | 'radio',
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()

  return [
    ...basecoatAttrs<Message>(input, inputRoot),
    h.Type(type),
    ...(input.id === undefined ? [] : [h.Id(input.id)]),
    ...(input.name === undefined ? [] : [h.Name(input.name)]),
    ...(input.value === undefined ? [] : [h.Value(input.value)]),
    ...(input.checked === undefined ? [] : [h.Checked(input.checked)]),
    ...(input.disabled === true ? [h.Disabled(true)] : []),
    ...(input.required === true ? [h.Required(true)] : []),
    ...(input.invalid === true ? [h.AriaInvalid(true)] : []),
    ...(input.describedBy === undefined
      ? []
      : [h.AriaDescribedBy(input.describedBy)]),
    ...(input.ariaLabel === undefined ? [] : [h.AriaLabel(input.ariaLabel)]),
  ]
}

export const checkbox = <Message>(input: CheckboxProps<Message>): Html => {
  const h = html<Message>()

  return h.input(nativeSelectionAttrs<Message>(input, 'checkbox'))
}

export const radio = <Message>(input: RadioProps<Message>): Html => {
  const h = html<Message>()

  return h.input(nativeSelectionAttrs<Message>(input, 'radio'))
}

export const switchControl = <Message>(input: SwitchProps<Message>): Html => {
  const h = html<Message>()

  return h.input([
    ...nativeSelectionAttrs<Message>(input, 'checkbox'),
    h.Role('switch'),
    ...dataAttr<Message>('size', input.size === 'sm' ? 'sm' : undefined),
  ])
}

export { switchControl as switch }

export const checkboxGroup = <Message>(
  input: CheckboxGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('group'),
      h.DataAttribute('slot', 'checkbox-group'),
      ...(input.ariaLabel === undefined ? [] : [h.AriaLabel(input.ariaLabel)]),
    ],
    input.children,
  )
}

export const radioGroup = <Message>(input: RadioGroupProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('radiogroup'),
      h.DataAttribute('slot', 'radio-group'),
      ...(input.ariaLabel === undefined ? [] : [h.AriaLabel(input.ariaLabel)]),
    ],
    input.children,
  )
}
