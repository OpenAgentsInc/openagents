import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type FormMethod = 'get' | 'post' | 'dialog'

export type FormTarget = '_self' | '_blank' | '_parent' | '_top'

export type FormProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  name?: string
  action?: string
  method?: FormMethod
  target?: FormTarget
  ariaLabel?: string
}>

export type FormFieldProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type FormFieldGroupProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type FormSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type FormSectionTitleProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    level?: 2 | 3
  }>

export type FormDescriptionProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    id?: string
  }>

export type FormOptionLabelProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    for?: string
  }>

export type FormSwitchFieldProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type FormSwitchContentProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    disabled?: boolean
  }>

export type FormSwitchLabelProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    for?: string
  }>

const formRoot = basecoatClass('form')
const formLayout = basecoatClass('grid gap-6')
const formFieldLayout = basecoatClass('grid gap-2')
const formFieldGroupLayout = basecoatClass('flex flex-col gap-3')
const formSectionLayout = basecoatClass('grid gap-4')
const formSectionTitleClass = basecoatClass('text-lg font-medium')
const formDescriptionClass = basecoatClass('text-muted-foreground text-sm')
const formOptionLabelClass = basecoatClass('font-normal')
const formSwitchFieldClass = basecoatClass(
  'gap-2 flex flex-row items-start justify-between rounded-lg border p-4 shadow-xs',
)
const formSwitchContentClass = basecoatClass('flex flex-col gap-0.5')
const formSwitchContentDisabledClass = basecoatClass('opacity-60')
const formSwitchLabelClass = basecoatClass('leading-normal')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  value === undefined ? [] : [attr(value)]

export const form = <Message>(input: FormProps<Message>): Html => {
  const h = html<Message>()

  return h.form(
    [
      ...basecoatAttrs<Message>(input, formRoot, formLayout),
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...optionalStringAttr<Message>(input.name, h.Name),
      ...optionalStringAttr<Message>(input.action, h.Action),
      ...(input.method === undefined ? [] : [h.Method(input.method)]),
      ...optionalStringAttr<Message>(input.target, h.Target),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
    ],
    input.children,
  )
}

export const formField = <Message>(
  input: FormFieldProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(basecoatAttrs<Message>(input, formFieldLayout), input.children)
}

export const formFieldGroup = <Message>(
  input: FormFieldGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, formFieldGroupLayout),
    input.children,
  )
}

export const formSection = <Message>(
  input: FormSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(
    basecoatAttrs<Message>(input, formSectionLayout),
    input.children,
  )
}

export const formSectionTitle = <Message>(
  input: FormSectionTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = basecoatAttrs<Message>(input, formSectionTitleClass)

  return input.level === 2
    ? h.h2(attrs, input.children)
    : h.h3(attrs, input.children)
}

export const formDescription = <Message>(
  input: FormDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input, formDescriptionClass),
      ...optionalStringAttr<Message>(input.id, h.Id),
    ],
    input.children,
  )
}

export const formOptionLabel = <Message>(
  input: FormOptionLabelProps<Message>,
): Html => {
  const h = html<Message>()

  return h.label(
    [
      ...basecoatAttrs<Message>(input, formOptionLabelClass),
      ...optionalStringAttr<Message>(input.for, h.For),
    ],
    input.children,
  )
}

export const formSwitchField = <Message>(
  input: FormSwitchFieldProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, formSwitchFieldClass),
    input.children,
  )
}

export const formSwitchContent = <Message>(
  input: FormSwitchContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(
      input,
      formSwitchContentClass,
      input.disabled === true ? formSwitchContentDisabledClass : null,
    ),
    input.children,
  )
}

export const formSwitchLabel = <Message>(
  input: FormSwitchLabelProps<Message>,
): Html => {
  const h = html<Message>()

  return h.label(
    [
      ...basecoatAttrs<Message>(input, formSwitchLabelClass),
      ...optionalStringAttr<Message>(input.for, h.For),
    ],
    input.children,
  )
}
