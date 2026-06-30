import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type InputType =
  | 'button'
  | 'checkbox'
  | 'color'
  | 'date'
  | 'datetime-local'
  | 'email'
  | 'file'
  | 'hidden'
  | 'month'
  | 'number'
  | 'password'
  | 'radio'
  | 'range'
  | 'search'
  | 'submit'
  | 'tel'
  | 'text'
  | 'time'
  | 'url'
  | 'week'

export type InputMode =
  | 'none'
  | 'text'
  | 'decimal'
  | 'numeric'
  | 'tel'
  | 'search'
  | 'email'
  | 'url'

export type InputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  autocomplete?: string
  checked?: boolean
  control?: boolean
  describedBy?: string
  disabled?: boolean
  id?: string
  inputMode?: InputMode
  invalid?: boolean
  name?: string
  placeholder?: string
  readonly?: boolean
  required?: boolean
  type?: InputType
  value?: string
}>

export type TextareaProps<Message> = BasecoatAttrs<Message> & Readonly<{
  autocomplete?: string
  control?: boolean
  describedBy?: string
  disabled?: boolean
  id?: string
  inputMode?: InputMode
  invalid?: boolean
  name?: string
  placeholder?: string
  readonly?: boolean
  required?: boolean
  rows?: number
  value?: string
}>

export type LabelProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  for?: string
  htmlFor?: string
}>

export type FieldOrientation = 'horizontal' | 'responsive'

export type FieldProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  invalid?: boolean
  orientation?: FieldOrientation
  role?: 'group' | 'radiogroup' | false
}>

export type FieldsetProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  role?: 'group' | 'radiogroup'
}>

export type FieldLegendVariant = 'legend' | 'label'

export type FieldLegendProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  variant?: FieldLegendVariant
}>

export type FieldSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type FieldDescriptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  alert?: boolean
  children: BasecoatChildren
  id?: string
}>

export type FieldErrorProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
}>

export type FieldSeparatorProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children?: BasecoatChildren
}>

export type InputGroupOrientation = 'vertical'

export type InputGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  orientation?: InputGroupOrientation
  role?: 'group'
}>

export type InputGroupAddonAlign =
  | 'start'
  | 'end'
  | 'inline-start'
  | 'inline-end'
  | 'block-start'
  | 'block-end'

export type InputGroupAlign = InputGroupAddonAlign

export type InputGroupAddonElement = 'span' | 'header' | 'footer' | 'div'

export type InputGroupAddonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  align?: InputGroupAddonAlign
  ariaHidden?: boolean
  as?: InputGroupAddonElement
  children: BasecoatChildren
  element?: InputGroupAddonElement
  hidden?: boolean
  role?: string
}>

const inputRoot = basecoatClass('input')
const textareaRoot = basecoatClass('textarea')
const labelRoot = basecoatClass('label')
const fieldRoot = basecoatClass('field')
const fieldsetRoot = basecoatClass('fieldset')
const fieldSeparatorRoot = basecoatClass('field-separator')
const inputGroupRoot = basecoatClass('input-group')

const optionalAttr = <Message>(
  h: ReturnType<typeof html<Message>>,
  key: string,
  value: string | undefined,
): ReadonlyArray<Attribute<Message>> =>
  value === undefined ? [] : [h.Attribute(key, value)]

const booleanAttrs = <Message>(
  input: Readonly<{
    checked?: boolean
    disabled?: boolean
    invalid?: boolean
    readonly?: boolean
    required?: boolean
  }>,
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()

  return [
    ...(input.checked === true ? [h.Checked(true)] : []),
    ...(input.disabled === true ? [h.Disabled(true)] : []),
    ...(input.invalid === true ? [h.Attribute('aria-invalid', 'true')] : []),
    ...(input.readonly === true ? [h.Readonly(true)] : []),
    ...(input.required === true ? [h.Required(true)] : []),
  ]
}

