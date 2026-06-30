import { Option } from 'effect'
import type { Attribute, Html, KeyboardModifiers } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type ToastCategory = 'success' | 'error' | 'info' | 'warning'
export type ToastAlign = 'start' | 'center' | 'end'
export type ToastControl = 'action' | 'cancel'

export type ToastAction = Readonly<{
  label: string
  href?: string
}>

export type ToastItem = Readonly<{
  id: string
  category?: ToastCategory
  title?: BasecoatChildren
  description?: BasecoatChildren
  icon?: Html
  action?: ToastAction
  cancel?: ToastAction
  duration?: number
  open?: boolean
}>

export type ToastSelection = Readonly<{
  toastId: string
  control: ToastControl
}>

export type ToastFocus = Readonly<{
  toastId: string
  control: ToastControl | null
}>

export type ToastModel = Readonly<{
  toasts: ReadonlyArray<ToastItem>
  paused: boolean
  focus: ToastFocus | null
  selection: ToastSelection | null
}>

export type ToastInit = Readonly<{
  toasts?: ReadonlyArray<ToastItem>
  paused?: boolean
  focus?: ToastFocus | null
  selection?: ToastSelection | null
}>

export type ToastKey =
  | 'Escape'
  | 'ArrowDown'
  | 'ArrowRight'
  | 'ArrowUp'
  | 'ArrowLeft'
  | 'Home'
  | 'End'
  | 'Enter'
  | ' '
  | 'Spacebar'

export type ToastMessage =
  | Readonly<{ _tag: 'ToastOpened'; toast: ToastItem }>
  | Readonly<{ _tag: 'ToastClosed'; toastId: string }>
  | Readonly<{ _tag: 'ToastClosedAll' }>
  | Readonly<{ _tag: 'ToastRemoved'; toastId: string }>
  | Readonly<{ _tag: 'ToastPaused' }>
  | Readonly<{ _tag: 'ToastResumed' }>
  | Readonly<{ _tag: 'ToastFocused'; toastId: string; control?: ToastControl | null }>
  | Readonly<{ _tag: 'ToastSelected'; toastId: string; control: ToastControl }>
  | Readonly<{ _tag: 'ToastKeyDown'; key: ToastKey }>

export type ToasterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ToastModel
  children?: BasecoatChildren
  id?: string
  align?: ToastAlign
  toMessage?: (message: ToastMessage) => Message
}>

export type ToastProps<Message> = BasecoatAttrs<Message> & Readonly<{
  toast: ToastItem
  focused?: boolean
  focusedControl?: ToastControl | null
  toMessage?: (message: ToastMessage) => Message
}>

export type ToastContentProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type ToastFooterProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

const toasterRoot = basecoatClass('toaster')
const toastRoot = basecoatClass('toast')
const toastContentRoot = basecoatClass('toast-content')
const buttonRoot = basecoatClass('btn')

export const toastDefaultDuration = (toast: ToastItem): number =>
  toast.duration === -1 ? -1 : toast.duration ?? (toast.category === 'error' ? 5000 : 3000)

export const initToastModel = (input: ToastInit = {}): ToastModel => ({
  toasts: (input.toasts ?? []).map(toast => ({
    ...toast,
    open: toast.open !== false,
  })),
  paused: input.paused === true,
  focus: input.focus ?? null,
  selection: input.selection ?? null,
})

export const toastOpened = (toast: ToastItem): ToastMessage => ({
  _tag: 'ToastOpened',
  toast,
})

export const toastClosed = (toastId: string): ToastMessage => ({
  _tag: 'ToastClosed',
  toastId,
})

export const toastClosedAll = (): ToastMessage => ({
  _tag: 'ToastClosedAll',
})

export const toastRemoved = (toastId: string): ToastMessage => ({
  _tag: 'ToastRemoved',
  toastId,
})

export const toastPaused = (): ToastMessage => ({
  _tag: 'ToastPaused',
})

export const toastResumed = (): ToastMessage => ({
  _tag: 'ToastResumed',
})

export const toastFocused = (
  toastId: string,
  control: ToastControl | null = null,
): ToastMessage => ({
  _tag: 'ToastFocused',
  toastId,
  control,
})

export const toastSelected = (
  toastId: string,
  control: ToastControl,
): ToastMessage => ({
  _tag: 'ToastSelected',
  toastId,
  control,
})

export const toastKeyDown = (key: ToastKey): ToastMessage => ({
  _tag: 'ToastKeyDown',
  key,
})

const isOpen = (toast: ToastItem): boolean => toast.open !== false

