import { Option } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type PopoverSide =
  | 'top'
  | 'right'
  | 'bottom'
  | 'left'
  | 'inline-start'
  | 'inline-end'

export type PopoverAlign = 'start' | 'center' | 'end'

export type PopoverModel = Readonly<{
  id: string
  open: boolean
  activeIndex: number | null
  selectedIndex: number | null
  itemCount: number
}>

export type PopoverInit = Readonly<{
  id: string
  open?: boolean
  activeIndex?: number | null
  selectedIndex?: number | null
  itemCount?: number
}>

export type PopoverMessage =
  | Readonly<{ _tag: 'PopoverToggled'; id: string; itemCount?: number }>
  | Readonly<{ _tag: 'PopoverOpened'; id: string; itemCount?: number }>
  | Readonly<{
      _tag: 'PopoverClosed'
      id: string
      focusTrigger?: boolean
    }>
  | Readonly<{ _tag: 'PopoverOutsidePressed'; id: string }>
  | Readonly<{ _tag: 'PopoverPeerOpened'; id: string; sourceId: string }>
  | Readonly<{ _tag: 'PopoverKeyPressed'; id: string; key: string }>
  | Readonly<{ _tag: 'PopoverItemFocused'; id: string; index: number }>
  | Readonly<{
      _tag: 'PopoverItemSelected'
      id: string
      index: number
      close?: boolean
    }>

export type PopoverEffect =
  | Readonly<{ _tag: 'none' }>
  | Readonly<{ _tag: 'opened'; sourceId: string; focus: 'content' | 'item' }>
  | Readonly<{ _tag: 'closed'; focus: 'trigger' | 'none' }>
  | Readonly<{ _tag: 'focusedItem'; index: number }>
  | Readonly<{ _tag: 'selected'; index: number; closed: boolean }>

export type PopoverUpdate = Readonly<{
  model: PopoverModel
  effect: PopoverEffect
}>

export type PopoverProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: PopoverModel
  trigger: BasecoatChildren
  children: BasecoatChildren
  toMessage: (message: PopoverMessage) => Message
  side?: PopoverSide
  align?: PopoverAlign
  disabled?: boolean | undefined
  triggerAttrs?: ReadonlyArray<Attribute<Message>> | undefined
  triggerClassName?: string | undefined
  contentAttrs?: ReadonlyArray<Attribute<Message>> | undefined
  contentClassName?: string | undefined
  triggerLabel?: string | undefined
  contentRole?: 'dialog' | 'menu' | 'listbox' | 'group' | undefined
}>

export type PopoverTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: PopoverModel
  children: BasecoatChildren
  toMessage: (message: PopoverMessage) => Message
  disabled?: boolean | undefined
  label?: string | undefined
}>

export type PopoverContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: PopoverModel
  children: BasecoatChildren
  toMessage: (message: PopoverMessage) => Message
  side?: PopoverSide | undefined
  align?: PopoverAlign | undefined
  role?: 'dialog' | 'menu' | 'listbox' | 'group' | undefined
}>

export type PopoverItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: PopoverModel
  index: number
  children: BasecoatChildren
  toMessage: (message: PopoverMessage) => Message
  selected?: boolean | undefined
  disabled?: boolean | undefined
}>

const popoverRoot = basecoatClass('popover')

const noneEffect: PopoverEffect = { _tag: 'none' }

const clampItemCount = (itemCount: number | undefined): number =>
  Math.max(0, Math.floor(itemCount ?? 0))

const normalizeIndex = (
  index: number | null | undefined,
  itemCount: number,
): number | null => {
  if (itemCount <= 0 || index === null || index === undefined) {
    return null
  }

  return Math.min(Math.max(0, Math.floor(index)), itemCount - 1)
}

const firstIndex = (itemCount: number): number | null =>
  itemCount <= 0 ? null : 0

const lastIndex = (itemCount: number): number | null =>
  itemCount <= 0 ? null : itemCount - 1

const nextIndex = (
  activeIndex: number | null,
  itemCount: number,
  direction: 1 | -1,
): number | null => {
  if (itemCount <= 0) {
    return null
  }

  if (activeIndex === null) {
    return direction === 1 ? 0 : itemCount - 1
  }

  return (activeIndex + direction + itemCount) % itemCount
}

const withItemCount = (
  model: PopoverModel,
  itemCount: number | undefined,
): PopoverModel => {
  const nextCount = itemCount === undefined
    ? model.itemCount
    : clampItemCount(itemCount)

  return {
    ...model,
    itemCount: nextCount,
    activeIndex: normalizeIndex(model.activeIndex, nextCount),
    selectedIndex: normalizeIndex(model.selectedIndex, nextCount),
  }
}

