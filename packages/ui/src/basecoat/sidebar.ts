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

export type SidebarSide = 'left' | 'right'

export type SidebarItem = Readonly<{
  value: string
  disabled?: boolean
}>

export type SidebarModel = Readonly<{
  open: boolean
  breakpoint: number
  items: ReadonlyArray<SidebarItem>
  focusedValue: string | null
  selectedValue: string | null
}>

export type SidebarInit = Readonly<{
  items: ReadonlyArray<SidebarItem>
  initialOpen?: boolean
  initialMobileOpen?: boolean
  breakpoint?: number
  viewportWidth?: number
  focusedValue?: string | null
  selectedValue?: string | null
}>

export type SidebarMessage =
  | Readonly<{ readonly _tag: 'SidebarOpened' }>
  | Readonly<{ readonly _tag: 'SidebarClosed' }>
  | Readonly<{ readonly _tag: 'SidebarToggled' }>
  | Readonly<{ readonly _tag: 'SidebarBackdropClicked' }>
  | Readonly<{ readonly _tag: 'SidebarFocused'; readonly value: string }>
  | Readonly<{ readonly _tag: 'SidebarSelected'; readonly value: string }>
  | Readonly<{
      readonly _tag: 'SidebarItemActivated'
      readonly value: string
      readonly mobile: boolean
      readonly keepMobileOpen?: boolean
    }>
  | Readonly<{ readonly _tag: 'SidebarKeyDown'; readonly key: string; readonly value: string }>

export type SidebarProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  open?: boolean
  initialOpen?: boolean
  initialMobileOpen?: boolean
  breakpoint?: number
  side?: SidebarSide
  label?: string
  onClickOutside?: Message
  onKeyDown?: (key: string) => Message | null
}>

export type SidebarNavProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  label?: string
}>

export type SidebarSlotProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type SidebarGroupProps<Message> = SidebarSlotProps<Message> & Readonly<{
  label?: string
}>

export type SidebarMenuItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type SidebarMenuButtonProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  value: string
  selected?: boolean
  focused?: boolean
  disabled?: boolean
  keepMobileOpen?: boolean
  onClick?: Message
  onFocus?: Message
  onKeyDown?: (key: string) => Message | null
}>

export type SidebarMenuLinkProps<Message> = SidebarMenuButtonProps<Message> & Readonly<{
  href: string
  target?: string
  rel?: string
}>

export type SidebarViewItem<Message> = SidebarItem & Readonly<{
  children: BasecoatChildren
  href?: string
  target?: string
  rel?: string
  keepMobileOpen?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
  className?: string
}>

export type SidebarViewGroup<Message> = Readonly<{
  label?: string
  items: ReadonlyArray<SidebarViewItem<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
  className?: string
}>

export type SidebarViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SidebarModel
  groups: ReadonlyArray<SidebarViewGroup<Message>>
  toMessage: (message: SidebarMessage) => Message
  mobile?: boolean
  side?: SidebarSide
  label?: string
}>

const sidebarRoot = basecoatClass('sidebar')

const enabledItems = (model: SidebarModel): ReadonlyArray<SidebarItem> =>
  model.items.filter(item => item.disabled !== true)

const hasItem = (items: ReadonlyArray<SidebarItem>, value: string): boolean =>
  items.some(item => item.value === value)

const isDisabled = (model: SidebarModel, value: string): boolean =>
  model.items.find(item => item.value === value)?.disabled === true

const focusByOffset = (model: SidebarModel, value: string, offset: number): SidebarModel => {
  const items = enabledItems(model)
  const index = items.findIndex(item => item.value === value)

  if (items.length === 0 || index === -1) {
    return model
  }

  const next = items[(index + offset + items.length) % items.length]

  return { ...model, focusedValue: next?.value ?? model.focusedValue }
}

const selectItem = (model: SidebarModel, value: string): SidebarModel =>
  isDisabled(model, value) || !hasItem(model.items, value)
    ? model
    : { ...model, selectedValue: value, focusedValue: value }

export const sidebarInit = (input: SidebarInit): SidebarModel => {
  const breakpoint = input.breakpoint ?? 768
  const viewportWidth = input.viewportWidth
  const initialOpen = input.initialOpen ?? true
  const initialMobileOpen = input.initialMobileOpen ?? false
  const open =
    viewportWidth === undefined || breakpoint <= 0
      ? initialOpen
      : viewportWidth >= breakpoint
        ? initialOpen
        : initialMobileOpen
  const firstEnabled = input.items.find(item => item.disabled !== true)?.value ?? null
  const focusedValue =
    input.focusedValue !== undefined &&
    input.focusedValue !== null &&
    hasItem(input.items, input.focusedValue) &&
    input.items.find(item => item.value === input.focusedValue)?.disabled !== true
      ? input.focusedValue
      : firstEnabled
  const selectedValue =
    input.selectedValue !== undefined &&
    input.selectedValue !== null &&
    hasItem(input.items, input.selectedValue) &&
    input.items.find(item => item.value === input.selectedValue)?.disabled !== true
      ? input.selectedValue
      : null

  return {
    open,
    breakpoint,
    items: input.items,
    focusedValue,
    selectedValue,
  }
}

