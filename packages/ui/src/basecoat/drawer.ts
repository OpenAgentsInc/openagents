import type { Attribute, Html, KeyboardModifiers } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type DrawerSide = 'top' | 'right' | 'bottom' | 'left'

export type DrawerModel = Readonly<{
  open: boolean
  closing: boolean
  pointerStartedOnBackdrop: boolean
  focusedIndex: number
  selectedValue?: string
}>

export type DrawerMessage =
  | Readonly<{ _tag: 'RequestedOpen' }>
  | Readonly<{ _tag: 'RequestedClose' }>
  | Readonly<{ _tag: 'CompletedClose' }>
  | Readonly<{ _tag: 'PressedBackdropPointer' }>
  | Readonly<{ _tag: 'ReleasedBackdropPointer' }>
  | Readonly<{ _tag: 'PressedContentPointer' }>
  | Readonly<{ _tag: 'FocusedItem'; index: number }>
  | Readonly<{
      _tag: 'SelectedItem'
      index: number
      value: string
      closeOnSelect: boolean
    }>
  | Readonly<{ _tag: 'PressedKey'; key: DrawerKey }>

export type DrawerKey =
  | 'Escape'
  | 'ArrowDown'
  | 'ArrowRight'
  | 'ArrowUp'
  | 'ArrowLeft'
  | 'Home'
  | 'End'

export type DrawerInit = Readonly<{
  open?: boolean
  focusedIndex?: number
  selectedValue?: string
}>

export type DrawerRootProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: DrawerModel
  children: BasecoatChildren
  onMessage?: (message: DrawerMessage) => Message
  side?: DrawerSide
  id?: string
  labelledBy?: string
  describedBy?: string
  ariaLabel?: string
}>

export type DrawerPanelProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  model?: DrawerModel
  onMessage?: (message: DrawerMessage) => Message
  labelledBy?: string
}>

export type DrawerSectionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type DrawerTitleProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  level?: 2 | 3
}>

export type DrawerDescriptionProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    id?: string
  }>

export type DrawerControlProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  onMessage?: (message: DrawerMessage) => Message
  disabled?: boolean
  ariaLabel?: string
}>

export type DrawerItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  index: number
  value: string
  model: DrawerModel
  onMessage?: (message: DrawerMessage) => Message
  closeOnSelect?: boolean
  disabled?: boolean
}>

const drawerRoot = basecoatClass('drawer')

export const initDrawer = (input: DrawerInit = {}): DrawerModel => ({
  open: input.open === true,
  closing: false,
  pointerStartedOnBackdrop: false,
  focusedIndex: Math.max(0, input.focusedIndex ?? 0),
  ...(input.selectedValue === undefined
    ? {}
    : { selectedValue: input.selectedValue }),
})

export const drawerRequestedOpen = (): DrawerMessage => ({
  _tag: 'RequestedOpen',
})

export const drawerRequestedClose = (): DrawerMessage => ({
  _tag: 'RequestedClose',
})

export const drawerCompletedClose = (): DrawerMessage => ({
  _tag: 'CompletedClose',
})

export const drawerPressedBackdropPointer = (): DrawerMessage => ({
  _tag: 'PressedBackdropPointer',
})

export const drawerReleasedBackdropPointer = (): DrawerMessage => ({
  _tag: 'ReleasedBackdropPointer',
})

export const drawerPressedContentPointer = (): DrawerMessage => ({
  _tag: 'PressedContentPointer',
})

export const drawerFocusedItem = (index: number): DrawerMessage => ({
  _tag: 'FocusedItem',
  index,
})

export const drawerSelectedItem = (
  index: number,
  value: string,
  closeOnSelect = false,
): DrawerMessage => ({
  _tag: 'SelectedItem',
  index,
  value,
  closeOnSelect,
})

export const drawerPressedKey = (key: DrawerKey): DrawerMessage => ({
  _tag: 'PressedKey',
  key,
})

const closeDrawer = (model: DrawerModel): DrawerModel =>
  model.open
    ? {
        ...model,
        closing: true,
        pointerStartedOnBackdrop: false,
      }
    : model

const completeClose = (model: DrawerModel): DrawerModel => ({
  ...model,
  open: false,
  closing: false,
  pointerStartedOnBackdrop: false,
})

const moveFocusedIndex = (
  focusedIndex: number,
  delta: number,
  itemCount: number,
): number => {
  if (itemCount <= 0) {
    return 0
  }

  return (focusedIndex + delta + itemCount) % itemCount
}

export const drawerKeyMessage = (
  key: string,
  _modifiers?: KeyboardModifiers,
): DrawerMessage | null => {
  switch (key) {
    case 'Escape':
    case 'ArrowDown':
    case 'ArrowRight':
    case 'ArrowUp':
    case 'ArrowLeft':
    case 'Home':
    case 'End':
      return drawerPressedKey(key)
    default:
      return null
  }
}

