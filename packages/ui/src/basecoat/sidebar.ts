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

export const DEFAULT_SIDEBAR_BREAKPOINT = 768

export type SidebarSide = 'left' | 'right'
export type SidebarItemVariant = 'default' | 'outline'
export type SidebarItemSize = 'default' | 'sm' | 'lg'

export type SidebarModel = Readonly<{
  open: boolean
  initialOpen: boolean
  initialMobileOpen: boolean
  breakpoint: number
  viewportWidth: number | null
  selectedItemId: string | null
  focusedItemId: string | null
  openSubmenuIds: ReadonlyArray<string>
  closedSubmenuIds: ReadonlyArray<string>
}>

export type SidebarInit = Readonly<{
  initialOpen?: boolean
  initialMobileOpen?: boolean
  breakpoint?: number
  viewportWidth?: number
  selectedItemId?: string | null
  focusedItemId?: string | null
  openSubmenuIds?: ReadonlyArray<string>
  closedSubmenuIds?: ReadonlyArray<string>
}>

export type SidebarActivateItemInput = Readonly<{
  itemId: string
  keepMobileSidebarOpen?: boolean
}>

export type SidebarKeyInput = Readonly<{
  key: string
  itemIds: ReadonlyArray<string>
  keepMobileSidebarOpenItemIds?: ReadonlyArray<string>
}>

export type SidebarToggleSubmenuInput = Readonly<{
  submenuId: string
  open?: boolean
}>

export type SidebarMessage =
  | ReturnType<typeof SidebarOpened>
  | ReturnType<typeof SidebarClosed>
  | ReturnType<typeof SidebarToggled>
  | ReturnType<typeof SidebarSetOpen>
  | ReturnType<typeof SidebarViewportChanged>
  | ReturnType<typeof SidebarClickedOverlay>
  | ReturnType<typeof SidebarActivatedItem>
  | ReturnType<typeof SidebarFocusedItem>
  | ReturnType<typeof SidebarBlurredItem>
  | ReturnType<typeof SidebarPressedKey>
  | ReturnType<typeof SidebarToggledSubmenu>

export const SidebarOpened = () => ({
  _tag: 'SidebarOpened' as const,
})

export const SidebarClosed = () => ({
  _tag: 'SidebarClosed' as const,
})

export const SidebarToggled = () => ({
  _tag: 'SidebarToggled' as const,
})

export const SidebarSetOpen = (input: Readonly<{ open: boolean }>) => ({
  _tag: 'SidebarSetOpen' as const,
  open: input.open,
})

export const SidebarViewportChanged = (
  input: Readonly<{ viewportWidth: number }>,
) => ({
  _tag: 'SidebarViewportChanged' as const,
  viewportWidth: input.viewportWidth,
})

export const SidebarClickedOverlay = () => ({
  _tag: 'SidebarClickedOverlay' as const,
})

export const SidebarActivatedItem = (input: SidebarActivateItemInput) => ({
  _tag: 'SidebarActivatedItem' as const,
  itemId: input.itemId,
  keepMobileSidebarOpen: input.keepMobileSidebarOpen === true,
})

export const SidebarFocusedItem = (input: Readonly<{ itemId: string }>) => ({
  _tag: 'SidebarFocusedItem' as const,
  itemId: input.itemId,
})

export const SidebarBlurredItem = (input: Readonly<{ itemId: string }>) => ({
  _tag: 'SidebarBlurredItem' as const,
  itemId: input.itemId,
})

export const SidebarPressedKey = (input: SidebarKeyInput) => ({
  _tag: 'SidebarPressedKey' as const,
  key: input.key,
  itemIds: uniqueIds(input.itemIds),
  keepMobileSidebarOpenItemIds: uniqueIds(
    input.keepMobileSidebarOpenItemIds ?? [],
  ),
})

export const SidebarToggledSubmenu = (
  input: SidebarToggleSubmenuInput,
) => ({
  _tag: 'SidebarToggledSubmenu' as const,
  submenuId: input.submenuId,
  open: input.open ?? null,
})