const openModel = (
  model: PopoverModel,
  itemCount: number | undefined,
  activeIndex: number | null = model.activeIndex,
): PopoverUpdate => {
  const withCount = withItemCount(model, itemCount)
  const nextActiveIndex = normalizeIndex(activeIndex, withCount.itemCount)

  return {
    model: {
      ...withCount,
      open: true,
      activeIndex: nextActiveIndex,
    },
    effect: {
      _tag: 'opened',
      sourceId: model.id,
      focus: nextActiveIndex === null ? 'content' : 'item',
    },
  }
}

const closeModel = (
  model: PopoverModel,
  focusTrigger: boolean,
): PopoverUpdate => {
  if (!model.open) {
    return { model, effect: noneEffect }
  }

  return {
    model: {
      ...model,
      open: false,
      activeIndex: null,
    },
    effect: {
      _tag: 'closed',
      focus: focusTrigger ? 'trigger' : 'none',
    },
  }
}

export const popoverInit = (input: PopoverInit): PopoverModel => {
  const itemCount = clampItemCount(input.itemCount)

  return {
    id: input.id,
    open: input.open ?? false,
    activeIndex: normalizeIndex(input.activeIndex, itemCount),
    selectedIndex: normalizeIndex(input.selectedIndex, itemCount),
    itemCount,
  }
}

export const PopoverToggled = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverToggled' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverToggled', ...input })

export const PopoverOpened = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverOpened' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverOpened', ...input })

export const PopoverClosed = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverClosed' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverClosed', ...input })

export const PopoverOutsidePressed = (
  input: Omit<
    Extract<PopoverMessage, { _tag: 'PopoverOutsidePressed' }>,
    '_tag'
  >,
): PopoverMessage => ({ _tag: 'PopoverOutsidePressed', ...input })

export const PopoverPeerOpened = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverPeerOpened' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverPeerOpened', ...input })

export const PopoverKeyPressed = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverKeyPressed' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverKeyPressed', ...input })

export const PopoverItemFocused = (
  input: Omit<Extract<PopoverMessage, { _tag: 'PopoverItemFocused' }>, '_tag'>,
): PopoverMessage => ({ _tag: 'PopoverItemFocused', ...input })

export const PopoverItemSelected = (
  input: Omit<
    Extract<PopoverMessage, { _tag: 'PopoverItemSelected' }>,
    '_tag'
  >,
): PopoverMessage => ({ _tag: 'PopoverItemSelected', ...input })

export const popoverUpdate = (
  model: PopoverModel,
  message: PopoverMessage,
): PopoverUpdate => {
  if (message.id !== model.id) {
    return { model, effect: noneEffect }
  }

  switch (message._tag) {
    case 'PopoverToggled':
      return model.open
        ? closeModel(model, true)
        : openModel(model, message.itemCount)
    case 'PopoverOpened':
      return openModel(model, message.itemCount)
    case 'PopoverClosed':
      return closeModel(model, message.focusTrigger ?? true)
    case 'PopoverOutsidePressed':
      return closeModel(model, false)
    case 'PopoverPeerOpened':
      return message.sourceId === model.id
        ? { model, effect: noneEffect }
        : closeModel(model, false)
    case 'PopoverItemFocused': {
      const activeIndex = normalizeIndex(message.index, model.itemCount)

      return {
        model: { ...model, activeIndex },
        effect: activeIndex === null
          ? noneEffect
          : { _tag: 'focusedItem', index: activeIndex },
      }
    }
    case 'PopoverItemSelected': {
      const selectedIndex = normalizeIndex(message.index, model.itemCount)
      const shouldClose = message.close ?? true

      return {
        model: {
          ...model,
          selectedIndex,
          activeIndex: selectedIndex,
          open: shouldClose ? false : model.open,
        },
        effect: selectedIndex === null
          ? noneEffect
          : { _tag: 'selected', index: selectedIndex, closed: shouldClose },
      }
    }
    case 'PopoverKeyPressed': {
      if (message.key === 'Escape') {
        return closeModel(model, true)
      }

      if (!model.open) {
        if (message.key === 'ArrowDown') {
          return openModel(model, undefined, firstIndex(model.itemCount))
        }

        if (message.key === 'ArrowUp') {
          return openModel(model, undefined, lastIndex(model.itemCount))
        }

        if (message.key === 'Enter' || message.key === ' ') {
          return openModel(model, undefined)
        }

        return { model, effect: noneEffect }
      }

      if (message.key === 'ArrowDown' || message.key === 'ArrowUp') {
        const index = nextIndex(
          model.activeIndex,
          model.itemCount,
          message.key === 'ArrowDown' ? 1 : -1,
        )

        return {
          model: { ...model, activeIndex: index },
          effect: index === null
            ? noneEffect
            : { _tag: 'focusedItem', index },
        }
      }

      if (message.key === 'Home' || message.key === 'End') {
        const index = message.key === 'Home'
          ? firstIndex(model.itemCount)
          : lastIndex(model.itemCount)

        return {
          model: { ...model, activeIndex: index },
          effect: index === null
            ? noneEffect
            : { _tag: 'focusedItem', index },
        }
      }

      if (
        (message.key === 'Enter' || message.key === ' ') &&
        model.activeIndex !== null
      ) {
        return popoverUpdate(
          model,
          PopoverItemSelected({ id: model.id, index: model.activeIndex }),
        )
      }

      return { model, effect: noneEffect }
    }
  }
}

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const keydownMessage = <Message>(
  id: string,
  toMessage: (message: PopoverMessage) => Message,
) => (key: string): Option.Option<Message> => {
  if (
    key === 'Escape' ||
    key === 'Enter' ||
    key === ' ' ||
    key === 'ArrowDown' ||
    key === 'ArrowUp' ||
    key === 'Home' ||
    key === 'End'
  ) {
    return Option.some(toMessage(PopoverKeyPressed({ id, key })))
  }

  return Option.none()
}

