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

export type DropdownMenuItemType = 'item' | 'checkbox' | 'radio'
export type DropdownMenuInitialSelection = false | 'first' | 'last'

export type DropdownMenuModelItem = Readonly<{
  value: string
  type?: DropdownMenuItemType
  disabled?: boolean
  radioGroup?: string
}>

export type DropdownMenuModel = Readonly<{
  items: ReadonlyArray<DropdownMenuModelItem>
  open: boolean
  activeValue: string | null
  selectedValue: string | null
  checkedValues: ReadonlyArray<string>
  radioValues: Readonly<Record<string, string>>
}>

export type DropdownMenuInit = Readonly<{
  items: ReadonlyArray<DropdownMenuModelItem>
  open?: boolean
  initialSelection?: DropdownMenuInitialSelection
  activeValue?: string | null
  selectedValue?: string | null
  checkedValues?: ReadonlyArray<string>
  radioValues?: Readonly<Record<string, string>>
}>

export type DropdownMenuMessage =
  | Readonly<{ readonly _tag: 'DropdownMenuOpened'; readonly initialSelection?: DropdownMenuInitialSelection }>
  | Readonly<{ readonly _tag: 'DropdownMenuClosed' }>
  | Readonly<{ readonly _tag: 'DropdownMenuToggled' }>
  | Readonly<{ readonly _tag: 'DropdownMenuItemActivated'; readonly value: string | null }>
  | Readonly<{ readonly _tag: 'DropdownMenuItemHovered'; readonly value: string }>
  | Readonly<{ readonly _tag: 'DropdownMenuMouseLeft' }>
  | Readonly<{ readonly _tag: 'DropdownMenuKeyDown'; readonly key: string }>
  | Readonly<{ readonly _tag: 'DropdownMenuPopoverOpenedElsewhere' }>

export type DropdownMenuProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  id?: string
  initialized?: boolean
  onKeyDown?: (key: string) => Message | null
}>

export type DropdownMenuTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  controlsId?: string
  open?: boolean
  activeDescendantId?: string
  disabled?: boolean
  onClick?: Message
}>

export type DropdownMenuPopoverProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  open?: boolean
}>

export type DropdownMenuMenuProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
  labelledBy?: string
  onMouseLeave?: Message
}>

export type DropdownMenuItemProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  type?: DropdownMenuItemType
  id?: string
  href?: string
  disabled?: boolean
  active?: boolean
  checked?: boolean
  shortcut?: BasecoatChildren
  indicator?: BasecoatChildren
  onClick?: Message
  onMouseEnter?: Message
}>

export type DropdownMenuGroupProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  labelledBy?: string
}>

export type DropdownMenuLabelProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
  id?: string
}>

export type DropdownMenuSeparatorProps<Message> = BasecoatAttrs<Message>

export type DropdownMenuShortcutProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type DropdownMenuIndicatorProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: BasecoatChildren
}>

export type DropdownMenuViewItem<Message> =
  | (DropdownMenuItemProps<Message> & Readonly<{
      value: string
      itemType?: DropdownMenuItemType
      radioGroup?: string
    }>)
  | Readonly<{
      type: 'separator'
      className?: string
      attrs?: ReadonlyArray<Attribute<Message>>
    }>
  | Readonly<{
      type: 'group'
      label?: BasecoatChildren
      labelId?: string
      children: ReadonlyArray<DropdownMenuViewItem<Message>>
      className?: string
      attrs?: ReadonlyArray<Attribute<Message>>
    }>

export type DropdownMenuViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: DropdownMenuModel
  trigger: BasecoatChildren
  items: ReadonlyArray<DropdownMenuViewItem<Message>>
  toMessage: (message: DropdownMenuMessage) => Message
  id?: string
}>

const dropdownMenuRoot = basecoatClass('dropdown-menu')
const activeClass = basecoatClass('active')

const enabledItems = (model: DropdownMenuModel): ReadonlyArray<DropdownMenuModelItem> =>
  model.items.filter(item => item.disabled !== true)

const findItem = (
  model: DropdownMenuModel,
  value: string,
): DropdownMenuModelItem | undefined =>
  model.items.find(item => item.value === value)

const isEnabledValue = (model: DropdownMenuModel, value: string | null): value is string => {
  if (value === null) {
    return false
  }

  const item = findItem(model, value)
  return item !== undefined && item.disabled !== true
}