export type SidebarViewActions<Message> = Readonly<{
  opened: () => Message
  closed: () => Message
  toggled: () => Message
  clickedOverlay: () => Message
  activatedItem: (input: SidebarActivateItemInput) => Message
  focusedItem: (itemId: string) => Message
  blurredItem: (itemId: string) => Message
  pressedKey: (input: SidebarKeyInput) => Message
  toggledSubmenu: (input: SidebarToggleSubmenuInput) => Message
}>

export const sidebarMessageActions: SidebarViewActions<SidebarMessage> = {
  opened: SidebarOpened,
  closed: SidebarClosed,
  toggled: SidebarToggled,
  clickedOverlay: SidebarClickedOverlay,
  activatedItem: SidebarActivatedItem,
  focusedItem: itemId => SidebarFocusedItem({ itemId }),
  blurredItem: itemId => SidebarBlurredItem({ itemId }),
  pressedKey: SidebarPressedKey,
  toggledSubmenu: SidebarToggledSubmenu,
}

export type SidebarSlotProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type SidebarGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  type: 'group'
  id: string
  label?: BasecoatChildren
  labelId?: string
  headingAttrs?: BasecoatAttrs<Message>
  items: ReadonlyArray<SidebarMenuEntry<Message>>
}>

export type SidebarSeparatorProps<Message> = BasecoatAttrs<Message> & Readonly<{
  type: 'separator'
  id?: string
}>

export type SidebarItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  type?: 'item'
  id: string
  label: BasecoatChildren
  icon?: Html
  href?: string
  current?: boolean
  active?: boolean
  disabled?: boolean
  ariaDisabled?: boolean
  variant?: SidebarItemVariant
  size?: SidebarItemSize
  keepMobileSidebarOpen?: boolean
  liAttrs?: BasecoatAttrs<Message>
}>

export type SidebarSubmenuProps<Message> = BasecoatAttrs<Message> & Readonly<{
  type: 'submenu'
  id: string
  label: BasecoatChildren
  icon?: Html
  open?: boolean
  current?: boolean
  active?: boolean
  variant?: SidebarItemVariant
  size?: SidebarItemSize
  detailsAttrs?: BasecoatAttrs<Message>
  listAttrs?: BasecoatAttrs<Message>
  liAttrs?: BasecoatAttrs<Message>
  items: ReadonlyArray<SidebarMenuEntry<Message>>
}>

export type SidebarMenuEntry<Message> =
  | SidebarGroupProps<Message>
  | SidebarSeparatorProps<Message>
  | SidebarItemProps<Message>
  | SidebarSubmenuProps<Message>

export type SidebarProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SidebarModel
  actions?: SidebarViewActions<Message>
  id?: string
  label?: string
  side?: SidebarSide
  header?: SidebarSlotProps<Message>
  footer?: SidebarSlotProps<Message>
  contentAttrs?: BasecoatAttrs<Message>
  navAttrs?: BasecoatAttrs<Message>
  children?: BasecoatChildren
  menu?: ReadonlyArray<SidebarMenuEntry<Message>>
}>

const sidebarRoot = basecoatClass('sidebar')

const uniqueIds = (ids: ReadonlyArray<string>): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const next: Array<string> = []

  for (const id of ids) {
    if (id === '' || seen.has(id)) {
      continue
    }

    seen.add(id)
    next.push(id)
  }

  return next
}

const normalizedBreakpoint = (breakpoint: number | undefined): number =>
  breakpoint === undefined ||
  !Number.isFinite(breakpoint) ||
  breakpoint <= 0
    ? DEFAULT_SIDEBAR_BREAKPOINT
    : Math.floor(breakpoint)

const openForViewport = (input: {
  initialOpen: boolean
  initialMobileOpen: boolean
  breakpoint: number
  viewportWidth: number | null
}): boolean =>
  input.viewportWidth === null
    ? input.initialOpen
    : input.viewportWidth >= input.breakpoint
      ? input.initialOpen
      : input.initialMobileOpen

