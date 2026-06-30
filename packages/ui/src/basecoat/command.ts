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

export type CommandItem = Readonly<{
  value: string
  label: string
  disabled?: boolean
  hidden?: boolean
  force?: boolean
  filter?: string
  keywords?: ReadonlyArray<string>
}>

export type CommandModel = Readonly<{
  open: boolean
  query: string
  manualFilter: boolean
  items: ReadonlyArray<CommandItem>
  activeValue: string | null
  selectedValue: string | null
}>

export type CommandInit = Readonly<{
  open?: boolean
  query?: string
  manualFilter?: boolean
  items: ReadonlyArray<CommandItem>
  activeValue?: string | null
  selectedValue?: string | null
}>

export type CommandKey =
  | 'ArrowDown'
  | 'ArrowUp'
  | 'Home'
  | 'End'
  | 'Enter'
  | 'Escape'

export type CommandMessage =
  | Readonly<{ _tag: 'CommandOpened' }>
  | Readonly<{ _tag: 'CommandClosed' }>
  | Readonly<{ _tag: 'CommandQueryChanged'; query: string }>
  | Readonly<{ _tag: 'CommandFocused'; value: string }>
  | Readonly<{ _tag: 'CommandSelected'; value: string; keepOpen: boolean }>
  | Readonly<{ _tag: 'CommandKeyDown'; key: CommandKey }>
  | Readonly<{ _tag: 'CommandItemsChanged'; items: ReadonlyArray<CommandItem> }>

export type CommandRootProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model?: CommandModel
  children: BasecoatChildren
  ariaLabel?: string
  labelledBy?: string
}>

export type CommandDialogProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    model: CommandModel
    children: BasecoatChildren
    onMessage?: (message: CommandMessage) => Message
    id?: string
    ariaLabel?: string
    labelledBy?: string
  }>

export type CommandTriggerProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    onMessage?: (message: CommandMessage) => Message
    disabled?: boolean
    ariaLabel?: string
  }>

export type CommandInputProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    model: CommandModel
    onMessage?: (message: CommandMessage) => Message
    id?: string
    menuId?: string
    placeholder?: string
    disabled?: boolean
    ariaLabel?: string
  }>

export type CommandMenuProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    id?: string
    empty?: string
  }>

export type CommandGroupProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    labelledBy?: string
  }>

export type CommandHeadingProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    id?: string
  }>

export type CommandItemProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
    model: CommandModel
    value: string
    filter?: string
    keywords?: ReadonlyArray<string>
    disabled?: boolean
    force?: boolean
    keepOpen?: boolean
    href?: string
    onMessage?: (message: CommandMessage) => Message
  }>

export type CommandInlineProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    children: BasecoatChildren
  }>

export type CommandViewItem<Message> = CommandItem & Readonly<{
  children: BasecoatChildren
  shortcut?: BasecoatChildren
  indicator?: BasecoatChildren
  href?: string
  keepOpen?: boolean
}>

export type CommandViewGroup<Message> = Readonly<{
  id?: string
  heading?: BasecoatChildren
  items: ReadonlyArray<CommandViewItem<Message>>
}>

export type CommandViewProps<Message> =
  BasecoatAttrs<Message> & Readonly<{
    model: CommandModel
    groups: ReadonlyArray<CommandViewGroup<Message>>
    toMessage: (message: CommandMessage) => Message
    inputId?: string
    menuId?: string
    placeholder?: string
    empty?: string
    ariaLabel?: string
  }>

const commandRoot = basecoatClass('command')
const commandDialogRoot = basecoatClass('command-dialog')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

