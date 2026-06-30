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

export type InputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  name?: string
  type?: InputType
  value?: string
  placeholder?: string
  disabled?: boolean
  required?: boolean
  checked?: boolean
  readonly?: boolean
}>

export type TextareaProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  name?: string
  value?: string
  placeholder?: string
  rows?: number
  disabled?: boolean
  required?: boolean
  readonly?: boolean
}>

export type LabelProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  for?: string
}>

export type FieldOrientation = 'horizontal' | 'responsive'

export type FieldProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  orientation?: FieldOrientation
  invalid?: boolean
  role?: 'group' | 'radiogroup'
}>

export type FieldsetProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  role?: 'group' | 'radiogroup'
}>

export type FieldSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type FieldDescriptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  alert?: boolean
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

export type InputGroupAddonElement = 'span' | 'header' | 'footer'

export type InputGroupAddonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  align?: InputGroupAddonAlign
  element?: InputGroupAddonElement
  hidden?: boolean
  role?: 'group' | 'status'
}>

const inputRoot = basecoatClass('input')
const textareaRoot = basecoatClass('textarea')
const labelRoot = basecoatClass('label')
const fieldRoot = basecoatClass('field')
const fieldsetRoot = basecoatClass('fieldset')
const fieldSeparatorRoot = basecoatClass('field-separator')
const inputGroupRoot = basecoatClass('input-group')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

export const input = <Message>(input: InputProps<Message>): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input, inputRoot),
    ...optionalStringAttr<Message>(input.id, h.Id),
    ...optionalStringAttr<Message>(input.name, h.Name),
    h.Type(input.type ?? 'text'),
    ...optionalStringAttr<Message>(input.value, h.Value),
    ...optionalStringAttr<Message>(input.placeholder, h.Placeholder),
    ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
    ...optionalBooleanAttr<Message>(input.required, h.Required),
    ...optionalBooleanAttr<Message>(input.checked, h.Checked),
    ...optionalBooleanAttr<Message>(input.readonly, h.Readonly),
  ])
}

export const textarea = <Message>(input: TextareaProps<Message>): Html => {
  const h = html<Message>()

  return h.textarea(
    [
      ...basecoatAttrs<Message>(input, textareaRoot),
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...optionalStringAttr<Message>(input.name, h.Name),
      ...optionalStringAttr<Message>(input.placeholder, h.Placeholder),
      ...(input.rows === undefined ? [] : [h.Rows(input.rows)]),
      ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
      ...optionalBooleanAttr<Message>(input.required, h.Required),
      ...optionalBooleanAttr<Message>(input.readonly, h.Readonly),
    ],
    [input.value ?? ''],
  )
}

export const label = <Message>(input: LabelProps<Message>): Html => {
  const h = html<Message>()

  return h.label(
    [
      ...basecoatAttrs<Message>(input, labelRoot),
      ...optionalStringAttr<Message>(input.for, h.For),
    ],
    input.children,
  )
}

export const field = <Message>(input: FieldProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, fieldRoot),
      h.Role(input.role ?? 'group'),
      ...dataAttr<Message>('orientation', input.orientation),
      ...(input.invalid === true ? [h.DataAttribute('invalid', 'true')] : []),
    ],
    input.children,
  )
}

export const fieldset = <Message>(input: FieldsetProps<Message>): Html => {
  const h = html<Message>()
  const roleAttrs =
    input.role === undefined ? [] : [h.Role(input.role)]

  return input.role === undefined
    ? h.fieldset(
        basecoatAttrs<Message>(input, fieldsetRoot),
        input.children,
      )
    : h.div(
        [
          ...basecoatAttrs<Message>(input, fieldsetRoot),
          ...roleAttrs,
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
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...(input.alert === true ? [h.Role('alert')] : []),
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
    input.children ?? [h.hr([h.Role('separator')])],
  )
}

export const inputGroup = <Message>(
  input: InputGroupProps<Message>,
): Html => {
  const h = html<Message>()
  const roleAttrs =
    input.role === undefined ? [] : [h.Role(input.role)]

  return h.div(
    [
      ...basecoatAttrs<Message>(input, inputGroupRoot),
      ...roleAttrs,
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
    ...optionalBooleanAttr<Message>(input.hidden, h.AriaHidden),
    ...(input.role === undefined ? [] : [h.Role(input.role)]),
  ]

  switch (input.element) {
    case 'header':
      return h.header(attrs, input.children)
    case 'footer':
      return h.footer(attrs, input.children)
    default:
      return h.span(attrs, input.children)
  }
}
