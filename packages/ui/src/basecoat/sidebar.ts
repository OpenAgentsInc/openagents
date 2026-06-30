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

export type SidebarRichMessage =
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

export const sidebarMessageActions: SidebarViewActions<SidebarRichMessage> = {
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

export type SidebarRichProps<Message> = BasecoatAttrs<Message> & Readonly<{
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
  message: SidebarRichMessage,
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

const sidebarRich = <Message>(input: SidebarRichProps<Message>): Html => {
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

export type SidebarPrimitiveItem = Readonly<{
  value: string
  disabled?: boolean
}>

export type SidebarPrimitiveModel = Readonly<{
  open: boolean
  breakpoint: number
  items: ReadonlyArray<SidebarPrimitiveItem>
  focusedValue: string | null
  selectedValue: string | null
}>

export type SidebarPrimitiveInit = Readonly<{
  items: ReadonlyArray<SidebarPrimitiveItem>
  initialOpen?: boolean
  initialMobileOpen?: boolean
  breakpoint?: number
  viewportWidth?: number
  focusedValue?: string | null
  selectedValue?: string | null
}>

export type SidebarPrimitiveMessage =
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

export type SidebarPrimitiveProps<Message> = BasecoatAttrs<Message> & Readonly<{
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

export type SidebarPrimitiveGroupProps<Message> = SidebarSlotProps<Message> & Readonly<{
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

export type SidebarViewItem<Message> = SidebarPrimitiveItem & Readonly<{
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
  model: SidebarPrimitiveModel
  groups: ReadonlyArray<SidebarViewGroup<Message>>
  toMessage: (message: SidebarPrimitiveMessage) => Message
  mobile?: boolean
  side?: SidebarSide
  label?: string
}>

const enabledItems = (model: SidebarPrimitiveModel): ReadonlyArray<SidebarPrimitiveItem> =>
  model.items.filter(item => item.disabled !== true)

const hasItem = (items: ReadonlyArray<SidebarPrimitiveItem>, value: string): boolean =>
  items.some(item => item.value === value)

const isDisabled = (model: SidebarPrimitiveModel, value: string): boolean =>
  model.items.find(item => item.value === value)?.disabled === true

const focusByOffset = (model: SidebarPrimitiveModel, value: string, offset: number): SidebarPrimitiveModel => {
  const items = enabledItems(model)
  const index = items.findIndex(item => item.value === value)

  if (items.length === 0 || index === -1) {
    return model
  }

  const next = items[(index + offset + items.length) % items.length]

  return { ...model, focusedValue: next?.value ?? model.focusedValue }
}

const selectItem = (model: SidebarPrimitiveModel, value: string): SidebarPrimitiveModel =>
  isDisabled(model, value) || !hasItem(model.items, value)
    ? model
    : { ...model, selectedValue: value, focusedValue: value }

export const sidebarInit = (input: SidebarPrimitiveInit): SidebarPrimitiveModel => {
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
  model: SidebarPrimitiveModel,
  message: SidebarPrimitiveMessage,
): SidebarPrimitiveModel => {
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

const sidebarPrimitive = <Message>(input: SidebarPrimitiveProps<Message>): Html => {
  const h = html<Message>()

  return h.aside(
    [
      ...basecoatAttrs<Message>(input, sidebarRoot),
      h.DataAttribute('sidebar-initialized', 'true'),
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

export const sidebarGroup = <Message>(input: SidebarPrimitiveGroupProps<Message>): Html => {
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

  return sidebarPrimitive<Message>({
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


export type SidebarMessage = SidebarRichMessage | SidebarPrimitiveMessage
export type SidebarProps<Message> =
  | SidebarRichProps<Message>
  | SidebarPrimitiveProps<Message>

export const sidebar = <Message>(input: SidebarProps<Message>): Html =>
  'model' in input
    ? sidebarRich<Message>(input)
    : sidebarPrimitive<Message>(input)