const mappedAttr = <Message>(
  onMessage: ((message: CommandMessage) => Message) | undefined,
  message: CommandMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  onMessage === undefined ? [] : [attr(onMessage(message))]

const itemSearchText = (item: CommandItem): string =>
  (item.filter ?? item.label).trim().toLowerCase()

const keywordMatches = (
  keywords: ReadonlyArray<string> | undefined,
  query: string,
): boolean =>
  (keywords ?? []).some(keyword => keyword.toLowerCase().includes(query))

export const commandVisibleItems = (
  model: CommandModel,
): ReadonlyArray<CommandItem> => {
  const query = model.query.trim().toLowerCase()

  return model.items.filter(item => {
    if (item.disabled === true) {
      return false
    }

    if (model.manualFilter) {
      return item.hidden !== true
    }

    if (item.force === true) {
      return true
    }

    return itemSearchText(item).includes(query) || keywordMatches(item.keywords, query)
  })
}

const firstVisibleValue = (model: CommandModel): string | null =>
  commandVisibleItems(model)[0]?.value ?? null

const activeValueFor = (
  model: CommandModel,
  preferredValue: string | null | undefined,
): string | null => {
  const visibleItems = commandVisibleItems(model)
  const preferred = visibleItems.find(item => item.value === preferredValue)

  return preferred?.value ?? visibleItems[0]?.value ?? null
}

export const initCommand = (input: CommandInit): CommandModel => {
  const model: CommandModel = {
    open: input.open === true,
    query: input.query ?? '',
    manualFilter: input.manualFilter === true,
    items: input.items,
    activeValue: null,
    selectedValue: input.selectedValue ?? null,
  }

  return {
    ...model,
    activeValue: activeValueFor(model, input.activeValue),
  }
}

export const commandOpened = (): CommandMessage => ({ _tag: 'CommandOpened' })
export const commandClosed = (): CommandMessage => ({ _tag: 'CommandClosed' })
export const commandQueryChanged = (query: string): CommandMessage => ({
  _tag: 'CommandQueryChanged',
  query,
})
export const commandFocused = (value: string): CommandMessage => ({
  _tag: 'CommandFocused',
  value,
})
export const commandSelected = (
  value: string,
  keepOpen = false,
): CommandMessage => ({
  _tag: 'CommandSelected',
  value,
  keepOpen,
})
export const commandKeyDown = (key: CommandKey): CommandMessage => ({
  _tag: 'CommandKeyDown',
  key,
})
export const commandItemsChanged = (
  items: ReadonlyArray<CommandItem>,
): CommandMessage => ({
  _tag: 'CommandItemsChanged',
  items,
})

export const commandKeyMessage = (
  key: string,
  _modifiers?: KeyboardModifiers,
): CommandMessage | null => {
  switch (key) {
    case 'ArrowDown':
    case 'ArrowUp':
    case 'Home':
    case 'End':
    case 'Enter':
    case 'Escape':
      return commandKeyDown(key)
    default:
      return null
  }
}

const moveActiveValue = (
  model: CommandModel,
  offset: number,
): string | null => {
  const visibleItems = commandVisibleItems(model)

  if (visibleItems.length === 0) {
    return null
  }

  const currentIndex = visibleItems.findIndex(item => item.value === model.activeValue)
  const boundedIndex =
    currentIndex === -1
      ? 0
      : Math.min(Math.max(currentIndex + offset, 0), visibleItems.length - 1)

  return visibleItems[boundedIndex]?.value ?? null
}

export const updateCommand = (
  model: CommandModel,
  message: CommandMessage,
): CommandModel => {
  switch (message._tag) {
    case 'CommandOpened':
      return { ...model, open: true, activeValue: activeValueFor(model, model.activeValue) }
    case 'CommandClosed':
      return { ...model, open: false }
    case 'CommandQueryChanged': {
      const next = { ...model, query: message.query }
      return { ...next, activeValue: firstVisibleValue(next) }
    }
    case 'CommandFocused':
      return commandVisibleItems(model).some(item => item.value === message.value)
        ? { ...model, activeValue: message.value }
        : model
    case 'CommandSelected':
      return commandVisibleItems(model).some(item => item.value === message.value)
        ? {
            ...model,
            activeValue: message.value,
            selectedValue: message.value,
            open: message.keepOpen ? model.open : false,
          }
        : model
    case 'CommandItemsChanged': {
      const next = { ...model, items: message.items }
      return { ...next, activeValue: activeValueFor(next, next.activeValue) }
    }
    case 'CommandKeyDown':
      switch (message.key) {
        case 'ArrowDown':
          return { ...model, activeValue: moveActiveValue(model, 1) }
        case 'ArrowUp':
          return { ...model, activeValue: moveActiveValue(model, -1) }
        case 'Home':
          return { ...model, activeValue: commandVisibleItems(model)[0]?.value ?? null }
        case 'End': {
          const visibleItems = commandVisibleItems(model)
          return {
            ...model,
            activeValue: visibleItems[visibleItems.length - 1]?.value ?? null,
          }
        }
        case 'Enter':
          return model.activeValue === null
            ? model
            : updateCommand(model, commandSelected(model.activeValue, false))
        case 'Escape':
          return { ...model, open: false }
      }
  }
}

const mappedKeydownAttr = <Message>(
  onMessage: ((message: CommandMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (onMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnKeyDownPreventDefault((key, modifiers) => {
      const message = commandKeyMessage(key, modifiers)
      return message === null ? Option.none() : Option.some(onMessage(message))
    }),
  ]
}

const searchIcon = <Message>(): Html => {
  const h = html<Message>()

  return h.svg(
    [
      h.Xmlns('http://www.w3.org/2000/svg'),
      h.Width('24'),
      h.Height('24'),
      h.ViewBox('0 0 24 24'),
      h.Fill('none'),
      h.Stroke('currentColor'),
      h.StrokeWidth('2'),
      h.StrokeLinecap('round'),
      h.StrokeLinejoin('round'),
      h.Class('lucide lucide-search-icon lucide-search'),
      h.AriaHidden(true),
    ],
    [
      h.circle([h.Cx('11'), h.Cy('11'), h.R('8')], []),
      h.path([h.D('m21 21-4.3-4.3')], []),
    ],
  )
}

export const command = <Message>(input: CommandRootProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, commandRoot),
      ...dataAttr<Message>(
        'filter',
        input.model?.manualFilter === true ? 'manual' : undefined,
      ),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
    ],
    input.children,
  )
}