export const sidebarUpdate = (
  model: SidebarModel,
  message: SidebarMessage,
): SidebarModel => {
  switch (message._tag) {
    case 'SidebarOpened':
      return { ...model, open: true }
    case 'SidebarClosed':
    case 'SidebarBackdropClicked':
      return { ...model, open: false }
    case 'SidebarToggled':
      return { ...model, open: !model.open }
    case 'SidebarFocused':
      return isDisabled(model, message.value) || !hasItem(model.items, message.value)
        ? model
        : { ...model, focusedValue: message.value }
    case 'SidebarSelected':
      return selectItem(model, message.value)
    case 'SidebarItemActivated': {
      const selected = selectItem(model, message.value)
      return message.mobile === true && message.keepMobileOpen !== true
        ? { ...selected, open: false }
        : selected
    }
    case 'SidebarKeyDown': {
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
          return selectItem(model, message.value)
        case 'Escape':
          return { ...model, open: false }
        default:
          return model
      }
    }
  }
}

const handledSidebarKeys = new Set([
  'ArrowDown',
  'ArrowRight',
  'ArrowUp',
  'ArrowLeft',
  'Home',
  'End',
  'Enter',
  ' ',
  'Spacebar',
  'Escape',
])

const sidebarInteractiveAttrs = <Message>(
  input: Pick<SidebarMenuButtonProps<Message>, 'disabled' | 'focused' | 'keepMobileOpen' | 'onClick' | 'onFocus' | 'onKeyDown' | 'selected' | 'value'>,
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()

  return [
    h.DataAttribute('value', input.value),
    ...(input.selected === true ? [h.AriaCurrent('page'), h.DataAttribute('active', 'true')] : []),
    ...(input.focused === true ? [h.Tabindex(0)] : [h.Tabindex(-1)]),
    ...(input.keepMobileOpen === true ? [h.DataAttribute('keep-mobile-sidebar-open', '')] : []),
    ...(input.disabled === true ? [h.AriaDisabled(true), h.DataAttribute('disabled', 'true')] : []),
    ...(input.onClick === undefined || input.disabled === true ? [] : [h.OnClick(input.onClick)]),
    ...(input.onFocus === undefined || input.disabled === true ? [] : [h.OnFocus(input.onFocus)]),
    ...(input.onKeyDown === undefined || input.disabled === true
      ? []
      : [
          h.OnKeyDownPreventDefault(key =>
            handledSidebarKeys.has(key)
              ? (() => {
                  const next = input.onKeyDown?.(key) ?? null
                  return next === null ? Option.none() : Option.some(next)
                })()
              : Option.none(),
          ),
        ]),
  ]
}

export const sidebar = <Message>(input: SidebarProps<Message>): Html => {
  const h = html<Message>()

  return h.aside(
    [
      ...basecoatAttrs<Message>(input, sidebarRoot),
      h.AriaHidden(input.open === false),
      ...(input.open === false ? [h.Inert(true)] : []),
      ...dataAttr<Message>('initial-open', input.initialOpen === false ? 'false' : undefined),
      ...dataAttr<Message>('initial-mobile-open', input.initialMobileOpen === true ? 'true' : undefined),
      ...dataAttr<Message>('breakpoint', input.breakpoint === undefined ? undefined : String(input.breakpoint)),
      ...dataAttr<Message>('side', input.side),
      ...(input.label === undefined ? [] : [h.AriaLabel(input.label)]),
      ...(input.onClickOutside === undefined ? [] : [h.OnClick(input.onClickOutside)]),
      ...(input.onKeyDown === undefined
        ? []
        : [
            h.OnKeyDownPreventDefault(key =>
              handledSidebarKeys.has(key)
                ? (() => {
                    const next = input.onKeyDown?.(key) ?? null
                    return next === null ? Option.none() : Option.some(next)
                  })()
                : Option.none(),
            ),
          ]),
    ],
    input.children,
  )
}

export const sidebarNav = <Message>(input: SidebarNavProps<Message>): Html => {
  const h = html<Message>()

  return h.nav(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.label === undefined ? [] : [h.AriaLabel(input.label)]),
    ],
    input.children,
  )
}

