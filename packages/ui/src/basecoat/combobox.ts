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

export type ComboboxFormat = 'value' | 'object'

export type ComboboxSelected = Readonly<{
  value: string
  label: string
}>

export type ComboboxOption = ComboboxSelected & Readonly<{
  disabled?: boolean
  filter?: string
  keywords?: ReadonlyArray<string>
  force?: boolean
}>

export type ComboboxModel = Readonly<{
  options: ReadonlyArray<ComboboxOption>
  multiple: boolean
  open: boolean
  inputValue: string
  selected: ReadonlyArray<ComboboxSelected>
  activeValue: string | null
  visibleValues: ReadonlyArray<string>
  closeOnSelect: boolean
  autoHighlight: boolean
  manualFilter: boolean
  format: ComboboxFormat
}>

export type ComboboxInit = Readonly<{
  options: ReadonlyArray<ComboboxOption>
  multiple?: boolean
  open?: boolean
  inputValue?: string
  selected?: ReadonlyArray<string | ComboboxSelected> | string | ComboboxSelected | null
  closeOnSelect?: boolean
  autoHighlight?: boolean
  manualFilter?: boolean
  format?: ComboboxFormat
}>

export type ComboboxKey =
  | 'ArrowDown'
  | 'ArrowUp'
  | 'Enter'
  | 'Home'
  | 'End'
  | 'Escape'
  | 'Backspace'

export type ComboboxMessage =
  | Readonly<{ _tag: 'ComboboxOpened' }>
  | Readonly<{ _tag: 'ComboboxClosed'; focusInput?: boolean }>
  | Readonly<{ _tag: 'ComboboxTriggerClicked' }>
  | Readonly<{ _tag: 'ComboboxInputFocused' }>
  | Readonly<{ _tag: 'ComboboxInputClicked' }>
  | Readonly<{ _tag: 'ComboboxInputChanged'; value: string }>
  | Readonly<{ _tag: 'ComboboxClearClicked' }>
  | Readonly<{ _tag: 'ComboboxOptionFocused'; value: string }>
  | Readonly<{ _tag: 'ComboboxOptionSelected'; value: string }>
  | Readonly<{ _tag: 'ComboboxOptionToggled'; value: string }>
  | Readonly<{ _tag: 'ComboboxOptionRemoved'; value: string }>
  | Readonly<{ _tag: 'ComboboxKeyDown'; key: ComboboxKey }>

export type ComboboxRootProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
  closeOnSelect?: boolean
  autoHighlight?: boolean
  manualFilter?: boolean
  format?: ComboboxFormat
}>

export type ComboboxHiddenInputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  name?: string
  value: string
}>

export type ComboboxInputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  name?: string
  value?: string
  placeholder?: string
  disabled?: boolean
  required?: boolean
  expanded?: boolean
  controlsId?: string
  activeDescendantId?: string
  autocomplete?: 'list' | 'both' | 'inline' | 'none'
  onInput?: (value: string) => Message
  onFocus?: Message
  onClick?: Message
  onKeyDown?: (key: ComboboxKey) => Message
}>

export type ComboboxTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children?: BasecoatChildren
  expanded?: boolean
  controlsId?: string
  disabled?: boolean
  onClick?: Message
}>

export type ComboboxClearProps<Message> = BasecoatAttrs<Message> & Readonly<{
  hidden?: boolean
  ariaLabel?: string
  onClick?: Message
}>

export type ComboboxPopoverProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  children: ReadonlyArray<Html>
  hidden?: boolean
}>

export type ComboboxListboxProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  children: ReadonlyArray<Html>
  multiple?: boolean
  labelledBy?: string
  emptyLabel?: string
}>

export type ComboboxOptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  value: string
  label?: string
  children: BasecoatChildren
  selected?: boolean
  disabled?: boolean
  hidden?: boolean
  active?: boolean
  filter?: string
  keywords?: ReadonlyArray<string>
  force?: boolean
  onClick?: Message
  onMouseMove?: Message
}>

export type ComboboxChipsProps<Message> = BasecoatAttrs<Message> & Readonly<{
  children: ReadonlyArray<Html>
}>

export type ComboboxChipProps<Message> = BasecoatAttrs<Message> & Readonly<{
  value: string
  label: string
  onRemove?: Message
}>