export const commandDialog = <Message>(
  input: CommandDialogProps<Message>,
): Html => {
  const h = html<Message>()

  return h.dialog(
    [
      ...basecoatAttrs<Message>(input, commandDialogRoot),
      ...(input.model.open ? [h.Attribute('open', '')] : []),
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
      ...mappedAttr<Message>(input.onMessage, commandClosed(), h.OnCancel),
      ...mappedAttr<Message>(input.onMessage, commandClosed(), h.OnClick),
      ...mappedKeydownAttr<Message>(input.onMessage),
    ],
    input.children,
  )
}

export const commandTrigger = <Message>(
  input: CommandTriggerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...mappedAttr<Message>(input.onMessage, commandOpened(), h.OnClick),
    ],
    input.children,
  )
}

export const commandInput = <Message>(
  input: CommandInputProps<Message>,
): Html => {
  const h = html<Message>()

  return h.header(
    basecoatAttrs<Message>({}),
    [
      searchIcon<Message>(),
      h.input([
        ...basecoatAttrs<Message>(input),
        h.Type('text'),
        h.Role('combobox'),
        h.AriaAutocomplete('list'),
        h.AriaExpanded(true),
        ...(input.model.activeValue === null
          ? []
          : [h.AriaActiveDescendant(input.model.activeValue)]),
        ...optionalStringAttr<Message>(input.id, h.Id),
        ...optionalStringAttr<Message>(input.menuId, h.AriaControls),
        ...optionalStringAttr<Message>(input.placeholder, h.Placeholder),
        ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
        ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
        h.Autocomplete('off'),
        h.Autocorrect('off'),
        h.Spellcheck(false),
        h.Value(input.model.query),
        ...(input.onMessage === undefined
          ? []
          : [h.OnInput(value => input.onMessage?.(commandQueryChanged(value)) as Message)]),
        ...mappedKeydownAttr<Message>(input.onMessage),
      ]),
    ],
  )
}

export const commandMenu = <Message>(
  input: CommandMenuProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('menu'),
      h.AriaOrientation('vertical'),
      ...optionalStringAttr<Message>(input.id, h.Id),
      ...dataAttr<Message>('empty', input.empty),
    ],
    input.children,
  )
}

export const commandGroup = <Message>(
  input: CommandGroupProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('group'),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
    ],
    input.children,
  )
}

export const commandHeading = <Message>(
  input: CommandHeadingProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('heading'),
      ...optionalStringAttr<Message>(input.id, h.Id),
    ],
    input.children,
  )
}

export const commandSeparator = <Message>(
  input: BasecoatAttrs<Message> = {},
): Html => {
  const h = html<Message>()

  return h.hr([...basecoatAttrs<Message>(input), h.Role('separator')])
}

