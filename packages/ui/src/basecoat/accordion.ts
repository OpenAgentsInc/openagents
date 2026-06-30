import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type AccordionType = 'single' | 'multiple'

export type AccordionItem = Readonly<{
  value: string
  disabled?: boolean
}>

export type AccordionModel = Readonly<{
  type: AccordionType
  items: ReadonlyArray<AccordionItem>
  openValues: ReadonlyArray<string>
  focusedValue: string | null
  selectedValue: string | null
}>

export type AccordionInit = Readonly<{
  type?: AccordionType
  items: ReadonlyArray<AccordionItem>
  defaultOpenValues?: ReadonlyArray<string>
  focusedValue?: string | null
  selectedValue?: string | null
}>

export type AccordionMessage =
  | Readonly<{ readonly _tag: 'AccordionToggled'; readonly value: string; readonly open: boolean }>
  | Readonly<{ readonly _tag: 'AccordionFocused'; readonly value: string }>
  | Readonly<{ readonly _tag: 'AccordionSelected'; readonly value: string }>
  | Readonly<{ readonly _tag: 'AccordionKeyDown'; readonly key: string; readonly value: string }>

export type AccordionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  type?: AccordionType
}>

export type AccordionItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  value: string
  open?: boolean
  disabled?: boolean
  children: ReadonlyArray<Html>
  onToggle?: (open: boolean) => Message
}>

export type AccordionTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  itemValue: string
  open?: boolean
  disabled?: boolean
  controlsId?: string
  tabIndex?: number
  onClick?: Message
  onFocus?: Message
  onKeyDown?: (key: string) => Message | null
}>

export type AccordionContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  labelledBy?: string
  hidden?: boolean
}>

export type AccordionViewItem<Message> = AccordionItem & Readonly<{
  trigger: BasecoatChildren
  content: BasecoatChildren
  contentId?: string
  triggerId?: string
}>

export type AccordionViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: AccordionModel
  items: ReadonlyArray<AccordionViewItem<Message>>
  toMessage: (message: AccordionMessage) => Message
}>

const accordionRoot = basecoatClass('accordion')

const enabledItems = (model: AccordionModel): ReadonlyArray<AccordionItem> =>
  model.items.filter(item => item.disabled !== true)

const hasItem = (items: ReadonlyArray<AccordionItem>, value: string): boolean =>
  items.some(item => item.value === value)

const isDisabled = (model: AccordionModel, value: string): boolean =>
  model.items.find(item => item.value === value)?.disabled === true

const normalizeOpenValues = (
  type: AccordionType,
  items: ReadonlyArray<AccordionItem>,
  openValues: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const allowed = new Set(items.filter(item => item.disabled !== true).map(item => item.value))
  const next = openValues.filter((value, index) =>
    allowed.has(value) && openValues.indexOf(value) === index
  )

  return type === 'single' ? next.slice(0, 1) : next
}

export const accordionInit = (input: AccordionInit): AccordionModel => {
  const type = input.type ?? 'single'
  const openValues = normalizeOpenValues(
    type,
    input.items,
    input.defaultOpenValues ?? [],
  )
  const firstEnabled = input.items.find(item => item.disabled !== true)?.value ?? null
  const focusedValue =
    input.focusedValue !== undefined &&
    input.focusedValue !== null &&
    hasItem(input.items, input.focusedValue) &&
    !isDisabled({ type, items: input.items, openValues, focusedValue: null, selectedValue: null }, input.focusedValue)
      ? input.focusedValue
      : firstEnabled
  const selectedValue =
    input.selectedValue !== undefined && input.selectedValue !== null
      ? input.selectedValue
      : openValues[0] ?? null

  return {
    type,
    items: input.items,
    openValues,
    focusedValue,
    selectedValue,
  }
}

const setOpenValue = (
  model: AccordionModel,
  value: string,
  open: boolean,
): AccordionModel => {
  if (isDisabled(model, value) || !hasItem(model.items, value)) {
    return model
  }

  const openValues = open
    ? model.type === 'single'
      ? [value]
      : normalizeOpenValues(model.type, model.items, [...model.openValues, value])
    : model.openValues.filter(candidate => candidate !== value)

  return {
    ...model,
    openValues,
    selectedValue: open ? value : model.selectedValue,
  }
}

const focusByOffset = (model: AccordionModel, value: string, offset: number): AccordionModel => {
  const items = enabledItems(model)
  const index = items.findIndex(item => item.value === value)

  if (items.length === 0 || index === -1) {
    return model
  }

  const next = items[(index + offset + items.length) % items.length]

  return { ...model, focusedValue: next?.value ?? model.focusedValue }
}