export type ComboboxViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: ComboboxModel
  toMessage: (message: ComboboxMessage) => Message
  id?: string
  name?: string
  placeholder?: string
  listboxId?: string
  popoverId?: string
  triggerId?: string
  inputId?: string
  clearable?: boolean
  trigger?: boolean
  chips?: boolean
  emptyLabel?: string
}>

const comboboxRoot = basecoatClass('combobox')
const comboboxChipsRoot = basecoatClass('combobox-chips')
const comboboxChipRoot = basecoatClass('combobox-chip')
const comboboxChipRemoveRoot = basecoatClass('combobox-chip-remove btn')
const activeOptionClass = basecoatClass('active')

const enabledOptions = (
  options: ReadonlyArray<ComboboxOption>,
): ReadonlyArray<ComboboxOption> =>
  options.filter(option => option.disabled !== true)

const findOption = (
  options: ReadonlyArray<ComboboxOption>,
  value: string,
): ComboboxOption | undefined =>
  options.find(option => option.value === value)

const normalizeEntry = (
  entry: string | ComboboxSelected | null | undefined,
  options: ReadonlyArray<ComboboxOption>,
): ComboboxSelected | null => {
  if (entry === null || entry === undefined) {
    return null
  }

  const value = typeof entry === 'string' ? entry : String(entry.value)
  if (value.length === 0) {
    return null
  }

  const option = findOption(options, value)
  return {
    value,
    label: option?.label ?? (typeof entry === 'string' ? value : String(entry.label)),
  }
}

const uniqueEntries = (
  entries: ReadonlyArray<ComboboxSelected>,
): ReadonlyArray<ComboboxSelected> => {
  const seen = new Set<string>()
  const next: Array<ComboboxSelected> = []

  for (const entry of entries) {
    if (seen.has(entry.value)) {
      continue
    }
    seen.add(entry.value)
    next.push(entry)
  }

  return next
}

const normalizeSelected = (
  selected: ComboboxInit['selected'],
  options: ReadonlyArray<ComboboxOption>,
  multiple: boolean,
): ReadonlyArray<ComboboxSelected> => {
  const entries = Array.isArray(selected)
    ? selected
    : selected === null || selected === undefined
      ? []
      : [selected]
  const normalized = entries
    .map(entry => normalizeEntry(entry, options))
    .filter((entry): entry is ComboboxSelected => entry !== null)

  return multiple ? uniqueEntries(normalized) : normalized.slice(0, 1)
}

const optionMatches = (option: ComboboxOption, search: string): boolean => {
  if (option.force === true) {
    return true
  }

  if (search.length === 0) {
    return true
  }

  const haystack = (option.filter ?? option.label).trim().toLowerCase()
  const keywords = (option.keywords ?? []).map(keyword => keyword.toLowerCase())

  return haystack.includes(search) ||
    keywords.some(keyword => keyword.includes(search))
}

const filterVisibleValues = (
  options: ReadonlyArray<ComboboxOption>,
  inputValue: string,
  manualFilter: boolean,
): ReadonlyArray<string> => {
  const enabled = enabledOptions(options)
  if (manualFilter) {
    return enabled.map(option => option.value)
  }

  const search = inputValue.trim().toLowerCase()
  return enabled
    .filter(option => optionMatches(option, search))
    .map(option => option.value)
}

const normalizeActiveValue = (
  activeValue: string | null,
  visibleValues: ReadonlyArray<string>,
  autoHighlight: boolean,
): string | null => {
  if (activeValue !== null && visibleValues.includes(activeValue)) {
    return activeValue
  }

  return autoHighlight ? visibleValues[0] ?? null : null
}

const withVisibleState = (
  model: ComboboxModel,
  activeValue: string | null = model.activeValue,
  searchValue = model.inputValue,
): ComboboxModel => {
  const visibleValues = filterVisibleValues(
    model.options,
    searchValue,
    model.manualFilter,
  )

  return {
    ...model,
    visibleValues,
    activeValue: normalizeActiveValue(
      activeValue,
      visibleValues,
      model.autoHighlight,
    ),
  }
}