export const commandItem = <Message>(
  input: CommandItemProps<Message>,
): Html => {
  const h = html<Message>()
  const active = input.model.activeValue === input.value
  const selected = input.model.selectedValue === input.value
  const hidden = !commandVisibleItems(input.model).some(item => item.value === input.value)
  const attrs = [
    ...basecoatAttrs<Message>(input, active ? 'active' : null),
    h.Role('menuitem'),
    h.Id(input.value),
    h.DataAttribute('value', input.value),
    h.DataAttribute('filter', input.filter ?? input.value),
    h.AriaHidden(hidden),
    h.AriaSelected(selected),
    ...(selected ? [h.DataAttribute('selected', 'true')] : []),
    ...(input.keywords === undefined
      ? []
      : [h.DataAttribute('keywords', input.keywords.join(' '))]),
    ...(input.force === true ? [h.DataAttribute('force', '')] : []),
    ...(input.keepOpen === true ? [h.DataAttribute('keep-command-open', '')] : []),
    ...(input.disabled === true
      ? [h.AriaDisabled(true), h.DataAttribute('disabled', 'true')]
      : []),
    ...mappedAttr<Message>(
      input.onMessage,
      commandFocused(input.value),
      h.OnMouseMove,
    ),
    ...mappedAttr<Message>(
      input.onMessage,
      commandSelected(input.value, input.keepOpen === true),
      h.OnClick,
    ),
  ]

  return input.href === undefined
    ? h.div(attrs, input.children)
    : h.a([...attrs, h.Href(input.href)], input.children)
}

export const commandShortcut = <Message>(
  input: CommandInlineProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [...basecoatAttrs<Message>(input), h.DataAttribute('shortcut', '')],
    input.children,
  )
}

export const commandIndicator = <Message>(
  input: CommandInlineProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [...basecoatAttrs<Message>(input), h.DataAttribute('indicator', '')],
    input.children,
  )
}

export const commandView = <Message>(
  input: CommandViewProps<Message>,
): Html => {
  const menuChildren = input.groups.flatMap((group, groupIndex) => {
    const headingId = group.id ?? `command-group-${groupIndex}`
    const groupChildren = [
      ...(group.heading === undefined
        ? []
        : [commandHeading<Message>({ id: headingId, children: group.heading })]),
      ...group.items.map(item => commandItem<Message>({
        model: input.model,
        value: item.value,
        ...(item.filter === undefined ? {} : { filter: item.filter }),
        ...(item.keywords === undefined ? {} : { keywords: item.keywords }),
        ...(item.disabled === undefined ? {} : { disabled: item.disabled }),
        ...(item.force === undefined ? {} : { force: item.force }),
        ...(item.href === undefined ? {} : { href: item.href }),
        ...(item.keepOpen === undefined ? {} : { keepOpen: item.keepOpen }),
        onMessage: input.toMessage,
        children: [
          ...item.children,
          ...(item.shortcut === undefined
            ? []
            : [commandShortcut<Message>({ children: item.shortcut })]),
          ...(item.indicator === undefined
            ? []
            : [commandIndicator<Message>({ children: item.indicator })]),
        ],
      })),
    ]

    return [
      commandGroup<Message>({
        ...(group.heading === undefined ? {} : { labelledBy: headingId }),
        children: groupChildren,
      }),
      ...(groupIndex === input.groups.length - 1
        ? []
        : [commandSeparator<Message>()]),
    ]
  })

  return command<Message>({
    model: input.model,
    ...(input.ariaLabel === undefined ? {} : { ariaLabel: input.ariaLabel }),
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
    children: [
      commandInput<Message>({
        model: input.model,
        onMessage: input.toMessage,
        ...(input.inputId === undefined ? {} : { id: input.inputId }),
        ...(input.menuId === undefined ? {} : { menuId: input.menuId }),
        ...(input.placeholder === undefined ? {} : { placeholder: input.placeholder }),
      }),
      commandMenu<Message>({
        ...(input.menuId === undefined ? {} : { id: input.menuId }),
        ...(input.empty === undefined ? {} : { empty: input.empty }),
        children: menuChildren,
      }),
    ],
  })
}