export const initSidebar = (input: SidebarInit = {}): SidebarModel => {
  const initialOpen = input.initialOpen !== false
  const initialMobileOpen = input.initialMobileOpen === true
  const breakpoint = normalizedBreakpoint(input.breakpoint)
  const viewportWidth = input.viewportWidth ?? null

  return {
    open: openForViewport({
      initialOpen,
      initialMobileOpen,
      breakpoint,
      viewportWidth,
    }),
    initialOpen,
    initialMobileOpen,
    breakpoint,
    viewportWidth,
    selectedItemId: input.selectedItemId ?? null,
    focusedItemId: input.focusedItemId ?? input.selectedItemId ?? null,
    openSubmenuIds: uniqueIds(input.openSubmenuIds ?? []),
    closedSubmenuIds: uniqueIds(input.closedSubmenuIds ?? []),
  }
}

export const isMobileSidebar = (model: SidebarModel): boolean =>
  model.viewportWidth !== null && model.viewportWidth < model.breakpoint

export const openSidebar = (model: SidebarModel): SidebarModel => ({
  ...model,
  open: true,
})

export const closeSidebar = (model: SidebarModel): SidebarModel => ({
  ...model,
  open: false,
  focusedItemId: null,
})

export const toggleSidebar = (model: SidebarModel): SidebarModel =>
  model.open ? closeSidebar(model) : openSidebar(model)

const addId = (
  ids: ReadonlyArray<string>,
  id: string,
): ReadonlyArray<string> => uniqueIds([...ids, id])

const removeId = (
  ids: ReadonlyArray<string>,
  id: string,
): ReadonlyArray<string> => ids.filter(value => value !== id)

const activateItem = (
  model: SidebarModel,
  input: SidebarActivateItemInput,
): SidebarModel => {
  const shouldClose = isMobileSidebar(model) && input.keepMobileSidebarOpen !== true

  return {
    ...model,
    open: shouldClose ? false : model.open,
    selectedItemId: input.itemId,
    focusedItemId: shouldClose ? null : input.itemId,
  }
}

const focusItemForKey = (
  model: SidebarModel,
  key: string,
  itemIds: ReadonlyArray<string>,
): string | null => {
  if (itemIds.length === 0) {
    return null
  }

  if (key === 'Home') {
    return itemIds[0] ?? null
  }

  if (key === 'End') {
    return itemIds[itemIds.length - 1] ?? null
  }

  const currentId = model.focusedItemId ?? model.selectedItemId
  const currentIndex = currentId === null ? -1 : itemIds.indexOf(currentId)

  if (key === 'ArrowDown' || key === 'PageDown') {
    return itemIds[(currentIndex + 1 + itemIds.length) % itemIds.length] ?? null
  }

  if (key === 'ArrowUp' || key === 'PageUp') {
    return itemIds[
      (currentIndex === -1 ? itemIds.length - 1 : currentIndex - 1 + itemIds.length) %
        itemIds.length
    ] ?? null
  }

  return null
}

const isActivationKey = (key: string): boolean => key === 'Enter' || key === ' '

const isHandledSidebarKey = (key: string): boolean =>
  key === 'Escape' ||
  key === 'ArrowDown' ||
  key === 'ArrowUp' ||
  key === 'Home' ||
  key === 'End' ||
  key === 'PageDown' ||
  key === 'PageUp' ||
  isActivationKey(key)

const submenuOpen = (model: SidebarModel, submenuId: string): boolean =>
  model.openSubmenuIds.includes(submenuId) &&
  !model.closedSubmenuIds.includes(submenuId)