const normalizeCheckedValues = (
  items: ReadonlyArray<DropdownMenuModelItem>,
  checkedValues: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const allowed = new Set(
    items
      .filter(item => item.type === 'checkbox' && item.disabled !== true)
      .map(item => item.value),
  )

  return checkedValues.filter((value, index) =>
    allowed.has(value) && checkedValues.indexOf(value) === index
  )
}

const normalizeRadioValues = (
  items: ReadonlyArray<DropdownMenuModelItem>,
  radioValues: Readonly<Record<string, string>>,
): Readonly<Record<string, string>> => {
  const next: Record<string, string> = {}

  for (const item of items) {
    if (item.type !== 'radio' || item.disabled === true) {
      continue
    }

    const group = item.radioGroup ?? 'default'
    if (radioValues[group] === item.value) {
      next[group] = item.value
    }
  }

  return next
}

const itemByInitialSelection = (
  model: DropdownMenuModel,
  initialSelection: DropdownMenuInitialSelection | undefined,
): string | null => {
  if (initialSelection === false || initialSelection === undefined) {
    return null
  }

  const items = enabledItems(model)
  return initialSelection === 'last'
    ? items[items.length - 1]?.value ?? null
    : items[0]?.value ?? null
}

const openWithSelection = (
  model: DropdownMenuModel,
  initialSelection: DropdownMenuInitialSelection | undefined,
): DropdownMenuModel => ({
  ...model,
  open: true,
  activeValue: itemByInitialSelection(model, initialSelection),
})

const close = (model: DropdownMenuModel): DropdownMenuModel => ({
  ...model,
  open: false,
  activeValue: null,
})

const moveActive = (
  model: DropdownMenuModel,
  direction: 'next' | 'previous' | 'first' | 'last',
): DropdownMenuModel => {
  const items = enabledItems(model)
  if (items.length === 0) {
    return model
  }

  const currentIndex = items.findIndex(item => item.value === model.activeValue)
  const nextIndex =
    direction === 'first'
      ? 0
      : direction === 'last'
        ? items.length - 1
        : direction === 'next'
          ? currentIndex === -1
            ? 0
            : Math.min(currentIndex + 1, items.length - 1)
          : currentIndex === -1
            ? items.length - 1
            : Math.max(currentIndex - 1, 0)

  return { ...model, activeValue: items[nextIndex]?.value ?? null }
}

const activateItem = (
  model: DropdownMenuModel,
  value: string | null,
): DropdownMenuModel => {
  if (!isEnabledValue(model, value)) {
    return close(model)
  }

  const item = findItem(model, value)
  if (item === undefined) {
    return close(model)
  }

  if (item.type === 'checkbox') {
    const checkedValues = model.checkedValues.includes(value)
      ? model.checkedValues.filter(candidate => candidate !== value)
      : [...model.checkedValues, value]

    return close({
      ...model,
      checkedValues,
      selectedValue: value,
    })
  }

  if (item.type === 'radio') {
    const group = item.radioGroup ?? 'default'

    return close({
      ...model,
      radioValues: {
        ...model.radioValues,
        [group]: value,
      },
      selectedValue: value,
    })
  }

  return close({
    ...model,
    selectedValue: value,
  })
}

export const dropdownMenuInit = (input: DropdownMenuInit): DropdownMenuModel => {
  const base: DropdownMenuModel = {
    items: input.items,
    open: input.open === true,
    activeValue: null,
    selectedValue: input.selectedValue ?? null,
    checkedValues: normalizeCheckedValues(input.items, input.checkedValues ?? []),
    radioValues: normalizeRadioValues(input.items, input.radioValues ?? {}),
  }

  const activeValue =
    input.activeValue !== undefined && isEnabledValue(base, input.activeValue)
      ? input.activeValue
      : input.open === true
        ? itemByInitialSelection(base, input.initialSelection)
        : null

  return { ...base, activeValue }
}