export const sidebarHeader = <Message>(input: SidebarSlotProps<Message>): Html => {
  const h = html<Message>()

  return h.header(
    [...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-header')],
    input.children,
  )
}

export const sidebarContent = <Message>(input: SidebarSlotProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-content')],
    input.children,
  )
}

export const sidebarFooter = <Message>(input: SidebarSlotProps<Message>): Html => {
  const h = html<Message>()

  return h.footer(
    [...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-footer')],
    input.children,
  )
}

export const sidebarGroup = <Message>(input: SidebarGroupProps<Message>): Html => {
  const h = html<Message>()

  return h.section(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('group'),
      ...(input.label === undefined ? [] : [h.AriaLabel(input.label)]),
    ],
    input.children,
  )
}

export const sidebarGroupLabel = <Message>(input: SidebarSlotProps<Message>): Html => {
  const h = html<Message>()

  return h.h3(
    [...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-group-label')],
    input.children,
  )
}

export const sidebarMenu = <Message>(input: SidebarSlotProps<Message>): Html => {
  const h = html<Message>()

  return h.ul(
    [...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-menu')],
    input.children,
  )
}

export const sidebarMenuItem = <Message>(input: SidebarMenuItemProps<Message>): Html => {
  const h = html<Message>()

  return h.li(basecoatAttrs<Message>(input), input.children)
}

export const sidebarMenuButton = <Message>(
  input: SidebarMenuButtonProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      ...sidebarInteractiveAttrs<Message>(input),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
    ],
    input.children,
  )
}

export const sidebarMenuLink = <Message>(
  input: SidebarMenuLinkProps<Message>,
): Html => {
  const h = html<Message>()

  return h.a(
    [
      ...basecoatAttrs<Message>(input),
      h.Href(input.href),
      ...(input.target === undefined ? [] : [h.Target(input.target)]),
      ...(input.rel === undefined ? [] : [h.Rel(input.rel)]),
      ...sidebarInteractiveAttrs<Message>(input),
    ],
    input.children,
  )
}

export const sidebarSeparator = <Message>(input: BasecoatAttrs<Message> = {}): Html => {
  const h = html<Message>()

  return h.hr([...basecoatAttrs<Message>(input), h.DataAttribute('slot', 'sidebar-separator')])
}

export const sidebarView = <Message>(input: SidebarViewProps<Message>): Html => {
  const groups = input.groups.map(group =>
    sidebarGroup<Message>({
      children: [
        ...(group.label === undefined
          ? []
          : [sidebarGroupLabel<Message>({ children: [group.label] })]),
        sidebarMenu<Message>({
          children: group.items.map(item => {
            const common = {
              value: item.value,
              selected: input.model.selectedValue === item.value,
              focused: input.model.focusedValue === item.value,
              onClick: input.toMessage({
                _tag: 'SidebarItemActivated',
                value: item.value,
                mobile: input.mobile === true,
                ...(item.keepMobileOpen === undefined ? {} : { keepMobileOpen: item.keepMobileOpen }),
              }),
              onFocus: input.toMessage({
                _tag: 'SidebarFocused',
                value: item.value,
              }),
              onKeyDown: (key: string) =>
                input.toMessage({
                  _tag: 'SidebarKeyDown',
                  key,
                  value: item.value,
                }),
              children: item.children,
              ...(item.disabled === undefined ? {} : { disabled: item.disabled }),
              ...(item.keepMobileOpen === undefined ? {} : { keepMobileOpen: item.keepMobileOpen }),
              ...(item.attrs === undefined ? {} : { attrs: item.attrs }),
              ...(item.className === undefined ? {} : { className: item.className }),
            }

            return sidebarMenuItem<Message>({
              children: [
                item.href === undefined
                  ? sidebarMenuButton<Message>(common)
                  : sidebarMenuLink<Message>({
                      ...common,
                      href: item.href,
                      ...(item.target === undefined ? {} : { target: item.target }),
                      ...(item.rel === undefined ? {} : { rel: item.rel }),
                    }),
              ],
            })
          }),
        }),
      ],
      ...(group.attrs === undefined ? {} : { attrs: group.attrs }),
      ...(group.className === undefined ? {} : { className: group.className }),
      ...(group.label === undefined ? {} : { label: group.label }),
    }),
  )

  return sidebar<Message>({
    open: input.model.open,
    breakpoint: input.model.breakpoint,
    onKeyDown: key => key === 'Escape' ? input.toMessage({ _tag: 'SidebarKeyDown', key, value: input.model.focusedValue ?? '' }) : null,
    children: [
      sidebarNav<Message>({
        children: groups,
        ...(input.label === undefined ? {} : { label: input.label }),
      }),
    ],
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
    ...(input.side === undefined ? {} : { side: input.side }),
    ...(input.label === undefined ? {} : { label: input.label }),
  })
}