export const comboboxInit = (input: ComboboxInit): ComboboxModel => {
  const multiple = input.multiple === true
  const selected = normalizeSelected(input.selected, input.options, multiple)
  const inputValue = multiple
    ? ''
    : input.inputValue ?? selected[0]?.label ?? ''
  const model: ComboboxModel = {
    options: input.options,
    multiple,
    open: input.open === true,
    inputValue,
    selected,
    activeValue: null,
    visibleValues: [],
    closeOnSelect: input.closeOnSelect === true,
    autoHighlight: input.autoHighlight === true,
    manualFilter: input.manualFilter === true,
    format: input.format ?? 'value',
  }

  return withVisibleState(model, model.activeValue, selected.length > 0 && !multiple ? '' : inputValue)
}

export const comboboxSerializedValue = (model: ComboboxModel): string => {
  if (model.format === 'object') {
    return JSON.stringify(
      model.multiple ? model.selected : model.selected[0] ?? null,
    )
  }

  const values = model.selected.map(entry => entry.value)
  return model.multiple ? JSON.stringify(values) : values[0] ?? ''
}

export const comboboxCanonicalValue = (
  model: ComboboxModel,
): string | ReadonlyArray<string> =>
  model.multiple ? model.selected.map(entry => entry.value) : model.selected[0]?.value ?? ''

export const comboboxSelectedDetail = (
  model: ComboboxModel,
): ComboboxSelected | ReadonlyArray<ComboboxSelected> | null =>
  model.multiple ? model.selected : model.selected[0] ?? null

const setSelected = (
  model: ComboboxModel,
  selected: ReadonlyArray<ComboboxSelected>,
): ComboboxModel => {
  const normalized = model.multiple ? uniqueEntries(selected) : selected.slice(0, 1)
  return withVisibleState({
    ...model,
    selected: normalized,
    inputValue: model.multiple ? '' : normalized[0]?.label ?? '',
  })
}

const selectValue = (model: ComboboxModel, value: string): ComboboxModel => {
  const option = findOption(model.options, value)
  if (option === undefined || option.disabled === true) {
    return model
  }

  const entry = { value: option.value, label: option.label }

  if (model.multiple) {
    const selected = [
      ...model.selected.filter(candidate => candidate.value !== entry.value),
      entry,
    ]
    return setSelected({
      ...model,
      open: model.closeOnSelect ? false : model.open,
      activeValue: entry.value,
    }, selected)
  }

  return setSelected({
    ...model,
    open: false,
    activeValue: entry.value,
  }, [entry])
}

const removeValue = (model: ComboboxModel, value: string): ComboboxModel =>
  setSelected(model, model.selected.filter(entry => entry.value !== value))

const toggleValue = (model: ComboboxModel, value: string): ComboboxModel =>
  model.selected.some(entry => entry.value === value)
    ? removeValue(model, value)
    : selectValue(model, value)

const moveActiveValue = (
  model: ComboboxModel,
  key: Extract<ComboboxKey, 'ArrowDown' | 'ArrowUp' | 'Home' | 'End'>,
): ComboboxModel => {
  if (model.visibleValues.length === 0) {
    return { ...model, activeValue: null }
  }

  const current = model.activeValue === null
    ? -1
    : model.visibleValues.indexOf(model.activeValue)
  let next = current

  if (key === 'ArrowDown') {
    next = Math.min(current + 1, model.visibleValues.length - 1)
  } else if (key === 'ArrowUp') {
    next = current <= 0 ? 0 : current - 1
  } else if (key === 'Home') {
    next = 0
  } else {
    next = model.visibleValues.length - 1
  }

  return { ...model, activeValue: model.visibleValues[next] ?? null }
}