export const updateDrawer = (
  model: DrawerModel,
  message: DrawerMessage,
  itemCount = 0,
): DrawerModel => {
  switch (message._tag) {
    case 'RequestedOpen':
      return {
        ...model,
        open: true,
        closing: false,
        pointerStartedOnBackdrop: false,
      }
    case 'RequestedClose':
      return closeDrawer(model)
    case 'CompletedClose':
      return model.closing ? completeClose(model) : model
    case 'PressedBackdropPointer':
      return { ...model, pointerStartedOnBackdrop: true }
    case 'ReleasedBackdropPointer':
      return model.pointerStartedOnBackdrop ? closeDrawer(model) : model
    case 'PressedContentPointer':
      return { ...model, pointerStartedOnBackdrop: false }
    case 'FocusedItem':
      return {
        ...model,
        focusedIndex: Math.max(0, message.index),
      }
    case 'SelectedItem':
      return {
        ...(message.closeOnSelect ? closeDrawer(model) : model),
        focusedIndex: Math.max(0, message.index),
        selectedValue: message.value,
      }
    case 'PressedKey':
      switch (message.key) {
        case 'Escape':
          return closeDrawer(model)
        case 'ArrowDown':
        case 'ArrowRight':
          return {
            ...model,
            focusedIndex: moveFocusedIndex(model.focusedIndex, 1, itemCount),
          }
        case 'ArrowUp':
        case 'ArrowLeft':
          return {
            ...model,
            focusedIndex: moveFocusedIndex(model.focusedIndex, -1, itemCount),
          }
        case 'Home':
          return { ...model, focusedIndex: 0 }
        case 'End':
          return { ...model, focusedIndex: Math.max(0, itemCount - 1) }
      }
  }
}

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

const mappedAttr = <Message>(
  onMessage: ((message: DrawerMessage) => Message) | undefined,
  message: DrawerMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  onMessage === undefined ? [] : [attr(onMessage(message))]

const mappedKeydownAttr = <Message>(
  onMessage: ((message: DrawerMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (onMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnKeyDown((key, modifiers) => {
      const message = drawerKeyMessage(key, modifiers)
      return onMessage(message ?? drawerPressedContentPointer())
    }),
  ]
}

export const drawer = <Message>(input: DrawerRootProps<Message>): Html => {
  const h = html<Message>()
  const open = input.model.open || input.model.closing

  return h.dialog(
    [
      ...basecoatAttrs<Message>(input, drawerRoot),
      ...(open ? [h.Attribute('open', '')] : []),
      ...dataAttr<Message>('side', input.side),
      ...(input.model.closing ? [h.DataAttribute('closing', 'true')] : []),
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
      ...optionalStringAttr<Message>(input.describedBy, h.AriaDescribedBy),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      h.AriaModal(true),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerRequestedClose(),
        h.OnCancel,
      ),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerPressedBackdropPointer(),
        h.OnMouseDown,
      ),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerReleasedBackdropPointer(),
        h.OnMouseUp,
      ),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerCompletedClose(),
        h.OnTransitionEnd,
      ),
      ...mappedKeydownAttr<Message>(input.onMessage),
    ],
    input.children,
  )
}

export const drawerPanel = <Message>(
  input: DrawerPanelProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('slot', 'drawer-content'),
      h.Tabindex(-1),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerPressedContentPointer(),
        h.OnMouseDown,
      ),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerPressedContentPointer(),
        h.OnMouseUp,
      ),
      ...mappedKeydownAttr<Message>(input.onMessage),
    ],
    input.children,
  )
}

export const drawerHeader = <Message>(
  input: DrawerSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.header(basecoatAttrs<Message>(input), input.children)
}

export const drawerBody = <Message>(
  input: DrawerSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.section(basecoatAttrs<Message>(input), input.children)
}

export const drawerFooter = <Message>(
  input: DrawerSectionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}

export const drawerTitle = <Message>(
  input: DrawerTitleProps<Message>,
): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>(input),
    ...optionalStringAttr<Message>(input.id, h.Id),
  ]

  return input.level === 3
    ? h.h3(attrs, input.children)
    : h.h2(attrs, input.children)
}

export const drawerDescription = <Message>(
  input: DrawerDescriptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.p(
    [
      ...basecoatAttrs<Message>(input),
      ...optionalStringAttr<Message>(input.id, h.Id),
    ],
    input.children,
  )
}

export const drawerTrigger = <Message>(
  input: DrawerControlProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerRequestedOpen(),
        h.OnClick,
      ),
    ],
    input.children,
  )
}

export const drawerClose = <Message>(
  input: DrawerControlProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.DataAttribute('slot', 'drawer-close'),
      ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerRequestedClose(),
        h.OnClick,
      ),
    ],
    input.children,
  )
}

export const drawerItem = <Message>(
  input: DrawerItemProps<Message>,
): Html => {
  const h = html<Message>()
  const selected = input.model.selectedValue === input.value
  const focused = input.model.focusedIndex === input.index

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.Role('menuitem'),
      h.DataAttribute('slot', 'drawer-item'),
      h.DataAttribute('value', input.value),
      h.AriaSelected(selected),
      h.Tabindex(focused ? 0 : -1),
      ...(selected ? [h.DataAttribute('selected', 'true')] : []),
      ...(focused ? [h.DataAttribute('focused', 'true')] : []),
      ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerFocusedItem(input.index),
        h.OnFocus,
      ),
      ...mappedAttr<Message>(
        input.onMessage,
        drawerSelectedItem(
          input.index,
          input.value,
          input.closeOnSelect === true,
        ),
        h.OnClick,
      ),
    ],
    input.children,
  )
}