const toastControlOrder = (toast: ToastItem): ReadonlyArray<ToastControl> => [
  ...(toast.action === undefined ? [] : ['action' as const]),
  ...(toast.cancel === undefined ? [] : ['cancel' as const]),
]

const focusTargets = (model: ToastModel): ReadonlyArray<ToastFocus> => {
  const targets: Array<ToastFocus> = []

  for (const toast of model.toasts) {
    if (!isOpen(toast)) {
      continue
    }

    const controls = toastControlOrder(toast)
    if (controls.length === 0) {
      targets.push({ toastId: toast.id, control: null })
      continue
    }

    for (const control of controls) {
      targets.push({ toastId: toast.id, control })
    }
  }

  return targets
}

const focusIndex = (
  targets: ReadonlyArray<ToastFocus>,
  focus: ToastFocus | null,
): number =>
  focus === null
    ? -1
    : targets.findIndex(target =>
        target.toastId === focus.toastId && target.control === focus.control
      )

const moveFocus = (
  model: ToastModel,
  direction: 'next' | 'previous' | 'first' | 'last',
): ToastModel => {
  const targets = focusTargets(model)
  if (targets.length === 0) {
    return { ...model, focus: null }
  }

  const currentIndex = focusIndex(targets, model.focus)
  const nextIndex =
    direction === 'first'
      ? 0
      : direction === 'last'
        ? targets.length - 1
        : direction === 'next'
          ? currentIndex === -1
            ? 0
            : Math.min(currentIndex + 1, targets.length - 1)
          : currentIndex === -1
            ? targets.length - 1
            : Math.max(currentIndex - 1, 0)

  return { ...model, focus: targets[nextIndex] ?? null }
}

const closeToast = (model: ToastModel, toastId: string): ToastModel => ({
  ...model,
  toasts: model.toasts.map(toast =>
    toast.id === toastId ? { ...toast, open: false } : toast
  ),
  focus: model.focus?.toastId === toastId ? null : model.focus,
})

const selectFocused = (model: ToastModel): ToastModel => {
  if (model.focus === null || model.focus.control === null) {
    return model
  }

  return {
    ...closeToast(model, model.focus.toastId),
    selection: {
      toastId: model.focus.toastId,
      control: model.focus.control,
    },
  }
}

export const toastKeyMessage = (
  key: string,
  _modifiers?: KeyboardModifiers,
): ToastMessage | null => {
  switch (key) {
    case 'Escape':
    case 'ArrowDown':
    case 'ArrowRight':
    case 'ArrowUp':
    case 'ArrowLeft':
    case 'Home':
    case 'End':
    case 'Enter':
    case ' ':
    case 'Spacebar':
      return toastKeyDown(key)
    default:
      return null
  }
}

export const updateToast = (
  model: ToastModel,
  message: ToastMessage,
): ToastModel => {
  switch (message._tag) {
    case 'ToastOpened':
      return {
        ...model,
        toasts: [
          ...model.toasts.filter(toast => toast.id !== message.toast.id),
          { ...message.toast, open: message.toast.open !== false },
        ],
      }
    case 'ToastClosed':
      return closeToast(model, message.toastId)
    case 'ToastClosedAll':
      return {
        ...model,
        toasts: model.toasts.map(toast => ({ ...toast, open: false })),
        focus: null,
      }
    case 'ToastRemoved':
      return {
        ...model,
        toasts: model.toasts.filter(toast => toast.id !== message.toastId),
        focus: model.focus?.toastId === message.toastId ? null : model.focus,
      }
    case 'ToastPaused':
      return { ...model, paused: true }
    case 'ToastResumed':
      return { ...model, paused: false }
    case 'ToastFocused':
      return model.toasts.some(toast => toast.id === message.toastId && isOpen(toast))
        ? {
            ...model,
            focus: {
              toastId: message.toastId,
              control: message.control ?? null,
            },
          }
        : model
    case 'ToastSelected':
      return {
        ...closeToast(model, message.toastId),
        selection: {
          toastId: message.toastId,
          control: message.control,
        },
      }
    case 'ToastKeyDown':
      switch (message.key) {
        case 'Escape':
          return model.focus === null ? model : closeToast(model, model.focus.toastId)
        case 'ArrowDown':
        case 'ArrowRight':
          return moveFocus(model, 'next')
        case 'ArrowUp':
        case 'ArrowLeft':
          return moveFocus(model, 'previous')
        case 'Home':
          return moveFocus(model, 'first')
        case 'End':
          return moveFocus(model, 'last')
        case 'Enter':
        case ' ':
        case 'Spacebar':
          return selectFocused(model)
      }
  }
}