export const comboboxUpdate = (
  model: ComboboxModel,
  message: ComboboxMessage,
): ComboboxModel => {
  switch (message._tag) {
    case 'ComboboxOpened':
    case 'ComboboxInputFocused':
    case 'ComboboxInputClicked':
      return withVisibleState(
        { ...model, open: true },
        model.activeValue,
        !model.multiple && model.selected[0]?.label === model.inputValue
          ? ''
          : model.inputValue,
      )
    case 'ComboboxClosed':
      return { ...model, open: false, activeValue: null }
    case 'ComboboxTriggerClicked':
      return model.open
        ? { ...model, open: false, activeValue: null }
        : withVisibleState(
            { ...model, open: true, inputValue: model.multiple ? '' : model.inputValue },
            model.activeValue,
            !model.multiple && model.selected[0]?.label === model.inputValue
              ? ''
              : model.inputValue,
          )
    case 'ComboboxInputChanged':
      return withVisibleState({
        ...model,
        open: true,
        inputValue: message.value,
        selected: model.multiple ? model.selected : [],
      })
    case 'ComboboxClearClicked':
      return withVisibleState({
        ...model,
        inputValue: '',
        selected: [],
        activeValue: null,
      })
    case 'ComboboxOptionFocused':
      return model.visibleValues.includes(message.value)
        ? { ...model, activeValue: message.value }
        : model
    case 'ComboboxOptionSelected':
      return selectValue(model, message.value)
    case 'ComboboxOptionToggled':
      return model.multiple
        ? toggleValue(model, message.value)
        : selectValue(model, message.value)
    case 'ComboboxOptionRemoved':
      return removeValue(model, message.value)
    case 'ComboboxKeyDown':
      switch (message.key) {
        case 'Escape':
          return { ...model, open: false, activeValue: null }
        case 'Backspace': {
          if (!model.multiple || model.inputValue.length > 0) {
            return model
          }
          const last = model.selected[model.selected.length - 1]
          return last === undefined ? model : removeValue(model, last.value)
        }
        case 'Enter':
          return model.open && model.activeValue !== null
            ? model.multiple
              ? toggleValue(model, model.activeValue)
              : selectValue(model, model.activeValue)
            : model
        case 'ArrowDown':
        case 'ArrowUp':
        case 'Home':
        case 'End':
          return moveActiveValue(
            model.open ? model : withVisibleState({ ...model, open: true }),
            message.key,
          )
      }
  }
}

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  value === undefined ? [] : [attr(value)]

const handledKeys = new Set<string>([
  'ArrowDown',
  'ArrowUp',
  'Enter',
  'Home',
  'End',
  'Escape',
  'Backspace',
])

export const comboboxKeyMessage = (
  key: string,
  _modifiers?: KeyboardModifiers,
): ComboboxKey | null =>
  handledKeys.has(key) ? key as ComboboxKey : null

export const combobox = <Message>(
  input: ComboboxRootProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, comboboxRoot),
      ...dataAttr<Message>('close-on-select', input.closeOnSelect === true ? 'true' : undefined),
      ...dataAttr<Message>('auto-highlight', input.autoHighlight === true ? 'true' : undefined),
      ...dataAttr<Message>('filter', input.manualFilter === true ? 'manual' : undefined),
      ...dataAttr<Message>('format', input.format === 'object' ? 'object' : undefined),
    ],
    input.children,
  )
}

export const comboboxHiddenInput = <Message>(
  input: ComboboxHiddenInputProps<Message>,
): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input),
    h.Type('hidden'),
    h.Value(input.value),
    ...optionalStringAttr<Message>(input.name, h.Name),
  ])
}

export const comboboxInput = <Message>(
  input: ComboboxInputProps<Message>,
): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input),
    h.Type('text'),
    h.Role('combobox'),
    h.AriaExpanded(input.expanded === true),
    h.AriaAutocomplete(input.autocomplete ?? 'list'),
    h.Autocomplete('off'),
    ...(input.id === undefined ? [] : [h.Id(input.id)]),
    ...(input.name === undefined ? [] : [h.Name(input.name)]),
    ...(input.value === undefined ? [] : [h.Value(input.value)]),
    ...(input.placeholder === undefined ? [] : [h.Placeholder(input.placeholder)]),
    ...(input.controlsId === undefined ? [] : [h.AriaControls(input.controlsId)]),
    ...(input.activeDescendantId === undefined
      ? []
      : [h.AriaActiveDescendant(input.activeDescendantId)]),
    ...(input.disabled === true ? [h.Disabled(true)] : []),
    ...(input.required === true ? [h.Required(true)] : []),
    ...(input.onInput === undefined ? [] : [h.OnInput(input.onInput)]),
    ...(input.onFocus === undefined ? [] : [h.OnFocus(input.onFocus)]),
    ...(input.onClick === undefined ? [] : [h.OnClick(input.onClick)]),
    ...(input.onKeyDown === undefined
      ? []
      : [
          h.OnKeyDownPreventDefault((key, modifiers) => {
            const next = comboboxKeyMessage(key, modifiers)
            return next === null || input.onKeyDown === undefined
              ? Option.none()
              : Option.some(input.onKeyDown(next))
          }),
        ]),
  ])
}