export const updateSidebar = (
  model: SidebarModel,
  message: SidebarMessage,
): SidebarModel => {
  switch (message._tag) {
    case 'SidebarOpened':
      return openSidebar(model)
    case 'SidebarClosed':
    case 'SidebarClickedOverlay':
      return closeSidebar(model)
    case 'SidebarToggled':
      return toggleSidebar(model)
    case 'SidebarSetOpen':
      return message.open ? openSidebar(model) : closeSidebar(model)
    case 'SidebarViewportChanged':
      return {
        ...model,
        viewportWidth: message.viewportWidth,
      }
    case 'SidebarActivatedItem':
      return activateItem(model, message)
    case 'SidebarFocusedItem':
      return {
        ...model,
        focusedItemId: message.itemId,
      }
    case 'SidebarBlurredItem':
      return model.focusedItemId === message.itemId
        ? {
            ...model,
            focusedItemId: null,
          }
        : model
    case 'SidebarPressedKey': {
      if (message.key === 'Escape') {
        return closeSidebar(model)
      }

      if (isActivationKey(message.key)) {
        const itemId = model.focusedItemId ?? model.selectedItemId
        return itemId === null
          ? model
          : activateItem(model, {
              itemId,
              keepMobileSidebarOpen:
                message.keepMobileSidebarOpenItemIds.includes(itemId),
            })
      }

      const focusedItemId = focusItemForKey(model, message.key, message.itemIds)
      return focusedItemId === null
        ? model
        : {
            ...model,
            focusedItemId,
          }
    }
    case 'SidebarToggledSubmenu': {
      const nextOpen =
        message.open === null
          ? !submenuOpen(model, message.submenuId)
          : message.open

      return {
        ...model,
        openSubmenuIds: nextOpen
          ? addId(model.openSubmenuIds, message.submenuId)
          : removeId(model.openSubmenuIds, message.submenuId),
        closedSubmenuIds: nextOpen
          ? removeId(model.closedSubmenuIds, message.submenuId)
          : addId(model.closedSubmenuIds, message.submenuId),
      }
    }
  }
}

export const sidebarItemIds = <Message>(
  entries: ReadonlyArray<SidebarMenuEntry<Message>>,
): ReadonlyArray<string> =>
  uniqueIds(
    entries.flatMap(entry => {
      if (entry.type === 'group') {
        return sidebarItemIds(entry.items)
      }

      if (entry.type === 'separator') {
        return []
      }

      if (entry.type === 'submenu') {
        return [entry.id, ...sidebarItemIds(entry.items)]
      }

      return entry.disabled === true || entry.ariaDisabled === true
        ? []
        : [entry.id]
    }),
  )

export const sidebarVisibleItemIds = <Message>(
  model: SidebarModel,
  entries: ReadonlyArray<SidebarMenuEntry<Message>>,
): ReadonlyArray<string> =>
  uniqueIds(
    entries.flatMap(entry => {
      if (entry.type === 'group') {
        return sidebarVisibleItemIds(model, entry.items)
      }

      if (entry.type === 'separator') {
        return []
      }

      if (entry.type === 'submenu') {
        return [
          entry.id,
          ...(isSubmenuOpenForView(model, entry)
            ? sidebarVisibleItemIds(model, entry.items)
            : []),
        ]
      }

      return entry.disabled === true || entry.ariaDisabled === true
        ? []
        : [entry.id]
    }),
  )

export const sidebarKeepMobileOpenItemIds = <Message>(
  entries: ReadonlyArray<SidebarMenuEntry<Message>>,
): ReadonlyArray<string> =>
  uniqueIds(
    entries.flatMap(entry => {
      if (entry.type === 'group' || entry.type === 'submenu') {
        return sidebarKeepMobileOpenItemIds(entry.items)
      }

      if (entry.type === 'separator') {
        return []
      }

      return entry.keepMobileSidebarOpen === true ? [entry.id] : []
    }),
  )

const slotAttrs = <Message>(
  input: BasecoatAttrs<Message> | undefined,
): ReadonlyArray<Attribute<Message>> =>
  basecoatAttrs<Message>(input ?? {})

const itemFocusedId = <Message>(
  model: SidebarModel,
  entries: ReadonlyArray<SidebarMenuEntry<Message>>,
): string | null => {
  const itemIds = sidebarVisibleItemIds(model, entries)
  const preferred = model.focusedItemId ?? model.selectedItemId

  if (preferred !== null && itemIds.includes(preferred)) {
    return preferred
  }

  return itemIds[0] ?? null
}