export const dropdownMenuUpdate = (
  model: DropdownMenuModel,
  message: DropdownMenuMessage,
): DropdownMenuModel => {
  switch (message._tag) {
    case 'DropdownMenuOpened':
      return openWithSelection(model, message.initialSelection)
    case 'DropdownMenuClosed':
    case 'DropdownMenuPopoverOpenedElsewhere':
      return close(model)
    case 'DropdownMenuToggled':
      return model.open ? close(model) : openWithSelection(model, false)
    case 'DropdownMenuItemActivated':
      return activateItem(model, message.value)
    case 'DropdownMenuItemHovered':
      return isEnabledValue(model, message.value)
        ? { ...model, activeValue: message.value }
        : model
    case 'DropdownMenuMouseLeft':
      return { ...model, activeValue: null }
    case 'DropdownMenuKeyDown': {
      if (!model.open) {
        switch (message.key) {
          case 'Enter':
          case ' ':
          case 'Spacebar':
            return openWithSelection(model, false)
          case 'ArrowDown':
            return openWithSelection(model, 'first')
          case 'ArrowUp':
            return openWithSelection(model, 'last')
          default:
            return model
        }
      }

      switch (message.key) {
        case 'Escape':
          return close(model)
        case 'ArrowDown':
          return moveActive(model, 'next')
        case 'ArrowUp':
          return moveActive(model, 'previous')
        case 'Home':
          return moveActive(model, 'first')
        case 'End':
          return moveActive(model, 'last')
        case 'Enter':
        case ' ':
        case 'Spacebar':
          return activateItem(model, model.activeValue)
        default:
          return model
      }
    }
  }
}

const handledDropdownMenuKeys = new Set([
  'Escape',
  'Enter',
  ' ',
  'Spacebar',
  'ArrowDown',
  'ArrowUp',
  'Home',
  'End',
])

export const dropdownMenu = <Message>(input: DropdownMenuProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, dropdownMenuRoot),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...dataAttr<Message>('dropdown-menu-initialized', input.initialized === true ? 'true' : undefined),
      ...(input.onKeyDown === undefined
        ? []
        : [
            h.OnKeyDownPreventDefault(key =>
              handledDropdownMenuKeys.has(key)
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

export const dropdownMenuTrigger = <Message>(
  input: DropdownMenuTriggerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.AriaHasPopup('menu'),
      h.AriaExpanded(input.open === true),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.controlsId === undefined ? [] : [h.AriaControls(input.controlsId)]),
      ...(input.activeDescendantId === undefined ? [] : [h.AriaActiveDescendant(input.activeDescendantId)]),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...(input.onClick === undefined || input.disabled === true ? [] : [h.OnClick(input.onClick)]),
    ],
    input.children,
  )
}

export const dropdownMenuPopover = <Message>(
  input: DropdownMenuPopoverProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      h.DataAttribute('popover', ''),
      h.AriaHidden(input.open === true ? false : true),
    ],
    input.children,
  )
}

export const dropdownMenuMenu = <Message>(
  input: DropdownMenuMenuProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('menu'),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.labelledBy === undefined ? [] : [h.AriaLabelledBy(input.labelledBy)]),
      ...(input.onMouseLeave === undefined ? [] : [h.OnMouseLeave(input.onMouseLeave)]),
    ],
    input.children,
  )
}

export const dropdownMenuShortcut = <Message>(
  input: DropdownMenuShortcutProps<Message>,
): Html => {
  const h = html<Message>()

  return h.kbd(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('shortcut', ''),
    ],
    input.children,
  )
}

export const dropdownMenuIndicator = <Message>(
  input: DropdownMenuIndicatorProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('indicator', ''),
      h.AriaHidden(true),
    ],
    input.children,
  )
}

export const dropdownMenuItem = <Message>(
  input: DropdownMenuItemProps<Message>,
): Html => {
  const h = html<Message>()
  const itemType = input.type ?? 'item'
  const role =
    itemType === 'checkbox'
      ? 'menuitemcheckbox'
      : itemType === 'radio'
        ? 'menuitemradio'
        : 'menuitem'
  const children = [
    ...(input.indicator === undefined ? [] : [dropdownMenuIndicator<Message>({ children: input.indicator })]),
    ...input.children,
    ...(input.shortcut === undefined ? [] : [dropdownMenuShortcut<Message>({ children: input.shortcut })]),
  ]
  const attrs = [
    ...basecoatAttrs<Message>(input, input.active === true ? activeClass : null),
    h.Role(role),
    ...(input.id === undefined ? [] : [h.Id(input.id)]),
    ...(input.disabled === true ? [h.AriaDisabled(true)] : []),
    ...(itemType === 'checkbox' || itemType === 'radio'
      ? [h.AriaChecked(input.checked === true)]
      : []),
    ...(input.onClick === undefined || input.disabled === true ? [] : [h.OnClick(input.onClick)]),
    ...(input.onMouseEnter === undefined || input.disabled === true ? [] : [h.OnMouseEnter(input.onMouseEnter)]),
  ]

  if (input.href !== undefined) {
    return h.a(
      [
        ...attrs,
        h.Href(input.href),
      ],
      children,
    )
  }

  return h.button(
    [
      ...attrs,
      h.Type('button'),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
    ],
    children,
  )
}