export const accordionUpdate = (
  model: AccordionModel,
  message: AccordionMessage,
): AccordionModel => {
  switch (message._tag) {
    case 'AccordionToggled':
      return setOpenValue(model, message.value, message.open)
    case 'AccordionFocused':
      return isDisabled(model, message.value) || !hasItem(model.items, message.value)
        ? model
        : { ...model, focusedValue: message.value }
    case 'AccordionSelected':
      return setOpenValue(model, message.value, !model.openValues.includes(message.value))
    case 'AccordionKeyDown': {
      if (isDisabled(model, message.value)) {
        return model
      }

      switch (message.key) {
        case 'ArrowDown':
        case 'ArrowRight':
          return focusByOffset(model, message.value, 1)
        case 'ArrowUp':
        case 'ArrowLeft':
          return focusByOffset(model, message.value, -1)
        case 'Home': {
          const first = enabledItems(model)[0]?.value
          return first === undefined ? model : { ...model, focusedValue: first }
        }
        case 'End': {
          const items = enabledItems(model)
          const last = items[items.length - 1]?.value
          return last === undefined ? model : { ...model, focusedValue: last }
        }
        case 'Enter':
        case ' ':
        case 'Spacebar':
          return setOpenValue(
            { ...model, focusedValue: message.value },
            message.value,
            !model.openValues.includes(message.value),
          )
        default:
          return model
      }
    }
  }
}

export const accordion = <Message>(input: AccordionProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, accordionRoot),
      ...dataAttr<Message>('multiple', input.type === 'multiple' ? 'true' : undefined),
    ],
    input.children,
  )
}

export const accordionItem = <Message>(
  input: AccordionItemProps<Message>,
): Html => {
  const h = html<Message>()

  return h.details(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('value', input.value),
      ...(input.open === true ? [h.Open(true)] : []),
      ...(input.disabled === true ? [h.AriaDisabled(true)] : []),
      ...(input.disabled === true ? [h.DataAttribute('disabled', 'true')] : []),
      ...(input.onToggle === undefined ? [] : [h.OnToggle(input.onToggle)]),
    ],
    input.children,
  )
}

const chevronDown = <Message>(): Html => {
  const h = html<Message>()

  return h.svg(
    [
      h.Width('16'),
      h.Height('16'),
      h.ViewBox('0 0 24 24'),
      h.Fill('none'),
      h.Stroke('currentColor'),
      h.StrokeWidth('2'),
      h.StrokeLinecap('round'),
      h.StrokeLinejoin('round'),
      h.AriaHidden(true),
    ],
    [h.path([h.D('m6 9 6 6 6-6')], [])],
  )
}

const handledAccordionKeys = new Set([
  'ArrowDown',
  'ArrowRight',
  'ArrowUp',
  'ArrowLeft',
  'Home',
  'End',
  'Enter',
  ' ',
  'Spacebar',
])

export const accordionTrigger = <Message>(
  input: AccordionTriggerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.summary(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('value', input.itemValue),
      h.AriaExpanded(input.open === true),
      ...(input.controlsId === undefined ? [] : [h.AriaControls(input.controlsId)]),
      ...(input.disabled === true ? [h.AriaDisabled(true)] : []),
      ...(input.tabIndex === undefined ? [] : [h.Tabindex(input.tabIndex)]),
      ...(input.onClick === undefined || input.disabled === true
        ? []
        : [h.OnClick(input.onClick)]),
      ...(input.onFocus === undefined || input.disabled === true
        ? []
        : [h.OnFocus(input.onFocus)]),
      ...(input.onKeyDown === undefined
        ? []
        : [
            h.OnKeyDownPreventDefault(key =>
              handledAccordionKeys.has(key)
                ? (() => {
                    const next = input.onKeyDown?.(key) ?? null
                    return next === null ? Option.none() : Option.some(next)
                  })()
                : Option.none(),
            ),
          ]),
    ],
    [...input.children, chevronDown<Message>()],
  )
}

export const accordionContent = <Message>(
  input: AccordionContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.labelledBy === undefined ? [] : [h.AriaLabelledBy(input.labelledBy)]),
      ...(input.hidden === true ? [h.Hidden(true)] : []),
    ],
    input.children,
  )
}

export const accordionView = <Message>(
  input: AccordionViewProps<Message>,
): Html => {
  const items = input.items.map((item) => {
    const open = input.model.openValues.includes(item.value)
    const triggerId = item.triggerId ?? `${item.value}-trigger`
    const contentId = item.contentId ?? `${item.value}-content`
    const triggerInput: AccordionTriggerProps<Message> = {
      itemValue: item.value,
      open,
      controlsId: contentId,
      tabIndex: input.model.focusedValue === item.value ? 0 : -1,
      onClick: input.toMessage({
        _tag: 'AccordionSelected',
        value: item.value,
      }),
      onFocus: input.toMessage({
        _tag: 'AccordionFocused',
        value: item.value,
      }),
      onKeyDown: key =>
        input.toMessage({
          _tag: 'AccordionKeyDown',
          key,
          value: item.value,
        }),
      attrs: [html<Message>().Id(triggerId)],
      children: item.trigger,
      ...(item.disabled === undefined ? {} : { disabled: item.disabled }),
    }

    return accordionItem<Message>({
      value: item.value,
      open,
      onToggle: toggledOpen =>
        input.toMessage({
          _tag: 'AccordionToggled',
          value: item.value,
          open: toggledOpen,
        }),
      children: [
        accordionTrigger<Message>(triggerInput),
        accordionContent<Message>({
          id: contentId,
          labelledBy: triggerId,
          hidden: !open,
          children: item.content,
        }),
      ],
      ...(item.disabled === undefined ? {} : { disabled: item.disabled }),
    })
  })

  const rootInput: AccordionProps<Message> = {
    type: input.model.type,
    children: items,
  }

  return accordion<Message>({
    ...rootInput,
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
  })
}