const itemControlAttrs = <Message>(
  input: SidebarItemProps<Message>,
  model: SidebarModel,
  actions: SidebarViewActions<Message> | undefined,
  focusedId: string | null,
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const active =
    input.active === true ||
    input.current === true ||
    model.selectedItemId === input.id
  const disabled = input.disabled === true || input.ariaDisabled === true

  return [
    ...basecoatAttrs<Message>(input),
    h.DataAttribute('sidebar-item-id', input.id),
    h.Tabindex(disabled ? -1 : input.id === focusedId ? 0 : -1),
    ...(input.id === focusedId ? [h.DataAttribute('focused', 'true')] : []),
    ...(active ? [h.DataAttribute('active', 'true')] : []),
    ...(input.current === true ? [h.AriaCurrent('page')] : []),
    ...(input.ariaDisabled === true ? [h.AriaDisabled(true)] : []),
    ...(input.keepMobileSidebarOpen === true
      ? [h.DataAttribute('keep-mobile-sidebar-open', '')]
      : []),
    ...dataAttr<Message>('variant', input.variant),
    ...dataAttr<Message>('size', input.size),
    ...(actions === undefined || disabled
      ? []
      : [
          h.OnClick(
            actions.activatedItem({
              itemId: input.id,
              keepMobileSidebarOpen: input.keepMobileSidebarOpen === true,
            }),
          ),
          h.OnFocus(actions.focusedItem(input.id)),
          h.OnBlur(actions.blurredItem(input.id)),
        ]),
  ]
}

const submenuSummaryAttrs = <Message>(
  input: SidebarSubmenuProps<Message>,
  model: SidebarModel,
  actions: SidebarViewActions<Message> | undefined,
  focusedId: string | null,
  contentId: string,
): ReadonlyArray<Attribute<Message>> => {
  const h = html<Message>()
  const active =
    input.active === true ||
    input.current === true ||
    model.selectedItemId === input.id

  return [
    ...slotAttrs(input),
    h.DataAttribute('sidebar-item-id', input.id),
    h.AriaControls(contentId),
    h.Tabindex(input.id === focusedId ? 0 : -1),
    ...(input.id === focusedId ? [h.DataAttribute('focused', 'true')] : []),
    ...(active ? [h.DataAttribute('active', 'true')] : []),
    ...(input.current === true ? [h.AriaCurrent('page')] : []),
    ...dataAttr<Message>('variant', input.variant),
    ...dataAttr<Message>('size', input.size),
    ...(actions === undefined
      ? []
      : [
          h.OnFocus(actions.focusedItem(input.id)),
          h.OnBlur(actions.blurredItem(input.id)),
        ]),
  ]
}

const menuItemChildren = <Message>(input: {
  icon?: Html
  label: BasecoatChildren
}): BasecoatChildren => {
  const h = html<Message>()
  return [input.icon ?? null, h.span([], input.label)]
}

const isSubmenuOpenForView = <Message>(
  model: SidebarModel,
  input: SidebarSubmenuProps<Message>,
): boolean => {
  if (model.closedSubmenuIds.includes(input.id)) {
    return false
  }

  return model.openSubmenuIds.includes(input.id) || input.open === true
}