export const popoverTrigger = <Message>(
  input: PopoverTriggerProps<Message>,
): Html => {
  const h = html<Message>()
  const { model } = input

  return h.button(
    [
      ...basecoatAttrs<Message>({
        attrs: input.attrs,
        className: input.className,
      }),
      h.Type('button'),
      h.Id(`${model.id}-trigger`),
      h.AriaExpanded(model.open),
      h.AriaControls(`${model.id}-content`),
      h.DataAttribute('popover-trigger', ''),
      ...(input.disabled === true
        ? [h.Disabled(true), h.AriaDisabled(true)]
        : [
            h.OnClick(input.toMessage(PopoverToggled({ id: model.id }))),
            h.OnKeyDownPreventDefault(keydownMessage(model.id, input.toMessage)),
          ]),
      ...optionalStringAttr<Message>(input.label, h.AriaLabel),
    ],
    input.children,
  )
}

export const popoverContent = <Message>(
  input: PopoverContentProps<Message>,
): Html => {
  const h = html<Message>()
  const { model } = input

  return h.div(
    [
      ...basecoatAttrs<Message>({
        attrs: input.attrs,
        className: input.className,
      }),
      h.Id(`${model.id}-content`),
      h.DataAttribute('popover', ''),
      h.AriaHidden(!model.open),
      h.Role(input.role ?? 'dialog'),
      h.Tabindex(-1),
      ...dataAttr<Message>('side', input.side),
      ...dataAttr<Message>('align', input.align),
      h.OnKeyDownPreventDefault(keydownMessage(model.id, input.toMessage)),
    ],
    input.children,
  )
}

export const popoverItem = <Message>(
  input: PopoverItemProps<Message>,
): Html => {
  const h = html<Message>()
  const active = input.model.activeIndex === input.index
  const selected = input.selected ?? input.model.selectedIndex === input.index

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.Role('menuitem'),
      h.Tabindex(active ? 0 : -1),
      h.DataAttribute('popover-item', ''),
      h.DataAttribute('index', String(input.index)),
      ...(active ? [h.DataAttribute('active', '')] : []),
      ...(selected ? [h.AriaSelected(true)] : []),
      ...(input.disabled === true
        ? [h.Disabled(true), h.AriaDisabled(true)]
        : [
            h.OnFocus(
              input.toMessage(
                PopoverItemFocused({ id: input.model.id, index: input.index }),
              ),
            ),
            h.OnClick(
              input.toMessage(
                PopoverItemSelected({ id: input.model.id, index: input.index }),
              ),
            ),
          ]),
    ],
    input.children,
  )
}

export const popover = <Message>(input: PopoverProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, popoverRoot),
    [
      popoverTrigger<Message>({
        model: input.model,
        toMessage: input.toMessage,
        children: input.trigger,
        attrs: input.triggerAttrs,
        className: input.triggerClassName,
        disabled: input.disabled,
        label: input.triggerLabel,
      }),
      popoverContent<Message>({
        model: input.model,
        toMessage: input.toMessage,
        children: input.children,
        attrs: input.contentAttrs,
        className: input.contentClassName,
        side: input.side,
        align: input.align,
        role: input.contentRole,
      }),
    ],
  )
}
