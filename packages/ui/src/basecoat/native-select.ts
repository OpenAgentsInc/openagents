import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type NativeSelectSize = 'default' | 'sm'

export type NativeSelectProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  name?: string
  size?: NativeSelectSize
  disabled?: boolean
  required?: boolean
  multiple?: boolean
  invalid?: boolean
  describedBy?: string
  ariaLabel?: string
}>

export type NativeSelectOptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  value?: string
  disabled?: boolean
  selected?: boolean
}>

export type NativeSelectOptgroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  label: string
  disabled?: boolean
}>

const nativeSelectRoot = basecoatClass('select')

export const nativeSelect = <Message>(
  input: NativeSelectProps<Message>,
): Html => {
  const h = html<Message>()

  return h.select(
    [
      ...basecoatAttrs<Message>(input, nativeSelectRoot),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.name === undefined ? [] : [h.Name(input.name)]),
      ...dataAttr<Message>('size', input.size === 'sm' ? 'sm' : undefined),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...(input.required === true ? [h.Required(true)] : []),
      ...(input.multiple === true ? [h.Multiple(true)] : []),
      ...(input.invalid === true ? [h.AriaInvalid(true)] : []),
      ...(input.describedBy === undefined
        ? []
        : [h.AriaDescribedBy(input.describedBy)]),
      ...(input.ariaLabel === undefined ? [] : [h.AriaLabel(input.ariaLabel)]),
    ],
    input.children,
  )
}

export const nativeSelectOption = <Message>(
  input: NativeSelectOptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.option(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.value === undefined ? [] : [h.Value(input.value)]),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...(input.selected === true ? [h.Selected(true)] : []),
    ],
    input.children,
  )
}

export const nativeSelectOptgroup = <Message>(
  input: NativeSelectOptgroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.optgroup(
    [
      ...basecoatAttrs<Message>(input),
      h.LabelAttr(input.label),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
    ],
    input.children,
  )
}