const renderSidebarEntry = <Message>(
  entry: SidebarMenuEntry<Message>,
  model: SidebarModel,
  actions: SidebarViewActions<Message> | undefined,
  focusedId: string | null,
): Html => {
  const h = html<Message>()

  if (entry.type === 'group') {
    const labelId = entry.labelId ?? `${entry.id}-label`

    return h.div(
      [
        ...basecoatAttrs<Message>(entry),
        h.Role('group'),
        ...(entry.label === undefined ? [] : [h.AriaLabelledBy(labelId)]),
      ],
      [
        entry.label === undefined
          ? null
          : h.h3(
              [
                ...slotAttrs(entry.headingAttrs),
                h.Id(labelId),
              ],
              entry.label,
            ),
        h.ul(
          [],
          entry.items.map(item =>
            renderSidebarEntry(item, model, actions, focusedId),
          ),
        ),
      ],
    )
  }

  if (entry.type === 'separator') {
    return h.hr([
      ...basecoatAttrs<Message>(entry),
      ...(entry.id === undefined ? [] : [h.Id(entry.id)]),
      h.Role('separator'),
    ])
  }

  if (entry.type === 'submenu') {
    const contentId = `${entry.id}-content`
    const open = isSubmenuOpenForView(model, entry)

    return h.li(
      basecoatAttrs<Message>(entry.liAttrs ?? {}),
      [
        h.details(
          [
            ...slotAttrs(entry.detailsAttrs),
            h.Id(entry.id),
            ...(open ? [h.Open(true)] : []),
            ...(actions === undefined
              ? []
              : [
                  h.OnToggle(isOpen =>
                    actions.toggledSubmenu({
                      submenuId: entry.id,
                      open: isOpen,
                    }),
                  ),
                ]),
          ],
          [
            h.summary(
              submenuSummaryAttrs(
                entry,
                model,
                actions,
                focusedId,
                contentId,
              ),
              menuItemChildren<Message>(entry),
            ),
            h.ul(
              [
                ...slotAttrs(entry.listAttrs),
                h.Id(contentId),
              ],
              entry.items.map(item =>
                renderSidebarEntry(item, model, actions, focusedId),
              ),
            ),
          ],
        ),
      ],
    )
  }

  const controlAttrs = itemControlAttrs(entry, model, actions, focusedId)
  const children = menuItemChildren<Message>(entry)

  return h.li(
    basecoatAttrs<Message>(entry.liAttrs ?? {}),
    [
      entry.href === undefined
        ? h.button(
            [
              ...controlAttrs,
              h.Type('button'),
              ...(entry.disabled === true ? [h.Disabled(true)] : []),
            ],
            children,
          )
        : h.a(
            [
              ...controlAttrs,
              h.Href(entry.href),
            ],
            children,
          ),
    ],
  )
}

const keyboardAttrs = <Message>(
  model: SidebarModel,
  actions: SidebarViewActions<Message> | undefined,
  menu: ReadonlyArray<SidebarMenuEntry<Message>> | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (actions === undefined || menu === undefined) {
    return []
  }

  const itemIds = sidebarVisibleItemIds(model, menu)
  const keepMobileSidebarOpenItemIds = sidebarKeepMobileOpenItemIds(menu)

  if (itemIds.length === 0) {
    return []
  }

  return [
    html<Message>().OnKeyDownPreventDefault(key =>
      isHandledSidebarKey(key)
        ? Option.some(
            actions.pressedKey({
              key,
              itemIds,
              keepMobileSidebarOpenItemIds,
            }),
          )
        : Option.none(),
    ),
  ]
}

export const sidebar = <Message>(input: SidebarProps<Message>): Html => {
  const h = html<Message>()
  const menu = input.menu
  const focusedId = menu === undefined ? null : itemFocusedId(input.model, menu)
  const contentChildren =
    menu === undefined
      ? input.children ?? []
      : menu.map(entry =>
          renderSidebarEntry(entry, input.model, input.actions, focusedId),
        )

  return h.aside(
    [
      ...basecoatAttrs<Message>(input, sidebarRoot),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      h.DataAttribute('side', input.side ?? 'left'),
      h.DataAttribute('sidebar-initialized', 'true'),
      h.DataAttribute('initial-open', String(input.model.initialOpen)),
      h.DataAttribute(
        'initial-mobile-open',
        String(input.model.initialMobileOpen),
      ),
      h.DataAttribute('breakpoint', String(input.model.breakpoint)),
      h.AriaHidden(!input.model.open),
      ...(input.model.open ? [] : [h.Inert(true)]),
    ],
    [
      h.nav(
        [
          ...slotAttrs(input.navAttrs),
          h.AriaLabel(input.label ?? 'Sidebar navigation'),
          ...keyboardAttrs(input.model, input.actions, menu),
        ],
        [
          input.header === undefined
            ? null
            : h.header(slotAttrs(input.header), input.header.children),
          h.section(slotAttrs(input.contentAttrs), contentChildren),
          input.footer === undefined
            ? null
            : h.footer(slotAttrs(input.footer), input.footer.children),
        ],
      ),
    ],
  )
}