const chevronIcon = <Message>(): Html => {
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
      h.Class('combobox-trigger-icon'),
    ],
    [h.path([h.D('m6 9 6 6 6-6')], [])],
  )
}

const xIcon = <Message>(): Html => {
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
    [
      h.path([h.D('M18 6 6 18')], []),
      h.path([h.D('m6 6 12 12')], []),
    ],
  )
}

export const comboboxTrigger = <Message>(
  input: ComboboxTriggerProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.AriaHasPopup('listbox'),
      h.AriaExpanded(input.expanded === true),
      ...(input.controlsId === undefined ? [] : [h.AriaControls(input.controlsId)]),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...(input.onClick === undefined || input.disabled === true
        ? []
        : [h.OnClick(input.onClick)]),
    ],
    input.children ?? [chevronIcon<Message>()],
  )
}

export const comboboxClear = <Message>(
  input: ComboboxClearProps<Message>,
): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.DataAttribute('clear', ''),
      h.AriaLabel(input.ariaLabel ?? 'Clear selection'),
      ...(input.hidden !== false ? [h.Hidden(true)] : []),
      ...(input.onClick === undefined ? [] : [h.OnClick(input.onClick)]),
    ],
    [xIcon<Message>()],
  )
}

export const comboboxPopover = <Message>(
  input: ComboboxPopoverProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('popover', ''),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      h.AriaHidden(input.hidden !== false),
    ],
    input.children,
  )
}

export const comboboxListbox = <Message>(
  input: ComboboxListboxProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('listbox'),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.labelledBy === undefined ? [] : [h.AriaLabelledBy(input.labelledBy)]),
      ...(input.multiple === true ? [h.AriaMultiSelectable(true)] : []),
      ...(input.emptyLabel === undefined
        ? []
        : [h.DataAttribute('empty', input.emptyLabel)]),
    ],
    input.children,
  )
}

export const comboboxOption = <Message>(
  input: ComboboxOptionProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(
        input,
        input.active === true ? activeOptionClass : null,
      ),
      h.Role('option'),
      h.DataAttribute('value', input.value),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.label === undefined ? [] : [h.DataAttribute('label', input.label)]),
      ...(input.filter === undefined ? [] : [h.DataAttribute('filter', input.filter)]),
      ...(input.keywords === undefined || input.keywords.length === 0
        ? []
        : [h.DataAttribute('keywords', input.keywords.join(' '))]),
      ...(input.force === true ? [h.DataAttribute('force', 'true')] : []),
      ...(input.selected === true ? [h.AriaSelected(true)] : []),
      ...(input.disabled === true ? [h.AriaDisabled(true)] : []),
      ...(input.hidden === true ? [h.AriaHidden(true)] : []),
      ...(input.onClick === undefined || input.disabled === true
        ? []
        : [h.OnClick(input.onClick)]),
      ...(input.onMouseMove === undefined || input.disabled === true
        ? []
        : [h.OnMouseMove(input.onMouseMove)]),
    ],
    input.children,
  )
}

export const comboboxChips = <Message>(
  input: ComboboxChipsProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    basecoatAttrs<Message>(input, comboboxChipsRoot),
    input.children,
  )
}

export const comboboxChip = <Message>(
  input: ComboboxChipProps<Message>,
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...basecoatAttrs<Message>(input, comboboxChipRoot),
      h.DataAttribute('value', input.value),
    ],
    [
      h.span([], [input.label]),
      h.button(
        [
          ...basecoatAttrs<Message>(
            input.className === undefined ? {} : { className: input.className },
            comboboxChipRemoveRoot,
          ),
          h.Type('button'),
          h.DataAttribute('variant', 'ghost'),
          h.DataAttribute('size', 'icon-xs'),
          h.AriaLabel(`Remove ${input.label}`),
          ...(input.onRemove === undefined ? [] : [h.OnClick(input.onRemove)]),
        ],
        [xIcon<Message>()],
      ),
    ],
  )
}

const selectedLabel = (model: ComboboxModel, placeholder: string | undefined): string =>
  model.multiple
    ? model.selected.map(entry => entry.label).join(', ')
    : model.selected[0]?.label ?? placeholder ?? ''