export const dropdownMenuCheckboxItem = <Message>(
  input: Omit<DropdownMenuItemProps<Message>, 'type'>,
): Html =>
  dropdownMenuItem<Message>({
    ...input,
    type: 'checkbox',
  })

export const dropdownMenuRadioItem = <Message>(
  input: Omit<DropdownMenuItemProps<Message>, 'type'>,
): Html =>
  dropdownMenuItem<Message>({
    ...input,
    type: 'radio',
  })

export const dropdownMenuGroup = <Message>(
  input: DropdownMenuGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('group'),
      ...(input.labelledBy === undefined ? [] : [h.AriaLabelledBy(input.labelledBy)]),
    ],
    input.children,
  )
}

export const dropdownMenuLabel = <Message>(
  input: DropdownMenuLabelProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('heading'),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
    ],
    input.children,
  )
}

export const dropdownMenuSeparator = <Message>(
  input: DropdownMenuSeparatorProps<Message>,
): Html => {
  const h = html<Message>()

  return h.hr([
    ...basecoatAttrs<Message>(input),
    h.Role('separator'),
  ])
}

const itemChecked = <Message>(
  model: DropdownMenuModel,
  item: DropdownMenuViewItem<Message>,
): boolean => {
  if (!('value' in item)) {
    return false
  }

  const itemType = item.itemType ?? item.type ?? 'item'
  if (itemType === 'checkbox') {
    return model.checkedValues.includes(item.value)
  }

  if (itemType === 'radio') {
    return model.radioValues[item.radioGroup ?? 'default'] === item.value
  }

  return false
}

const renderViewItems = <Message>(
  model: DropdownMenuModel,
  items: ReadonlyArray<DropdownMenuViewItem<Message>>,
  toMessage: (message: DropdownMenuMessage) => Message,
  menuId: string,
): ReadonlyArray<Html> =>
  items.map((item, index) => {
    if ('type' in item && item.type === 'separator') {
      return dropdownMenuSeparator<Message>({
        ...(item.attrs === undefined ? {} : { attrs: item.attrs }),
        ...(item.className === undefined ? {} : { className: item.className }),
      })
    }

    if ('type' in item && item.type === 'group') {
      const labelId = item.labelId ?? `${menuId}-group-${index}-label`
      return dropdownMenuGroup<Message>({
        ...(item.attrs === undefined ? {} : { attrs: item.attrs }),
        ...(item.className === undefined ? {} : { className: item.className }),
        ...(item.label === undefined ? {} : { labelledBy: labelId }),
        children: [
          ...(item.label === undefined
            ? []
            : [dropdownMenuLabel<Message>({ id: labelId, children: item.label })]),
          ...renderViewItems(model, item.children, toMessage, `${menuId}-group-${index}`),
        ],
      })
    }

    const itemType = item.itemType ?? item.type ?? 'item'
    const id = item.id ?? `${menuId}-${item.value}`
    return dropdownMenuItem<Message>({
      ...item,
      id,
      type: itemType,
      active: model.activeValue === item.value,
      checked: itemChecked(model, item),
      onClick: toMessage({
        _tag: 'DropdownMenuItemActivated',
        value: item.value,
      }),
      onMouseEnter: toMessage({
        _tag: 'DropdownMenuItemHovered',
        value: item.value,
      }),
    })
  })

export const dropdownMenuView = <Message>(
  input: DropdownMenuViewProps<Message>,
): Html => {
  const id = input.id ?? 'dropdown-menu'
  const triggerId = `${id}-trigger`
  const popoverId = `${id}-popover`
  const menuId = `${id}-menu`

  return dropdownMenu<Message>({
    id,
    initialized: true,
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
    onKeyDown: key =>
      input.toMessage({
        _tag: 'DropdownMenuKeyDown',
        key,
      }),
    children: [
      dropdownMenuTrigger<Message>({
        id: triggerId,
        controlsId: menuId,
        open: input.model.open,
        ...(input.model.activeValue === null
          ? {}
          : { activeDescendantId: `${menuId}-${input.model.activeValue}` }),
        onClick: input.toMessage({ _tag: 'DropdownMenuToggled' }),
        children: input.trigger,
      }),
      dropdownMenuPopover<Message>({
        id: popoverId,
        open: input.model.open,
        children: [
          dropdownMenuMenu<Message>({
            id: menuId,
            labelledBy: triggerId,
            onMouseLeave: input.toMessage({ _tag: 'DropdownMenuMouseLeft' }),
            children: renderViewItems(input.model, input.items, input.toMessage, menuId),
          }),
        ],
      }),
    ],
  })
}