export const input = <Message>(input: InputProps<Message>): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input, inputRoot),
    ...optionalAttr(h, 'id', input.id),
    ...optionalAttr(h, 'name', input.name),
    h.Type(input.type ?? 'text'),
    ...(input.value === undefined ? [] : [h.Value(input.value)]),
    ...(input.placeholder === undefined ? [] : [h.Placeholder(input.placeholder)]),
    ...optionalAttr(h, 'aria-describedby', input.describedBy),
    ...optionalAttr(h, 'autocomplete', input.autocomplete),
    ...optionalAttr(h, 'inputmode', input.inputMode),
    ...(input.control === true ? [h.DataAttribute('control', '')] : []),
    ...booleanAttrs<Message>(input),
  ])
}

export const textarea = <Message>(input: TextareaProps<Message>): Html => {
  const h = html<Message>()

  return h.textarea(
    [
      ...basecoatAttrs<Message>(input, textareaRoot),
      ...optionalAttr(h, 'id', input.id),
      ...optionalAttr(h, 'name', input.name),
      ...(input.placeholder === undefined ? [] : [h.Placeholder(input.placeholder)]),
      ...(input.rows === undefined ? [] : [h.Rows(input.rows)]),
      ...optionalAttr(h, 'aria-describedby', input.describedBy),
      ...optionalAttr(h, 'autocomplete', input.autocomplete),
      ...optionalAttr(h, 'inputmode', input.inputMode),
      ...(input.control === true ? [h.DataAttribute('control', '')] : []),
      ...booleanAttrs<Message>(input),
    ],
    [input.value ?? ''],
  )
}

export const label = <Message>(input: LabelProps<Message>): Html => {
  const h = html<Message>()
  const forValue = input.htmlFor ?? input.for

  return h.label(
    [
      ...basecoatAttrs<Message>(input, labelRoot),
      ...(forValue === undefined ? [] : [h.For(forValue)]),
    ],
    input.children,
  )
}

export const field = <Message>(input: FieldProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, fieldRoot),
      ...(input.role === false ? [] : [h.Role(input.role ?? 'group')]),
      ...dataAttr<Message>('orientation', input.orientation),
      ...(input.invalid === true ? [h.DataAttribute('invalid', 'true')] : []),
    ],
    input.children,
  )
}

export const fieldset = <Message>(input: FieldsetProps<Message>): Html => {
  const h = html<Message>()

  return input.role === undefined
    ? h.fieldset(
        basecoatAttrs<Message>(input, fieldsetRoot),
        input.children,
      )
    : h.div(
        [
          ...basecoatAttrs<Message>(input, fieldsetRoot),
          h.Role(input.role),
        ],
        input.children,
      )
}

export const fieldLegend = <Message>(
  input: FieldLegendProps<Message>,
): Html => {
  const h = html<Message>()

  return h.legend(
    [
      ...basecoatAttrs<Message>(input),
      ...dataAttr<Message>('variant', input.variant),
    ],
    input.children,
  )
}

export const fieldSection = <Message>(
  input: FieldSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const fieldDescription = <Message>(
  input: FieldDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.alert === true ? [h.Role('alert')] : []),
      ...optionalAttr(h, 'id', input.id),
    ],
    input.children,
  )
}

export const fieldError = <Message>(input: FieldErrorProps<Message>): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('alert'),
      ...optionalAttr(h, 'id', input.id),
    ],
    input.children,
  )
}

export const fieldSeparator = <Message>(
  input: FieldSeparatorProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, fieldSeparatorRoot),
    input.children === undefined
      ? [h.hr([h.Role('separator')])]
      : [
          h.hr([h.Role('separator')]),
          h.span([], input.children),
        ],
  )
}

export const inputGroup = <Message>(
  input: InputGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, inputGroupRoot),
      ...(input.role === undefined ? [] : [h.Role(input.role)]),
      ...dataAttr<Message>('orientation', input.orientation),
    ],
    input.children,
  )
}

export const inputGroupAddon = <Message>(
  input: InputGroupAddonProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>(input),
    ...dataAttr<Message>('align', input.align),
    ...(input.ariaHidden === true || input.hidden === true
      ? [h.AriaHidden(true)]
      : []),
    ...(input.role === undefined ? [] : [h.Role(input.role)]),
  ]

  switch (input.as ?? input.element) {
    case 'header':
      return h.header(attrs, input.children)
    case 'footer':
      return h.footer(attrs, input.children)
    case 'div':
      return h.div(attrs, input.children)
    default:
      return h.span(attrs, input.children)
  }
}