export const comboboxView = <Message>(
  input: ComboboxViewProps<Message>,
): Html => {
  const h = html<Message>()
  const id = input.id ?? 'combobox'
  const listboxId = input.listboxId ?? `${id}-listbox`
  const popoverId = input.popoverId ?? `${id}-popover`
  const inputId = input.inputId ?? `${id}-input`
  const activeId = input.model.activeValue === null
    ? undefined
    : `${id}-option-${input.model.activeValue}`
  const hasSelection = input.model.selected.length > 0
  const clearVisible = input.clearable === true &&
    (hasSelection || input.model.inputValue.length > 0)
  const inputNode = comboboxInput<Message>({
    id: inputId,
    value: input.model.inputValue,
    expanded: input.model.open,
    controlsId: listboxId,
    onFocus: input.toMessage({ _tag: 'ComboboxInputFocused' }),
    onClick: input.toMessage({ _tag: 'ComboboxInputClicked' }),
    onInput: value => input.toMessage({
      _tag: 'ComboboxInputChanged',
      value,
    }),
    onKeyDown: key => input.toMessage({
      _tag: 'ComboboxKeyDown',
      key,
    }),
    ...(input.placeholder === undefined ? {} : { placeholder: input.placeholder }),
    ...(activeId === undefined ? {} : { activeDescendantId: activeId }),
  })
  const field = input.model.multiple && input.chips !== false
    ? comboboxChips<Message>({
        children: [
          ...input.model.selected.map(entry =>
            comboboxChip<Message>({
              value: entry.value,
              label: entry.label,
              onRemove: input.toMessage({
                _tag: 'ComboboxOptionRemoved',
                value: entry.value,
              }),
            }),
          ),
          inputNode,
        ],
      })
    : inputNode
  const options = input.model.options.map(option =>
    comboboxOption<Message>({
      id: `${id}-option-${option.value}`,
      value: option.value,
      label: option.label,
      selected: input.model.selected.some(entry => entry.value === option.value),
      hidden: !input.model.visibleValues.includes(option.value) && option.force !== true,
      active: input.model.activeValue === option.value,
      onClick: input.toMessage({
        _tag: input.model.multiple ? 'ComboboxOptionToggled' : 'ComboboxOptionSelected',
        value: option.value,
      }),
      onMouseMove: input.toMessage({
        _tag: 'ComboboxOptionFocused',
        value: option.value,
      }),
      children: [option.label],
      ...(option.disabled === undefined ? {} : { disabled: option.disabled }),
      ...(option.filter === undefined ? {} : { filter: option.filter }),
      ...(option.keywords === undefined ? {} : { keywords: option.keywords }),
      ...(option.force === undefined ? {} : { force: option.force }),
    }),
  )
  const children: Array<Html> = [
    comboboxHiddenInput<Message>({
      value: comboboxSerializedValue(input.model),
      ...(input.name === undefined ? {} : { name: input.name }),
    }),
    field,
  ]

  if (input.clearable === true) {
    children.push(comboboxClear<Message>({
      hidden: !clearVisible,
      onClick: input.toMessage({ _tag: 'ComboboxClearClicked' }),
    }))
  }

  if (input.trigger === true) {
    children.push(comboboxTrigger<Message>({
      expanded: input.model.open,
      controlsId: listboxId,
      onClick: input.toMessage({ _tag: 'ComboboxTriggerClicked' }),
      children: [
        h.span(
          [
            h.DataAttribute('value', ''),
            ...(input.placeholder === undefined
              ? []
              : [h.DataAttribute('placeholder', input.placeholder)]),
          ],
          [selectedLabel(input.model, input.placeholder)],
        ),
        chevronIcon<Message>(),
      ],
    }))
  }

  children.push(comboboxPopover<Message>({
    id: popoverId,
    hidden: !input.model.open,
    children: [
      comboboxListbox<Message>({
        id: listboxId,
        multiple: input.model.multiple,
        children: options,
        ...(input.triggerId === undefined ? {} : { labelledBy: input.triggerId }),
        ...(input.emptyLabel === undefined ? {} : { emptyLabel: input.emptyLabel }),
      }),
    ],
  }))

  return combobox<Message>({
    children,
    closeOnSelect: input.model.closeOnSelect,
    autoHighlight: input.model.autoHighlight,
    manualFilter: input.model.manualFilter,
    format: input.model.format,
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
  })
}