const mappedAttr = <Message>(
  toMessage: ((message: ToastMessage) => Message) | undefined,
  message: ToastMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  toMessage === undefined ? [] : [attr(toMessage(message))]

const mappedKeydownAttr = <Message>(
  toMessage: ((message: ToastMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (toMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnKeyDownPreventDefault((key, modifiers) => {
      const message = toastKeyMessage(key, modifiers)
      return message === null ? Option.none() : Option.some(toMessage(message))
    }),
  ]
}

export const toastContent = <Message>(
  input: ToastContentProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [...basecoatAttrs<Message>(input, toastContentRoot)],
    input.children,
  )
}

export const toastFooter = <Message>(
  input: ToastFooterProps<Message>,
): Html => {
  const h = html<Message>()

  return h.footer(basecoatAttrs<Message>(input), input.children)
}

const toastActionElement = <Message>(
  toast: ToastItem,
  action: ToastAction,
  control: ToastControl,
  focused: boolean,
  toMessage: ((message: ToastMessage) => Message) | undefined,
): Html => {
  const h = html<Message>()
  const attrs = [
    ...basecoatAttrs<Message>({}, buttonRoot),
    h.DataAttribute(control === 'action' ? 'toast-action' : 'toast-cancel', ''),
    ...(focused ? [h.Attribute('autofocus', '')] : []),
    ...mappedAttr<Message>(
      toMessage,
      toastSelected(toast.id, control),
      h.OnClick,
    ),
    ...mappedAttr<Message>(
      toMessage,
      toastFocused(toast.id, control),
      h.OnFocus,
    ),
  ]

  return action.href === undefined
    ? h.button([h.Type('button'), ...attrs], [action.label])
    : h.a([h.Href(action.href), ...attrs], [action.label])
}

export const toast = <Message>(input: ToastProps<Message>): Html => {
  const h = html<Message>()
  const footerChildren = [
    ...(input.toast.action === undefined
      ? []
      : [
          toastActionElement<Message>(
            input.toast,
            input.toast.action,
            'action',
            input.focusedControl === 'action',
            input.toMessage,
          ),
        ]),
    ...(input.toast.cancel === undefined
      ? []
      : [
          toastActionElement<Message>(
            input.toast,
            input.toast.cancel,
            'cancel',
            input.focusedControl === 'cancel',
            input.toMessage,
          ),
        ]),
  ]

  return h.div(
    [
      ...basecoatAttrs<Message>(input, toastRoot),
      h.Role('status'),
      h.AriaAtomic(true),
      h.AriaHidden(!isOpen(input.toast)),
      h.DataAttribute('toast-id', input.toast.id),
      ...dataAttr<Message>('category', input.toast.category),
      ...(input.toast.duration === undefined
        ? []
        : [h.DataAttribute('duration', String(input.toast.duration))]),
      h.Tabindex(input.focused === true ? 0 : -1),
      ...mappedAttr<Message>(
        input.toMessage,
        toastFocused(input.toast.id),
        h.OnFocus,
      ),
      ...mappedAttr<Message>(
        input.toMessage,
        toastRemoved(input.toast.id),
        h.OnTransitionEnd,
      ),
      ...mappedKeydownAttr<Message>(input.toMessage),
    ],
    [
      toastContent<Message>({
        children: [
          input.toast.icon ?? null,
          h.section([], [
            ...(input.toast.title === undefined
              ? []
              : [h.h2([], input.toast.title)]),
            ...(input.toast.description === undefined
              ? []
              : [h.p([], input.toast.description)]),
          ]),
          footerChildren.length === 0
            ? null
            : toastFooter<Message>({ children: footerChildren }),
        ],
      }),
    ],
  )
}

export const toaster = <Message>(input: ToasterProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, toasterRoot),
      h.Id(input.id ?? 'toaster'),
      ...dataAttr<Message>('align', input.align),
      ...(input.model.paused ? [h.DataAttribute('paused', 'true')] : []),
      ...mappedAttr<Message>(
        input.toMessage,
        toastPaused(),
        h.OnMouseEnter,
      ),
      ...mappedAttr<Message>(
        input.toMessage,
        toastResumed(),
        h.OnMouseLeave,
      ),
      ...mappedKeydownAttr<Message>(input.toMessage),
    ],
    input.children ?? input.model.toasts.map(item =>
      toast<Message>({
        toast: item,
        focused: input.model.focus?.toastId === item.id,
        focusedControl: input.model.focus?.toastId === item.id
          ? input.model.focus.control
          : null,
        ...(input.toMessage === undefined ? {} : { toMessage: input.toMessage }),
      })
    ),
  )
}
