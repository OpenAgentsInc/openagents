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
  | 'text'
  | 'email'
  | 'password'
  | 'number'
  | 'file'
  | 'tel'
  | 'url'
  | 'search'
  | 'date'
  | 'datetime-local'
  | 'month'
  | 'week'
  | 'time'
  | 'checkbox'
  | 'radio'
  | 'range'

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
}>

export type FieldLegendVariant = 'legend' | 'label'

export type FieldLegendProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  variant?: FieldLegendVariant
}>

export type FieldTextProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
}>

export type FieldErrorProps<Message> = FieldTextProps<Message>

export type FieldSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type FieldSeparatorProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children?: BasecoatChildren
}>

export type InputGroupOrientation = 'vertical'

export type InputGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html | string | null>
  orientation?: InputGroupOrientation
}>

export type InputGroupAlign =
  | 'start'
  | 'end'
  | 'inline-start'
  | 'inline-end'
  | 'block-start'
  | 'block-end'

export type InputGroupAddonElement = 'span' | 'header' | 'footer' | 'div'

export type InputGroupAddonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  align?: InputGroupAlign
  ariaHidden?: boolean
  as?: InputGroupAddonElement
  children: BasecoatChildren
  role?: string
}>

const inputRoot = basecoatClass('input')
const textareaRoot = basecoatClass('textarea')
const labelRoot = basecoatClass('label')
const fieldRoot = basecoatClass('field')
const fieldsetRoot = basecoatClass('fieldset')
const fieldSeparatorRoot = basecoatClass('field-separator')
const inputGroupRoot = basecoatClass('input-group')

const idAttr = <Message>(
  h: ReturnType<typeof html<Message>>,
  id: string | undefined,
): ReadonlyArray<Attribute<Message>> =>
  id === undefined ? [] : [h.Id(id)]

const nameAttr = <Message>(
  h: ReturnType<typeof html<Message>>,
  name: string | undefined,
): ReadonlyArray<Attribute<Message>> =>
  name === undefined ? [] : [h.Name(name)]

const placeholderAttr = <Message>(
  h: ReturnType<typeof html<Message>>,
  placeholder: string | undefined,
): ReadonlyArray<Attribute<Message>> =>
  placeholder === undefined ? [] : [h.Placeholder(placeholder)]

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
    ...(input.readonly === true ? [h.Attribute('readonly', '')] : []),
    ...(input.required === true ? [h.Required(true)] : []),
  ]
}

export const input = <Message>(input: InputProps<Message>): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input, inputRoot),
    ...idAttr(h, input.id),
    ...nameAttr(h, input.name),
    ...(input.type === undefined ? [] : [h.Type(input.type)]),
    ...(input.value === undefined ? [] : [h.Value(input.value)]),
    ...placeholderAttr(h, input.placeholder),
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
      ...idAttr(h, input.id),
      ...nameAttr(h, input.name),
      ...placeholderAttr(h, input.placeholder),
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

  return h.label(
    [
      ...basecoatAttrs<Message>(input, labelRoot),
      ...(input.htmlFor === undefined ? [] : [h.For(input.htmlFor)]),
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

  return h.fieldset(
    basecoatAttrs<Message>(input, fieldsetRoot),
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

export const fieldDescription = <Message>(
  input: FieldTextProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input),
      ...idAttr(h, input.id),
    ],
    input.children,
  )
}

export const fieldError = <Message>(input: FieldErrorProps<Message>): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input),
      ...idAttr(h, input.id),
      h.Role('alert'),
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

export const fieldSeparator = <Message>(
  input: FieldSeparatorProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, fieldSeparatorRoot),
    [
      h.hr([h.Role('separator')]),
      input.children === undefined
        ? null
        : h.span([], input.children),
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
    ...(input.ariaHidden === true ? [h.AriaHidden(true)] : []),
    ...(input.role === undefined ? [] : [h.Role(input.role)]),
  ]

  switch (input.as) {
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
